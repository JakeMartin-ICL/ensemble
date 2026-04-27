//! Party mode endpoints.

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use serde_json::Value;
use std::str::FromStr;
use uuid::Uuid;

use crate::AppState;

type ApiResult<T> = Result<Json<T>, (StatusCode, Json<Value>)>;

fn err(
    status: StatusCode,
    e: impl std::fmt::Display + std::fmt::Debug,
) -> (StatusCode, Json<Value>) {
    let msg = format!("{e:#?}");
    if status.is_server_error() {
        tracing::error!("{msg}");
    }
    (status, Json(serde_json::json!({ "error": format!("{e}") })))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/sessions", post(create_session))
        .route("/sessions/active", get(get_active_session))
        .route("/sessions/join", post(join_session))
        .route("/sessions/{id}", get(get_session))
        .route("/sessions/{id}/playback", get(get_playback))
        .route("/sessions/{id}/pause", post(pause_session))
        .route("/sessions/{id}/resume", post(resume_session))
        .route("/sessions/{id}/restart", post(restart_session))
        .route("/sessions/{id}/skip", post(skip_to_next))
        .route("/sessions/{id}/mode", post(update_mode))
        .route("/sessions/{id}/queue", get(get_queue))
        .route("/sessions/{id}/queue/add", post(add_queue_track))
        .route("/sessions/{id}/queue/search", get(search_queue_tracks))
        .route("/sessions/{id}/queue/reorder", post(reorder_queue))
        .route("/sessions/{id}/queue/remove", post(remove_queue_track))
        .route("/sessions/{id}/end", post(end_session))
        .route("/library/tracks", get(get_library_tracks))
        .route("/track/{uri}", get(get_track))
}

fn spawn_heartbeat(state: &AppState, session_id: Uuid) {
    let params = party::heartbeat::HeartbeatParams {
        session_id,
        pool: state.pool.clone(),
        spotify_client_id: state.spotify_client_id.clone(),
        spotify_client_secret: state.spotify_client_secret.clone(),
    };

    let fut = party::heartbeat::run(params);
    let handle = tokio::spawn(fut).abort_handle();
    state.heartbeat_tasks.insert(session_id, handle);
}

fn ensure_heartbeat(state: &AppState, session: &db::party::PartySession) {
    if session.is_active && !state.heartbeat_tasks.contains_key(&session.id) {
        spawn_heartbeat(state, session.id);
    }
}

fn stop_heartbeat(state: &AppState, session_id: Uuid) {
    if let Some((_, handle)) = state.heartbeat_tasks.remove(&session_id) {
        handle.abort();
    }
}

#[derive(serde::Serialize)]
struct SessionResponse {
    id: Uuid,
    host_user_id: Uuid,
    room_code: String,
    mode: String,
    current_track_uri: Option<String>,
    is_host: bool,
}

#[derive(serde::Deserialize)]
struct JoinSessionBody {
    room_code: String,
}

#[derive(serde::Serialize)]
struct PlaybackResponse {
    track_uri: String,
    progress_ms: u64,
    duration_ms: u64,
    is_playing: bool,
}

#[derive(serde::Serialize)]
struct QueueResponse {
    items: Vec<QueueItemResponse>,
}

#[derive(serde::Serialize, Clone)]
struct QueueItemResponse {
    id: Uuid,
    uri: String,
    name: Option<String>,
    artist: Option<String>,
    album_art_url: Option<String>,
    duration_ms: Option<u64>,
    position: usize,
    added_by_user_id: Option<Uuid>,
}

#[derive(serde::Deserialize)]
struct AddQueueTrackBody {
    track: AddQueueTrack,
}

#[derive(serde::Deserialize)]
struct AddQueueTrack {
    uri: String,
    name: Option<String>,
    artist: Option<String>,
    album_art_url: Option<String>,
    duration_ms: Option<u64>,
}

#[derive(serde::Deserialize)]
struct SearchQueueQuery {
    q: String,
    scope: Option<String>,
}

#[derive(serde::Deserialize)]
struct LibraryTracksQuery {
    limit: Option<usize>,
}

#[derive(serde::Serialize)]
struct TrackSearchResponse {
    results: Vec<TrackSearchResultResponse>,
}

#[derive(serde::Serialize, Clone)]
struct TrackSearchResultResponse {
    uri: String,
    name: Option<String>,
    artist: Option<String>,
    album_art_url: Option<String>,
    duration_ms: Option<u64>,
    playlist_index: Option<usize>,
    playlist_id: Option<String>,
    playlist_name: Option<String>,
}

#[derive(serde::Deserialize)]
struct ReorderQueueBody {
    item_id: Uuid,
    to_position: usize,
}

#[derive(serde::Deserialize)]
struct RemoveQueueTrackBody {
    item_id: Uuid,
}

#[derive(serde::Deserialize)]
struct UpdateModeBody {
    mode: String,
}

#[derive(serde::Serialize)]
struct TrackResponse {
    name: String,
    artist: String,
    album_art_url: Option<String>,
    duration_ms: u64,
}

impl From<spotify::player::PlaybackState> for PlaybackResponse {
    fn from(state: spotify::player::PlaybackState) -> Self {
        Self {
            track_uri: state.track_uri,
            progress_ms: state.progress_ms,
            duration_ms: state.duration_ms,
            is_playing: state.is_playing,
        }
    }
}

async fn get_access_token(
    state: &AppState,
    user_id: Uuid,
) -> Result<String, (StatusCode, Json<Value>)> {
    let user = db::users::get_user(&state.pool, user_id)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "user not found"))?;

    if user
        .token_expires_at
        .signed_duration_since(chrono::Utc::now())
        < chrono::Duration::seconds(60)
    {
        let tokens = spotify::auth::refresh_token(
            &user.refresh_token,
            &state.spotify_client_id,
            &state.spotify_client_secret,
        )
        .await
        .map_err(|e| err(StatusCode::BAD_GATEWAY, e))?;

        let new_expires_at =
            chrono::Utc::now() + chrono::Duration::seconds(tokens.expires_in as i64);
        db::users::update_tokens(&state.pool, user_id, &tokens.access_token, new_expires_at)
            .await
            .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;

        Ok(tokens.access_token)
    } else {
        Ok(user.access_token)
    }
}

async fn create_session(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<SessionResponse> {
    let user_id = crate::routes::session::user_id_from_headers(&state.pool, &headers).await?;

    db::party::deactivate_user_sessions(&state.pool, user_id)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let mut last_error: Option<anyhow::Error> = None;
    for _ in 0..5 {
        let room_code = room_code();
        match db::party::create_session(
            &state.pool,
            &db::party::NewPartySession {
                host_user_id: user_id,
                room_code,
            },
        )
        .await
        {
            Ok(session) => {
                stop_heartbeat(&state, session.id);
                spawn_heartbeat(&state, session.id);
                return Ok(Json(session_response(session, user_id)));
            }
            Err(e) => last_error = Some(e),
        }
    }

    Err(err(
        StatusCode::INTERNAL_SERVER_ERROR,
        last_error.unwrap_or_else(|| anyhow::anyhow!("creating party session")),
    ))
}

async fn get_active_session(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Option<SessionResponse>> {
    let user_id = crate::routes::session::user_id_from_headers(&state.pool, &headers).await?;
    let session = db::party::get_active_session(&state.pool, user_id)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;

    if let Some(ref s) = session {
        ensure_heartbeat(&state, s);
    }

    Ok(Json(session.map(|s| session_response(s, user_id))))
}

async fn join_session(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<JoinSessionBody>,
) -> ApiResult<SessionResponse> {
    let user_id = crate::routes::session::user_id_from_headers(&state.pool, &headers).await?;
    let code = normalize_room_code(&body.room_code);
    let session = db::party::get_session_by_room_code(&state.pool, &code)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "party room not found"))?;

    ensure_heartbeat(&state, &session);

    Ok(Json(session_response(session, user_id)))
}

async fn get_session(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
) -> ApiResult<SessionResponse> {
    let user_id = crate::routes::session::user_id_from_headers(&state.pool, &headers).await?;
    let session = get_existing_session(&state, session_id).await?;
    ensure_heartbeat(&state, &session);
    Ok(Json(session_response(session, user_id)))
}

async fn get_playback(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
) -> ApiResult<Option<PlaybackResponse>> {
    let user_id = crate::routes::session::user_id_from_headers(&state.pool, &headers).await?;
    let session = get_host_session(&state, session_id, user_id).await?;
    let access_token = get_access_token(&state, session.host_user_id).await?;

    let playback = spotify::player::get_playback_state(&access_token)
        .await
        .map_err(|e| err(StatusCode::BAD_GATEWAY, e))?;

    Ok(Json(playback.map(Into::into)))
}

async fn pause_session(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
) -> ApiResult<Option<PlaybackResponse>> {
    let user_id = crate::routes::session::user_id_from_headers(&state.pool, &headers).await?;
    let session = get_host_session(&state, session_id, user_id).await?;
    let access_token = get_access_token(&state, session.host_user_id).await?;

    spotify::player::pause_playback(&access_token)
        .await
        .map_err(|e| err(StatusCode::BAD_GATEWAY, e))?;

    let playback = spotify::player::get_playback_state(&access_token)
        .await
        .map_err(|e| err(StatusCode::BAD_GATEWAY, e))?;

    Ok(Json(playback.map(Into::into)))
}

async fn resume_session(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
) -> ApiResult<Option<PlaybackResponse>> {
    let user_id = crate::routes::session::user_id_from_headers(&state.pool, &headers).await?;
    let session = get_host_session(&state, session_id, user_id).await?;
    let access_token = get_access_token(&state, session.host_user_id).await?;

    spotify::player::resume_playback(&access_token)
        .await
        .map_err(|e| err(StatusCode::BAD_GATEWAY, e))?;

    let playback = spotify::player::get_playback_state(&access_token)
        .await
        .map_err(|e| err(StatusCode::BAD_GATEWAY, e))?;

    Ok(Json(playback.map(Into::into)))
}

async fn restart_session(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
) -> ApiResult<Option<PlaybackResponse>> {
    let user_id = crate::routes::session::user_id_from_headers(&state.pool, &headers).await?;
    let session = get_host_session(&state, session_id, user_id).await?;
    let access_token = get_access_token(&state, session.host_user_id).await?;

    spotify::player::seek_to_start(&access_token)
        .await
        .map_err(|e| err(StatusCode::BAD_GATEWAY, e))?;

    let playback = spotify::player::get_playback_state(&access_token)
        .await
        .map_err(|e| err(StatusCode::BAD_GATEWAY, e))?;

    Ok(Json(playback.map(Into::into)))
}

async fn skip_to_next(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
) -> ApiResult<SessionResponse> {
    let user_id = crate::routes::session::user_id_from_headers(&state.pool, &headers).await?;
    let session = get_host_session(&state, session_id, user_id).await?;
    let access_token = get_access_token(&state, session.host_user_id).await?;
    let item = db::party::pop_next_queue_item(&state.pool, session_id)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?
        .ok_or_else(|| err(StatusCode::UNPROCESSABLE_ENTITY, "queue is empty"))?;

    spotify::player::start_track(&access_token, &item.track.uri)
        .await
        .map_err(|e| err(StatusCode::BAD_GATEWAY, e))?;

    db::party::set_current_track(&state.pool, session_id, Some(&item.track.uri))
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let updated = get_existing_session(&state, session_id).await?;
    Ok(Json(session_response(updated, user_id)))
}

async fn get_queue(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
) -> ApiResult<QueueResponse> {
    crate::routes::session::user_id_from_headers(&state.pool, &headers).await?;
    get_existing_session(&state, session_id).await?;
    Ok(Json(build_queue_response(&state, session_id).await?))
}

async fn update_mode(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    Json(body): Json<UpdateModeBody>,
) -> ApiResult<SessionResponse> {
    let user_id = crate::routes::session::user_id_from_headers(&state.pool, &headers).await?;
    get_host_session(&state, session_id, user_id).await?;
    let mode =
        db::party::PartyMode::from_str(&body.mode).map_err(|e| err(StatusCode::BAD_REQUEST, e))?;

    let session = db::party::set_mode(&state.pool, session_id, mode)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(session_response(session, user_id)))
}

async fn search_queue_tracks(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    Query(query): Query<SearchQueueQuery>,
) -> ApiResult<TrackSearchResponse> {
    let user_id = crate::routes::session::user_id_from_headers(&state.pool, &headers).await?;
    get_existing_session(&state, session_id).await?;

    let term = query.q.trim();
    let scope = query.scope.unwrap_or_else(|| "local".to_string());
    if term.len() < 2 {
        return Ok(Json(TrackSearchResponse {
            results: Vec::new(),
        }));
    }

    let access_token = get_access_token(&state, user_id).await?;
    let results = if scope == "spotify" {
        spotify::playlist::search_tracks(&access_token, term)
            .await
            .map_err(|e| err(StatusCode::BAD_GATEWAY, e))?
            .into_iter()
            .map(|track| TrackSearchResultResponse {
                uri: track.uri,
                name: Some(track.name),
                artist: Some(track.artist),
                album_art_url: track.album_art_url,
                duration_ms: Some(track.duration_ms),
                playlist_index: None,
                playlist_id: None,
                playlist_name: None,
            })
            .collect()
    } else {
        search_user_playlist_tracks(&access_token, term)
            .await
            .map_err(|e| err(StatusCode::BAD_GATEWAY, e))?
    };

    Ok(Json(TrackSearchResponse { results }))
}

async fn add_queue_track(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    Json(body): Json<AddQueueTrackBody>,
) -> ApiResult<QueueResponse> {
    let user_id = crate::routes::session::user_id_from_headers(&state.pool, &headers).await?;
    let session = get_existing_session(&state, session_id).await?;
    if !session.is_active {
        return Err(err(StatusCode::GONE, "party session has ended"));
    }

    let position = db::party::next_position(&state.pool, session_id)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;

    db::party::add_queue_item(
        &state.pool,
        &db::party::NewPartyQueueItem {
            session_id,
            position,
            added_by_user_id: user_id,
            track: db::party::PartyTrack {
                uri: body.track.uri,
                name: body.track.name,
                artist: body.track.artist,
                album_art_url: body.track.album_art_url,
                duration_ms: body.track.duration_ms,
            },
        },
    )
    .await
    .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(build_queue_response(&state, session_id).await?))
}

async fn reorder_queue(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    Json(body): Json<ReorderQueueBody>,
) -> ApiResult<QueueResponse> {
    let user_id = crate::routes::session::user_id_from_headers(&state.pool, &headers).await?;
    get_queue_editor_session(&state, session_id, user_id).await?;

    let items = db::party::queue_items(&state.pool, session_id)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let mut ids = items.iter().map(|item| item.id).collect::<Vec<_>>();
    let Some(from_position) = ids.iter().position(|id| *id == body.item_id) else {
        return Err(err(StatusCode::NOT_FOUND, "queue item not found"));
    };
    let item_id = ids.remove(from_position);
    let to_position = body.to_position.min(ids.len());
    ids.insert(to_position, item_id);

    db::party::update_queue_positions(&state.pool, session_id, &ids)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(build_queue_response(&state, session_id).await?))
}

async fn remove_queue_track(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    Json(body): Json<RemoveQueueTrackBody>,
) -> ApiResult<QueueResponse> {
    let user_id = crate::routes::session::user_id_from_headers(&state.pool, &headers).await?;
    get_queue_editor_session(&state, session_id, user_id).await?;

    db::party::remove_queue_item(&state.pool, session_id, body.item_id)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(build_queue_response(&state, session_id).await?))
}

async fn get_library_tracks(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<LibraryTracksQuery>,
) -> ApiResult<TrackSearchResponse> {
    const DEFAULT_LIMIT: usize = 1_500;
    const MAX_LIMIT: usize = 3_000;

    let user_id = crate::routes::session::user_id_from_headers(&state.pool, &headers).await?;
    let access_token = get_access_token(&state, user_id).await?;
    let limit = query.limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT);
    let results = user_playlist_tracks(&access_token, limit)
        .await
        .map_err(|e| err(StatusCode::BAD_GATEWAY, e))?;

    Ok(Json(TrackSearchResponse { results }))
}

async fn end_session(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
) -> ApiResult<Value> {
    let user_id = crate::routes::session::user_id_from_headers(&state.pool, &headers).await?;
    get_host_session(&state, session_id, user_id).await?;

    db::party::end_session(&state.pool, session_id)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;

    stop_heartbeat(&state, session_id);

    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn get_track(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(uri): Path<String>,
) -> ApiResult<TrackResponse> {
    let user_id = crate::routes::session::user_id_from_headers(&state.pool, &headers).await?;
    let access_token = get_access_token(&state, user_id).await?;

    let track_id = uri
        .split(':')
        .nth(2)
        .ok_or_else(|| err(StatusCode::BAD_REQUEST, "invalid track URI"))?;

    let details = spotify::player::get_track(&access_token, track_id)
        .await
        .map_err(|e| err(StatusCode::BAD_GATEWAY, e))?;

    Ok(Json(TrackResponse {
        name: details.name,
        artist: details.artist,
        album_art_url: details.album_art_url,
        duration_ms: details.duration_ms,
    }))
}

async fn build_queue_response(
    state: &AppState,
    session_id: Uuid,
) -> Result<QueueResponse, (StatusCode, Json<Value>)> {
    let items = db::party::queue_items(&state.pool, session_id)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?
        .into_iter()
        .enumerate()
        .map(|(position, item)| QueueItemResponse {
            id: item.id,
            uri: item.track.uri.clone(),
            name: item.track.name.clone(),
            artist: item.track.artist.clone(),
            album_art_url: item.track.album_art_url.clone(),
            duration_ms: item.track.duration_ms,
            position,
            added_by_user_id: item.added_by_user_id,
        })
        .collect();

    Ok(QueueResponse { items })
}

async fn search_user_playlist_tracks(
    access_token: &str,
    term: &str,
) -> anyhow::Result<Vec<TrackSearchResultResponse>> {
    let needle = term.to_lowercase();
    let mut results = Vec::new();

    for track in user_playlist_tracks(access_token, 1_500).await? {
        let name = track.name.as_deref().unwrap_or("");
        let artist = track.artist.as_deref().unwrap_or("");
        if !name.to_lowercase().contains(&needle) && !artist.to_lowercase().contains(&needle) {
            continue;
        }

        results.push(track);
        if results.len() >= 20 {
            return Ok(results);
        }
    }

    Ok(results)
}

async fn user_playlist_tracks(
    access_token: &str,
    limit: usize,
) -> anyhow::Result<Vec<TrackSearchResultResponse>> {
    let playlists = spotify::playlist::get_user_playlists(access_token).await?;
    let mut results = Vec::new();

    for (playlist_index, playlist) in playlists.iter().enumerate() {
        if results.len() >= limit {
            break;
        }

        let tracks = match spotify::playlist::get_tracks(access_token, &playlist.id).await {
            Ok(tracks) => tracks,
            Err(e) => {
                tracing::warn!(
                    playlist_id = playlist.id,
                    "party library: failed to fetch playlist tracks: {e:#}"
                );
                continue;
            }
        };

        for track in tracks {
            if results.len() >= limit {
                break;
            }

            results.push(TrackSearchResultResponse {
                uri: track.uri,
                name: Some(track.name),
                artist: Some(track.artist),
                album_art_url: track.album_art_url,
                duration_ms: Some(track.duration_ms),
                playlist_index: Some(playlist_index),
                playlist_id: Some(playlist.id.clone()),
                playlist_name: Some(playlist.name.clone()),
            });
        }
    }

    Ok(results)
}

async fn get_existing_session(
    state: &AppState,
    session_id: Uuid,
) -> Result<db::party::PartySession, (StatusCode, Json<Value>)> {
    db::party::get_session(&state.pool, session_id)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "party session not found"))
}

async fn get_host_session(
    state: &AppState,
    session_id: Uuid,
    user_id: Uuid,
) -> Result<db::party::PartySession, (StatusCode, Json<Value>)> {
    let session = get_existing_session(state, session_id).await?;
    if session.host_user_id != user_id {
        return Err(err(StatusCode::FORBIDDEN, "host controls only"));
    }
    Ok(session)
}

async fn get_queue_editor_session(
    state: &AppState,
    session_id: Uuid,
    user_id: Uuid,
) -> Result<db::party::PartySession, (StatusCode, Json<Value>)> {
    let session = get_existing_session(state, session_id).await?;
    if !session.is_active {
        return Err(err(StatusCode::GONE, "party session has ended"));
    }
    if session.host_user_id == user_id || session.mode == db::party::PartyMode::SharedQueue.as_str()
    {
        return Ok(session);
    }

    Err(err(StatusCode::FORBIDDEN, "host controls only"))
}

fn session_response(session: db::party::PartySession, user_id: Uuid) -> SessionResponse {
    let is_host = session.host_user_id == user_id;
    SessionResponse {
        id: session.id,
        host_user_id: session.host_user_id,
        room_code: session.room_code,
        mode: session.mode,
        current_track_uri: session.current_track_uri,
        is_host,
    }
}

fn room_code() -> String {
    Uuid::new_v4()
        .simple()
        .to_string()
        .chars()
        .take(6)
        .collect::<String>()
        .to_uppercase()
}

fn normalize_room_code(code: &str) -> String {
    code.trim()
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .collect::<String>()
        .to_uppercase()
}
