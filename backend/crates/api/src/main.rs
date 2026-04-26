mod routes;

use anyhow::Context;
use axum::{
    Router,
    http::{HeaderName, HeaderValue, Method, header},
    routing::get,
};
use tower_http::cors::CorsLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Clone)]
pub struct AppState {
    pub pool: db::PgPool,
    pub spotify_client_id: String,
    pub spotify_client_secret: String,
    pub spotify_redirect_uri: String,
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

    let database_url = std::env::var("DATABASE_URL").context("DATABASE_URL must be set")?;
    let pool = db::connect(&database_url).await?;

    let state = AppState {
        pool,
        spotify_client_id: std::env::var("SPOTIFY_CLIENT_ID")
            .context("SPOTIFY_CLIENT_ID must be set")?,
        spotify_client_secret: std::env::var("SPOTIFY_CLIENT_SECRET")
            .context("SPOTIFY_CLIENT_SECRET must be set")?,
        spotify_redirect_uri: std::env::var("SPOTIFY_REDIRECT_URI")
            .context("SPOTIFY_REDIRECT_URI must be set")?,
    };

    let allowed_origin: HeaderValue = std::env::var("ALLOWED_ORIGIN")
        .context("ALLOWED_ORIGIN must be set")?
        .parse()
        .context("ALLOWED_ORIGIN is not a valid header value")?;

    let cors = CorsLayer::new()
        .allow_origin(allowed_origin)
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([
            header::CONTENT_TYPE,
            HeaderName::from_static("x-user-id"),
        ]);

    let app = Router::new()
        .route("/health", get(|| async { "ok" }))
        .nest("/auth", routes::auth::router())
        .nest("/car", routes::car::router())
        .route("/me", get(routes::me::me))
        // Party mode routes will be added here
        .layer(cors)
        .with_state(state);

    let addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:3000".into());
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("Listening on {addr}");
    axum::serve(listener, app).await?;
    Ok(())
}
