//! Widget styles derived from the resolved theme colours.
//!
//! The window background is `colors.background`; the sidebar, main view and queue sit on it as
//! rounded `colors.surface` panels. Everything else is a tint of those two and the text colour.

use iced::gradient::{self, Gradient};
use iced::widget::{button, container, pick_list, scrollable, slider, text_input};
use iced::widget::overlay::menu;
use iced::{Background, Border, Color, Radians, Shadow, Theme, Vector};

use crate::theme::Colors;

pub fn mix(a: Color, b: Color, t: f32) -> Color {
    Color {
        r: a.r + (b.r - a.r) * t,
        g: a.g + (b.g - a.g) * t,
        b: a.b + (b.b - a.b) * t,
        a: a.a + (b.a - a.a) * t,
    }
}

pub fn alpha(c: Color, a: f32) -> Color {
    Color { a, ..c }
}

/// A tint of the surface toward the text colour: hover and selection backgrounds, chips, rails.
pub fn tint(colors: Colors, t: f32) -> Color {
    mix(colors.surface, colors.text, t)
}

pub fn panel(colors: Colors, radius: f32) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style {
        background: Some(Background::Color(colors.surface)),
        text_color: Some(colors.text),
        border: Border { radius: radius.into(), ..Border::default() },
        ..container::Style::default()
    }
}

/// A slightly raised block inside a panel, for settings groups and DJ messages.
pub fn card(colors: Colors, radius: f32) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style {
        background: Some(Background::Color(tint(colors, 0.04))),
        text_color: Some(colors.text),
        border: Border { radius: radius.into(), ..Border::default() },
        ..container::Style::default()
    }
}

/// A flat colour block, for playlist initials and avatars.
pub fn tile(color: Color, radius: f32) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style {
        background: Some(Background::Color(color)),
        text_color: Some(readable_on(color)),
        border: Border { radius: radius.into(), ..Border::default() },
        ..container::Style::default()
    }
}

/// The accent fading into the panel, behind page headers.
pub fn hero(colors: Colors, radius: f32) -> impl Fn(&Theme) -> container::Style {
    move |_| {
        let top = mix(colors.surface, colors.accent, 0.32);
        let gradient = gradient::Linear::new(Radians(std::f32::consts::PI))
            .add_stop(0.0, top)
            .add_stop(1.0, colors.surface);
        container::Style {
            background: Some(Background::Gradient(Gradient::Linear(gradient))),
            text_color: Some(colors.text),
            border: Border { radius: radius.into(), ..Border::default() },
            ..container::Style::default()
        }
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
            0.3,
        ))),
        text_color: Some(colors.text),
        border: Border { radius: radius.into(), ..Border::default() },
        ..container::Style::default()
    }
}

/// Sidebar entries: muted until hovered or selected.
pub fn nav(colors: Colors, radius: f32, selected: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
        let bg = if selected {
            Some(tint(colors, 0.1))
        } else if hovered {
            Some(tint(colors, 0.05))
        } else {
            None
        };
        button::Style {
            background: bg.map(Background::Color),
            text_color: if selected || hovered { colors.text } else { colors.muted_text },
            border: Border { radius: radius.into(), ..Border::default() },
            ..button::Style::default()
        }
    }
}

/// Track list and queue rows; the playing row is marked by accent text, not a fill.
pub fn row(colors: Colors, radius: f32, selected: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let bg = match status {
            button::Status::Hovered => Some(tint(colors, 0.06)),
            button::Status::Pressed => Some(tint(colors, 0.1)),
            _ if selected => Some(tint(colors, 0.035)),
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

/// Filled accent pill for the main action on a page.
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
            shadow: Shadow { color: alpha(Color::BLACK, 0.25), offset: Vector::new(0.0, 2.0), blur_radius: 8.0 },
            ..button::Style::default()
        }
    }
}

/// Quiet filled buttons for secondary actions.
pub fn secondary(colors: Colors, radius: f32) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let t = match status {
            button::Status::Hovered => 0.16,
            button::Status::Pressed => 0.22,
            _ => 0.09,
        };
        button::Style {
            background: Some(Background::Color(tint(colors, t))),
            text_color: colors.text,
            border: Border { radius: radius.into(), ..Border::default() },
            ..button::Style::default()
        }
    }
}

/// Bare icon buttons: muted, brighter on hover, accent when switched on.
pub fn icon_button(colors: Colors, active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
        button::Style {
            background: hovered.then(|| Background::Color(tint(colors, 0.08))),
            text_color: if active {
                colors.accent
            } else if hovered {
                colors.text
            } else {
                colors.muted_text
            },
            border: Border { radius: 999.0.into(), ..Border::default() },
            ..button::Style::default()
        }
    }
}

/// The round play/pause button: text colour on the background colour, like a light switch.
pub fn play_button(colors: Colors) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let bg = match status {
            button::Status::Hovered => mix(colors.text, colors.accent, 0.15),
            button::Status::Pressed => mix(colors.text, colors.background, 0.2),
            button::Status::Disabled => mix(colors.text, colors.surface, 0.6),
            button::Status::Active => colors.text,
        };
        button::Style {
            background: Some(Background::Color(bg)),
            text_color: colors.background,
            border: Border { radius: 999.0.into(), ..Border::default() },
            ..button::Style::default()
        }
    }
}

/// Clickable lyric lines: no background until hovered.
pub fn lyric_line(colors: Colors, radius: f32) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let bg = match status {
            button::Status::Hovered | button::Status::Pressed => Some(alpha(colors.text, 0.06)),
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

/// Thin seek and volume bars that light up in the accent colour on hover.
pub fn seek_slider(colors: Colors) -> impl Fn(&Theme, slider::Status) -> slider::Style {
    move |_, status| {
        let active = !matches!(status, slider::Status::Active);
        slider::Style {
            rail: slider::Rail {
                backgrounds: (
                    Background::Color(if active { colors.accent } else { colors.text }),
                    Background::Color(tint(colors, 0.2)),
                ),
                width: 4.0,
                border: Border { radius: 2.0.into(), ..Border::default() },
            },
            handle: slider::Handle {
                shape: slider::HandleShape::Circle { radius: if active { 6.0 } else { 0.0 } },
                background: Background::Color(colors.text),
                border_width: 0.0,
                border_color: Color::TRANSPARENT,
            },
        }
    }
}

/// Settings sliders always show their handle.
pub fn form_slider(colors: Colors) -> impl Fn(&Theme, slider::Status) -> slider::Style {
    move |theme, status| {
        let mut style = seek_slider(colors)(theme, slider::Status::Hovered);
        if matches!(status, slider::Status::Active) {
            style.handle.background = Background::Color(mix(colors.text, colors.accent, 0.2));
        }
        style
    }
}

pub fn scrollbar(colors: Colors) -> impl Fn(&Theme, scrollable::Status) -> scrollable::Style {
    move |theme, status| {
        let hovered = matches!(status, scrollable::Status::Hovered { .. } | scrollable::Status::Dragged { .. });
        let rail = scrollable::Rail {
            background: None,
            border: Border::default(),
            scroller: scrollable::Scroller {
                background: Background::Color(tint(colors, if hovered { 0.3 } else { 0.16 })),
                border: Border { radius: 4.0.into(), ..Border::default() },
            },
        };
        scrollable::Style {
            container: container::Style::default(),
            vertical_rail: rail,
            horizontal_rail: rail,
            gap: None,
            auto_scroll: scrollable::default(theme, status).auto_scroll,
        }
    }
}

pub fn input(colors: Colors, radius: f32) -> impl Fn(&Theme, text_input::Status) -> text_input::Style {
    move |_, status| {
        let (bg, border) = match status {
            text_input::Status::Focused { .. } => (tint(colors, 0.1), colors.accent),
            text_input::Status::Hovered => (tint(colors, 0.1), Color::TRANSPARENT),
            _ => (tint(colors, 0.07), Color::TRANSPARENT),
        };
        text_input::Style {
            background: Background::Color(bg),
            border: Border { radius: radius.into(), width: 1.0, color: border },
            icon: colors.muted_text,
            placeholder: alpha(colors.muted_text, 0.8),
            value: colors.text,
            selection: alpha(colors.accent, 0.35),
        }
    }
}

pub fn pick(colors: Colors, radius: f32) -> impl Fn(&Theme, pick_list::Status) -> pick_list::Style {
    move |_, status| {
        let t = if matches!(status, pick_list::Status::Active) { 0.07 } else { 0.12 };
        pick_list::Style {
            text_color: colors.text,
            placeholder_color: colors.muted_text,
            handle_color: colors.muted_text,
            background: Background::Color(tint(colors, t)),
            border: Border { radius: radius.into(), ..Border::default() },
        }
    }
}

pub fn menu(colors: Colors, radius: f32) -> impl Fn(&Theme) -> menu::Style {
    move |_| menu::Style {
        background: Background::Color(tint(colors, 0.08)),
        border: Border { radius: radius.into(), width: 1.0, color: tint(colors, 0.14) },
        text_color: colors.text,
        selected_text_color: readable_on(colors.accent),
        selected_background: Background::Color(colors.accent),
        shadow: Shadow { color: alpha(Color::BLACK, 0.35), offset: Vector::new(0.0, 4.0), blur_radius: 16.0 },
    }
}

/// Black or white, whichever reads better on `bg`.
pub fn readable_on(bg: Color) -> Color {
    let luminance = 0.2126 * bg.r + 0.7152 * bg.g + 0.0722 * bg.b;
    if luminance > 0.55 { Color::BLACK } else { Color::WHITE }
}

/// A stable, pleasant colour for a name, for playlist tiles without a cover.
pub fn name_color(name: &str, colors: Colors) -> Color {
    let hash = name.bytes().fold(5381u32, |h, b| h.wrapping_mul(33) ^ b as u32);
    let hue = (hash % 360) as f32;
    let (r, g, b) = hsl(hue, 0.45, if readable_on(colors.surface) == Color::WHITE { 0.42 } else { 0.62 });
    Color::from_rgb(r, g, b)
}

fn hsl(h: f32, s: f32, l: f32) -> (f32, f32, f32) {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;
    let (r, g, b) = match (h / 60.0) as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    (r + m, g + m, b + m)
}
