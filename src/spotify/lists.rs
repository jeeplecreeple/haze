//! Reads playlists through the playback login, the way Spotify's own apps do.
//!
//! Since 2026 the Web API only shows a developer app the songs of playlists you own or help
//! edit, so other people's playlists (and Spotify's own) come back "403 Forbidden". Spotify's
//! apps read them from its internal playlist and metadata services instead, which also answer
//! a hundred songs per request rather than fifty, without the Web API's rate limits.

use anyhow::{Result, anyhow};
use librespot_core::session::Session;
use librespot_core::spotify_id::SpotifyId;
use librespot_protocol::extended_metadata::{BatchedEntityRequest, EntityRequest, ExtensionQuery};
use librespot_protocol::extension_kind::ExtensionKind;
use librespot_protocol::metadata;
use librespot_protocol::playlist4_external::SelectedListContent;
use protobuf::{EnumOrUnknown, Message};
use reqwest::Method;

use super::api::Track;

/// Songs asked for per metadata request.
pub const CHUNK: usize = 100;
/// Same cap as the Web API reader.
const MAX_LIST: usize = 1000;

/// The song links in a playlist, in order (podcast episodes and local files are skipped).
pub async fn playlist_uris(session: &Session, playlist_id: &str) -> Result<Vec<String>> {
    let id = SpotifyId::from_base62(playlist_id).map_err(|e| anyhow!("bad playlist id {playlist_id}: {e}"))?;
    let id = id.to_base62().map_err(|e| anyhow!("{e}"))?;
    let mut uris = Vec::new();
    let mut from = 0usize;
    loop {
        let endpoint = format!("/playlist/v2/playlist/{id}?from={from}&length={CHUNK}");
        let bytes = session
            .spclient()
            .request(&Method::GET, &endpoint, None, None)
            .await
            .map_err(|e| anyhow!("Spotify playlist service: {e}"))?;
        let list = SelectedListContent::parse_from_bytes(&bytes)?;
        let items = &list.contents.items;
        let got = items.len();
        // Some answers ignore the range and send the whole list again; stop rather than repeat it.
        if from > 0 && list.contents.pos() == 0 {
            break;
        }
        uris.extend(items.iter().map(|i| i.uri().to_string()).filter(|u| u.starts_with("spotify:track:")));
        from += got;
        let length = list.length.unwrap_or(0).max(0) as usize;
        if got == 0 || from >= length || from >= MAX_LIST || !list.contents.truncated() {
            break;
        }
    }
    uris.truncate(MAX_LIST);
    Ok(uris)
}

/// Song details for up to [`CHUNK`] song links, in the same order (songs Spotify has nothing on are left out).
pub async fn tracks(session: &Session, uris: &[String]) -> Result<Vec<Track>> {
    let request = BatchedEntityRequest {
        entity_request: uris
            .iter()
            .map(|uri| EntityRequest {
                entity_uri: uri.clone(),
                query: vec![ExtensionQuery { extension_kind: EnumOrUnknown::new(ExtensionKind::TRACK_V4), ..Default::default() }],
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    };
    let response = session
        .spclient()
        .get_extended_metadata(request)
        .await
        .map_err(|e| anyhow!("Spotify song details: {e}"))?;
    let mut found = std::collections::HashMap::new();
    for entity in response.extended_metadata {
        for data in entity.extension_data {
            let Some(any) = data.extension_data.as_ref() else { continue };
            if let Ok(track) = metadata::Track::parse_from_bytes(&any.value) {
                found.insert(data.entity_uri.clone(), track);
            }
        }
    }
    Ok(uris.iter().filter_map(|uri| found.get(uri).and_then(|t| to_track(uri, t))).collect())
}

fn to_track(uri: &str, t: &metadata::Track) -> Option<Track> {
    let id = uri.strip_prefix("spotify:track:")?.to_string();
    let album = t.album.as_ref();
    let image_url = album.and_then(|a| {
        let images = if a.cover_group.image.is_empty() { &a.cover } else { &a.cover_group.image };
        // DEFAULT is the 300 px size, the one the Web API reader picks too.
        let pick = images
            .iter()
            .find(|i| i.size() == metadata::image::Size::DEFAULT)
            .or_else(|| images.iter().find(|i| i.size() == metadata::image::Size::LARGE))
            .or(images.first())?;
        let hex: String = pick.file_id().iter().map(|b| format!("{b:02x}")).collect();
        (!hex.is_empty()).then(|| format!("https://i.scdn.co/image/{hex}"))
    });
    Some(Track {
        id,
        uri: uri.to_string(),
        name: t.name().to_string(),
        artists: t.artist.iter().map(|a| a.name().to_string()).collect(),
        album: album.map(|a| a.name().to_string()).unwrap_or_default(),
        duration_ms: t.duration().max(0) as u32,
        explicit: t.explicit(),
        image_url,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn song_details_become_tracks() {
        let mut t = metadata::Track::new();
        t.set_name("Song".into());
        t.set_duration(201_000);
        t.set_explicit(true);
        let mut artist = metadata::Artist::new();
        artist.set_name("Someone".into());
        t.artist.push(artist);
        let mut album = metadata::Album::new();
        album.set_name("Record".into());
        let mut small = metadata::Image::new();
        small.set_size(metadata::image::Size::SMALL);
        small.set_file_id(vec![0x01]);
        let mut normal = metadata::Image::new();
        normal.set_size(metadata::image::Size::DEFAULT);
        normal.set_file_id(vec![0xab, 0x0c]);
        album.cover_group.mut_or_insert_default().image.extend([small, normal]);
        t.album = Some(album).into();
        // Round-trip through bytes, as it arrives from Spotify.
        let t = metadata::Track::parse_from_bytes(&t.write_to_bytes().unwrap()).unwrap();

        let track = to_track("spotify:track:abc", &t).unwrap();
        assert_eq!(track.id, "abc");
        assert_eq!(track.name, "Song");
        assert_eq!(track.artist_line(), "Someone");
        assert_eq!(track.album, "Record");
        assert_eq!(track.duration_ms, 201_000);
        assert!(track.explicit);
        assert_eq!(track.image_url.as_deref(), Some("https://i.scdn.co/image/ab0c"));
        assert!(to_track("spotify:episode:abc", &t).is_none());
    }
}
