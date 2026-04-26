//! Database access layer.
//! Wraps sqlx pool and exposes typed queries for each domain.

pub mod car;
pub mod users;
// pub mod party; // Not yet implemented

pub use sqlx::PgPool;

pub async fn connect() -> anyhow::Result<PgPool> {
    use sqlx::postgres::PgConnectOptions;

    let opts = PgConnectOptions::new()
        .host(&std::env::var("DB_HOST")?)
        .port(std::env::var("DB_PORT")?.parse()?)
        .username(&std::env::var("DB_USER")?)
        .password(&std::env::var("DB_PASSWORD")?)
        .database(&std::env::var("DB_NAME")?);

    Ok(PgPool::connect_with(opts).await?)
}
