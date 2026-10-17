//! Lucide icons from the bundled icon font, and the bundled Inter fonts.

use iced::Font;
use iced::widget::{Text, text};

pub const FONT: Font = Font::with_name("lucide");

/// Font files compiled into the binary, loaded at startup.
pub const BUNDLED_FONTS: [&[u8]; 6] = [
    include_bytes!("../../assets/fonts/Inter-Regular.ttf"),
    include_bytes!("../../assets/fonts/Inter-SemiBold.ttf"),
    include_bytes!("../../assets/fonts/Inter-Bold.ttf"),
    include_bytes!("../../assets/fonts/InterDisplay-Bold.ttf"),
    include_bytes!("../../assets/fonts/InterDisplay-ExtraBold.ttf"),
    include_bytes!("../../assets/fonts/lucide-subset.ttf"),
];

pub fn icon<'a>(glyph: char, size: f32) -> Text<'a> {
    text(glyph.to_string()).font(FONT).size(size).line_height(1.0)
}

pub const PLAY: char = '\u{e13c}';
pub const PAUSE: char = '\u{e12e}';
pub const SKIP_BACK: char = '\u{e15f}';
pub const SKIP_FORWARD: char = '\u{e160}';
pub const SHUFFLE: char = '\u{e15e}';
pub const REPEAT: char = '\u{e146}';
pub const REPEAT_1: char = '\u{e1fd}';
pub const VOLUME_2: char = '\u{e1ab}';
pub const VOLUME_1: char = '\u{e1aa}';
pub const VOLUME_X: char = '\u{e1ac}';
pub const LIST_MUSIC: char = '\u{e2e0}';
pub const MIC_VOCAL: char = '\u{e349}';
pub const SPARKLES: char = '\u{e412}';
pub const SEARCH: char = '\u{e151}';
pub const HEART: char = '\u{e0f2}';
pub const SETTINGS: char = '\u{e154}';
pub const X: char = '\u{e1b2}';
pub const LIST_PLUS: char = '\u{e23f}';
pub const MAXIMIZE_2: char = '\u{e113}';
pub const MINIMIZE_2: char = '\u{e11b}';
pub const MUSIC: char = '\u{e122}';
pub const FOLDER_OPEN: char = '\u{e247}';
pub const IMAGE: char = '\u{e0f6}';
pub const TYPE: char = '\u{e198}';
pub const ROTATE_CCW: char = '\u{e148}';
pub const CORNER_DOWN_RIGHT: char = '\u{e0a2}';
pub const LOG_OUT: char = '\u{e10e}';
pub const SAVE: char = '\u{e14d}';
pub const CIRCLE_STOP: char = '\u{e083}';
pub const AUDIO_LINES: char = '\u{e55a}';
pub const PALETTE: char = '\u{e1dd}';
pub const SLIDERS_HORIZONTAL: char = '\u{e29a}';
pub const LIBRARY: char = '\u{e100}';
