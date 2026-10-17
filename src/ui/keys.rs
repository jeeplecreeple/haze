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
        Key::Named(Named::Backspace) => "backspace".into(),
        Key::Named(Named::Delete) => "delete".into(),
        Key::Named(Named::Home) => "home".into(),
        Key::Named(Named::End) => "end".into(),
        Key::Named(Named::PageUp) => "pageup".into(),
        Key::Named(Named::PageDown) => "pagedown".into(),
        Key::Named(n) => {
            let f = [
                Named::F1, Named::F2, Named::F3, Named::F4, Named::F5, Named::F6,
                Named::F7, Named::F8, Named::F9, Named::F10, Named::F11, Named::F12,
            ];
            return f.iter().position(|k| k == n).map(|i| format!("f{}", i + 1));
        }
        // "+" is the separator in a shortcut, so the plus key gets a name.
        Key::Character(c) if c.as_str() == "+" => "plus".into(),
        Key::Character(c) => c.to_lowercase(),
        _ => return None,
    })
}

/// The shortcut a key press makes, like "ctrl+shift+k", for recording one in Settings.
pub fn chord(event: &keyboard::Event) -> Option<String> {
    let keyboard::Event::KeyPressed { key, modifiers, .. } = event else { return None };
    let name = key_name(key)?;
    let mut parts = Vec::new();
    if modifiers.control() || modifiers.logo() {
        parts.push("ctrl".to_string());
    }
    if modifiers.alt() {
        parts.push("alt".into());
    }
    if modifiers.shift() {
        parts.push("shift".into());
    }
    parts.push(name);
    Some(parts.join("+"))
}

/// A shortcut as people write it: "Ctrl + Space", or "⌘ Space" on a Mac.
pub fn pretty(spec: &str) -> String {
    let Some(c) = parse(spec) else { return "Not set".into() };
    let mut parts: Vec<String> = Vec::new();
    let mac = cfg!(target_os = "macos");
    if c.ctrl {
        parts.push(if mac { "⌘" } else { "Ctrl" }.into());
    }
    if c.alt {
        parts.push(if mac { "⌥" } else { "Alt" }.into());
    }
    if c.shift {
        parts.push(if mac { "⇧" } else { "Shift" }.into());
    }
    let key = match c.key.as_str() {
        "left" => "←".to_string(),
        "right" => "→".into(),
        "up" => "↑".into(),
        "down" => "↓".into(),
        "plus" => "+".into(),
        k if k.chars().count() == 1 => k.to_uppercase(),
        k => {
            let mut chars = k.chars();
            chars.next().map(|f| f.to_uppercase().chain(chars).collect()).unwrap_or_default()
        }
    };
    parts.push(key);
    parts.join(if mac { " " } else { " + " })
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
    let table: [(&str, fn() -> Message); 13] = [
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
        (&keys.now_playing, || Message::TogglePlayer),
        (&keys.fullscreen, || Message::ToggleFullscreen),
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

    #[test]
    fn records_and_shows_chords() {
        let press = |key: Key, modifiers: Modifiers| keyboard::Event::KeyPressed {
            key: key.clone(),
            modified_key: key,
            physical_key: keyboard::key::Physical::Unidentified(keyboard::key::NativeCode::Unidentified),
            location: keyboard::Location::Standard,
            modifiers,
            text: None,
            repeat: false,
        };
        let c = chord(&press(Key::Character("K".into()), Modifiers::CTRL | Modifiers::SHIFT)).unwrap();
        assert_eq!(c, "ctrl+shift+k");
        assert!(matches(&c, "k", Modifiers::CTRL | Modifiers::SHIFT));
        assert_eq!(chord(&press(Key::Named(Named::F11), Modifiers::empty())).unwrap(), "f11");
        assert!(chord(&press(Key::Named(Named::Shift), Modifiers::SHIFT)).is_none());
        if !cfg!(target_os = "macos") {
            assert_eq!(pretty("ctrl+space"), "Ctrl + Space");
            assert_eq!(pretty("shift+right"), "Shift + →");
        }
        assert_eq!(pretty(""), "Not set");
    }
}
