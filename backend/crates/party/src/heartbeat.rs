//! Party-mode heartbeat adapter.

use db::PgPool;
use playback::{BoxFuture, HeartbeatDriver};
use uuid::Uuid;

pub struct HeartbeatParams {
    pub session_id: Uuid,
    pub pool: PgPool,
    pub spotify_client_id: String,
    pub spotify_client_secret: String,
}

pub async fn run(params: HeartbeatParams) {
    let driver = PartyHeartbeat {
        session_id: params.session_id,
        pool: params.pool,
    };

    playback::run(playback::HeartbeatParams {
        driver,
        spotify_client_id: params.spotify_client_id,
        spotify_client_secret: params.spotify_client_secret,
    })
    .await;
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
            db::party::set_current_track(&self.pool, self.session_id, Some(track_uri)).await
        })
    }

    fn promote_queued_track<'a>(
        &'a self,
        _session: &'a Self::Session,
        track_uri: &'a str,
    ) -> BoxFuture<'a, anyhow::Result<()>> {
        Box::pin(async move {
            db::party::remove_first_queue_item_by_uri(&self.pool, self.session_id, track_uri)
                .await?;
            db::party::set_current_track(&self.pool, self.session_id, Some(track_uri)).await
        })
    }

    fn next_track_uri<'a>(
        &'a self,
        _session: &'a Self::Session,
    ) -> BoxFuture<'a, anyhow::Result<Option<String>>> {
        Box::pin(async move {
            Ok(db::party::first_queue_item(&self.pool, self.session_id)
                .await?
                .map(|item| item.track.uri.clone()))
        })
    }

    fn set_queued_track<'a>(&'a self, track_uri: &'a str) -> BoxFuture<'a, anyhow::Result<()>> {
        Box::pin(async move {
            db::party::set_queued_track(&self.pool, self.session_id, track_uri).await
        })
    }
}
