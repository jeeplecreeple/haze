//! User configuration, stored as TOML in the platform config directory:
//! Linux `~/.config/haze/config.toml`, macOS `~/Library/Application Support/haze/config.toml`,
//! Windows `%APPDATA%\haze\config\config.toml`.
//!
//! Every field has a default, so a partial file (or none at all) is valid.

use std::path::PathBuf;

use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

pub fn dirs() -> Result<ProjectDirs> {
    ProjectDirs::from("", "", "haze").context("could not determine a home directory")
}

/// The app's name before it was renamed. Its folders are moved over on first launch.
pub const OLD_NAME: &str = "haztify";

/// Moves the config and cache folders from the old name to the new one, once, and points any
/// saved paths (icon, covers, fonts) at the new folder. Does nothing when the new folders exist.
fn migrate_old_folders() {
    let (Some(old), Ok(new)) = (ProjectDirs::from("", "", OLD_NAME), dirs()) else { return };
    if old.config_dir() == new.config_dir() {
        return;
    }
    for (from, to) in [(old.config_dir(), new.config_dir()), (old.cache_dir(), new.cache_dir())] {
        if !from.exists() || to.exists() {
            continue;
        }
        let moved = to.parent().map_or(Ok(()), std::fs::create_dir_all).and_then(|_| std::fs::rename(from, to));
        match moved {
            Ok(()) => log::info!("moved {} to {}", from.display(), to.display()),
            Err(e) => log::warn!("couldn't move {} to {}: {e}", from.display(), to.display()),
        }
    }
    let path = new.config_dir().join("config.toml");
    let Ok(text) = std::fs::read_to_string(&path) else { return };
    let (from, to) = (old.config_dir().to_string_lossy(), new.config_dir().to_string_lossy());
    // TOML strings escape backslashes, so Windows paths appear doubled in the file.
    let fixed = text
        .replace(from.as_ref(), to.as_ref())
        .replace(&from.replace('\\', "\\\\"), &to.replace('\\', "\\\\"))
        .replace(&format!("theme = \"{OLD_NAME}\""), "theme = \"haze\"");
    if fixed != text {
        let _ = std::fs::write(&path, fixed);
    }
}

pub fn config_path() -> Result<PathBuf> {
    Ok(dirs()?.config_dir().join("config.toml"))
}

pub fn cache_dir() -> Result<PathBuf> {
    Ok(dirs()?.cache_dir().to_path_buf())
}

/// A folder next to the config file, created on demand: `fonts`, `themes`, `covers`...
pub fn data_dir(name: &str) -> Result<PathBuf> {
    let dir = dirs()?.config_dir().join(name);
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Set once the first-run setup is finished, so it isn't shown again.
    pub setup_done: bool,
    pub spotify: SpotifyConfig,
    pub playback: PlaybackConfig,
    pub appearance: Appearance,
    pub layout: Layout,
    pub dj: DjConfig,
    pub lyrics: LyricsConfig,
    pub keys: Keybindings,
    pub updates: UpdateConfig,
    /// Your own cover images, by playlist id ("liked" for Liked Songs). Set them from the app.
    pub covers: std::collections::BTreeMap<String, String>,
}

impl Config {
    /// Loads the config, writing a default one on first run so users have something to edit.
    pub fn load_or_create() -> Result<Self> {
        migrate_old_folders();
        let path = config_path()?;
        if !path.exists() {
            let config = Config::default();
            config.save()?;
            return Ok(config);
        }
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
    }

    pub fn save(&self) -> Result<()> {
        let path = config_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = toml::to_string_pretty(self)?;
        std::fs::write(&path, text)?;
        restrict_permissions(&path);
        Ok(())
    }
}

/// The config can hold an LLM API key, so keep it readable by the owner only.
pub fn restrict_permissions(path: &std::path::Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    #[cfg(not(unix))]
    let _ = path;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SpotifyConfig {
    /// Client ID of your own app from https://developer.spotify.com/dashboard
    pub client_id: String,
    /// Must be registered as a Redirect URI on that app, exactly as written here.
    pub redirect_uri: String,
}

impl Default for SpotifyConfig {
    fn default() -> Self {
        Self {
            client_id: String::new(),
            redirect_uri: "http://127.0.0.1:8898/login".into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Bitrate {
    Low,
    Normal,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UpdateConfig {
    /// Look for a new release on GitHub when Haze starts.
    pub check: bool,
}

impl Default for UpdateConfig {
    fn default() -> Self {
        Self { check: true }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PlaybackConfig {
    /// low = 96 kbps, normal = 160 kbps, high = 320 kbps
    pub bitrate: Bitrate,
    /// 0-100
    pub volume: u8,
    pub normalisation: bool,
    pub gapless: bool,
    /// Megabytes of audio to keep on disk so replays don't re-download. 0 disables the cache.
    pub audio_cache_mb: u64,
}

impl Default for PlaybackConfig {
    fn default() -> Self {
        Self {
            bitrate: Bitrate::High,
            volume: 70,
            normalisation: true,
            gapless: true,
            audio_cache_mb: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Appearance {
    /// A built-in preset (haze, spotify, apple-music, apple-music-dark, nord, gruvbox,
    /// catppuccin, dracula, light) or the name of a file in the `themes` folder.
    /// Any colour set in `colors` overrides the theme.
    pub theme: String,
    pub colors: ColorOverrides,
    pub font_size: f32,
    /// Leave empty for the default font, or name an installed font family or one in the
    /// `fonts` folder next to this file (.ttf/.otf files there are loaded automatically).
    pub font_family: String,
    /// Extra font files to load, anywhere on disk.
    pub font_files: Vec<String>,
    /// The app icon: a built-in one by name (haze, sunset, ocean, mint, midnight, mono, aurora,
    /// ember) or a PNG/JPG file. Empty = haze.
    pub app_icon: String,
    /// Tint the accent colour from the current song's cover art.
    pub accent_from_cover: bool,
    /// A blurred glow of the current song's cover behind the whole window.
    pub ambient_background: bool,
    pub corner_radius: f32,
    pub show_album_art: bool,
    /// Compact rows fit more tracks on screen.
    pub compact: bool,
    pub window_width: f32,
    pub window_height: f32,
    /// "gpu" (wgpu: Vulkan, Metal or DirectX) or "cpu" (software, lowest memory).
    pub renderer: String,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            theme: "haze".into(),
            colors: ColorOverrides::default(),
            font_size: 14.0,
            font_family: String::new(),
            font_files: Vec::new(),
            app_icon: String::new(),
            accent_from_cover: false,
            ambient_background: true,
            corner_radius: 8.0,
            show_album_art: true,
            compact: false,
            window_width: 1100.0,
            window_height: 720.0,
            renderer: "cpu".into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LyricsAlign {
    Left,
    Center,
}

impl LyricsAlign {
    pub const ALL: [LyricsAlign; 2] = [LyricsAlign::Left, LyricsAlign::Center];
}

impl std::fmt::Display for LyricsAlign {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            LyricsAlign::Left => "Left",
            LyricsAlign::Center => "Centre",
        })
    }
}

/// The choices for `lyrics.sources` in Settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LyricsProvider {
    SpotifyFirst,
    LrclibFirst,
    SpotifyOnly,
    LrclibOnly,
}

impl LyricsProvider {
    pub const ALL: [LyricsProvider; 4] = [Self::SpotifyFirst, Self::LrclibFirst, Self::SpotifyOnly, Self::LrclibOnly];

    pub fn sources(self) -> Vec<String> {
        let list: &[&str] = match self {
            Self::SpotifyFirst => &["spotify", "lrclib"],
            Self::LrclibFirst => &["lrclib", "spotify"],
            Self::SpotifyOnly => &["spotify"],
            Self::LrclibOnly => &["lrclib"],
        };
        list.iter().map(|s| s.to_string()).collect()
    }

    /// The choice matching a config's `sources`, if it's one of these.
    pub fn of(sources: &[String]) -> Option<Self> {
        let lower: Vec<String> = sources.iter().map(|s| s.trim().to_ascii_lowercase()).collect();
        Self::ALL.into_iter().find(|p| p.sources() == lower)
    }
}

impl std::fmt::Display for LyricsProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::SpotifyFirst => "Spotify, then LRCLIB",
            Self::LrclibFirst => "LRCLIB, then Spotify",
            Self::SpotifyOnly => "Spotify only",
            Self::LrclibOnly => "LRCLIB only",
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LyricsConfig {
    /// Where to look, in order: "spotify" (Spotify's own lyrics) and "lrclib" (lrclib.net).
    pub sources: Vec<String>,
    /// Light up each word as it's sung, not just the line.
    pub word_highlight: bool,
    /// A blurred, darkened copy of the cover art behind the lyrics.
    pub blurred_background: bool,
    pub font_size: f32,
    /// Empty = the app font.
    pub font_family: String,
    pub align: LyricsAlign,
    /// Shift lyrics earlier (negative) or later (positive), in milliseconds.
    pub offset_ms: i32,
    /// Lines shown before the current one.
    pub lines_before: usize,
}

impl Default for LyricsConfig {
    fn default() -> Self {
        Self {
            sources: vec!["spotify".into(), "lrclib".into()],
            word_highlight: true,
            blurred_background: true,
            font_size: 34.0,
            font_family: String::new(),
            align: LyricsAlign::Left,
            offset_ms: 0,
            lines_before: 2,
        }
    }
}

/// Hex colours like "#1db954". Empty strings fall back to the preset.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ColorOverrides {
    pub background: String,
    pub surface: String,
    pub text: String,
    pub muted_text: String,
    pub accent: String,
    pub danger: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Edge {
    Top,
    Bottom,
}

impl Side {
    pub const ALL: [Side; 2] = [Side::Left, Side::Right];
}

impl Edge {
    pub const ALL: [Edge; 2] = [Edge::Top, Edge::Bottom];
}

impl std::fmt::Display for Side {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Side::Left => "Left",
            Side::Right => "Right",
        })
    }
}

impl std::fmt::Display for Edge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Edge::Top => "Top",
            Edge::Bottom => "Bottom",
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Layout {
    pub sidebar: Side,
    pub sidebar_width: f32,
    pub player_bar: Edge,
    /// Panel with the up-next queue and the DJ console.
    pub show_side_panel: bool,
    pub side_panel: Side,
    pub side_panel_width: f32,
    /// Columns shown in track lists, in order: title, artist, album, duration.
    pub track_columns: Vec<String>,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            sidebar: Side::Left,
            sidebar_width: 220.0,
            player_bar: Edge::Bottom,
            show_side_panel: true,
            side_panel: Side::Right,
            side_panel_width: 300.0,
            track_columns: vec!["title".into(), "artist".into(), "album".into(), "duration".into()],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LlmProvider {
    /// OpenAI, or any other OpenAI-compatible API via `base_url` (Together, LM Studio...).
    OpenAI,
    Anthropic,
    Gemini,
    // The rest speak the OpenAI API; they only differ in their defaults.
    Mistral,
    Groq,
    OpenRouter,
    DeepSeek,
    Xai,
    Cerebras,
    /// Models running on your own computer. Needs no key.
    Ollama,
}

impl LlmProvider {
    pub const ALL: [LlmProvider; 10] = [
        LlmProvider::OpenAI,
        LlmProvider::Anthropic,
        LlmProvider::Gemini,
        LlmProvider::Mistral,
        LlmProvider::Groq,
        LlmProvider::OpenRouter,
        LlmProvider::DeepSeek,
        LlmProvider::Xai,
        LlmProvider::Cerebras,
        LlmProvider::Ollama,
    ];

    /// False where a key is optional: local servers, or a custom OpenAI-compatible URL.
    pub fn needs_key(self) -> bool {
        !matches!(self, LlmProvider::OpenAI | LlmProvider::Ollama)
    }

    pub fn default_model(self) -> &'static str {
        match self {
            LlmProvider::OpenAI => "gpt-4o-mini",
            LlmProvider::Anthropic => "claude-opus-5-5",
            LlmProvider::Gemini => "gemini-3.5-flash",
            LlmProvider::Mistral => "mistral-small-latest",
            LlmProvider::Groq => "openai/gpt-oss-120b",
            LlmProvider::OpenRouter => "openrouter/free",
            LlmProvider::DeepSeek => "deepseek-chat",
            LlmProvider::Xai => "grok-4-1-fast-non-reasoning",
            LlmProvider::Cerebras => "gpt-oss-120b",
            LlmProvider::Ollama => "llama3.2",
        }
    }

    pub fn default_base_url(self) -> &'static str {
        match self {
            LlmProvider::OpenAI => "https://api.openai.com/v1",
            LlmProvider::Anthropic => "https://api.anthropic.com/v1",
            LlmProvider::Gemini => "https://generativelanguage.googleapis.com/v1beta",
            LlmProvider::Mistral => "https://api.mistral.ai/v1",
            LlmProvider::Groq => "https://api.groq.com/openai/v1",
            LlmProvider::OpenRouter => "https://openrouter.ai/api/v1",
            LlmProvider::DeepSeek => "https://api.deepseek.com",
            LlmProvider::Xai => "https://api.x.ai/v1",
            LlmProvider::Cerebras => "https://api.cerebras.ai/v1",
            LlmProvider::Ollama => "http://localhost:11434/v1",
        }
    }

    pub fn default_key_env(self) -> &'static str {
        match self {
            LlmProvider::OpenAI => "OPENAI_API_KEY",
            LlmProvider::Anthropic => "ANTHROPIC_API_KEY",
            LlmProvider::Gemini => "GEMINI_API_KEY",
            LlmProvider::Mistral => "MISTRAL_API_KEY",
            LlmProvider::Groq => "GROQ_API_KEY",
            LlmProvider::OpenRouter => "OPENROUTER_API_KEY",
            LlmProvider::DeepSeek => "DEEPSEEK_API_KEY",
            LlmProvider::Xai => "XAI_API_KEY",
            LlmProvider::Cerebras => "CEREBRAS_API_KEY",
            LlmProvider::Ollama => "OLLAMA_API_KEY",
        }
    }
}

impl std::fmt::Display for LlmProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            LlmProvider::OpenAI => "OpenAI / compatible",
            LlmProvider::Anthropic => "Anthropic (Claude)",
            LlmProvider::Gemini => "Google Gemini",
            LlmProvider::Mistral => "Mistral",
            LlmProvider::Groq => "Groq (free tier)",
            LlmProvider::OpenRouter => "OpenRouter (free models)",
            LlmProvider::DeepSeek => "DeepSeek",
            LlmProvider::Xai => "xAI (Grok)",
            LlmProvider::Cerebras => "Cerebras (free tier)",
            LlmProvider::Ollama => "Ollama (on this computer)",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DjMode {
    /// DJ only picks and queues songs.
    Queue,
    /// DJ also writes short intros shown in the app.
    Talk,
    /// DJ intros are spoken out loud over the music.
    Voice,
}

impl DjMode {
    pub const ALL: [DjMode; 3] = [DjMode::Queue, DjMode::Talk, DjMode::Voice];
}

impl std::fmt::Display for DjMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            DjMode::Queue => "Only queue songs",
            DjMode::Talk => "Queue + text intros",
            DjMode::Voice => "Queue + spoken intros",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TtsEngine {
    /// The OS voice: `say` on macOS, `espeak-ng`/`spd-say` on Linux, SAPI on Windows.
    System,
    /// An OpenAI-compatible `/audio/speech` endpoint.
    OpenAI,
    /// Piper, a free natural-sounding voice that runs on this computer.
    Piper,
}

impl TtsEngine {
    pub const ALL: [TtsEngine; 3] = [TtsEngine::System, TtsEngine::Piper, TtsEngine::OpenAI];
}

impl std::fmt::Display for TtsEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            TtsEngine::System => "System voice",
            TtsEngine::OpenAI => "OpenAI-compatible TTS",
            TtsEngine::Piper => "Piper (natural, offline)",
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DjConfig {
    pub provider: LlmProvider,
    /// Leave empty to use the provider's default model.
    pub model: String,
    /// Leave empty for the provider's official endpoint.
    pub base_url: String,
    /// Prefer `api_key_env`; a key written here is stored in plain text in this file.
    pub api_key: String,
    /// Environment variable holding the key. Empty = the provider's usual one (e.g. OPENAI_API_KEY).
    pub api_key_env: String,
    pub mode: DjMode,
    /// Name the DJ uses for itself.
    pub name: String,
    /// Free-form personality and style instructions.
    pub persona: String,
    /// How many songs to ask for each time the DJ refills the queue.
    pub batch_size: usize,
    /// Refill when this many or fewer songs remain in the queue.
    pub refill_at: usize,
    /// Talk before every Nth DJ song (1 = every song). Only used in talk/voice modes.
    pub talk_every: usize,
    /// Upper bound on words per intro.
    pub max_words: usize,
    /// 0 = only songs you already love, 100 = all new discoveries.
    pub discovery: u8,
    pub allow_explicit: bool,
    /// Language the DJ speaks in.
    pub language: String,
    pub temperature: f32,
    pub tts: TtsConfig,
}

impl Default for DjConfig {
    fn default() -> Self {
        Self {
            provider: LlmProvider::OpenAI,
            model: String::new(),
            base_url: String::new(),
            api_key: String::new(),
            api_key_env: String::new(),
            mode: DjMode::Talk,
            name: "Haz DJ".into(),
            persona: "Warm, upbeat late-night radio host. Knows music history and loves a good segue."
                .into(),
            batch_size: 5,
            refill_at: 1,
            talk_every: 2,
            max_words: 45,
            discovery: 40,
            allow_explicit: true,
            language: "English".into(),
            temperature: 0.9,
            tts: TtsConfig::default(),
        }
    }
}

impl DjConfig {
    pub fn model(&self) -> String {
        non_empty(&self.model).unwrap_or(self.provider.default_model()).to_string()
    }

    pub fn base_url(&self) -> String {
        non_empty(&self.base_url)
            .unwrap_or(self.provider.default_base_url())
            .trim_end_matches('/')
            .to_string()
    }

    /// The key from the config, else from the environment. Local servers (Ollama, LM Studio) need none.
    pub fn api_key(&self) -> Option<String> {
        if let Some(key) = non_empty(&self.api_key) {
            return Some(key.to_string());
        }
        let var = non_empty(&self.api_key_env).unwrap_or(self.provider.default_key_env());
        std::env::var(var).ok().filter(|k| !k.trim().is_empty())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TtsConfig {
    pub engine: TtsEngine,
    /// System: a voice name your OS knows (empty = default). OpenAI: alloy, echo, nova, onyx, shimmer...
    /// Piper: a voice file (.onnx); empty = the English voice Haze downloads for you.
    pub voice: String,
    /// The Piper program. Empty = look for `piper-tts` or `piper` in the usual places.
    pub piper_path: String,
    pub model: String,
    /// Empty = reuse the DJ's base URL and key when the provider is OpenAI-compatible.
    pub base_url: String,
    pub api_key_env: String,
    /// Music volume (percent of normal) while the DJ talks. 0 pauses the music instead.
    pub duck_to: u8,
}

impl Default for TtsConfig {
    fn default() -> Self {
        Self {
            engine: TtsEngine::System,
            voice: String::new(),
            piper_path: String::new(),
            model: "gpt-4o-mini-tts".into(),
            base_url: String::new(),
            api_key_env: String::new(),
            duck_to: 25,
        }
    }
}

/// Key names: a single character, or space, enter, escape, tab, left, right, up, down.
/// Prefix with "ctrl+" or "shift+" for modifiers.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Keybindings {
    pub play_pause: String,
    pub next: String,
    pub previous: String,
    pub volume_up: String,
    pub volume_down: String,
    pub seek_forward: String,
    pub seek_back: String,
    pub search: String,
    pub toggle_side_panel: String,
    pub dj_now: String,
    pub lyrics: String,
}

impl Default for Keybindings {
    fn default() -> Self {
        Self {
            play_pause: "ctrl+space".into(),
            next: "ctrl+right".into(),
            previous: "ctrl+left".into(),
            volume_up: "ctrl+up".into(),
            volume_down: "ctrl+down".into(),
            seek_forward: "shift+right".into(),
            seek_back: "shift+left".into(),
            search: "ctrl+f".into(),
            toggle_side_panel: "ctrl+b".into(),
            dj_now: "ctrl+d".into(),
            lyrics: "ctrl+l".into(),
        }
    }
}

pub fn non_empty(s: &str) -> Option<&str> {
    let s = s.trim();
    (!s.is_empty()).then_some(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_provider_saves_and_loads() {
        for provider in LlmProvider::ALL {
            let mut config = Config::default();
            config.dj.provider = provider;
            let back: Config = toml::from_str(&toml::to_string_pretty(&config).unwrap()).unwrap();
            assert_eq!(back.dj.provider, provider);
        }
        let config: Config = toml::from_str("[dj]\nprovider = \"openrouter\"").unwrap();
        assert_eq!(config.dj.provider, LlmProvider::OpenRouter);
    }

    #[test]
    fn lyrics_provider_matches_its_sources() {
        for p in LyricsProvider::ALL {
            assert_eq!(LyricsProvider::of(&p.sources()), Some(p));
        }
        assert_eq!(LyricsProvider::of(&["Spotify".into(), " LRCLIB".into()]), Some(LyricsProvider::SpotifyFirst));
        assert_eq!(LyricsProvider::of(&["genius".into()]), None);
    }

    #[test]
    fn remembers_the_finished_setup() {
        let config = Config { setup_done: true, ..Config::default() };
        let back: Config = toml::from_str(&toml::to_string_pretty(&config).unwrap()).unwrap();
        assert!(back.setup_done);
        assert!(!toml::from_str::<Config>("[spotify]\nclient_id = \"x\"").unwrap().setup_done);
    }

    #[test]
    fn partial_config_uses_defaults() {
        let config: Config = toml::from_str(
            r##"
            [appearance]
            theme = "nord"
            [dj]
            provider = "gemini"
            mode = "voice"
            "##,
        )
        .unwrap();
        assert_eq!(config.appearance.theme, "nord");
        assert_eq!(config.dj.provider, LlmProvider::Gemini);
        assert_eq!(config.dj.mode, DjMode::Voice);
        assert_eq!(config.dj.model(), "gemini-3.5-flash");
        assert_eq!(config.layout.sidebar, Side::Left);
    }

    #[test]
    fn example_config_parses() {
        let config: Config = toml::from_str(include_str!("../config.example.toml")).unwrap();
        assert_eq!(config.dj.tts.duck_to, 25);
    }

    #[test]
    fn default_config_round_trips() {
        let text = toml::to_string_pretty(&Config::default()).unwrap();
        let back: Config = toml::from_str(&text).unwrap();
        assert_eq!(back.dj.batch_size, 5);
    }
}
