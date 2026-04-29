use axum::http::{HeaderMap, StatusCode};
use chrono::Utc;
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

pub async fn cached_user_id_from_headers(
    state: &crate::AppState,
    headers: &HeaderMap,
) -> Result<Uuid, (StatusCode, axum::Json<Value>)> {
    let token = bearer_token(headers)?;
    cached_user_id_for_token(state, token).await
}

pub async fn cached_user_id_for_token(
    state: &crate::AppState,
    token: &str,
) -> Result<Uuid, (StatusCode, axum::Json<Value>)> {
    if let Some(cached) = state.auth_sessions.get(token) {
        if cached.expires_at > Utc::now() {
            return Ok(cached.user_id);
        }
    }

    state.auth_sessions.remove(token);
    let session = db::users::user_session_for_token(&state.pool, token)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?
        .ok_or_else(|| err(StatusCode::UNAUTHORIZED, "invalid or expired session"))?;

    state.auth_sessions.insert(
        token.to_string(),
        crate::CachedAuthSession {
            user_id: session.user_id,
            expires_at: session.expires_at,
        },
    );

    Ok(session.user_id)
}

pub async fn cached_access_token(
    state: &crate::AppState,
    user_id: Uuid,
) -> Result<String, (StatusCode, axum::Json<Value>)> {
    if let Some(cached) = state.spotify_tokens.get(&user_id) {
        if cached.expires_at.signed_duration_since(Utc::now()) > chrono::Duration::seconds(60) {
            return Ok(cached.access_token.clone());
        }
    }

    state.spotify_tokens.remove(&user_id);
    let user = db::users::get_user(&state.pool, user_id)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "user not found"))?;

    if user.token_expires_at.signed_duration_since(Utc::now()) > chrono::Duration::seconds(60) {
        state.spotify_tokens.insert(
            user_id,
            crate::CachedSpotifyToken {
                access_token: user.access_token.clone(),
                expires_at: user.token_expires_at,
            },
        );
        return Ok(user.access_token);
    }

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

    Ok(tokens.access_token)
}

fn err(status: StatusCode, msg: impl std::fmt::Display) -> (StatusCode, axum::Json<Value>) {
    (
        status,
        axum::Json(serde_json::json!({ "error": msg.to_string() })),
    )
}
