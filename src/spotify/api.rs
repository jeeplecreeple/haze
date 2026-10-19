//! Thin Spotify Web API client for library, search and listening history.

use std::time::Duration;

use anyhow::{Result, anyhow};
use reqwest::{Method, StatusCode};
use serde::Deserialize;
use serde::de::DeserializeOwned;

use super::auth::Auth;

const BASE: &str = "https://api.spotify.com/v1";
/// Stop paging long playlists here; keeps memory small and the UI quick.
pub const MAX_LIST: usize = 1000;

/// A refusal from the Web API, kept as a type so callers can tell a 403 from a 404.
#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub message: String,
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ApiError {}

/// The HTTP status of a Web API refusal, if that's what `e` is.
pub fn status(e: &anyhow::Error) -> Option<StatusCode> {
    e.downcast_ref::<ApiError>().map(|e| e.status)
}

#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    pub id: String,
    pub uri: String,
    pub name: String,
    pub artists: Vec<String>,
    pub album: String,
    pub duration_ms: u32,
    pub explicit: bool,
    pub image_url: Option<String>,
}

impl Track {
    pub fn artist_line(&self) -> String {
        self.artists.join(", ")
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Playlist {
    pub id: String,
    pub name: String,
    pub owner: String,
    pub track_count: u32,
    pub image_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Api {
    http: reqwest::Client,
    auth: Auth,
}

impl Api {
    pub fn new(auth: Auth) -> Self {
        let http = reqwest::Client::builder()
            .user_agent(concat!("haze/", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(20))
            .build()
            .expect("HTTP client");
        Self { http, auth }
    }

    async fn get<T: DeserializeOwned>(&self, url: &str) -> Result<T> {
        self.get_as(url, None).await
    }

    /// GET with your login, or with `token` when one is given.
    async fn get_as<T: DeserializeOwned>(&self, url: &str, token: Option<&str>) -> Result<T> {
        let url = if url.starts_with("http") { url.to_string() } else { format!("{BASE}{url}") };
        for attempt in 0..3 {
            let bearer = match token {
                Some(t) => t.to_string(),
                None => self.auth.access_token().await?,
            };
            let resp = self.http.request(Method::GET, &url).bearer_auth(bearer).send().await?;
            match resp.status() {
                StatusCode::TOO_MANY_REQUESTS if attempt < 2 => {
                    let wait = resp
                        .headers()
                        .get("retry-after")
                        .and_then(|v| v.to_str().ok())
                        .and_then(|v| v.parse::<u64>().ok())
                        .unwrap_or(2);
                    // A borrowed login is a fallback; when Spotify wants a long pause, give up so
                    // the caller can say so instead of sitting on "Loading…".
                    if token.is_some() && wait > 5 {
                        return Err(ApiError { status: StatusCode::TOO_MANY_REQUESTS, message: format!("Spotify API asked to wait {wait}s for {url}") }.into());
                    }
                    tokio::time::sleep(Duration::from_secs(wait.min(30))).await;
                }
                s if s.is_success() => return Ok(resp.json().await?),
                s => {
                    let body = resp.text().await.unwrap_or_default();
                    return Err(ApiError { status: s, message: format!("Spotify API {s} for {url}: {}", truncate(&body, 300)) }.into());
                }
            }
        }
        Err(anyhow!("Spotify API kept rate-limiting {url}"))
    }

    pub async fn display_name(&self) -> Result<String> {
        #[derive(Deserialize)]
        struct Me {
            display_name: Option<String>,
            id: String,
        }
        let me: Me = self.get("/me").await?;
        Ok(me.display_name.unwrap_or(me.id))
    }

    pub async fn playlists(&self) -> Result<Vec<Playlist>> {
        let mut out = Vec::new();
        let mut next = Some("/me/playlists?limit=50".to_string());
        while let Some(url) = next.take() {
            let page: Page<Option<RawPlaylist>> = self.get(&url).await?;
            out.extend(page.items.into_iter().flatten().map(|p| Playlist {
                id: p.id,
                name: p.name,
                owner: p.owner.and_then(|o| o.display_name).unwrap_or_default(),
                track_count: p.tracks.or(p.items).map(|t| t.total).unwrap_or(0),
                image_url: pick_image(p.images.as_deref().unwrap_or_default()),
            }));
            if out.len() < MAX_LIST {
                next = page.next;
            }
        }
        Ok(out)
    }

    /// A playlist's songs. `token` reads it with another login (see `Audio::web_token`), for
    /// playlists Spotify won't show to your developer app.
    pub async fn playlist_tracks(&self, playlist_id: &str, token: Option<&str>) -> Result<Vec<Track>> {
        let (mut out, mut next) = self.playlist_first_page(playlist_id, token).await?;
        while let Some(url) = next.take().filter(|_| out.len() < MAX_LIST) {
            let (more, after) = self.track_page(&url, token).await?;
            out.extend(more);
            next = after;
        }
        Ok(out)
    }

    /// The first page of a playlist's songs, and the link to the next page.
    pub async fn playlist_first_page(&self, playlist_id: &str, token: Option<&str>) -> Result<(Vec<Track>, Option<String>)> {
        let id = urlencoding::encode(playlist_id);
        // Newer API versions serve playlist contents under /items; older ones under /tracks. Only
        // a missing endpoint is worth the second try: a refusal (403) would just be refused again.
        match self.track_page(&format!("/playlists/{id}/items?limit=50"), token).await {
            Err(e) if status(&e) == Some(StatusCode::NOT_FOUND) => {
                self.track_page(&format!("/playlists/{id}/tracks?limit=50"), token).await
            }
            r => r,
        }
    }

    /// One page of songs from a paged list, and the link to the next page.
    pub async fn track_page(&self, url: &str, token: Option<&str>) -> Result<(Vec<Track>, Option<String>)> {
        let page: Page<SavedItem> = self.get_as(url, token).await?;
        let tracks = page.items.into_iter().filter_map(|i| i.track.or(i.item)).filter_map(RawTrack::into_track).collect();
        Ok((tracks, page.next))
    }

    pub async fn liked_tracks(&self) -> Result<Vec<Track>> {
        self.paged_tracks("/me/tracks?limit=50".into(), None).await
    }

    async fn paged_tracks(&self, first: String, token: Option<&str>) -> Result<Vec<Track>> {
        let mut out = Vec::new();
        let mut next = Some(first);
        while let Some(url) = next.take() {
            let (more, after) = self.track_page(&url, token).await?;
            out.extend(more);
            if out.len() < MAX_LIST {
                next = after;
            }
        }
        Ok(out)
    }

    pub async fn search_tracks(&self, query: &str, limit: u8) -> Result<Vec<Track>> {
        #[derive(Deserialize)]
        struct Search {
            tracks: Page<RawTrack>,
        }
        let url = format!(
            "/search?type=track&limit={}&q={}",
            limit.clamp(1, 10),
            urlencoding::encode(query)
        );
        let res: Search = self.get(&url).await?;
        Ok(res.tracks.items.into_iter().filter_map(RawTrack::into_track).collect())
    }

    /// Finds the best match for a song the DJ named.
    pub async fn find_track(&self, title: &str, artist: &str) -> Result<Option<Track>> {
        let precise = format!("track:{title} artist:{artist}");
        let mut hits = self.search_tracks(&precise, 5).await?;
        if hits.is_empty() {
            hits = self.search_tracks(&format!("{title} {artist}"), 5).await?;
        }
        let want_artist = artist.to_lowercase();
        let best = hits
            .iter()
            .position(|t| t.artists.iter().any(|a| want_artist.contains(&a.to_lowercase()) || a.to_lowercase().contains(&want_artist)))
            .unwrap_or(0);
        Ok(if hits.is_empty() { None } else { Some(hits.swap_remove(best)) })
    }

    pub async fn top_tracks(&self, time_range: &str, limit: u8) -> Result<Vec<Track>> {
        let page: Page<RawTrack> =
            self.get(&format!("/me/top/tracks?time_range={time_range}&limit={limit}")).await?;
        Ok(page.items.into_iter().filter_map(RawTrack::into_track).collect())
    }

    pub async fn top_artists(&self, limit: u8) -> Result<Vec<String>> {
        #[derive(Deserialize)]
        struct Artist {
            name: String,
            #[serde(default)]
            genres: Vec<String>,
        }
        let page: Page<Artist> =
            self.get(&format!("/me/top/artists?time_range=medium_term&limit={limit}")).await?;
        Ok(page
            .items
            .into_iter()
            .map(|a| {
                if a.genres.is_empty() {
                    a.name
                } else {
                    format!("{} ({})", a.name, a.genres.into_iter().take(3).collect::<Vec<_>>().join(", "))
                }
            })
            .collect())
    }

    pub async fn recently_played(&self, limit: u8) -> Result<Vec<Track>> {
        let page: Page<SavedItem> =
            self.get(&format!("/me/player/recently-played?limit={limit}")).await?;
        Ok(page.items.into_iter().filter_map(|i| i.track.or(i.item)).filter_map(RawTrack::into_track).collect())
    }

    /// Downloads cover art (small sizes only, to keep memory low).
    pub async fn fetch_image(&self, url: &str) -> Result<Vec<u8>> {
        Ok(self.http.get(url).send().await?.error_for_status()?.bytes().await?.to_vec())
    }
}

fn truncate(s: &str, n: usize) -> &str {
    match s.char_indices().nth(n) {
        Some((i, _)) => &s[..i],
        None => s,
    }
}

#[derive(Deserialize)]
struct Page<T> {
    #[serde(default = "Vec::new")]
    items: Vec<T>,
    #[serde(default)]
    next: Option<String>,
}

#[derive(Deserialize)]
struct SavedItem {
    #[serde(default)]
    track: Option<RawTrack>,
    #[serde(default)]
    item: Option<RawTrack>,
}

#[derive(Deserialize)]
struct RawPlaylist {
    id: String,
    name: String,
    owner: Option<RawOwner>,
    tracks: Option<Total>,
    items: Option<Total>,
    images: Option<Vec<RawImage>>,
}

#[derive(Deserialize)]
struct RawOwner {
    display_name: Option<String>,
}

#[derive(Deserialize)]
struct Total {
    total: u32,
}

#[derive(Deserialize)]
struct RawTrack {
    id: Option<String>,
    uri: Option<String>,
    name: Option<String>,
    #[serde(rename = "type", default)]
    kind: Option<String>,
    #[serde(default)]
    duration_ms: u32,
    #[serde(default)]
    explicit: bool,
    #[serde(default)]
    is_local: bool,
    #[serde(default)]
    artists: Vec<RawArtist>,
    album: Option<RawAlbum>,
}

#[derive(Deserialize)]
struct RawArtist {
    name: String,
}

#[derive(Deserialize)]
struct RawAlbum {
    name: String,
    #[serde(default)]
    images: Vec<RawImage>,
}

#[derive(Deserialize)]
struct RawImage {
    url: String,
    width: Option<u32>,
}

/// The smallest image that is still sharp on the lyrics page and playlist headers (300px).
fn pick_image(images: &[RawImage]) -> Option<String> {
    let mut imgs: Vec<&RawImage> = images.iter().collect();
    imgs.sort_by_key(|i| i.width.unwrap_or(640));
    imgs.iter().find(|i| i.width.unwrap_or(640) >= 300).or(imgs.last()).map(|i| i.url.clone())
}

impl RawTrack {
    fn into_track(self) -> Option<Track> {
        if self.is_local || self.kind.as_deref().is_some_and(|k| k != "track") {
            return None;
        }
        let album = self.album;
        let image_url = album.as_ref().and_then(|a| pick_image(&a.images));
        Some(Track {
            id: self.id?,
            uri: self.uri?,
            name: self.name.unwrap_or_default(),
            artists: self.artists.into_iter().map(|a| a.name).collect(),
            album: album.map(|a| a.name).unwrap_or_default(),
            duration_ms: self.duration_ms,
            explicit: self.explicit,
            image_url,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_playlist_page_old_and_new_shapes() {
        let json = r#"{"items":[
            {"track":{"id":"a","uri":"spotify:track:a","name":"One","type":"track","duration_ms":1000,
              "artists":[{"name":"X"}],"album":{"name":"Al","images":[
                {"url":"big","width":640},{"url":"mid","width":300},{"url":"small","width":64}]}}},
            {"item":{"id":"b","uri":"spotify:track:b","name":"Two","type":"track","artists":[]}},
            {"track":{"id":"c","uri":"spotify:episode:c","name":"Pod","type":"episode"}},
            {"track":null}
        ],"next":null}"#;
        let page: Page<SavedItem> = serde_json::from_str(json).unwrap();
        let tracks: Vec<Track> =
            page.items.into_iter().filter_map(|i| i.track.or(i.item)).filter_map(RawTrack::into_track).collect();
        assert_eq!(tracks.len(), 2);
        assert_eq!(tracks[0].image_url.as_deref(), Some("mid"));
        assert_eq!(tracks[1].name, "Two");
    }
}
