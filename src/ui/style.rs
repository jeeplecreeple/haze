//! Widget styles derived from the resolved theme colours.

use iced::widget::{button, container};
use iced::{Background, Border, Color, Theme};

use crate::theme::Colors;

fn mix(a: Color, b: Color, t: f32) -> Color {
    Color {
        r: a.r + (b.r - a.r) * t,
        g: a.g + (b.g - a.g) * t,
        b: a.b + (b.b - a.b) * t,
        a: a.a + (b.a - a.a) * t,
    }
}

pub fn panel(colors: Colors, radius: f32) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style {
        background: Some(Background::Color(colors.surface)),
        text_color: Some(colors.text),
        border: Border { radius: radius.into(), ..Border::default() },
        ..container::Style::default()
    }
}

pub fn page(colors: Colors) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style {
        background: Some(Background::Color(colors.background)),
        text_color: Some(colors.text),
        ..container::Style::default()
    }
}

pub fn banner(colors: Colors, radius: f32, error: bool) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style {
        background: Some(Background::Color(mix(
            colors.surface,
            if error { colors.danger } else { colors.accent },
            0.25,
        ))),
        text_color: Some(colors.text),
        border: Border { radius: radius.into(), ..Border::default() },
        ..container::Style::default()
    }
}

/// Flat list rows and sidebar entries; `selected` highlights the active one.
pub fn row(colors: Colors, radius: f32, selected: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let bg = match (selected, status) {
            (true, _) => Some(mix(colors.surface, colors.accent, 0.22)),
            (_, button::Status::Hovered) => Some(mix(colors.surface, colors.text, 0.08)),
            (_, button::Status::Pressed) => Some(mix(colors.surface, colors.text, 0.14)),
            _ => None,
        };
        button::Style {
            background: bg.map(Background::Color),
            text_color: colors.text,
            border: Border { radius: radius.into(), ..Border::default() },
            ..button::Style::default()
        }
    }
}

/// Filled accent button for primary actions.
pub fn primary(colors: Colors, radius: f32) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let bg = match status {
            button::Status::Hovered => mix(colors.accent, Color::WHITE, 0.12),
            button::Status::Pressed => mix(colors.accent, Color::BLACK, 0.12),
            button::Status::Disabled => mix(colors.accent, colors.surface, 0.6),
            button::Status::Active => colors.accent,
        };
        button::Style {
            background: Some(Background::Color(bg)),
            text_color: readable_on(colors.accent),
            border: Border { radius: radius.into(), ..Border::default() },
            ..button::Style::default()
        }
    }
}

/// Icon-like transparent buttons in the player bar.
pub fn ghost(colors: Colors, radius: f32, active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let bg = match status {
            button::Status::Hovered => Some(mix(colors.surface, colors.text, 0.1)),
            button::Status::Pressed => Some(mix(colors.surface, colors.text, 0.18)),
            _ => None,
        };
        button::Style {
            background: bg.map(Background::Color),
            text_color: if active { colors.accent } else { colors.text },
            border: Border { radius: radius.into(), ..Border::default() },
            ..button::Style::default()
        }
    }
}

/// Black or white, whichever reads better on `bg`.
pub fn readable_on(bg: Color) -> Color {
    let luminance = 0.2126 * bg.r + 0.7152 * bg.g + 0.0722 * bg.b;
    if luminance > 0.55 { Color::BLACK } else { Color::WHITE }
}

/// Clickable lyric lines: no background until hovered.
pub fn lyric_line(colors: Colors, radius: f32) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let bg = match status {
            button::Status::Hovered | button::Status::Pressed => Some(Color { a: 0.07, ..colors.text }),
            _ => None,
        };
        button::Style {
            background: bg.map(Background::Color),
            text_color: colors.text,
            border: Border { radius: radius.into(), ..Border::default() },
            ..button::Style::default()
        }
    }
}

/// Quiet filled buttons for secondary actions in forms.
pub fn secondary(colors: Colors, radius: f32) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let t = match status {
            button::Status::Hovered => 0.16,
            button::Status::Pressed => 0.22,
            _ => 0.1,
        };
        button::Style {
            background: Some(Background::Color(mix(colors.surface, colors.text, t))),
            text_color: colors.text,
            border: Border { radius: radius.into(), ..Border::default() },
            ..button::Style::default()
        }
    }
}
