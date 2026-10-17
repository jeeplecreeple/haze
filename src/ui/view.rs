//! All drawing. Layout follows `[layout]` in the config: sidebar side, player bar edge, side panel.

use iced::widget::{
    Column, Row, Space, button, column, container, image, pick_list, responsive, rich_text, row, scrollable, slider,
    span, stack, text, text_input, toggler,
};
use iced::{Alignment, ContentFit, Element, Fill, Font, Length, Padding, Shrink, font};

use super::icon::{self, icon};
use super::{App, DEFAULT_FONT, Edit, Login, Message, Page, SYSTEM_FONT, SettingsTab, UpdateStatus, custom, style};
use crate::config::{DjMode, Edge, LlmProvider, LyricsAlign, LyricsProvider, Side, TtsEngine, non_empty};
use crate::lyrics::{Line, Lyrics};
use crate::queue::{Origin, Repeat};
use crate::spotify::api::{Playlist, Track};

mod setup;

type El<'a> = Element<'a, Message>;

fn mmss(ms: u32) -> String {
    let s = ms / 1000;
    format!("{}:{:02}", s / 60, s % 60)
}

/// The gap between panels, and their inner padding.
const GAP: f32 = 8.0;

impl App {
    fn fs(&self, scale: f32) -> f32 {
        self.config.appearance.font_size * scale
    }

    fn r(&self) -> f32 {
        self.config.appearance.corner_radius
    }

    /// Panels are a little rounder than the controls inside them.
    fn panel_r(&self) -> f32 {
        self.r() * 1.5
    }

    /// The blurred cover behind the window: the strong lyrics one on the lyrics page, otherwise
    /// the ambient glow, when they're switched on and a song is playing.
    fn backdrop(&self) -> Option<&image::Handle> {
        let a = &self.config.appearance;
        if !a.show_album_art || self.setup.is_some() || self.login != Login::LoggedIn || self.queue.current().is_none() {
            return None;
        }
        if self.lyrics_backdrop() {
            return self.lyrics_background.as_ref();
        }
        self.ambient.as_ref().filter(|_| a.ambient_background)
    }

    fn lyrics_backdrop(&self) -> bool {
        self.page == Page::Lyrics && self.config.lyrics.blurred_background && self.lyrics_background.is_some()
    }

    /// The main panels: see-through over the cover glow, solid otherwise.
    fn glass(&self) -> impl Fn(&iced::Theme) -> container::Style + use<> {
        style::glass(self.colors, self.panel_r(), if self.backdrop().is_some() { 0.78 } else { 1.0 })
    }

    fn font(&self) -> Font {
        custom::font(&self.config.appearance.font_family)
    }

    fn weight(&self, weight: font::Weight) -> Font {
        Font { weight, ..self.font() }
    }

    fn muted<'a>(&self, s: impl text::IntoFragment<'a>) -> text::Text<'a> {
        text(s).size(self.fs(0.9)).color(self.colors.muted_text)
    }

    fn label<'a>(&self, s: impl text::IntoFragment<'a>) -> text::Text<'a> {
        text(s).size(self.fs(0.78)).font(self.weight(font::Weight::Semibold)).color(self.colors.muted_text)
    }

    fn heading<'a>(&self, s: impl text::IntoFragment<'a>, scale: f32) -> text::Text<'a> {
        text(s).size(self.fs(scale)).font(self.weight(font::Weight::Bold))
    }

    fn primary_button<'a>(&self, glyph: Option<char>, label: &'a str, msg: Option<Message>) -> El<'a> {
        let mut content = Row::new().spacing(8).align_y(Alignment::Center);
        if let Some(g) = glyph {
            content = content.push(icon(g, self.fs(1.0)));
        }
        content = content.push(
            text(label).size(self.fs(0.95)).font(self.weight(font::Weight::Semibold)).wrapping(text::Wrapping::None),
        );
        button(content).padding([9, 18]).style(style::primary(self.colors, 999.0)).on_press_maybe(msg).into()
    }

    fn small_button<'a>(&self, glyph: Option<char>, label: &'a str, msg: Message) -> El<'a> {
        let mut content = Row::new().spacing(6).align_y(Alignment::Center);
        if let Some(g) = glyph {
            content = content.push(icon(g, self.fs(0.95)));
        }
        content = content.push(
            text(label).size(self.fs(0.88)).font(self.weight(font::Weight::Semibold)).wrapping(text::Wrapping::None),
        );
        button(content).padding([6, 12]).style(style::secondary(self.colors, 999.0)).on_press(msg).into()
    }

    fn icon_button<'a>(&self, glyph: char, size: f32, msg: Option<Message>, active: bool) -> El<'a> {
        button(container(icon(glyph, size)).center(size + 14.0))
            .padding(0)
            .style(style::icon_button(self.colors, active))
            .on_press_maybe(msg)
            .into()
    }

    fn scroll<'a>(&self, content: impl Into<El<'a>>) -> scrollable::Scrollable<'a, Message> {
        scrollable(content)
            .direction(scrollable::Direction::Vertical(scrollable::Scrollbar::new().width(6).scroller_width(6).margin(2)))
            .style(style::scrollbar(self.colors))
    }

    fn input<'a>(&self, placeholder: &str, value: &str) -> text_input::TextInput<'a, Message> {
        text_input(placeholder, value)
            .padding([8, 12])
            .size(self.fs(0.95))
            .style(style::input(self.colors, self.r()))
    }

    fn picker<'a, T>(&self, options: impl std::borrow::Borrow<[T]> + 'a, selected: Option<T>, on: impl Fn(T) -> Message + 'a) -> El<'a>
    where
        T: ToString + PartialEq + Clone + 'a,
    {
        pick_list(options, selected, on)
            .padding([7, 12])
            .text_size(self.fs(0.95))
            .style(style::pick(self.colors, self.r()))
            .menu_style(style::menu(self.colors, self.r()))
            .into()
    }

    fn slider_widget<'a>(&self, range: std::ops::RangeInclusive<f32>, value: f32, on: impl Fn(f32) -> Message + 'a) -> slider::Slider<'a, f32, Message> {
        slider(range, value, on).height(16).style(style::seek_slider(self.colors))
    }

    pub fn view(&self) -> El<'_> {
        let content: El = match (&self.setup, &self.login) {
            (Some(setup), _) => self.setup_view(setup),
            (None, Login::LoggedIn) => self.main_layout(),
            (None, _) => self.login_view(),
        };
        let mut root = Column::new();
        if let Some(notice) = &self.notice {
            root = root.push(
                container(container(
                    row![
                        text(&notice.text).size(self.fs(0.95)).width(Fill),
                        self.icon_button(icon::X, self.fs(1.0), Some(Message::DismissNotice), false)
                    ]
                    .align_y(Alignment::Center),
                )
                .padding([4, 12])
                .width(Fill)
                .style(style::banner(self.colors, self.panel_r(), notice.error)),
            )
            .padding(Padding::new(GAP).bottom(0)),
            );
        }
        if self.setup.is_none()
            && let Some(banner) = self.update_banner()
        {
            root = root.push(banner);
        }
        let window = container(root.push(content)).width(Fill).height(Fill);
        match self.backdrop() {
            Some(glow) => stack![
                container(image(glow.clone()).width(Fill).height(Fill).content_fit(ContentFit::Fill).filter_method(image::FilterMethod::Linear))
                    .style(style::page(self.colors)),
                window.style(style::clear(self.colors)),
            ]
            .into(),
            None => window.style(style::page(self.colors)).into(),
        }
    }

    /// The strip across the top that offers a new release and shows how installing it goes.
    fn update_banner(&self) -> Option<El<'_>> {
        let u = self.update.as_ref()?;
        let v = &u.release.version;
        let mut line = Row::new().spacing(10).align_y(Alignment::Center).push(icon(icon::SPARKLES, self.fs(1.0)).color(self.colors.accent));
        let page = Message::OpenUrl(u.release.page.clone());
        match &u.status {
            UpdateStatus::Available if u.release.installable() => {
                line = line
                    .push(text(format!("Haze {v} is available.")).size(self.fs(0.95)).width(Fill))
                    .push(self.small_button(None, "What's new", page))
                    .push(self.small_button(None, "Later", Message::DismissUpdate))
                    .push(self.primary_button(None, "Update now", Some(Message::InstallUpdate)));
            }
            UpdateStatus::Available => {
                line = line
                    .push(
                        text(format!("Haze {v} is available. Update it the same way you installed it."))
                            .size(self.fs(0.95))
                            .width(Fill),
                    )
                    .push(self.small_button(None, "Later", Message::DismissUpdate))
                    .push(self.primary_button(Some(icon::EXTERNAL_LINK), "Download", Some(page)));
            }
            UpdateStatus::Installing => {
                line = line.push(text(format!("Downloading Haze {v}…")).size(self.fs(0.95)).width(Fill));
            }
            UpdateStatus::Installed(_) => {
                line = line
                    .push(text(format!("Haze {v} is installed. Restart to start using it.")).size(self.fs(0.95)).width(Fill))
                    .push(self.small_button(None, "Later", Message::DismissUpdate))
                    .push(self.primary_button(None, "Restart now", Some(Message::RestartForUpdate)));
            }
            UpdateStatus::Failed(e) => {
                line = line
                    .push(text(format!("Couldn't update: {e}")).size(self.fs(0.95)).width(Fill))
                    .push(self.small_button(None, "Close", Message::DismissUpdate))
                    .push(self.primary_button(Some(icon::EXTERNAL_LINK), "Download it yourself", Some(page)));
            }
        }
        Some(
            container(
                container(line)
                    .padding([8, 14])
                    .width(Fill)
                    .style(style::banner(self.colors, self.panel_r(), matches!(u.status, UpdateStatus::Failed(_)))),
            )
            .padding(Padding::new(GAP).bottom(0))
            .into(),
        )
    }

    fn login_view(&self) -> El<'_> {
        let brand = column![
            image(self.icon.clone()).width(72).height(72),
            self.heading("Haze", 2.0),
        ]
        .spacing(12)
        .align_x(Alignment::Center);
        let body: El = match self.login {
            Login::NeedsClientId => column![
                text("Haze talks to Spotify through your own free developer app, so your login stays yours.")
                    .size(self.fs(1.0)),
                self.muted("1. Open developer.spotify.com/dashboard and create an app."),
                self.muted(format!("2. Add this Redirect URI exactly: {}", self.config.spotify.redirect_uri)),
                self.muted("3. Tick \"Web API\" and \"Web Playback SDK\", save, and copy the Client ID."),
                self.input("Paste your Client ID", &self.client_id_input)
                    .on_input(Message::ClientIdChanged)
                    .on_submit(Message::SaveClientId),
                self.primary_button(None, "Save and log in", Some(Message::SaveClientId)),
            ]
            .spacing(12)
            .into(),
            Login::InProgress => column![
                text("Logging in…").size(self.fs(1.2)),
                self.muted("Finish logging in in your browser, then come back here."),
            ]
            .spacing(8)
            .align_x(Alignment::Center)
            .into(),
            _ => column![
                self.muted("Log in with your Spotify Premium account to start listening."),
                self.primary_button(Some(icon::MUSIC), "Log in with Spotify", Some(Message::Login)),
            ]
            .spacing(16)
            .align_x(Alignment::Center)
            .into(),
        };
        let card = container(column![brand, body].spacing(24).align_x(Alignment::Center).max_width(520))
            .padding(36)
            .style(style::panel(self.colors, self.panel_r()));
        container(card).center(Fill).padding(24).into()
    }

    fn main_layout(&self) -> El<'_> {
        let layout = &self.config.layout;
        let on_lyrics = self.page == Page::Lyrics;
        let main = container(self.page_view())
            .width(Fill)
            .height(Fill)
            .padding(if on_lyrics { 0.0 } else { 24.0 })
            .clip(true)
            .style(if on_lyrics && self.lyrics_backdrop() {
                // The lyrics float straight on the blurred cover that fills the window.
                style::glass(self.colors, self.panel_r(), 0.0)
            } else {
                style::glass(self.colors, self.panel_r(), if self.backdrop().is_some() { 0.78 } else { 1.0 })
            });
        let bar = container(self.player_bar()).padding(GAP);
        if on_lyrics && self.fullscreen {
            return column![container(main).padding(Padding::new(GAP).bottom(0)).height(Fill), bar].into();
        }
        let mut left: Vec<El> = Vec::new();
        let mut right: Vec<El> = Vec::new();
        match layout.sidebar {
            Side::Left => left.push(self.sidebar()),
            Side::Right => right.push(self.sidebar()),
        }
        if self.show_side_panel {
            match layout.side_panel {
                Side::Left => left.push(self.side_panel()),
                Side::Right => right.insert(0, self.side_panel()),
            }
        }
        let body = row(left).push(main).extend(right).spacing(GAP).height(Fill);
        match layout.player_bar {
            Edge::Top => column![bar, container(body).padding(Padding::new(GAP).top(0))].into(),
            Edge::Bottom => column![container(body).padding(Padding::new(GAP).bottom(0)), bar].into(),
        }
    }

    fn nav<'a>(&self, glyph: char, label: &'a str, page: Page) -> El<'a> {
        let selected = self.page == page;
        let glyph = icon(glyph, self.fs(1.15));
        let glyph = if selected { glyph.color(self.colors.accent) } else { glyph };
        button(
            row![glyph, text(label).size(self.fs(1.0)).font(self.weight(font::Weight::Semibold))]
                .spacing(12)
                .align_y(Alignment::Center),
        )
        .width(Fill)
        .padding([8, 12])
        .style(style::nav(self.colors, self.r(), selected))
        .on_press(Message::Open(page))
        .into()
    }

    /// A playlist's picture for small places: your cover, Spotify's if it's loaded, else its initial.
    fn playlist_thumb(&self, key: &str, name: &str, url: Option<&str>, size: f32) -> El<'_> {
        if let Some(handle) = self.collection_handle(key, url) {
            return image(handle).width(size).height(size).content_fit(ContentFit::Cover).border_radius(self.r() * 0.6).into();
        }
        let initial: String = name.chars().find(|c| c.is_alphanumeric()).map(|c| c.to_uppercase().collect()).unwrap_or_default();
        let color = style::name_color(name, self.colors);
        container(text(initial).size(size * 0.45).font(self.weight(font::Weight::Bold)))
            .center(size)
            .style(style::tile(color, self.r() * 0.6))
            .into()
    }

    fn playlist_entry<'a>(&'a self, p: &'a Playlist) -> El<'a> {
        let page = Page::Playlist(p.clone());
        let selected = self.page == page;
        let subtitle = if p.owner.is_empty() { "Playlist".to_string() } else { format!("Playlist · {}", p.owner) };
        button(
            row![
                self.playlist_thumb(&p.id, &p.name, p.image_url.as_deref(), 38.0),
                column![
                    text(&p.name).size(self.fs(0.95)).wrapping(text::Wrapping::None),
                    self.muted(subtitle).size(self.fs(0.8)).wrapping(text::Wrapping::None),
                ]
                .spacing(1)
                .clip(true)
            ]
            .spacing(10)
            .align_y(Alignment::Center),
        )
        .width(Fill)
        .padding([5, 8])
        .style(style::nav(self.colors, self.r(), selected))
        .on_press(Message::Open(page))
        .into()
    }

    fn sidebar(&self) -> El<'_> {
        let brand = row![
            image(self.icon.clone()).width(30).height(30),
            self.heading("Haze", 1.35).color(self.colors.text),
        ]
        .spacing(10)
        .align_y(Alignment::Center);

        let nav = column![
            self.nav(icon::SPARKLES, "DJ", Page::Dj),
            self.nav(icon::SEARCH, "Search", Page::Search),
            self.nav(icon::HEART, "Liked Songs", Page::Liked),
            self.nav(icon::MIC_VOCAL, "Lyrics", Page::Lyrics),
        ]
        .spacing(2);

        let mut playlists = Column::new().spacing(2);
        for p in &self.playlists {
            playlists = playlists.push(self.playlist_entry(p));
        }

        let name = if self.user_name.is_empty() { "…".to_string() } else { self.user_name.clone() };
        let initial: String = name.chars().next().map(|c| c.to_uppercase().collect()).unwrap_or_default();
        let footer = row![
            container(text(initial).size(self.fs(0.9)).font(self.weight(font::Weight::Bold)))
                .center(30)
                .style(style::tile(self.colors.accent, 999.0)),
            text(name).size(self.fs(0.95)).width(Fill).wrapping(text::Wrapping::None),
            self.icon_button(icon::SETTINGS, self.fs(1.15), Some(Message::Open(Page::Settings)), self.page == Page::Settings),
        ]
        .spacing(10)
        .align_y(Alignment::Center);

        container(
            column![
                container(brand).padding([4, 8]),
                nav,
                container(row![icon(icon::LIBRARY, self.fs(1.0)).color(self.colors.muted_text), self.label("YOUR PLAYLISTS")].spacing(8))
                    .padding(Padding::from([0, 12]).top(10)),
                self.scroll(playlists).height(Fill),
                footer,
            ]
            .spacing(8),
        )
        .width(self.config.layout.sidebar_width)
        .height(Fill)
        .padding(10)
        .style(self.glass())
        .into()
    }

    fn page_view(&self) -> El<'_> {
        match &self.page {
            Page::Dj => self.dj_page(),
            Page::Search => {
                let input = self
                    .input("What do you want to listen to?", &self.search)
                    .on_input(Message::SearchChanged)
                    .on_submit(Message::SearchSubmit)
                    .icon(text_input::Icon {
                        font: icon::FONT,
                        code_point: icon::SEARCH,
                        size: Some(self.fs(1.1).into()),
                        spacing: 10.0,
                        side: text_input::Side::Left,
                    })
                    .padding([12, 16])
                    .size(self.fs(1.05))
                    .style(style::input(self.colors, 999.0));
                column![self.heading("Search", 2.0), input, self.track_list()].spacing(18).into()
            }
            Page::Liked => column![
                self.collection_header("liked", "Liked Songs".into(), format!("{} songs", self.tracks.len()), None),
                self.track_list()
            ]
            .spacing(16)
            .into(),
            Page::Playlist(p) => column![
                self.collection_header(
                    &p.id,
                    p.name.clone(),
                    format!("{} · {} songs", p.owner, p.track_count),
                    p.image_url.as_deref()
                ),
                self.track_list()
            ]
            .spacing(16)
            .into(),
            Page::Lyrics => self.lyrics_page(),
            Page::Settings => self.settings_page(),
        }
    }

    fn column_width(name: &str) -> Length {
        match name {
            "title" => Length::FillPortion(5),
            "artist" | "album" => Length::FillPortion(3),
            _ => Length::Fixed(56.0),
        }
    }

    /// The columns to draw. The artist goes under the title when both are shown, like most players.
    fn shown_columns(&self) -> (Vec<&str>, bool) {
        let cols = &self.config.layout.track_columns;
        let stacked = cols.iter().any(|c| c == "title") && cols.iter().any(|c| c == "artist");
        let shown = cols.iter().map(String::as_str).filter(|c| !(stacked && *c == "artist")).collect();
        (shown, stacked)
    }

    fn cell<'a>(&self, name: &str, track: &'a Track, playing: bool, stacked: bool) -> El<'a> {
        let width = Self::column_width(name);
        let content: El = match name {
            "title" => {
                let mut title = row![
                    text(&track.name)
                        .size(self.fs(1.0))
                        .color(if playing { self.colors.accent } else { self.colors.text })
                        .wrapping(text::Wrapping::None)
                ]
                .spacing(6)
                .align_y(Alignment::Center);
                if track.explicit {
                    title = title.push(
                        container(text("E").size(self.fs(0.62)).font(self.weight(font::Weight::Bold)))
                            .padding([1, 4])
                            .style(style::tile(self.colors.muted_text, 2.0)),
                    );
                }
                if stacked {
                    column![title, self.muted(track.artist_line()).size(self.fs(0.85)).wrapping(text::Wrapping::None)]
                        .spacing(2)
                        .into()
                } else {
                    title.into()
                }
            }
            "artist" => self.muted(track.artist_line()).wrapping(text::Wrapping::None).into(),
            "album" => self.muted(track.album.as_str()).wrapping(text::Wrapping::None).into(),
            "duration" => self.muted(mmss(track.duration_ms)).into(),
            _ => Space::new().into(),
        };
        container(content).width(width).clip(true).into()
    }

    fn track_list(&self) -> El<'_> {
        if self.loading {
            return self.muted("Loading…").into();
        }
        if self.tracks.is_empty() {
            return self.muted("Nothing here yet.").into();
        }
        let (columns, stacked) = self.shown_columns();
        let pad = if self.config.appearance.compact { 2 } else { 5 };
        let playing_uri = self.queue.current().map(|e| e.track.uri.as_str());

        let mut header = row![container(self.label("#")).width(28)].spacing(14).padding([0, 12]);
        for c in &columns {
            header = header.push(container(self.label(c.to_uppercase())).width(Self::column_width(c)));
        }
        header = header.push(Space::new().width(72));

        let mut list = Column::new().spacing(0);
        for (i, track) in self.tracks.iter().enumerate() {
            let playing = playing_uri == Some(track.uri.as_str());
            let index: El = if playing {
                icon(if self.playing { icon::AUDIO_LINES } else { icon::MUSIC }, self.fs(1.0)).color(self.colors.accent).into()
            } else {
                self.muted((i + 1).to_string()).into()
            };
            let mut cells = row![container(index).width(28)].spacing(14).align_y(Alignment::Center);
            for c in &columns {
                cells = cells.push(self.cell(c, track, playing, stacked));
            }
            let actions = row![
                self.icon_button(icon::CORNER_DOWN_RIGHT, self.fs(1.0), Some(Message::PlayNextTrack(i)), false),
                self.icon_button(icon::LIST_PLUS, self.fs(1.0), Some(Message::QueueTrack(i)), false),
            ]
            .spacing(2)
            .align_y(Alignment::Center);
            let line = button(row![cells, actions].spacing(14).align_y(Alignment::Center))
                .width(Fill)
                .padding([pad, 12])
                .style(style::row(self.colors, self.r(), playing))
                .on_press(Message::PlayFrom(i));
            list = list.push(line);
        }
        column![
            header,
            container(Space::new().height(1)).width(Fill).style(style::tile(style::tint(self.colors, 0.1), 0.0)),
            self.scroll(list).height(Fill)
        ]
        .spacing(6)
        .into()
    }

    fn cover(&self, track: Option<&Track>, size: f32) -> El<'_> {
        let handle = track.and_then(|t| t.image_url.as_deref()).and_then(|u| self.image(u));
        match (self.config.appearance.show_album_art, handle) {
            (true, Some(h)) => {
                let art = image(h.clone()).width(size).height(size).content_fit(ContentFit::Cover).border_radius(self.r() * 0.75);
                if size >= 100.0 { container(art).style(style::art_edge(self.r() * 0.75)).into() } else { art.into() }
            }
            (true, None) => container(icon(icon::MUSIC, size * 0.4).color(self.colors.muted_text))
                .center(size)
                .style(style::tile(style::tint(self.colors, 0.08), self.r() * 0.75))
                .into(),
            (false, _) => Space::new().width(0).into(),
        }
    }

    fn collection_handle(&self, key: &str, url: Option<&str>) -> Option<image::Handle> {
        match self.config.covers.get(key) {
            Some(path) if std::path::Path::new(path).exists() => Some(image::Handle::from_path(path)),
            _ => url.and_then(|u| self.image(u)).cloned(),
        }
    }

    /// A playlist's (or Liked Songs') cover: yours if you set one, otherwise Spotify's.
    fn collection_cover(&self, key: &str, url: Option<&str>, size: f32) -> El<'_> {
        match self.collection_handle(key, url) {
            Some(h) => container(image(h).width(size).height(size).content_fit(ContentFit::Cover).border_radius(self.r()))
                .style(style::art_edge(self.r()))
                .into(),
            None => {
                let glyph = if key == "liked" { icon::HEART } else { icon::MUSIC };
                container(icon(glyph, size * 0.36).color(style::readable_on(self.colors.accent)))
                    .center(size)
                    .style(style::gradient_tile(self.colors, self.r()))
                    .into()
            }
        }
    }

    fn collection_header<'a>(&'a self, key: &str, title: String, subtitle: String, url: Option<&str>) -> El<'a> {
        let custom = self.config.covers.contains_key(key);
        let mut actions = row![
            self.primary_button(Some(icon::PLAY), "Play", (!self.tracks.is_empty() && self.has_audio()).then_some(Message::PlayFrom(0))),
            self.small_button(Some(icon::IMAGE), "Change cover…", Message::PickCover(key.to_string())),
        ]
        .spacing(8)
        .align_y(Alignment::Center);
        if custom {
            actions = actions.push(self.small_button(Some(icon::ROTATE_CCW), "Reset cover", Message::ResetCover(key.to_string())));
        }
        container(
            row![
                self.collection_cover(key, url, 168.0),
                column![
                    self.label(if key == "liked" { "COLLECTION" } else { "PLAYLIST" }),
                    self.heading(title, 2.6).wrapping(text::Wrapping::None),
                    self.muted(subtitle),
                    Space::new().height(6),
                    actions
                ]
                .spacing(4)
                .clip(true)
            ]
            .spacing(24)
            .align_y(Alignment::End),
        )
        .padding(20)
        .width(Fill)
        .style(style::hero(self.colors, self.r()))
        .into()
    }

    /// The lyrics font: Inter Display ExtraBold unless you picked another one.
    fn lyrics_font(&self) -> Font {
        match non_empty(&self.config.lyrics.font_family) {
            Some(name) => Font { weight: font::Weight::Bold, ..custom::font(name) },
            None => Font { weight: font::Weight::ExtraBold, ..Font::with_name("Inter Display") },
        }
    }

    fn lyrics_page(&self) -> El<'_> {
        let ly = &self.config.lyrics;
        let current = self.queue.current().map(|e| &e.track);
        let lyrics = self.lyrics.as_ref().filter(|l| current.is_some_and(|t| t.id == l.track_id));
        let align = match ly.align {
            LyricsAlign::Left => Alignment::Start,
            LyricsAlign::Center => Alignment::Center,
        };
        let source = match lyrics {
            Some(l) if l.synced => format!("Lyrics from {}", l.source),
            Some(l) => format!("Lyrics from {} · not time-synced", l.source),
            None => String::new(),
        };
        let content = responsive(move |size| {
            let wide = size.width >= 880.0;
            let art = (size.width * 0.27).clamp(220.0, 380.0);
            let lyrics_width = if wide { size.width - 88.0 - art - 48.0 } else { size.width - 48.0 };
            let body: El = match (current, lyrics) {
                (None, _) => self.lyrics_message("Play a song to see its lyrics here."),
                (Some(_), None) if self.lyrics_loading => self.lyrics_message("Looking for lyrics…"),
                (Some(_), None) => self.lyrics_message("No lyrics for this song."),
                (Some(_), Some(l)) if l.synced => self.synced_lyrics(l, align, lyrics_width),
                (Some(_), Some(l)) => self.plain_lyrics(l, align),
            };
            let name = current.map(|t| t.name.clone()).unwrap_or_else(|| "Lyrics".into());
            let artist = current.map(|t| t.artist_line()).unwrap_or_default();
            if wide {
                // Cover and song on the left, lyrics on the right, like a full-screen lyrics view.
                let info = column![
                    self.cover(current, art),
                    Space::new().height(8),
                    self.heading(name, 1.7).width(art),
                    text(artist).size(self.fs(1.1)).color(self.colors.muted_text).width(art),
                    self.muted(source.clone()).size(self.fs(0.78)),
                    Space::new().height(8),
                    self.small_button(
                        Some(if self.fullscreen { icon::MINIMIZE_2 } else { icon::MAXIMIZE_2 }),
                        if self.fullscreen { "Exit full screen" } else { "Full screen" },
                        Message::ToggleFullscreen,
                    ),
                ]
                .spacing(4)
                .width(art);
                row![container(info).center_y(Fill), container(body).width(Fill).height(Fill)]
                    .spacing(48)
                    .padding([28, 44])
                    .into()
            } else {
                let header = row![
                    self.cover(current, 64.0),
                    column![
                        self.heading(name, 1.25).wrapping(text::Wrapping::None),
                        self.muted(artist).wrapping(text::Wrapping::None),
                    ]
                    .spacing(2)
                    .width(Fill)
                    .clip(true),
                ]
                .spacing(14)
                .align_y(Alignment::Center);
                column![header, container(body).width(Fill).height(Fill)].spacing(20).padding([20, 24]).into()
            }
        });

        content.into()
    }

    fn lyrics_message(&self, message: &'static str) -> El<'_> {
        container(text(message).size(self.config.lyrics.font_size * 0.7).font(self.lyrics_font()).color(style::alpha(self.colors.text, 0.6)))
            .center(Fill)
            .into()
    }

    fn plain_lyrics(&self, l: &Lyrics, align: Alignment) -> El<'_> {
        let size = self.config.lyrics.font_size * 0.8;
        let mut lines = Column::new().spacing(size * 0.45).align_x(align).width(Fill);
        for line in &l.lines {
            lines = lines.push(
                text(line.text())
                    .size(size)
                    .font(self.lyrics_font())
                    .color(style::alpha(self.colors.text, 0.85))
                    .align_x(align)
                    .width(Fill),
            );
        }
        self.scroll(container(lines).padding(Padding::from([24, 12]))).height(Fill).into()
    }

    /// Karaoke-style lyrics: the line being sung is bright and fills in word by word, with a soft
    /// shine on the edge of the fill. When a new line starts the lines glide up rather than jump,
    /// lines further away fade out, and long instrumental breaks show three breathing dots that
    /// fill up until the singing starts again.
    fn synced_lyrics(&self, l: &Lyrics, align: Alignment, width: f32) -> El<'_> {
        let ly = &self.config.lyrics;
        let heard = self.live_position().saturating_sub(crate::player::OUTPUT_DELAY_MS);
        let pos = (heard as i64 - ly.offset_ms as i64).max(0) as u32;
        let active = l.active_line(pos);
        let in_gap = active.is_none_or(|i| pos >= l.lines[i].end_ms);
        let first = active.map(|i| i.saturating_sub(ly.lines_before)).unwrap_or(0);
        let next = active.map_or(0, |i| i + 1);
        let size = ly.font_size;
        let font = self.lyrics_font();
        let faded = |alpha: f32| style::alpha(self.colors.text, alpha);
        let spacing = size * 0.3;
        let pad_y = size * 0.12;

        // The glide: right after a line starts, the line that just scrolled off still takes up
        // (a shrinking share of) its height, so everything below moves up smoothly.
        let mut glide = 0.0;
        if let Some(a) = active.filter(|&a| a > 0 && first > (a - 1).saturating_sub(ly.lines_before)) {
            let t = (pos.saturating_sub(l.lines[a].start_ms) as f32 / 420.0).min(1.0);
            let eased = 1.0 - (1.0 - t).powi(3);
            let gone = &l.lines[first - 1];
            // Roughly how many rows the line wrapped to: bold display text averages ~0.56 em a letter.
            let per_row = ((width - 28.0) / (size * 0.56)).max(8.0);
            let rows = (gone.text().chars().count() as f32 / per_row).ceil().max(1.0);
            glide = (1.0 - eased) * (rows * size * 1.3 + 2.0 * pad_y + spacing);
        }

        let mut col = Column::new().spacing(spacing).align_x(align).width(Fill);
        col = col.push(Space::new().height(size * 1.2 + glide));
        for (i, line) in l.lines.iter().enumerate().skip(first).take(ly.lines_before + 8) {
            if i == next && in_gap {
                let gap_start = active.map_or(0, |a| l.lines[a].end_ms);
                if line.start_ms.saturating_sub(gap_start) >= 3_000 {
                    let progress = (pos - gap_start.min(pos)) as f32 / (line.start_ms - gap_start) as f32;
                    let breath = 0.5 + 0.5 * (pos as f32 / 380.0).sin();
                    let dots: Vec<text::Span<'_, (), Font>> = (0..3)
                        .map(|d| {
                            let fill = (progress * 3.0 - d as f32).clamp(0.0, 1.0);
                            let glow = if fill > 0.0 && fill < 1.0 { 0.15 * breath } else { 0.0 };
                            span("●  ").color(style::mix(faded(0.22 + 0.7 * fill), self.colors.accent, glow))
                        })
                        .collect();
                    col = col.push(container(rich_text(dots).size(size * 0.55).font(font)).padding([size * 0.25, 14.0]));
                }
            }
            let distance = active.map_or(i as f32 + 1.0, |a| (i as f32 - a as f32).abs());
            let body: El = if Some(i) == active && !in_gap {
                self.active_line(line, pos, size, font, align)
            } else {
                let past = active.is_some_and(|a| i <= a);
                let alpha = if past { 0.3 - 0.05 * distance } else { 0.5 - 0.07 * distance };
                text(line.text()).size(size).font(font).color(faded(alpha.max(0.1))).align_x(align).width(Fill).into()
            };
            col = col.push(
                button(body)
                    .width(Fill)
                    .padding([pad_y, 14.0])
                    .style(style::lyric_line(self.colors, self.r()))
                    .on_press(Message::SeekTo(line.start_ms)),
            );
        }
        container(col).height(Fill).clip(true).into()
    }

    fn active_line(&self, line: &Line, pos: u32, size: f32, font: Font, align: Alignment) -> El<'_> {
        if !self.config.lyrics.word_highlight {
            return text(line.text()).size(size).font(font).align_x(align).width(Fill).into();
        }
        let lit = self.colors.text;
        let unlit = style::alpha(self.colors.text, 0.36);
        // The shine riding on the edge of the fill: the text colour warmed by the accent.
        let shine = style::mix(self.colors.text, style::mix(self.colors.accent, iced::Color::WHITE, 0.55), 0.85);
        let mut spans: Vec<text::Span<'_, (), Font>> = Vec::new();
        for (i, w) in line.words.iter().enumerate() {
            let sep = if i + 1 < line.words.len() { " " } else { "" };
            if pos >= w.end_ms {
                // Just-finished words keep a fading trace of the shine.
                let after = (pos - w.end_ms) as f32 / 350.0;
                let colour = if after < 1.0 { style::mix(shine, lit, after) } else { lit };
                spans.push(span(format!("{}{sep}", w.text)).color(colour));
            } else if pos < w.start_ms {
                spans.push(span(format!("{}{sep}", w.text)).color(unlit));
            } else {
                // The word being sung fills in letter by letter; the letters at the edge shine.
                let progress = (pos - w.start_ms) as f32 / (w.end_ms - w.start_ms).max(1) as f32;
                let chars: Vec<char> = w.text.chars().collect();
                let edge = progress * chars.len() as f32;
                for (k, c) in chars.iter().enumerate() {
                    let fill = (edge - k as f32).clamp(0.0, 1.0);
                    let near = 1.0 - ((edge - k as f32 - 0.5).abs() / 1.6).min(1.0);
                    let colour = style::mix(style::mix(unlit, lit, fill), shine, near * fill.max(0.4));
                    spans.push(span(c.to_string()).color(colour));
                }
                if !sep.is_empty() {
                    spans.push(span(sep).color(unlit));
                }
            }
        }
        rich_text(spans).size(size).font(font).align_x(align).width(Fill).into()
    }

    fn dj_page(&self) -> El<'_> {
        let dj = &self.config.dj;
        let now = self.queue.current().map(|e| &e.track);
        let status = if self.dj_busy {
            format!("{} is picking songs…", dj.name)
        } else if self.dj_on {
            format!("On air · {}", dj.mode)
        } else {
            "Off air".to_string()
        };
        let controls = row![
            self.input("What are you in the mood for?", &self.dj_request)
                .on_input(Message::DjRequestChanged)
                .on_submit(Message::DjStart)
                .padding([11, 16])
                .style(style::input(self.colors, 999.0))
                .width(Fill),
            self.primary_button(
                Some(icon::SPARKLES),
                if self.dj_on { "New set" } else { "Start DJ" },
                (!self.dj_busy).then_some(Message::DjStart)
            ),
        ]
        .push(if self.dj_on {
            self.small_button(Some(icon::CIRCLE_STOP), "Stop", Message::DjStop)
        } else {
            Space::new().into()
        })
        .spacing(10)
        .align_y(Alignment::Center);

        let chip_color = if self.dj_on { self.colors.accent } else { style::tint(self.colors, 0.14) };
        let mut hero = column![
            row![
                container(icon(icon::SPARKLES, 30.0)).center(60).style(style::tile(self.colors.accent, 999.0)),
                column![
                    self.heading(dj.name.clone(), 2.2),
                    container(text(status).size(self.fs(0.82)).font(self.weight(font::Weight::Semibold)))
                        .padding([3, 10])
                        .style(style::tile(chip_color, 999.0)),
                ]
                .spacing(6)
            ]
            .spacing(16)
            .align_y(Alignment::Center),
        ]
        .spacing(18);
        if dj.api_key().is_none() && dj.provider != LlmProvider::OpenAI {
            hero = hero.push(self.muted("Add your API key in Settings to wake the DJ up."));
        }
        hero = hero.push(controls);

        let now_playing = container(
            row![
                self.cover(now, 76.0),
                column![
                    self.label("NOW PLAYING"),
                    self.heading(now.map(|t| t.name.clone()).unwrap_or_else(|| "Nothing yet".into()), 1.25),
                    self.muted(now.map(|t| t.artist_line()).unwrap_or_default()),
                ]
                .spacing(3)
            ]
            .spacing(16)
            .align_y(Alignment::Center),
        )
        .padding(14)
        .width(Fill)
        .style(style::card(self.colors, self.r()));

        let mut log = Column::new().spacing(8);
        for line in self.dj_log.iter().rev().take(30) {
            let (who, said) = line.split_once(": ").unwrap_or(("", line.as_str()));
            let mut message = Column::new().spacing(3);
            if !who.is_empty() {
                message = message.push(text(who).size(self.fs(0.82)).font(self.weight(font::Weight::Bold)).color(self.colors.accent));
            }
            message = message.push(text(said).size(self.fs(0.98)));
            log = log.push(container(message).padding([10, 14]).width(Fill).style(style::card(self.colors, self.r())));
        }
        if self.dj_log.is_empty() {
            log = log.push(self.muted("What the DJ says shows up here."));
        }

        column![
            container(hero).padding(24).width(Fill).style(style::hero(self.colors, self.r())),
            now_playing,
            self.label("ON AIR"),
            self.scroll(log).height(Fill),
        ]
        .spacing(16)
        .into()
    }

    fn side_panel(&self) -> El<'_> {
        let current = self.queue.current();
        let mut list = Column::new().spacing(2);
        for (i, entry) in self.queue.up_next().iter().enumerate().take(200) {
            let mut title = row![text(&entry.track.name).size(self.fs(0.95)).wrapping(text::Wrapping::None)]
                .spacing(6)
                .align_y(Alignment::Center);
            if matches!(entry.origin, Origin::Dj { .. }) {
                title = title.push(icon(icon::SPARKLES, self.fs(0.8)).color(self.colors.accent));
            }
            let line = button(
                column![title, self.muted(entry.track.artist_line()).size(self.fs(0.82)).wrapping(text::Wrapping::None)]
                    .spacing(2)
                    .clip(true),
            )
            .width(Fill)
            .padding([6, 10])
            .style(style::row(self.colors, self.r(), false))
            .on_press(Message::JumpTo(i));
            list = list.push(
                row![line, self.icon_button(icon::X, self.fs(0.9), Some(Message::RemoveFromQueue(i)), false)]
                    .spacing(2)
                    .align_y(Alignment::Center),
            );
        }
        let header = row![
            self.heading("Queue", 1.2).width(Fill),
            self.small_button(None, "Clear", Message::ClearQueue),
        ]
        .align_y(Alignment::Center);

        let now: El = match current {
            Some(e) => container(
                row![
                    self.cover(Some(&e.track), 48.0),
                    column![
                        text(&e.track.name).size(self.fs(0.98)).color(self.colors.accent).wrapping(text::Wrapping::None),
                        self.muted(e.track.artist_line()).size(self.fs(0.82)).wrapping(text::Wrapping::None),
                    ]
                    .spacing(2)
                    .clip(true)
                ]
                .spacing(10)
                .align_y(Alignment::Center),
            )
            .padding(8)
            .width(Fill)
            .style(style::accent_card(self.colors, self.r()))
            .into(),
            None => self.muted("Nothing playing.").into(),
        };
        let body: El = if self.queue.up_next().is_empty() {
            self.muted(if self.dj_on { "The DJ will add more soon." } else { "Queue is empty." }).into()
        } else {
            self.scroll(list).height(Fill).into()
        };
        container(column![header, self.label("NOW PLAYING"), now, self.label("NEXT UP"), body].spacing(10))
            .width(self.config.layout.side_panel_width)
            .height(Fill)
            .padding(14)
            .style(self.glass())
            .into()
    }

    fn player_bar(&self) -> El<'_> {
        let current = self.queue.current().map(|e| &e.track);
        let duration = current.map(|t| t.duration_ms).unwrap_or(0) as f32;
        let position = self.seek_preview.unwrap_or(self.live_position() as f32).min(duration.max(1.0));

        let info = row![
            self.cover(current, 56.0),
            column![
                text(current.map(|t| t.name.clone()).unwrap_or_else(|| "Not playing".into()))
                    .size(self.fs(1.0))
                    .font(self.weight(font::Weight::Semibold))
                    .wrapping(text::Wrapping::None),
                self.muted(current.map(|t| t.artist_line()).unwrap_or_default()).size(self.fs(0.85)).wrapping(text::Wrapping::None),
            ]
            .clip(true)
            .spacing(3)
        ]
        .spacing(12)
        .align_y(Alignment::Center)
        .width(Length::FillPortion(3));

        let repeat = if self.queue.repeat == Repeat::One { icon::REPEAT_1 } else { icon::REPEAT };
        let play = button(
            container(icon(if self.playing { icon::PAUSE } else { icon::PLAY }, self.fs(1.2))).center(40),
        )
        .padding(0)
        .style(style::play_button(self.colors))
        .on_press_maybe(self.has_audio().then_some(Message::TogglePlay));
        let size = self.fs(1.15);
        let buttons = row![
            self.icon_button(icon::SHUFFLE, size, Some(Message::ToggleShuffle), self.queue.shuffle),
            self.icon_button(icon::SKIP_BACK, size, Some(Message::Previous), false),
            play,
            self.icon_button(icon::SKIP_FORWARD, size, Some(Message::Next), false),
            self.icon_button(repeat, size, Some(Message::CycleRepeat), self.queue.repeat != Repeat::Off),
        ]
        .spacing(10)
        .align_y(Alignment::Center);

        let seek = row![
            self.muted(mmss(position as u32)).size(self.fs(0.78)),
            self.slider_widget(0.0..=duration.max(1.0), position, Message::SeekPreview)
                .on_release(Message::SeekCommit)
                .width(Fill),
            self.muted(mmss(duration as u32)).size(self.fs(0.78)),
        ]
        .spacing(10)
        .align_y(Alignment::Center);

        let center = column![buttons, seek].spacing(4).align_x(Alignment::Center).width(Length::FillPortion(4)).max_width(640);

        let volume_icon = match self.volume {
            0 => icon::VOLUME_X,
            1..=49 => icon::VOLUME_1,
            _ => icon::VOLUME_2,
        };
        let right = row![
            self.icon_button(icon::MIC_VOCAL, size, Some(Message::ToggleLyrics), self.page == Page::Lyrics),
            self.icon_button(
                icon::SPARKLES,
                size,
                Some(if self.dj_on { Message::DjStop } else { Message::DjStart }),
                self.dj_on
            ),
            self.icon_button(icon::LIST_MUSIC, size, Some(Message::ToggleSidePanel), self.show_side_panel),
            icon(volume_icon, size).color(self.colors.muted_text),
            self.slider_widget(0.0..=100.0, self.volume as f32, Message::Volume)
                .on_release(Message::VolumeCommit)
                .width(96),
        ]
        .spacing(4)
        .align_y(Alignment::Center);

        container(
            row![info, center, container(right).width(Length::FillPortion(3)).align_x(Alignment::End)]
                .spacing(16)
                .align_y(Alignment::Center),
        )
        .padding([10, 14])
        .width(Fill)
        .height(Shrink)
        .style(self.glass())
        .into()
    }

    fn setting<'a>(&self, label: &'a str, control: impl Into<El<'a>>) -> El<'a> {
        row![text(label).size(self.fs(0.95)).width(170), container(control).width(Fill)]
            .spacing(12)
            .align_y(Alignment::Center)
            .into()
    }

    fn number<'a>(&self, label: &'a str, value: f32, range: std::ops::RangeInclusive<f32>, edit: fn(f32) -> Edit) -> El<'a> {
        self.setting(
            label,
            row![
                slider(range, value, move |v| Message::Edit(edit(v)))
                    .step(1.0_f32)
                    .height(16)
                    .style(style::form_slider(self.colors))
                    .width(Fill),
                text(format!("{value:.0}")).size(self.fs(0.9)).width(44),
            ]
            .spacing(10)
            .align_y(Alignment::Center),
        )
    }

    fn field<'a>(&self, label: &'a str, placeholder: &'a str, value: &'a str, edit: fn(String) -> Edit) -> El<'a> {
        self.setting(label, self.input(placeholder, value).on_input(move |s| Message::Edit(edit(s))))
    }

    fn toggle<'a>(&self, label: &'a str, value: bool, edit: fn(bool) -> Edit) -> El<'a> {
        self.setting(label, toggler(value).size(20).on_toggle(move |v| Message::Edit(edit(v))))
    }

    /// The built-in font, the system font, the fonts you added, and whatever is set now.
    fn font_picker<'a>(&self, current: &str, edit: fn(String) -> Edit) -> El<'a> {
        let mut options = vec![DEFAULT_FONT.to_string(), SYSTEM_FONT.to_string()];
        options.extend(self.font_families.iter().cloned());
        let selected = match current {
            "" => DEFAULT_FONT.to_string(),
            c if c.eq_ignore_ascii_case("system") => SYSTEM_FONT.to_string(),
            c => c.to_string(),
        };
        if !options.contains(&selected) {
            options.push(selected.clone());
        }
        self.picker(options, Some(selected), move |f| Message::Edit(edit(f)))
    }

    /// The built-in app icons, plus your own when you've chosen one. Picking one sets the window,
    /// taskbar and menu icon and the logo in the sidebar.
    fn icon_picker(&self, size: f32) -> El<'_> {
        let current = self.config.appearance.app_icon.trim();
        let custom = !current.is_empty() && custom::builtin_icon(current).is_none();
        let colors = self.colors;
        let tile = move |handle: image::Handle, selected: bool, msg: Option<Message>| -> El<'_> {
            button(image(handle).width(size).height(size))
                .padding(3)
                .style(move |_, status| {
                    let color = if selected {
                        colors.accent
                    } else if status == button::Status::Hovered {
                        style::tint(colors, 0.3)
                    } else {
                        iced::Color::TRANSPARENT
                    };
                    button::Style { border: iced::Border { color, width: 2.0, radius: (size * 0.3).into() }, ..button::Style::default() }
                })
                .on_press_maybe(msg)
                .into()
        };
        let mut tiles: Vec<El> = self
            .icon_choices
            .iter()
            .map(|(name, handle)| {
                let selected = if current.is_empty() { *name == "haze" } else { name.eq_ignore_ascii_case(current) };
                tile(handle.clone(), selected, Some(Message::SetIcon(name)))
            })
            .collect();
        if custom {
            tiles.push(tile(self.icon.clone(), true, None));
        }
        Row::with_children(tiles).spacing(4).wrap().into()
    }

    /// A row of pill tabs so Settings shows one group at a time.
    fn settings_tabs(&self) -> El<'_> {
        let tabs = SettingsTab::ALL.iter().map(|tab| {
            let selected = *tab == self.settings_tab;
            let colors = self.colors;
            button(
                row![icon(tab.icon(), self.fs(0.95)), text(tab.label()).size(self.fs(0.9)).font(self.weight(font::Weight::Semibold))]
                    .spacing(7)
                    .align_y(Alignment::Center),
            )
            .padding([7, 14])
            .style(move |_, status| {
                let background = if selected {
                    colors.accent
                } else if status == button::Status::Hovered {
                    style::tint(colors, 0.16)
                } else {
                    style::tint(colors, 0.08)
                };
                button::Style {
                    background: Some(iced::Background::Color(background)),
                    text_color: if selected { style::readable_on(colors.accent) } else { colors.text },
                    border: iced::Border { radius: 999.0.into(), ..iced::Border::default() },
                    ..button::Style::default()
                }
            })
            .on_press(Message::SettingsTab(*tab))
            .into()
        });
        Row::with_children(tabs).spacing(8).wrap().into()
    }

    fn section<'a>(&self, glyph: char, title: &'a str, rows: Vec<El<'a>>) -> El<'a> {
        let heading = row![icon(glyph, self.fs(1.2)).color(self.colors.accent), self.heading(title, 1.2)]
            .spacing(10)
            .align_y(Alignment::Center);
        container(column![heading, Column::with_children(rows).spacing(12)].spacing(16))
            .padding(20)
            .width(Fill)
            .style(style::card(self.colors, self.r()))
            .into()
    }

    fn settings_page(&self) -> El<'_> {
        let a = &self.config.appearance;
        let l = &self.config.layout;
        let dj = &self.config.dj;
        let ly = &self.config.lyrics;
        let key_env = dj.provider.default_key_env();

        let dj_rows = vec![
            self.setting("Provider", self.picker(LlmProvider::ALL, Some(dj.provider), |p| Message::Edit(Edit::Provider(p)))),
            self.field("Model", dj.provider.default_model(), &dj.model, Edit::Model),
            self.field("API base URL", dj.provider.default_base_url(), &dj.base_url, Edit::BaseUrl),
            self.setting(
                "API key",
                self.input("Stored in your config file", &dj.api_key).on_input(|s| Message::Edit(Edit::ApiKey(s))).secure(true),
            ),
            self.field("…or key from env var", key_env, &dj.api_key_env, Edit::ApiKeyEnv),
            self.setting("What the DJ does", self.picker(DjMode::ALL, Some(dj.mode), |m| Message::Edit(Edit::Mode(m)))),
            self.field("DJ name", "Haz DJ", &dj.name, Edit::DjName),
            self.field("Personality", "Describe the DJ's style", &dj.persona, Edit::Persona),
            self.field("Language", "English", &dj.language, Edit::Language),
            self.number("Songs per set", dj.batch_size as f32, 1.0..=20.0, Edit::BatchSize),
            self.number("Refill when queue has", dj.refill_at as f32, 0.0..=10.0, Edit::RefillAt),
            self.number("Talk every Nth song", dj.talk_every as f32, 1.0..=10.0, Edit::TalkEvery),
            self.number("Max words per intro", dj.max_words as f32, 10.0..=120.0, Edit::MaxWords),
            self.number("Discovery (new vs. familiar)", dj.discovery as f32, 0.0..=100.0, Edit::Discovery),
            self.toggle("Explicit songs", dj.allow_explicit, Edit::Explicit),
        ];
        let voice_rows = vec![
            self.setting("Voice engine", self.picker(TtsEngine::ALL, Some(dj.tts.engine), |e| Message::Edit(Edit::TtsEngine(e)))),
            self.field("Voice", "default", &dj.tts.voice, Edit::Voice),
            self.field("TTS model", "gpt-4o-mini-tts", &dj.tts.model, Edit::TtsModel),
            self.number("Music volume while talking %", dj.tts.duck_to as f32, 0.0..=100.0, Edit::DuckTo),
        ];
        let icon_controls = column![
            self.icon_picker(34.0),
            self.small_button(Some(icon::IMAGE), "Use your own image…", Message::PickIcon),
        ]
        .spacing(10);
        let look_rows = vec![
            self.setting(
                "Theme",
                row![
                    self.picker(self.theme_names.clone(), Some(a.theme.clone()), |t| Message::Edit(Edit::Theme(t))),
                    self.small_button(Some(icon::FOLDER_OPEN), "Open folder", Message::OpenConfigFolder),
                ]
                .spacing(10)
                .align_y(Alignment::Center),
            ),
            self.field("Accent colour", "#ff5c8a", &a.colors.accent, Edit::Accent),
            self.toggle("Accent from cover art", a.accent_from_cover, Edit::AccentFromCover),
            self.toggle("Cover glow background", a.ambient_background, Edit::AmbientBackground),
            self.setting("App icon", icon_controls),
            self.setting(
                "App font",
                row![
                    self.font_picker(&a.font_family, Edit::AppFont),
                    self.small_button(Some(icon::TYPE), "Add font file…", Message::PickFont),
                ]
                .spacing(10)
                .align_y(Alignment::Center),
            ),
            self.field("…or an installed font", "e.g. Noto Sans", &a.font_family, Edit::AppFont),
            self.muted("The app font and text size apply after a restart. Lyrics fonts apply right away.").into(),
            self.number("Text size", a.font_size, 10.0..=24.0, Edit::FontSize),
            self.number("Corner radius", a.corner_radius, 0.0..=20.0, Edit::Radius),
            self.toggle("Compact rows", a.compact, Edit::Compact),
            self.toggle("Album art", a.show_album_art, Edit::AlbumArt),
        ];
        let lyrics_rows = vec![
            self.setting(
                "Lyrics provider",
                self.picker(LyricsProvider::ALL, LyricsProvider::of(&ly.sources), |p| Message::Edit(Edit::LyricsProvider(p))),
            ),
            self.toggle("Word-by-word highlight", ly.word_highlight, Edit::WordHighlight),
            self.toggle("Blurred cover background", ly.blurred_background, Edit::BlurredBackground),
            self.setting("Alignment", self.picker(LyricsAlign::ALL, Some(ly.align), |v| Message::Edit(Edit::LyricsAlign(v)))),
            self.setting("Lyrics font", self.font_picker(&ly.font_family, Edit::LyricsFont)),
            self.number("Lyrics text size", ly.font_size, 16.0..=72.0, Edit::LyricsSize),
            self.number("Past lines shown", ly.lines_before as f32, 0.0..=6.0, Edit::LinesBefore),
            self.setting(
                "Timing offset (ms)",
                row![
                    slider(-3000.0..=3000.0, ly.offset_ms as f32, |v| Message::Edit(Edit::LyricsOffset(v)))
                        .step(50.0_f32)
                        .height(16)
                        .style(style::form_slider(self.colors))
                        .width(Fill),
                    text(format!("{:+}", ly.offset_ms)).size(self.fs(0.9)).width(56),
                ]
                .spacing(10)
                .align_y(Alignment::Center),
            ),
        ];
        let layout_rows = vec![
            self.setting("Sidebar", self.picker(Side::ALL, Some(l.sidebar), |s| Message::Edit(Edit::Sidebar(s)))),
            self.setting("Queue panel", self.picker(Side::ALL, Some(l.side_panel), |s| Message::Edit(Edit::SidePanel(s)))),
            self.setting("Player bar", self.picker(Edge::ALL, Some(l.player_bar), |e| Message::Edit(Edit::PlayerBar(e)))),
        ];

        let update_rows = vec![
            self.setting(
                "Check at startup",
                toggler(self.config.updates.check).on_toggle(|v| Message::Edit(Edit::CheckUpdates(v))).size(20),
            ),
            self.setting(
                "Version",
                row![
                    text(format!("Haze {}", env!("CARGO_PKG_VERSION"))).size(self.fs(0.95)),
                    self.small_button(None, "Check now", Message::CheckForUpdates(true)),
                ]
                .spacing(12)
                .align_y(Alignment::Center),
            ),
        ];

        let form = column![
            self.heading("Settings", 2.0),
            self.muted(format!(
                "Saved to {}. Every option, including ones not shown here, lives in that file.",
                crate::config::config_path().map(|p| p.display().to_string()).unwrap_or_default()
            )),
            row![
                self.primary_button(Some(icon::SAVE), "Save settings", Some(Message::SaveSettings)),
                self.small_button(Some(icon::LOG_OUT), "Log out of Spotify", Message::Logout),
                self.small_button(Some(icon::SPARKLES), "Run setup again", Message::RunSetup),
            ]
            .spacing(10)
            .align_y(Alignment::Center),
            self.settings_tabs(),
            match self.settings_tab {
                SettingsTab::Dj => column![
                    self.section(icon::SPARKLES, "DJ", dj_rows),
                    self.section(icon::AUDIO_LINES, "DJ voice", voice_rows)
                ]
                .spacing(16)
                .into(),
                SettingsTab::Look => self.section(icon::PALETTE, "Appearance", look_rows),
                SettingsTab::Lyrics => self.section(icon::MIC_VOCAL, "Lyrics", lyrics_rows),
                SettingsTab::Layout => self.section(icon::SLIDERS_HORIZONTAL, "Layout", layout_rows),
                SettingsTab::Updates => self.section(icon::ROTATE_CCW, "Updates", update_rows),
            },
        ]
        .spacing(16)
        .max_width(820);
        self.scroll(container(form).padding(Padding { top: 0.0, right: 16.0, bottom: 8.0, left: 0.0 }))
            .height(Fill)
            .into()
    }
}
