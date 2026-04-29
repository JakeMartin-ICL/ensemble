//! Spotify OAuth endpoints.
//! POST /auth/callback - exchange code for tokens
//! POST /auth/refresh  - refresh an access token

use crate::AppState;
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::post,
    Json, Router,
};
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
    client_id: String,
    code_verifier: String,
}

#[derive(serde::Serialize)]
struct CallbackResponse {
    user_id: Uuid,
    spotify_id: String,
    display_name: String,
    session_token: String,
}

async fn callback(
    State(state): State<AppState>,
    Json(body): Json<CallbackRequest>,
) -> ApiResult<CallbackResponse> {
    let client_id = body.client_id.trim();
    if client_id.is_empty() {
        return Err(err(
            StatusCode::BAD_REQUEST,
            "Spotify client ID is required",
        ));
    }

    let tokens = spotify::auth::exchange_code_pkce(
        &body.code,
        client_id,
        &body.code_verifier,
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
        client_id,
        token_expires_at,
    )
    .await
    .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let session_token = format!("ens_{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let session_expires_at = Utc::now() + chrono::Duration::days(30);
    db::users::create_session(&state.pool, user_id, &session_token, session_expires_at)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;

    state.auth_sessions.insert(
        session_token.clone(),
        crate::CachedAuthSession {
            user_id,
            expires_at: session_expires_at,
        },
    );
    state.spotify_tokens.insert(
        user_id,
        crate::CachedSpotifyToken {
            access_token: tokens.access_token.clone(),
            expires_at: token_expires_at,
        },
    );

    Ok(Json(CallbackResponse {
        user_id,
        spotify_id: me.id,
        display_name: me.display_name,
        session_token,
    }))
}

#[derive(serde::Serialize)]
struct RefreshResponse {
    access_token: String,
    expires_at: String,
}

async fn refresh(State(state): State<AppState>, headers: HeaderMap) -> ApiResult<RefreshResponse> {
    let user_id = crate::routes::session::cached_user_id_from_headers(&state, &headers).await?;
    let user = db::users::get_user(&state.pool, user_id)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "user not found"))?;

    let client_id = user.spotify_client_id.as_deref().ok_or_else(|| {
        err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Spotify client ID is missing; reconnect Spotify",
        )
    })?;
    let tokens = spotify::auth::refresh_token_pkce(&user.refresh_token, client_id)
        .await
        .map_err(|e| err(StatusCode::BAD_GATEWAY, e))?;

    let token_expires_at = Utc::now() + chrono::Duration::seconds(tokens.expires_in as i64);

    db::users::update_tokens(
        &state.pool,
        user_id,
        &tokens.access_token,
        tokens.refresh_token.as_deref(),
        token_expires_at,
    )
    .await
    .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;

    state.spotify_tokens.insert(
        user_id,
        crate::CachedSpotifyToken {
            access_token: tokens.access_token.clone(),
            expires_at: token_expires_at,
        },
    );

    Ok(Json(RefreshResponse {
        access_token: tokens.access_token,
        expires_at: token_expires_at.to_rfc3339(),
    }))
}
