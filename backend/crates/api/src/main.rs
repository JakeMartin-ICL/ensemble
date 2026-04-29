mod routes;
mod spotify_cache;

use anyhow::Context;
use axum::{
    http::{header, HeaderValue, Method},
    routing::get,
    Router,
};
use dashmap::DashMap;
use std::sync::Arc;
use tokio::task::AbortHandle;
use tower_http::cors::CorsLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use uuid::Uuid;

#[derive(Clone)]
pub struct CachedAuthSession {
    pub user_id: Uuid,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Clone)]
pub struct CachedSpotifyToken {
    pub access_token: String,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Clone)]
pub struct HeartbeatTask {
    pub run_id: Uuid,
    pub abort_handle: AbortHandle,
}

#[derive(Clone)]
pub struct AppState {
    pub pool: db::PgPool,
    pub spotify_redirect_uri: String,
    pub heartbeat_tasks: Arc<DashMap<Uuid, HeartbeatTask>>,
    pub auth_sessions: Arc<DashMap<String, CachedAuthSession>>,
    pub spotify_tokens: Arc<DashMap<Uuid, CachedSpotifyToken>>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    dotenvy::dotenv().ok();

    let pool = db::connect().await?;

    let state = AppState {
        pool,
        spotify_redirect_uri: std::env::var("SPOTIFY_REDIRECT_URI")
            .context("SPOTIFY_REDIRECT_URI must be set")?,
        heartbeat_tasks: Arc::new(DashMap::new()),
        auth_sessions: Arc::new(DashMap::new()),
        spotify_tokens: Arc::new(DashMap::new()),
    };

    let allowed_origin: HeaderValue = std::env::var("ALLOWED_ORIGIN")
        .context("ALLOWED_ORIGIN must be set")?
        .parse()
        .context("ALLOWED_ORIGIN is not a valid header value")?;

    let cors = CorsLayer::new()
        .allow_origin(allowed_origin)
        .allow_methods([Method::GET, Method::POST, Method::DELETE])
        .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION]);

    let app = Router::new()
        .route("/health", get(|| async { "ok" }))
        .nest("/auth", routes::auth::router())
        .nest("/weave", routes::weave::router())
        .nest("/party", routes::party::router())
        .route("/me", get(routes::me::me))
        .layer(cors)
        .with_state(state);

    let addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:3000".into());
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("Listening on {addr}");
    axum::serve(listener, app).await?;
    Ok(())
}
