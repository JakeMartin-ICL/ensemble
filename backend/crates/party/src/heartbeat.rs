//! Party-mode heartbeat adapter.

use db::PgPool;
use playback::{BoxFuture, HeartbeatDriver};
use uuid::Uuid;

pub struct HeartbeatParams {
    pub session_id: Uuid,
    pub pool: PgPool,
}

pub async fn run(params: HeartbeatParams) {
    let driver = PartyHeartbeat {
        session_id: params.session_id,
        pool: params.pool,
    };

    playback::run(playback::HeartbeatParams { driver }).await;
}

struct PartyHeartbeat {
    session_id: Uuid,
    pool: PgPool,
}

impl HeartbeatDriver for PartyHeartbeat {
    type Session = db::party::PartySession;

    fn label(&self) -> &'static str {
        "party"
    }

    fn session_id(&self) -> Uuid {
        self.session_id
    }

    fn pool(&self) -> &PgPool {
        &self.pool
    }

    fn load_session(&self) -> BoxFuture<'_, anyhow::Result<Option<Self::Session>>> {
        Box::pin(async move { db::party::get_session(&self.pool, self.session_id).await })
    }

    fn is_active(&self, session: &Self::Session) -> bool {
        session.is_active
    }

    fn host_user_id(&self, session: &Self::Session) -> Uuid {
        session.host_user_id
    }

    fn current_track_uri<'a>(&self, session: &'a Self::Session) -> Option<&'a str> {
        session.current_track_uri.as_deref()
    }

    fn queued_track_uri<'a>(&self, session: &'a Self::Session) -> Option<&'a str> {
        session.queued_track_uri.as_deref()
    }

    fn sync_external_track<'a>(
        &'a self,
        _session: &'a Self::Session,
        track_uri: &'a str,
    ) -> BoxFuture<'a, anyhow::Result<()>> {
        Box::pin(async move {
            let track = db::party::PartyTrack {
                uri: track_uri.to_string(),
                name: None,
                artist: None,
                album_art_url: None,
                duration_ms: None,
            };
            db::party::set_current_track(&self.pool, self.session_id, Some(track_uri)).await?;
            db::party::add_played_track(
                &self.pool,
                &db::party::NewPartyPlayedTrack {
                    session_id: self.session_id,
                    track,
                    added_by_user_id: None,
                    added_by_guest_id: None,
                },
            )
            .await?;
            Ok(())
        })
    }

    fn promote_queued_track<'a>(
        &'a self,
        session: &'a Self::Session,
        track_uri: &'a str,
    ) -> BoxFuture<'a, anyhow::Result<()>> {
        Box::pin(async move {
            let item =
                db::party::remove_first_queue_item_by_uri(&self.pool, self.session_id, track_uri)
                    .await?;
            db::party::set_current_track(&self.pool, self.session_id, Some(track_uri)).await?;
            let track = item
                .as_ref()
                .map(|item| item.track.0.clone())
                .unwrap_or_else(|| db::party::PartyTrack {
                    uri: track_uri.to_string(),
                    name: None,
                    artist: None,
                    album_art_url: None,
                    duration_ms: None,
                });
            db::party::add_played_track(
                &self.pool,
                &db::party::NewPartyPlayedTrack {
                    session_id: self.session_id,
                    track,
                    added_by_user_id: item.as_ref().and_then(|item| item.added_by_user_id),
                    added_by_guest_id: item.as_ref().and_then(|item| item.added_by_guest_id),
                },
            )
            .await?;
            if session.mode == db::party::PartyMode::VotedQueue.as_str() {
                db::party::sort_voted_queue(&self.pool, self.session_id).await?;
                db::party::refill_queue_from_source(&self.pool, self.session_id).await?;
            } else if session.mode == db::party::PartyMode::BalancedQueue.as_str() {
                db::party::refill_queue_from_source(&self.pool, self.session_id).await?;
                let owner_key = db::party::current_track_owner_key(
                    &self.pool,
                    self.session_id,
                    Some(track_uri),
                )
                .await?;
                let items = db::party::balanced_queue_items_after(
                    &self.pool,
                    self.session_id,
                    owner_key.as_deref(),
                )
                .await?;
                let ids = items.iter().map(|item| item.id).collect::<Vec<_>>();
                db::party::update_queue_positions(&self.pool, self.session_id, &ids).await?;
            } else {
                db::party::refill_queue_from_source(&self.pool, self.session_id).await?;
            }
            Ok(())
        })
    }

    fn next_track_uri<'a>(
        &'a self,
        session: &'a Self::Session,
    ) -> BoxFuture<'a, anyhow::Result<Option<String>>> {
        Box::pin(async move {
            if session.mode == db::party::PartyMode::VotedQueue.as_str() {
                db::party::sort_voted_queue(&self.pool, self.session_id).await?;
                db::party::refill_queue_from_source(&self.pool, self.session_id).await?;
            } else {
                db::party::refill_queue_from_source(&self.pool, self.session_id).await?;
            }
            let item = if session.mode == db::party::PartyMode::BalancedQueue.as_str() {
                let owner_key = db::party::current_track_owner_key(
                    &self.pool,
                    self.session_id,
                    session.current_track_uri.as_deref(),
                )
                .await?;
                db::party::first_balanced_queue_item(
                    &self.pool,
                    self.session_id,
                    owner_key.as_deref(),
                )
                .await?
            } else {
                db::party::first_queue_item(&self.pool, self.session_id).await?
            };
            Ok(item.map(|item| item.track.uri.clone()))
        })
    }

    fn set_queued_track<'a>(&'a self, track_uri: &'a str) -> BoxFuture<'a, anyhow::Result<()>> {
        Box::pin(async move {
            db::party::set_queued_track(&self.pool, self.session_id, track_uri).await
        })
    }

    fn update_playback<'a>(
        &'a self,
        playback: &'a spotify::player::PlaybackState,
    ) -> BoxFuture<'a, anyhow::Result<()>> {
        Box::pin(async move {
            db::party::update_playback_state(
                &self.pool,
                self.session_id,
                &playback.track_uri,
                playback.progress_ms as i64,
                playback.duration_ms as i64,
                playback.is_playing,
            )
            .await
        })
    }
}
