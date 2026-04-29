//! Shared Spotify response caches.

use anyhow::Context;
use chrono::{DateTime, Utc};
use sqlx::{types::Json, PgPool};
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CachedPlaylistSummary {
    pub id: String,
    pub name: String,
    pub track_count: u32,
    pub image_url: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CachedPlaylistTrack {
    pub uri: String,
    pub name: String,
    pub artist: String,
    pub album_art_url: Option<String>,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct UserPlaylistCache {
    pub user_id: Uuid,
    pub playlists: Json<Vec<CachedPlaylistSummary>>,
    pub fetched_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PlaylistTrackCache {
    pub playlist_id: String,
    pub name: String,
    pub snapshot_id: Option<String>,
    pub track_count: i32,
    pub tracks: Json<Vec<CachedPlaylistTrack>>,
    pub fetched_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

pub async fn get_user_playlists(
    pool: &PgPool,
    user_id: Uuid,
) -> anyhow::Result<Option<UserPlaylistCache>> {
    sqlx::query_as::<_, UserPlaylistCache>(
        r#"
        SELECT user_id, playlists, fetched_at, expires_at
        FROM public.spotify_user_playlist_cache
        WHERE user_id = $1
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .context("fetching cached Spotify user playlists")
}

pub async fn upsert_user_playlists(
    pool: &PgPool,
    user_id: Uuid,
    playlists: &[CachedPlaylistSummary],
    expires_at: DateTime<Utc>,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        INSERT INTO public.spotify_user_playlist_cache
            (user_id, playlists, fetched_at, expires_at)
        VALUES ($1, $2, now(), $3)
        ON CONFLICT (user_id) DO UPDATE
        SET playlists = excluded.playlists,
            fetched_at = now(),
            expires_at = excluded.expires_at
        "#,
    )
    .bind(user_id)
    .bind(Json(playlists))
    .bind(expires_at)
    .execute(pool)
    .await
    .context("upserting cached Spotify user playlists")?;
    Ok(())
}

pub async fn get_playlist_tracks(
    pool: &PgPool,
    playlist_id: &str,
) -> anyhow::Result<Option<PlaylistTrackCache>> {
    sqlx::query_as::<_, PlaylistTrackCache>(
        r#"
        SELECT playlist_id, name, snapshot_id, track_count, tracks, fetched_at, expires_at
        FROM public.spotify_playlist_track_cache
        WHERE playlist_id = $1
        "#,
    )
    .bind(playlist_id)
    .fetch_optional(pool)
    .await
    .context("fetching cached Spotify playlist tracks")
}

pub async fn upsert_playlist_tracks(
    pool: &PgPool,
    playlist_id: &str,
    name: &str,
    snapshot_id: Option<&str>,
    track_count: i32,
    tracks: &[CachedPlaylistTrack],
    expires_at: DateTime<Utc>,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        INSERT INTO public.spotify_playlist_track_cache
            (playlist_id, name, snapshot_id, track_count, tracks, fetched_at, expires_at)
        VALUES ($1, $2, $3, $4, $5, now(), $6)
        ON CONFLICT (playlist_id) DO UPDATE
        SET name = excluded.name,
            snapshot_id = excluded.snapshot_id,
            track_count = excluded.track_count,
            tracks = excluded.tracks,
            fetched_at = now(),
            expires_at = excluded.expires_at
        "#,
    )
    .bind(playlist_id)
    .bind(name)
    .bind(snapshot_id)
    .bind(track_count)
    .bind(Json(tracks))
    .bind(expires_at)
    .execute(pool)
    .await
    .context("upserting cached Spotify playlist tracks")?;
    Ok(())
}

pub async fn extend_playlist_tracks(
    pool: &PgPool,
    playlist_id: &str,
    name: &str,
    snapshot_id: Option<&str>,
    track_count: i32,
    expires_at: DateTime<Utc>,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        UPDATE public.spotify_playlist_track_cache
        SET name = $1,
            snapshot_id = $2,
            track_count = $3,
            expires_at = $4
        WHERE playlist_id = $5
        "#,
    )
    .bind(name)
    .bind(snapshot_id)
    .bind(track_count)
    .bind(expires_at)
    .bind(playlist_id)
    .execute(pool)
    .await
    .context("extending cached Spotify playlist tracks")?;
    Ok(())
}
