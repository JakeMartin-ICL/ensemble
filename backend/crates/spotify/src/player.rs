//! Spotify Connect player control.

use anyhow::Context;

pub struct SpotifyMe {
    pub id: String,
    pub display_name: String,
}

pub struct ActiveDevice {
    pub id: Option<String>,
    pub name: String,
    pub device_type: String,
    pub is_active: bool,
    pub is_restricted: bool,
}

pub struct PlaybackState {
    pub track_uri: String,
    pub progress_ms: u64,
    pub duration_ms: u64,
    pub is_playing: bool,
}

#[derive(serde::Deserialize)]
struct RawMe {
    id: String,
    display_name: Option<String>,
}

#[derive(serde::Deserialize)]
struct RawPlayerState {
    device: Option<RawDevice>,
    item: Option<RawTrackItem>,
    progress_ms: Option<u64>,
    is_playing: bool,
}

#[derive(serde::Deserialize)]
struct RawDevice {
    id: Option<String>,
    name: String,
    #[serde(rename = "type")]
    device_type: String,
    is_active: bool,
    #[serde(default)]
    is_restricted: bool,
}

#[derive(serde::Deserialize)]
struct RawTrackItem {
    uri: String,
    duration_ms: u64,
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
        .json::<RawPlayerState>()
        .await
        .context("parsing /v1/me/player response")?;

    Ok(state.device.map(|d| ActiveDevice {
        id: d.id,
        name: d.name,
        device_type: d.device_type,
        is_active: d.is_active,
        is_restricted: d.is_restricted,
    }))
}

#[derive(serde::Deserialize)]
struct RawDevicesResponse {
    devices: Vec<RawDevice>,
}

pub async fn get_available_devices(access_token: &str) -> anyhow::Result<Vec<ActiveDevice>> {
    let devices = reqwest::Client::new()
        .get("https://api.spotify.com/v1/me/player/devices")
        .bearer_auth(access_token)
        .send()
        .await
        .context("fetching /v1/me/player/devices")?
        .error_for_status()
        .context("Spotify /v1/me/player/devices returned error")?
        .json::<RawDevicesResponse>()
        .await
        .context("parsing /v1/me/player/devices response")?;

    Ok(devices
        .devices
        .into_iter()
        .map(|d| ActiveDevice {
            id: d.id,
            name: d.name,
            device_type: d.device_type,
            is_active: d.is_active,
            is_restricted: d.is_restricted,
        })
        .collect())
}

pub async fn get_playback_state(access_token: &str) -> anyhow::Result<Option<PlaybackState>> {
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
        .json::<RawPlayerState>()
        .await
        .context("parsing /v1/me/player response")?;

    let Some(item) = state.item else {
        return Ok(None);
    };
    let progress_ms = state.progress_ms.unwrap_or(0);

    Ok(Some(PlaybackState {
        track_uri: item.uri,
        progress_ms,
        duration_ms: item.duration_ms,
        is_playing: state.is_playing,
    }))
}

pub async fn queue_track(access_token: &str, track_uri: &str) -> anyhow::Result<()> {
    let resp = reqwest::Client::new()
        .post("https://api.spotify.com/v1/me/player/queue")
        .bearer_auth(access_token)
        .query(&[("uri", track_uri)])
        .header(reqwest::header::CONTENT_LENGTH, "0")
        .body("")
        .send()
        .await
        .context("queuing track")?;

    // 204 = success, 404 = no active device (not an error we can do anything about)
    if resp.status() == 204 || resp.status() == 404 {
        return Ok(());
    }

    resp.error_for_status()
        .context("Spotify queue endpoint returned error")?;
    Ok(())
}

#[derive(serde::Serialize)]
struct PlayRequest {
    uris: Vec<String>,
}

pub async fn start_track(access_token: &str, track_uri: &str) -> anyhow::Result<()> {
    start_tracks(access_token, &[track_uri.to_string()]).await
}

pub async fn resume_playback(access_token: &str) -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    let resp = client
        .put("https://api.spotify.com/v1/me/player/play")
        .bearer_auth(access_token)
        .header(reqwest::header::CONTENT_LENGTH, "0")
        .body("")
        .send()
        .await
        .context("resuming playback")?;

    if resp.status().is_success() {
        return Ok(());
    }

    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();

    if status != reqwest::StatusCode::NOT_FOUND {
        anyhow::bail!("Spotify resume playback returned {status}: {body}");
    }

    let devices = get_available_devices(access_token).await?;
    let Some(device) = devices
        .iter()
        .filter(|d| !d.is_restricted)
        .find(|d| d.is_active && d.id.is_some())
        .or_else(|| devices.iter().find(|d| !d.is_restricted && d.id.is_some()))
    else {
        anyhow::bail!(
            "Spotify has no available playback device. Open Spotify on a phone, desktop app, or web player, then try again."
        );
    };

    let Some(device_id) = device.id.as_deref() else {
        anyhow::bail!(
            "Spotify found a playback device but did not provide a controllable device id. Open Spotify on another device, then try again."
        );
    };

    let resp = client
        .put("https://api.spotify.com/v1/me/player/play")
        .bearer_auth(access_token)
        .query(&[("device_id", device_id)])
        .header(reqwest::header::CONTENT_LENGTH, "0")
        .body("")
        .send()
        .await
        .context("resuming playback on available device")?;

    if resp.status().is_success() {
        return Ok(());
    }

    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    anyhow::bail!("Spotify resume playback on available device returned {status}: {body}");
}

pub async fn pause_playback(access_token: &str) -> anyhow::Result<()> {
    let resp = reqwest::Client::new()
        .put("https://api.spotify.com/v1/me/player/pause")
        .bearer_auth(access_token)
        .header(reqwest::header::CONTENT_LENGTH, "0")
        .body("")
        .send()
        .await
        .context("pausing playback")?;

    if resp.status().is_success() {
        return Ok(());
    }

    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    anyhow::bail!("Spotify pause playback returned {status}: {body}");
}

pub async fn seek_to_start(access_token: &str) -> anyhow::Result<()> {
    let resp = reqwest::Client::new()
        .put("https://api.spotify.com/v1/me/player/seek")
        .bearer_auth(access_token)
        .query(&[("position_ms", "0")])
        .header(reqwest::header::CONTENT_LENGTH, "0")
        .body("")
        .send()
        .await
        .context("seeking playback")?;

    if resp.status().is_success() {
        return Ok(());
    }

    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    anyhow::bail!("Spotify seek playback returned {status}: {body}");
}

pub async fn start_tracks(access_token: &str, track_uris: &[String]) -> anyhow::Result<()> {
    if track_uris.is_empty() {
        anyhow::bail!("cannot start playback with no tracks");
    }

    let client = reqwest::Client::new();
    let resp = client
        .put("https://api.spotify.com/v1/me/player/play")
        .bearer_auth(access_token)
        .json(&PlayRequest {
            uris: track_uris.to_vec(),
        })
        .send()
        .await
        .context("starting playback")?;

    if resp.status().is_success() {
        return Ok(());
    }

    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();

    if status != reqwest::StatusCode::NOT_FOUND {
        anyhow::bail!("Spotify start playback returned {status}: {body}");
    }

    let devices = get_available_devices(access_token).await?;
    let Some(device) = devices
        .iter()
        .filter(|d| !d.is_restricted)
        .find(|d| d.is_active && d.id.is_some())
        .or_else(|| devices.iter().find(|d| !d.is_restricted && d.id.is_some()))
    else {
        anyhow::bail!(
            "Spotify has no available playback device. Open Spotify on a phone, desktop app, or web player, then try again."
        );
    };

    let Some(device_id) = device.id.as_deref() else {
        anyhow::bail!(
            "Spotify found a playback device but did not provide a controllable device id. Open Spotify on another device, then try again."
        );
    };

    let resp = client
        .put("https://api.spotify.com/v1/me/player/play")
        .bearer_auth(access_token)
        .query(&[("device_id", device_id)])
        .json(&PlayRequest {
            uris: track_uris.to_vec(),
        })
        .send()
        .await
        .context("starting playback on available device")?;

    if resp.status().is_success() {
        tracing::info!(
            device_name = device.name,
            device_type = device.device_type,
            "started Spotify playback on available device"
        );
        return Ok(());
    }

    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    anyhow::bail!("Spotify start playback on available device returned {status}: {body}");
}

pub struct TrackDetails {
    pub name: String,
    pub artist: String,
    pub album_art_url: Option<String>,
    pub duration_ms: u64,
}

#[derive(serde::Deserialize)]
struct RawTrackDetails {
    name: String,
    artists: Vec<RawArtist>,
    album: RawAlbum,
    duration_ms: u64,
}

#[derive(serde::Deserialize)]
struct RawArtist {
    name: String,
}

#[derive(serde::Deserialize)]
struct RawAlbum {
    images: Vec<RawImage>,
}

#[derive(serde::Deserialize)]
struct RawImage {
    url: String,
}

pub async fn get_track(access_token: &str, track_id: &str) -> anyhow::Result<TrackDetails> {
    let raw = reqwest::Client::new()
        .get(format!("https://api.spotify.com/v1/tracks/{track_id}"))
        .bearer_auth(access_token)
        .send()
        .await
        .context("fetching track")?
        .error_for_status()
        .context("Spotify /v1/tracks returned error")?
        .json::<RawTrackDetails>()
        .await
        .context("parsing track response")?;

    Ok(TrackDetails {
        name: raw.name,
        artist: raw
            .artists
            .into_iter()
            .map(|a| a.name)
            .collect::<Vec<_>>()
            .join(", "),
        album_art_url: raw.album.images.into_iter().next().map(|i| i.url),
        duration_ms: raw.duration_ms,
    })
}
