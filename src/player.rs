//! In-process audio via librespot: streams, decodes and plays Spotify tracks (Premium only).

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use librespot_core::authentication::Credentials;
use librespot_core::cache::Cache;
use librespot_core::config::SessionConfig;
use librespot_core::session::Session;
use librespot_core::spotify_uri::SpotifyUri;
use librespot_playback::audio_backend::{self, Sink, SinkResult};
use librespot_playback::convert::Converter;
use librespot_playback::decoder::AudioPacket;
use librespot_playback::config::{AudioFormat, Bitrate as LsBitrate, PlayerConfig};
use librespot_playback::mixer::{self, Mixer, MixerConfig};
use librespot_playback::player::{Player, PlayerEvent};
use tokio::sync::mpsc::UnboundedSender;

use crate::config::{self, Bitrate, PlaybackConfig};

/// Player state changes, forwarded to the UI.
#[derive(Debug, Clone)]
pub enum PlayerUpdate {
    Playing { uri: String, position_ms: u32 },
    Paused { uri: String, position_ms: u32 },
    Position { uri: String, position_ms: u32 },
    EndOfTrack { uri: String },
    Unavailable { uri: String },
    Stopped,
}

pub struct Audio {
    session: std::sync::Mutex<Session>,
    player: Arc<Player>,
    mixer: Arc<dyn Mixer>,
}

impl std::fmt::Debug for Audio {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Audio")
    }
}

impl Audio {
    pub async fn connect(settings: PlaybackConfig, events: UnboundedSender<PlayerUpdate>) -> Result<Self> {
        let session = new_session(&settings).await?;

        let mixer = mixer::find(None).context("no software mixer")?(MixerConfig::default())
            .map_err(|e| anyhow!("audio mixer: {e}"))?;
        mixer.set_volume(volume_to_u16(settings.volume));

        let player_config = PlayerConfig {
            bitrate: match settings.bitrate {
                Bitrate::Low => LsBitrate::Bitrate96,
                Bitrate::Normal => LsBitrate::Bitrate160,
                Bitrate::High => LsBitrate::Bitrate320,
            },
            gapless: settings.gapless,
            normalisation: settings.normalisation,
            position_update_interval: Some(Duration::from_millis(500)),
            ..PlayerConfig::default()
        };
        let backend = audio_backend::find(None).context("no audio output backend")?;
        let player = Player::new(player_config, session.clone(), mixer.get_soft_volume(), move || {
            Box::new(Tapped(backend(None, AudioFormat::default())))
        });

        let mut channel = player.get_player_event_channel();
        tokio::spawn(async move {
            while let Some(event) = channel.recv().await {
                if let Some(update) = map_event(event)
                    && events.send(update).is_err() {
                        break;
                    }
            }
        });

        Ok(Self { session: std::sync::Mutex::new(session), player, mixer })
    }

    /// Reconnects the audio session if Spotify dropped it (sleep, network change, ...).
    pub async fn ensure_connected(&self, settings: &PlaybackConfig) -> Result<()> {
        let dropped = self.session.lock().expect("session lock").is_invalid();
        if dropped {
            log::info!("Spotify session dropped; reconnecting");
            let session = new_session(settings).await?;
            self.player.set_session(session.clone());
            *self.session.lock().expect("session lock") = session;
        }
        Ok(())
    }

    pub fn load(&self, uri: &str, start_playing: bool, position_ms: u32) -> Result<()> {
        let uri = SpotifyUri::from_uri(uri).map_err(|e| anyhow!("bad track URI {uri}: {e}"))?;
        self.player.load(uri, start_playing, position_ms);
        Ok(())
    }

    pub fn preload(&self, uri: &str) {
        if let Ok(uri) = SpotifyUri::from_uri(uri) {
            self.player.preload(uri);
        }
    }

    pub fn play(&self) {
        self.player.play();
    }

    pub fn pause(&self) {
        self.player.pause();
    }

    pub fn stop(&self) {
        self.player.stop();
    }

    pub fn seek(&self, position_ms: u32) {
        self.player.seek(position_ms);
    }

    /// Spotify's own (time-synced) lyrics for a track, as raw JSON.
    pub async fn spotify_lyrics(&self, track_id: &str) -> Result<Vec<u8>> {
        let id = librespot_core::spotify_id::SpotifyId::from_base62(track_id).map_err(|e| anyhow!("bad track id: {e}"))?;
        let session = self.session.lock().expect("session lock").clone();
        let bytes = session.spclient().get_lyrics(&id).await.map_err(|e| anyhow!("Spotify lyrics: {e}"))?;
        Ok(bytes.to_vec())
    }

    /// A playlist's song links, read through the playback login (see `spotify::lists`).
    pub async fn playlist_uris(&self, settings: &PlaybackConfig, playlist_id: &str) -> Result<Vec<String>> {
        self.ensure_connected(settings).await?;
        let session = self.session.lock().expect("session lock").clone();
        crate::spotify::lists::playlist_uris(&session, playlist_id).await
    }

    /// Song details for up to `spotify::lists::CHUNK` song links, through the playback login.
    pub async fn tracks(&self, uris: &[String]) -> Result<Vec<crate::spotify::api::Track>> {
        let session = self.session.lock().expect("session lock").clone();
        crate::spotify::lists::tracks(&session, uris).await
    }

    /// A Web API token from the playback login. Spotify's own client isn't held to the limits of
    /// developer apps, so this can read playlists your app gets "403 Forbidden" for.
    pub async fn web_token(&self, settings: &PlaybackConfig) -> Result<String> {
        self.ensure_connected(settings).await?;
        let session = self.session.lock().expect("session lock").clone();
        // login5 is what Spotify's own apps use now. The older keymaster route refuses these
        // scopes ("Invalid request"), so it's only a fallback.
        let token = match session.login5().auth_token().await {
            Ok(token) => token,
            Err(login5) => session
                .token_provider()
                .get_token("playlist-read-private,playlist-read-collaborative,user-library-read")
                .await
                .map_err(|e| anyhow!("playback login token: {login5}; keymaster: {e}"))?,
        };
        Ok(token.access_token)
    }

    /// 0-100
    pub fn set_volume(&self, volume: u8) {
        self.mixer.set_volume(volume_to_u16(volume));
    }
}

async fn new_session(settings: &PlaybackConfig) -> Result<Session> {
    let config = SessionConfig::default();
    let cache = new_cache(settings)?;
    if let Some(saved) = cache.credentials() {
        let session = Session::new(config.clone(), Some(cache));
        match session.connect(saved, true).await {
            Ok(()) => return Ok(session),
            // The saved playback login stopped working (password change, signed out everywhere).
            Err(e) => log::warn!("saved playback login rejected ({e}); logging in again"),
        }
        forget_playback_login();
    }
    let credentials = playback_login(&config.client_id).await?;
    let session = Session::new(config, Some(new_cache(settings)?));
    session
        .connect(credentials, true)
        .await
        .map_err(|e| anyhow!("connecting to Spotify for playback (Premium is required): {e}"))?;
    Ok(session)
}

fn new_cache(settings: &PlaybackConfig) -> Result<Cache> {
    let cache_dir = config::cache_dir()?;
    let audio_dir = (settings.audio_cache_mb > 0).then(|| cache_dir.join("audio"));
    let size_limit = (settings.audio_cache_mb > 0).then_some(settings.audio_cache_mb * 1024 * 1024);
    Cache::new(Some(cache_dir.join("session")), None, audio_dir, size_limit).map_err(|e| anyhow!("audio cache: {e}"))
}

/// Spotify only streams audio to its own client IDs, so a token from your developer app (which
/// the Web API is happy with) connects but every track comes back unavailable. Playback logs in
/// once more with the ID Spotify's desktop app and librespot use; librespot then saves a
/// reusable login in the cache, so this browser step only happens the first time.
async fn playback_login(client_id: &str) -> Result<Credentials> {
    let client = librespot_oauth::OAuthClientBuilder::new(client_id, PLAYBACK_REDIRECT, vec!["streaming"])
        .open_in_browser()
        .with_custom_message(crate::spotify::auth::LOGIN_PAGE)
        .build()
        .map_err(|e| anyhow!("playback login setup failed: {e}"))?;
    // librespot's "async" login still waits for the browser on a blocking socket, which stalls
    // whatever else the runtime thread had queued (your profile and playlists), so it gets its own thread.
    let token = tokio::task::spawn_blocking(move || client.get_access_token())
        .await?
        .map_err(|e| anyhow!("playback login failed: {e}"))?;
    Ok(Credentials::with_access_token(token.access_token))
}

/// The real audio output, with each packet also handed to the visualizer on its way through.
struct Tapped(Box<dyn Sink>);

impl Sink for Tapped {
    fn start(&mut self) -> SinkResult<()> {
        self.0.start()
    }

    fn stop(&mut self) -> SinkResult<()> {
        self.0.stop()
    }

    fn write(&mut self, packet: AudioPacket, converter: &mut Converter) -> SinkResult<()> {
        if let AudioPacket::Samples(samples) = &packet {
            crate::visualizer::feed(samples);
        }
        self.0.write(packet, converter)
    }
}

/// How far librespot's reported position runs ahead of what you hear: it counts what has been
/// decoded, and the rodio output keeps about half a second of that queued.
pub const OUTPUT_DELAY_MS: u32 = 450;

/// The redirect address registered for Spotify's own client ID.
const PLAYBACK_REDIRECT: &str = "http://127.0.0.1:8898/login";

/// Forgets the saved playback login, for logging out.
/// Whether a reusable playback login is saved, so connecting won't open the browser.
pub fn has_playback_login() -> bool {
    config::cache_dir().is_ok_and(|dir| dir.join("session").join("credentials.json").exists())
}

pub fn forget_playback_login() {
    if let Ok(dir) = config::cache_dir() {
        let _ = std::fs::remove_file(dir.join("session").join("credentials.json"));
    }
}

static LAST_PROBLEM: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

/// Passes everything to the real logger, and remembers librespot's latest warning so the UI can
/// say why a song wouldn't play.
pub struct ProblemLogger(pub env_logger::Logger);

impl log::Log for ProblemLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        self.0.enabled(metadata)
    }

    fn log(&self, record: &log::Record) {
        if record.level() <= log::Level::Warn && record.target().starts_with("librespot") {
            *LAST_PROBLEM.lock().expect("problem lock") = Some(record.args().to_string());
        }
        self.0.log(record);
    }

    fn flush(&self) {
        self.0.flush();
    }
}

/// librespot's most recent warning or error, cleared by reading it.
pub fn take_last_problem() -> Option<String> {
    LAST_PROBLEM.lock().expect("problem lock").take()
}

fn volume_to_u16(volume: u8) -> u16 {
    (volume.min(100) as u32 * u16::MAX as u32 / 100) as u16
}

fn map_event(event: PlayerEvent) -> Option<PlayerUpdate> {
    Some(match event {
        PlayerEvent::Playing { track_id, position_ms, .. } => {
            PlayerUpdate::Playing { uri: track_id.to_uri().ok()?, position_ms }
        }
        PlayerEvent::Paused { track_id, position_ms, .. } => {
            PlayerUpdate::Paused { uri: track_id.to_uri().ok()?, position_ms }
        }
        PlayerEvent::PositionChanged { track_id, position_ms, .. }
        | PlayerEvent::PositionCorrection { track_id, position_ms, .. }
        | PlayerEvent::Seeked { track_id, position_ms, .. } => {
            PlayerUpdate::Position { uri: track_id.to_uri().ok()?, position_ms }
        }
        PlayerEvent::EndOfTrack { track_id, .. } => PlayerUpdate::EndOfTrack { uri: track_id.to_uri().ok()? },
        PlayerEvent::Unavailable { track_id, .. } => PlayerUpdate::Unavailable { uri: track_id.to_uri().ok()? },
        PlayerEvent::Stopped { .. } => PlayerUpdate::Stopped,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn volume_scale() {
        assert_eq!(super::volume_to_u16(0), 0);
        assert_eq!(super::volume_to_u16(100), u16::MAX);
        assert_eq!(super::volume_to_u16(250), u16::MAX);
    }
}
