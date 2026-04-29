//! Shared Spotify playback heartbeat orchestration.

use chrono::{DateTime, Utc};
use db::PgPool;
use std::{future::Future, pin::Pin};
use tracing::{info, warn};
use uuid::Uuid;

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub struct HeartbeatParams<D> {
    pub driver: D,
}

pub trait HeartbeatDriver: Send + Sync + 'static {
    type Session: Send + Sync;

    fn label(&self) -> &'static str;
    fn session_id(&self) -> Uuid;
    fn pool(&self) -> &PgPool;
    fn load_session(&self) -> BoxFuture<'_, anyhow::Result<Option<Self::Session>>>;
    fn is_active(&self, session: &Self::Session) -> bool;
    fn host_user_id(&self, session: &Self::Session) -> Uuid;
    fn current_track_uri<'a>(&self, session: &'a Self::Session) -> Option<&'a str>;
    fn queued_track_uri<'a>(&self, session: &'a Self::Session) -> Option<&'a str>;
    fn sync_external_track<'a>(
        &'a self,
        session: &'a Self::Session,
        track_uri: &'a str,
    ) -> BoxFuture<'a, anyhow::Result<()>>;
    fn promote_queued_track<'a>(
        &'a self,
        session: &'a Self::Session,
        track_uri: &'a str,
    ) -> BoxFuture<'a, anyhow::Result<()>>;
    fn next_track_uri<'a>(
        &'a self,
        session: &'a Self::Session,
    ) -> BoxFuture<'a, anyhow::Result<Option<String>>>;
    fn set_queued_track<'a>(&'a self, track_uri: &'a str) -> BoxFuture<'a, anyhow::Result<()>>;

    fn update_playback<'a>(
        &'a self,
        _playback: &'a spotify::player::PlaybackState,
    ) -> BoxFuture<'a, anyhow::Result<()>> {
        Box::pin(async { Ok(()) })
    }
}

pub async fn run<D>(params: HeartbeatParams<D>)
where
    D: HeartbeatDriver,
{
    let label = params.driver.label();
    let session_id = params.driver.session_id();
    if let Err(e) = run_inner(params).await {
        warn!("{label} heartbeat for session {session_id} ended with error: {e:#}");
    }
}

async fn run_inner<D>(params: HeartbeatParams<D>) -> anyhow::Result<()>
where
    D: HeartbeatDriver,
{
    let HeartbeatParams { driver } = params;
    let session_id = driver.session_id();
    let label = driver.label();
    let mut queued_for: Option<String> = None;
    let mut sleep_ms = 10_000u64;
    let mut cached_user: Option<CachedUserToken> = None;
    let mut paused_observations = 0u8;

    loop {
        tokio::time::sleep(tokio::time::Duration::from_millis(sleep_ms)).await;

        // Default to slow polling; updated at end of iteration if we have playback state.
        sleep_ms = 10_000;

        let mut session = match driver.load_session().await? {
            Some(s) if driver.is_active(&s) => s,
            _ => {
                info!("{label} heartbeat: session {session_id} no longer active, stopping");
                return Ok(());
            }
        };

        let host_user_id = driver.host_user_id(&session);
        let access_token =
            cached_access_token(driver.pool(), host_user_id, &mut cached_user, label).await?;

        let playback = match spotify::player::get_playback_state(&access_token).await {
            Ok(Some(p)) => p,
            Ok(None) => {
                paused_observations = paused_observations.saturating_add(1);
                if paused_observations >= 3 {
                    info!(
                        "{label} heartbeat: session {session_id} had no playback for three polls, stopping"
                    );
                    return Ok(());
                }
                continue;
            }
            Err(e) => {
                warn!("{label} heartbeat: error fetching playback: {e:#}");
                continue;
            }
        };

        if let Err(e) = driver.update_playback(&playback).await {
            warn!("{label} heartbeat: failed to update playback state: {e:#}");
        }

        if playback.is_playing {
            paused_observations = 0;
        } else {
            paused_observations = paused_observations.saturating_add(1);
            if paused_observations >= 3 {
                info!("{label} heartbeat: session {session_id} paused for three polls, stopping");
                return Ok(());
            }
        }

        if driver.current_track_uri(&session) != Some(playback.track_uri.as_str()) {
            let result = if driver.queued_track_uri(&session) == Some(playback.track_uri.as_str()) {
                driver
                    .promote_queued_track(&session, &playback.track_uri)
                    .await
            } else {
                driver
                    .sync_external_track(&session, &playback.track_uri)
                    .await
            };

            if let Err(e) = result {
                warn!("{label} heartbeat: failed to handle track change: {e:#}");
            }
            queued_for = None;

            session = match driver.load_session().await? {
                Some(s) if driver.is_active(&s) => s,
                _ => {
                    info!("{label} heartbeat: session {session_id} no longer active, stopping");
                    return Ok(());
                }
            };
        }

        if playback.duration_ms == 0 {
            continue;
        }

        let remaining_ms = playback.duration_ms.saturating_sub(playback.progress_ms);

        // Use 1s polling in the last 10s of a track for responsive queue-ahead.
        if remaining_ms <= 10_000 {
            sleep_ms = 1_000;
        }

        if remaining_ms > 5_000
            || queued_for.as_deref() == Some(&playback.track_uri)
            || driver.queued_track_uri(&session).is_some()
        {
            continue;
        }

        let Some(next_track_uri) = driver.next_track_uri(&session).await? else {
            continue;
        };

        match spotify::player::queue_track(&access_token, &next_track_uri).await {
            Ok(()) => {
                if let Err(e) = driver.set_queued_track(&next_track_uri).await {
                    warn!("{label} heartbeat: failed to record queued track: {e:#}");
                }
                queued_for = Some(playback.track_uri.clone());
                info!("{label} heartbeat: queued {next_track_uri} for session {session_id}");
            }
            Err(e) => warn!("{label} heartbeat: failed to queue track: {e:#}"),
        }
    }
}

struct CachedUserToken {
    user_id: Uuid,
    access_token: String,
    token_expires_at: DateTime<Utc>,
}

async fn cached_access_token(
    pool: &PgPool,
    user_id: Uuid,
    cached_user: &mut Option<CachedUserToken>,
    label: &str,
) -> anyhow::Result<String> {
    if let Some(cached) = cached_user.as_ref() {
        if cached.user_id == user_id
            && cached.token_expires_at.signed_duration_since(Utc::now())
                > chrono::Duration::seconds(60)
        {
            return Ok(cached.access_token.clone());
        }
    }

    let user = db::users::get_user(pool, user_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("{label} heartbeat: user not found"))?;
    let (access_token, token_expires_at) = maybe_refresh_token(pool, user_id, &user).await?;

    *cached_user = Some(CachedUserToken {
        user_id,
        access_token: access_token.clone(),
        token_expires_at,
    });

    Ok(access_token)
}

async fn maybe_refresh_token(
    pool: &PgPool,
    user_id: Uuid,
    user: &db::users::User,
) -> anyhow::Result<(String, DateTime<Utc>)> {
    if user
        .token_expires_at
        .signed_duration_since(chrono::Utc::now())
        > chrono::Duration::seconds(60)
    {
        return Ok((user.access_token.clone(), user.token_expires_at));
    }

    let client_id = user
        .spotify_client_id
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("Spotify client ID is missing; reconnect Spotify"))?;
    let tokens = spotify::auth::refresh_token_pkce(&user.refresh_token, client_id).await?;
    let new_expires_at = chrono::Utc::now() + chrono::Duration::seconds(tokens.expires_in as i64);
    db::users::update_tokens(
        pool,
        user_id,
        &tokens.access_token,
        tokens.refresh_token.as_deref(),
        new_expires_at,
    )
    .await?;
    Ok((tokens.access_token, new_expires_at))
}
