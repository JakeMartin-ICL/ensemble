mod routes;

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
pub struct AppState {
    pub pool: db::PgPool,
    pub spotify_client_id: String,
    pub spotify_client_secret: String,
    pub spotify_redirect_uri: String,
    pub heartbeat_tasks: Arc<DashMap<Uuid, AbortHandle>>,
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
        spotify_client_id: std::env::var("SPOTIFY_CLIENT_ID")
            .context("SPOTIFY_CLIENT_ID must be set")?,
        spotify_client_secret: std::env::var("SPOTIFY_CLIENT_SECRET")
            .context("SPOTIFY_CLIENT_SECRET must be set")?,
        spotify_redirect_uri: std::env::var("SPOTIFY_REDIRECT_URI")
            .context("SPOTIFY_REDIRECT_URI must be set")?,
        heartbeat_tasks: Arc::new(DashMap::new()),
    };

    let allowed_origin: HeaderValue = std::env::var("ALLOWED_ORIGIN")
        .context("ALLOWED_ORIGIN must be set")?
        .parse()
        .context("ALLOWED_ORIGIN is not a valid header value")?;

    let cors = CorsLayer::new()
        .allow_origin(allowed_origin)
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION]);

    let app = Router::new()
        .route("/health", get(|| async { "ok" }))
        .nest("/auth", routes::auth::router())
        .nest("/car", routes::car::router())
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
