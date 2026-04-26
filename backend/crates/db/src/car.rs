//! Database queries for car mode sessions.

use sqlx::PgPool;
use uuid::Uuid;

pub struct CarSession {
    pub id: Uuid,
    pub host_spotify_id: String,
    pub playlist_a_id: String,
    pub playlist_b_id: String,
    pub current_turn: Turn,
    pub playlist_a_order: Vec<String>, // shuffled track URIs
    pub playlist_b_order: Vec<String>,
    pub playlist_a_index: i32,
    pub playlist_b_index: i32,
}

#[derive(Debug, Clone, PartialEq, sqlx::Type, serde::Serialize, serde::Deserialize)]
#[sqlx(type_name = "turn", rename_all = "lowercase")]
pub enum Turn {
    A,
    B,
}

pub async fn create_session(_pool: &PgPool, _session: &CarSession) -> anyhow::Result<Uuid> {
    todo!()
}

pub async fn get_session(_pool: &PgPool, _id: Uuid) -> anyhow::Result<Option<CarSession>> {
    todo!()
}

pub async fn update_session(_pool: &PgPool, _session: &CarSession) -> anyhow::Result<()> {
    todo!()
}
