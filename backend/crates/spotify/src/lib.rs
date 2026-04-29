//! Spotify API client.
//! Handles OAuth token exchange/refresh and all Spotify Connect API calls.

pub mod auth;
pub mod player;
pub mod playlist;

pub(crate) static LOG_CALLS: std::sync::LazyLock<bool> =
    std::sync::LazyLock::new(|| std::env::var("SPOTIFY_LOG_CALLS").is_ok());

#[derive(Debug)]
pub(crate) struct SpotifyApiError {
    pub operation: &'static str,
    pub status: reqwest::StatusCode,
    pub retry_after: Option<String>,
    pub body: String,
}

impl std::fmt::Display for SpotifyApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Spotify {} returned {}", self.operation, self.status)?;
        if let Some(retry_after) = &self.retry_after {
            write!(f, " (retry after {retry_after}s)")?;
        }
        if !self.body.is_empty() {
            write!(f, ": {}", self.body)?;
        }
        Ok(())
    }
}

impl std::error::Error for SpotifyApiError {}

pub(crate) async fn spotify_error(operation: &'static str, resp: reqwest::Response) -> anyhow::Error {
    let status = resp.status();
    let retry_after = resp
        .headers()
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|v| v.to_str().ok())
        .map(ToOwned::to_owned);
    let body = resp.text().await.unwrap_or_default();
    SpotifyApiError { operation, status, retry_after, body }.into()
}
