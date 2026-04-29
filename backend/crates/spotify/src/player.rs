//! Spotify Connect player control.

use anyhow::Context;

macro_rules! log_call {
    ($method:literal, $url:expr) => {
        if *crate::LOG_CALLS {
            tracing::info!("Spotify API: {} {}", $method, $url);
        }
    };
}

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
    const URL: &str = "https://api.spotify.com/v1/me";
    log_call!("GET", URL);
    let resp = reqwest::Client::new()
        .get(URL)
        .bearer_auth(access_token)
        .send()
        .await
        .context("fetching /v1/me")?;

    if !resp.status().is_success() {
        return Err(crate::spotify_error("/v1/me", resp).await);
    }

    let raw = resp
        .json::<RawMe>()
        .await
        .context("parsing /v1/me response")?;

    Ok(SpotifyMe {
        id: raw.id,
        display_name: raw.display_name.unwrap_or_default(),
    })
}

pub async fn get_player(access_token: &str) -> anyhow::Result<Option<ActiveDevice>> {
    const URL: &str = "https://api.spotify.com/v1/me/player";
    log_call!("GET", URL);
    let resp = reqwest::Client::new()
        .get(URL)
        .bearer_auth(access_token)
        .send()
        .await
        .context("fetching /v1/me/player")?;

    if resp.status() == 204 {
        return Ok(None);
    }

    if !resp.status().is_success() {
        return Err(crate::spotify_error("/v1/me/player", resp).await);
    }

    let state = resp
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
    const URL: &str = "https://api.spotify.com/v1/me/player/devices";
    log_call!("GET", URL);
    let resp = reqwest::Client::new()
        .get(URL)
        .bearer_auth(access_token)
        .send()
        .await
        .context("fetching /v1/me/player/devices")?;

    if !resp.status().is_success() {
        return Err(crate::spotify_error("/v1/me/player/devices", resp).await);
    }

    let devices = resp
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
    const URL: &str = "https://api.spotify.com/v1/me/player";
    log_call!("GET", URL);
    let resp = reqwest::Client::new()
        .get(URL)
        .bearer_auth(access_token)
        .send()
        .await
        .context("fetching /v1/me/player")?;

    if resp.status() == 204 {
        return Ok(None);
    }

    if !resp.status().is_success() {
        return Err(crate::spotify_error("/v1/me/player", resp).await);
    }

    let state = resp
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
    const URL: &str = "https://api.spotify.com/v1/me/player/queue";
    log_call!("POST", URL);
    let resp = reqwest::Client::new()
        .post(URL)
        .bearer_auth(access_token)
        .query(&[("uri", track_uri)])
        .header(reqwest::header::CONTENT_LENGTH, "0")
        .body("")
        .send()
        .await
        .context("queuing track")?;

    if resp.status() == 204 {
        return Ok(());
    }

    if !resp.status().is_success() {
        return Err(crate::spotify_error("/v1/me/player/queue", resp).await);
    }

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
    const URL: &str = "https://api.spotify.com/v1/me/player/play";
    log_call!("PUT", URL);
    let client = reqwest::Client::new();
    let resp = client
        .put(URL)
        .bearer_auth(access_token)
        .header(reqwest::header::CONTENT_LENGTH, "0")
        .body("")
        .send()
        .await
        .context("resuming playback")?;

    if resp.status().is_success() {
        return Ok(());
    }

    if resp.status() != reqwest::StatusCode::NOT_FOUND {
        return Err(crate::spotify_error("/v1/me/player/play (resume)", resp).await);
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

    log_call!("PUT", format!("{URL}?device_id={device_id}"));
    let resp = client
        .put(URL)
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

    Err(crate::spotify_error("/v1/me/player/play (resume on device)", resp).await)
}

pub async fn pause_playback(access_token: &str) -> anyhow::Result<()> {
    const URL: &str = "https://api.spotify.com/v1/me/player/pause";
    log_call!("PUT", URL);
    let resp = reqwest::Client::new()
        .put(URL)
        .bearer_auth(access_token)
        .header(reqwest::header::CONTENT_LENGTH, "0")
        .body("")
        .send()
        .await
        .context("pausing playback")?;

    if resp.status().is_success() {
        return Ok(());
    }

    Err(crate::spotify_error("/v1/me/player/pause", resp).await)
}

pub async fn seek_to_start(access_token: &str) -> anyhow::Result<()> {
    const URL: &str = "https://api.spotify.com/v1/me/player/seek?position_ms=0";
    log_call!("PUT", URL);
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

    Err(crate::spotify_error("/v1/me/player/seek", resp).await)
}

pub async fn start_tracks(access_token: &str, track_uris: &[String]) -> anyhow::Result<()> {
    if track_uris.is_empty() {
        anyhow::bail!("cannot start playback with no tracks");
    }

    const URL: &str = "https://api.spotify.com/v1/me/player/play";
    log_call!("PUT", URL);
    let client = reqwest::Client::new();
    let resp = client
        .put(URL)
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

    if resp.status() != reqwest::StatusCode::NOT_FOUND {
        return Err(crate::spotify_error("/v1/me/player/play (start)", resp).await);
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

    log_call!("PUT", format!("{URL}?device_id={device_id}"));
    let resp = client
        .put(URL)
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

    Err(crate::spotify_error("/v1/me/player/play (start on device)", resp).await)
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
    width: Option<u32>,
    height: Option<u32>,
}

fn image_url_for_size(images: Vec<RawImage>, target_px: u32) -> Option<String> {
    images
        .into_iter()
        .min_by_key(|image| {
            let size = image.width.or(image.height).unwrap_or(u32::MAX);
            if size >= target_px {
                (0, size - target_px)
            } else {
                (1, target_px - size)
            }
        })
        .map(|image| image.url)
}

pub async fn get_track(access_token: &str, track_id: &str) -> anyhow::Result<TrackDetails> {
    let url = format!("https://api.spotify.com/v1/tracks/{track_id}");
    log_call!("GET", &url);
    let resp = reqwest::Client::new()
        .get(&url)
        .bearer_auth(access_token)
        .send()
        .await
        .context("fetching track")?;

    if !resp.status().is_success() {
        return Err(crate::spotify_error("/v1/tracks/{id}", resp).await);
    }

    let raw = resp
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
        album_art_url: image_url_for_size(raw.album.images, 560),
        duration_ms: raw.duration_ms,
    })
}
