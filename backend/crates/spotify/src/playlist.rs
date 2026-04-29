//! Fetch playlist tracks and user playlists from Spotify.

use anyhow::Context;

macro_rules! log_call {
    ($method:literal, $url:expr) => {
        if *crate::LOG_CALLS {
            tracing::info!("Spotify API: {} {}", $method, $url);
        }
    };
}

pub struct PlaylistSummary {
    pub id: String,
    pub name: String,
    pub track_count: u32,
    pub image_url: Option<String>,
}

pub struct PlaylistTrack {
    pub uri: String,
    pub name: String,
    pub artist: String,
    pub album_art_url: Option<String>,
    pub duration_ms: u64,
}

pub struct TrackSearchResult {
    pub uri: String,
    pub name: String,
    pub artist: String,
    pub album_art_url: Option<String>,
    pub duration_ms: u64,
}

pub struct CreatedPlaylist {
    pub id: String,
    pub url: String,
}

#[derive(serde::Deserialize)]
struct RawCreatedPlaylist {
    id: String,
    external_urls: RawExternalUrls,
}

#[derive(serde::Deserialize)]
struct RawExternalUrls {
    spotify: String,
}

#[derive(serde::Serialize)]
struct CreatePlaylistRequest<'a> {
    name: &'a str,
    description: &'a str,
    public: bool,
}

#[derive(serde::Serialize)]
struct AddItemsRequest<'a> {
    uris: &'a [String],
}

#[derive(serde::Deserialize)]
struct RawTracksPage {
    items: Vec<Option<RawTrackItem>>,
    next: Option<String>,
}

#[derive(serde::Deserialize)]
struct RawTrackItem {
    #[serde(default)]
    is_local: bool,
    #[serde(alias = "track")]
    item: Option<RawTrack>,
}

#[derive(serde::Deserialize)]
struct RawTrack {
    uri: Option<String>,
    name: Option<String>,
    artists: Option<Vec<RawArtist>>,
    album: Option<RawAlbum>,
    duration_ms: Option<u64>,
    #[serde(rename = "type")]
    item_type: Option<String>,
    is_playable: Option<bool>,
    restrictions: Option<RawRestrictions>,
}

#[derive(serde::Deserialize)]
struct RawRestrictions {
    reason: Option<String>,
}

#[derive(serde::Deserialize)]
struct RawArtist {
    name: String,
}

#[derive(serde::Deserialize)]
struct RawAlbum {
    images: Vec<RawImage>,
}

pub async fn get_tracks(
    access_token: &str,
    playlist_id: &str,
) -> anyhow::Result<Vec<PlaylistTrack>> {
    let client = reqwest::Client::new();
    let mut tracks = Vec::new();
    let mut url = Some(format!(
        "https://api.spotify.com/v1/playlists/{playlist_id}/items"
    ));
    let mut first_page = true;

    while let Some(next_url) = url {
        log_call!("GET", &next_url);
        let mut req = client.get(&next_url).bearer_auth(access_token);
        if first_page {
            req = req.query(&[
                ("limit", "50"),
                ("market", "from_token"),
                (
                    "fields",
                    "items(is_local,item(uri,type,name,artists(name),album(images(url,width,height)),duration_ms,is_playable,restrictions(reason))),next",
                ),
            ]);
            first_page = false;
        }

        let resp = req.send().await.context("fetching playlist tracks")?;

        if !resp.status().is_success() {
            return Err(crate::spotify_error("playlist tracks", resp).await);
        }

        let page = resp
            .json::<RawTracksPage>()
            .await
            .context("parsing playlist tracks response")?;

        for playlist_item in page.items.into_iter().flatten() {
            if playlist_item.is_local {
                continue;
            }

            if let Some(item) = playlist_item.item {
                if item.item_type.as_deref().is_some_and(|t| t != "track") {
                    continue;
                }

                if item.is_playable == Some(false) {
                    tracing::debug!(
                        playlist_id,
                        reason = item
                            .restrictions
                            .as_ref()
                            .and_then(|r| r.reason.as_deref())
                            .unwrap_or("unknown"),
                        "skipping unplayable playlist track"
                    );
                    continue;
                }

                if let Some(uri) = item.uri {
                    tracks.push(PlaylistTrack {
                        uri,
                        name: item.name.unwrap_or_else(|| "Unknown track".to_string()),
                        artist: item
                            .artists
                            .unwrap_or_default()
                            .into_iter()
                            .map(|a| a.name)
                            .collect::<Vec<_>>()
                            .join(", "),
                        album_art_url: item.album.and_then(|a| image_url_for_size(a.images, 128)),
                        duration_ms: item.duration_ms.unwrap_or(0),
                    });
                }
            }
        }

        url = page.next;
    }

    tracing::debug!(
        playlist_id,
        track_count = tracks.len(),
        "fetched playable playlist tracks"
    );

    Ok(tracks)
}

pub async fn create_playlist(
    access_token: &str,
    name: &str,
    description: &str,
) -> anyhow::Result<CreatedPlaylist> {
    const URL: &str = "https://api.spotify.com/v1/me/playlists";
    log_call!("POST", URL);
    let resp = reqwest::Client::new()
        .post(URL)
        .bearer_auth(access_token)
        .json(&CreatePlaylistRequest {
            name,
            description,
            public: false,
        })
        .send()
        .await
        .context("creating Spotify playlist")?;

    if !resp.status().is_success() {
        return Err(crate::spotify_error("create playlist", resp).await);
    }

    let playlist = resp
        .json::<RawCreatedPlaylist>()
        .await
        .context("parsing Spotify create playlist response")?;

    Ok(CreatedPlaylist {
        id: playlist.id,
        url: playlist.external_urls.spotify,
    })
}

pub async fn add_items_to_playlist(
    access_token: &str,
    playlist_id: &str,
    uris: &[String],
) -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    let url = format!("https://api.spotify.com/v1/playlists/{playlist_id}/items");
    for chunk in uris.chunks(100) {
        log_call!("POST", &url);
        let resp = client
            .post(&url)
            .bearer_auth(access_token)
            .json(&AddItemsRequest { uris: chunk })
            .send()
            .await
            .context("adding Spotify playlist items")?;

        if !resp.status().is_success() {
            return Err(crate::spotify_error("add playlist items", resp).await);
        }
    }

    Ok(())
}

#[derive(serde::Deserialize)]
struct RawPlaylistsPage {
    items: Vec<Option<RawPlaylist>>,
    next: Option<String>,
}

#[derive(serde::Deserialize)]
struct RawPlaylist {
    id: String,
    name: String,
    items: RawCollectionMeta,
    images: Option<Vec<RawImage>>,
}

#[derive(serde::Deserialize)]
struct RawCollectionMeta {
    total: u32,
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

pub async fn get_user_playlists(access_token: &str) -> anyhow::Result<Vec<PlaylistSummary>> {
    let client = reqwest::Client::new();
    let mut playlists = Vec::new();
    let mut url = Some("https://api.spotify.com/v1/me/playlists?limit=50".to_string());

    while let Some(next_url) = url {
        log_call!("GET", &next_url);
        let resp = client
            .get(&next_url)
            .bearer_auth(access_token)
            .send()
            .await
            .context("fetching user playlists")?;

        if !resp.status().is_success() {
            return Err(crate::spotify_error("user playlists", resp).await);
        }

        let page = resp
            .json::<RawPlaylistsPage>()
            .await
            .context("parsing playlists response")?;

        for p in page.items.into_iter().flatten() {
            playlists.push(PlaylistSummary {
                id: p.id,
                name: p.name,
                track_count: p.items.total,
                image_url: p
                    .images
                    .unwrap_or_default()
                    .into_iter()
                    .next()
                    .map(|i| i.url),
            });
        }

        url = page.next;
    }

    Ok(playlists)
}

#[derive(serde::Deserialize)]
struct RawSearchResponse {
    tracks: RawSearchTracks,
}

#[derive(serde::Deserialize)]
struct RawSearchTracks {
    items: Vec<RawTrack>,
}

pub async fn search_tracks(
    access_token: &str,
    query: &str,
) -> anyhow::Result<Vec<TrackSearchResult>> {
    const URL: &str = "https://api.spotify.com/v1/search";
    log_call!("GET", URL);
    let resp = reqwest::Client::new()
        .get(URL)
        .bearer_auth(access_token)
        .query(&[("q", query), ("type", "track"), ("limit", "10")])
        .send()
        .await
        .context("searching Spotify tracks")?;

    if !resp.status().is_success() {
        return Err(crate::spotify_error("track search", resp).await);
    }

    let raw = resp
        .json::<RawSearchResponse>()
        .await
        .context("parsing Spotify track search response")?;

    Ok(raw
        .tracks
        .items
        .into_iter()
        .filter(|item| item.item_type.as_deref().is_none_or(|t| t == "track"))
        .filter(|item| item.is_playable != Some(false))
        .filter_map(|item| {
            item.uri.map(|uri| TrackSearchResult {
                uri,
                name: item.name.unwrap_or_else(|| "Unknown track".to_string()),
                artist: item
                    .artists
                    .unwrap_or_default()
                    .into_iter()
                    .map(|a| a.name)
                    .collect::<Vec<_>>()
                    .join(", "),
                album_art_url: item.album.and_then(|a| image_url_for_size(a.images, 128)),
                duration_ms: item.duration_ms.unwrap_or(0),
            })
        })
        .collect())
}
