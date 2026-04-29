//! GET /me — return current user info.

use crate::AppState;
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    Json,
};
use serde_json::Value;

type ApiResult<T> = Result<Json<T>, (StatusCode, Json<Value>)>;

fn err(status: StatusCode, msg: impl std::fmt::Display) -> (StatusCode, Json<Value>) {
    (
        status,
        Json(serde_json::json!({ "error": msg.to_string() })),
    )
}

#[derive(serde::Serialize)]
pub struct MeResponse {
    display_name: String,
    active_device: Option<DeviceInfo>,
}

#[derive(serde::Serialize)]
struct DeviceInfo {
    name: String,
    #[serde(rename = "type")]
    device_type: String,
    is_active: bool,
}

pub async fn me(State(state): State<AppState>, headers: HeaderMap) -> ApiResult<MeResponse> {
    let user_id = crate::routes::session::cached_user_id_from_headers(&state, &headers).await?;

    let user = db::users::get_user(&state.pool, user_id)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "user not found"))?;

    Ok(Json(MeResponse {
        display_name: user.display_name,
        active_device: None,
    }))
}
