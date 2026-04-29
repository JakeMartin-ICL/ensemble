//! Database access layer.
//! Wraps sqlx pool and exposes typed queries for each domain.

use anyhow::Context;

pub mod party;
pub mod spotify_cache;
pub mod users;
pub mod weave;

pub use sqlx::PgPool;

pub async fn connect() -> anyhow::Result<PgPool> {
    use std::time::Duration;

    use sqlx::postgres::{PgConnectOptions, PgPoolOptions};

    let opts = PgConnectOptions::new()
        .host(&std::env::var("DB_HOST")?)
        .port(std::env::var("DB_PORT")?.parse()?)
        .username(&std::env::var("DB_USER")?)
        .password(&std::env::var("DB_PASSWORD")?)
        .database(&std::env::var("DB_NAME")?);

    let max_connections = env_u32("DB_MAX_CONNECTIONS", 5)?;
    let min_connections = env_u32("DB_MIN_CONNECTIONS", 0)?.min(max_connections);
    let acquire_timeout_seconds = env_u64("DB_ACQUIRE_TIMEOUT_SECONDS", 10)?;
    let idle_timeout_seconds = env_u64("DB_IDLE_TIMEOUT_SECONDS", 300)?;

    Ok(PgPoolOptions::new()
        .max_connections(max_connections)
        .min_connections(min_connections)
        .acquire_timeout(Duration::from_secs(acquire_timeout_seconds))
        .idle_timeout(Duration::from_secs(idle_timeout_seconds))
        .connect_with(opts)
        .await?)
}

fn env_u32(name: &str, default: u32) -> anyhow::Result<u32> {
    std::env::var(name)
        .map(|value| {
            value
                .parse()
                .with_context(|| format!("{name} must be an unsigned integer"))
        })
        .unwrap_or(Ok(default))
}

fn env_u64(name: &str, default: u64) -> anyhow::Result<u64> {
    std::env::var(name)
        .map(|value| {
            value
                .parse()
                .with_context(|| format!("{name} must be an unsigned integer"))
        })
        .unwrap_or(Ok(default))
}
