//! Database queries for party mode sessions.

use anyhow::Context;
use chrono::{DateTime, Utc};
use sqlx::{types::Json, PgPool};
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PartyTrack {
    pub uri: String,
    pub name: Option<String>,
    pub artist: Option<String>,
    pub album_art_url: Option<String>,
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PartySession {
    pub id: Uuid,
    pub host_user_id: Uuid,
    pub room_code: String,
    pub mode: String,
    pub allow_guest_playlist_adds: bool,
    pub source_min_queue_size: i32,
    pub add_added_tracks_to_source: bool,
    pub show_queue_attribution: bool,
    pub current_track_uri: Option<String>,
    pub queued_track_uri: Option<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PartyQueueItem {
    pub id: Uuid,
    pub session_id: Uuid,
    pub position: i32,
    pub pin_position: Option<i32>,
    pub track: Json<PartyTrack>,
    pub added_by_user_id: Option<Uuid>,
    pub added_by_display_name: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PartyQueueVote {
    pub queue_item_id: Uuid,
    pub user_id: Uuid,
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PartySourceQueueItem {
    pub id: Uuid,
    pub session_id: Uuid,
    pub position: i32,
    pub disabled: bool,
    pub track: Json<PartyTrack>,
    pub added_by_user_id: Option<Uuid>,
    pub added_by_display_name: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PartyPlayedTrack {
    pub id: Uuid,
    pub session_id: Uuid,
    pub play_order: i32,
    pub track: Json<PartyTrack>,
    pub added_by_user_id: Option<Uuid>,
    pub added_by_display_name: Option<String>,
    pub created_at: DateTime<Utc>,
}

pub struct NewPartySession {
    pub host_user_id: Uuid,
    pub room_code: String,
    pub source_min_queue_size: i32,
    pub add_added_tracks_to_source: bool,
}

pub struct NewPartyQueueItem {
    pub session_id: Uuid,
    pub position: i32,
    pub track: PartyTrack,
    pub added_by_user_id: Option<Uuid>,
}

pub struct NewPartySourceQueueItem {
    pub session_id: Uuid,
    pub position: i32,
    pub track: PartyTrack,
    pub added_by_user_id: Uuid,
}

pub struct NewPartyPlayedTrack {
    pub session_id: Uuid,
    pub track: PartyTrack,
    pub added_by_user_id: Option<Uuid>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartyMode {
    OpenQueue,
    SharedQueue,
    VotedQueue,
}

impl PartyMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OpenQueue => "open_queue",
            Self::SharedQueue => "shared_queue",
            Self::VotedQueue => "voted_queue",
        }
    }
}

impl std::str::FromStr for PartyMode {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "open_queue" => Ok(Self::OpenQueue),
            "shared_queue" => Ok(Self::SharedQueue),
            "voted_queue" => Ok(Self::VotedQueue),
            _ => Err(anyhow::anyhow!("unsupported party mode")),
        }
    }
}

pub async fn create_session(pool: &PgPool, s: &NewPartySession) -> anyhow::Result<PartySession> {
    let session = sqlx::query_as::<_, PartySession>(
        r#"
        INSERT INTO public.party_sessions (
            host_user_id, room_code, source_min_queue_size, add_added_tracks_to_source
        )
        VALUES ($1, $2, $3, $4)
        RETURNING id, host_user_id, room_code, mode, allow_guest_playlist_adds,
                  source_min_queue_size, add_added_tracks_to_source, show_queue_attribution,
                  current_track_uri, queued_track_uri, is_active, created_at, updated_at
        "#,
    )
    .bind(s.host_user_id)
    .bind(&s.room_code)
    .bind(s.source_min_queue_size)
    .bind(s.add_added_tracks_to_source)
    .fetch_one(pool)
    .await
    .context("inserting party session")?;
    Ok(session)
}

pub async fn get_active_session(
    pool: &PgPool,
    host_user_id: Uuid,
) -> anyhow::Result<Option<PartySession>> {
    let session = sqlx::query_as::<_, PartySession>(
        r#"
        SELECT id, host_user_id, room_code, mode, allow_guest_playlist_adds,
               source_min_queue_size, add_added_tracks_to_source, show_queue_attribution,
               current_track_uri, queued_track_uri, is_active, created_at, updated_at
        FROM public.party_sessions
        WHERE host_user_id = $1 AND is_active = true
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .bind(host_user_id)
    .fetch_optional(pool)
    .await
    .context("fetching active party session")?;
    Ok(session)
}

pub async fn get_session(pool: &PgPool, session_id: Uuid) -> anyhow::Result<Option<PartySession>> {
    let session = sqlx::query_as::<_, PartySession>(
        r#"
        SELECT id, host_user_id, room_code, mode, allow_guest_playlist_adds,
               source_min_queue_size, add_added_tracks_to_source, show_queue_attribution,
               current_track_uri, queued_track_uri, is_active, created_at, updated_at
        FROM public.party_sessions
        WHERE id = $1
        "#,
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await
    .context("fetching party session")?;
    Ok(session)
}

pub async fn get_session_by_room_code(
    pool: &PgPool,
    room_code: &str,
) -> anyhow::Result<Option<PartySession>> {
    let session = sqlx::query_as::<_, PartySession>(
        r#"
        SELECT id, host_user_id, room_code, mode, allow_guest_playlist_adds,
               source_min_queue_size, add_added_tracks_to_source, show_queue_attribution,
               current_track_uri, queued_track_uri, is_active, created_at, updated_at
        FROM public.party_sessions
        WHERE room_code = $1 AND is_active = true
        "#,
    )
    .bind(room_code)
    .fetch_optional(pool)
    .await
    .context("fetching party session by room code")?;
    Ok(session)
}

pub async fn deactivate_user_sessions(pool: &PgPool, host_user_id: Uuid) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE public.party_sessions SET is_active = false, updated_at = now() WHERE host_user_id = $1 AND is_active = true",
    )
    .bind(host_user_id)
    .execute(pool)
    .await
    .context("deactivating party sessions")?;
    Ok(())
}

pub async fn end_session(pool: &PgPool, session_id: Uuid) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        UPDATE public.party_sessions
        SET is_active = false,
            current_track_uri = NULL,
            queued_track_uri = NULL,
            updated_at = now()
        WHERE id = $1
        "#,
    )
    .bind(session_id)
    .execute(pool)
    .await
    .context("ending party session")?;

    sqlx::query("DELETE FROM public.party_queue_items WHERE session_id = $1")
        .bind(session_id)
        .execute(pool)
        .await
        .context("clearing party queue items")?;

    sqlx::query("DELETE FROM public.party_source_queue_items WHERE session_id = $1")
        .bind(session_id)
        .execute(pool)
        .await
        .context("clearing party source queue items")?;

    sqlx::query("DELETE FROM public.party_played_tracks WHERE session_id = $1")
        .bind(session_id)
        .execute(pool)
        .await
        .context("clearing party played tracks")?;

    Ok(())
}

pub async fn set_mode(
    pool: &PgPool,
    session_id: Uuid,
    mode: PartyMode,
) -> anyhow::Result<PartySession> {
    let session = sqlx::query_as::<_, PartySession>(
        r#"
        UPDATE public.party_sessions
        SET mode = $1, updated_at = now()
        WHERE id = $2
        RETURNING id, host_user_id, room_code, mode, allow_guest_playlist_adds,
                  source_min_queue_size, add_added_tracks_to_source, show_queue_attribution,
                  current_track_uri, queued_track_uri, is_active, created_at, updated_at
        "#,
    )
    .bind(mode.as_str())
    .bind(session_id)
    .fetch_one(pool)
    .await
    .context("updating party session mode")?;
    Ok(session)
}

pub async fn set_allow_guest_playlist_adds(
    pool: &PgPool,
    session_id: Uuid,
    allow_guest_playlist_adds: bool,
) -> anyhow::Result<PartySession> {
    let session = sqlx::query_as::<_, PartySession>(
        r#"
        UPDATE public.party_sessions
        SET allow_guest_playlist_adds = $1, updated_at = now()
        WHERE id = $2
        RETURNING id, host_user_id, room_code, mode, allow_guest_playlist_adds,
                  source_min_queue_size, add_added_tracks_to_source, show_queue_attribution,
                  current_track_uri, queued_track_uri, is_active, created_at, updated_at
        "#,
    )
    .bind(allow_guest_playlist_adds)
    .bind(session_id)
    .fetch_one(pool)
    .await
    .context("updating party guest playlist setting")?;
    Ok(session)
}

pub async fn set_source_settings(
    pool: &PgPool,
    session_id: Uuid,
    source_min_queue_size: i32,
    add_added_tracks_to_source: bool,
) -> anyhow::Result<PartySession> {
    let session = sqlx::query_as::<_, PartySession>(
        r#"
        UPDATE public.party_sessions
        SET source_min_queue_size = $1, add_added_tracks_to_source = $2, updated_at = now()
        WHERE id = $3
        RETURNING id, host_user_id, room_code, mode, allow_guest_playlist_adds,
                  source_min_queue_size, add_added_tracks_to_source, show_queue_attribution,
                  current_track_uri, queued_track_uri, is_active, created_at, updated_at
        "#,
    )
    .bind(source_min_queue_size)
    .bind(add_added_tracks_to_source)
    .bind(session_id)
    .fetch_one(pool)
    .await
    .context("updating party source settings")?;
    Ok(session)
}

pub async fn set_show_queue_attribution(
    pool: &PgPool,
    session_id: Uuid,
    show_queue_attribution: bool,
) -> anyhow::Result<PartySession> {
    let session = sqlx::query_as::<_, PartySession>(
        r#"
        UPDATE public.party_sessions
        SET show_queue_attribution = $1, updated_at = now()
        WHERE id = $2
        RETURNING id, host_user_id, room_code, mode, allow_guest_playlist_adds,
                  source_min_queue_size, add_added_tracks_to_source, show_queue_attribution,
                  current_track_uri, queued_track_uri, is_active, created_at, updated_at
        "#,
    )
    .bind(show_queue_attribution)
    .bind(session_id)
    .fetch_one(pool)
    .await
    .context("updating party queue attribution setting")?;
    Ok(session)
}

pub async fn set_current_track(
    pool: &PgPool,
    session_id: Uuid,
    track_uri: Option<&str>,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        UPDATE public.party_sessions
        SET current_track_uri = $1, queued_track_uri = NULL, updated_at = now()
        WHERE id = $2
        "#,
    )
    .bind(track_uri)
    .bind(session_id)
    .execute(pool)
    .await
    .context("setting party current track")?;
    Ok(())
}

pub async fn set_queued_track(
    pool: &PgPool,
    session_id: Uuid,
    track_uri: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        UPDATE public.party_sessions
        SET queued_track_uri = $1, updated_at = now()
        WHERE id = $2
        "#,
    )
    .bind(track_uri)
    .bind(session_id)
    .execute(pool)
    .await
    .context("setting party queued track")?;
    Ok(())
}

pub async fn add_played_track(
    pool: &PgPool,
    item: &NewPartyPlayedTrack,
) -> anyhow::Result<PartyPlayedTrack> {
    let play_order = sqlx::query_scalar::<_, Option<i32>>(
        "SELECT max(play_order) + 1 FROM public.party_played_tracks WHERE session_id = $1",
    )
    .bind(item.session_id)
    .fetch_one(pool)
    .await
    .context("fetching next party played-track order")?
    .unwrap_or(0);

    let played = sqlx::query_as::<_, PartyPlayedTrack>(
        r#"
        INSERT INTO public.party_played_tracks (
            session_id, play_order, track, added_by_user_id
        )
        VALUES ($1, $2, $3, $4)
        RETURNING id, session_id, play_order, track, added_by_user_id,
                  NULL::text AS added_by_display_name, created_at
        "#,
    )
    .bind(item.session_id)
    .bind(play_order)
    .bind(Json(&item.track))
    .bind(item.added_by_user_id)
    .fetch_one(pool)
    .await
    .context("adding party played track")?;
    Ok(played)
}

pub async fn played_tracks(
    pool: &PgPool,
    session_id: Uuid,
) -> anyhow::Result<Vec<PartyPlayedTrack>> {
    let items = sqlx::query_as::<_, PartyPlayedTrack>(
        r#"
        SELECT p.id, p.session_id, p.play_order, p.track, p.added_by_user_id,
               u.display_name AS added_by_display_name, p.created_at
        FROM public.party_played_tracks p
        LEFT JOIN public.users u ON u.id = p.added_by_user_id
        WHERE p.session_id = $1
        ORDER BY p.play_order ASC, p.created_at ASC
        "#,
    )
    .bind(session_id)
    .fetch_all(pool)
    .await
    .context("fetching party played tracks")?;
    Ok(items)
}

pub async fn queue_items(pool: &PgPool, session_id: Uuid) -> anyhow::Result<Vec<PartyQueueItem>> {
    let items = sqlx::query_as::<_, PartyQueueItem>(
        r#"
        SELECT q.id, q.session_id, q.position, q.pin_position, q.track, q.added_by_user_id,
               u.display_name AS added_by_display_name, q.created_at
        FROM public.party_queue_items q
        LEFT JOIN public.users u ON u.id = q.added_by_user_id
        WHERE q.session_id = $1
        ORDER BY q.position ASC, q.created_at ASC
        "#,
    )
    .bind(session_id)
    .fetch_all(pool)
    .await
    .context("fetching party queue items")?;
    Ok(items)
}

pub async fn first_queue_item(
    pool: &PgPool,
    session_id: Uuid,
) -> anyhow::Result<Option<PartyQueueItem>> {
    let item = sqlx::query_as::<_, PartyQueueItem>(
        r#"
        SELECT q.id, q.session_id, q.position, q.pin_position, q.track, q.added_by_user_id,
               u.display_name AS added_by_display_name, q.created_at
        FROM public.party_queue_items q
        LEFT JOIN public.users u ON u.id = q.added_by_user_id
        WHERE q.session_id = $1
        ORDER BY q.position ASC, q.created_at ASC
        LIMIT 1
        "#,
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await
    .context("fetching first party queue item")?;
    Ok(item)
}

pub async fn next_position(pool: &PgPool, session_id: Uuid) -> anyhow::Result<i32> {
    let position = sqlx::query_scalar::<_, Option<i32>>(
        "SELECT max(position) + 1 FROM public.party_queue_items WHERE session_id = $1",
    )
    .bind(session_id)
    .fetch_one(pool)
    .await
    .context("fetching next party queue position")?
    .unwrap_or(0);
    Ok(position)
}

async fn source_append_position(pool: &PgPool, session_id: Uuid) -> anyhow::Result<i32> {
    let position = sqlx::query_scalar::<_, Option<i32>>(
        "SELECT max(position) + 1 FROM public.party_source_queue_items WHERE session_id = $1 AND position >= 0",
    )
    .bind(session_id)
    .fetch_one(pool)
    .await
    .context("fetching next party source position")?
    .unwrap_or(0);
    Ok(position)
}

async fn source_deferred_position(pool: &PgPool, session_id: Uuid) -> anyhow::Result<i32> {
    let position = sqlx::query_scalar::<_, Option<i32>>(
        "SELECT min(position) - 1 FROM public.party_source_queue_items WHERE session_id = $1",
    )
    .bind(session_id)
    .fetch_one(pool)
    .await
    .context("fetching deferred party source position")?
    .unwrap_or(-1);
    Ok(position.min(-1))
}

pub async fn add_queue_item(
    pool: &PgPool,
    item: &NewPartyQueueItem,
) -> anyhow::Result<PartyQueueItem> {
    let item = sqlx::query_as::<_, PartyQueueItem>(
        r#"
        INSERT INTO public.party_queue_items (session_id, position, track, added_by_user_id)
        VALUES ($1, $2, $3, $4)
        RETURNING id, session_id, position, NULL::integer AS pin_position, track, added_by_user_id,
                  NULL::text AS added_by_display_name, created_at
        "#,
    )
    .bind(item.session_id)
    .bind(item.position)
    .bind(Json(&item.track))
    .bind(item.added_by_user_id)
    .fetch_one(pool)
    .await
    .context("adding party queue item")?;
    Ok(item)
}

pub async fn source_queue_items(
    pool: &PgPool,
    session_id: Uuid,
) -> anyhow::Result<Vec<PartySourceQueueItem>> {
    let items = sqlx::query_as::<_, PartySourceQueueItem>(
        r#"
        SELECT s.id, s.session_id, s.position, s.disabled, s.track, s.added_by_user_id,
               u.display_name AS added_by_display_name, s.created_at
        FROM public.party_source_queue_items s
        LEFT JOIN public.users u ON u.id = s.added_by_user_id
        WHERE s.session_id = $1
        ORDER BY s.disabled ASC, (s.position < 0) ASC, s.position ASC, s.created_at ASC
        "#,
    )
    .bind(session_id)
    .fetch_all(pool)
    .await
    .context("fetching party source queue items")?;
    Ok(items)
}

pub async fn set_source_queue_item_disabled(
    pool: &PgPool,
    session_id: Uuid,
    item_id: Uuid,
    disabled: bool,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        UPDATE public.party_source_queue_items
        SET disabled = $1
        WHERE id = $2 AND session_id = $3
        "#,
    )
    .bind(disabled)
    .bind(item_id)
    .bind(session_id)
    .execute(pool)
    .await
    .context("updating party source queue disabled state")?;

    sqlx::query("UPDATE public.party_sessions SET updated_at = now() WHERE id = $1")
        .bind(session_id)
        .execute(pool)
        .await
        .context("touching party session after source queue disabled update")?;

    Ok(())
}

pub async fn add_source_queue_items(
    pool: &PgPool,
    session_id: Uuid,
    start_position: i32,
    tracks: &[PartyTrack],
    added_by_user_id: Uuid,
) -> anyhow::Result<()> {
    let positions = tracks
        .iter()
        .enumerate()
        .map(|(offset, _)| {
            let offset = i32::try_from(offset).context("source queue position overflow")?;
            start_position
                .checked_add(offset)
                .context("source queue position overflow")
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    let track_json = tracks
        .iter()
        .map(serde_json::to_value)
        .collect::<Result<Vec<_>, _>>()
        .context("serializing party source queue tracks")?;

    sqlx::query(
        r#"
        INSERT INTO public.party_source_queue_items (session_id, position, track, added_by_user_id)
        SELECT $1, mapped.position, mapped.track, $4
        FROM unnest($2::integer[], $3::jsonb[]) AS mapped(position, track)
        ON CONFLICT (session_id, (track->>'uri'))
        DO UPDATE SET
          position = EXCLUDED.position,
          track = EXCLUDED.track,
          added_by_user_id = EXCLUDED.added_by_user_id
        "#,
    )
    .bind(session_id)
    .bind(&positions)
    .bind(&track_json)
    .bind(added_by_user_id)
    .execute(pool)
    .await
    .context("adding party source queue items")?;

    sqlx::query("UPDATE public.party_sessions SET updated_at = now() WHERE id = $1")
        .bind(session_id)
        .execute(pool)
        .await
        .context("touching party session after source queue add")?;

    Ok(())
}

pub async fn append_source_queue_items(
    pool: &PgPool,
    session_id: Uuid,
    tracks: &[PartyTrack],
    added_by_user_id: Uuid,
) -> anyhow::Result<()> {
    let position = source_append_position(pool, session_id).await?;
    add_source_queue_items(pool, session_id, position, tracks, added_by_user_id).await
}

pub async fn defer_source_queue_track(
    pool: &PgPool,
    session_id: Uuid,
    track: &PartyTrack,
    added_by_user_id: Uuid,
) -> anyhow::Result<()> {
    let position = source_deferred_position(pool, session_id).await?;
    let track_json = serde_json::to_value(track).context("serializing deferred source track")?;

    sqlx::query(
        r#"
        INSERT INTO public.party_source_queue_items (session_id, position, track, added_by_user_id)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (session_id, (track->>'uri'))
        DO UPDATE SET
          position = EXCLUDED.position,
          track = EXCLUDED.track,
          added_by_user_id = EXCLUDED.added_by_user_id
        "#,
    )
    .bind(session_id)
    .bind(position)
    .bind(track_json)
    .bind(added_by_user_id)
    .execute(pool)
    .await
    .context("deferring party source queue track")?;

    sqlx::query("UPDATE public.party_sessions SET updated_at = now() WHERE id = $1")
        .bind(session_id)
        .execute(pool)
        .await
        .context("touching party session after source queue defer")?;

    Ok(())
}

pub async fn add_queue_items(
    pool: &PgPool,
    session_id: Uuid,
    start_position: i32,
    tracks: &[PartyTrack],
    added_by_user_id: Uuid,
) -> anyhow::Result<()> {
    let positions = tracks
        .iter()
        .enumerate()
        .map(|(offset, _)| {
            let offset = i32::try_from(offset).context("queue position overflow")?;
            start_position
                .checked_add(offset)
                .context("queue position overflow")
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    let track_json = tracks
        .iter()
        .map(serde_json::to_value)
        .collect::<Result<Vec<_>, _>>()
        .context("serializing party queue tracks")?;

    sqlx::query(
        r#"
        INSERT INTO public.party_queue_items (session_id, position, track, added_by_user_id)
        SELECT $1, mapped.position, mapped.track, $4
        FROM unnest($2::integer[], $3::jsonb[]) AS mapped(position, track)
        "#,
    )
    .bind(session_id)
    .bind(&positions)
    .bind(&track_json)
    .bind(added_by_user_id)
    .execute(pool)
    .await
    .context("adding party queue items")?;

    sqlx::query("UPDATE public.party_sessions SET updated_at = now() WHERE id = $1")
        .bind(session_id)
        .execute(pool)
        .await
        .context("touching party session after playlist add")?;

    Ok(())
}

pub async fn update_queue_positions(
    pool: &PgPool,
    session_id: Uuid,
    item_ids: &[Uuid],
) -> anyhow::Result<()> {
    let positions = item_ids
        .iter()
        .enumerate()
        .map(|(position, _)| i32::try_from(position).context("queue position overflow"))
        .collect::<anyhow::Result<Vec<_>>>()?;

    sqlx::query(
        r#"
        UPDATE public.party_queue_items q
        SET position = mapped.position
        FROM unnest($1::uuid[], $2::integer[]) AS mapped(id, position)
        WHERE q.id = mapped.id AND q.session_id = $3
        "#,
    )
    .bind(item_ids)
    .bind(&positions)
    .bind(session_id)
    .execute(pool)
    .await
    .context("bulk updating party queue positions")?;

    sqlx::query("UPDATE public.party_sessions SET updated_at = now() WHERE id = $1")
        .bind(session_id)
        .execute(pool)
        .await
        .context("touching party session after queue reorder")?;

    Ok(())
}

pub async fn remove_queue_item(
    pool: &PgPool,
    session_id: Uuid,
    item_id: Uuid,
) -> anyhow::Result<()> {
    let position = sqlx::query_scalar::<_, Option<i32>>(
        "SELECT position FROM public.party_queue_items WHERE id = $1 AND session_id = $2",
    )
    .bind(item_id)
    .bind(session_id)
    .fetch_one(pool)
    .await
    .context("fetching position of queue item to remove")?
    .unwrap_or(0);

    sqlx::query("DELETE FROM public.party_queue_items WHERE id = $1 AND session_id = $2")
        .bind(item_id)
        .bind(session_id)
        .execute(pool)
        .await
        .context("removing party queue item")?;

    decrement_pins_after_position(pool, session_id, position).await?;
    compact_queue_positions(pool, session_id).await
}

pub async fn pop_next_queue_item(
    pool: &PgPool,
    session_id: Uuid,
) -> anyhow::Result<Option<PartyQueueItem>> {
    let item = sqlx::query_as::<_, PartyQueueItem>(
        r#"
        DELETE FROM public.party_queue_items
        WHERE id = (
            SELECT id
            FROM public.party_queue_items
            WHERE session_id = $1
            ORDER BY position ASC, created_at ASC
            LIMIT 1
        )
        RETURNING id, session_id, position, NULL::integer AS pin_position, track, added_by_user_id,
                  NULL::text AS added_by_display_name, created_at
        "#,
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await
    .context("popping next party queue item")?;

    decrement_pins_after_position(pool, session_id, -1).await?;
    compact_queue_positions(pool, session_id).await?;

    Ok(item)
}

pub async fn remove_first_queue_item_by_uri(
    pool: &PgPool,
    session_id: Uuid,
    track_uri: &str,
) -> anyhow::Result<Option<PartyQueueItem>> {
    let item = sqlx::query_as::<_, PartyQueueItem>(
        r#"
        DELETE FROM public.party_queue_items
        WHERE id = (
            SELECT id
            FROM public.party_queue_items
            WHERE session_id = $1 AND track->>'uri' = $2
            ORDER BY position ASC, created_at ASC
            LIMIT 1
        )
        RETURNING id, session_id, position, NULL::integer AS pin_position, track, added_by_user_id,
                  NULL::text AS added_by_display_name, created_at
        "#,
    )
    .bind(session_id)
    .bind(track_uri)
    .fetch_optional(pool)
    .await
    .context("removing first party queue item by uri")?;

    let removed_position = item.as_ref().map(|i| i.position).unwrap_or(0);
    decrement_pins_after_position(pool, session_id, removed_position - 1).await?;
    compact_queue_positions(pool, session_id).await?;

    Ok(item)
}

pub async fn refill_queue_from_source(pool: &PgPool, session_id: Uuid) -> anyhow::Result<()> {
    let min_queue_size = sqlx::query_scalar::<_, i32>(
        "SELECT source_min_queue_size FROM public.party_sessions WHERE id = $1",
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await
    .context("fetching party source minimum queue size")?
    .unwrap_or(0);

    if min_queue_size <= 0 {
        return Ok(());
    }

    let mut visible_count = queue_len(pool, session_id).await?;
    while visible_count < min_queue_size {
        let Some(source_item) = pop_next_source_queue_item(pool, session_id).await? else {
            break;
        };

        let position = next_position(pool, session_id).await?;
        add_queue_item(
            pool,
            &NewPartyQueueItem {
                session_id,
                position,
                track: source_item.track.0,
                added_by_user_id: source_item.added_by_user_id,
            },
        )
        .await?;
        visible_count += 1;
    }

    Ok(())
}

async fn queue_len(pool: &PgPool, session_id: Uuid) -> anyhow::Result<i32> {
    let count = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM public.party_queue_items WHERE session_id = $1",
    )
    .bind(session_id)
    .fetch_one(pool)
    .await
    .context("counting party queue items")?;
    i32::try_from(count).context("party queue count overflow")
}

async fn source_enabled_len(pool: &PgPool, session_id: Uuid) -> anyhow::Result<i64> {
    sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM public.party_source_queue_items WHERE session_id = $1 AND disabled = false",
    )
    .bind(session_id)
    .fetch_one(pool)
    .await
    .context("counting enabled party source queue items")
}

async fn pop_next_source_queue_item(
    pool: &PgPool,
    session_id: Uuid,
) -> anyhow::Result<Option<PartySourceQueueItem>> {
    let item = take_next_source_queue_item(pool, session_id).await?;
    if item.is_some() || source_enabled_len(pool, session_id).await? == 0 {
        return Ok(item);
    }

    reshuffle_source_queue(pool, session_id).await?;
    take_next_source_queue_item(pool, session_id).await
}

async fn take_next_source_queue_item(
    pool: &PgPool,
    session_id: Uuid,
) -> anyhow::Result<Option<PartySourceQueueItem>> {
    let item = sqlx::query_as::<_, PartySourceQueueItem>(
        r#"
        WITH next_item AS (
            SELECT id
            FROM public.party_source_queue_items s
            WHERE s.session_id = $1
              AND s.position >= 0
              AND s.disabled = false
              AND NOT EXISTS (
                SELECT 1
                FROM public.party_queue_items q
                WHERE q.session_id = $1
                  AND q.track->>'uri' = s.track->>'uri'
              )
            ORDER BY position ASC, created_at ASC
            LIMIT 1
        ),
        deferred AS (
            SELECT least(coalesce(min(position) - 1, -1), -1) AS position
            FROM public.party_source_queue_items
            WHERE session_id = $1
        )
        UPDATE public.party_source_queue_items s
        SET position = deferred.position
        FROM next_item, deferred
        WHERE s.id = next_item.id
        RETURNING s.id, s.session_id, s.position, s.disabled, s.track, s.added_by_user_id, NULL::text AS added_by_display_name, s.created_at
        "#,
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await
    .context("taking next party source queue item")?;
    Ok(item)
}

async fn reshuffle_source_queue(pool: &PgPool, session_id: Uuid) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        WITH ranked AS (
            SELECT id, row_number() OVER (ORDER BY random()) - 1 AS new_position
            FROM public.party_source_queue_items
            WHERE session_id = $1
        )
        UPDATE public.party_source_queue_items s
        SET position = ranked.new_position
        FROM ranked
        WHERE s.id = ranked.id
        "#,
    )
    .bind(session_id)
    .execute(pool)
    .await
    .context("reshuffling party source queue")?;
    Ok(())
}

async fn compact_queue_positions(pool: &PgPool, session_id: Uuid) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        WITH ranked AS (
            SELECT id, row_number() OVER (ORDER BY position ASC, created_at ASC) - 1 AS new_position
            FROM public.party_queue_items
            WHERE session_id = $1
        )
        UPDATE public.party_queue_items q
        SET position = ranked.new_position
        FROM ranked
        WHERE q.id = ranked.id
        "#,
    )
    .bind(session_id)
    .execute(pool)
    .await
    .context("compacting party queue positions")?;

    sqlx::query("UPDATE public.party_sessions SET updated_at = now() WHERE id = $1")
        .bind(session_id)
        .execute(pool)
        .await
        .context("touching party session after queue compaction")?;

    Ok(())
}

async fn decrement_pins_after_position(
    pool: &PgPool,
    session_id: Uuid,
    removed_position: i32,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        UPDATE public.party_queue_items
        SET pin_position = pin_position - 1
        WHERE session_id = $1
          AND pin_position IS NOT NULL
          AND pin_position > $2
        "#,
    )
    .bind(session_id)
    .bind(removed_position)
    .execute(pool)
    .await
    .context("decrementing pin positions after queue item removal")?;
    Ok(())
}

pub async fn clear_all_pins(pool: &PgPool, session_id: Uuid) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE public.party_queue_items SET pin_position = NULL WHERE session_id = $1",
    )
    .bind(session_id)
    .execute(pool)
    .await
    .context("clearing all queue item pins")?;
    Ok(())
}

pub async fn set_queue_item_pin(
    pool: &PgPool,
    session_id: Uuid,
    item_id: Uuid,
    pin_position: i32,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        UPDATE public.party_queue_items
        SET pin_position = $1
        WHERE id = $2 AND session_id = $3
        "#,
    )
    .bind(pin_position)
    .bind(item_id)
    .bind(session_id)
    .execute(pool)
    .await
    .context("setting queue item pin position")?;
    Ok(())
}

pub async fn clear_queue_item_pin(
    pool: &PgPool,
    session_id: Uuid,
    item_id: Uuid,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        UPDATE public.party_queue_items
        SET pin_position = NULL
        WHERE id = $1 AND session_id = $2
        "#,
    )
    .bind(item_id)
    .bind(session_id)
    .execute(pool)
    .await
    .context("clearing queue item pin position")?;
    Ok(())
}

pub async fn vote_queue_item(
    pool: &PgPool,
    session_id: Uuid,
    item_id: Uuid,
    user_id: Uuid,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        INSERT INTO public.party_queue_votes (queue_item_id, user_id)
        SELECT $1, $2
        WHERE EXISTS (
            SELECT 1 FROM public.party_queue_items
            WHERE id = $1 AND session_id = $3
        )
        ON CONFLICT (queue_item_id, user_id) DO NOTHING
        "#,
    )
    .bind(item_id)
    .bind(user_id)
    .bind(session_id)
    .execute(pool)
    .await
    .context("adding queue item vote")?;
    Ok(())
}

pub async fn unvote_queue_item(
    pool: &PgPool,
    session_id: Uuid,
    item_id: Uuid,
    user_id: Uuid,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        DELETE FROM public.party_queue_votes
        WHERE queue_item_id = $1
          AND user_id = $2
          AND EXISTS (
              SELECT 1 FROM public.party_queue_items
              WHERE id = $1 AND session_id = $3
          )
        "#,
    )
    .bind(item_id)
    .bind(user_id)
    .bind(session_id)
    .execute(pool)
    .await
    .context("removing queue item vote")?;
    Ok(())
}

pub async fn votes_for_queue_items(
    pool: &PgPool,
    session_id: Uuid,
) -> anyhow::Result<Vec<PartyQueueVote>> {
    let votes = sqlx::query_as::<_, PartyQueueVote>(
        r#"
        SELECT v.queue_item_id, v.user_id, u.display_name
        FROM public.party_queue_votes v
        JOIN public.party_queue_items qi ON qi.id = v.queue_item_id
        LEFT JOIN public.users u ON u.id = v.user_id
        WHERE qi.session_id = $1
        ORDER BY v.created_at ASC
        "#,
    )
    .bind(session_id)
    .fetch_all(pool)
    .await
    .context("fetching votes for queue items")?;
    Ok(votes)
}

pub async fn sort_voted_queue(pool: &PgPool, session_id: Uuid) -> anyhow::Result<()> {
    #[derive(sqlx::FromRow)]
    struct VoteSortRow {
        id: Uuid,
        pin_position: Option<i32>,
        created_at: DateTime<Utc>,
        vote_count: Option<i64>,
    }

    let rows = sqlx::query_as::<_, VoteSortRow>(
        r#"
        SELECT qi.id, qi.pin_position, qi.created_at,
               COUNT(v.id)::bigint AS vote_count
        FROM public.party_queue_items qi
        LEFT JOIN public.party_queue_votes v ON v.queue_item_id = qi.id
        WHERE qi.session_id = $1
        GROUP BY qi.id, qi.pin_position, qi.created_at
        "#,
    )
    .bind(session_id)
    .fetch_all(pool)
    .await
    .context("fetching items for vote sort")?;

    if rows.is_empty() {
        return Ok(());
    }

    let n = rows.len();

    let mut pinned: Vec<(i32, Uuid)> = rows
        .iter()
        .filter_map(|r| r.pin_position.map(|p| (p, r.id)))
        .collect();
    pinned.sort_by_key(|(p, _)| *p);

    let unpinned: Vec<Uuid> = {
        let mut u: Vec<_> = rows.iter().filter(|r| r.pin_position.is_none()).collect();
        u.sort_by(|a, b| {
            let va = a.vote_count.unwrap_or(0);
            let vb = b.vote_count.unwrap_or(0);
            vb.cmp(&va).then_with(|| a.created_at.cmp(&b.created_at))
        });
        u.into_iter().map(|r| r.id).collect()
    };

    let mut slots: Vec<Option<Uuid>> = vec![None; n];

    for (pin_pos, id) in &pinned {
        let target = (*pin_pos as usize).min(n - 1);
        let actual = (target..n)
            .find(|&i| slots[i].is_none())
            .or_else(|| (0..n).find(|&i| slots[i].is_none()));
        if let Some(idx) = actual {
            slots[idx] = Some(*id);
        }
    }

    let mut unpinned_iter = unpinned.into_iter();
    for slot in &mut slots {
        if slot.is_none() {
            *slot = unpinned_iter.next();
        }
    }

    let ordered_ids: Vec<Uuid> = slots.into_iter().flatten().collect();
    update_queue_positions(pool, session_id, &ordered_ids).await
}
