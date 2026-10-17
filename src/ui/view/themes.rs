//! Settings > Theme maker: pick a theme from swatches, or make your own and save or share it.

use iced::widget::{Column, Row, Space, button, column, container, row, slider, text};
use iced::{Alignment, Color, Fill, Length};

use super::super::icon;
use super::super::{App, Message, style};
use super::El;
use crate::theme::{self, Colors};

/// The colours a theme sets, in the order the maker shows them.
pub const THEME_SLOTS: [(&str, &str); 6] = [
    ("background", "Background"),
    ("surface", "Panels"),
    ("text", "Text"),
    ("muted_text", "Quiet text"),
    ("accent", "Accent"),
    ("danger", "Warnings"),
];

pub fn theme_slot_color(colors: &Colors, i: usize) -> Color {
    match i {
        0 => colors.background,
        1 => colors.surface,
        2 => colors.text,
        3 => colors.muted_text,
        4 => colors.accent,
        _ => colors.danger,
    }
}

impl App {
    /// A theme's look in miniature: its page with a panel, text and accent on it.
    fn swatch(&self, name: String, selected: bool) -> El<'_> {
        let c = theme::colors_of(&name);
        let chip = |color: Color, w: f32, h: f32| container(Space::new()).width(w).height(h).style(style::tile(color, 4.0));
        let preview = container(
            column![
                row![chip(c.accent, 22.0, 22.0), column![chip(c.text, 46.0, 6.0), chip(c.muted_text, 30.0, 5.0)].spacing(4)]
                    .spacing(8)
                    .align_y(Alignment::Center),
                container(Space::new()).width(Fill).height(10).style(style::tile(c.surface, 4.0)),
            ]
            .spacing(8),
        )
        .padding(10)
        .width(Fill)
        .style(style::tile(c.background, 8.0));
        let colors = self.colors;
        let mut label = Row::new()
            .spacing(6)
            .align_y(Alignment::Center)
            .push(text(name.clone()).size(self.fs(0.85)).width(Fill).wrapping(text::Wrapping::None));
        if theme::is_yours(&name) {
            label = label.push(
                button(super::super::icon::icon(icon::TRASH_2, self.fs(0.85)))
                    .padding(2)
                    .style(style::icon_button(self.colors, false))
                    .on_press(Message::DeleteTheme(name.clone())),
            );
        }
        button(column![preview, label].spacing(6))
            .width(150)
            .padding(6)
            .style(move |_, status| {
                let edge = if selected {
                    colors.accent
                } else if status == button::Status::Hovered {
                    style::tint(colors, 0.3)
                } else {
                    Color::TRANSPARENT
                };
                button::Style {
                    background: Some(iced::Background::Color(style::tint(colors, 0.04))),
                    text_color: colors.text,
                    border: iced::Border { color: edge, width: 2.0, radius: 12.0.into() },
                    ..button::Style::default()
                }
            })
            .on_press(Message::Edit(super::super::Edit::Theme(name)))
            .into()
    }

    pub(super) fn theme_maker(&self) -> El<'_> {
        let a = &self.config.appearance;
        let mut grid = Row::new().spacing(10);
        for name in &self.theme_names {
            grid = grid.push(self.swatch(name.clone(), *name == a.theme));
        }
        let picker = vec![
            text("Click a theme to use it. Your own themes and ones from plugins are here too.")
                .size(self.fs(0.9))
                .color(self.colors.muted_text)
                .into(),
            grid.wrap().vertical_spacing(10).into(),
        ];

        // The maker edits the colours live (as overrides on the current theme) until you save.
        let colors = self.colors;
        let mut slots = Column::new().spacing(8);
        for (i, (key, label)) in THEME_SLOTS.iter().enumerate() {
            let c = theme_slot_color(&colors, i);
            let selected = self.theme_slot == i;
            let accent = self.colors.accent;
            let swatch = button(Space::new().width(30).height(30))
                .padding(0)
                .style(move |_, _| button::Style {
                    background: Some(iced::Background::Color(c)),
                    border: iced::Border {
                        color: if selected { accent } else { style::alpha(Color::WHITE, 0.25) },
                        width: if selected { 3.0 } else { 1.0 },
                        radius: 999.0.into(),
                    },
                    ..button::Style::default()
                })
                .on_press(Message::ThemeSlot(i));
            let hex_now = theme::to_hex(c);
            let key = *key;
            slots = slots.push(
                row![
                    swatch,
                    text(*label).size(self.fs(0.95)).width(96),
                    self.input("#rrggbb", &self.hex_drafts.get(key).cloned().unwrap_or(hex_now))
                        .on_input(move |s| Message::ThemeColor(i, s))
                        .width(Fill),
                ]
                .spacing(12)
                .align_y(Alignment::Center),
            );
        }
        let (h, s, l) = theme::rgb_to_hsl(theme_slot_color(&colors, self.theme_slot));
        let current = self.theme_slot;
        let hsl_slider = |label: &'static str, value: f32, max: f32, make: Box<dyn Fn(f32) -> (f32, f32, f32)>| -> El<'_> {
            row![
                text(label).size(self.fs(0.9)).width(90),
                slider(0.0..=max, value, move |v| {
                    let (h, s, l) = make(v);
                    let (r, g, b) = style::hsl(h.rem_euclid(360.0), s.clamp(0.0, 1.0), l.clamp(0.0, 1.0));
                    Message::ThemeColor(current, theme::to_hex(Color::from_rgb(r, g, b)))
                })
                .step(0.5_f32)
                .height(16)
                .style(style::form_slider(self.colors))
                .width(Fill),
                text(format!("{value:.0}")).size(self.fs(0.85)).width(36),
            ]
            .spacing(10)
            .align_y(Alignment::Center)
            .into()
        };
        let tuner = column![
            text(format!("Tune {}", THEME_SLOTS[current].1.to_lowercase())).size(self.fs(0.95)).font(self.weight(iced::font::Weight::Semibold)),
            hsl_slider("Hue", h, 359.5, Box::new(move |v| (v, s, l))),
            hsl_slider("Saturation", s * 100.0, 100.0, Box::new(move |v| (h, v / 100.0, l))),
            hsl_slider("Lightness", l * 100.0, 100.0, Box::new(move |v| (h, s, v / 100.0))),
        ]
        .spacing(10);
        let preview = self.theme_preview();
        let maker = vec![
            text("Change any colour and the whole app follows. Save it to keep it, or copy it to share with someone.")
                .size(self.fs(0.9))
                .color(self.colors.muted_text)
                .into(),
            row![
                self.small_button(Some(icon::SPARKLES), "Surprise me", Message::ThemeSurprise),
                self.small_button(Some(icon::IMAGE), "From this song's cover", Message::ThemeFromCover),
                self.small_button(Some(icon::ROTATE_CCW), "Undo my changes", Message::ThemeReset),
            ]
            .spacing(8)
            .wrap()
            .into(),
            row![container(slots).width(Length::FillPortion(5)), container(preview).width(Length::FillPortion(4))]
                .spacing(20)
                .into(),
            tuner.into(),
            self.setting("Theme name", self.input("My theme", &self.theme_name_input).on_input(Message::ThemeName)),
            row![
                self.primary_button(Some(icon::SAVE), "Save theme", Some(Message::SaveTheme)),
                self.small_button(Some(icon::COPY), "Copy to share", Message::CopyTheme),
                self.small_button(Some(icon::CLIPBOARD_PASTE), "Paste a theme", Message::PasteTheme),
                self.small_button(Some(icon::DOWNLOAD), "Import file…", Message::ImportTheme),
                self.small_button(Some(icon::FOLDER_OPEN), "Themes folder", Message::OpenDataFolder("themes")),
            ]
            .spacing(8)
            .wrap()
            .vertical_spacing(8)
            .into(),
        ];
        column![self.section(icon::SWATCH_BOOK, "Themes", picker), self.section(icon::PAINTBRUSH, "Make a theme", maker)]
            .spacing(16)
            .into()
    }

    /// A little mock of the app in the colours being made.
    fn theme_preview(&self) -> El<'_> {
        let c = self.colors;
        let line = |color: Color, w: f32| container(Space::new()).width(w).height(8).style(style::tile(color, 4.0));
        let row_item = |hi: bool| {
            row![
                container(Space::new()).width(26).height(26).style(style::tile(style::tint(c, 0.15), 5.0)),
                column![line(if hi { c.accent } else { c.text }, 90.0), line(c.muted_text, 60.0)].spacing(5),
            ]
            .spacing(10)
            .align_y(Alignment::Center)
        };
        container(
            column![
                row![
                    text("Haze").size(self.fs(1.1)).font(self.weight(iced::font::Weight::Bold)).color(c.text).width(Fill),
                    container(text("Play").size(self.fs(0.8)).color(style::readable_on(c.accent)))
                        .padding([4, 12])
                        .style(style::tile(c.accent, 999.0)),
                ]
                .align_y(Alignment::Center),
                container(column![row_item(true), row_item(false), row_item(false)].spacing(10))
                    .padding(12)
                    .width(Fill)
                    .style(style::tile(c.surface, 10.0)),
                text("Can't reach Spotify").size(self.fs(0.8)).color(c.danger),
            ]
            .spacing(12),
        )
        .padding(14)
        .width(Fill)
        .style(style::tile(c.background, 14.0))
        .into()
    }
}
