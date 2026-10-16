//! The AI DJ: asks the user's chosen LLM for the next few songs (and, in talk/voice modes, short
//! radio-style intros), then resolves the picks to real Spotify tracks.

pub mod llm;
pub mod tts;

use std::collections::HashSet;
use std::fmt::Write as _;

use anyhow::{Result, anyhow};
use serde::Deserialize;

use crate::config::{DjConfig, DjMode};
use crate::spotify::api::{Api, Track};

/// What the DJ knows about the listener when it plans the next set.
#[derive(Debug, Clone, Default)]
pub struct Context {
    pub now_playing: Option<Track>,
    pub history: Vec<Track>,
    pub up_next: Vec<Track>,
    pub top_tracks: Vec<Track>,
    pub top_artists: Vec<String>,
    /// What the listener asked for, e.g. "rainy day indie" (may be empty).
    pub request: String,
    /// The first set of a session gets an opening line.
    pub opening: bool,
}

/// A planned set, resolved to playable tracks.
#[derive(Debug, Clone)]
pub struct Set {
    pub opener: Option<String>,
    pub tracks: Vec<(Track, Option<String>)>,
    /// Songs the LLM named that Spotify couldn't find.
    pub missed: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct RawSet {
    #[serde(default)]
    opener: Option<String>,
    #[serde(default)]
    tracks: Vec<RawPick>,
}

#[derive(Debug, Deserialize)]
struct RawPick {
    title: String,
    artist: String,
    #[serde(default)]
    intro: Option<String>,
}

pub fn system_prompt(config: &DjConfig) -> String {
    let mut s = format!(
        "You are {name}, a radio DJ inside a music player. Persona: {persona}\n\
         You pick songs that flow well together and fit the listener's taste and request.\n\
         Only pick real songs that exist on Spotify; give the exact song title and main artist.\n\
         Discovery level {discovery}/100: at 0 stick to the listener's favourites, at 100 pick songs \
         they have probably never heard but will like.\n",
        name = config.name,
        persona = config.persona,
        discovery = config.discovery,
    );
    if !config.allow_explicit {
        s.push_str("Never pick songs with explicit lyrics.\n");
    }
    match config.mode {
        DjMode::Queue => s.push_str("Do not write any commentary. Leave opener and intro out.\n"),
        DjMode::Talk | DjMode::Voice => {
            let _ = writeln!(
                s,
                "Write short intros in {lang}, at most {words} words each, like a real radio host: \
                 a fact, a story or a segue, never a list. Don't invent facts you are unsure of. \
                 {spoken}",
                lang = config.language,
                words = config.max_words,
                spoken = if config.mode == DjMode::Voice {
                    "Intros are read aloud by a speech engine, so no emoji, hashtags or markdown."
                } else {
                    ""
                },
            );
        }
    }
    s.push_str(
        "Reply with only a JSON object: {\"opener\": string or null, \"tracks\": \
         [{\"title\": string, \"artist\": string, \"intro\": string or null}]}",
    );
    s
}

fn describe(t: &Track) -> String {
    format!("{} - {}", t.name, t.artist_line())
}

pub fn user_prompt(config: &DjConfig, ctx: &Context) -> String {
    let mut s = String::new();
    let list = |s: &mut String, title: &str, tracks: &[Track]| {
        if !tracks.is_empty() {
            let _ = writeln!(s, "{title}:");
            for t in tracks {
                let _ = writeln!(s, "- {}", describe(t));
            }
        }
    };
    if let Some(t) = &ctx.now_playing {
        let _ = writeln!(s, "Now playing: {}", describe(t));
    }
    list(&mut s, "Recently played (oldest first)", &ctx.history);
    list(&mut s, "Already queued", &ctx.up_next);
    list(&mut s, "Listener's current favourite tracks", &ctx.top_tracks);
    if !ctx.top_artists.is_empty() {
        let _ = writeln!(s, "Listener's favourite artists: {}", ctx.top_artists.join("; "));
    }
    let request = ctx.request.trim();
    if !request.is_empty() {
        let _ = writeln!(s, "The listener asked for: {request}");
    }
    let _ = writeln!(
        s,
        "\nPick the next {n} songs. Don't repeat anything listed above.",
        n = config.batch_size.clamp(1, 20)
    );
    if config.mode != DjMode::Queue {
        let every = config.talk_every.max(1);
        if every == 1 {
            s.push_str("Give every song an intro.\n");
        } else {
            let _ = writeln!(s, "Give an intro to every {every}th song only (the 1st, then every {every}th); null for the rest.");
        }
        if ctx.opening {
            s.push_str("Also write an opener welcoming the listener to the set.\n");
        }
    }
    s
}

/// Extracts the JSON object from a reply, tolerating code fences or chatter around it.
fn parse(text: &str) -> Result<RawSet> {
    let start = text.find('{').ok_or_else(|| anyhow!("DJ reply had no JSON"))?;
    let end = text.rfind('}').ok_or_else(|| anyhow!("DJ reply had no JSON"))?;
    serde_json::from_str(&text[start..=end]).map_err(|e| anyhow!("DJ reply wasn't valid JSON: {e}"))
}

fn clean(s: Option<String>) -> Option<String> {
    s.map(|s| s.trim().to_string()).filter(|s| !s.is_empty() && s != "null")
}

/// Asks the LLM for a set and finds each pick on Spotify.
pub async fn plan_set(api: &Api, config: &DjConfig, ctx: Context) -> Result<Set> {
    let reply = llm::complete(config, &system_prompt(config), &user_prompt(config, &ctx)).await?;
    let raw = parse(&reply)?;

    let mut seen: HashSet<String> = ctx
        .history
        .iter()
        .chain(&ctx.up_next)
        .chain(&ctx.now_playing)
        .map(|t| t.id.clone())
        .collect();
    let talk = config.mode != DjMode::Queue;
    let mut set = Set {
        opener: if talk && ctx.opening { clean(raw.opener) } else { None },
        tracks: Vec::new(),
        missed: Vec::new(),
    };
    for pick in raw.tracks.into_iter().take(config.batch_size.clamp(1, 20)) {
        match api.find_track(&pick.title, &pick.artist).await {
            Ok(Some(track)) if (config.allow_explicit || !track.explicit) && seen.insert(track.id.clone()) => {
                set.tracks.push((track, if talk { clean(pick.intro) } else { None }));
            }
            Ok(_) => set.missed.push(format!("{} - {}", pick.title, pick.artist)),
            Err(e) => {
                log::warn!("DJ search failed: {e}");
                set.missed.push(format!("{} - {}", pick.title, pick.artist));
            }
        }
    }
    if set.tracks.is_empty() {
        return Err(anyhow!("The DJ's picks couldn't be found on Spotify. Try again or change the request."));
    }
    Ok(set)
}

/// Gathers listening history for the DJ. Failures here only make the DJ less personal.
/// Returns (top tracks, top artists, recently played, oldest first).
pub async fn taste(api: &Api) -> (Vec<Track>, Vec<String>, Vec<Track>) {
    let (tracks, artists, recent) =
        tokio::join!(api.top_tracks("short_term", 15), api.top_artists(10), api.recently_played(10));
    let mut recent = recent.unwrap_or_default();
    recent.reverse();
    (tracks.unwrap_or_default(), artists.unwrap_or_default(), recent)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_fenced_json() {
        let reply = "Sure!\n```json\n{\"opener\": \"Hey\", \"tracks\": [{\"title\": \"Song\", \"artist\": \"Band\", \"intro\": null}]}\n```";
        let set = parse(reply).unwrap();
        assert_eq!(set.opener.as_deref(), Some("Hey"));
        assert_eq!(set.tracks[0].artist, "Band");
        assert!(set.tracks[0].intro.is_none());
    }

    #[test]
    fn queue_mode_prompt_has_no_intros() {
        let mut c = DjConfig::default();
        c.mode = DjMode::Queue;
        let p = user_prompt(&c, &Context { opening: true, ..Default::default() });
        assert!(!p.contains("intro"));
        assert!(system_prompt(&c).contains("Do not write any commentary"));
    }
}
