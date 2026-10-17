//! All drawing. Layout follows `[layout]` in the config: sidebar side, player bar edge, side panel.

use iced::widget::{
    Column, Space, button, column, container, image, pick_list, rich_text, row, rule, scrollable, slider, span, stack,
    text, text_input, toggler,
};
use iced::{Alignment, Color, ContentFit, Element, Fill, Font, Length, Shrink, font};

use super::{App, DEFAULT_FONT, Edit, Login, Message, Page, custom, style};
use crate::config::{DjMode, Edge, LlmProvider, LyricsAlign, Side, TtsEngine, non_empty};
use crate::lyrics::{Line, Lyrics};
use crate::queue::{Origin, Repeat};
use crate::spotify::api::Track;

type El<'a> = Element<'a, Message>;

fn mmss(ms: u32) -> String {
    let s = ms / 1000;
    format!("{}:{:02}", s / 60, s % 60)
}

impl App {
    fn fs(&self, scale: f32) -> f32 {
        self.config.appearance.font_size * scale
    }

    fn r(&self) -> f32 {
        self.config.appearance.corner_radius
    }

    fn muted<'a>(&self, s: impl text::IntoFragment<'a>) -> text::Text<'a> {
        text(s).size(self.fs(0.9)).color(self.colors.muted_text)
    }

    fn heading<'a>(&self, s: impl text::IntoFragment<'a>) -> text::Text<'a> {
        text(s).size(self.fs(1.6))
    }

    fn primary_button<'a>(&self, label: &'a str, msg: Option<Message>) -> El<'a> {
        button(text(label).size(self.fs(1.0)))
            .padding([8, 16])
            .style(style::primary(self.colors, self.r()))
            .on_press_maybe(msg)
            .into()
    }

    fn ghost<'a>(&self, label: &'a str, msg: Message, active: bool) -> El<'a> {
        button(text(label).size(self.fs(1.15)))
            .padding([4, 10])
            .style(style::ghost(self.colors, self.r(), active))
            .on_press(msg)
            .into()
    }

    fn small_button<'a>(&self, label: &'a str, msg: Message) -> El<'a> {
        button(text(label).size(self.fs(0.9)))
            .padding([5, 10])
            .style(style::secondary(self.colors, self.r()))
            .on_press(msg)
            .into()
    }

    pub fn view(&self) -> El<'_> {
        let content: El = match self.login {
            Login::LoggedIn => self.main_layout(),
            _ => self.login_view(),
        };
        let mut root = Column::new();
        if let Some(notice) = &self.notice {
            root = root.push(
                container(
                    row![
                        text(&notice.text).size(self.fs(0.95)).width(Fill),
                        self.ghost("✕", Message::DismissNotice, false)
                    ]
                    .align_y(Alignment::Center),
                )
                .padding([6, 12])
                .width(Fill)
                .style(style::banner(self.colors, 0.0, notice.error)),
            );
        }
        container(root.push(content)).width(Fill).height(Fill).style(style::page(self.colors)).into()
    }

    fn login_view(&self) -> El<'_> {
        let body: El = match self.login {
            Login::NeedsClientId => column![
                self.heading("Welcome to Haztify"),
                text("Haztify talks to Spotify through your own free developer app, so your login stays yours."),
                self.muted("1. Open developer.spotify.com/dashboard and create an app."),
                self.muted(format!(
                    "2. Add this Redirect URI exactly: {}",
                    self.config.spotify.redirect_uri
                )),
                self.muted("3. Tick \"Web API\" and \"Web Playback SDK\", save, and copy the Client ID."),
                text_input("Paste your Client ID", &self.client_id_input)
                    .on_input(Message::ClientIdChanged)
                    .on_submit(Message::SaveClientId)
                    .padding(10),
                self.primary_button("Save and log in", Some(Message::SaveClientId)),
            ]
            .spacing(12)
            .max_width(560)
            .into(),
            Login::InProgress => column![
                self.heading("Logging in…"),
                self.muted("Finish logging in in your browser, then come back here."),
            ]
            .spacing(12)
            .into(),
            _ => column![
                self.heading("Haztify"),
                self.muted("Log in with your Spotify Premium account to start listening."),
                self.primary_button("Log in with Spotify", Some(Message::Login)),
            ]
            .spacing(12)
            .into(),
        };
        container(body).center(Fill).padding(24).into()
    }

    fn main_layout(&self) -> El<'_> {
        let layout = &self.config.layout;
        let on_lyrics = self.page == Page::Lyrics;
        let main = container(self.page_view()).width(Fill).height(Fill).padding(if on_lyrics { 0 } else { 16 });
        if on_lyrics && self.fullscreen {
            return column![main, self.player_bar()].into();
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
        let body = row(left).push(main).extend(right).height(Fill);

        let bar = self.player_bar();
        match layout.player_bar {
            Edge::Top => column![bar, body].into(),
            Edge::Bottom => column![body, bar].into(),
        }
    }

    fn nav<'a>(&self, label: String, page: Page) -> El<'a> {
        let selected = self.page == page;
        button(text(label).size(self.fs(1.0)))
            .width(Fill)
            .padding([6, 10])
            .style(style::row(self.colors, self.r(), selected))
            .on_press(Message::Open(page))
            .into()
    }

    fn sidebar(&self) -> El<'_> {
        let mut playlists = Column::new().spacing(2);
        for p in &self.playlists {
            playlists = playlists.push(self.nav(p.name.clone(), Page::Playlist(p.clone())));
        }
        let top = column![
            text("Haztify").size(self.fs(1.4)).color(self.colors.accent),
            self.muted(if self.user_name.is_empty() { "…".to_string() } else { self.user_name.clone() }),
            Space::new().height(8),
            self.nav("✦ DJ".into(), Page::Dj),
            self.nav("⌕ Search".into(), Page::Search),
            self.nav("♥ Liked Songs".into(), Page::Liked),
            self.nav("♪ Lyrics".into(), Page::Lyrics),
            self.nav("⚙ Settings".into(), Page::Settings),
            Space::new().height(8),
            self.muted("PLAYLISTS"),
        ]
        .spacing(4);
        container(column![top, scrollable(playlists).height(Fill)].spacing(6))
            .width(self.config.layout.sidebar_width)
            .height(Fill)
            .padding(12)
            .style(style::panel(self.colors, 0.0))
            .into()
    }

    fn page_view(&self) -> El<'_> {
        match &self.page {
            Page::Dj => self.dj_page(),
            Page::Search => {
                let input = text_input("Search songs", &self.search)
                    .on_input(Message::SearchChanged)
                    .on_submit(Message::SearchSubmit)
                    .padding(10)
                    .size(self.fs(1.05));
                column![self.heading("Search"), input, self.track_list()].spacing(12).into()
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
            "title" => Length::FillPortion(4),
            "artist" | "album" => Length::FillPortion(3),
            _ => Length::Fixed(56.0),
        }
    }

    fn cell<'a>(&self, name: &str, track: &'a Track) -> El<'a> {
        let width = Self::column_width(name);
        let t = match name {
            "title" => {
                let mut title = track.name.clone();
                if track.explicit {
                    title.push_str("  🅴");
                }
                text(title).size(self.fs(1.0))
            }
            "artist" => self.muted(track.artist_line()),
            "album" => self.muted(track.album.as_str()),
            "duration" => self.muted(mmss(track.duration_ms)),
            _ => text(""),
        };
        container(t.wrapping(text::Wrapping::None)).width(width).clip(true).into()
    }

    fn track_list(&self) -> El<'_> {
        if self.loading {
            return self.muted("Loading…").into();
        }
        if self.tracks.is_empty() {
            return self.muted("Nothing here yet.").into();
        }
        let columns = &self.config.layout.track_columns;
        let pad = if self.config.appearance.compact { 3 } else { 8 };
        let playing_uri = self.queue.current().map(|e| e.track.uri.as_str());

        let mut header = row![].spacing(12).padding([0, 10]);
        for c in columns {
            header = header.push(
                container(self.muted(c.to_uppercase()).size(self.fs(0.75))).width(Self::column_width(c)),
            );
        }
        header = header.push(Space::new().width(64));

        let mut list = Column::new().spacing(1);
        for (i, track) in self.tracks.iter().enumerate() {
            let mut cells = row![].spacing(12).align_y(Alignment::Center);
            for c in columns {
                cells = cells.push(self.cell(c, track));
            }
            let actions = row![
                self.ghost("↪", Message::PlayNextTrack(i), false),
                self.ghost("+", Message::QueueTrack(i), false),
            ]
            .spacing(0);
            let line = button(cells)
                .width(Fill)
                .padding([pad, 10])
                .style(style::row(self.colors, self.r(), playing_uri == Some(track.uri.as_str())))
                .on_press(Message::PlayFrom(i));
            list = list.push(row![line, actions].align_y(Alignment::Center));
        }
        column![header, rule::horizontal(1), scrollable(list).height(Fill)].spacing(4).into()
    }

    fn cover(&self, track: Option<&Track>, size: f32) -> El<'_> {
        let handle = track.and_then(|t| t.image_url.as_deref()).and_then(|u| self.image(u));
        match (self.config.appearance.show_album_art, handle) {
            (true, Some(h)) => image(h.clone()).width(size).height(size).into(),
            (true, None) => container(Space::new().width(size).height(size))
                .style(style::panel(self.colors, self.r()))
                .into(),
            (false, _) => Space::new().width(0).into(),
        }
    }

    /// A playlist's (or Liked Songs') cover: yours if you set one, otherwise Spotify's.
    fn collection_cover(&self, key: &str, url: Option<&str>, size: f32) -> El<'_> {
        let handle = match self.config.covers.get(key) {
            Some(path) if std::path::Path::new(path).exists() => Some(image::Handle::from_path(path)),
            _ => url.and_then(|u| self.image(u)).cloned(),
        };
        match handle {
            Some(h) => container(image(h).width(size).height(size).content_fit(ContentFit::Cover))
                .clip(true)
                .style(style::panel(self.colors, self.r()))
                .into(),
            None => container(text(if key == "liked" { "♥" } else { "♪" }).size(size * 0.4).color(self.colors.accent))
                .center_x(size)
                .center_y(size)
                .style(style::panel(self.colors, self.r()))
                .into(),
        }
    }

    fn collection_header<'a>(&'a self, key: &str, title: String, subtitle: String, url: Option<&str>) -> El<'a> {
        let custom = self.config.covers.contains_key(key);
        let mut actions = row![self.small_button("Change cover…", Message::PickCover(key.to_string()))].spacing(6);
        if custom {
            actions = actions.push(self.small_button("Reset cover", Message::ResetCover(key.to_string())));
        }
        row![
            self.collection_cover(key, url, 132.0),
            column![self.heading(title), self.muted(subtitle), actions].spacing(6)
        ]
        .spacing(20)
        .align_y(Alignment::End)
        .into()
    }

    fn lyrics_font(&self) -> Font {
        let ly = &self.config.lyrics;
        custom::font(non_empty(&ly.font_family).unwrap_or(&self.config.appearance.font_family))
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
        let header = row![
            self.cover(current, 84.0),
            column![
                text(current.map(|t| t.name.clone()).unwrap_or_else(|| "Lyrics".into())).size(self.fs(1.5)),
                self.muted(current.map(|t| t.artist_line()).unwrap_or_default()),
                self.muted(source).size(self.fs(0.8)),
            ]
            .spacing(3)
            .width(Fill),
            self.small_button(if self.fullscreen { "Exit full screen" } else { "⛶ Full screen" }, Message::ToggleFullscreen),
        ]
        .spacing(16)
        .align_y(Alignment::Center);

        let body: El = match (current, lyrics) {
            (None, _) => self.muted("Play a song to see its lyrics here.").into(),
            (Some(_), None) if self.lyrics_loading => self.muted("Looking for lyrics…").into(),
            (Some(_), None) => self.muted("No lyrics found for this song.").into(),
            (Some(_), Some(l)) if l.synced => self.synced_lyrics(l, align),
            (Some(_), Some(l)) => {
                let mut lines = Column::new().spacing(ly.font_size * 0.4).align_x(align).width(Fill);
                for line in &l.lines {
                    lines = lines.push(
                        text(line.text())
                            .size(ly.font_size * 0.8)
                            .font(self.lyrics_font())
                            .align_x(align)
                            .width(Fill),
                    );
                }
                scrollable(lines).height(Fill).into()
            }
        };

        let content = container(column![header, container(body).height(Fill).width(Fill)].spacing(28))
            .padding([24, 32])
            .width(Fill)
            .height(Fill);
        match (&self.lyrics_background, ly.blurred_background && self.config.appearance.show_album_art) {
            (Some(bg), true) if current.is_some() => stack![
                image(bg.clone())
                    .width(Fill)
                    .height(Fill)
                    .content_fit(ContentFit::Cover)
                    .filter_method(image::FilterMethod::Linear),
                content
            ]
            .into(),
            _ => content.into(),
        }
    }

    /// Karaoke-style lyrics: the line being sung is bright and bold, its words light up as they're
    /// sung, past lines fade out and long instrumental breaks show three filling dots.
    fn synced_lyrics(&self, l: &Lyrics, align: Alignment) -> El<'_> {
        let ly = &self.config.lyrics;
        let pos = (self.live_position() as i64 - ly.offset_ms as i64).max(0) as u32;
        let active = l.active_line(pos);
        let in_gap = active.is_none_or(|i| pos >= l.lines[i].end_ms);
        let first = active.map(|i| i.saturating_sub(ly.lines_before)).unwrap_or(0);
        let next = active.map_or(0, |i| i + 1);
        let size = ly.font_size;
        let font = self.lyrics_font();
        let faded = |alpha: f32| Color { a: alpha, ..self.colors.text };

        let mut col = Column::new().spacing(size * 0.35).align_x(align).width(Fill);
        for (i, line) in l.lines.iter().enumerate().skip(first).take(ly.lines_before + 8) {
            if i == next && in_gap {
                let gap_start = active.map_or(0, |a| l.lines[a].end_ms);
                if line.start_ms.saturating_sub(gap_start) >= 3_000 {
                    let progress = (pos - gap_start.min(pos)) as f32 / (line.start_ms - gap_start) as f32;
                    let dots: Vec<text::Span<'_, (), Font>> = (0..3)
                        .map(|d| {
                            let lit = progress > (d as f32 + 0.5) / 3.0;
                            span("● ").color(if lit { self.colors.text } else { faded(0.3) })
                        })
                        .collect();
                    col = col.push(container(rich_text(dots).size(size * 0.6)).padding([4, 12]));
                }
            }
            let active_line = Some(i) == active && !in_gap;
            let body: El = if active_line {
                self.active_line(line, pos, size, font, align)
            } else {
                let alpha = if active.is_some_and(|a| i <= a) { 0.32 } else { 0.55 };
                text(line.text()).size(size).font(font).color(faded(alpha)).align_x(align).width(Fill).into()
            };
            col = col.push(
                button(body)
                    .width(Fill)
                    .padding([4, 12])
                    .style(style::lyric_line(self.colors, self.r()))
                    .on_press(Message::SeekTo(line.start_ms)),
            );
        }
        col.into()
    }

    fn active_line(&self, line: &Line, pos: u32, size: f32, font: Font, align: Alignment) -> El<'_> {
        let bold = Font { weight: font::Weight::Bold, ..font };
        if !self.config.lyrics.word_highlight {
            return text(line.text()).size(size).font(bold).align_x(align).width(Fill).into();
        }
        let upcoming = Color { a: 0.5, ..self.colors.text };
        let spans: Vec<text::Span<'_, (), Font>> = line
            .words
            .iter()
            .enumerate()
            .map(|(i, w)| {
                let color = if pos >= w.end_ms {
                    self.colors.text
                } else if pos >= w.start_ms {
                    self.colors.accent
                } else {
                    upcoming
                };
                let sep = if i + 1 < line.words.len() { " " } else { "" };
                span(format!("{}{sep}", w.text)).color(color)
            })
            .collect();
        rich_text(spans).size(size).font(bold).align_x(align).width(Fill).into()
    }

    fn dj_page(&self) -> El<'_> {
        let dj = &self.config.dj;
        let now = self.queue.current().map(|e| &e.track);
        let status = if self.dj_busy {
            format!("{} is picking songs…", dj.name)
        } else if self.dj_on {
            format!("{} is on air · {}", dj.name, dj.mode)
        } else {
            format!("{} is off air", dj.name)
        };
        let key_hint = if dj.api_key().is_none() && dj.provider != LlmProvider::OpenAI {
            Some(self.muted("Add your API key in Settings to wake the DJ up."))
        } else {
            None
        };
        let controls = row![
            text_input("Tell the DJ what you're in the mood for (optional)", &self.dj_request)
                .on_input(Message::DjRequestChanged)
                .on_submit(Message::DjStart)
                .padding(10)
                .width(Fill),
            self.primary_button(if self.dj_on { "New set" } else { "Start DJ" }, (!self.dj_busy).then_some(Message::DjStart)),
        ]
        .push(if self.dj_on { self.ghost("Stop", Message::DjStop, false) } else { Space::new().into() })
        .spacing(8)
        .align_y(Alignment::Center);

        let mut log = Column::new().spacing(8);
        for line in self.dj_log.iter().rev().take(30) {
            log = log.push(text(line.as_str()).size(self.fs(1.0)));
        }

        let now_playing = row![
            self.cover(now, 120.0),
            column![
                self.muted("NOW PLAYING"),
                text(now.map(|t| t.name.clone()).unwrap_or_else(|| "Nothing yet".into())).size(self.fs(1.4)),
                self.muted(now.map(|t| t.artist_line()).unwrap_or_default()),
            ]
            .spacing(4)
        ]
        .spacing(16)
        .align_y(Alignment::Center);

        column![
            self.heading("DJ"),
            text(status).color(self.colors.accent),
        ]
        .push(match key_hint {
            Some(hint) => El::from(hint),
            None => Space::new().into(),
        })
        .push(controls)
        .push(now_playing)
        .push(rule::horizontal(1))
        .push(self.muted("ON AIR"))
        .push(scrollable(log).height(Fill))
        .spacing(12)
        .into()
    }

    fn side_panel(&self) -> El<'_> {
        let mut list = Column::new().spacing(2);
        for (i, entry) in self.queue.up_next().iter().enumerate().take(200) {
            let badge = if matches!(entry.origin, Origin::Dj { .. }) { "✦ " } else { "" };
            let line = button(
                column![
                    text(format!("{badge}{}", entry.track.name)).size(self.fs(0.95)).wrapping(text::Wrapping::None),
                    self.muted(entry.track.artist_line()).size(self.fs(0.8)).wrapping(text::Wrapping::None),
                ]
                .clip(true),
            )
            .width(Fill)
            .padding([4, 8])
            .style(style::row(self.colors, self.r(), false))
            .on_press(Message::JumpTo(i));
            list = list.push(row![line, self.ghost("✕", Message::RemoveFromQueue(i), false)].align_y(Alignment::Center));
        }
        let header = row![
            text("Up next").size(self.fs(1.15)).width(Fill),
            self.ghost("Clear", Message::ClearQueue, false),
        ]
        .align_y(Alignment::Center);
        let body: El = if self.queue.up_next().is_empty() {
            self.muted(if self.dj_on { "The DJ will add more soon." } else { "Queue is empty." }).into()
        } else {
            scrollable(list).height(Fill).into()
        };
        container(column![header, body].spacing(8))
            .width(self.config.layout.side_panel_width)
            .height(Fill)
            .padding(12)
            .style(style::panel(self.colors, 0.0))
            .into()
    }

    fn player_bar(&self) -> El<'_> {
        let current = self.queue.current().map(|e| &e.track);
        let duration = current.map(|t| t.duration_ms).unwrap_or(0) as f32;
        let position = self.seek_preview.unwrap_or(self.position_ms as f32).min(duration.max(1.0));

        let info = row![
            self.cover(current, 52.0),
            column![
                text(current.map(|t| t.name.clone()).unwrap_or_else(|| "Not playing".into()))
                    .size(self.fs(1.0))
                    .wrapping(text::Wrapping::None),
                self.muted(current.map(|t| t.artist_line()).unwrap_or_default()).wrapping(text::Wrapping::None),
            ]
            .clip(true)
            .spacing(2)
        ]
        .spacing(10)
        .align_y(Alignment::Center)
        .width(Length::FillPortion(3));

        let repeat = match self.queue.repeat {
            Repeat::Off => "↻",
            Repeat::All => "↻",
            Repeat::One => "↻1",
        };
        let buttons = row![
            self.ghost("⤮", Message::ToggleShuffle, self.queue.shuffle),
            self.ghost("⏮", Message::Previous, false),
            button(text(if self.playing { "⏸" } else { "▶" }).size(self.fs(1.3)))
                .padding([4, 14])
                .style(style::primary(self.colors, 999.0))
                .on_press_maybe(self.has_audio().then_some(Message::TogglePlay)),
            self.ghost("⏭", Message::Next, false),
            self.ghost(repeat, Message::CycleRepeat, self.queue.repeat != Repeat::Off),
        ]
        .spacing(6)
        .align_y(Alignment::Center);

        let seek = row![
            self.muted(mmss(position as u32)),
            slider(0.0..=duration.max(1.0), position, Message::SeekPreview)
                .on_release(Message::SeekCommit)
                .width(Fill),
            self.muted(mmss(duration as u32)),
        ]
        .spacing(8)
        .align_y(Alignment::Center);

        let center = column![buttons, seek].spacing(4).align_x(Alignment::Center).width(Length::FillPortion(4));

        let right = row![
            self.ghost("♪", Message::ToggleLyrics, self.page == Page::Lyrics),
            self.ghost(if self.dj_on { "✦ DJ on" } else { "✦ DJ" }, if self.dj_on { Message::DjStop } else { Message::DjStart }, self.dj_on),
            self.ghost("☰", Message::ToggleSidePanel, self.show_side_panel),
            self.muted("Vol"),
            slider(0.0..=100.0, self.volume as f32, Message::Volume)
                .on_release(Message::VolumeCommit)
                .width(100),
        ]
        .spacing(6)
        .align_y(Alignment::Center);

        container(row![info, center, container(right).width(Length::FillPortion(3)).align_x(Alignment::End)].spacing(16).align_y(Alignment::Center))
            .padding([10, 16])
            .width(Fill)
            .height(Shrink)
            .style(style::panel(self.colors, 0.0))
            .into()
    }

    fn setting<'a>(&self, label: &'a str, control: impl Into<El<'a>>) -> El<'a> {
        row![text(label).size(self.fs(0.95)).width(200), container(control).width(Fill)]
            .spacing(12)
            .align_y(Alignment::Center)
            .into()
    }

    fn number<'a>(&self, label: &'a str, value: f32, range: std::ops::RangeInclusive<f32>, edit: fn(f32) -> Edit) -> El<'a> {
        self.setting(
            label,
            row![
                slider(range, value, move |v| Message::Edit(edit(v))).step(1.0_f32).width(Fill),
                text(format!("{value:.0}")).width(40),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        )
    }

    fn field<'a>(&self, label: &'a str, placeholder: &'a str, value: &'a str, edit: fn(String) -> Edit) -> El<'a> {
        self.setting(label, text_input(placeholder, value).on_input(move |s| Message::Edit(edit(s))).padding(6))
    }

    /// "Default", the fonts you added, and whatever is set now (it may be an installed font).
    fn font_picker<'a>(&self, current: &str, edit: fn(String) -> Edit) -> El<'a> {
        let mut options = vec![DEFAULT_FONT.to_string()];
        options.extend(self.font_families.iter().cloned());
        let selected = if current.is_empty() { DEFAULT_FONT.to_string() } else { current.to_string() };
        if !options.contains(&selected) {
            options.push(selected.clone());
        }
        pick_list(options, Some(selected), move |f| Message::Edit(edit(f))).into()
    }

    fn settings_page(&self) -> El<'_> {
        let a = &self.config.appearance;
        let l = &self.config.layout;
        let dj = &self.config.dj;
        let ly = &self.config.lyrics;
        let section = |s: &'static str| text(s).size(self.fs(1.25)).color(self.colors.accent);

        let key_env = dj.provider.default_key_env();
        let mut form = column![
            self.heading("Settings"),
            self.muted(format!(
                "Saved to {}. Every option, including ones not shown here, lives in that file.",
                crate::config::config_path().map(|p| p.display().to_string()).unwrap_or_default()
            )),
            section("DJ"),
            self.setting("Provider", pick_list(LlmProvider::ALL, Some(dj.provider), |p| Message::Edit(Edit::Provider(p)))),
            self.field("Model", dj.provider.default_model(), &dj.model, Edit::Model),
            self.field("API base URL", dj.provider.default_base_url(), &dj.base_url, Edit::BaseUrl),
            self.setting(
                "API key",
                text_input("Stored in your config file", &dj.api_key)
                    .on_input(|s| Message::Edit(Edit::ApiKey(s)))
                    .secure(true)
                    .padding(6)
            ),
            self.field("…or key from env var", key_env, &dj.api_key_env, Edit::ApiKeyEnv),
            self.setting("What the DJ does", pick_list(DjMode::ALL, Some(dj.mode), |m| Message::Edit(Edit::Mode(m)))),
            self.field("DJ name", "Haz DJ", &dj.name, Edit::DjName),
            self.field("Personality", "Describe the DJ's style", &dj.persona, Edit::Persona),
            self.field("Language", "English", &dj.language, Edit::Language),
            self.number("Songs per set", dj.batch_size as f32, 1.0..=20.0, Edit::BatchSize),
            self.number("Refill when queue has", dj.refill_at as f32, 0.0..=10.0, Edit::RefillAt),
            self.number("Talk every Nth song", dj.talk_every as f32, 1.0..=10.0, Edit::TalkEvery),
            self.number("Max words per intro", dj.max_words as f32, 10.0..=120.0, Edit::MaxWords),
            self.number("Discovery (new vs. familiar)", dj.discovery as f32, 0.0..=100.0, Edit::Discovery),
            self.setting("Explicit songs", toggler(dj.allow_explicit).on_toggle(|v| Message::Edit(Edit::Explicit(v)))),
            section("DJ voice"),
            self.setting("Voice engine", pick_list(TtsEngine::ALL, Some(dj.tts.engine), |e| Message::Edit(Edit::TtsEngine(e)))),
            self.field("Voice", "default", &dj.tts.voice, Edit::Voice),
            self.field("TTS model", "gpt-4o-mini-tts", &dj.tts.model, Edit::TtsModel),
            self.number("Music volume while talking %", dj.tts.duck_to as f32, 0.0..=100.0, Edit::DuckTo),
            section("Appearance"),
            self.setting(
                "Theme",
                row![
                    pick_list(self.theme_names.clone(), Some(a.theme.clone()), |t| Message::Edit(Edit::Theme(t))),
                    self.small_button("Open folder", Message::OpenConfigFolder),
                ]
                .spacing(8)
                .align_y(Alignment::Center)
            ),
            self.field("Accent colour", "#ff5c8a", &a.colors.accent, Edit::Accent),
            self.setting(
                "Accent from cover art",
                toggler(a.accent_from_cover).on_toggle(|v| Message::Edit(Edit::AccentFromCover(v)))
            ),
            self.setting(
                "App icon",
                row![
                    image(self.icon.clone()).width(32).height(32),
                    self.small_button("Choose image…", Message::PickIcon),
                ]
                .push(if a.app_icon.is_empty() {
                    El::from(self.muted("Built-in"))
                } else {
                    self.small_button("Reset", Message::ResetIcon)
                })
                .spacing(8)
                .align_y(Alignment::Center)
            ),
            self.setting(
                "App font",
                row![
                    self.font_picker(&a.font_family, Edit::AppFont),
                    self.small_button("Add font file…", Message::PickFont),
                ]
                .spacing(8)
                .align_y(Alignment::Center)
            ),
            self.field("…or an installed font", "e.g. Inter", &a.font_family, Edit::AppFont),
            self.muted("The app font and text size apply after a restart. Lyrics fonts apply right away."),
            self.number("Text size", a.font_size, 10.0..=24.0, Edit::FontSize),
            self.number("Corner radius", a.corner_radius, 0.0..=20.0, Edit::Radius),
            self.setting("Compact rows", toggler(a.compact).on_toggle(|v| Message::Edit(Edit::Compact(v)))),
            self.setting("Album art", toggler(a.show_album_art).on_toggle(|v| Message::Edit(Edit::AlbumArt(v)))),
            section("Lyrics"),
            self.setting(
                "Word-by-word highlight",
                toggler(ly.word_highlight).on_toggle(|v| Message::Edit(Edit::WordHighlight(v)))
            ),
            self.setting(
                "Blurred cover background",
                toggler(ly.blurred_background).on_toggle(|v| Message::Edit(Edit::BlurredBackground(v)))
            ),
            self.setting("Alignment", pick_list(LyricsAlign::ALL, Some(ly.align), |v| Message::Edit(Edit::LyricsAlign(v)))),
            self.setting("Lyrics font", self.font_picker(&ly.font_family, Edit::LyricsFont)),
            self.number("Lyrics text size", ly.font_size, 16.0..=64.0, Edit::LyricsSize),
            self.number("Past lines shown", ly.lines_before as f32, 0.0..=6.0, Edit::LinesBefore),
            self.setting(
                "Timing offset (ms)",
                row![
                    slider(-3000.0..=3000.0, ly.offset_ms as f32, |v| Message::Edit(Edit::LyricsOffset(v)))
                        .step(50.0_f32)
                        .width(Fill),
                    text(format!("{:+}", ly.offset_ms)).width(56),
                ]
                .spacing(8)
                .align_y(Alignment::Center)
            ),
            section("Layout"),
            self.setting("Sidebar", pick_list(Side::ALL, Some(l.sidebar), |s| Message::Edit(Edit::Sidebar(s)))),
            self.setting("Queue panel", pick_list(Side::ALL, Some(l.side_panel), |s| Message::Edit(Edit::SidePanel(s)))),
            self.setting("Player bar", pick_list(Edge::ALL, Some(l.player_bar), |e| Message::Edit(Edit::PlayerBar(e)))),
        ]
        .spacing(10)
        .max_width(760);
        form = form.push(
            row![
                self.primary_button("Save settings", Some(Message::SaveSettings)),
                self.ghost("Log out of Spotify", Message::Logout, false),
            ]
            .spacing(12),
        );
        scrollable(container(form).padding(iced::Padding { top: 0.0, right: 20.0, bottom: 8.0, left: 8.0 }))
            .height(Fill)
            .into()
    }
}
