//! Spotify OAuth endpoints.
//! POST /auth/callback - exchange code for tokens
//! POST /auth/refresh  - refresh an access token

use crate::AppState;
use axum::{extract::State, http::StatusCode, routing::post, Json, Router};
use chrono::Utc;
use serde_json::Value;
use uuid::Uuid;

type ApiResult<T> = Result<Json<T>, (StatusCode, Json<Value>)>;

fn err(status: StatusCode, msg: impl std::fmt::Display) -> (StatusCode, Json<Value>) {
    (
        status,
        Json(serde_json::json!({ "error": msg.to_string() })),
    )
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/callback", post(callback))
        .route("/refresh", post(refresh))
}

#[derive(serde::Deserialize)]
struct CallbackRequest {
    code: String,
}

#[derive(serde::Serialize)]
struct CallbackResponse {
    user_id: Uuid,
    spotify_id: String,
    display_name: String,
}

async fn callback(
    State(state): State<AppState>,
    Json(body): Json<CallbackRequest>,
) -> ApiResult<CallbackResponse> {
    let tokens = spotify::auth::exchange_code(
        &body.code,
        &state.spotify_client_id,
        &state.spotify_client_secret,
        &state.spotify_redirect_uri,
    )
    .await
    .map_err(|e| err(StatusCode::BAD_GATEWAY, e))?;

    let me = spotify::player::get_me(&tokens.access_token)
        .await
        .map_err(|e| err(StatusCode::BAD_GATEWAY, e))?;

    let refresh_token = tokens.refresh_token.ok_or_else(|| {
        err(
            StatusCode::BAD_GATEWAY,
            "Spotify did not return a refresh token",
        )
    })?;

    let token_expires_at = Utc::now() + chrono::Duration::seconds(tokens.expires_in as i64);

    let user_id = db::users::upsert_user(
        &state.pool,
        &me.id,
        &me.display_name,
        &tokens.access_token,
        &refresh_token,
        token_expires_at,
    )
    .await
    .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(CallbackResponse {
        user_id,
        spotify_id: me.id,
        display_name: me.display_name,
    }))
}

#[derive(serde::Deserialize)]
struct RefreshRequest {
    user_id: Uuid,
}

#[derive(serde::Serialize)]
struct RefreshResponse {
    access_token: String,
    expires_at: String,
}

async fn refresh(
    State(state): State<AppState>,
    Json(body): Json<RefreshRequest>,
) -> ApiResult<RefreshResponse> {
    let user = db::users::get_user(&state.pool, body.user_id)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "user not found"))?;

    let tokens = spotify::auth::refresh_token(
        &user.refresh_token,
        &state.spotify_client_id,
        &state.spotify_client_secret,
    )
    .await
    .map_err(|e| err(StatusCode::BAD_GATEWAY, e))?;

    let token_expires_at = Utc::now() + chrono::Duration::seconds(tokens.expires_in as i64);

    db::users::update_tokens(
        &state.pool,
        body.user_id,
        &tokens.access_token,
        token_expires_at,
    )
    .await
    .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(RefreshResponse {
        access_token: tokens.access_token,
        expires_at: token_expires_at.to_rfc3339(),
    }))
}
