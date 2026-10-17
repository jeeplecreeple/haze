//! Settings > Plugins: install from GitHub, a link, a .zip or a folder, and switch plugins on or off.

use iced::widget::{Column, column, container, row, text, toggler};
use iced::{Alignment, Fill};

use super::super::icon::{self, icon};
use super::super::{App, Message, style};
use super::El;
use crate::plugins;

impl App {
    pub(super) fn plugins_page(&self) -> El<'_> {
        let install = vec![
            text("Plugins add themes, layouts and player looks, and can run little programs when a song changes \
                  (for example to show a notification or tell OBS what's playing).")
                .size(self.fs(0.9))
                .color(self.colors.muted_text)
                .into(),
            row![
                self.input("GitHub repo (owner/name), a link to a .zip, or a folder", &self.plugin_input)
                    .on_input(Message::PluginInput)
                    .on_submit(Message::InstallPlugin)
                    .width(Fill),
                self.primary_button(
                    Some(icon::DOWNLOAD),
                    if self.plugin_busy { "Installing…" } else { "Install" },
                    (!self.plugin_busy).then_some(Message::InstallPlugin)
                ),
            ]
            .spacing(10)
            .align_y(Alignment::Center)
            .into(),
            row![
                self.small_button(Some(icon::FOLDER_OPEN), "Install a .zip file…", Message::PickPlugin),
                self.small_button(Some(icon::FOLDER_OPEN), "Plugins folder", Message::OpenDataFolder("plugins")),
                self.small_button(Some(icon::ROTATE_CCW), "Reload", Message::ReloadPlugins),
            ]
            .spacing(8)
            .wrap()
            .into(),
            self.muted("You can also drag a plugin's .zip onto the Haze window.").size(self.fs(0.82)).into(),
        ];

        let mut list = Column::new().spacing(10);
        if self.plugins.is_empty() {
            list = list.push(self.muted("No plugins yet."));
        }
        for p in &self.plugins {
            let on = self.config.plugins.enabled.contains(&p.id);
            let m = &p.manifest;
            let mut title = row![text(p.name().to_string()).size(self.fs(1.05)).font(self.weight(iced::font::Weight::Bold))]
                .spacing(8)
                .align_y(Alignment::Center);
            if !m.version.is_empty() {
                title = title.push(self.muted(format!("v{}", m.version)).size(self.fs(0.82)));
            }
            if !m.author.is_empty() {
                title = title.push(self.muted(format!("by {}", m.author)).size(self.fs(0.82)));
            }
            let mut has = Vec::new();
            if p.themes > 0 {
                has.push(format!("{} theme{}", p.themes, if p.themes == 1 { "" } else { "s" }));
            }
            if p.layouts > 0 {
                has.push(format!("{} layout{}", p.layouts, if p.layouts == 1 { "" } else { "s" }));
            }
            if p.runs_commands() {
                let known = |e: &str| crate::plugins::EVENTS.iter().any(|(k, _)| *k == e);
                let events: Vec<String> =
                    m.hooks.keys().map(|e| if known(e) { e.clone() } else { format!("{e} (not an event Haze knows)") }).collect();
                has.push(format!("runs commands on: {}", events.join(", ")));
            }
            let mut info = column![title].spacing(4).width(Fill);
            if !m.description.is_empty() {
                info = info.push(text(m.description.clone()).size(self.fs(0.92)));
            }
            if !has.is_empty() {
                info = info.push(self.muted(has.join(" · ")).size(self.fs(0.82)));
            }
            if p.runs_commands() && !on {
                info = info.push(
                    row![
                        icon(icon::CIRCLE_STOP, self.fs(0.85)).color(self.colors.danger),
                        text("This plugin runs programs on your computer. Turn it on only if you trust where it came from.")
                            .size(self.fs(0.82))
                            .color(self.colors.danger),
                    ]
                    .spacing(6)
                    .align_y(Alignment::Center),
                );
            }
            let id = p.id.clone();
            let mut buttons = row![toggler(on).size(20).on_toggle(move |v| Message::TogglePlugin(id.clone(), v))]
                .spacing(6)
                .align_y(Alignment::Center);
            if !m.homepage.is_empty() {
                buttons = buttons.push(self.icon_button(icon::EXTERNAL_LINK, self.fs(0.95), Some(Message::OpenUrl(m.homepage.clone())), false));
            }
            buttons = buttons
                .push(self.icon_button(icon::FOLDER_OPEN, self.fs(0.95), Some(Message::OpenPath(p.dir.clone())), false))
                .push(self.icon_button(icon::TRASH_2, self.fs(0.95), Some(Message::UninstallPlugin(p.id.clone())), false));
            list = list.push(
                container(row![icon(icon::PUZZLE, self.fs(1.3)).color(self.colors.accent), info, buttons].spacing(14).align_y(Alignment::Center))
                    .padding(14)
                    .width(Fill)
                    .style(style::card(self.colors, self.r())),
            );
        }
        let events = plugins::EVENTS.iter().map(|(e, what)| format!("{e} ({what})")).collect::<Vec<_>>().join(", ");
        let make = vec![
            self.muted(format!(
                "A plugin is a folder with a plugin.toml (name, version, author, description) and any of: a themes \
                 folder, a layouts folder, and a [hooks] table of commands. Hooks can be: {events}. They get \
                 HAZE_TITLE, HAZE_ARTIST, HAZE_ALBUM, HAZE_TRACK_ID, HAZE_COVER and HAZE_DJ_TEXT. The guide has examples."
            ))
            .size(self.fs(0.85))
            .into(),
        ];
        column![
            self.section(icon::DOWNLOAD, "Install a plugin", install),
            self.section(icon::PUZZLE, "Installed", vec![list.into()]),
            self.section(icon::PAINTBRUSH, "Making a plugin", make),
        ]
        .spacing(16)
        .into()
    }
}
