//! Spotify Connect player control.
//! Currently playing, queue track, skip, seek.

use anyhow::Context;

pub struct SpotifyMe {
    pub id: String,
    pub display_name: String,
}

pub struct ActiveDevice {
    pub name: String,
    pub device_type: String,
    pub is_active: bool,
}

#[derive(serde::Deserialize)]
struct RawMe {
    id: String,
    display_name: Option<String>,
}

#[derive(serde::Deserialize)]
struct PlayerState {
    device: Option<RawDevice>,
}

#[derive(serde::Deserialize)]
struct RawDevice {
    name: String,
    #[serde(rename = "type")]
    device_type: String,
    is_active: bool,
}

pub async fn get_me(access_token: &str) -> anyhow::Result<SpotifyMe> {
    let raw = reqwest::Client::new()
        .get("https://api.spotify.com/v1/me")
        .bearer_auth(access_token)
        .send()
        .await
        .context("fetching /v1/me")?
        .error_for_status()
        .context("Spotify /v1/me returned error")?
        .json::<RawMe>()
        .await
        .context("parsing /v1/me response")?;
    Ok(SpotifyMe {
        id: raw.id,
        display_name: raw.display_name.unwrap_or_default(),
    })
}

pub async fn get_player(access_token: &str) -> anyhow::Result<Option<ActiveDevice>> {
    let resp = reqwest::Client::new()
        .get("https://api.spotify.com/v1/me/player")
        .bearer_auth(access_token)
        .send()
        .await
        .context("fetching /v1/me/player")?;

    if resp.status() == 204 {
        return Ok(None);
    }

    let state = resp
        .error_for_status()
        .context("Spotify /v1/me/player returned error")?
        .json::<PlayerState>()
        .await
        .context("parsing /v1/me/player response")?;

    Ok(state.device.map(|d| ActiveDevice {
        name: d.name,
        device_type: d.device_type,
        is_active: d.is_active,
    }))
}

// Car mode stubs — used by the car crate

pub async fn currently_playing(_access_token: &str) -> anyhow::Result<Option<String>> {
    todo!()
}

pub async fn queue_track(_access_token: &str, _track_uri: &str) -> anyhow::Result<()> {
    todo!()
}

pub async fn skip(_access_token: &str) -> anyhow::Result<()> {
    todo!()
}
