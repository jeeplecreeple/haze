//! Builds the iced theme from a preset plus the user's colour overrides.

use iced::Color;
use iced::theme::{Palette, Theme};

use crate::config::{Appearance, ColorOverrides};

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

pub const PRESETS: [&str; 9] = [
    "haze",
    "spotify",
    "apple-music",
    "apple-music-dark",
    "nord",
    "gruvbox",
    "catppuccin",
    "dracula",
    "light",
];

/// A built-in theme's colours.
pub fn preset(name: &str) -> Colors {
    let c = |hex: &str| parse_hex(hex).unwrap_or(Color::BLACK);
    let (bg, surface, text, muted, accent, danger) = match name.to_ascii_lowercase().as_str() {
        "spotify" => ("#121212", "#181818", "#ffffff", "#b3b3b3", "#1db954", "#e22134"),
        "apple-music" => ("#ffffff", "#f5f5f7", "#1d1d1f", "#86868b", "#fa2d48", "#ff3b30"),
        "apple-music-dark" => ("#1c1c1e", "#2c2c2e", "#f5f5f7", "#98989d", "#fc3c44", "#ff453a"),
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

/// A theme file in the `themes` folder: `base` names the preset it starts from, and any colour
/// key (background, surface, text, muted_text, accent, danger) overrides it.
#[derive(Debug, Default, serde::Deserialize)]
#[serde(default)]
struct ThemeFile {
    base: String,
    #[serde(flatten)]
    colors: ColorOverrides,
}

fn theme_file(name: &str) -> Option<ThemeFile> {
    if PRESETS.contains(&name) || name.contains(['/', '\\', '.']) {
        return None;
    }
    let path = crate::config::data_dir("themes").ok()?.join(format!("{name}.toml"));
    let text = std::fs::read_to_string(path).ok()?;
    toml::from_str(&text).map_err(|e| log::warn!("theme {name}: {e}")).ok()
}

/// Theme names from the `themes` folder.
pub fn custom_themes() -> Vec<String> {
    let Ok(dir) = crate::config::data_dir("themes") else { return Vec::new() };
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            (p.extension()? == "toml").then(|| p.file_stem()?.to_str().map(str::to_string)).flatten()
        })
        .collect();
    names.sort();
    names
}

fn apply_overrides(colors: &mut Colors, o: &ColorOverrides) {
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
}

pub fn resolve(appearance: &Appearance) -> Colors {
    let mut colors = match theme_file(&appearance.theme) {
        Some(file) => {
            let mut c = preset(&file.base);
            apply_overrides(&mut c, &file.colors);
            c
        }
        None => preset(&appearance.theme),
    };
    apply_overrides(&mut colors, &appearance.colors);
    colors
}

/// A vivid colour from cover art, for `accent_from_cover`. Picks the most saturated, reasonably
/// bright pixels of a downscaled copy and averages them.
pub fn accent_from_image(bytes: &[u8]) -> Option<Color> {
    let img = image::load_from_memory(bytes).ok()?.thumbnail(24, 24).to_rgb8();
    let mut scored: Vec<(f32, [u8; 3])> = img
        .pixels()
        .map(|p| {
            let [r, g, b] = p.0.map(|v| v as f32 / 255.0);
            let max = r.max(g).max(b);
            let min = r.min(g).min(b);
            let sat = if max > 0.0 { (max - min) / max } else { 0.0 };
            (sat * (0.35 + max).min(1.0) * if max < 0.25 { 0.2 } else { 1.0 }, p.0)
        })
        .collect();
    scored.sort_by(|a, b| b.0.total_cmp(&a.0));
    let top = &scored[..scored.len().min(24)];
    if top.first()?.0 < 0.15 {
        return None; // greyscale cover: keep the theme accent
    }
    let n = top.len() as f32;
    let avg = |i: usize| top.iter().map(|(_, p)| p[i] as f32).sum::<f32>() / n / 255.0;
    // Lift dark picks so the accent stays readable on dark themes.
    let (r, g, b) = (avg(0), avg(1), avg(2));
    let lift = (0.6 - r.max(g).max(b)).max(0.0);
    Some(Color::from_rgb((r + lift).min(1.0), (g + lift).min(1.0), (b + lift).min(1.0)))
}

pub fn iced_theme(colors: &Colors) -> Theme {
    Theme::custom(
        "Haze",
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
