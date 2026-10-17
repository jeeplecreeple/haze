//! Settings > Layout: a little picture of the window you can rearrange, plus the sizes.

use iced::widget::{Column, Space, column, container, row, slider, text, toggler};
use iced::{Alignment, Fill, Length};

use super::super::icon;
use super::super::{App, Edit, Message, style};
use super::El;
use crate::config::{Edge, Layout, Side};

const COLUMNS: [(&str, &str); 4] = [("title", "Title"), ("artist", "Artist"), ("album", "Album"), ("duration", "Length")];

impl App {
    /// A message that swaps in the layout with one change made.
    fn with_layout(&self, change: impl Fn(&mut Layout) + 'static) -> Message {
        let mut l = self.config.layout.clone();
        l.show_side_panel = self.show_side_panel;
        change(&mut l);
        Message::Edit(Edit::LayoutStyle(Box::new(l)))
    }

    fn l_num<'a>(&self, label: &'a str, value: f32, range: std::ops::RangeInclusive<f32>, set: fn(&mut Layout, f32)) -> El<'a> {
        let base = self.config.layout.clone();
        let panel = self.show_side_panel;
        self.setting(
            label,
            row![
                slider(range, value, move |v| {
                    let mut l = base.clone();
                    l.show_side_panel = panel;
                    set(&mut l, v);
                    Message::Edit(Edit::LayoutStyle(Box::new(l)))
                })
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

    /// One panel in the picture: its name and buttons to move or hide it.
    fn layout_box<'a>(&self, title: &'a str, buttons: Vec<El<'a>>, width: Length, height: Length, strong: bool) -> El<'a> {
        let mut col = Column::new().spacing(6).align_x(Alignment::Center).push(
            text(title).size(self.fs(0.85)).font(self.weight(iced::font::Weight::Semibold)),
        );
        for b in buttons {
            col = col.push(b);
        }
        let fill = if strong { style::tint(self.colors, 0.14) } else { style::tint(self.colors, 0.07) };
        container(col).center_x(width).center_y(height).padding(6).style(style::tile(fill, self.r())).into()
    }

    pub(super) fn layout_editor(&self) -> El<'_> {
        let l = &self.config.layout;
        let panel = self.show_side_panel;
        let flip = |s: Side| if s == Side::Left { Side::Right } else { Side::Left };
        let (side_label, queue_label) = (
            if l.sidebar == Side::Left { "Move right" } else { "Move left" },
            if l.side_panel == Side::Left { "Move right" } else { "Move left" },
        );
        let h = Length::Fixed(200.0);
        let sidebar = l.show_sidebar.then(|| {
            let s = l.sidebar;
            self.layout_box(
                "Sidebar",
                vec![
                    self.small_button(None, side_label, self.with_layout(move |l| l.sidebar = flip(s))),
                    self.small_button(None, "Hide", self.with_layout(|l| l.show_sidebar = false)),
                ],
                Length::Fixed((l.sidebar_width * 0.45).clamp(90.0, 170.0)),
                h,
                false,
            )
        });
        let queue = panel.then(|| {
            let s = l.side_panel;
            self.layout_box(
                "Queue",
                vec![
                    self.small_button(None, queue_label, self.with_layout(move |l| l.side_panel = flip(s))),
                    self.small_button(None, "Hide", self.with_layout(|l| l.show_side_panel = false)),
                ],
                Length::Fixed((l.side_panel_width * 0.42).clamp(90.0, 210.0)),
                h,
                false,
            )
        });
        let main = self.layout_box("Your music", Vec::new(), Length::Fill, h, true);
        let mut left = Vec::new();
        let mut right = Vec::new();
        if let Some(s) = sidebar {
            if l.sidebar == Side::Left { left.push(s) } else { right.push(s) }
        }
        if let Some(q) = queue {
            if l.side_panel == Side::Left { left.push(q) } else { right.insert(0, q) }
        }
        let gap = (l.gap * 0.6).clamp(2.0, 20.0);
        let body = iced::widget::Row::with_children(left).push(main).extend(right).spacing(gap);
        let edge = l.player_bar;
        let bar = container(
            row![
                text("Player bar").size(self.fs(0.85)).font(self.weight(iced::font::Weight::Semibold)).width(Fill),
                self.small_button(
                    None,
                    if edge == Edge::Top { "Move to the bottom" } else { "Move to the top" },
                    self.with_layout(move |l| l.player_bar = if edge == Edge::Top { Edge::Bottom } else { Edge::Top }),
                ),
            ]
            .align_y(Alignment::Center),
        )
        .padding([6, 12])
        .width(Fill)
        .style(style::tile(style::tint(self.colors, 0.11), self.r()));
        let picture = match edge {
            Edge::Top => column![bar, body],
            Edge::Bottom => column![body, bar],
        }
        .spacing(gap);

        let mut hidden = row![].spacing(8);
        if !l.show_sidebar {
            hidden = hidden.push(self.small_button(Some(icon::LIBRARY), "Show the sidebar", self.with_layout(|l| l.show_sidebar = true)));
        }
        if !panel {
            hidden = hidden.push(self.small_button(Some(icon::LIST_MUSIC), "Show the queue", self.with_layout(|l| l.show_side_panel = true)));
        }

        let mut columns = Column::new().spacing(8);
        for (i, (key, label)) in COLUMNS.iter().enumerate() {
            let on = l.track_columns.iter().any(|c| c == key);
            let key = key.to_string();
            let toggle_key = key.clone();
            let place = l.track_columns.iter().position(|c| *c == key);
            let up = place.filter(|&p| p > 0).map(|p| self.with_layout(move |l| l.track_columns.swap(p, p - 1)));
            let down = place.filter(|&p| p + 1 < l.track_columns.len()).map(|p| self.with_layout(move |l| l.track_columns.swap(p, p + 1)));
            let toggle = {
                let base = self.config.layout.clone();
                toggler(on).size(18).on_toggle(move |v| {
                    let mut l = base.clone();
                    l.show_side_panel = panel;
                    l.track_columns.retain(|c| *c != toggle_key);
                    if v {
                        let at = COLUMNS[..i].iter().filter(|(k, _)| l.track_columns.iter().any(|c| c == k)).count();
                        l.track_columns.insert(at.min(l.track_columns.len()), toggle_key.clone());
                    }
                    Message::Edit(Edit::LayoutStyle(Box::new(l)))
                })
            };
            let mut line = row![toggle, text(*label).size(self.fs(0.95)).width(90)].spacing(10).align_y(Alignment::Center);
            if on {
                line = line.push(self.icon_button(icon::CHEVRON_UP, self.fs(0.85), up, false));
                line = line.push(self.icon_button(icon::CHEVRON_DOWN, self.fs(0.85), down, false));
                line = line.push(self.muted(match place { Some(p) => format!("#{}", p + 1), None => String::new() }).size(self.fs(0.8)).wrapping(text::Wrapping::None));
            }
            columns = columns.push(line);
        }

        let rows = vec![
            text("Click the buttons in the picture to move or hide panels. The window changes as you go.")
                .size(self.fs(0.9))
                .color(self.colors.muted_text)
                .into(),
            container(picture).padding(12).width(Fill).style(style::tile(style::tint(self.colors, 0.03), self.panel_r())).into(),
            hidden.into(),
            self.l_num("Sidebar width", l.sidebar_width, 160.0..=360.0, |l, v| l.sidebar_width = v),
            self.l_num("Queue width", l.side_panel_width, 220.0..=520.0, |l, v| l.side_panel_width = v),
            self.l_num("Space between panels", l.gap, 0.0..=32.0, |l, v| l.gap = v),
            self.l_num("Space inside the page", l.padding, 0.0..=56.0, |l, v| l.padding = v),
            self.setting("Song list columns", columns),
            Space::new().height(4).into(),
            row![self.small_button(Some(icon::ROTATE_CCW), "Reset the layout", Message::Edit(Edit::LayoutStyle(Box::default())))].into(),
        ];
        self.section(icon::LAYOUT_TEMPLATE, "Layout editor", rows)
    }
}
