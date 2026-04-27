//! Per-session polling loop that monitors Spotify playback and advances the queue.

use db::PgPool;
use tracing::{info, warn};
use uuid::Uuid;

pub struct HeartbeatParams {
    pub session_id: Uuid,
    pub pool: PgPool,
    pub spotify_client_id: String,
    pub spotify_client_secret: String,
}

pub async fn run(params: HeartbeatParams) {
    if let Err(e) = run_inner(params).await {
        warn!("heartbeat for session ended with error: {e:#}");
    }
}

async fn run_inner(params: HeartbeatParams) -> anyhow::Result<()> {
    let HeartbeatParams {
        session_id,
        pool,
        spotify_client_id,
        spotify_client_secret,
    } = params;

    // Track whether we've already queued the next track for the current playing track,
    // to avoid double-queuing on successive polls.
    let mut queued_for: Option<String> = None;

    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        let session = match db::car::get_session(&pool, session_id).await? {
            Some(s) if s.is_active => s,
            _ => {
                info!("heartbeat: session {session_id} no longer active, stopping");
                return Ok(());
            }
        };

        let user = db::users::get_user(&pool, session.host_user_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("heartbeat: user not found"))?;

        let access_token = maybe_refresh_token(
            &pool,
            session.host_user_id,
            &user,
            &spotify_client_id,
            &spotify_client_secret,
        )
        .await?;

        let playback = match spotify::player::get_playback_state(&access_token).await {
            Ok(Some(p)) => p,
            Ok(None) => continue, // nothing playing, keep polling
            Err(e) => {
                warn!("heartbeat: error fetching playback: {e:#}");
                continue;
            }
        };

        // New track started. If it is our prequeued handoff, promote that known
        // queued track. Otherwise sync the DB to external playback without
        // advancing playlist indexes.
        if session.current_track_uri.as_deref() != Some(&playback.track_uri) {
            if session.queued_track_uri.as_deref() == Some(&playback.track_uri) {
                if let Some(advance) = crate::session::next_playlist(&session) {
                    if let Err(e) = db::car::update_position_and_track_and_clear_queue(
                        &pool,
                        session_id,
                        advance.playlist_index,
                        &playback.track_uri,
                        &advance.track_indexes,
                    )
                    .await
                    {
                        warn!("heartbeat: failed to promote queued track: {e:#}");
                    }
                } else {
                    warn!(
                        "heartbeat: session {session_id} has an empty playlist, cannot advance turn"
                    );
                }
            } else if let Err(e) = db::car::update_position_and_track_and_clear_queue(
                &pool,
                session_id,
                session.current_playlist_index,
                &playback.track_uri,
                &session.playlist_track_indexes,
            )
            .await
            {
                warn!("heartbeat: failed to sync external track change: {e:#}");
            }
            queued_for = None;
        }

        let session = match db::car::get_session(&pool, session_id).await? {
            Some(s) if s.is_active => s,
            _ => {
                info!("heartbeat: session {session_id} no longer active, stopping");
                return Ok(());
            }
        };

        // Approaching end of track — queue the next one.
        if playback.duration_ms > 0 {
            let remaining_ms = playback.duration_ms.saturating_sub(playback.progress_ms);
            if remaining_ms <= 5_000
                && queued_for.as_deref() != Some(&playback.track_uri)
                && session.queued_track_uri.is_none()
            {
                if let Some(next) = crate::session::next_playlist(&session) {
                    match spotify::player::queue_track(&access_token, &next.track_uri).await {
                        Ok(()) => {
                            if let Err(e) =
                                db::car::set_queued_track(&pool, session_id, &next.track_uri).await
                            {
                                warn!("heartbeat: failed to record queued track: {e:#}");
                            }
                            queued_for = Some(playback.track_uri.clone());
                            info!(
                                "heartbeat: queued {} for session {session_id}",
                                next.track_uri
                            );
                        }
                        Err(e) => warn!("heartbeat: failed to queue track: {e:#}"),
                    }
                }
            }
        }
    }
}

async fn maybe_refresh_token(
    pool: &PgPool,
    user_id: Uuid,
    user: &db::users::User,
    client_id: &str,
    client_secret: &str,
) -> anyhow::Result<String> {
    if user
        .token_expires_at
        .signed_duration_since(chrono::Utc::now())
        > chrono::Duration::seconds(60)
    {
        return Ok(user.access_token.clone());
    }

    let tokens =
        spotify::auth::refresh_token(&user.refresh_token, client_id, client_secret).await?;
    let new_expires_at = chrono::Utc::now() + chrono::Duration::seconds(tokens.expires_in as i64);
    db::users::update_tokens(pool, user_id, &tokens.access_token, new_expires_at).await?;
    Ok(tokens.access_token)
}
