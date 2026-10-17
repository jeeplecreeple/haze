//! Application state and update loop.

mod custom;
mod icon;
mod keys;
mod style;
mod view;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use iced::widget::image;
use iced::{Color, Subscription, Task, Theme, keyboard, window};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use crate::config::{Config, DjMode};
use crate::dj;
use crate::lyrics::{self, Lyrics};
use crate::player::{Audio, PlayerUpdate};
use crate::queue::{Entry, Origin, Queue};
use crate::spotify::api::{Api, Playlist, Track};
use crate::spotify::auth::Auth;
use crate::theme::{self, Colors};
use crate::update;

/// Player events flow from librespot's thread into the UI through this channel.
static PLAYER_TX: OnceLock<UnboundedSender<PlayerUpdate>> = OnceLock::new();
static PLAYER_RX: Mutex<Option<UnboundedReceiver<PlayerUpdate>>> = Mutex::new(None);

/// Keep at most this many cover images decoded in memory.
const IMAGE_CACHE: usize = 32;

pub fn run(config: Config) -> iced::Result {
    let (tx, rx) = unbounded_channel();
    let _ = PLAYER_TX.set(tx);
    *PLAYER_RX.lock().expect("player channel") = Some(rx);

    custom::migrate_menu_entry();
    if let Err(e) = custom::sync_menu_entry(&config.appearance.app_icon) {
        log::warn!("app menu entry: {e:#}");
    }
    let (fonts, families) = custom::load_fonts(&config.appearance.font_files);
    if !families.is_empty() {
        log::info!("loaded fonts: {}", families.join(", "));
    }
    let mut settings = iced::Settings::default();
    settings.default_text_size = config.appearance.font_size.into();
    settings.default_font = custom::font(&config.appearance.font_family);
    let window = window::Settings {
        size: iced::Size::new(config.appearance.window_width, config.appearance.window_height),
        icon: custom::window_icon(&config.appearance.app_icon),
        // Matches haze.desktop, which is where Wayland desktops find the icon.
        #[cfg(target_os = "linux")]
        platform_specific: window::settings::PlatformSpecific {
            application_id: "haze".into(),
            ..Default::default()
        },
        ..Default::default()
    };

    let mut app = iced::application(move || App::new(config.clone(), families.clone()), App::update, App::view)
        .title(App::title)
        .theme(App::theme)
        .subscription(App::subscription)
        .settings(settings)
        .window(window);
    for font in icon::BUNDLED_FONTS {
        app = app.font(font);
    }
    for font in fonts {
        app = app.font(font);
    }
    app.run()
}

/// The font pickers' labels for "no font set" (the bundled Inter) and the system's own font.
pub const DEFAULT_FONT: &str = "Inter (built-in)";
pub const SYSTEM_FONT: &str = "System font";

#[derive(Debug, Clone, PartialEq)]
pub enum Page {
    Dj,
    Search,
    Liked,
    Playlist(Playlist),
    Lyrics,
    Settings,
}

/// A newer release, and how far installing it has got.
#[derive(Debug, Clone)]
pub struct Update {
    pub release: update::Release,
    pub status: UpdateStatus,
}

#[derive(Debug, Clone)]
pub enum UpdateStatus {
    Available,
    Installing,
    /// Installed; restarting runs this program.
    Installed(PathBuf),
    Failed(String),
}

/// The groups on the Settings page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsTab {
    Dj,
    Look,
    Lyrics,
    Layout,
    Updates,
}

impl SettingsTab {
    pub const ALL: [SettingsTab; 5] = [Self::Dj, Self::Look, Self::Lyrics, Self::Layout, Self::Updates];

    pub fn label(self) -> &'static str {
        match self {
            Self::Dj => "DJ",
            Self::Look => "Appearance",
            Self::Lyrics => "Lyrics",
            Self::Layout => "Layout",
            Self::Updates => "Updates",
        }
    }

    pub fn icon(self) -> char {
        match self {
            Self::Dj => icon::SPARKLES,
            Self::Look => icon::PALETTE,
            Self::Lyrics => icon::MIC_VOCAL,
            Self::Layout => icon::SLIDERS_HORIZONTAL,
            Self::Updates => icon::ROTATE_CCW,
        }
    }
}

/// The steps of the first-run setup, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetupStep {
    Hello,
    Look,
    Spotify,
    Dj,
    Login,
    Done,
}

impl SetupStep {
    pub const ALL: [SetupStep; 6] = [Self::Hello, Self::Look, Self::Spotify, Self::Dj, Self::Login, Self::Done];

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|s| *s == self).unwrap_or(0)
    }
}

/// The animated first-run setup, shown until it's finished.
#[derive(Debug, Clone)]
pub struct Setup {
    pub step: SetupStep,
    /// When the step appeared, for its entrance animation.
    pub entered: Instant,
    /// The step was reached with Back, so it slides in from the other side.
    pub backwards: bool,
    /// When the Copy button last copied the redirect URI.
    pub copied: Option<Instant>,
}

impl Setup {
    fn new(step: SetupStep) -> Self {
        Self { step, entered: Instant::now(), backwards: false, copied: None }
    }

    fn go(&mut self, step: SetupStep) {
        self.backwards = step.index() < self.step.index();
        self.step = step;
        self.entered = Instant::now();
    }

    /// Whether anything on screen is still moving, so frames are only drawn while needed.
    fn animating(&self, login: &Login) -> bool {
        let age = self.entered.elapsed();
        self.step == SetupStep::Hello
            || age < Duration::from_millis(900)
            || (self.step == SetupStep::Login && *login == Login::InProgress)
            || (self.step == SetupStep::Done && age < Duration::from_millis(2500))
            || self.copied.is_some_and(|t| t.elapsed() < Duration::from_millis(2200))
    }
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
    /// Look for a new release; `true` when you asked, so "up to date" gets said.
    CheckForUpdates(bool),
    UpdateChecked(bool, Result<Option<update::Release>, String>),
    InstallUpdate,
    UpdateInstalled(Result<PathBuf, String>),
    RestartForUpdate,
    DismissUpdate,
    SettingsTab(SettingsTab),
    SetupNext,
    SetupBack,
    SetupFinish,
    RunSetup,
    OpenUrl(String),
    CopyRedirectUri,
    /// A redraw while something animates.
    Frame,

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

    // Lyrics
    ToggleLyrics,
    ToggleFullscreen,
    LyricsFound(String, Result<Option<Lyrics>, String>),
    SeekTo(u32),
    Tick,

    // DJ
    DjRequestChanged(String),
    DjStart,
    DjStop,
    DjSet(Result<dj::Set, String>),
    SpeechDone(Result<(), String>),

    // Settings
    Edit(Edit),
    PickIcon,
    IconPicked(Option<PathBuf>),
    /// No file picker could be opened.
    PickFailed(String),
    /// A file was dragged onto the window.
    FileDropped(PathBuf),
    /// A built-in icon by name.
    SetIcon(&'static str),
    PickCover(String),
    CoverPicked(String, Option<PathBuf>),
    ResetCover(String),
    PickFont,
    FontPicked(Option<PathBuf>),
    FontLoaded(Result<(), iced::font::Error>),
    OpenConfigFolder,
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
    AccentFromCover(bool),
    AmbientBackground(bool),
    AppFont(String),
    LyricsFont(String),
    WordHighlight(bool),
    BlurredBackground(bool),
    LyricsAlign(crate::config::LyricsAlign),
    LyricsProvider(crate::config::LyricsProvider),
    LyricsSize(f32),
    LyricsOffset(f32),
    LinesBefore(f32),
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
    CheckUpdates(bool),
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
    pub setup: Option<Setup>,
    pub settings_tab: SettingsTab,
    /// Songs Spotify refused one after another; playback stops after a few.
    unavailable_in_a_row: u8,
    pub update: Option<Update>,
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
    /// When `position_ms` was last updated, to move the lyrics along between player updates.
    position_at: Instant,
    pub seek_preview: Option<f32>,
    pub volume: u8,
    preloaded: Option<String>,
    images: HashMap<String, image::Handle>,
    image_order: Vec<String>,
    pub show_side_panel: bool,
    pub notice: Option<Notice>,
    previous_page: Page,
    pub theme_names: Vec<String>,
    /// Family names of the fonts in the fonts folder and `font_files`.
    pub font_families: Vec<String>,
    pub icon: image::Handle,
    /// The built-in icons, decoded once for the pickers.
    pub icon_choices: Vec<(&'static str, image::Handle)>,

    pub lyrics: Option<Lyrics>,
    lyrics_for: Option<String>,
    pub lyrics_loading: bool,
    pub fullscreen: bool,
    /// Cover-derived extras for the current song: the lyrics background and the accent colour.
    cover_fx_for: Option<String>,
    pub lyrics_background: Option<image::Handle>,
    /// The same, stronger, behind the whole window (`appearance.ambient_background`).
    pub ambient: Option<image::Handle>,
    cover_accent: Option<Color>,

    pub dj_on: bool,
    pub dj_busy: bool,
    pub dj_request: String,
    pub dj_log: Vec<String>,
    dj_opened: bool,
    pub speaking: bool,
    resume_after_speech: bool,
    /// The system voice is missing (Linux without espeak-ng), so intros aren't spoken.
    no_system_voice: bool,
}

fn err(e: anyhow::Error) -> String {
    format!("{e:#}")
}

impl App {
    fn new(config: Config, font_families: Vec<String>) -> (Self, Task<Message>) {
        let colors = theme::resolve(&config.appearance);
        let has_client = crate::config::non_empty(&config.spotify.client_id).is_some();
        let app = Self {
            theme: theme::iced_theme(&colors),
            colors,
            login: if has_client { Login::LoggedOut } else { Login::NeedsClientId },
            setup: (!config.setup_done).then(|| Setup::new(SetupStep::Hello)),
            update: None,
            settings_tab: SettingsTab::Dj,
            unavailable_in_a_row: 0,
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
            position_at: Instant::now(),
            seek_preview: None,
            volume: config.playback.volume,
            preloaded: None,
            images: HashMap::new(),
            image_order: Vec::new(),
            show_side_panel: config.layout.show_side_panel,
            notice: None,
            previous_page: Page::Dj,
            theme_names: Vec::new(),
            font_families,
            icon: icon_handle(&config.appearance.app_icon),
            icon_choices: custom::BUILTIN_ICONS.iter().map(|(name, bytes)| (*name, image::Handle::from_bytes(*bytes))).collect(),
            lyrics: None,
            lyrics_for: None,
            lyrics_loading: false,
            fullscreen: false,
            cover_fx_for: None,
            lyrics_background: None,
            ambient: None,
            cover_accent: None,
            dj_on: false,
            dj_busy: false,
            dj_request: String::new(),
            dj_log: Vec::new(),
            dj_opened: false,
            speaking: false,
            resume_after_speech: false,
            no_system_voice: false,
            config,
        };
        let mut app = app;
        app.refresh_theme_names();
        let demo = std::env::var("HAZE_DEMO").ok();
        // HAZE_DEMO=setup (or setup-look, setup-spotify, setup-dj, setup-login, setup-done) shows the setup.
        if let Some(step) = demo.as_deref().and_then(|d| d.strip_prefix("setup")) {
            let step = match step {
                "-look" => SetupStep::Look,
                "-spotify" => SetupStep::Spotify,
                "-dj" => SetupStep::Dj,
                "-login" => SetupStep::Login,
                "-done" => SetupStep::Done,
                _ => SetupStep::Hello,
            };
            app.user_name = "Haz".into();
            app.setup = Some(Setup::new(step));
            return (app, Task::none());
        }
        if demo.is_some() {
            app.load_demo();
            if demo.as_deref() == Some("update") {
                app.update = Some(Update { release: update::Release::demo(), status: UpdateStatus::Available });
            }
            return (app, Task::none());
        }
        // Log straight back in when a previous login is cached. During the setup this waits for
        // its login step, so the browser doesn't open over the welcome screen.
        let login = if has_client && app.setup.is_none() && Auth::has_cached_login() { Task::done(Message::Login) } else { Task::none() };
        let check = if app.config.updates.check { Task::done(Message::CheckForUpdates(false)) } else { Task::none() };
        (app, Task::batch([login, check]))
    }

    fn title(&self) -> String {
        match self.queue.current() {
            Some(e) => format!("{} · {} — Haze", e.track.name, e.track.artist_line()),
            None => "Haze".into(),
        }
    }

    fn theme(&self) -> Theme {
        self.theme.clone()
    }

    fn subscription(&self) -> Subscription<Message> {
        let mut subs = vec![
            Subscription::run(player_events),
            keyboard::listen().map(Message::Key),
            iced::event::listen_with(|event, _, _| match event {
                iced::Event::Window(window::Event::FileDropped(path)) => Some(Message::FileDropped(path)),
                _ => None,
            }),
        ];
        if let Some(setup) = self.setup.as_ref().filter(|s| s.animating(&self.login)) {
            // Slides get every frame; the slower loops (the hello, waiting dots) run at 30 fps,
            // which looks the same and costs half the CPU.
            if setup.entered.elapsed() < Duration::from_millis(900) {
                subs.push(window::frames().map(|_| Message::Frame));
            } else {
                subs.push(iced::time::every(Duration::from_millis(33)).map(|_| Message::Frame));
            }
        }
        // Redraw at 30 fps for the smooth scroll and word fill, only while synced lyrics are on screen.
        if self.page == Page::Lyrics && self.playing && self.lyrics.as_ref().is_some_and(|l| l.synced) {
            subs.push(iced::time::every(Duration::from_millis(33)).map(|_| Message::Tick));
        }
        Subscription::batch(subs)
    }

    fn set_position(&mut self, ms: u32) {
        self.position_ms = ms;
        self.position_at = Instant::now();
    }

    /// The playback position right now, estimated between the player's half-second updates.
    pub fn live_position(&self) -> u32 {
        if !self.playing {
            return self.position_ms;
        }
        let elapsed = self.position_at.elapsed().as_millis().min(2_000) as u32;
        self.position_ms + elapsed
    }

    fn refresh_theme_names(&mut self) {
        self.theme_names = theme::PRESETS.iter().map(|s| s.to_string()).chain(theme::custom_themes()).collect();
    }

    /// Recomputes the blurred lyrics background and the cover accent when the song's cover changes.
    fn update_cover_fx(&mut self, force: bool) {
        let url = self.queue.current().and_then(|e| e.track.image_url.clone());
        if !force && url == self.cover_fx_for {
            return;
        }
        let bytes = url.as_ref().and_then(|u| match self.images.get(u) {
            Some(image::Handle::Bytes(_, bytes)) => Some(bytes.clone()),
            _ => None,
        });
        if url.is_some() && bytes.is_none() {
            return; // the cover is still downloading; Message::Image calls back
        }
        self.cover_fx_for = url;
        let base = theme::resolve(&self.config.appearance);
        self.lyrics_background = bytes.as_ref().and_then(|b| custom::blurred_cover(b, base.background, 0.5));
        self.ambient = bytes.as_ref().and_then(|b| custom::blurred_cover(b, base.background, 0.55));
        self.cover_accent = bytes.as_ref().and_then(|b| theme::accent_from_image(b));
        self.refresh_theme();
    }

    fn set_fullscreen(&mut self, on: bool) -> Task<Message> {
        if self.fullscreen == on {
            return Task::none();
        }
        self.fullscreen = on;
        let mode = if on { window::Mode::Fullscreen } else { window::Mode::Windowed };
        window::latest().and_then(move |id| window::set_mode(id, mode))
    }

    /// Looks up lyrics for the current song unless they're already loaded or loading.
    fn fetch_lyrics(&mut self) -> Task<Message> {
        let Some(track) = self.queue.current().map(|e| e.track.clone()) else {
            return Task::none();
        };
        if self.lyrics_for.as_deref() == Some(track.id.as_str()) {
            return Task::none();
        }
        self.lyrics_for = Some(track.id.clone());
        self.lyrics = None;
        self.lyrics_loading = true;
        let audio = self.audio.clone();
        let sources = self.config.lyrics.sources.clone();
        let settings = self.config.playback.clone();
        let id = track.id.clone();
        Task::perform(
            async move {
                for source in sources {
                    let found = match source.to_ascii_lowercase().as_str() {
                        "spotify" => {
                            let Some(audio) = &audio else { continue };
                            async {
                                audio.ensure_connected(&settings).await?;
                                let json = audio.spotify_lyrics(&track.id).await?;
                                lyrics::parse_spotify(&track, &json).map(Some)
                            }
                            .await
                        }
                        "lrclib" => lyrics::lrclib(&track).await,
                        other => {
                            log::warn!("unknown lyrics source {other:?}");
                            continue;
                        }
                    };
                    match found {
                        Ok(Some(found)) => return Ok(Some(found)),
                        Ok(None) => {}
                        Err(e) => log::debug!("lyrics from {source}: {e:#}"),
                    }
                }
                Ok(None)
            },
            move |r| Message::LyricsFound(id.clone(), r),
        )
    }

    /// A file dragged onto the window: a font is added, and an image becomes the cover of the
    /// playlist on screen, or the app icon from Settings or the setup.
    fn file_dropped(&mut self, path: PathBuf) -> Task<Message> {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or_default().to_ascii_lowercase();
        if ["ttf", "otf", "ttc"].contains(&ext.as_str()) {
            return Task::done(Message::FontPicked(Some(path)));
        }
        if !custom::IMAGE_TYPES.contains(&ext.as_str()) {
            self.notify("Haze can use PNG, JPEG, WebP, GIF, BMP, ICO, TIFF, TGA and QOI images, and TTF or OTF fonts.", true);
            return Task::none();
        }
        match &self.page {
            _ if self.setup.is_some() => Task::done(Message::IconPicked(Some(path))),
            Page::Settings => Task::done(Message::IconPicked(Some(path))),
            Page::Liked => Task::done(Message::CoverPicked("liked".into(), Some(path))),
            Page::Playlist(p) => Task::done(Message::CoverPicked(p.id.clone(), Some(path))),
            _ => {
                self.notify("Drop an image on a playlist to make it the cover, or on Settings to make it the app icon.", false);
                Task::none()
            }
        }
    }

    fn apply_icon(&mut self) -> Task<Message> {
        self.icon = icon_handle(&self.config.appearance.app_icon);
        let Some(icon) = custom::window_icon(&self.config.appearance.app_icon) else {
            return Task::none();
        };
        window::latest().and_then(move |id| window::set_icon(id, icon.clone()))
    }

    fn sync_menu_entry(&mut self) {
        if let Err(e) = custom::sync_menu_entry(&self.config.appearance.app_icon) {
            log::warn!("app menu entry: {e:#}");
        }
    }

    fn save_quietly(&mut self) {
        if let Err(e) = self.config.save() {
            self.notify(format!("Couldn't save config: {e:#}"), true);
        }
    }

    /// Moves the setup on, saving what each step collected.
    fn setup_next(&mut self) -> Task<Message> {
        let Some(step) = self.setup.as_ref().map(|s| s.step) else { return Task::none() };
        let next = match step {
            SetupStep::Hello => SetupStep::Look,
            SetupStep::Look => SetupStep::Spotify,
            SetupStep::Spotify => {
                let id = self.client_id_input.trim().to_string();
                if id.is_empty() {
                    return Task::none();
                }
                if id != self.config.spotify.client_id && self.login == Login::LoggedIn {
                    // A different Spotify app means logging in again.
                    let _ = self.update(Message::Logout);
                }
                self.config.spotify.client_id = id;
                if self.login == Login::NeedsClientId {
                    self.login = Login::LoggedOut;
                }
                SetupStep::Dj
            }
            SetupStep::Dj if self.login == Login::LoggedIn => SetupStep::Done,
            SetupStep::Dj => SetupStep::Login,
            SetupStep::Login => return Task::done(Message::Login),
            SetupStep::Done => return Task::done(Message::SetupFinish),
        };
        if step != SetupStep::Hello
            && let Err(e) = self.config.save()
        {
            self.notify(format!("Couldn't save settings: {e:#}"), true);
        }
        if let Some(setup) = &mut self.setup {
            setup.go(next);
        }
        Task::none()
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
        if self.config.appearance.accent_from_cover
            && let Some(accent) = self.cover_accent
        {
            self.colors.accent = accent;
        }
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
        let leave_fullscreen = if page == Page::Lyrics { Task::none() } else { self.set_fullscreen(false) };
        if page == Page::Lyrics && self.page != Page::Lyrics {
            self.previous_page = self.page.clone();
        }
        let extra = match &page {
            Page::Lyrics => self.fetch_lyrics(),
            Page::Playlist(p) => self.fetch_image(p.image_url.as_ref()),
            Page::Settings => {
                self.refresh_theme_names();
                Task::none()
            }
            _ => Task::none(),
        };
        Task::batch([leave_fullscreen, extra, self.open_page(page)])
    }

    fn open_page(&mut self, page: Page) -> Task<Message> {
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
                let audio = self.audio.clone();
                let settings = self.config.playback.clone();
                let load = async move {
                    match api.playlist_tracks(&id, None).await {
                        Ok(tracks) => Ok(tracks),
                        // Developer apps can't read some playlists (Spotify's own, and some other
                        // people's), so read it with the playback login instead.
                        Err(e) if e.to_string().contains("403") => {
                            let Some(audio) = audio else { return Err(e) };
                            let blocked = || {
                                anyhow::anyhow!("Spotify won't let Haze open this playlist. Try saving its songs to a playlist of your own.")
                            };
                            let token = audio.web_token(&settings).await.map_err(|e| {
                                log::warn!("couldn't get a playback login token for a 403 playlist: {e}");
                                blocked()
                            })?;
                            api.playlist_tracks(&id, Some(&token)).await.map_err(|e| {
                                log::warn!("playlist still refused with the playback login: {e}");
                                blocked()
                            })
                        }
                        Err(e) => Err(e),
                    }
                };
                Some(Task::perform(async move { load.await.map_err(err) }, {
                    let page = page.clone();
                    move |r| Message::Tracks(page.clone(), r)
                }))
            }
            Page::Search => {
                self.tracks.clear();
                None
            }
            Page::Dj | Page::Settings | Page::Lyrics => None,
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
        self.set_position(0);
        self.preloaded = None;
        self.update_cover_fx(false);
        let lyrics = if self.page == Page::Lyrics { self.fetch_lyrics() } else { Task::none() };

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

        let mut tasks = vec![load, self.fetch_image(entry.track.image_url.as_ref()), lyrics, self.maybe_refill()];
        if let Some(text) = intro {
            tasks.push(self.speak(text, start_playing));
        }
        Task::batch(tasks)
    }

    fn speak(&mut self, text: String, music_playing: bool) -> Task<Message> {
        let Some(audio) = self.audio.clone() else {
            return Task::none();
        };
        if self.no_system_voice && self.config.dj.tts.engine == crate::config::TtsEngine::System {
            return if music_playing { Task::none() } else { Task::done(Message::SpeechDone(Ok(()))) };
        }
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
                if let Some(setup) = self.setup.as_mut().filter(|s| s.step == SetupStep::Login) {
                    setup.go(SetupStep::Done);
                }
                self.api = Some(api.clone());
                let tx = PLAYER_TX.get().expect("player channel").clone();
                let (a1, a2) = (api.clone(), api.clone());
                let playback = self.config.playback.clone();
                if !crate::player::has_playback_login() {
                    self.notify("Your browser opened Spotify once more so Haze can play music. Approve it there.", false);
                }
                return Task::batch([
                    Task::perform(
                        async move { Audio::connect(playback, tx).await.map(Arc::new).map_err(err) },
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
            Message::AudioReady(Ok(audio)) => {
                self.audio = Some(audio);
                if self.notice.as_ref().is_some_and(|n| n.text.starts_with("Your browser opened Spotify once more")) {
                    self.notice = None;
                }
            }
            Message::AudioReady(Err(e)) => self.notify(e, true),
            Message::Profile(Ok(name)) => self.user_name = name,
            Message::Playlists(Ok(list)) => {
                self.playlists = list;
                // Covers for the sidebar. They stay loaded (see Message::Image).
                let urls: Vec<String> = self.playlists.iter().filter_map(|p| p.image_url.clone()).collect();
                return Task::batch(urls.iter().map(|u| self.fetch_image(Some(u))));
            }
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
            Message::SetupNext => return self.setup_next(),
            Message::SetupBack => {
                if let Some(setup) = &mut self.setup
                    && let Some(i) = setup.step.index().checked_sub(1)
                {
                    setup.go(SetupStep::ALL[i]);
                }
            }
            Message::SetupFinish => {
                self.setup = None;
                if !self.config.setup_done {
                    self.config.setup_done = true;
                    if let Err(e) = self.config.save() {
                        self.notify(format!("Couldn't save config: {e:#}"), true);
                    }
                }
                if self.login != Login::LoggedIn {
                    self.login = if self.config.spotify.client_id.trim().is_empty() {
                        Login::NeedsClientId
                    } else {
                        Login::LoggedOut
                    };
                }
            }
            Message::RunSetup => self.setup = Some(Setup::new(SetupStep::Hello)),
            Message::OpenUrl(url) => custom::open_url(&url),
            Message::CopyRedirectUri => {
                if let Some(setup) = &mut self.setup {
                    setup.copied = Some(Instant::now());
                }
                return iced::clipboard::write(self.config.spotify.redirect_uri.clone());
            }
            Message::Frame => {}

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
                    self.set_position(v as u32);
                }
            }
            Message::SeekStep(delta) => {
                let duration = self.queue.current().map(|e| e.track.duration_ms as i64).unwrap_or(0);
                if let Some(audio) = &self.audio {
                    let target = (self.position_ms as i64 + delta).clamp(0, (duration - 1000).max(0));
                    audio.seek(target as u32);
                    self.set_position(target as u32);
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
                // Playlist covers are always on screen in the sidebar, so only song covers expire.
                if !self.playlists.iter().any(|p| p.image_url.as_ref() == Some(&url)) {
                    self.image_order.push(url);
                }
                while self.image_order.len() > IMAGE_CACHE {
                    let old = self.image_order.remove(0);
                    self.images.remove(&old);
                }
                self.update_cover_fx(false);
            }
            Message::Image(_, Err(e)) => log::debug!("cover art: {e}"),

            Message::ToggleLyrics => {
                if self.page == Page::Lyrics {
                    // Back to where you were, without reloading it.
                    self.page = self.previous_page.clone();
                    return self.set_fullscreen(false);
                }
                return self.load_page(Page::Lyrics);
            }
            Message::ToggleFullscreen => {
                let on = !self.fullscreen;
                return self.set_fullscreen(on);
            }
            Message::LyricsFound(id, result) => {
                if self.lyrics_for.as_deref() == Some(id.as_str()) {
                    self.lyrics_loading = false;
                    match result {
                        Ok(found) => self.lyrics = found,
                        Err(e) => log::warn!("lyrics: {e}"),
                    }
                }
            }
            Message::SeekTo(ms) => {
                if let Some(audio) = &self.audio {
                    audio.seek(ms);
                }
                self.set_position(ms);
            }
            Message::Tick => {}

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
                match result {
                    // Said once; after that the DJ just talks on screen.
                    Err(e) if e == dj::tts::NO_SYSTEM_VOICE => {
                        self.no_system_voice = true;
                        self.notify(e, false);
                    }
                    Err(e) => self.notify(format!("DJ voice: {e}"), true),
                    Ok(()) => {}
                }
            }

            Message::Edit(edit) => {
                self.apply_edit(edit);
                // Picks up a change of lyrics provider (a no-op when the lyrics are current).
                if self.page == Page::Lyrics {
                    return self.fetch_lyrics();
                }
            }
            Message::PickIcon => {
                return Task::perform(custom::pick_image("Choose an app icon"), |r| match r {
                    Ok(path) => Message::IconPicked(path),
                    Err(e) => Message::PickFailed(e),
                });
            }
            Message::IconPicked(None) | Message::CoverPicked(_, None) | Message::FontPicked(None) => {}
            Message::IconPicked(Some(path)) => match custom::import_image(&path, "icons", "icon", 512) {
                Ok(saved) => {
                    custom::remove_imported(&self.config.appearance.app_icon, "icons");
                    self.config.appearance.app_icon = saved.display().to_string();
                    self.save_quietly();
                    self.sync_menu_entry();
                    self.notify(
                        if cfg!(target_os = "macos") {
                            "Icon set for the window. To change the Dock icon, use Get Info on Haze.app in Finder."
                        } else {
                            "App icon updated"
                        },
                        false,
                    );
                    return self.apply_icon();
                }
                Err(e) => self.notify(format!("Couldn't use that image: {e:#}"), true),
            },
            Message::SetIcon(name) => {
                custom::remove_imported(&self.config.appearance.app_icon, "icons");
                self.config.appearance.app_icon = if name == "haze" { String::new() } else { name.to_string() };
                self.save_quietly();
                self.sync_menu_entry();
                return self.apply_icon();
            }
            Message::PickCover(key) => {
                return Task::perform(
                    async move { (key, custom::pick_image("Choose a cover").await) },
                    |(key, r)| match r {
                        Ok(path) => Message::CoverPicked(key, path),
                        Err(e) => Message::PickFailed(e),
                    },
                );
            }
            Message::CoverPicked(key, Some(path)) => match custom::import_image(&path, "covers", &key, 640) {
                Ok(saved) => {
                    if let Some(old) = self.config.covers.insert(key, saved.display().to_string()) {
                        custom::remove_imported(&old, "covers");
                    }
                    self.save_quietly();
                }
                Err(e) => self.notify(format!("Couldn't use that image: {e:#}"), true),
            },
            Message::ResetCover(key) => {
                if let Some(old) = self.config.covers.remove(&key) {
                    custom::remove_imported(&old, "covers");
                    self.save_quietly();
                }
            }
            Message::PickFont => {
                return Task::perform(custom::pick_font(), |r| match r {
                    Ok(path) => Message::FontPicked(path),
                    Err(e) => Message::PickFailed(e),
                });
            }
            Message::PickFailed(e) => self.notify(e, true),
            Message::FileDropped(path) => return self.file_dropped(path),
            Message::FontPicked(Some(path)) => {
                let added = std::fs::read(&path).map_err(anyhow::Error::from).and_then(|bytes| {
                    let family = custom::font_family(&bytes).ok_or_else(|| anyhow::anyhow!("not a font file"))?;
                    let dir = crate::config::data_dir("fonts")?;
                    let name = path.file_name().ok_or_else(|| anyhow::anyhow!("no file name"))?;
                    if path.parent() != Some(dir.as_path()) {
                        std::fs::copy(&path, dir.join(name))?;
                    }
                    Ok((family, bytes))
                });
                match added {
                    Ok((family, bytes)) => {
                        if !self.font_families.contains(&family) {
                            self.font_families.push(family.clone());
                        }
                        self.notify(format!("Added {family}. Pick it as the app or lyrics font below."), false);
                        return iced::font::load(bytes).map(Message::FontLoaded);
                    }
                    Err(e) => self.notify(format!("Couldn't add that font: {e:#}"), true),
                }
            }
            Message::FontLoaded(result) => {
                if let Err(e) = result {
                    log::warn!("font: {e:?}");
                }
            }
            Message::OpenConfigFolder => {
                if let Some(dir) = crate::config::config_path().ok().and_then(|p| p.parent().map(PathBuf::from)) {
                    for folder in ["themes", "fonts"] {
                        let _ = crate::config::data_dir(folder);
                    }
                    custom::open_folder(&dir);
                }
            }
            Message::SaveSettings => match self.config.save() {
                Ok(()) => self.notify("Settings saved", false),
                Err(e) => self.notify(format!("Couldn't save settings: {e:#}"), true),
            },
            Message::ToggleSidePanel => self.show_side_panel = !self.show_side_panel,
            Message::DismissNotice => self.notice = None,
            Message::CheckForUpdates(asked) => {
                return Task::perform(async { update::check().await.map_err(err) }, move |r| Message::UpdateChecked(asked, r));
            }
            Message::UpdateChecked(asked, result) => match result {
                Ok(Some(release)) => self.update = Some(Update { release, status: UpdateStatus::Available }),
                Ok(None) if asked => self.notify(format!("Haze {} is up to date", env!("CARGO_PKG_VERSION")), false),
                Ok(None) => {}
                Err(e) if asked => self.notify(format!("Couldn't check for updates: {e}"), true),
                Err(e) => log::info!("update check: {e}"),
            },
            Message::InstallUpdate => {
                if let Some(u) = &mut self.update {
                    u.status = UpdateStatus::Installing;
                    let release = u.release.clone();
                    return Task::perform(async move { update::install(release).await.map_err(err) }, Message::UpdateInstalled);
                }
            }
            Message::UpdateInstalled(result) => {
                if let Some(u) = &mut self.update {
                    u.status = match result {
                        Ok(program) => UpdateStatus::Installed(program),
                        Err(e) => UpdateStatus::Failed(e),
                    };
                }
            }
            Message::RestartForUpdate => {
                if let Some(Update { status: UpdateStatus::Installed(program), .. }) = &self.update {
                    let _ = self.config.save();
                    update::restart(program);
                }
            }
            Message::DismissUpdate => self.update = None,
            Message::SettingsTab(tab) => self.settings_tab = tab,
            Message::Key(event) => {
                if self.fullscreen
                    && let keyboard::Event::KeyPressed { key: keyboard::Key::Named(keyboard::key::Named::Escape), .. } =
                        &event
                {
                    return self.set_fullscreen(false);
                }
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
                self.unavailable_in_a_row = 0;
                self.playing = true;
                self.set_position(position_ms);
            }
            PlayerUpdate::Paused { uri, position_ms } if is_current(&uri) => {
                self.playing = false;
                self.set_position(position_ms);
            }
            PlayerUpdate::Position { uri, position_ms } if is_current(&uri) => {
                self.set_position(position_ms);
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
                self.unavailable_in_a_row += 1;
                let why = crate::player::take_last_problem().map(|p| format!(" Spotify said: {p}")).unwrap_or_default();
                if self.unavailable_in_a_row >= 3 {
                    // Something is wrong with playback as a whole; don't run through the whole queue.
                    self.unavailable_in_a_row = 0;
                    self.playing = false;
                    self.notify(format!("Spotify wouldn't play the last few songs, so playback stopped.{why}"), true);
                    return Task::none();
                }
                self.notify(format!("That song isn't available, so it was skipped.{why}"), true);
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
        let recolour_cover = matches!(edit, Theme(_) | AccentFromCover(_));
        let a = &mut self.config.appearance;
        let dj = &mut self.config.dj;
        let ly = &mut self.config.lyrics;
        let font_name = |s: String| match s.as_str() {
            DEFAULT_FONT => String::new(),
            SYSTEM_FONT => "system".to_string(),
            _ => s,
        };
        match edit {
            Theme(t) => a.theme = t,
            Accent(c) => a.colors.accent = c,
            FontSize(v) => a.font_size = v,
            Radius(v) => a.corner_radius = v,
            Compact(v) => a.compact = v,
            AlbumArt(v) => a.show_album_art = v,
            AccentFromCover(v) => a.accent_from_cover = v,
            AmbientBackground(v) => a.ambient_background = v,
            AppFont(s) => a.font_family = font_name(s),
            LyricsFont(s) => ly.font_family = font_name(s),
            WordHighlight(v) => ly.word_highlight = v,
            BlurredBackground(v) => ly.blurred_background = v,
            LyricsAlign(v) => ly.align = v,
            LyricsProvider(p) => {
                ly.sources = p.sources();
                // Look the song up again with the new provider.
                self.lyrics_for = None;
            }
            LyricsSize(v) => ly.font_size = v,
            LyricsOffset(v) => ly.offset_ms = v as i32,
            LinesBefore(v) => ly.lines_before = v as usize,
            Sidebar(s) => self.config.layout.sidebar = s,
            SidePanel(s) => self.config.layout.side_panel = s,
            PlayerBar(e) => self.config.layout.player_bar = e,
            Provider(p) if p != dj.provider => {
                // A model or URL typed for the old provider would only make the new one fail.
                dj.provider = p;
                dj.model.clear();
                dj.base_url.clear();
                dj.api_key_env.clear();
            }
            Provider(_) => {}
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
            // Try the system voice again, in case a speech program was installed since.
            TtsEngine(e) => {
                dj.tts.engine = e;
                self.no_system_voice = false;
            }
            Voice(s) => dj.tts.voice = s,
            TtsModel(s) => dj.tts.model = s,
            DuckTo(v) => dj.tts.duck_to = v as u8,
            CheckUpdates(v) => self.config.updates.check = v,
        }
        if recolour_cover {
            self.update_cover_fx(true);
        }
        self.refresh_theme();
    }

    /// `HAZE_DEMO=1` fills the UI with sample data (no Spotify login) for previewing themes and layouts.
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
        self.setup = None;
        self.user_name = "Haz".into();
        self.playlists = ["Late Night Drive", "Gym", "Focus", "Indie Finds"]
            .iter()
            .map(|n| Playlist {
                id: n.to_string(),
                name: n.to_string(),
                owner: "Haz".into(),
                track_count: 42,
                image_url: None,
            })
            .collect();
        self.tracks = vec![
            t("Midnight City", "M83", "Hurry Up, We're Dreaming", 243_000),
            t("Electric Feel", "MGMT", "Oracular Spectacular", 229_000),
            t("Breezeblocks", "alt-J", "An Awesome Wave", 227_000),
            t("Redbone", "Childish Gambino", "Awaken, My Love!", 327_000),
            t("Do I Wanna Know?", "Arctic Monkeys", "AM", 272_000),
        ];
        // The built-in icon stands in for cover art, so the lyrics background and cover accent show.
        self.images.insert("demo:cover".into(), image::Handle::from_bytes(custom::BUILTIN_ICON));
        self.tracks[0].image_url = Some("demo:cover".into());
        let tracks = self.tracks.clone();
        self.queue.play_list(&tracks, 0);
        for (i, e) in tracks.iter().enumerate().skip(1).take(2) {
            self.queue.push(Entry { track: e.clone(), origin: Origin::Dj { intro: Some(format!("intro {i}")) } });
        }
        self.page = match std::env::var("HAZE_DEMO").as_deref() {
            Ok("settings") => Page::Settings,
            Ok("settings-look") => {
                self.settings_tab = SettingsTab::Look;
                Page::Settings
            }
            Ok("tracks") => Page::Liked,
            Ok("lyrics") => Page::Lyrics,
            _ => Page::Dj,
        };
        let lines = [
            "City lights are humming low",
            "Every window holds a glow",
            "We keep driving, we don't know",
            "Where the night is gonna go",
            "Turn it up and let it ride",
            "Static singing on the side",
            "Hold the moment, hold it tight",
            "We were made for one more night",
        ];
        let lrc: String = lines
            .iter()
            .enumerate()
            .map(|(i, l)| {
                let ms = 70_000 + i as u32 * 4_200;
                format!("[{:02}:{:05.2}]{l}\n", ms / 60_000, (ms % 60_000) as f32 / 1000.0)
            })
            .collect();
        self.lyrics = lyrics::parse_lrc(&tracks[0], &lrc, "the demo");
        self.lyrics_for = Some(tracks[0].id.clone());
        self.set_position(81_000);
        self.update_cover_fx(true);
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

fn icon_handle(app_icon: &str) -> image::Handle {
    match custom::builtin_icon(app_icon) {
        Some(bytes) => image::Handle::from_bytes(bytes),
        None if std::path::Path::new(app_icon.trim()).exists() => image::Handle::from_path(app_icon.trim()),
        None => image::Handle::from_bytes(custom::BUILTIN_ICON),
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
