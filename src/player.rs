//! In-process audio via librespot: streams, decodes and plays Spotify tracks (Premium only).

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use librespot_core::authentication::Credentials;
use librespot_core::cache::Cache;
use librespot_core::config::SessionConfig;
use librespot_core::session::Session;
use librespot_core::spotify_uri::SpotifyUri;
use librespot_playback::audio_backend;
use librespot_playback::config::{AudioFormat, Bitrate as LsBitrate, PlayerConfig};
use librespot_playback::mixer::{self, Mixer, MixerConfig};
use librespot_playback::player::{Player, PlayerEvent};
use tokio::sync::mpsc::UnboundedSender;

use crate::config::{self, Bitrate, PlaybackConfig};
use crate::spotify::auth::Auth;

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
    auth: Auth,
}

impl std::fmt::Debug for Audio {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Audio")
    }
}

impl Audio {
    pub async fn connect(auth: Auth, settings: PlaybackConfig, events: UnboundedSender<PlayerUpdate>) -> Result<Self> {
        let session = new_session(&auth, &settings).await?;

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
            backend(None, AudioFormat::default())
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

        Ok(Self { session: std::sync::Mutex::new(session), player, mixer, auth })
    }

    /// Reconnects the audio session if Spotify dropped it (sleep, network change, ...).
    pub async fn ensure_connected(&self, settings: &PlaybackConfig) -> Result<()> {
        let dropped = self.session.lock().expect("session lock").is_invalid();
        if dropped {
            log::info!("Spotify session dropped; reconnecting");
            let session = new_session(&self.auth, settings).await?;
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

    /// 0-100
    pub fn set_volume(&self, volume: u8) {
        self.mixer.set_volume(volume_to_u16(volume));
    }
}

async fn new_session(auth: &Auth, settings: &PlaybackConfig) -> Result<Session> {
    let cache_dir = config::cache_dir()?;
    let audio_dir = (settings.audio_cache_mb > 0).then(|| cache_dir.join("audio"));
    let size_limit = (settings.audio_cache_mb > 0).then_some(settings.audio_cache_mb * 1024 * 1024);
    let cache = Cache::new(Some(cache_dir.join("session")), None, audio_dir, size_limit)
        .map_err(|e| anyhow!("audio cache: {e}"))?;

    let session = Session::new(SessionConfig::default(), Some(cache));
    let token = auth.access_token().await?;
    session
        .connect(Credentials::with_access_token(token), false)
        .await
        .map_err(|e| anyhow!("connecting to Spotify for playback (Premium is required): {e}"))?;
    Ok(session)
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
