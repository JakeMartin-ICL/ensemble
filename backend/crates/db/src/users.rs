//! Database queries for the users table.

use anyhow::Context;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(sqlx::FromRow)]
pub struct User {
    pub id: Uuid,
    pub spotify_id: String,
    pub display_name: String,
    pub access_token: String,
    pub refresh_token: String,
    pub spotify_client_id: Option<String>,
    pub token_expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow)]
pub struct UserSession {
    pub user_id: Uuid,
    pub expires_at: DateTime<Utc>,
}

pub async fn upsert_user(
    pool: &PgPool,
    spotify_id: &str,
    display_name: &str,
    access_token: &str,
    refresh_token: &str,
    spotify_client_id: &str,
    token_expires_at: DateTime<Utc>,
) -> anyhow::Result<Uuid> {
    let id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO public.users
            (spotify_id, display_name, access_token, refresh_token, spotify_client_id, token_expires_at)
        VALUES ($1, $2, $3, $4, $5, $6)
        ON CONFLICT (spotify_id) DO UPDATE SET
            display_name     = EXCLUDED.display_name,
            access_token     = EXCLUDED.access_token,
            refresh_token    = EXCLUDED.refresh_token,
            spotify_client_id = EXCLUDED.spotify_client_id,
            token_expires_at = EXCLUDED.token_expires_at,
            updated_at       = now()
        RETURNING id
        "#,
    )
    .bind(spotify_id)
    .bind(display_name)
    .bind(access_token)
    .bind(refresh_token)
    .bind(spotify_client_id)
    .bind(token_expires_at)
    .fetch_one(pool)
    .await
    .context("upserting user")?;
    Ok(id)
}

pub async fn get_user(pool: &PgPool, id: Uuid) -> anyhow::Result<Option<User>> {
    let user = sqlx::query_as::<_, User>(
        r#"
        SELECT id, spotify_id, display_name, access_token, refresh_token, spotify_client_id,
               token_expires_at, created_at, updated_at
        FROM public.users
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .context("fetching user")?;
    Ok(user)
}

pub async fn update_tokens(
    pool: &PgPool,
    id: Uuid,
    access_token: &str,
    refresh_token: Option<&str>,
    token_expires_at: DateTime<Utc>,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        UPDATE public.users
        SET access_token = $1,
            refresh_token = COALESCE($2, refresh_token),
            token_expires_at = $3,
            updated_at = now()
        WHERE id = $4
        "#,
    )
    .bind(access_token)
    .bind(refresh_token)
    .bind(token_expires_at)
    .bind(id)
    .execute(pool)
    .await
    .context("updating tokens")?;
    Ok(())
}

pub async fn create_session(
    pool: &PgPool,
    user_id: Uuid,
    token: &str,
    expires_at: DateTime<Utc>,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        INSERT INTO public.user_sessions (user_id, token_hash, expires_at)
        VALUES ($1, encode(digest($2, 'sha256'), 'hex'), $3)
        "#,
    )
    .bind(user_id)
    .bind(token)
    .bind(expires_at)
    .execute(pool)
    .await
    .context("creating user session")?;
    Ok(())
}

pub async fn user_id_for_session(pool: &PgPool, token: &str) -> anyhow::Result<Option<Uuid>> {
    Ok(user_session_for_token(pool, token)
        .await?
        .map(|session| session.user_id))
}

pub async fn user_session_for_token(
    pool: &PgPool,
    token: &str,
) -> anyhow::Result<Option<UserSession>> {
    let session = sqlx::query_as::<_, UserSession>(
        r#"
        SELECT user_id, expires_at
        FROM public.user_sessions
        WHERE token_hash = encode(digest($1, 'sha256'), 'hex')
          AND expires_at > now()
        "#,
    )
    .bind(token)
    .fetch_optional(pool)
    .await
    .context("fetching user session")?;
    Ok(session)
}
