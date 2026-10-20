//! `[player]`: how the Now Playing page looks. Every part of it can be changed in Settings > Player.

use serde::{Deserialize, Serialize};

use super::Side;

/// An enum of choices, saved in lowercase, with a label for Settings and a list of all of them.
macro_rules! choices {
    ($(#[$meta:meta])* $name:ident { $($variant:ident => $label:literal),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum $name { $($variant),+ }

        impl $name {
            pub const ALL: &'static [$name] = &[$($name::$variant),+];
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(match self { $($name::$variant => $label),+ })
            }
        }
    };
}

choices!(
    /// The outline of the cover art, and of the play button.
    Shape {
        Cookie => "Cookie (wavy edge)",
        Circle => "Circle",
        Flower => "Flower",
        Burst => "Burst (pointy edge)",
        Hexagon => "Hexagon",
        Squircle => "Squircle",
        Rounded => "Rounded square",
        Square => "Square",
    }
);

choices!(
    Visualizer {
        Ring => "Ring of bars",
        RingWave => "Ring wave",
        Dots => "Ring of dots",
        Spikes => "Spiky ring",
        Bars => "Bars underneath",
        Wave => "Wave underneath",
        Off => "Off",
    }
);

choices!(
    VisualizerColor {
        Accent => "Accent colour",
        Cover => "From the cover",
        Rainbow => "Rainbow",
        Gradient => "Gradient (two colours)",
        Text => "Text colour",
        Custom => "Custom colour",
    }
);

choices!(
    Progress {
        Wave => "Wave",
        Line => "Thin line",
        Thick => "Thick bar",
        Dots => "Dots",
        Ring => "Ring around the cover",
        Hidden => "Hidden",
    }
);

choices!(
    PlayerLayout {
        Split => "Player and lyrics side by side",
        Centered => "Player only, centred",
        Compact => "Small player above the lyrics",
    }
);

choices!(
    PlayerBackground {
        Blur => "Blurred cover",
        Glow => "Soft cover glow",
        Theme => "Theme background",
    }
);

choices!(
    TextAlign {
        Center => "Centre",
        Left => "Left",
    }
);

impl Visualizer {
    /// Drawn around the cover rather than in a strip under it.
    pub fn is_ring(self) -> bool {
        matches!(self, Self::Ring | Self::RingWave | Self::Dots | Self::Spikes)
    }

    pub fn is_under(self) -> bool {
        matches!(self, Self::Bars | Self::Wave)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PlayerStyle {
    pub layout: PlayerLayout,
    /// Where the lyrics go in the side-by-side layout.
    pub lyrics_side: Side,
    /// The current lyric line sits on a soft card.
    pub lyrics_card: bool,
    /// Hide the sidebar, queue and player bar while the Now Playing page is open.
    pub full_window: bool,
    pub background: PlayerBackground,

    pub cover_shape: Shape,
    /// Pixels; 0 sizes it to the window.
    pub cover_size: f32,
    /// Turn the cover slowly while music plays, like a record.
    pub spin: bool,
    /// Turns per minute.
    pub spin_speed: f32,
    /// How deep the wavy edge of the cookie and flower shapes is, 0-100.
    pub wave_depth: f32,
    /// How many bumps the cookie and flower shapes have.
    pub wave_count: u32,
    /// How much the cover grows on each beat, 0-100 (0 keeps it still).
    pub pulse: f32,
    /// An edge around the cover in the visualizer's colour, in pixels (0 = none).
    pub cover_border: f32,

    pub visualizer: Visualizer,
    pub visualizer_color: VisualizerColor,
    /// Used when `visualizer_color` is custom, and as the first colour of the gradient.
    pub visualizer_custom: String,
    /// Where the gradient ends (empty = the cover's colour).
    pub visualizer_custom_end: String,
    /// Number of bars (or points on the wave).
    pub visualizer_bars: usize,
    /// How far the bars reach, as a percent of the cover's size.
    pub visualizer_height: f32,
    /// 10-300 percent.
    pub visualizer_sensitivity: f32,
    /// 0 = jumpy, 95 = very smooth.
    pub visualizer_smoothing: f32,
    /// The ring mirrors its left half onto the right, so lows sit at the bottom on both sides.
    pub visualizer_mirror: bool,
    /// A soft glow behind the bars.
    pub visualizer_glow: bool,
    /// Bar thickness in pixels.
    pub bar_width: f32,
    /// Redraws per second while the visualizer moves (higher is smoother but uses more CPU).
    pub fps: u32,

    pub play_shape: Shape,
    pub play_size: f32,
    /// Turn the play button's wavy edge slowly while music plays.
    pub play_spin: bool,
    pub control_size: f32,
    pub progress: Progress,
    pub show_times: bool,
    pub show_volume: bool,
    pub show_shuffle_repeat: bool,
    pub show_album: bool,
    /// Buttons for the lyrics, queue and DJ.
    pub show_extras: bool,
    pub title_size: f32,
    pub align: TextAlign,
}

impl Default for PlayerStyle {
    fn default() -> Self {
        Self {
            layout: PlayerLayout::Split,
            lyrics_side: Side::Right,
            lyrics_card: true,
            full_window: true,
            background: PlayerBackground::Blur,
            cover_shape: Shape::Cookie,
            cover_size: 0.0,
            spin: false,
            spin_speed: 4.0,
            wave_depth: 45.0,
            wave_count: 12,
            pulse: 0.0,
            cover_border: 0.0,
            visualizer: Visualizer::Ring,
            visualizer_color: VisualizerColor::Accent,
            visualizer_custom: String::new(),
            visualizer_custom_end: String::new(),
            visualizer_bars: 72,
            visualizer_height: 22.0,
            visualizer_sensitivity: 100.0,
            visualizer_smoothing: 60.0,
            visualizer_mirror: true,
            visualizer_glow: true,
            bar_width: 4.0,
            fps: 30,
            play_shape: Shape::Cookie,
            play_size: 64.0,
            play_spin: true,
            control_size: 20.0,
            progress: Progress::Wave,
            show_times: true,
            show_volume: true,
            show_shuffle_repeat: true,
            show_album: true,
            show_extras: false,
            title_size: 26.0,
            align: TextAlign::Center,
        }
    }
}

/// Ready-made looks for the player, picked from Settings > Player.
pub const PLAYER_PRESETS: [&str; 8] = ["Halo", "Vinyl", "Minimal", "Studio", "Bloom", "Pulse", "Neon", "Sunburst"];

pub fn player_preset(name: &str) -> PlayerStyle {
    let base = PlayerStyle::default();
    match name {
        "Vinyl" => PlayerStyle {
            cover_shape: Shape::Circle,
            spin: true,
            visualizer: Visualizer::RingWave,
            visualizer_color: VisualizerColor::Cover,
            play_shape: Shape::Circle,
            play_spin: false,
            progress: Progress::Line,
            ..base
        },
        "Minimal" => PlayerStyle {
            cover_shape: Shape::Rounded,
            visualizer: Visualizer::Off,
            play_shape: Shape::Circle,
            play_spin: false,
            play_size: 52.0,
            progress: Progress::Line,
            lyrics_card: false,
            background: PlayerBackground::Glow,
            title_size: 22.0,
            ..base
        },
        "Studio" => PlayerStyle {
            cover_shape: Shape::Square,
            visualizer: Visualizer::Bars,
            visualizer_bars: 48,
            visualizer_color: VisualizerColor::Text,
            visualizer_glow: false,
            play_shape: Shape::Squircle,
            play_spin: false,
            progress: Progress::Thick,
            align: TextAlign::Left,
            show_extras: true,
            ..base
        },
        "Bloom" => PlayerStyle {
            cover_shape: Shape::Flower,
            wave_count: 8,
            wave_depth: 55.0,
            visualizer: Visualizer::Ring,
            visualizer_color: VisualizerColor::Rainbow,
            visualizer_bars: 96,
            play_shape: Shape::Flower,
            progress: Progress::Dots,
            ..base
        },
        "Pulse" => PlayerStyle {
            cover_shape: Shape::Circle,
            pulse: 60.0,
            visualizer: Visualizer::Dots,
            visualizer_color: VisualizerColor::Gradient,
            visualizer_bars: 60,
            bar_width: 5.0,
            play_shape: Shape::Circle,
            play_spin: false,
            progress: Progress::Ring,
            ..base
        },
        "Neon" => PlayerStyle {
            cover_shape: Shape::Hexagon,
            cover_border: 3.0,
            visualizer: Visualizer::Spikes,
            visualizer_color: VisualizerColor::Gradient,
            visualizer_custom: "#ff3df0".into(),
            visualizer_custom_end: "#3dd8ff".into(),
            visualizer_bars: 90,
            visualizer_height: 26.0,
            play_shape: Shape::Hexagon,
            play_spin: false,
            progress: Progress::Thick,
            background: PlayerBackground::Glow,
            ..base
        },
        "Sunburst" => PlayerStyle {
            cover_shape: Shape::Burst,
            wave_count: 16,
            wave_depth: 60.0,
            spin: true,
            spin_speed: 2.0,
            pulse: 35.0,
            visualizer: Visualizer::Spikes,
            visualizer_color: VisualizerColor::Rainbow,
            visualizer_bars: 120,
            play_shape: Shape::Burst,
            progress: Progress::Dots,
            ..base
        },
        _ => base,
    }
}
