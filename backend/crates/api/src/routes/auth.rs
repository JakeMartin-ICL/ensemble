//! Spotify OAuth endpoints.
//! POST /auth/callback - exchange code for tokens
//! POST /auth/refresh  - refresh an access token

use axum::{Router, routing::post};

pub fn router() -> Router {
    Router::new()
        .route("/callback", post(callback))
        .route("/refresh", post(refresh))
}

async fn callback() -> &'static str { "TODO" }
async fn refresh()  -> &'static str { "TODO" }
