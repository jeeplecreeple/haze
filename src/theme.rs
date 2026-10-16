//! Builds the iced theme from a preset plus the user's colour overrides.

use iced::Color;
use iced::theme::{Palette, Theme};

use crate::config::Appearance;

/// The resolved colours every view draws with.
#[derive(Debug, Clone, Copy)]
pub struct Colors {
    pub background: Color,
    pub surface: Color,
    pub text: Color,
    pub muted_text: Color,
    pub accent: Color,
    pub danger: Color,
}

pub const PRESETS: [&str; 7] = ["haztify", "spotify", "nord", "gruvbox", "catppuccin", "dracula", "light"];

fn preset(name: &str) -> Colors {
    let c = |hex: &str| parse_hex(hex).unwrap_or(Color::BLACK);
    let (bg, surface, text, muted, accent, danger) = match name.to_ascii_lowercase().as_str() {
        "spotify" => ("#121212", "#181818", "#ffffff", "#b3b3b3", "#1db954", "#e22134"),
        "nord" => ("#2e3440", "#3b4252", "#eceff4", "#a3abbd", "#88c0d0", "#bf616a"),
        "gruvbox" => ("#282828", "#32302f", "#ebdbb2", "#a89984", "#fabd2f", "#fb4934"),
        "catppuccin" => ("#1e1e2e", "#27273a", "#cdd6f4", "#a6adc8", "#cba6f7", "#f38ba8"),
        "dracula" => ("#282a36", "#343746", "#f8f8f2", "#a4a8c2", "#ff79c6", "#ff5555"),
        "light" => ("#f6f6f8", "#ffffff", "#1c1c22", "#5f6070", "#7c3aed", "#d92d20"),
        _ => ("#0f0f14", "#1a1a23", "#f1f1f5", "#9a9aae", "#ff5c8a", "#ff4d4f"),
    };
    Colors {
        background: c(bg),
        surface: c(surface),
        text: c(text),
        muted_text: c(muted),
        accent: c(accent),
        danger: c(danger),
    }
}

pub fn resolve(appearance: &Appearance) -> Colors {
    let mut colors = preset(&appearance.theme);
    let o = &appearance.colors;
    let apply = |slot: &mut Color, hex: &str| {
        if let Some(c) = parse_hex(hex) {
            *slot = c;
        }
    };
    apply(&mut colors.background, &o.background);
    apply(&mut colors.surface, &o.surface);
    apply(&mut colors.text, &o.text);
    apply(&mut colors.muted_text, &o.muted_text);
    apply(&mut colors.accent, &o.accent);
    apply(&mut colors.danger, &o.danger);
    colors
}

pub fn iced_theme(colors: &Colors) -> Theme {
    Theme::custom(
        "Haztify",
        Palette {
            background: colors.background,
            text: colors.text,
            primary: colors.accent,
            success: colors.accent,
            warning: Color::from_rgb8(0xf5, 0xa5, 0x24),
            danger: colors.danger,
        },
    )
}

/// Parses "#rgb", "#rrggbb" or "#rrggbbaa" (the "#" is optional).
pub fn parse_hex(hex: &str) -> Option<Color> {
    let h = hex.trim().trim_start_matches('#');
    let byte = |i: usize| u8::from_str_radix(h.get(i..i + 2)?, 16).ok();
    match h.len() {
        3 => {
            let d = |i: usize| u8::from_str_radix(h.get(i..i + 1)?, 16).ok().map(|v| v * 17);
            Some(Color::from_rgb8(d(0)?, d(1)?, d(2)?))
        }
        6 => Some(Color::from_rgb8(byte(0)?, byte(2)?, byte(4)?)),
        8 => Some(Color::from_rgba8(byte(0)?, byte(2)?, byte(4)?, byte(6)? as f32 / 255.0)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_parsing() {
        assert_eq!(parse_hex("#ffffff"), Some(Color::WHITE));
        assert_eq!(parse_hex("000"), Some(Color::BLACK));
        assert_eq!(parse_hex("#12"), None);
        assert_eq!(parse_hex(""), None);
    }

    #[test]
    fn overrides_win_over_preset() {
        let mut a = Appearance::default();
        a.theme = "nord".into();
        a.colors.accent = "#ff0000".into();
        let c = resolve(&a);
        assert_eq!(c.accent, Color::from_rgb8(255, 0, 0));
        assert_eq!(c.background, parse_hex("#2e3440").unwrap());
    }
}
