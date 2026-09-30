//! Weave-mode heartbeat adapter.

use db::PgPool;
use playback::{BoxFuture, HeartbeatDriver};
use tokio::sync::broadcast;
use tracing::warn;
use uuid::Uuid;

pub struct HeartbeatParams {
    pub session_id: Uuid,
    pub pool: PgPool,
    pub events: broadcast::Sender<String>,
}

pub async fn run(params: HeartbeatParams) -> Option<playback::StopReason> {
    let driver = WeaveHeartbeat {
        session_id: params.session_id,
        pool: params.pool,
        events: params.events,
    };

    playback::run(playback::HeartbeatParams { driver }).await
}

struct WeaveHeartbeat {
    session_id: Uuid,
    pool: PgPool,
    events: broadcast::Sender<String>,
}

impl WeaveHeartbeat {
    fn emit(&self, event: &str) {
        let _ = self.events.send(event.to_string());
    }
}

impl HeartbeatDriver for WeaveHeartbeat {
    type Session = db::weave::WeaveSession;

    fn label(&self) -> &'static str {
        "weave"
    }

    fn session_id(&self) -> Uuid {
        self.session_id
    }

    fn pool(&self) -> &PgPool {
        &self.pool
    }

    fn load_session(&self) -> BoxFuture<'_, anyhow::Result<Option<Self::Session>>> {
        Box::pin(async move { db::weave::get_session(&self.pool, self.session_id).await })
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
        session: &'a Self::Session,
        track_uri: &'a str,
    ) -> BoxFuture<'a, anyhow::Result<()>> {
        Box::pin(async move {
            db::weave::update_position_and_track_and_clear_queue(
                &self.pool,
                self.session_id,
                session.current_playlist_index,
                track_uri,
                &session.playlist_track_indexes,
            )
            .await?;
            self.emit("session");
            self.emit("queue");
            Ok(())
        })
    }

    fn promote_queued_track<'a>(
        &'a self,
        session: &'a Self::Session,
        track_uri: &'a str,
    ) -> BoxFuture<'a, anyhow::Result<()>> {
        Box::pin(async move {
            if let Some(advance) = crate::session::next_playlist(session) {
                db::weave::update_position_and_track_and_clear_queue(
                    &self.pool,
                    self.session_id,
                    advance.playlist_index,
                    track_uri,
                    &advance.track_indexes,
                )
                .await?;
                self.emit("session");
                self.emit("queue");
                Ok(())
            } else {
                warn!(
                    "weave heartbeat: session {} has an empty playlist, cannot advance turn",
                    self.session_id
                );
                Ok(())
            }
        })
    }

    fn next_track_uri<'a>(
        &'a self,
        session: &'a Self::Session,
    ) -> BoxFuture<'a, anyhow::Result<Option<String>>> {
        Box::pin(
            async move { Ok(crate::session::next_playlist(session).map(|next| next.track_uri)) },
        )
    }

    fn set_queued_track<'a>(&'a self, track_uri: &'a str) -> BoxFuture<'a, anyhow::Result<()>> {
        Box::pin(async move {
            db::weave::set_queued_track(&self.pool, self.session_id, track_uri).await?;
            self.emit("queue");
            Ok(())
        })
    }

    fn update_playback<'a>(
        &'a self,
        playback: &'a spotify::player::PlaybackState,
    ) -> BoxFuture<'a, anyhow::Result<()>> {
        Box::pin(async move {
            db::weave::update_playback_state(
                &self.pool,
                self.session_id,
                &playback.track_uri,
                playback.progress_ms as i64,
                playback.duration_ms as i64,
                playback.is_playing,
            )
            .await?;
            self.emit("playback");
            Ok(())
        })
    }

    fn heartbeat_idle<'a>(&'a self) -> BoxFuture<'a, anyhow::Result<()>> {
        Box::pin(async move {
            self.emit("heartbeat_idle");
            Ok(())
        })
    }
}
