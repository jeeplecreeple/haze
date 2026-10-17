//! Settings > Shortcuts: click a shortcut, then press the keys you want for it.

use iced::widget::{Column, button, column, container, row, text};
use iced::{Alignment, Fill};

use super::super::icon;
use super::super::{App, Message, keys, style};
use super::El;
use crate::config::Keybindings;

impl App {
    pub(super) fn shortcuts_page(&self) -> El<'_> {
        let k = &self.config.keys;
        let mut list = Column::new().spacing(8);
        for (i, action) in Keybindings::ACTIONS.iter().enumerate() {
            let recording = self.recording_key == Some(i);
            let spec = k.get(i);
            let label = if recording { "Press the keys…".to_string() } else { keys::pretty(&spec) };
            let colors = self.colors;
            let r = self.r();
            let key = button(text(label).size(self.fs(0.92)).font(self.weight(iced::font::Weight::Semibold)))
                .padding([6, 14])
                .style(move |_, status| {
                    let bg = if recording {
                        colors.accent
                    } else if status == button::Status::Hovered {
                        style::tint(colors, 0.16)
                    } else {
                        style::tint(colors, 0.09)
                    };
                    button::Style {
                        background: Some(iced::Background::Color(bg)),
                        text_color: if recording { style::readable_on(colors.accent) } else { colors.text },
                        border: iced::Border { radius: r.into(), width: 1.0, color: style::alpha(colors.text, 0.12) },
                        ..button::Style::default()
                    }
                })
                .on_press(if recording { Message::StopRecordingKey } else { Message::RecordKey(i) });
            list = list.push(
                row![
                    text(*action).size(self.fs(0.95)).width(Fill),
                    key,
                    self.icon_button(icon::X, self.fs(0.9), (!spec.is_empty()).then_some(Message::ClearKey(i)), false),
                ]
                .spacing(10)
                .align_y(Alignment::Center),
            );
        }
        let rows = vec![
            text("Click a shortcut, then press the keys you want, like Ctrl and a letter. Esc cancels. \
                  Media keys on your keyboard always work too.")
                .size(self.fs(0.9))
                .color(self.colors.muted_text)
                .into(),
            container(list).width(Fill).into(),
            row![self.small_button(Some(icon::ROTATE_CCW), "Reset all shortcuts", Message::ResetKeys)].into(),
        ];
        column![self.section(icon::KEYBOARD, "Keyboard shortcuts", rows)].into()
    }
}
