//! The saved-layouts part of Settings > Layout: switch between built-in and saved layouts, save
//! your own, and import or share them.

use iced::widget::{Row, column, row, text, toggler};
use iced::Alignment;

use super::super::icon;
use super::super::{App, Message};
use super::El;
use crate::layouts;

impl App {
    pub(super) fn layouts_section(&self) -> El<'_> {
        let mut built_in = Row::new().spacing(8);
        for name in layouts::BUILT_IN {
            built_in = built_in.push(self.small_button(None, name, Message::ApplyLayout(name.to_string())));
        }
        let mut rows: Vec<El> = vec![
            text("Switch the whole layout in one click. Saved layouts can also carry a player look.")
                .size(self.fs(0.9))
                .color(self.colors.muted_text)
                .into(),
            built_in.wrap().vertical_spacing(8).into(),
        ];
        if !self.layout_names.is_empty() {
            let mut saved = Row::new().spacing(8);
            for name in &self.layout_names {
                let mut chip = row![self.small_button(Some(icon::LAYOUT_TEMPLATE), name, Message::ApplyLayout(name.clone()))]
                    .spacing(2)
                    .align_y(Alignment::Center);
                if layouts::is_yours(name) {
                    chip = chip.push(self.icon_button(icon::TRASH_2, self.fs(0.85), Some(Message::DeleteLayout(name.clone())), false));
                }
                saved = saved.push(chip);
            }
            rows.push(self.label("SAVED").into());
            rows.push(saved.wrap().vertical_spacing(8).into());
        }
        rows.push(self.setting("Save as", self.input("My layout", &self.layout_name_input).on_input(Message::LayoutName)));
        rows.push(self.setting(
            "Include the player look",
            toggler(self.layout_with_player).size(20).on_toggle(Message::LayoutWithPlayer),
        ));
        rows.push(
            row![
                self.primary_button(Some(icon::SAVE), "Save layout", Some(Message::SaveLayout)),
                self.small_button(Some(icon::COPY), "Copy to share", Message::CopyLayout),
                self.small_button(Some(icon::CLIPBOARD_PASTE), "Paste a layout", Message::PasteLayout),
                self.small_button(Some(icon::DOWNLOAD), "Import file…", Message::ImportLayout),
                self.small_button(Some(icon::FOLDER_OPEN), "Layouts folder", Message::OpenDataFolder("layouts")),
            ]
            .spacing(8)
            .wrap()
            .vertical_spacing(8)
            .into(),
        );
        column![self.section(icon::LAYOUT_TEMPLATE, "Layouts", rows)].into()
    }
}
