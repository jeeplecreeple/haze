//! The Now Playing page: the cover inside a ring that moves with the music, big wavy controls,
//! and the lyrics beside it. Everything here follows `[player]` in the config.

use std::f32::consts::{FRAC_PI_2, TAU};

use iced::widget::canvas::{self, Canvas, Frame, Geometry, Path, Stroke, stroke};
use iced::widget::{Column, Row, Space, button, column, container, image, responsive, row, stack, text};
use iced::{Alignment, Color, ContentFit, Fill, Length, Padding, Point, Radians, Rectangle, Renderer, Theme, Vector, mouse};

use super::super::icon::{self, icon};
use super::super::shape::Outline;
use super::super::{App, Message, SettingsTab, style};
use super::{El, mmss};
use crate::config::{LyricsAlign, PlayerLayout, Progress, Side, TextAlign, Visualizer, VisualizerColor};
use crate::queue::Repeat;

impl App {
    pub(in crate::ui) fn cover_outline(&self) -> Outline {
        let p = &self.config.player;
        Outline { shape: p.cover_shape, waves: p.wave_count, depth: p.wave_depth / 100.0 }
    }

    fn play_outline(&self) -> Outline {
        let p = &self.config.player;
        Outline { shape: p.play_shape, waves: (p.wave_count * 2 / 3).max(5), depth: p.wave_depth / 100.0 }
    }

    /// The colour of each visualizer bar.
    fn bar_colors(&self, count: usize) -> Vec<Color> {
        let p = &self.config.player;
        let cover = self.cover_accent.unwrap_or(self.colors.accent);
        let one = match p.visualizer_color {
            VisualizerColor::Accent => self.colors.accent,
            VisualizerColor::Cover => cover,
            VisualizerColor::Text => self.colors.text,
            VisualizerColor::Custom => crate::theme::parse_hex(&p.visualizer_custom).unwrap_or(self.colors.accent),
            VisualizerColor::Rainbow => {
                return (0..count)
                    .map(|i| {
                        let (r, g, b) = style::hsl(i as f32 / count as f32 * 360.0, 0.85, 0.65);
                        Color::from_rgb(r, g, b)
                    })
                    .collect();
            }
            VisualizerColor::Gradient => {
                let from = crate::theme::parse_hex(&p.visualizer_custom).unwrap_or(self.colors.accent);
                let to = crate::theme::parse_hex(&p.visualizer_custom_end).unwrap_or(cover);
                // A mirrored ring runs from the bottom to the top on both sides, so the colours do too.
                let mirrored = p.visualizer_mirror && p.visualizer.is_ring();
                let half = (count / 2).max(1);
                return (0..count)
                    .map(|i| {
                        let t = if mirrored { i.min(count - i) as f32 / half as f32 } else { i as f32 / (count - 1).max(1) as f32 };
                        mix(from, to, t.clamp(0.0, 1.0))
                    })
                    .collect();
            }
        };
        vec![one; count]
    }

    /// The colour of the cover's edge and the seek ring: the visualizer's colour where it has one.
    fn edge_color(&self) -> Color {
        match self.config.player.visualizer_color {
            VisualizerColor::Rainbow => self.colors.accent,
            VisualizerColor::Gradient => self.bar_colors(2)[0],
            _ => self.bar_colors(1)[0],
        }
    }

    /// How hard the bass hits right now, 0-1 (what the cover pulses to).
    fn beat(&self) -> f32 {
        let lows = (self.viz.len() / 8).max(2).min(self.viz.len());
        if lows == 0 { 0.0 } else { self.viz[..lows].iter().sum::<f32>() / lows as f32 }
    }

    /// How many frequency bands the visualizer asks for: a mirrored ring shows each one twice.
    pub(in crate::ui) fn visualizer_bands(&self) -> usize {
        let p = &self.config.player;
        let bars = p.visualizer_bars.clamp(8, 256);
        if p.visualizer_mirror && p.visualizer.is_ring() { bars / 2 + 1 } else { bars }
    }

    pub(super) fn player_page(&self) -> El<'_> {
        let p = &self.config.player;
        let current = self.queue.current().map(|e| &e.track);
        let show_lyrics = self.player_lyrics && p.layout != PlayerLayout::Centered;
        responsive(move |size| {
            let wide = size.width >= 820.0;
            let side_by_side = show_lyrics && wide && p.layout == PlayerLayout::Split;
            let column_w = if side_by_side { (size.width * 0.44).min(560.0) } else { size.width.min(620.0) };
            let compact = show_lyrics && !side_by_side;
            let auto = if compact {
                (size.height * 0.22).clamp(96.0, 200.0)
            } else {
                // Whatever height the song, controls and ring leave over goes to the cover.
                let ring = p.visualizer.is_ring();
                let ring_scale = (if ring { 1.0 + 2.0 * p.visualizer_height / 100.0 } else { 1.0 }) * (1.0 + PULSE * p.pulse / 100.0);
                let ring_pad = if ring { p.bar_width * 4.0 + 20.0 } else { 0.0 } + 2.0 * self.orbit_width();
                let under = if p.visualizer.is_under() { 1.0 + p.visualizer_height / 100.0 } else { 1.0 };
                let rest = 56.0
                    + 20.0
                    + p.title_size * (if p.show_album { 2.6 } else { 2.0 })
                    + match p.progress {
                        Progress::Hidden => 0.0,
                        Progress::Ring if !p.show_times => 0.0,
                        Progress::Ring => 20.0,
                        _ => 34.0,
                    }
                    + p.play_size.max(p.control_size) + 14.0
                    + if p.show_volume { 32.0 } else { 0.0 }
                    + if p.show_extras { p.control_size + 30.0 } else { 0.0 }
                    + 14.0 * 3.0;
                let fits = (size.height - rest - ring_pad) / (ring_scale * under);
                (column_w * 0.6).min(fits).clamp(120.0, 440.0)
            };
            let cover = if p.cover_size > 0.0 { p.cover_size } else { auto };
            let player = self.player_column(cover, column_w, compact);

            let body: El = if !show_lyrics {
                container(player).center(Fill).into()
            } else {
                let lyrics_w = if side_by_side { size.width - column_w - 120.0 } else { size.width - 80.0 };
                let lyrics = container(self.player_lyrics_view(lyrics_w)).width(Fill).height(Fill);
                if side_by_side {
                    let player = container(player).width(column_w).center_y(Fill);
                    match p.lyrics_side {
                        Side::Right => row![player, lyrics].spacing(48).into(),
                        Side::Left => row![lyrics, player].spacing(48).into(),
                    }
                } else {
                    column![container(player).center_x(Fill), lyrics].spacing(16).into()
                }
            };
            let _ = current;
            stack![
                container(body).width(Fill).height(Fill).padding(Padding::from([28, 40])),
                container(self.player_toolbar()).width(Fill).align_x(Alignment::End).padding(12),
            ]
            .into()
        })
        .into()
    }

    fn player_toolbar(&self) -> El<'_> {
        let s = self.fs(1.05);
        let lyrics = if self.config.player.layout == PlayerLayout::Centered {
            None
        } else {
            Some(self.icon_button(icon::MIC_VOCAL, s, Some(Message::TogglePlayerLyrics), self.player_lyrics))
        };
        let mut bar = Row::new().spacing(2).align_y(Alignment::Center);
        if let Some(l) = lyrics {
            bar = bar.push(l);
        }
        bar = bar
            .push(self.icon_button(icon::SLIDERS_HORIZONTAL, s, Some(Message::OpenSettingsTab(SettingsTab::Player)), false))
            .push(self.icon_button(
                if self.fullscreen { icon::MINIMIZE_2 } else { icon::MAXIMIZE_2 },
                s,
                Some(Message::ToggleFullscreen),
                self.fullscreen,
            ))
            .push(self.icon_button(icon::X, s, Some(Message::TogglePlayer), false));
        container(bar).padding([3, 6]).style(style::glass(self.colors, 999.0, 0.55)).into()
    }

    fn player_column(&self, cover: f32, width: f32, compact: bool) -> El<'_> {
        let p = &self.config.player;
        let current = self.queue.current().map(|e| &e.track);
        let align = if p.align == TextAlign::Center && !compact { Alignment::Center } else { Alignment::Start };
        let text_align = if align == Alignment::Center { iced::alignment::Horizontal::Center } else { iced::alignment::Horizontal::Left };

        let title = self
            .heading(current.map(|t| t.name.clone()).unwrap_or_else(|| "Nothing playing".into()), p.title_size / self.fs(1.0))
            .align_x(text_align)
            .width(Fill);
        let artist = text(current.map(|t| t.artist_line()).unwrap_or_default())
            .size(p.title_size * 0.62)
            .color(style::alpha(self.colors.text, 0.75))
            .align_x(text_align)
            .width(Fill);
        let mut info = column![title, artist].spacing(4).width(Fill);
        if p.show_album
            && let Some(album) = current.map(|t| t.album.clone()).filter(|a| !a.is_empty())
        {
            info = info.push(text(album).size(p.title_size * 0.46).color(self.colors.muted_text).align_x(text_align).width(Fill));
        }

        let mut col = Column::new().spacing(14).align_x(align).width(width);
        if compact {
            // A row: small cover beside the song, controls underneath.
            col = col.push(row![self.halo(cover), container(info).center_y(Length::Shrink)].spacing(20).align_y(Alignment::Center));
        } else {
            col = col.push(container(self.halo(cover)).center_x(Fill));
            if p.visualizer.is_under() {
                col = col.push(self.under_visualizer(width.min(cover * 1.4), (cover * p.visualizer_height / 100.0).max(16.0)));
            }
            col = col.push(Space::new().height(6)).push(info);
        }
        match p.progress {
            Progress::Hidden => {}
            Progress::Ring if !p.show_times => {}
            Progress::Ring => col = col.push(self.ring_times(align)),
            _ => col = col.push(self.player_progress()),
        }
        col = col.push(self.player_controls());
        if p.show_volume {
            col = col.push(self.player_volume());
        }
        if p.show_extras {
            let s = p.control_size * 0.85;
            col = col.push(
                container(
                    row![
                        self.icon_button(icon::LIST_MUSIC, s, Some(Message::ToggleSidePanel), self.show_side_panel),
                        self.icon_button(icon::SPARKLES, s, Some(if self.dj_on { Message::DjStop } else { Message::DjStart }), self.dj_on),
                        self.icon_button(icon::MIC_VOCAL, s, Some(Message::ToggleLyrics), false),
                    ]
                    .spacing(8),
                )
                .center_x(Fill),
            );
        }
        col.into()
    }

    /// Room outside the cover for its edge and the seek ring.
    fn orbit_width(&self) -> f32 {
        let p = &self.config.player;
        let border = p.cover_border.max(0.0);
        let ring = if p.progress == Progress::Ring { ORBIT_GAP + ORBIT_WIDTH + 4.0 } else { 0.0 };
        if border + ring > 0.0 { border + ring + 2.0 } else { 0.0 }
    }

    /// The cover, with the ring visualizer around it, its edge and the seek ring.
    fn halo(&self, cover: f32) -> El<'_> {
        let p = &self.config.player;
        let ring = p.visualizer.is_ring();
        let orbit = self.orbit_width();
        // The box is sized for the biggest beat, so the page doesn't shuffle while the cover pulses.
        let grow = PULSE * p.pulse.clamp(0.0, 100.0) / 100.0;
        let room = cover * (1.0 + grow);
        let shown = cover * (1.0 + grow * self.beat());
        let reach = if ring { cover * p.visualizer_height / 100.0 } else { 0.0 };
        let pad = orbit + if ring { reach + p.bar_width * 2.0 + 10.0 } else { 0.0 };
        let side = room + pad * 2.0;
        let bands = self.visualizer_bands();
        let art: El = match &self.player_cover {
            Some((_, handle)) if self.config.appearance.show_album_art => image(handle.clone())
                .width(shown)
                .height(shown)
                .content_fit(ContentFit::Fill)
                .filter_method(image::FilterMethod::Linear)
                .rotation(iced::Rotation::Floating(Radians(self.spin)))
                .into(),
            _ => Canvas::new(Blob {
                outline: self.cover_outline(),
                color: style::tint(self.colors, 0.12),
                turn: self.spin,
                glyph: None,
            })
            .width(shown)
            .height(shown)
            .into(),
        };
        let mut layers = stack![];
        if ring {
            layers = layers.push(
                Canvas::new(Halo {
                    levels: self.levels(bands),
                    colors: self.bar_colors(p.visualizer_bars.clamp(8, 256)),
                    outline: self.cover_outline(),
                    cover: shown,
                    reach,
                    turn: self.spin,
                    look: p.visualizer,
                    mirror: p.visualizer_mirror,
                    width: p.bar_width,
                    glow: p.visualizer_glow,
                    bars: p.visualizer_bars.clamp(8, 256),
                    gap: 6.0 + orbit,
                })
                .width(side)
                .height(side),
            );
        }
        layers = layers.push(container(art).center(side));
        if orbit > 0.0 {
            let duration = self.queue.current().map(|e| e.track.duration_ms).unwrap_or(0) as f32;
            layers = layers.push(
                Canvas::new(Orbit {
                    outline: self.cover_outline(),
                    cover: shown,
                    turn: self.spin,
                    border: p.cover_border.max(0.0),
                    color: self.edge_color(),
                    seek: (p.progress == Progress::Ring).then(|| {
                        (self.seek_preview.unwrap_or(self.live_position() as f32).min(duration.max(1.0)), duration.max(1.0))
                    }),
                    rest: style::alpha(self.colors.text, 0.18),
                })
                .width(side)
                .height(side),
            );
        }
        layers.into()
    }

    /// "1:23 / 3:45" under the song, for the seek ring.
    fn ring_times(&self, align: Alignment) -> El<'_> {
        let duration = self.queue.current().map(|e| e.track.duration_ms).unwrap_or(0);
        let position = self.seek_preview.map(|v| v as u32).unwrap_or(self.live_position()).min(duration);
        container(text(format!("{} / {}", mmss(position), mmss(duration))).size(self.fs(0.85)).color(self.colors.muted_text))
            .width(Fill)
            .align_x(align)
            .into()
    }

    fn levels(&self, bands: usize) -> Vec<f32> {
        if self.viz.len() == bands { self.viz.clone() } else { vec![0.0; bands] }
    }

    fn under_visualizer(&self, width: f32, height: f32) -> El<'_> {
        let p = &self.config.player;
        let bars = p.visualizer_bars.clamp(8, 256);
        container(
            Canvas::new(Under {
                levels: self.levels(self.visualizer_bands()),
                colors: self.bar_colors(bars),
                wave: p.visualizer == Visualizer::Wave,
                mirror: p.visualizer_mirror,
                width: p.bar_width,
                glow: p.visualizer_glow,
            })
            .width(width)
            .height(height),
        )
        .center_x(Fill)
        .into()
    }

    fn player_progress(&self) -> El<'_> {
        let p = &self.config.player;
        let current = self.queue.current().map(|e| &e.track);
        let duration = current.map(|t| t.duration_ms).unwrap_or(0) as f32;
        let position = self.seek_preview.unwrap_or(self.live_position() as f32).min(duration.max(1.0));
        let track = Canvas::new(Track {
            value: position,
            max: duration.max(1.0),
            style: p.progress,
            played: self.colors.text,
            rest: style::alpha(self.colors.text, 0.22),
            phase: self.live_position() as f32 / 1000.0 * TAU * 0.6,
            moving: self.playing,
            on_change: Message::SeekPreview,
            on_release: Message::SeekCommit,
        })
        .width(Fill)
        .height(22);
        if !p.show_times {
            return track.into();
        }
        let time = |s: String| text(s).size(self.fs(0.78)).color(self.colors.muted_text).width(44);
        row![
            time(mmss(position as u32)),
            track,
            time(format!("-{}", mmss((duration - position).max(0.0) as u32))).align_x(iced::alignment::Horizontal::Right),
        ]
        .spacing(10)
        .align_y(Alignment::Center)
        .into()
    }

    fn player_controls(&self) -> El<'_> {
        let p = &self.config.player;
        let s = p.control_size;
        let play_size = p.play_size.max(32.0);
        let glyph = if self.playing { icon::PAUSE } else { icon::PLAY };
        let fill = self.colors.text;
        let play = button(stack![
            Canvas::new(Blob { outline: self.play_outline(), color: fill, turn: self.play_turn, glyph: None })
                .width(play_size)
                .height(play_size),
            container(icon(glyph, play_size * 0.36).color(style::readable_on(fill))).center(play_size),
        ])
        .padding(0)
        .style(style::bare())
        .on_press_maybe(self.has_audio().then_some(Message::TogglePlay));
        let repeat = if self.queue.repeat == Repeat::One { icon::REPEAT_1 } else { icon::REPEAT };
        let mut controls = Row::new().spacing(s * 1.1).align_y(Alignment::Center);
        if p.show_shuffle_repeat {
            controls = controls.push(self.icon_button(icon::SHUFFLE, s * 0.85, Some(Message::ToggleShuffle), self.queue.shuffle));
        }
        controls = controls
            .push(self.icon_button(icon::SKIP_BACK, s, Some(Message::Previous), false))
            .push(play)
            .push(self.icon_button(icon::SKIP_FORWARD, s, Some(Message::Next), false));
        if p.show_shuffle_repeat {
            controls = controls.push(self.icon_button(repeat, s * 0.85, Some(Message::CycleRepeat), self.queue.repeat != Repeat::Off));
        }
        let align = if p.align == TextAlign::Center { Alignment::Center } else { Alignment::Start };
        container(controls).width(Fill).align_x(align).into()
    }

    fn player_volume(&self) -> El<'_> {
        let glyph = match self.volume {
            0 => icon::VOLUME_X,
            1..=49 => icon::VOLUME_1,
            _ => icon::VOLUME_2,
        };
        row![
            icon(glyph, self.fs(1.0)).color(self.colors.muted_text),
            Canvas::new(Track {
                value: self.volume as f32,
                max: 100.0,
                style: Progress::Line,
                played: self.colors.text,
                rest: style::alpha(self.colors.text, 0.22),
                phase: 0.0,
                moving: false,
                on_change: Message::Volume,
                on_release: Message::VolumeCommit,
            })
            .width(Fill)
            .height(18),
        ]
        .spacing(12)
        .align_y(Alignment::Center)
        .into()
    }

    fn player_lyrics_view(&self, width: f32) -> El<'_> {
        let current = self.queue.current().map(|e| &e.track);
        let lyrics = self.lyrics.as_ref().filter(|l| current.is_some_and(|t| t.id == l.track_id));
        let align = match self.config.lyrics.align {
            LyricsAlign::Left => Alignment::Start,
            LyricsAlign::Center => Alignment::Center,
        };
        match (current, lyrics) {
            (None, _) => self.lyrics_message("Play a song to see its lyrics here."),
            (Some(_), None) if self.lyrics_loading => self.lyrics_message("Looking for lyrics…"),
            (Some(_), None) => self.lyrics_message("No lyrics for this song."),
            (Some(_), Some(l)) if l.synced => self.synced_lyrics(l, align, width, self.config.player.lyrics_card),
            (Some(_), Some(l)) => self.plain_lyrics(l, align),
        }
    }
}

/// Wider, fainter copies drawn under the bars, so they glow softly.
const GLOW: &[(f32, f32)] = &[(4.2, 0.035), (3.0, 0.05), (2.0, 0.08), (1.0, 1.0)];

/// A filled outline (cookie, circle...): the play button, or the cover's stand-in.
struct Blob {
    outline: Outline,
    color: Color,
    turn: f32,
    glyph: Option<char>,
}

impl<M> canvas::Program<M> for Blob {
    type State = ();

    fn draw(&self, _: &(), renderer: &Renderer, _: &Theme, bounds: Rectangle, _: mouse::Cursor) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let half = bounds.width.min(bounds.height) / 2.0;
        frame.fill(&self.outline.path(frame.center(), half, self.turn), self.color);
        let _ = self.glyph;
        vec![frame.into_geometry()]
    }
}

/// The ring of bars (or a wave) around the cover, hugging its outline.
struct Halo {
    levels: Vec<f32>,
    colors: Vec<Color>,
    outline: Outline,
    cover: f32,
    reach: f32,
    turn: f32,
    look: Visualizer,
    mirror: bool,
    width: f32,
    glow: bool,
    bars: usize,
    /// Space between the cover and the bars.
    gap: f32,
}

impl Halo {
    /// The level for bar `i`: lows at the bottom, highs at the top, mirrored left and right.
    fn level(&self, i: usize) -> f32 {
        let n = self.levels.len().max(1);
        let band = if self.mirror {
            let half = self.bars / 2;
            let j = if i <= half { i } else { self.bars - i };
            j * (n - 1) / half.max(1)
        } else {
            i * n / self.bars
        };
        self.levels.get(band.min(n - 1)).copied().unwrap_or(0.0)
    }
}

impl<M> canvas::Program<M> for Halo {
    type State = ();

    fn draw(&self, _: &(), renderer: &Renderer, _: &Theme, bounds: Rectangle, _: mouse::Cursor) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let c = frame.center();
        let gap = self.gap;
        let at = |a: f32, r: f32| c + Vector::new(a.cos() * r, a.sin() * r);
        let edge = |a: f32| self.outline.radius(a - self.turn) * self.cover / 2.0 + gap;
        let angle = |i: usize| FRAC_PI_2 + i as f32 / self.bars as f32 * TAU;
        if self.look == Visualizer::Dots {
            // A faint dot where each bar starts, and a bright one riding out with the music.
            for i in 0..self.bars {
                let (a, level) = (angle(i), self.level(i));
                let color = self.colors[i % self.colors.len()];
                let r0 = edge(a) + self.width * 0.5;
                frame.fill(&Path::circle(at(a, r0), self.width * 0.32), style::alpha(color, 0.35));
                let tip = at(a, r0 + 2.0 + level * self.reach);
                let size = self.width * (0.45 + 0.4 * level);
                if self.glow {
                    frame.fill(&Path::circle(tip, size * 2.2), style::alpha(color, 0.08 + 0.1 * level));
                }
                frame.fill(&Path::circle(tip, size), style::alpha(color, 0.55 + 0.45 * level));
            }
            return vec![frame.into_geometry()];
        }
        if self.look == Visualizer::Spikes {
            // A pointed triangle per bar, each as wide as its slice of the ring.
            let half_step = TAU / self.bars as f32 * 0.42;
            for i in 0..self.bars {
                let (a, level) = (angle(i), self.level(i));
                let color = self.colors[i % self.colors.len()];
                let spike = |reach: f32| {
                    Path::new(|p| {
                        p.move_to(at(a - half_step, edge(a - half_step)));
                        p.line_to(at(a, edge(a) + 3.0 + level * reach));
                        p.line_to(at(a + half_step, edge(a + half_step)));
                        p.close();
                    })
                };
                if self.glow {
                    frame.fill(&spike(self.reach * 1.25 + self.width), style::alpha(color, 0.1 + 0.08 * level));
                }
                frame.fill(&spike(self.reach), style::alpha(color, 0.5 + 0.5 * level));
            }
            return vec![frame.into_geometry()];
        }
        if self.look == Visualizer::RingWave {
            let points: Vec<Point> = (0..self.bars)
                .map(|i| {
                    let a = angle(i);
                    at(a, edge(a) + 2.0 + self.level(i) * self.reach)
                })
                .collect();
            let path = smooth_closed(&points);
            let color = self.colors[0];
            frame.fill(&path, style::alpha(color, 0.22));
            if self.glow {
                frame.stroke(&path, Stroke::default().with_width(self.width * 3.0).with_color(style::alpha(color, 0.12)));
            }
            frame.stroke(&path, Stroke::default().with_width(2.0).with_color(style::alpha(color, 0.9)));
            return vec![frame.into_geometry()];
        }
        let passes: &[(f32, f32)] = if self.glow { GLOW } else { &[(1.0, 1.0)] };
        for &(wide, strength) in passes {
            for i in 0..self.bars {
                let a = angle(i);
                let level = self.level(i);
                let r0 = edge(a);
                let line = Path::line(at(a, r0), at(a, r0 + 2.0 + level * self.reach));
                let alpha = (0.45 + 0.55 * level) * strength;
                let color = style::alpha(self.colors[i % self.colors.len()], alpha);
                frame.stroke(
                    &line,
                    Stroke::default().with_width(self.width * wide).with_color(color).with_line_cap(stroke::LineCap::Round),
                );
            }
        }
        vec![frame.into_geometry()]
    }
}

/// How much bigger the cover gets on the hardest beat at full pulse.
const PULSE: f32 = 0.09;
/// The seek ring: its distance from the cover's edge, and its thickness.
const ORBIT_GAP: f32 = 5.0;
const ORBIT_WIDTH: f32 = 4.0;

/// Drawn over the cover: an edge hugging its outline, and the seek ring you can click or drag.
struct Orbit {
    outline: Outline,
    cover: f32,
    turn: f32,
    border: f32,
    color: Color,
    /// Position and length, when the seek ring is on.
    seek: Option<(f32, f32)>,
    rest: Color,
}

impl Orbit {
    /// The seek ring's distance from the centre at `angle`.
    fn ring_radius(&self, angle: f32) -> f32 {
        self.outline.radius(angle - self.turn) * self.cover / 2.0 + self.border + ORBIT_GAP + ORBIT_WIDTH / 2.0
    }

    /// The angle under `p` as a share of the song (0 at the top, going clockwise), if `p` is on the ring.
    fn share_at(&self, bounds: Rectangle, p: Point) -> Option<f32> {
        let c = bounds.center();
        let (dx, dy) = (p.x - c.x, p.y - c.y);
        let a = dy.atan2(dx);
        ((dx.hypot(dy) - self.ring_radius(a)).abs() <= 14.0).then(|| (a + FRAC_PI_2).rem_euclid(TAU) / TAU)
    }

    /// The ring from the top round to `share` of the way.
    fn arc(&self, center: Point, share: f32) -> Path {
        let steps = ((240.0 * share).ceil() as usize).max(2);
        Path::new(|p| {
            for i in 0..=steps {
                let a = -FRAC_PI_2 + share * TAU * i as f32 / steps as f32;
                let r = self.ring_radius(a);
                let point = center + Vector::new(a.cos() * r, a.sin() * r);
                if i == 0 { p.move_to(point) } else { p.line_to(point) }
            }
        })
    }
}

impl canvas::Program<Message> for Orbit {
    type State = Dragging;

    fn update(&self, state: &mut Dragging, event: &canvas::Event, bounds: Rectangle, cursor: mouse::Cursor) -> Option<canvas::Action<Message>> {
        let (_, max) = self.seek?;
        match event {
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let share = self.share_at(bounds, cursor.position_over(bounds)?)?;
                state.0 = true;
                Some(canvas::Action::publish(Message::SeekPreview(share * max)).and_capture())
            }
            canvas::Event::Mouse(mouse::Event::CursorMoved { position }) if state.0 => {
                let c = bounds.center();
                let share = ((position.y - c.y).atan2(position.x - c.x) + FRAC_PI_2).rem_euclid(TAU) / TAU;
                Some(canvas::Action::publish(Message::SeekPreview(share * max)).and_capture())
            }
            canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) if state.0 => {
                state.0 = false;
                Some(canvas::Action::publish(Message::SeekCommit).and_capture())
            }
            _ => None,
        }
    }

    fn draw(&self, state: &Dragging, renderer: &Renderer, _: &Theme, bounds: Rectangle, cursor: mouse::Cursor) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let c = frame.center();
        if self.border > 0.0 {
            let edge = self.outline.path(c, self.cover / 2.0 + self.border / 2.0, self.turn);
            frame.stroke(&edge, Stroke::default().with_width(self.border).with_color(self.color));
        }
        if let Some((value, max)) = self.seek {
            let share = (value / max).clamp(0.0, 1.0);
            let hot = state.0 || cursor.position_over(bounds).is_some_and(|p| self.share_at(bounds, p).is_some());
            let width = if hot { ORBIT_WIDTH + 2.0 } else { ORBIT_WIDTH };
            let line = |color: Color| Stroke::default().with_width(width).with_color(color).with_line_cap(stroke::LineCap::Round);
            frame.stroke(&self.arc(c, 1.0), line(self.rest));
            if share > 0.0 {
                frame.stroke(&self.arc(c, share), line(self.color));
            }
            // A dot where you are.
            let a = -FRAC_PI_2 + share * TAU;
            let r = self.ring_radius(a);
            frame.fill(&Path::circle(c + Vector::new(a.cos() * r, a.sin() * r), width * if hot { 1.3 } else { 1.0 }), self.color);
        }
        vec![frame.into_geometry()]
    }

    fn mouse_interaction(&self, state: &Dragging, bounds: Rectangle, cursor: mouse::Cursor) -> mouse::Interaction {
        let over = self.seek.is_some() && cursor.position_over(bounds).is_some_and(|p| self.share_at(bounds, p).is_some());
        if state.0 || over { mouse::Interaction::Pointer } else { mouse::Interaction::default() }
    }
}

/// Bars or a wave in a strip under the cover, mirrored around the middle.
struct Under {
    levels: Vec<f32>,
    colors: Vec<Color>,
    wave: bool,
    mirror: bool,
    width: f32,
    glow: bool,
}

impl<M> canvas::Program<M> for Under {
    type State = ();

    fn draw(&self, _: &(), renderer: &Renderer, _: &Theme, bounds: Rectangle, _: mouse::Cursor) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let n = self.colors.len();
        let (w, h) = (bounds.width, bounds.height);
        let mid = h / 2.0;
        let level = |i: usize| {
            let k = self.levels.len().max(1);
            // Mirrored: lows in the middle, highs at both ends.
            let pos = if self.mirror { (2.0 * i as f32 / (n - 1).max(1) as f32 - 1.0).abs() } else { i as f32 / n as f32 };
            self.levels.get(((pos * (k - 1) as f32) as usize).min(k - 1)).copied().unwrap_or(0.0)
        };
        let x = |i: usize| (i as f32 + 0.5) * w / n as f32;
        if self.wave {
            let top: Vec<Point> = (0..n).map(|i| Point::new(x(i), mid - level(i) * (mid - 2.0) - 1.0)).collect();
            let path = Path::new(|p| {
                p.move_to(Point::new(0.0, mid));
                for pt in &top {
                    p.line_to(*pt);
                }
                p.line_to(Point::new(w, mid));
                for i in (0..n).rev() {
                    p.line_to(Point::new(x(i), mid + level(i) * (mid - 2.0) + 1.0));
                }
                p.close();
            });
            frame.fill(&path, style::alpha(self.colors[0], 0.7));
            return vec![frame.into_geometry()];
        }
        let passes: &[(f32, f32)] = if self.glow { GLOW } else { &[(1.0, 1.0)] };
        for &(wide, strength) in passes {
            for i in 0..n {
                let half = 1.0 + level(i) * (mid - self.width);
                let line = Path::line(Point::new(x(i), mid - half), Point::new(x(i), mid + half));
                let color = style::alpha(self.colors[i], (0.5 + 0.5 * level(i)) * strength);
                frame.stroke(
                    &line,
                    Stroke::default()
                        .with_width((self.width * wide).min(w / n as f32 * 0.8 * wide))
                        .with_color(color)
                        .with_line_cap(stroke::LineCap::Round),
                );
            }
        }
        vec![frame.into_geometry()]
    }
}

/// A seek or volume bar you can click and drag: a wave, a thin line, a thick bar, or dots.
struct Track {
    value: f32,
    max: f32,
    style: Progress,
    played: Color,
    rest: Color,
    /// Moves the wave along while music plays.
    phase: f32,
    moving: bool,
    on_change: fn(f32) -> Message,
    on_release: Message,
}

#[derive(Default)]
struct Dragging(bool);

impl canvas::Program<Message> for Track {
    type State = Dragging;

    fn update(&self, state: &mut Dragging, event: &canvas::Event, bounds: Rectangle, cursor: mouse::Cursor) -> Option<canvas::Action<Message>> {
        let value_at = |p: Point| ((p.x - bounds.x) / bounds.width).clamp(0.0, 1.0) * self.max;
        match event {
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let p = cursor.position_over(bounds)?;
                state.0 = true;
                Some(canvas::Action::publish((self.on_change)(value_at(p))).and_capture())
            }
            canvas::Event::Mouse(mouse::Event::CursorMoved { position }) if state.0 => {
                Some(canvas::Action::publish((self.on_change)(value_at(*position))).and_capture())
            }
            canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) if state.0 => {
                state.0 = false;
                Some(canvas::Action::publish(self.on_release.clone()).and_capture())
            }
            _ => None,
        }
    }

    fn draw(&self, state: &Dragging, renderer: &Renderer, _: &Theme, bounds: Rectangle, cursor: mouse::Cursor) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let (w, h) = (bounds.width, bounds.height);
        let mid = h / 2.0;
        let end = (self.value / self.max).clamp(0.0, 1.0) * w;
        let hot = state.0 || cursor.is_over(bounds);
        let line = |frame: &mut Frame, from: f32, to: f32, width: f32, color: Color| {
            if to > from {
                frame.stroke(
                    &Path::line(Point::new(from, mid), Point::new(to, mid)),
                    Stroke::default().with_width(width).with_color(color).with_line_cap(stroke::LineCap::Round),
                );
            }
        };
        match self.style {
            Progress::Wave => {
                let amp = if self.moving { 3.0 } else { 1.0 };
                let wave = Path::new(|p| {
                    let mut x = 0.0;
                    p.move_to(Point::new(0.0, mid + amp * self.phase.sin()));
                    while x < end - 4.0 {
                        x = (x + 2.0).min(end - 4.0);
                        p.line_to(Point::new(x, mid + amp * (x / 18.0 * TAU - self.phase).sin()));
                    }
                });
                frame.stroke(&wave, Stroke::default().with_width(3.0).with_color(self.played).with_line_cap(stroke::LineCap::Round));
                line(&mut frame, end + 6.0, w, 3.0, self.rest);
                thumb(&mut frame, end, mid, h, self.played);
            }
            Progress::Line => {
                line(&mut frame, 0.0, end, 3.0, self.played);
                line(&mut frame, end, w, 3.0, self.rest);
                if hot || self.max == 100.0 {
                    thumb(&mut frame, end, mid, h, self.played);
                }
            }
            Progress::Thick => {
                let bar = (h * 0.45).min(10.0);
                line(&mut frame, bar / 2.0, w - bar / 2.0, bar, self.rest);
                line(&mut frame, bar / 2.0, end.max(bar / 2.0), bar, self.played);
            }
            Progress::Dots => {
                let step = 9.0;
                let count = (w / step) as usize;
                for i in 0..=count {
                    let x = i as f32 * step + 2.0;
                    let on = x <= end;
                    let r = if on { 2.4 } else { 1.6 };
                    frame.fill(&Path::circle(Point::new(x.min(w - 2.0), mid), r), if on { self.played } else { self.rest });
                }
            }
            // Drawn round the cover instead (see `Orbit`).
            Progress::Ring | Progress::Hidden => {}
        }
        vec![frame.into_geometry()]
    }

    fn mouse_interaction(&self, state: &Dragging, bounds: Rectangle, cursor: mouse::Cursor) -> mouse::Interaction {
        if state.0 || cursor.is_over(bounds) { mouse::Interaction::Pointer } else { mouse::Interaction::default() }
    }
}

/// The upright bar that marks where you are.
fn thumb(frame: &mut Frame, x: f32, mid: f32, h: f32, color: Color) {
    let half = (h * 0.38).min(8.0);
    frame.stroke(
        &Path::line(Point::new(x, mid - half), Point::new(x, mid + half)),
        Stroke::default().with_width(4.0).with_color(color).with_line_cap(stroke::LineCap::Round),
    );
}

/// Part way from `a` to `b`.
fn mix(a: Color, b: Color, t: f32) -> Color {
    Color::from_rgb(a.r + (b.r - a.r) * t, a.g + (b.g - a.g) * t, a.b + (b.b - a.b) * t)
}

/// A smooth closed curve through `points`, using each point as a control between midpoints.
fn smooth_closed(points: &[Point]) -> Path {
    let mid = |a: Point, b: Point| Point::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
    Path::new(|p| {
        let n = points.len();
        if n < 3 {
            return;
        }
        p.move_to(mid(points[n - 1], points[0]));
        for i in 0..n {
            p.quadratic_curve_to(points[i], mid(points[i], points[(i + 1) % n]));
        }
        p.close();
    })
}
