use axum::http::{HeaderMap, StatusCode};
use serde_json::Value;
use uuid::Uuid;

pub fn bearer_token(headers: &HeaderMap) -> Result<&str, (StatusCode, axum::Json<Value>)> {
    let value = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| err(StatusCode::UNAUTHORIZED, "missing Authorization header"))?;

    value
        .strip_prefix("Bearer ")
        .filter(|token| !token.is_empty())
        .ok_or_else(|| err(StatusCode::UNAUTHORIZED, "invalid Authorization header"))
}

pub async fn user_id_from_headers(
    pool: &db::PgPool,
    headers: &HeaderMap,
) -> Result<Uuid, (StatusCode, axum::Json<Value>)> {
    let token = bearer_token(headers)?;
    db::users::user_id_for_session(pool, token)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?
        .ok_or_else(|| err(StatusCode::UNAUTHORIZED, "invalid or expired session"))
}

fn err(status: StatusCode, msg: impl std::fmt::Display) -> (StatusCode, axum::Json<Value>) {
    (
        status,
        axum::Json(serde_json::json!({ "error": msg.to_string() })),
    )
}
