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
    pub track: Json<PartyTrack>,
    pub added_by_user_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

pub struct NewPartySession {
    pub host_user_id: Uuid,
    pub room_code: String,
}

pub struct NewPartyQueueItem {
    pub session_id: Uuid,
    pub position: i32,
    pub track: PartyTrack,
    pub added_by_user_id: Uuid,
}

pub async fn create_session(pool: &PgPool, s: &NewPartySession) -> anyhow::Result<PartySession> {
    let session = sqlx::query_as::<_, PartySession>(
        r#"
        INSERT INTO public.party_sessions (host_user_id, room_code)
        VALUES ($1, $2)
        RETURNING id, host_user_id, room_code, mode, current_track_uri, queued_track_uri,
                  is_active, created_at, updated_at
        "#,
    )
    .bind(s.host_user_id)
    .bind(&s.room_code)
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
        SELECT id, host_user_id, room_code, mode, current_track_uri, queued_track_uri,
               is_active, created_at, updated_at
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
        SELECT id, host_user_id, room_code, mode, current_track_uri, queued_track_uri,
               is_active, created_at, updated_at
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
        SELECT id, host_user_id, room_code, mode, current_track_uri, queued_track_uri,
               is_active, created_at, updated_at
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
        "UPDATE public.party_sessions SET is_active = false, updated_at = now() WHERE id = $1",
    )
    .bind(session_id)
    .execute(pool)
    .await
    .context("ending party session")?;
    Ok(())
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

pub async fn queue_items(pool: &PgPool, session_id: Uuid) -> anyhow::Result<Vec<PartyQueueItem>> {
    let items = sqlx::query_as::<_, PartyQueueItem>(
        r#"
        SELECT id, session_id, position, track, added_by_user_id, created_at
        FROM public.party_queue_items
        WHERE session_id = $1
        ORDER BY position ASC, created_at ASC
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
        SELECT id, session_id, position, track, added_by_user_id, created_at
        FROM public.party_queue_items
        WHERE session_id = $1
        ORDER BY position ASC, created_at ASC
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

pub async fn add_queue_item(
    pool: &PgPool,
    item: &NewPartyQueueItem,
) -> anyhow::Result<PartyQueueItem> {
    let item = sqlx::query_as::<_, PartyQueueItem>(
        r#"
        INSERT INTO public.party_queue_items (session_id, position, track, added_by_user_id)
        VALUES ($1, $2, $3, $4)
        RETURNING id, session_id, position, track, added_by_user_id, created_at
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
    sqlx::query("DELETE FROM public.party_queue_items WHERE id = $1 AND session_id = $2")
        .bind(item_id)
        .bind(session_id)
        .execute(pool)
        .await
        .context("removing party queue item")?;

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
        RETURNING id, session_id, position, track, added_by_user_id, created_at
        "#,
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await
    .context("popping next party queue item")?;

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
        RETURNING id, session_id, position, track, added_by_user_id, created_at
        "#,
    )
    .bind(session_id)
    .bind(track_uri)
    .fetch_optional(pool)
    .await
    .context("removing first party queue item by uri")?;

    compact_queue_positions(pool, session_id).await?;

    Ok(item)
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
