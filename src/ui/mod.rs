//! Application state and update loop.

mod keys;
mod style;
mod view;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use iced::keyboard;
use iced::widget::image;
use iced::{Subscription, Task, Theme};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use crate::config::{Config, DjMode};
use crate::dj;
use crate::player::{Audio, PlayerUpdate};
use crate::queue::{Entry, Origin, Queue};
use crate::spotify::api::{Api, Playlist, Track};
use crate::spotify::auth::Auth;
use crate::theme::{self, Colors};

/// Player events flow from librespot's thread into the UI through this channel.
static PLAYER_TX: OnceLock<UnboundedSender<PlayerUpdate>> = OnceLock::new();
static PLAYER_RX: Mutex<Option<UnboundedReceiver<PlayerUpdate>>> = Mutex::new(None);

/// Keep at most this many cover images decoded in memory.
const IMAGE_CACHE: usize = 32;

pub fn run(config: Config) -> iced::Result {
    let (tx, rx) = unbounded_channel();
    let _ = PLAYER_TX.set(tx);
    *PLAYER_RX.lock().expect("player channel") = Some(rx);

    let mut settings = iced::Settings::default();
    settings.default_text_size = config.appearance.font_size.into();
    if let Some(family) = crate::config::non_empty(&config.appearance.font_family) {
        // iced wants a 'static name; this happens once at startup.
        settings.default_font = iced::Font::with_name(Box::leak(family.to_string().into_boxed_str()));
    }
    let size = (config.appearance.window_width, config.appearance.window_height);

    iced::application(move || App::new(config.clone()), App::update, App::view)
        .title(App::title)
        .theme(App::theme)
        .subscription(App::subscription)
        .settings(settings)
        .window_size(size)
        .run()
}

#[derive(Debug, Clone, PartialEq)]
pub enum Page {
    Dj,
    Search,
    Liked,
    Playlist(Playlist),
    Settings,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Login {
    NeedsClientId,
    LoggedOut,
    InProgress,
    LoggedIn,
}

#[derive(Debug, Clone)]
pub enum Message {
    // Login and setup
    ClientIdChanged(String),
    SaveClientId,
    Login,
    LoggedIn(Result<Api, String>),
    AudioReady(Result<Arc<Audio>, String>),
    Profile(Result<String, String>),
    Playlists(Result<Vec<Playlist>, String>),
    Logout,

    // Browsing
    Open(Page),
    Tracks(Page, Result<Vec<Track>, String>),
    SearchChanged(String),
    SearchSubmit,
    PlayFrom(usize),
    QueueTrack(usize),
    PlayNextTrack(usize),

    // Playback
    Player(PlayerUpdate),
    PlayerResult(Result<(), String>),
    TogglePlay,
    Next,
    Previous,
    SeekPreview(f32),
    SeekStep(i64),
    VolumeStep(i32),
    SeekCommit,
    Volume(f32),
    VolumeCommit,
    ToggleShuffle,
    CycleRepeat,
    JumpTo(usize),
    RemoveFromQueue(usize),
    ClearQueue,
    Image(String, Result<Vec<u8>, String>),

    // DJ
    DjRequestChanged(String),
    DjStart,
    DjStop,
    DjSet(Result<dj::Set, String>),
    SpeechDone(Result<(), String>),

    // Settings
    Edit(Edit),
    SaveSettings,
    ToggleSidePanel,
    DismissNotice,
    Key(keyboard::Event),
}

/// A change to one setting from the settings page; applied live, persisted on Save.
#[derive(Debug, Clone)]
pub enum Edit {
    Theme(String),
    Accent(String),
    FontSize(f32),
    Radius(f32),
    Compact(bool),
    AlbumArt(bool),
    Sidebar(crate::config::Side),
    SidePanel(crate::config::Side),
    PlayerBar(crate::config::Edge),
    Provider(crate::config::LlmProvider),
    Model(String),
    BaseUrl(String),
    ApiKey(String),
    ApiKeyEnv(String),
    Mode(DjMode),
    DjName(String),
    Persona(String),
    Language(String),
    BatchSize(f32),
    RefillAt(f32),
    TalkEvery(f32),
    MaxWords(f32),
    Discovery(f32),
    Explicit(bool),
    TtsEngine(crate::config::TtsEngine),
    Voice(String),
    TtsModel(String),
    DuckTo(f32),
}

#[derive(Debug, Clone)]
pub struct Notice {
    pub text: String,
    pub error: bool,
}

pub struct App {
    pub config: Config,
    pub colors: Colors,
    theme: Theme,
    pub login: Login,
    pub client_id_input: String,
    api: Option<Api>,
    audio: Option<Arc<Audio>>,
    pub user_name: String,
    pub playlists: Vec<Playlist>,
    pub page: Page,
    pub tracks: Vec<Track>,
    pub loading: bool,
    pub search: String,
    pub queue: Queue,
    pub playing: bool,
    pub position_ms: u32,
    pub seek_preview: Option<f32>,
    pub volume: u8,
    preloaded: Option<String>,
    images: HashMap<String, image::Handle>,
    image_order: Vec<String>,
    pub show_side_panel: bool,
    pub notice: Option<Notice>,

    pub dj_on: bool,
    pub dj_busy: bool,
    pub dj_request: String,
    pub dj_log: Vec<String>,
    dj_opened: bool,
    pub speaking: bool,
    resume_after_speech: bool,
}

fn err(e: anyhow::Error) -> String {
    format!("{e:#}")
}

impl App {
    fn new(config: Config) -> (Self, Task<Message>) {
        let colors = theme::resolve(&config.appearance);
        let has_client = crate::config::non_empty(&config.spotify.client_id).is_some();
        let app = Self {
            theme: theme::iced_theme(&colors),
            colors,
            login: if has_client { Login::LoggedOut } else { Login::NeedsClientId },
            client_id_input: config.spotify.client_id.clone(),
            api: None,
            audio: None,
            user_name: String::new(),
            playlists: Vec::new(),
            page: Page::Dj,
            tracks: Vec::new(),
            loading: false,
            search: String::new(),
            queue: Queue::default(),
            playing: false,
            position_ms: 0,
            seek_preview: None,
            volume: config.playback.volume,
            preloaded: None,
            images: HashMap::new(),
            image_order: Vec::new(),
            show_side_panel: config.layout.show_side_panel,
            notice: None,
            dj_on: false,
            dj_busy: false,
            dj_request: String::new(),
            dj_log: Vec::new(),
            dj_opened: false,
            speaking: false,
            resume_after_speech: false,
            config,
        };
        let mut app = app;
        if std::env::var_os("HAZTIFY_DEMO").is_some() {
            app.load_demo();
            return (app, Task::none());
        }
        // Log straight back in when a previous login is cached.
        let task = if has_client && Auth::has_cached_login() { Task::done(Message::Login) } else { Task::none() };
        (app, task)
    }

    fn title(&self) -> String {
        match self.queue.current() {
            Some(e) => format!("{} · {} — Haztify", e.track.name, e.track.artist_line()),
            None => "Haztify".into(),
        }
    }

    fn theme(&self) -> Theme {
        self.theme.clone()
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            Subscription::run(player_events),
            keyboard::listen().map(Message::Key),
        ])
    }

    fn notify(&mut self, text: impl Into<String>, error: bool) {
        let text = text.into();
        if error {
            log::error!("{text}");
        }
        self.notice = Some(Notice { text, error });
    }

    fn refresh_theme(&mut self) {
        self.colors = theme::resolve(&self.config.appearance);
        self.theme = theme::iced_theme(&self.colors);
    }

    pub fn image(&self, url: &str) -> Option<&image::Handle> {
        self.images.get(url)
    }

    fn fetch_image(&self, url: Option<&String>) -> Task<Message> {
        let (Some(api), Some(url)) = (self.api.clone(), url.cloned()) else {
            return Task::none();
        };
        if !self.config.appearance.show_album_art || self.images.contains_key(&url) {
            return Task::none();
        }
        Task::perform(
            async move {
                let bytes = api.fetch_image(&url).await.map_err(err);
                (url, bytes)
            },
            |(url, bytes)| Message::Image(url, bytes),
        )
    }

    fn load_page(&mut self, page: Page) -> Task<Message> {
        self.page = page.clone();
        let Some(api) = self.api.clone() else {
            return Task::none();
        };
        let fetch = match &page {
            Page::Liked => {
                let api = api.clone();
                Some(Task::perform(async move { api.liked_tracks().await.map_err(err) }, {
                    let page = page.clone();
                    move |r| Message::Tracks(page.clone(), r)
                }))
            }
            Page::Playlist(p) => {
                let id = p.id.clone();
                Some(Task::perform(async move { api.playlist_tracks(&id).await.map_err(err) }, {
                    let page = page.clone();
                    move |r| Message::Tracks(page.clone(), r)
                }))
            }
            Page::Search => {
                self.tracks.clear();
                None
            }
            Page::Dj | Page::Settings => None,
        };
        match fetch {
            Some(task) => {
                self.loading = true;
                self.tracks.clear();
                task
            }
            None => Task::none(),
        }
    }

    /// Starts playing the queue's current entry, saying the DJ intro first if there is one.
    fn play_current(&mut self) -> Task<Message> {
        let Some(entry) = self.queue.current().cloned() else {
            self.playing = false;
            return Task::none();
        };
        let Some(audio) = self.audio.clone() else {
            self.notify("Playback isn't connected yet", true);
            return Task::none();
        };
        self.position_ms = 0;
        self.preloaded = None;

        let intro = match &entry.origin {
            Origin::Dj { intro: Some(intro) } if self.dj_on && self.config.dj.mode != DjMode::Queue => {
                self.dj_log.push(format!("{}: {}", self.config.dj.name, intro));
                (self.config.dj.mode == DjMode::Voice).then(|| intro.clone())
            }
            _ => None,
        };

        let duck_to = self.config.dj.tts.duck_to;
        let start_playing = intro.is_none() || duck_to > 0;
        let settings = self.config.playback.clone();
        let uri = entry.track.uri.clone();
        let load_audio = audio.clone();
        let load = Task::perform(
            async move {
                load_audio.ensure_connected(&settings).await.map_err(err)?;
                load_audio.load(&uri, start_playing, 0).map_err(err)
            },
            Message::PlayerResult,
        );

        let mut tasks = vec![load, self.fetch_image(entry.track.image_url.as_ref()), self.maybe_refill()];
        if let Some(text) = intro {
            tasks.push(self.speak(text, start_playing));
        }
        Task::batch(tasks)
    }

    fn speak(&mut self, text: String, music_playing: bool) -> Task<Message> {
        let Some(audio) = self.audio.clone() else {
            return Task::none();
        };
        self.speaking = true;
        self.resume_after_speech = !music_playing;
        if music_playing {
            audio.set_volume((self.volume as u32 * self.config.dj.tts.duck_to as u32 / 100) as u8);
        }
        let config = self.config.dj.clone();
        Task::perform(async move { dj::tts::speak(&config, &text).await.map_err(err) }, Message::SpeechDone)
    }

    /// Asks the DJ for more songs when it is on and the queue is running low.
    fn maybe_refill(&mut self) -> Task<Message> {
        if !self.dj_on || self.dj_busy || self.queue.up_next().len() > self.config.dj.refill_at {
            return Task::none();
        }
        self.request_set()
    }

    fn request_set(&mut self) -> Task<Message> {
        let Some(api) = self.api.clone() else {
            return Task::none();
        };
        self.dj_busy = true;
        let config = self.config.dj.clone();
        let opening = !self.dj_opened;
        let base = dj::Context {
            now_playing: self.queue.current().map(|e| e.track.clone()),
            history: self.queue.history(10).iter().map(|e| e.track.clone()).collect(),
            up_next: self.queue.up_next().iter().take(10).map(|e| e.track.clone()).collect(),
            request: self.dj_request.clone(),
            opening,
            ..Default::default()
        };
        Task::perform(
            async move {
                let (top_tracks, top_artists, recent) = dj::taste(&api).await;
                let history = if base.history.is_empty() { recent } else { base.history.clone() };
                let ctx = dj::Context { top_tracks, top_artists, history, ..base };
                dj::plan_set(&api, &config, ctx).await.map_err(err)
            },
            Message::DjSet,
        )
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::ClientIdChanged(s) => self.client_id_input = s,
            Message::SaveClientId => {
                let id = self.client_id_input.trim().to_string();
                if id.is_empty() {
                    return Task::none();
                }
                self.config.spotify.client_id = id;
                if let Err(e) = self.config.save() {
                    self.notify(format!("Couldn't save config: {e:#}"), true);
                }
                self.login = Login::LoggedOut;
                return Task::done(Message::Login);
            }
            Message::Login => {
                self.login = Login::InProgress;
                let settings = self.config.spotify.clone();
                return Task::perform(
                    async move { Auth::login(settings).await.map(Api::new).map_err(err) },
                    Message::LoggedIn,
                );
            }
            Message::LoggedIn(Ok(api)) => {
                self.login = Login::LoggedIn;
                self.api = Some(api.clone());
                let tx = PLAYER_TX.get().expect("player channel").clone();
                let (a1, a2) = (api.clone(), api.clone());
                let playback = self.config.playback.clone();
                return Task::batch([
                    Task::perform(
                        async move { Audio::connect(api.auth().clone(), playback, tx).await.map(Arc::new).map_err(err) },
                        Message::AudioReady,
                    ),
                    Task::perform(async move { a1.display_name().await.map_err(err) }, Message::Profile),
                    Task::perform(async move { a2.playlists().await.map_err(err) }, Message::Playlists),
                ]);
            }
            Message::LoggedIn(Err(e)) => {
                self.login = Login::LoggedOut;
                self.notify(e, true);
            }
            Message::AudioReady(Ok(audio)) => self.audio = Some(audio),
            Message::AudioReady(Err(e)) => self.notify(e, true),
            Message::Profile(Ok(name)) => self.user_name = name,
            Message::Playlists(Ok(list)) => self.playlists = list,
            Message::Profile(Err(e)) | Message::Playlists(Err(e)) => self.notify(e, true),
            Message::Logout => {
                if let Some(audio) = &self.audio {
                    audio.stop();
                }
                if let Err(e) = Auth::logout() {
                    self.notify(format!("{e:#}"), true);
                }
                self.api = None;
                self.audio = None;
                self.playlists.clear();
                self.tracks.clear();
                self.queue = Queue::default();
                self.login = Login::LoggedOut;
            }

            Message::Open(page) => return self.load_page(page),
            Message::Tracks(page, result) => {
                if page == self.page {
                    self.loading = false;
                    match result {
                        Ok(tracks) => self.tracks = tracks,
                        Err(e) => self.notify(e, true),
                    }
                }
            }
            Message::SearchChanged(s) => self.search = s,
            Message::SearchSubmit => {
                let (Some(api), q) = (self.api.clone(), self.search.trim().to_string()) else {
                    return Task::none();
                };
                if q.is_empty() {
                    return Task::none();
                }
                self.loading = true;
                return Task::perform(async move { api.search_tracks(&q, 10).await.map_err(err) }, |r| {
                    Message::Tracks(Page::Search, r)
                });
            }
            Message::PlayFrom(i) => {
                let tracks = self.tracks.clone();
                self.queue.play_list(&tracks, i);
                return self.play_current();
            }
            Message::QueueTrack(i) => {
                if let Some(track) = self.tracks.get(i).cloned() {
                    self.queue.push(Entry { track, origin: Origin::User });
                    if self.queue.current().is_none() {
                        self.queue.advance(true);
                        return self.play_current();
                    }
                }
            }
            Message::PlayNextTrack(i) => {
                if let Some(track) = self.tracks.get(i).cloned() {
                    self.queue.play_next(Entry { track, origin: Origin::User });
                }
            }

            Message::Player(update) => return self.on_player(update),
            Message::PlayerResult(Err(e)) => self.notify(e, true),
            Message::PlayerResult(Ok(())) => {}
            Message::TogglePlay => {
                let Some(audio) = &self.audio else { return Task::none() };
                if self.queue.current().is_none() {
                    if self.tracks.is_empty() {
                        return Task::none();
                    }
                    return self.update(Message::PlayFrom(0));
                }
                if self.playing { audio.pause() } else { audio.play() }
            }
            Message::Next => {
                if self.queue.advance(true).is_some() {
                    return self.play_current();
                }
                return self.maybe_refill();
            }
            Message::Previous => {
                if self.position_ms > 3000 {
                    if let Some(audio) = &self.audio {
                        audio.seek(0);
                    }
                } else if self.queue.back().is_some() {
                    return self.play_current();
                }
            }
            Message::SeekPreview(v) => self.seek_preview = Some(v),
            Message::SeekCommit => {
                if let (Some(v), Some(audio)) = (self.seek_preview.take(), &self.audio) {
                    audio.seek(v as u32);
                    self.position_ms = v as u32;
                }
            }
            Message::SeekStep(delta) => {
                let duration = self.queue.current().map(|e| e.track.duration_ms as i64).unwrap_or(0);
                if let Some(audio) = &self.audio {
                    let target = (self.position_ms as i64 + delta).clamp(0, (duration - 1000).max(0));
                    audio.seek(target as u32);
                    self.position_ms = target as u32;
                }
            }
            Message::VolumeStep(delta) => {
                let v = (self.volume as i32 + delta).clamp(0, 100);
                let _ = self.update(Message::Volume(v as f32));
                return self.update(Message::VolumeCommit);
            }
            Message::Volume(v) => {
                self.volume = v as u8;
                if let Some(audio) = &self.audio
                    && !self.speaking {
                        audio.set_volume(self.volume);
                    }
            }
            Message::VolumeCommit => {
                self.config.playback.volume = self.volume;
                let _ = self.config.save();
            }
            Message::ToggleShuffle => self.queue.toggle_shuffle(),
            Message::CycleRepeat => self.queue.repeat = self.queue.repeat.cycle(),
            Message::JumpTo(offset) => {
                self.queue.jump(offset);
                return self.play_current();
            }
            Message::RemoveFromQueue(offset) => self.queue.remove_up_next(offset),
            Message::ClearQueue => self.queue.clear_up_next(),
            Message::Image(url, Ok(bytes)) => {
                self.images.insert(url.clone(), image::Handle::from_bytes(bytes));
                self.image_order.push(url);
                while self.image_order.len() > IMAGE_CACHE {
                    let old = self.image_order.remove(0);
                    self.images.remove(&old);
                }
            }
            Message::Image(_, Err(e)) => log::debug!("cover art: {e}"),

            Message::DjRequestChanged(s) => self.dj_request = s,
            Message::DjStart => {
                if self.audio.is_none() {
                    self.notify("Log in to Spotify first", true);
                    return Task::none();
                }
                self.dj_on = true;
                if self.dj_busy {
                    return Task::none();
                }
                // A fresh request replaces whatever was queued, so the new vibe starts next.
                self.queue.clear_up_next();
                return self.request_set();
            }
            Message::DjStop => {
                self.dj_on = false;
                self.dj_opened = false;
            }
            Message::DjSet(result) => {
                self.dj_busy = false;
                match result {
                    Ok(set) => {
                        if !set.missed.is_empty() {
                            log::info!("DJ picks not found on Spotify: {:?}", set.missed);
                        }
                        let was_idle = self.queue.current().is_none() || !self.playing;
                        let first = !self.dj_opened;
                        self.dj_opened = true;
                        for (track, intro) in set.tracks {
                            self.queue.push(Entry { track, origin: Origin::Dj { intro } });
                        }
                        if !self.dj_on {
                            return Task::none();
                        }
                        let mut tasks = Vec::new();
                        if let Some(opener) = set.opener.filter(|_| first) {
                            self.dj_log.push(format!("{}: {}", self.config.dj.name, opener));
                            if self.config.dj.mode == DjMode::Voice {
                                tasks.push(self.speak(opener, self.playing));
                            }
                        }
                        if was_idle {
                            // Jump to the first DJ pick.
                            let current = self.queue.current_index();
                            let first_dj = self.queue.entries().iter().enumerate().position(|(i, e)| {
                                matches!(e.origin, Origin::Dj { .. }) && current.is_none_or(|c| i > c)
                            });
                            if let Some(index) = first_dj {
                                let offset = index - current.map(|c| c + 1).unwrap_or(0);
                                self.queue.jump(offset);
                                tasks.push(self.play_current());
                            }
                        }
                        return Task::batch(tasks);
                    }
                    Err(e) => {
                        self.dj_log.push(format!("⚠ {e}"));
                        self.notify(e, true);
                    }
                }
            }
            Message::SpeechDone(result) => {
                self.speaking = false;
                if let Some(audio) = &self.audio {
                    audio.set_volume(self.volume);
                    if std::mem::take(&mut self.resume_after_speech) {
                        audio.play();
                    }
                }
                if let Err(e) = result {
                    self.notify(format!("DJ voice: {e}"), true);
                }
            }

            Message::Edit(edit) => self.apply_edit(edit),
            Message::SaveSettings => match self.config.save() {
                Ok(()) => self.notify("Settings saved", false),
                Err(e) => self.notify(format!("Couldn't save settings: {e:#}"), true),
            },
            Message::ToggleSidePanel => self.show_side_panel = !self.show_side_panel,
            Message::DismissNotice => self.notice = None,
            Message::Key(event) => {
                if let Some(msg) = keys::action(&self.config.keys, &event) {
                    return self.update(msg);
                }
            }
        }
        Task::none()
    }

    fn on_player(&mut self, update: PlayerUpdate) -> Task<Message> {
        let current_uri = self.queue.current().map(|e| e.track.uri.clone());
        let is_current = |uri: &str| current_uri.as_deref() == Some(uri);
        match update {
            PlayerUpdate::Playing { uri, position_ms } if is_current(&uri) => {
                self.playing = true;
                self.position_ms = position_ms;
            }
            PlayerUpdate::Paused { uri, position_ms } if is_current(&uri) => {
                self.playing = false;
                self.position_ms = position_ms;
            }
            PlayerUpdate::Position { uri, position_ms } if is_current(&uri) => {
                self.position_ms = position_ms;
                // Preload the next track near the end for gapless playback.
                let duration = self.queue.current().map(|e| e.track.duration_ms).unwrap_or(0);
                if duration > 0 && duration.saturating_sub(position_ms) < 20_000
                    && let (Some(next), Some(audio)) = (self.queue.up_next().first(), &self.audio)
                        && self.preloaded.as_deref() != Some(&next.track.uri) {
                            audio.preload(&next.track.uri);
                            self.preloaded = Some(next.track.uri.clone());
                        }
            }
            PlayerUpdate::EndOfTrack { uri } if is_current(&uri) => {
                if self.queue.advance(false).is_some() {
                    return self.play_current();
                }
                self.playing = false;
                return self.maybe_refill();
            }
            PlayerUpdate::Unavailable { uri } if is_current(&uri) => {
                self.notify("That track isn't available here; skipping", true);
                if self.queue.advance(true).is_some() {
                    return self.play_current();
                }
            }
            PlayerUpdate::Stopped => self.playing = false,
            _ => {}
        }
        Task::none()
    }

    fn apply_edit(&mut self, edit: Edit) {
        use Edit::*;
        let a = &mut self.config.appearance;
        let dj = &mut self.config.dj;
        match edit {
            Theme(t) => a.theme = t,
            Accent(c) => a.colors.accent = c,
            FontSize(v) => a.font_size = v,
            Radius(v) => a.corner_radius = v,
            Compact(v) => a.compact = v,
            AlbumArt(v) => a.show_album_art = v,
            Sidebar(s) => self.config.layout.sidebar = s,
            SidePanel(s) => self.config.layout.side_panel = s,
            PlayerBar(e) => self.config.layout.player_bar = e,
            Provider(p) => dj.provider = p,
            Model(s) => dj.model = s,
            BaseUrl(s) => dj.base_url = s,
            ApiKey(s) => dj.api_key = s,
            ApiKeyEnv(s) => dj.api_key_env = s,
            Mode(m) => dj.mode = m,
            DjName(s) => dj.name = s,
            Persona(s) => dj.persona = s,
            Language(s) => dj.language = s,
            BatchSize(v) => dj.batch_size = v as usize,
            RefillAt(v) => dj.refill_at = v as usize,
            TalkEvery(v) => dj.talk_every = v as usize,
            MaxWords(v) => dj.max_words = v as usize,
            Discovery(v) => dj.discovery = v as u8,
            Explicit(v) => dj.allow_explicit = v,
            TtsEngine(e) => dj.tts.engine = e,
            Voice(s) => dj.tts.voice = s,
            TtsModel(s) => dj.tts.model = s,
            DuckTo(v) => dj.tts.duck_to = v as u8,
        }
        self.refresh_theme();
    }

    /// `HAZTIFY_DEMO=1` fills the UI with sample data (no Spotify login) for previewing themes and layouts.
    fn load_demo(&mut self) {
        let t = |name: &str, artist: &str, album: &str, ms: u32| Track {
            id: name.into(),
            uri: format!("spotify:track:{name}"),
            name: name.into(),
            artists: vec![artist.into()],
            album: album.into(),
            duration_ms: ms,
            explicit: false,
            image_url: None,
        };
        self.login = Login::LoggedIn;
        self.user_name = "Haz".into();
        self.playlists = ["Late Night Drive", "Gym", "Focus", "Indie Finds"]
            .iter()
            .map(|n| Playlist { id: n.to_string(), name: n.to_string(), owner: "Haz".into(), track_count: 42 })
            .collect();
        self.tracks = vec![
            t("Midnight City", "M83", "Hurry Up, We're Dreaming", 243_000),
            t("Electric Feel", "MGMT", "Oracular Spectacular", 229_000),
            t("Breezeblocks", "alt-J", "An Awesome Wave", 227_000),
            t("Redbone", "Childish Gambino", "Awaken, My Love!", 327_000),
            t("Do I Wanna Know?", "Arctic Monkeys", "AM", 272_000),
        ];
        let tracks = self.tracks.clone();
        self.queue.play_list(&tracks, 0);
        for (i, e) in tracks.iter().enumerate().skip(1).take(2) {
            self.queue.push(Entry { track: e.clone(), origin: Origin::Dj { intro: Some(format!("intro {i}")) } });
        }
        self.page = match std::env::var("HAZTIFY_DEMO").as_deref() {
            Ok("settings") => Page::Settings,
            Ok("tracks") => Page::Liked,
            _ => Page::Dj,
        };
        self.position_ms = 81_000;
        self.playing = true;
        self.dj_on = true;
        self.dj_log = vec![
            format!("{}: Evening, Haz. Let's ease into the night with some synth-soaked classics.", self.config.dj.name),
            format!("{}: That was M83, recorded in LA but dreaming of Paris. Up next, MGMT.", self.config.dj.name),
        ];
    }

    pub fn has_audio(&self) -> bool {
        self.audio.is_some()
    }
}

fn player_events() -> impl iced::futures::Stream<Item = Message> {
    iced::stream::channel(64, async |mut out: iced::futures::channel::mpsc::Sender<Message>| {
        use iced::futures::SinkExt;
        let Some(mut rx) = PLAYER_RX.lock().expect("player channel").take() else {
            return;
        };
        while let Some(update) = rx.recv().await {
            if out.send(Message::Player(update)).await.is_err() {
                break;
            }
        }
        // Keep the subscription alive without busy-looping if the channel ever closes.
        loop {
            tokio::time::sleep(Duration::from_secs(3600)).await;
        }
    })
}
