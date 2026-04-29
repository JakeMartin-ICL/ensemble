//! GET /me — return current user info from Spotify.

use crate::AppState;
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    Json,
};
use chrono::Utc;
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
    let user_id = crate::routes::session::user_id_from_headers(&state.pool, &headers).await?;

    let user = db::users::get_user(&state.pool, user_id)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "user not found"))?;

    let access_token = if user.token_expires_at.signed_duration_since(Utc::now())
        < chrono::Duration::seconds(60)
    {
        let client_id = user.spotify_client_id.as_deref().ok_or_else(|| {
            err(
                StatusCode::UNPROCESSABLE_ENTITY,
                "Spotify client ID is missing; reconnect Spotify",
            )
        })?;
        let tokens = spotify::auth::refresh_token_pkce(&user.refresh_token, client_id)
            .await
            .map_err(|e| err(StatusCode::BAD_GATEWAY, e))?;

        let new_expires_at = Utc::now() + chrono::Duration::seconds(tokens.expires_in as i64);
        db::users::update_tokens(
            &state.pool,
            user_id,
            &tokens.access_token,
            tokens.refresh_token.as_deref(),
            new_expires_at,
        )
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;

        tokens.access_token
    } else {
        user.access_token
    };

    let spotify_me = spotify::player::get_me(&access_token)
        .await
        .map_err(|e| err(StatusCode::BAD_GATEWAY, e))?;

    let active_device = spotify::player::get_player(&access_token)
        .await
        .map_err(|e| err(StatusCode::BAD_GATEWAY, e))?
        .map(|d| DeviceInfo {
            name: d.name,
            device_type: d.device_type,
            is_active: d.is_active,
        });

    Ok(Json(MeResponse {
        display_name: spotify_me.display_name,
        active_device,
    }))
}
