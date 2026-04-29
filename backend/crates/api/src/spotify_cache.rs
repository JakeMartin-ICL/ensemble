//! Shared cached access to Spotify playlist endpoints.

use anyhow::Context;
use chrono::Utc;
use uuid::Uuid;

const USER_PLAYLIST_TTL_HOURS: i64 = 24;
const PLAYLIST_TRACK_TTL_HOURS: i64 = 24;

pub async fn user_playlists(
    pool: &db::PgPool,
    user_id: Uuid,
    access_token: &str,
) -> anyhow::Result<Vec<spotify::playlist::PlaylistSummary>> {
    if let Some(cached) = db::spotify_cache::get_user_playlists(pool, user_id).await? {
        if cached.expires_at > Utc::now() {
            tracing::info!(
                user_id = %cached.user_id,
                playlist_count = cached.playlists.0.len(),
                fetched_at = %cached.fetched_at,
                "Spotify cache hit: user playlists"
            );
            return Ok(cached
                .playlists
                .0
                .into_iter()
                .map(cached_summary_to_spotify)
                .collect());
        }
    }

    tracing::info!(%user_id, "Spotify cache miss: user playlists");
    let playlists = spotify::playlist::get_user_playlists(access_token).await?;
    let cached = playlists
        .iter()
        .cloned()
        .map(spotify_summary_to_cached)
        .collect::<Vec<_>>();
    db::spotify_cache::upsert_user_playlists(
        pool,
        user_id,
        &cached,
        Utc::now() + chrono::Duration::hours(USER_PLAYLIST_TTL_HOURS),
    )
    .await?;

    Ok(playlists)
}

pub async fn playlist_tracks(
    pool: &db::PgPool,
    access_token: &str,
    playlist_id: &str,
) -> anyhow::Result<CachedPlaylistTracks> {
    let cached = db::spotify_cache::get_playlist_tracks(pool, playlist_id).await?;
    if let Some(cached) = cached {
        if cached.expires_at > Utc::now() {
            tracing::info!(
                playlist_id = cached.playlist_id,
                track_count = cached.tracks.0.len(),
                fetched_at = %cached.fetched_at,
                "Spotify cache hit: playlist tracks"
            );
            return Ok(CachedPlaylistTracks {
                id: cached.playlist_id,
                name: cached.name,
                tracks: cached
                    .tracks
                    .0
                    .into_iter()
                    .map(cached_track_to_spotify)
                    .collect(),
            });
        }

        tracing::info!(playlist_id, "Spotify cache stale: playlist tracks");
        let metadata = spotify::playlist::get_playlist_metadata(access_token, playlist_id).await?;
        if cached.snapshot_id.is_some() && cached.snapshot_id == metadata.snapshot_id {
            let track_count = i32::try_from(metadata.track_count)
                .context("Spotify playlist track count does not fit in i32")?;
            db::spotify_cache::extend_playlist_tracks(
                pool,
                playlist_id,
                &metadata.name,
                metadata.snapshot_id.as_deref(),
                track_count,
                Utc::now() + chrono::Duration::hours(PLAYLIST_TRACK_TTL_HOURS),
            )
            .await?;
            tracing::info!(
                playlist_id,
                "Spotify cache refreshed from unchanged snapshot: playlist tracks"
            );
            return Ok(CachedPlaylistTracks {
                id: cached.playlist_id,
                name: metadata.name,
                tracks: cached
                    .tracks
                    .0
                    .into_iter()
                    .map(cached_track_to_spotify)
                    .collect(),
            });
        }
    } else {
        tracing::info!(playlist_id, "Spotify cache miss: playlist tracks");
    }

    let (metadata, tracks) = tokio::try_join!(
        spotify::playlist::get_playlist_metadata(access_token, playlist_id),
        spotify::playlist::get_tracks(access_token, playlist_id),
    )?;
    let cached_tracks = tracks
        .iter()
        .cloned()
        .map(spotify_track_to_cached)
        .collect::<Vec<_>>();
    let track_count = i32::try_from(metadata.track_count)
        .context("Spotify playlist track count does not fit in i32")?;
    db::spotify_cache::upsert_playlist_tracks(
        pool,
        playlist_id,
        &metadata.name,
        metadata.snapshot_id.as_deref(),
        track_count,
        &cached_tracks,
        Utc::now() + chrono::Duration::hours(PLAYLIST_TRACK_TTL_HOURS),
    )
    .await?;
    tracing::info!(
        playlist_id,
        track_count = tracks.len(),
        "Spotify cache stored: playlist tracks"
    );

    Ok(CachedPlaylistTracks {
        id: playlist_id.to_string(),
        name: metadata.name,
        tracks,
    })
}

pub struct CachedPlaylistTracks {
    pub id: String,
    pub name: String,
    pub tracks: Vec<spotify::playlist::PlaylistTrack>,
}

fn spotify_summary_to_cached(
    playlist: spotify::playlist::PlaylistSummary,
) -> db::spotify_cache::CachedPlaylistSummary {
    db::spotify_cache::CachedPlaylistSummary {
        id: playlist.id,
        name: playlist.name,
        track_count: playlist.track_count,
        image_url: playlist.image_url,
    }
}

fn cached_summary_to_spotify(
    playlist: db::spotify_cache::CachedPlaylistSummary,
) -> spotify::playlist::PlaylistSummary {
    spotify::playlist::PlaylistSummary {
        id: playlist.id,
        name: playlist.name,
        track_count: playlist.track_count,
        image_url: playlist.image_url,
    }
}

fn spotify_track_to_cached(
    track: spotify::playlist::PlaylistTrack,
) -> db::spotify_cache::CachedPlaylistTrack {
    db::spotify_cache::CachedPlaylistTrack {
        uri: track.uri,
        name: track.name,
        artist: track.artist,
        album_art_url: track.album_art_url,
        duration_ms: track.duration_ms,
    }
}

fn cached_track_to_spotify(
    track: db::spotify_cache::CachedPlaylistTrack,
) -> spotify::playlist::PlaylistTrack {
    spotify::playlist::PlaylistTrack {
        uri: track.uri,
        name: track.name,
        artist: track.artist,
        album_art_url: track.album_art_url,
        duration_ms: track.duration_ms,
    }
}
