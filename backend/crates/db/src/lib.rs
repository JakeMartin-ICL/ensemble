//! Database access layer.
//! Wraps sqlx pool and exposes typed queries for each domain.

pub mod car;
pub mod users;
// pub mod party; // Not yet implemented

pub use sqlx::PgPool;

pub async fn connect(database_url: &str) -> anyhow::Result<PgPool> {
    let pool = sqlx::PgPool::connect(database_url).await?;
    Ok(pool)
}
