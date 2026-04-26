//! OAuth 2.0 token exchange and refresh.

use anyhow::Context;

pub struct TokenResponse {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_in: u64,
}

#[derive(serde::Deserialize)]
struct RawTokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: u64,
}

pub async fn exchange_code(
    code: &str,
    client_id: &str,
    client_secret: &str,
    redirect_uri: &str,
) -> anyhow::Result<TokenResponse> {
    let params = [
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", redirect_uri),
    ];
    let raw = reqwest::Client::new()
        .post("https://accounts.spotify.com/api/token")
        .basic_auth(client_id, Some(client_secret))
        .form(&params)
        .send()
        .await
        .context("sending token request")?
        .error_for_status()
        .context("Spotify token endpoint returned error")?
        .json::<RawTokenResponse>()
        .await
        .context("parsing token response")?;
    Ok(TokenResponse {
        access_token: raw.access_token,
        refresh_token: raw.refresh_token,
        expires_in: raw.expires_in,
    })
}

pub async fn refresh_token(
    refresh_token: &str,
    client_id: &str,
    client_secret: &str,
) -> anyhow::Result<TokenResponse> {
    let params = [
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token),
    ];
    let raw = reqwest::Client::new()
        .post("https://accounts.spotify.com/api/token")
        .basic_auth(client_id, Some(client_secret))
        .form(&params)
        .send()
        .await
        .context("sending refresh request")?
        .error_for_status()
        .context("Spotify token endpoint returned error")?
        .json::<RawTokenResponse>()
        .await
        .context("parsing refresh response")?;
    Ok(TokenResponse {
        access_token: raw.access_token,
        refresh_token: raw.refresh_token,
        expires_in: raw.expires_in,
    })
}
