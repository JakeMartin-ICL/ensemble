//! Weave-mode heartbeat adapter.

use db::PgPool;
use playback::{BoxFuture, HeartbeatDriver};
use tracing::warn;
use uuid::Uuid;

pub struct HeartbeatParams {
    pub session_id: Uuid,
    pub pool: PgPool,
}

pub async fn run(params: HeartbeatParams) {
    let driver = WeaveHeartbeat {
        session_id: params.session_id,
        pool: params.pool,
    };

    playback::run(playback::HeartbeatParams { driver })
    .await;
}

struct WeaveHeartbeat {
    session_id: Uuid,
    pool: PgPool,
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
            .await
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
                .await
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
            db::weave::set_queued_track(&self.pool, self.session_id, track_uri).await
        })
    }
}
