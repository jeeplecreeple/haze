//! Time-synced lyrics from Spotify's own lyrics service, with LRCLIB (lrclib.net) as a fallback.
//! Lines carry per-word timings: real ones when the source has them (enhanced LRC), otherwise
//! estimated by spreading the line's duration over its characters, for a karaoke-style highlight.

use std::time::Duration;

use anyhow::{Result, anyhow};
use serde::Deserialize;

use crate::spotify::api::Track;

#[derive(Debug, Clone, PartialEq)]
pub struct Word {
    pub text: String,
    pub start_ms: u32,
    pub end_ms: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub start_ms: u32,
    pub end_ms: u32,
    pub words: Vec<Word>,
}

impl Line {
    pub fn text(&self) -> String {
        self.words.iter().map(|w| w.text.as_str()).collect::<Vec<_>>().join(" ")
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Lyrics {
    pub track_id: String,
    pub lines: Vec<Line>,
    pub synced: bool,
    pub source: &'static str,
}

impl Lyrics {
    /// Index of the line being sung at `ms`, if any has started.
    pub fn active_line(&self, ms: u32) -> Option<usize> {
        if !self.synced {
            return None;
        }
        self.lines.iter().rposition(|l| l.start_ms <= ms)
    }
}

/// The longest a single line may stay lit when there is a long instrumental gap after it.
const MAX_LINE_MS: u32 = 8_000;

fn build_lines(raw: Vec<(u32, String)>, track_ms: u32) -> Vec<Line> {
    let mut lines = Vec::with_capacity(raw.len());
    for (i, (start, text)) in raw.iter().enumerate() {
        let next = raw.get(i + 1).map(|(s, _)| *s).unwrap_or(track_ms.max(start + 4_000));
        let end = next.min(start + MAX_LINE_MS).max(*start + 1);
        lines.push(Line { start_ms: *start, end_ms: end, words: estimate_words(text, *start, end) });
    }
    lines
}

/// Spreads a line's time over its words in proportion to their length.
fn estimate_words(text: &str, start: u32, end: u32) -> Vec<Word> {
    let parts: Vec<&str> = text.split_whitespace().collect();
    let total: usize = parts.iter().map(|p| p.chars().count() + 1).sum();
    let span = (end - start) as f32 * 0.9; // leave a short breath before the next line
    let mut t = start as f32;
    parts
        .into_iter()
        .map(|p| {
            let d = span * (p.chars().count() + 1) as f32 / total.max(1) as f32;
            let w = Word { text: p.to_string(), start_ms: t as u32, end_ms: (t + d) as u32 };
            t += d;
            w
        })
        .collect()
}

/// Parses Spotify's color-lyrics JSON.
pub fn parse_spotify(track: &Track, json: &[u8]) -> Result<Lyrics> {
    #[derive(Deserialize)]
    struct Root {
        lyrics: Inner,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Inner {
        sync_type: String,
        lines: Vec<RawLine>,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct RawLine {
        start_time_ms: String,
        words: String,
    }
    let root: Root = serde_json::from_slice(json)?;
    let synced = root.lyrics.sync_type != "UNSYNCED";
    let raw: Vec<(u32, String)> = root
        .lyrics
        .lines
        .into_iter()
        .map(|l| (l.start_time_ms.parse().unwrap_or(0), l.words.replace('♪', "").trim().to_string()))
        .filter(|(_, w)| !w.is_empty())
        .collect();
    if raw.is_empty() {
        return Err(anyhow!("no lyrics"));
    }
    Ok(Lyrics { track_id: track.id.clone(), lines: build_lines(raw, track.duration_ms), synced, source: "Spotify" })
}

fn parse_stamp(s: &str) -> Option<u32> {
    let (m, rest) = s.split_once(':')?;
    let secs: f32 = rest.parse().ok()?;
    Some(m.trim().parse::<u32>().ok()? * 60_000 + (secs * 1000.0) as u32)
}

/// Parses LRC, including "enhanced" LRC with `<mm:ss.xx>` word timings.
pub fn parse_lrc(track: &Track, lrc: &str, source: &'static str) -> Option<Lyrics> {
    let mut raw: Vec<(u32, String)> = Vec::new();
    let mut enhanced: Vec<(u32, Vec<(u32, String)>)> = Vec::new();
    for line in lrc.lines() {
        let mut rest = line.trim();
        let mut stamps = Vec::new();
        while let Some(stripped) = rest.strip_prefix('[') {
            let Some((tag, after)) = stripped.split_once(']') else { break };
            match parse_stamp(tag) {
                Some(ms) => stamps.push(ms),
                None => break, // metadata like [ar:...]
            }
            rest = after;
        }
        if stamps.is_empty() {
            continue;
        }
        // Enhanced LRC: "<00:12.30> word <00:12.80> word"
        let mut words = Vec::new();
        if rest.contains('<') {
            for chunk in rest.split('<').filter(|c| !c.is_empty()) {
                if let Some((tag, text)) = chunk.split_once('>')
                    && let Some(ms) = parse_stamp(tag)
                    && !text.trim().is_empty()
                {
                    words.push((ms, text.trim().to_string()));
                }
            }
        }
        let plain: String = if words.is_empty() {
            rest.trim().to_string()
        } else {
            words.iter().map(|(_, w)| w.as_str()).collect::<Vec<_>>().join(" ")
        };
        if plain.is_empty() {
            continue;
        }
        for ms in stamps {
            raw.push((ms, plain.clone()));
            if !words.is_empty() {
                enhanced.push((ms, words.clone()));
            }
        }
    }
    if raw.is_empty() {
        return None;
    }
    raw.sort_by_key(|(ms, _)| *ms);
    let mut lines = build_lines(raw, track.duration_ms);
    // Swap in real word timings where the file had them.
    for line in &mut lines {
        if let Some((_, words)) = enhanced.iter().find(|(ms, _)| *ms == line.start_ms) {
            line.words = words
                .iter()
                .enumerate()
                .map(|(i, (ms, text))| Word {
                    text: text.clone(),
                    start_ms: *ms,
                    end_ms: words.get(i + 1).map(|(n, _)| *n).unwrap_or(line.end_ms),
                })
                .collect();
        }
    }
    Some(Lyrics { track_id: track.id.clone(), lines, synced: true, source })
}

fn plain(track: &Track, text: &str, source: &'static str) -> Option<Lyrics> {
    let lines: Vec<Line> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(|l| Line { start_ms: 0, end_ms: 0, words: estimate_words(l, 0, 0) })
        .collect();
    (!lines.is_empty()).then(|| Lyrics { track_id: track.id.clone(), lines, synced: false, source })
}

/// Looks the track up on LRCLIB, a free community lyrics database.
pub async fn lrclib(track: &Track) -> Result<Option<Lyrics>> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Hit {
        synced_lyrics: Option<String>,
        plain_lyrics: Option<String>,
    }
    let http = reqwest::Client::builder()
        .user_agent(concat!("haztify/", env!("CARGO_PKG_VERSION"), " (https://github.com/jeeplecreeple/haztify)"))
        .timeout(Duration::from_secs(10))
        .build()?;
    let artist = track.artists.first().map(String::as_str).unwrap_or_default();
    let url = format!(
        "https://lrclib.net/api/get?track_name={}&artist_name={}&album_name={}&duration={}",
        urlencoding::encode(&track.name),
        urlencoding::encode(artist),
        urlencoding::encode(&track.album),
        track.duration_ms / 1000
    );
    let resp = http.get(&url).send().await?;
    let hit: Option<Hit> = if resp.status().is_success() {
        Some(resp.json().await?)
    } else {
        // No exact match: fall back to a search and take the first synced result.
        let url = format!(
            "https://lrclib.net/api/search?track_name={}&artist_name={}",
            urlencoding::encode(&track.name),
            urlencoding::encode(artist)
        );
        let mut hits: Vec<Hit> = http.get(&url).send().await?.error_for_status()?.json().await?;
        let best = hits.iter().position(|h| h.synced_lyrics.is_some()).unwrap_or(0);
        (!hits.is_empty()).then(|| hits.swap_remove(best))
    };
    Ok(hit.and_then(|h| {
        h.synced_lyrics
            .as_deref()
            .and_then(|l| parse_lrc(track, l, "LRCLIB"))
            .or_else(|| h.plain_lyrics.as_deref().and_then(|l| plain(track, l, "LRCLIB")))
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track() -> Track {
        Track {
            id: "t".into(),
            uri: "spotify:track:t".into(),
            name: "Song".into(),
            artists: vec!["Band".into()],
            album: "Album".into(),
            duration_ms: 60_000,
            explicit: false,
            image_url: None,
        }
    }

    #[test]
    fn parses_lrc_and_finds_active_line() {
        let lrc = "[ar:Band]\n[00:01.00]Hello there\n[00:05.50]General Kenobi\n[00:10.00][00:20.00]Chorus line\n";
        let l = parse_lrc(&track(), lrc, "test").unwrap();
        assert_eq!(l.lines.len(), 4);
        assert_eq!(l.lines[1].start_ms, 5_500);
        assert_eq!(l.lines[1].text(), "General Kenobi");
        assert_eq!(l.active_line(500), None);
        assert_eq!(l.active_line(6_000), Some(1));
        assert_eq!(l.active_line(25_000), Some(3));
        // Estimated words stay inside their line.
        let w = &l.lines[0].words;
        assert_eq!(w.len(), 2);
        assert!(w[0].start_ms == 1_000 && w[1].end_ms <= l.lines[0].end_ms);
    }

    #[test]
    fn parses_enhanced_lrc_word_timing() {
        let lrc = "[00:01.00]<00:01.00>Hel <00:01.40>lo <00:02.00>world\n[00:04.00]next";
        let l = parse_lrc(&track(), lrc, "test").unwrap();
        assert_eq!(l.lines[0].words.len(), 3);
        assert_eq!(l.lines[0].words[1].start_ms, 1_400);
        assert_eq!(l.lines[0].words[2].end_ms, 4_000);
    }

    #[test]
    fn parses_spotify_json() {
        let json = r#"{"lyrics":{"syncType":"LINE_SYNCED","lines":[
            {"startTimeMs":"1000","words":"First line","endTimeMs":"0"},
            {"startTimeMs":"3000","words":"♪","endTimeMs":"0"},
            {"startTimeMs":"5000","words":"Second","endTimeMs":"0"}]}}"#;
        let l = parse_spotify(&track(), json.as_bytes()).unwrap();
        assert!(l.synced);
        assert_eq!(l.lines.len(), 2);
        assert_eq!(l.lines[0].end_ms, 5_000);
    }
}

#[cfg(test)]
mod online {
    use super::*;

    /// `cargo test -- --ignored lrclib` checks the live LRCLIB service.
    #[tokio::test]
    #[ignore]
    async fn lrclib_finds_a_well_known_song() {
        let t = Track {
            id: "x".into(),
            uri: "spotify:track:x".into(),
            name: "Midnight City".into(),
            artists: vec!["M83".into()],
            album: "Hurry Up, We're Dreaming".into(),
            duration_ms: 243_000,
            explicit: false,
            image_url: None,
        };
        let l = lrclib(&t).await.unwrap().expect("lyrics");
        assert!(l.synced && l.lines.len() > 5);
    }
}
