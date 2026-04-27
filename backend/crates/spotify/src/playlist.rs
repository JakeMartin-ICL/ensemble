//! Fetch playlist tracks and user playlists from Spotify.

use anyhow::Context;

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
        let mut req = client.get(&next_url).bearer_auth(access_token);
        if first_page {
            req = req.query(&[
                ("limit", "50"),
                ("market", "from_token"),
                (
                    "fields",
                    "items(is_local,item(uri,type,name,artists(name),album(images(url)),duration_ms,is_playable,restrictions(reason))),next",
                ),
            ]);
            first_page = false;
        }

        let resp = req.send().await.context("fetching playlist tracks")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("Spotify playlist tracks returned {status}: {body}");
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
                        album_art_url: item
                            .album
                            .and_then(|a| a.images.into_iter().next())
                            .map(|i| i.url),
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
}

pub async fn get_user_playlists(access_token: &str) -> anyhow::Result<Vec<PlaylistSummary>> {
    let client = reqwest::Client::new();
    let mut playlists = Vec::new();
    let mut url = Some("https://api.spotify.com/v1/me/playlists?limit=50".to_string());

    while let Some(next_url) = url {
        let page = client
            .get(&next_url)
            .bearer_auth(access_token)
            .send()
            .await
            .context("fetching user playlists")?
            .error_for_status()
            .context("Spotify /v1/me/playlists returned error")?
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
    let resp = reqwest::Client::new()
        .get("https://api.spotify.com/v1/search")
        .bearer_auth(access_token)
        .query(&[("q", query), ("type", "track"), ("limit", "10")])
        .send()
        .await
        .context("searching Spotify tracks")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("Spotify track search returned {status}: {body}");
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
                album_art_url: item
                    .album
                    .and_then(|a| a.images.into_iter().next())
                    .map(|i| i.url),
                duration_ms: item.duration_ms.unwrap_or(0),
            })
        })
        .collect())
}
