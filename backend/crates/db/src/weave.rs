//! Database queries for weave mode sessions.

use anyhow::Context;
use chrono::{DateTime, Utc};
use sqlx::{types::Json, PgPool};
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PlaylistState {
    pub id: String,
    pub name: String,
    pub order: Vec<PlaylistTrack>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PlaylistTrack {
    pub uri: String,
    pub name: Option<String>,
    pub artist: Option<String>,
    pub album_art_url: Option<String>,
    pub duration_ms: Option<u64>,
}

impl<'de> serde::Deserialize<'de> for PlaylistTrack {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        #[serde(untagged)]
        enum RawTrack {
            Uri(String),
            Full {
                uri: String,
                name: Option<String>,
                artist: Option<String>,
                album_art_url: Option<String>,
                duration_ms: Option<u64>,
            },
        }

        match RawTrack::deserialize(deserializer)? {
            RawTrack::Uri(uri) => Ok(Self {
                uri,
                name: None,
                artist: None,
                album_art_url: None,
                duration_ms: None,
            }),
            RawTrack::Full {
                uri,
                name,
                artist,
                album_art_url,
                duration_ms,
            } => Ok(Self {
                uri,
                name,
                artist,
                album_art_url,
                duration_ms,
            }),
        }
    }
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct WeaveSession {
    pub id: Uuid,
    pub host_user_id: Uuid,
    pub playlists: Json<Vec<PlaylistState>>,
    pub current_playlist_index: i32,
    pub playlist_track_indexes: Vec<i32>,
    pub current_track_uri: Option<String>,
    pub queued_track_uri: Option<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl WeaveSession {
    pub fn playlists(&self) -> &[PlaylistState] {
        &self.playlists.0
    }
}

pub struct NewWeaveSession {
    pub host_user_id: Uuid,
    pub playlists: Vec<PlaylistState>,
    pub current_playlist_index: i32,
    pub playlist_track_indexes: Vec<i32>,
    pub current_track_uri: Option<String>,
    pub queued_track_uri: Option<String>,
}

pub async fn create_session(pool: &PgPool, s: &NewWeaveSession) -> anyhow::Result<WeaveSession> {
    let first_playlist = s
        .playlists
        .first()
        .context("new weave session must include at least one playlist")?;
    let second_playlist = s.playlists.get(1).unwrap_or(first_playlist);
    let first_playlist_uris = first_playlist
        .order
        .iter()
        .map(|track| track.uri.clone())
        .collect::<Vec<_>>();
    let second_playlist_uris = second_playlist
        .order
        .iter()
        .map(|track| track.uri.clone())
        .collect::<Vec<_>>();

    let session = sqlx::query_as::<_, WeaveSession>(
        r#"
        INSERT INTO public.weave_sessions
            (host_user_id, playlist_a_id, playlist_b_id, playlist_a_name, playlist_b_name,
             playlist_a_order, playlist_b_order, playlists, current_playlist_index,
             playlist_track_indexes, current_track_uri, queued_track_uri)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
        RETURNING id, host_user_id, playlists, current_playlist_index, playlist_track_indexes,
                  current_track_uri, queued_track_uri, is_active, created_at, updated_at
        "#,
    )
    .bind(s.host_user_id)
    .bind(&first_playlist.id)
    .bind(&second_playlist.id)
    .bind(&first_playlist.name)
    .bind(&second_playlist.name)
    .bind(&first_playlist_uris)
    .bind(&second_playlist_uris)
    .bind(Json(&s.playlists))
    .bind(s.current_playlist_index)
    .bind(&s.playlist_track_indexes)
    .bind(&s.current_track_uri)
    .bind(&s.queued_track_uri)
    .fetch_one(pool)
    .await
    .context("inserting weave session")?;
    Ok(session)
}

pub async fn get_active_session(
    pool: &PgPool,
    host_user_id: Uuid,
) -> anyhow::Result<Option<WeaveSession>> {
    let session = sqlx::query_as::<_, WeaveSession>(
        r#"
        SELECT id, host_user_id, playlists, current_playlist_index, playlist_track_indexes,
               current_track_uri, queued_track_uri, is_active, created_at, updated_at
        FROM public.weave_sessions
        WHERE host_user_id = $1 AND is_active = true
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .bind(host_user_id)
    .fetch_optional(pool)
    .await
    .context("fetching active weave session")?;
    Ok(session)
}

pub async fn get_session(pool: &PgPool, session_id: Uuid) -> anyhow::Result<Option<WeaveSession>> {
    let session = sqlx::query_as::<_, WeaveSession>(
        r#"
        SELECT id, host_user_id, playlists, current_playlist_index, playlist_track_indexes,
               current_track_uri, queued_track_uri, is_active, created_at, updated_at
        FROM public.weave_sessions
        WHERE id = $1
        "#,
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await
    .context("fetching weave session")?;
    Ok(session)
}

pub async fn update_position_and_track_and_clear_queue(
    pool: &PgPool,
    session_id: Uuid,
    current_playlist_index: i32,
    track_uri: &str,
    playlist_track_indexes: &[i32],
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        UPDATE public.weave_sessions
        SET current_playlist_index = $1, current_track_uri = $2,
            playlist_track_indexes = $3, queued_track_uri = NULL, updated_at = now()
        WHERE id = $4
        "#,
    )
    .bind(current_playlist_index)
    .bind(track_uri)
    .bind(playlist_track_indexes)
    .bind(session_id)
    .execute(pool)
    .await
    .context("updating playlist position and track and clearing queued track")?;
    Ok(())
}

pub async fn update_position_and_track_and_set_queue(
    pool: &PgPool,
    session_id: Uuid,
    current_playlist_index: i32,
    track_uri: &str,
    playlist_track_indexes: &[i32],
    queued_track_uri: Option<&str>,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        UPDATE public.weave_sessions
        SET current_playlist_index = $1, current_track_uri = $2,
            playlist_track_indexes = $3, queued_track_uri = $4, updated_at = now()
        WHERE id = $5
        "#,
    )
    .bind(current_playlist_index)
    .bind(track_uri)
    .bind(playlist_track_indexes)
    .bind(queued_track_uri)
    .bind(session_id)
    .execute(pool)
    .await
    .context("updating playlist position and track and setting queued track")?;
    Ok(())
}

pub async fn set_queued_track(
    pool: &PgPool,
    session_id: Uuid,
    track_uri: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        UPDATE public.weave_sessions
        SET queued_track_uri = $1, updated_at = now()
        WHERE id = $2
        "#,
    )
    .bind(track_uri)
    .bind(session_id)
    .execute(pool)
    .await
    .context("setting queued track")?;
    Ok(())
}

pub async fn update_playlists(
    pool: &PgPool,
    session_id: Uuid,
    playlists: &[PlaylistState],
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        UPDATE public.weave_sessions
        SET playlists = $1, updated_at = now()
        WHERE id = $2
        "#,
    )
    .bind(Json(playlists))
    .bind(session_id)
    .execute(pool)
    .await
    .context("updating playlists")?;
    Ok(())
}

pub async fn update_playlists_and_track_indexes(
    pool: &PgPool,
    session_id: Uuid,
    playlists: &[PlaylistState],
    playlist_track_indexes: &[i32],
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        UPDATE public.weave_sessions
        SET playlists = $1, playlist_track_indexes = $2, updated_at = now()
        WHERE id = $3
        "#,
    )
    .bind(Json(playlists))
    .bind(playlist_track_indexes)
    .bind(session_id)
    .execute(pool)
    .await
    .context("updating playlists and track indexes")?;
    Ok(())
}

pub async fn end_session(pool: &PgPool, session_id: Uuid) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE public.weave_sessions SET is_active = false, updated_at = now() WHERE id = $1",
    )
    .bind(session_id)
    .execute(pool)
    .await
    .context("ending weave session")?;
    Ok(())
}

pub async fn deactivate_user_sessions(pool: &PgPool, host_user_id: Uuid) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE public.weave_sessions SET is_active = false, updated_at = now() WHERE host_user_id = $1 AND is_active = true",
    )
    .bind(host_user_id)
    .execute(pool)
    .await
    .context("deactivating user sessions")?;
    Ok(())
}
