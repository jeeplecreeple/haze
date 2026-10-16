//! The play queue. Haztify owns its queue so the DJ can add to it freely.

use rand::seq::SliceRandom;

use crate::spotify::api::Track;

#[derive(Debug, Clone, PartialEq)]
pub enum Origin {
    User,
    /// Picked by the DJ, with an optional intro to say before it plays.
    Dj { intro: Option<String> },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub track: Track,
    pub origin: Origin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Repeat {
    #[default]
    Off,
    All,
    One,
}

impl Repeat {
    pub fn cycle(self) -> Self {
        match self {
            Repeat::Off => Repeat::All,
            Repeat::All => Repeat::One,
            Repeat::One => Repeat::Off,
        }
    }
}

#[derive(Debug, Default)]
pub struct Queue {
    entries: Vec<Entry>,
    current: Option<usize>,
    pub shuffle: bool,
    pub repeat: Repeat,
}

impl Queue {
    pub fn current(&self) -> Option<&Entry> {
        self.current.and_then(|i| self.entries.get(i))
    }

    pub fn current_index(&self) -> Option<usize> {
        self.current
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    pub fn up_next(&self) -> &[Entry] {
        match self.current {
            Some(i) => self.entries.get(i + 1..).unwrap_or(&[]),
            None => &self.entries,
        }
    }

    /// Replaces the queue with `tracks` and starts at `start`.
    pub fn play_list(&mut self, tracks: &[Track], start: usize) -> Option<&Entry> {
        self.entries = tracks.iter().cloned().map(|track| Entry { track, origin: Origin::User }).collect();
        if self.entries.is_empty() {
            self.current = None;
            return None;
        }
        let start = start.min(self.entries.len() - 1);
        if self.shuffle {
            let first = self.entries.remove(start);
            self.entries.shuffle(&mut rand::rng());
            self.entries.insert(0, first);
            self.current = Some(0);
        } else {
            self.current = Some(start);
        }
        self.current()
    }

    pub fn push(&mut self, entry: Entry) {
        self.entries.push(entry);
    }

    /// Inserts right after the current track.
    pub fn play_next(&mut self, entry: Entry) {
        let at = self.current.map(|i| i + 1).unwrap_or(0).min(self.entries.len());
        self.entries.insert(at, entry);
    }

    pub fn remove_up_next(&mut self, offset: usize) {
        let index = self.current.map(|i| i + 1).unwrap_or(0) + offset;
        if index < self.entries.len() {
            self.entries.remove(index);
        }
    }

    pub fn clear_up_next(&mut self) {
        match self.current {
            Some(i) => self.entries.truncate(i + 1),
            None => self.entries.clear(),
        }
    }

    /// Jumps to a track in the up-next list.
    pub fn jump(&mut self, offset: usize) -> Option<&Entry> {
        let index = self.current.map(|i| i + 1).unwrap_or(0) + offset;
        if index < self.entries.len() {
            self.current = Some(index);
        }
        self.current()
    }

    /// Moves on after a track ends (respects repeat-one) or the user skips (`manual`).
    pub fn advance(&mut self, manual: bool) -> Option<&Entry> {
        if self.entries.is_empty() {
            return None;
        }
        if !manual && self.repeat == Repeat::One {
            return self.current();
        }
        let next = self.current.map(|i| i + 1).unwrap_or(0);
        if next < self.entries.len() {
            self.current = Some(next);
        } else if self.repeat == Repeat::All {
            self.current = Some(0);
        } else {
            return None;
        }
        self.current()
    }

    pub fn back(&mut self) -> Option<&Entry> {
        if let Some(i) = self.current {
            self.current = Some(i.saturating_sub(1));
        }
        self.current()
    }

    pub fn toggle_shuffle(&mut self) {
        self.shuffle = !self.shuffle;
        if self.shuffle {
            let from = self.current.map(|i| i + 1).unwrap_or(0).min(self.entries.len());
            self.entries[from..].shuffle(&mut rand::rng());
        }
    }

    /// Recent history, newest last, for the DJ's context.
    pub fn history(&self, n: usize) -> &[Entry] {
        let end = self.current.map(|i| i + 1).unwrap_or(0);
        &self.entries[end.saturating_sub(n)..end]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(id: &str) -> Track {
        Track {
            id: id.into(),
            uri: format!("spotify:track:{id}"),
            name: id.into(),
            artists: vec![],
            album: String::new(),
            duration_ms: 0,
            explicit: false,
            image_url: None,
        }
    }

    #[test]
    fn advance_and_repeat() {
        let mut q = Queue::default();
        q.play_list(&[t("a"), t("b")], 0);
        assert_eq!(q.advance(false).unwrap().track.id, "b");
        assert!(q.advance(false).is_none());
        q.repeat = Repeat::All;
        assert_eq!(q.advance(false).unwrap().track.id, "a");
        q.repeat = Repeat::One;
        assert_eq!(q.advance(false).unwrap().track.id, "a");
        assert_eq!(q.advance(true).unwrap().track.id, "b");
    }

    #[test]
    fn play_next_and_up_next() {
        let mut q = Queue::default();
        q.play_list(&[t("a"), t("b")], 0);
        q.play_next(Entry { track: t("x"), origin: Origin::User });
        let ids: Vec<_> = q.up_next().iter().map(|e| e.track.id.as_str()).collect();
        assert_eq!(ids, ["x", "b"]);
        q.remove_up_next(0);
        assert_eq!(q.up_next().len(), 1);
        assert_eq!(q.history(5).len(), 1);
    }

    #[test]
    fn shuffle_keeps_chosen_track_first() {
        let mut q = Queue::default();
        q.shuffle = true;
        let tracks: Vec<_> = (0..20).map(|i| t(&i.to_string())).collect();
        assert_eq!(q.play_list(&tracks, 7).unwrap().track.id, "7");
        assert_eq!(q.entries().len(), 20);
    }
}
