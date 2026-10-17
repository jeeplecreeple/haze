//! Configurable keyboard shortcuts.

use iced::keyboard::{self, Key, Modifiers, key::Named};

use super::{Message, Page};
use crate::config::Keybindings;

#[derive(Debug, PartialEq)]
struct Chord {
    ctrl: bool,
    shift: bool,
    alt: bool,
    key: String,
}

/// Parses "ctrl+shift+f", "space", "right"... into a chord. "cmd" and "super" count as ctrl,
/// so "ctrl+..." bindings also work with the Command key on macOS.
fn parse(spec: &str) -> Option<Chord> {
    let mut chord = Chord { ctrl: false, shift: false, alt: false, key: String::new() };
    for part in spec.split('+').map(|p| p.trim().to_ascii_lowercase()) {
        match part.as_str() {
            "ctrl" | "control" | "cmd" | "command" | "super" | "meta" => chord.ctrl = true,
            "shift" => chord.shift = true,
            "alt" | "option" => chord.alt = true,
            "" => {}
            key => chord.key = key.to_string(),
        }
    }
    (!chord.key.is_empty()).then_some(chord)
}

fn key_name(key: &Key) -> Option<String> {
    Some(match key {
        Key::Named(Named::Space) => "space".into(),
        Key::Named(Named::Enter) => "enter".into(),
        Key::Named(Named::Escape) => "escape".into(),
        Key::Named(Named::Tab) => "tab".into(),
        Key::Named(Named::ArrowLeft) => "left".into(),
        Key::Named(Named::ArrowRight) => "right".into(),
        Key::Named(Named::ArrowUp) => "up".into(),
        Key::Named(Named::ArrowDown) => "down".into(),
        Key::Named(Named::MediaPlayPause) => "mediaplaypause".into(),
        Key::Named(Named::MediaTrackNext) => "medianext".into(),
        Key::Named(Named::MediaTrackPrevious) => "mediaprevious".into(),
        Key::Character(c) => c.to_lowercase(),
        _ => return None,
    })
}

fn matches(spec: &str, key: &str, mods: Modifiers) -> bool {
    parse(spec).is_some_and(|c| {
        c.key == key && c.ctrl == (mods.control() || mods.logo()) && c.shift == mods.shift() && c.alt == mods.alt()
    })
}

pub fn action(keys: &Keybindings, event: &keyboard::Event) -> Option<Message> {
    let keyboard::Event::KeyPressed { key, modifiers, .. } = event else {
        return None;
    };
    // Hardware media keys always work.
    match key {
        Key::Named(Named::MediaPlayPause) => return Some(Message::TogglePlay),
        Key::Named(Named::MediaTrackNext) => return Some(Message::Next),
        Key::Named(Named::MediaTrackPrevious) => return Some(Message::Previous),
        _ => {}
    }
    let name = key_name(key)?;
    let m = *modifiers;
    let table: [(&str, fn() -> Message); 11] = [
        (&keys.play_pause, || Message::TogglePlay),
        (&keys.next, || Message::Next),
        (&keys.previous, || Message::Previous),
        (&keys.volume_up, || Message::VolumeStep(5)),
        (&keys.volume_down, || Message::VolumeStep(-5)),
        (&keys.seek_forward, || Message::SeekStep(10_000)),
        (&keys.seek_back, || Message::SeekStep(-10_000)),
        (&keys.search, || Message::Open(Page::Search)),
        (&keys.toggle_side_panel, || Message::ToggleSidePanel),
        (&keys.dj_now, || Message::DjStart),
        (&keys.lyrics, || Message::ToggleLyrics),
    ];
    table.iter().find(|(spec, _)| matches(spec, &name, m)).map(|(_, f)| f())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_chords() {
        assert_eq!(parse("Ctrl+Space"), Some(Chord { ctrl: true, shift: false, alt: false, key: "space".into() }));
        assert_eq!(parse("shift+right").unwrap().key, "right");
        assert!(parse("ctrl+").is_none());
        assert!(matches("cmd+f", "f", Modifiers::LOGO));
        assert!(!matches("ctrl+f", "f", Modifiers::empty()));
    }
}
