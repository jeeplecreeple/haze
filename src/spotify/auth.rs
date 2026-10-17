//! Spotify login with OAuth (PKCE) against the user's own client ID.
//! The refresh token is cached on disk so later launches log in silently.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow};
use librespot_oauth::{OAuthClient, OAuthClientBuilder, OAuthToken};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::config::{self, SpotifyConfig};

const SCOPES: &[&str] = &[
    "streaming",
    "user-read-private",
    "user-read-email",
    "user-library-read",
    "user-library-modify",
    "playlist-read-private",
    "playlist-read-collaborative",
    "user-top-read",
    "user-read-recently-played",
];

const LOGIN_PAGE: &str = "<html><body style=\"font-family:sans-serif;background:#0f0f14;color:#f1f1f5;\
text-align:center;padding-top:20vh\"><h1>Haze is logged in</h1><p>You can close this tab.</p></body></html>";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredToken {
    access_token: String,
    refresh_token: String,
    /// Seconds since the Unix epoch.
    expires_at: u64,
}

impl StoredToken {
    fn from_oauth(token: OAuthToken, previous_refresh: Option<&str>) -> Self {
        let remaining = token.expires_at.saturating_duration_since(std::time::Instant::now());
        let refresh_token = if token.refresh_token.is_empty() {
            previous_refresh.unwrap_or_default().to_string()
        } else {
            token.refresh_token
        };
        Self {
            access_token: token.access_token,
            refresh_token,
            expires_at: now_secs() + remaining.as_secs(),
        }
    }

    fn expires_soon(&self) -> bool {
        now_secs() + 60 >= self.expires_at
    }
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or(Duration::ZERO).as_secs()
}

fn token_path() -> Result<PathBuf> {
    Ok(config::cache_dir()?.join("spotify-token.json"))
}

/// Shared access-token source for the Web API and the audio session.
#[derive(Clone)]
pub struct Auth {
    settings: SpotifyConfig,
    token: Arc<Mutex<StoredToken>>,
}

impl std::fmt::Debug for Auth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Auth")
    }
}

impl Auth {
    fn client(settings: &SpotifyConfig) -> Result<OAuthClient> {
        OAuthClientBuilder::new(&settings.client_id, &settings.redirect_uri, SCOPES.to_vec())
            .open_in_browser()
            .with_custom_message(LOGIN_PAGE)
            .build()
            .map_err(|e| anyhow!("OAuth setup failed: {e}"))
    }

    /// Whether a previous login can be reused without opening the browser.
    pub fn has_cached_login() -> bool {
        token_path().map(|p| p.exists()).unwrap_or(false)
    }

    /// Logs in from the cached refresh token if there is one, else opens the browser.
    pub async fn login(settings: SpotifyConfig) -> Result<Self> {
        if config::non_empty(&settings.client_id).is_none() {
            return Err(anyhow!("Set spotify.client_id in your config first"));
        }

        if let Some(cached) = read_cached() {
            let client = Self::client(&settings)?;
            match client.refresh_token_async(&cached.refresh_token).await {
                Ok(token) => {
                    let stored = StoredToken::from_oauth(token, Some(&cached.refresh_token));
                    write_cached(&stored)?;
                    return Ok(Self { settings, token: Arc::new(Mutex::new(stored)) });
                }
                Err(e) => log::warn!("Cached Spotify login expired ({e}); logging in again"),
            }
        }

        let interactive = settings.clone();
        let token = tokio::task::spawn_blocking(move || {
            Self::client(&interactive)?
                .get_access_token()
                .map_err(|e| anyhow!("Spotify login failed: {e}"))
        })
        .await??;
        let stored = StoredToken::from_oauth(token, None);
        write_cached(&stored)?;
        Ok(Self { settings, token: Arc::new(Mutex::new(stored)) })
    }

    /// A valid access token, refreshed first when it is about to expire.
    pub async fn access_token(&self) -> Result<String> {
        let mut token = self.token.lock().await;
        if token.expires_soon() {
            let fresh = Self::client(&self.settings)?
                .refresh_token_async(&token.refresh_token)
                .await
                .map_err(|e| anyhow!("refreshing Spotify token: {e}"))?;
            *token = StoredToken::from_oauth(fresh, Some(&token.refresh_token));
            write_cached(&token)?;
        }
        Ok(token.access_token.clone())
    }

    pub fn logout() -> Result<()> {
        let path = token_path()?;
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        Ok(())
    }
}

fn read_cached() -> Option<StoredToken> {
    let text = std::fs::read_to_string(token_path().ok()?).ok()?;
    serde_json::from_str::<StoredToken>(&text).ok().filter(|t| !t.refresh_token.is_empty())
}

fn write_cached(token: &StoredToken) -> Result<()> {
    let path = token_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, serde_json::to_vec(token)?).context("saving Spotify login")?;
    config::restrict_permissions(&path);
    Ok(())
}
