//! OAuth 2.0 token exchange and refresh.

use anyhow::{bail, Context};

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
    let response = reqwest::Client::new()
        .post("https://accounts.spotify.com/api/token")
        .basic_auth(client_id, Some(client_secret))
        .form(&params)
        .send()
        .await
        .context("sending token request")?;
    let raw = parse_token_response(response, "token")
        .await?
        .json::<RawTokenResponse>()
        .await
        .context("parsing token response")?;
    Ok(TokenResponse {
        access_token: raw.access_token,
        refresh_token: raw.refresh_token,
        expires_in: raw.expires_in,
    })
}

pub async fn exchange_code_pkce(
    code: &str,
    client_id: &str,
    code_verifier: &str,
    redirect_uri: &str,
) -> anyhow::Result<TokenResponse> {
    let params = [
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", redirect_uri),
        ("client_id", client_id),
        ("code_verifier", code_verifier),
    ];
    let response = reqwest::Client::new()
        .post("https://accounts.spotify.com/api/token")
        .form(&params)
        .send()
        .await
        .context("sending PKCE token request")?;
    let raw = parse_token_response(response, "PKCE token")
        .await?
        .json::<RawTokenResponse>()
        .await
        .context("parsing PKCE token response")?;
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
    let response = reqwest::Client::new()
        .post("https://accounts.spotify.com/api/token")
        .basic_auth(client_id, Some(client_secret))
        .form(&params)
        .send()
        .await
        .context("sending refresh request")?;
    let raw = parse_token_response(response, "refresh")
        .await?
        .json::<RawTokenResponse>()
        .await
        .context("parsing refresh response")?;
    Ok(TokenResponse {
        access_token: raw.access_token,
        refresh_token: raw.refresh_token,
        expires_in: raw.expires_in,
    })
}

pub async fn refresh_token_pkce(
    refresh_token: &str,
    client_id: &str,
) -> anyhow::Result<TokenResponse> {
    let params = [
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token),
        ("client_id", client_id),
    ];
    let response = reqwest::Client::new()
        .post("https://accounts.spotify.com/api/token")
        .form(&params)
        .send()
        .await
        .context("sending PKCE refresh request")?;
    let raw = parse_token_response(response, "PKCE refresh")
        .await?
        .json::<RawTokenResponse>()
        .await
        .context("parsing PKCE refresh response")?;
    Ok(TokenResponse {
        access_token: raw.access_token,
        refresh_token: raw.refresh_token,
        expires_in: raw.expires_in,
    })
}

async fn parse_token_response(
    response: reqwest::Response,
    request_kind: &str,
) -> anyhow::Result<reqwest::Response> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }

    let body = response
        .text()
        .await
        .unwrap_or_else(|e| format!("<failed to read response body: {e}>"));
    bail!("Spotify {request_kind} endpoint returned {status}: {body}");
}
