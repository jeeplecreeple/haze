//! Settings > Player: every part of the Now Playing page, plus ready-made looks.

use iced::widget::{Row, column, row, slider, text, toggler};
use iced::{Alignment, Fill};

use super::super::icon;
use super::super::{App, Edit, Message, style};
use super::El;
use crate::config::{
    PlayerBackground, PlayerLayout, PlayerStyle, Progress, Shape, Side, TextAlign, Visualizer, VisualizerColor, PLAYER_PRESETS, player_preset,
};

impl App {
    /// A message that swaps in the player settings with one field changed.
    fn with_player<T: 'static>(&self, set: fn(&mut PlayerStyle, T)) -> impl Fn(T) -> Message + 'static {
        let base = self.config.player.clone();
        move |v| {
            let mut p = base.clone();
            set(&mut p, v);
            Message::Edit(Edit::PlayerStyle(Box::new(p)))
        }
    }

    fn p_choice<'a, T>(&self, label: &'a str, options: &'static [T], value: T, set: fn(&mut PlayerStyle, T)) -> El<'a>
    where
        T: ToString + PartialEq + Clone + 'static,
    {
        self.setting(label, self.picker(options, Some(value), self.with_player(set)))
    }

    fn p_num<'a>(&self, label: &'a str, value: f32, range: std::ops::RangeInclusive<f32>, set: fn(&mut PlayerStyle, f32)) -> El<'a> {
        let shown = if value == 0.0 && label.starts_with("Cover size") { "auto".to_string() } else { format!("{value:.0}") };
        self.setting(
            label,
            row![
                slider(range, value, self.with_player(set)).step(1.0_f32).height(16).style(style::form_slider(self.colors)).width(Fill),
                text(shown).size(self.fs(0.9)).width(44),
            ]
            .spacing(10)
            .align_y(Alignment::Center),
        )
    }

    fn p_toggle<'a>(&self, label: &'a str, value: bool, set: fn(&mut PlayerStyle, bool)) -> El<'a> {
        self.setting(label, toggler(value).size(20).on_toggle(self.with_player(set)))
    }

    pub(super) fn player_settings(&self) -> El<'_> {
        let p = &self.config.player;
        let mut presets = Row::new().spacing(8).align_y(Alignment::Center);
        for name in PLAYER_PRESETS {
            presets = presets.push(self.small_button(
                None,
                name,
                Message::Edit(Edit::PlayerStyle(Box::new(player_preset(name)))),
            ));
        }
        let looks = vec![
            text("Start from a ready-made look, then change anything below. It all applies straight away.")
                .size(self.fs(0.9))
                .color(self.colors.muted_text)
                .into(),
            presets.wrap().into(),
            row![self.primary_button(Some(icon::DISC_3), "Open the player", Some(Message::TogglePlayer))].into(),
        ];
        let page = vec![
            self.p_choice("Layout", PlayerLayout::ALL, p.layout, |p, v| p.layout = v),
            self.p_choice("Lyrics side", &Side::ALL, p.lyrics_side, |p, v| p.lyrics_side = v),
            self.p_toggle("Current line on a card", p.lyrics_card, |p, v| p.lyrics_card = v),
            self.p_choice("Background", PlayerBackground::ALL, p.background, |p, v| p.background = v),
            self.p_toggle("Fill the whole window", p.full_window, |p, v| p.full_window = v),
            self.p_choice("Text alignment", TextAlign::ALL, p.align, |p, v| p.align = v),
            self.p_num("Title size", p.title_size, 14.0..=56.0, |p, v| p.title_size = v),
            self.p_toggle("Show the album", p.show_album, |p, v| p.show_album = v),
        ];
        let cover = vec![
            self.p_choice("Cover shape", Shape::ALL, p.cover_shape, |p, v| p.cover_shape = v),
            self.p_num("Cover size (0 = auto)", p.cover_size, 0.0..=600.0, |p, v| p.cover_size = v),
            self.p_num("Wavy edge bumps", p.wave_count as f32, 3.0..=24.0, |p, v| p.wave_count = v as u32),
            self.p_num("Wavy edge depth", p.wave_depth, 0.0..=100.0, |p, v| p.wave_depth = v),
            self.p_toggle("Spin like a record", p.spin, |p, v| p.spin = v),
            self.p_num("Spin speed (turns a minute)", p.spin_speed, 1.0..=33.0, |p, v| p.spin_speed = v),
        ];
        let mut viz = vec![
            self.p_choice("Visualizer", Visualizer::ALL, p.visualizer, |p, v| p.visualizer = v),
            self.p_choice("Colour", VisualizerColor::ALL, p.visualizer_color, |p, v| p.visualizer_color = v),
        ];
        if p.visualizer_color == VisualizerColor::Custom {
            viz.push(self.setting(
                "Custom colour",
                self.input("#ff5c8a", &p.visualizer_custom).on_input(self.with_player(|p, v: String| p.visualizer_custom = v)),
            ));
        }
        viz.extend([
            self.p_num("Bars", p.visualizer_bars as f32, 8.0..=256.0, |p, v| p.visualizer_bars = v as usize),
            self.p_num("Bar thickness", p.bar_width, 1.0..=14.0, |p, v| p.bar_width = v),
            self.p_num("Height (% of cover)", p.visualizer_height, 4.0..=60.0, |p, v| p.visualizer_height = v),
            self.p_num("Sensitivity %", p.visualizer_sensitivity, 10.0..=300.0, |p, v| p.visualizer_sensitivity = v),
            self.p_num("Smoothing", p.visualizer_smoothing, 0.0..=95.0, |p, v| p.visualizer_smoothing = v),
            self.p_toggle("Mirror left and right", p.visualizer_mirror, |p, v| p.visualizer_mirror = v),
            self.p_toggle("Glow", p.visualizer_glow, |p, v| p.visualizer_glow = v),
            self.p_num("Frames per second", p.fps as f32, 10.0..=120.0, |p, v| p.fps = v as u32),
        ]);
        let controls = vec![
            self.p_choice("Play button shape", Shape::ALL, p.play_shape, |p, v| p.play_shape = v),
            self.p_num("Play button size", p.play_size, 32.0..=120.0, |p, v| p.play_size = v),
            self.p_toggle("Play button turns while playing", p.play_spin, |p, v| p.play_spin = v),
            self.p_num("Other buttons size", p.control_size, 12.0..=40.0, |p, v| p.control_size = v),
            self.p_choice("Seek bar", Progress::ALL, p.progress, |p, v| p.progress = v),
            self.p_toggle("Show times", p.show_times, |p, v| p.show_times = v),
            self.p_toggle("Show volume", p.show_volume, |p, v| p.show_volume = v),
            self.p_toggle("Show shuffle and repeat", p.show_shuffle_repeat, |p, v| p.show_shuffle_repeat = v),
            self.p_toggle("Show queue, DJ and lyrics buttons", p.show_extras, |p, v| p.show_extras = v),
            row![self.small_button(Some(icon::ROTATE_CCW), "Reset the player", Message::Edit(Edit::PlayerStyle(Box::default())))].into(),
        ];
        column![
            self.section(icon::DISC_3, "Ready-made looks", looks),
            self.section(icon::LAYOUT_TEMPLATE, "Page", page),
            self.section(icon::IMAGE, "Cover", cover),
            self.section(icon::AUDIO_LINES, "Visualizer", viz),
            self.section(icon::SLIDERS_HORIZONTAL, "Controls", controls),
        ]
        .spacing(16)
        .into()
    }
}
