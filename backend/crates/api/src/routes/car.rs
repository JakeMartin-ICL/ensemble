//! Car mode endpoints.
//! POST   /car/sessions          - create a session
//! GET    /car/sessions/:id      - get session state
//! POST   /car/sessions/:id/next - advance to next track (skip song or natural end)
//! POST   /car/sessions/:id/skip-turn - skip to the other person's turn

use axum::{Router, routing::{get, post}};

pub fn router() -> Router {
    Router::new()
        .route("/sessions", post(create_session))
        .route("/sessions/:id", get(get_session))
        .route("/sessions/:id/next", post(next_track))
        .route("/sessions/:id/skip-turn", post(skip_turn))
}

async fn create_session() -> &'static str { "TODO" }
async fn get_session()    -> &'static str { "TODO" }
async fn next_track()     -> &'static str { "TODO" }
async fn skip_turn()      -> &'static str { "TODO" }
