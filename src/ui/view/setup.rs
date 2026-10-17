//! The animated first-run setup: hello in a dozen languages, then a few short steps that each slide
//! in, like setting up a new phone.

use std::time::Instant;

use iced::widget::{Column, Row, Space, button, column, container, rich_text, row, span, stack, text, toggler};
use iced::{Alignment, Background, Border, Color, Fill, Font, Length, Padding, font};

use super::super::icon::{self, icon};
use super::super::{App, Edit, Login, Message, Setup, SetupStep, style};
use super::El;
use crate::config::{DjMode, LlmProvider};
use crate::theme::{self, PRESETS};

const GREETINGS: [&str; 12] =
    ["hello", "hola", "bonjour", "hallo", "ciao", "olá", "привет", "hej", "merhaba", "γεια σου", "salut", "ahoj"];
/// How long each greeting stays on screen, in seconds.
const GREETING_SECS: f32 = 2.4;
const DASHBOARD: &str = "https://developer.spotify.com/dashboard";

fn ease_out(t: f32) -> f32 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3)
}

/// Overshoots a little before settling, for things that pop in.
fn ease_back(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0) - 1.0;
    1.0 + 2.7 * t.powi(3) + 1.7 * t.powi(2)
}

/// 0 before `from` seconds, 1 after `from + over`, eased in between.
fn fade_in(t: f32, from: f32, over: f32) -> f32 {
    ease_out((t - from) / over)
}

fn since(at: Instant) -> f32 {
    at.elapsed().as_secs_f32()
}

/// "apple-music-dark" -> "Apple Music Dark".
fn pretty(name: &str) -> String {
    name.split('-')
        .map(|w| {
            let mut c = w.chars();
            c.next().map(|f| f.to_uppercase().chain(c).collect::<String>()).unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

impl App {
    pub(super) fn setup_view(&self, setup: &Setup) -> El<'_> {
        let t = since(setup.entered);
        let body = match setup.step {
            SetupStep::Hello => self.hello(t),
            SetupStep::Look => self.look_step(),
            SetupStep::Spotify => self.spotify_step(setup),
            SetupStep::Dj => self.dj_step(),
            SetupStep::Login => self.login_step(t),
            SetupStep::Done => self.done_step(t),
        };

        // Each step slides in from the side it comes from and fades up from the background.
        let e = ease_out(t / 0.55);
        let shift = (1.0 - e) * 80.0;
        let pad = if setup.backwards { Padding::ZERO.right(shift) } else { Padding::ZERO.left(shift) };
        let mut moving = stack![container(body).padding(pad).center(Fill)];
        if e < 1.0 {
            let veil = style::alpha(self.colors.background, 1.0 - e);
            moving = moving.push(container(Space::new()).width(Fill).height(Fill).style(move |_| container::Style {
                background: Some(Background::Color(veil)),
                ..container::Style::default()
            }));
        }
        container(column![moving.height(Fill), self.step_dots(setup.step, e)].spacing(16).align_x(Alignment::Center))
            .padding(32)
            .width(Fill)
            .height(Fill)
            .into()
    }

    /// One dot per step after the hello; the current one stretches into a pill.
    fn step_dots(&self, step: SetupStep, e: f32) -> El<'_> {
        if step == SetupStep::Hello {
            return Space::new().height(8).into();
        }
        let dots = SetupStep::ALL[1..].iter().map(|s| {
            let current = *s == step;
            let width = if current { 8.0 + 16.0 * e } else { 8.0 };
            let color = if current { self.colors.accent } else { style::tint(self.colors, 0.25) };
            container(Space::new()).width(width).height(8).style(style::tile(color, 4.0)).into()
        });
        Row::with_children(dots).spacing(8).align_y(Alignment::Center).into()
    }

    /// The main button of a step. `fade` runs from 0 (invisible) to 1.
    fn cta<'a>(&self, glyph: Option<char>, label: &'a str, msg: Option<Message>, fade: f32) -> El<'a> {
        let accent = self.colors.accent;
        let ink = style::alpha(style::readable_on(accent), fade);
        let mut content = Row::new().spacing(10).align_y(Alignment::Center);
        content = content.push(
            text(label).size(self.fs(1.05)).font(self.weight(font::Weight::Semibold)).color(ink).wrapping(text::Wrapping::None),
        );
        if let Some(g) = glyph {
            content = content.push(icon(g, self.fs(1.05)).color(ink));
        }
        let surface = self.colors.surface;
        button(content)
            .padding([12, 28])
            .style(move |_, status| {
                let bg = match status {
                    button::Status::Hovered => style::mix(accent, Color::WHITE, 0.12),
                    button::Status::Pressed => style::mix(accent, Color::BLACK, 0.12),
                    button::Status::Disabled => style::mix(accent, surface, 0.6),
                    button::Status::Active => accent,
                };
                button::Style {
                    background: Some(Background::Color(style::alpha(bg, fade))),
                    border: Border { radius: 999.0.into(), ..Border::default() },
                    ..button::Style::default()
                }
            })
            .on_press_maybe(msg.filter(|_| fade > 0.3))
            .into()
    }

    /// The icon, title and subtitle at the top of a step.
    fn step_header<'a>(&self, glyph: char, title: &'a str, subtitle: impl text::IntoFragment<'a>) -> El<'a> {
        let badge = container(icon(glyph, 30.0).color(style::readable_on(self.colors.accent)))
            .center(64)
            .style(style::tile(self.colors.accent, 20.0));
        column![
            badge,
            self.heading(title, 2.2),
            text(subtitle).size(self.fs(1.0)).color(self.colors.muted_text).align_x(Alignment::Center),
        ]
        .spacing(12)
        .align_x(Alignment::Center)
        .into()
    }

    /// A step's layout. The buttons stay at the bottom; the rest scrolls when the window is short.
    fn step<'a>(&self, header: El<'a>, body: El<'a>, actions: El<'a>, width: f32) -> El<'a> {
        let content = column![header, body].spacing(28).align_x(Alignment::Center).max_width(width);
        let scrolled = self.scroll(container(content).padding([0.0, 14.0]).center_x(Fill));
        column![
            container(scrolled).center_x(Fill).center_y(Fill),
            container(actions).max_width(width + 28.0).padding([0.0, 14.0]),
        ]
        .spacing(20)
        .align_x(Alignment::Center)
        .height(Fill)
        .into()
    }

    fn back_button(&self) -> El<'_> {
        self.small_button(Some(icon::CHEVRON_LEFT), "Back", Message::SetupBack)
    }

    fn hello(&self, t: f32) -> El<'_> {
        let cycle = t / GREETING_SECS;
        let word = GREETINGS[cycle as usize % GREETINGS.len()];
        let local = cycle.fract() * GREETING_SECS;
        let leaving = 1.0 - ((local - (GREETING_SECS - 0.45)) / 0.45).clamp(0.0, 1.0);
        let dark = style::readable_on(self.colors.background) == Color::WHITE;
        // Letters arrive one after another, in colours that drift along the word over time.
        let spans: Vec<text::Span<'_, (), Font>> = word
            .chars()
            .enumerate()
            .map(|(i, c)| {
                let a = fade_in(local, i as f32 * 0.06, 0.4) * leaving;
                let (r, g, b) = style::hsl((i as f32 * 26.0 + t * 50.0).rem_euclid(360.0), 0.85, if dark { 0.68 } else { 0.5 });
                span(c.to_string()).color(Color::from_rgba(r, g, b, a))
            })
            .collect();
        let size = self.fs(6.5);
        let rise = (1.0 - ease_out(local / 0.7)) * 28.0;
        let greeting = container(
            rich_text(spans).size(size).font(Font { weight: font::Weight::ExtraBold, ..Font::with_name("Inter Display") }),
        )
        .padding(Padding::ZERO.top(rise))
        .height(size * 1.4 + 28.0)
        .center_x(Fill);

        let welcome = text("Welcome to Haze")
            .size(self.fs(1.2))
            .color(style::alpha(self.colors.muted_text, fade_in(t, 0.9, 0.8)));
        column![
            greeting,
            welcome,
            Space::new().height(36),
            self.cta(Some(icon::ARROW_RIGHT), "Get started", Some(Message::SetupNext), fade_in(t, 1.5, 0.8)),
        ]
        .spacing(8)
        .align_x(Alignment::Center)
        .into()
    }

    fn look_step(&self) -> El<'_> {
        let tiles: Vec<El> = PRESETS.iter().map(|name| self.theme_tile(name)).collect();
        let mut grid = Column::new().spacing(12);
        let mut tiles = tiles.into_iter().peekable();
        while tiles.peek().is_some() {
            grid = grid.push(Row::with_children(tiles.by_ref().take(3)).spacing(12));
        }
        let accent = row![
            text("Accent from cover art").size(self.fs(0.95)).width(Fill),
            toggler(self.config.appearance.accent_from_cover)
                .on_toggle(|v| Message::Edit(Edit::AccentFromCover(v)))
                .size(20),
        ]
        .align_y(Alignment::Center);
        self.step(
            self.step_header(icon::PALETTE, "Choose a look", "Pick a theme. You can change it, and much more, in Settings."),
            column![grid, accent].spacing(18).into(),
            row![self.back_button(), Space::new().width(Fill), self.cta(None, "Continue", Some(Message::SetupNext), 1.0)]
                .align_y(Alignment::Center)
                .into(),
            640.0,
        )
    }

    /// A small preview of a theme: a sidebar, a few rows and an accent pill in its own colours.
    fn theme_tile(&self, name: &'static str) -> El<'_> {
        let c = theme::preset(name);
        let selected = self.config.appearance.theme.eq_ignore_ascii_case(name);
        let bar = |width: Length, color: Color| container(Space::new()).width(width).height(6).style(style::tile(color, 3.0));
        let lines = column![
            bar(Length::FillPortion(3), style::mix(c.surface, c.text, 0.35)),
            bar(Fill, style::mix(c.surface, c.text, 0.15)),
            bar(Fill, style::mix(c.surface, c.text, 0.15)),
            container(Space::new()).width(26).height(10).style(style::tile(c.accent, 5.0)),
        ]
        .spacing(6);
        let preview = container(
            row![
                container(Space::new()).width(18).height(Fill).style(style::tile(c.surface, 4.0)),
                container(lines).padding(8).width(Fill).height(Fill).style(style::tile(c.surface, 4.0)),
            ]
            .spacing(4),
        )
        .padding(4)
        .width(Fill)
        .height(74)
        .style(style::tile(c.background, self.r()));
        let colors = self.colors;
        let radius = self.r() * 1.2;
        button(column![preview, text(pretty(name)).size(self.fs(0.88)).font(self.weight(font::Weight::Semibold))].spacing(8))
            .padding(6)
            .width(Fill)
            .style(move |_, status| {
                let border = if selected {
                    Border { color: colors.accent, width: 2.0, radius: radius.into() }
                } else {
                    let t = if status == button::Status::Hovered { 0.3 } else { 0.12 };
                    Border { color: style::tint(colors, t), width: 1.0, radius: radius.into() }
                };
                button::Style { background: None, text_color: colors.text, border, ..button::Style::default() }
            })
            .on_press(Message::Edit(Edit::Theme(name.to_string())))
            .into()
    }

    /// A numbered instruction with an optional control under it.
    fn numbered<'a>(&self, n: u8, line: &'a str, extra: Option<El<'a>>) -> El<'a> {
        let badge = container(text(n.to_string()).size(self.fs(0.85)).font(self.weight(font::Weight::Bold)).color(style::readable_on(self.colors.accent)))
            .center(26)
            .style(style::tile(self.colors.accent, 13.0));
        let mut body = Column::new().spacing(10).push(text(line).size(self.fs(0.98)));
        if let Some(extra) = extra {
            body = body.push(extra);
        }
        row![badge, body.width(Fill)].spacing(14).into()
    }

    fn spotify_step(&self, setup: &Setup) -> El<'_> {
        let copied = setup.copied.is_some_and(|t| since(t) < 2.0);
        let uri = container(
            row![
                text(&self.config.spotify.redirect_uri).size(self.fs(0.95)).font(Font::MONOSPACE).width(Fill),
                self.small_button(
                    Some(if copied { icon::CHECK } else { icon::COPY }),
                    if copied { "Copied" } else { "Copy" },
                    Message::CopyRedirectUri
                ),
            ]
            .align_y(Alignment::Center),
        )
        .padding(Padding::new(6.0).left(14.0))
        .style(style::tile(style::tint(self.colors, 0.08), self.r()));
        let steps = column![
            self.numbered(
                1,
                "Create an app on the Spotify dashboard. Any name and description will do.",
                Some(self.small_button(Some(icon::EXTERNAL_LINK), "Open the dashboard", Message::OpenUrl(DASHBOARD))),
            ),
            self.numbered(2, "Under Redirect URIs, add this address exactly:", Some(uri.into())),
            self.numbered(3, "Tick Web API and Web Playback SDK, save, and copy the Client ID from the app's Settings.", None),
        ]
        .spacing(18);
        let card = container(steps).padding(20).width(Fill).style(style::card(self.colors, self.panel_r()));
        let input = self
            .input("Paste your Client ID", &self.client_id_input)
            .on_input(Message::ClientIdChanged)
            .on_submit(Message::SetupNext)
            .padding([12, 16]);
        let ready = !self.client_id_input.trim().is_empty();
        self.step(
            self.step_header(
                icon::MUSIC,
                "Connect Spotify",
                "Haze plays through your own free Spotify developer app, so your login stays between you and Spotify.",
            ),
            column![card, input, self.muted("Playing music needs Spotify Premium.")].spacing(14).align_x(Alignment::Center).into(),
            row![
                self.back_button(),
                Space::new().width(Fill),
                self.cta(None, "Continue", ready.then_some(Message::SetupNext), 1.0)
            ]
            .align_y(Alignment::Center)
            .into(),
            600.0,
        )
    }

    fn dj_step(&self) -> El<'_> {
        let dj = &self.config.dj;
        let key = column![
            self.input("API key", &dj.api_key).on_input(|s| Message::Edit(Edit::ApiKey(s))).secure(true).padding([12, 16]),
            self.muted(format!(
                "Kept in your config file. Or leave it empty and set {} instead.",
                dj.provider.default_key_env()
            ))
            .size(self.fs(0.82)),
        ]
        .spacing(6);
        let form = column![
            self.setting("Provider", self.picker(LlmProvider::ALL, Some(dj.provider), |p| Message::Edit(Edit::Provider(p)))),
            self.setting("What the DJ does", self.picker(DjMode::ALL, Some(dj.mode), |m| Message::Edit(Edit::Mode(m)))),
            key,
        ]
        .spacing(14);
        let card = container(form).padding(20).width(Fill).style(style::card(self.colors, self.panel_r()));
        self.step(
            self.step_header(
                icon::SPARKLES,
                "Meet your DJ",
                "With a key from an AI provider, the DJ picks songs for you and can talk between them. You can skip this.",
            ),
            card.into(),
            row![
                self.back_button(),
                Space::new().width(Fill),
                self.small_button(None, "Set up later", Message::SetupNext),
                self.cta(None, "Continue", Some(Message::SetupNext), 1.0),
            ]
            .spacing(10)
            .align_y(Alignment::Center)
            .into(),
            560.0,
        )
    }

    fn login_step(&self, t: f32) -> El<'_> {
        let waiting = self.login == Login::InProgress;
        let body: El = if waiting {
            // Three dots that breathe in turn.
            let dots = (0..3).map(|i| {
                let wave = ((t * 4.0 - i as f32 * 0.7).sin() + 1.0) / 2.0;
                let size = 10.0 + 4.0 * wave;
                container(container(Space::new()).width(size).height(size).style(style::tile(style::alpha(self.colors.accent, 0.35 + 0.65 * wave), size / 2.0)))
                    .center(16)
                    .into()
            });
            column![
                Row::with_children(dots).spacing(6),
                self.muted("Waiting for your browser…"),
            ]
            .spacing(14)
            .align_x(Alignment::Center)
            .into()
        } else {
            self.cta(Some(icon::LOG_IN), "Log in with Spotify", Some(Message::SetupNext), 1.0)
        };
        self.step(
            self.step_header(
                icon::LOG_IN,
                "Log in",
                "Your browser opens Spotify's login page. Approve Haze there, then come back here.",
            ),
            body,
            row![self.back_button(), Space::new().width(Fill)].into(),
            520.0,
        )
    }

    fn done_step(&self, t: f32) -> El<'_> {
        let pop = ease_back(t / 0.6);
        let size = 96.0 * pop.max(0.0);
        let check = container(icon(icon::CHECK, 48.0 * pop.max(0.0)).color(style::readable_on(self.colors.accent)))
            .center(size)
            .style(style::tile(self.colors.accent, size / 2.0));
        // A ring that spreads out and fades, twice.
        let ring_t = (t * 0.8).fract();
        let ring_size = 96.0 + 90.0 * ease_out(ring_t);
        let ring_alpha = if t < 2.5 { 0.35 * (1.0 - ring_t) } else { 0.0 };
        let ring = container(Space::new())
            .width(ring_size)
            .height(ring_size)
            .style(style::tile(style::alpha(self.colors.accent, ring_alpha), ring_size / 2.0));
        let badge = stack![container(ring).center(190), container(check).center(190)];

        let title_fade = fade_in(t, 0.3, 0.6);
        let name = self.user_name.trim();
        let welcome =
            if name.is_empty() { "Welcome to Haze.".to_string() } else { format!("Welcome to Haze, {name}.") };
        column![
            badge,
            self.heading("You're all set", 2.4).color(style::alpha(self.colors.text, title_fade)),
            text(welcome).size(self.fs(1.1)).color(style::alpha(self.colors.muted_text, title_fade)),
            Space::new().height(24),
            self.cta(Some(icon::ARROW_RIGHT), "Start listening", Some(Message::SetupNext), fade_in(t, 0.8, 0.6)),
        ]
        .spacing(10)
        .align_x(Alignment::Center)
        .into()
    }
}
