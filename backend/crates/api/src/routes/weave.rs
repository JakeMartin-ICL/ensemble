//! Weave (weave mode) endpoints.

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use serde_json::Value;
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
        .route("/sessions/{id}/skip-song", post(skip_song))
        .route("/sessions/{id}/skip-turn", post(skip_turn))
        .route("/sessions/{id}/playback", get(get_playback))
        .route("/sessions/{id}/pause", post(pause_session))
        .route("/sessions/{id}/resume", post(resume_session))
        .route("/sessions/{id}/restart", post(restart_session))
        .route("/sessions/{id}/queue", get(get_queue))
        .route("/sessions/{id}/queue/add", post(add_queue_track))
        .route("/sessions/{id}/queue/search", get(search_queue_tracks))
        .route(
            "/sessions/{id}/queue/{playlist_index}/reorder",
            post(reorder_playlist_queue),
        )
        .route("/sessions/{id}/end", post(end_session))
        .route("/playlists", get(get_playlists))
        .route("/track/{uri}", get(get_track))
}

#[derive(serde::Deserialize)]
struct CreateSessionBody {
    playlist_ids: Vec<String>,
}

#[derive(serde::Serialize)]
struct SessionResponse {
    id: Uuid,
    playlists: Vec<SessionPlaylistResponse>,
    current_playlist_index: i32,
    current_playlist_id: String,
    current_playlist_name: String,
    current_track_uri: Option<String>,
}

#[derive(serde::Serialize)]
struct SessionPlaylistResponse {
    id: String,
    name: String,
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
    unified: Vec<QueueItemResponse>,
    playlists: Vec<PlaylistQueueResponse>,
}

#[derive(serde::Serialize, Clone)]
struct QueueItemResponse {
    uri: String,
    name: Option<String>,
    artist: Option<String>,
    album_art_url: Option<String>,
    duration_ms: Option<u64>,
    playlist_index: usize,
    playlist_id: String,
    playlist_name: String,
    position: usize,
}

#[derive(serde::Serialize)]
struct PlaylistQueueResponse {
    playlist_index: usize,
    playlist_id: String,
    playlist_name: String,
    items: Vec<QueueItemResponse>,
}

#[derive(serde::Deserialize)]
struct ReorderQueueBody {
    from_position: usize,
    to_position: usize,
}

#[derive(serde::Deserialize)]
struct SearchQueueQuery {
    q: String,
    scope: Option<String>,
}

#[derive(serde::Serialize)]
struct TrackSearchResponse {
    scope: String,
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
struct AddQueueTrackBody {
    playlist_index: usize,
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

impl From<db::weave::WeaveSession> for SessionResponse {
    fn from(s: db::weave::WeaveSession) -> Self {
        let current = current_playlist(&s);
        let current_playlist_id = current.map(|p| p.id.clone()).unwrap_or_default();
        let current_playlist_name = current.map(|p| p.name.clone()).unwrap_or_default();

        Self {
            id: s.id,
            playlists: s
                .playlists()
                .iter()
                .map(|p| SessionPlaylistResponse {
                    id: p.id.clone(),
                    name: p.name.clone(),
                })
                .collect(),
            current_playlist_index: s.current_playlist_index,
            current_playlist_id,
            current_playlist_name,
            current_track_uri: s.current_track_uri,
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

fn spawn_heartbeat(state: &AppState, session_id: Uuid) {
    let params = weave::heartbeat::HeartbeatParams {
        session_id,
        pool: state.pool.clone(),
        spotify_client_id: state.spotify_client_id.clone(),
        spotify_client_secret: state.spotify_client_secret.clone(),
    };

    let fut = weave::heartbeat::run(params);
    let handle = tokio::spawn(fut).abort_handle();
    state.heartbeat_tasks.insert(session_id, handle);
}

fn stop_heartbeat(state: &AppState, session_id: Uuid) {
    if let Some((_, handle)) = state.heartbeat_tasks.remove(&session_id) {
        handle.abort();
    }
}

async fn create_session(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreateSessionBody>,
) -> ApiResult<SessionResponse> {
    let user_id = crate::routes::session::user_id_from_headers(&state.pool, &headers).await?;
    let access_token = get_access_token(&state, user_id).await?;

    if body.playlist_ids.len() < 2 {
        return Err(err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "choose at least two playlists",
        ));
    }

    let mut playlists = Vec::with_capacity(body.playlist_ids.len());
    for playlist_id in body.playlist_ids {
        let (tracks, name) = tokio::try_join!(
            spotify::playlist::get_tracks(&access_token, &playlist_id),
            playlist_name(&access_token, &playlist_id),
        )
        .map_err(|e| err(StatusCode::BAD_GATEWAY, e))?;

        let mut order = tracks;
        weave::session::shuffle(&mut order);

        if order.is_empty() {
            return Err(err(
                StatusCode::UNPROCESSABLE_ENTITY,
                format!("playlist {name} has no playable Spotify tracks"),
            ));
        }

        playlists.push(db::weave::PlaylistState {
            id: playlist_id,
            name,
            order: order
                .into_iter()
                .map(|track| db::weave::PlaylistTrack {
                    uri: track.uri,
                    name: Some(track.name),
                    artist: Some(track.artist),
                    album_art_url: track.album_art_url,
                    duration_ms: Some(track.duration_ms),
                })
                .collect(),
        });
    }

    let first_track = playlists[0].order[0].uri.clone();
    let preview_track = playlists
        .get(1)
        .and_then(|p| p.order.first())
        .map(|track| track.uri.clone());
    let playback_uris = playback_uris(&first_track, preview_track.as_deref());

    spotify::player::start_tracks(&access_token, &playback_uris)
        .await
        .map_err(|e| err(StatusCode::BAD_GATEWAY, e))?;

    db::weave::deactivate_user_sessions(&state.pool, user_id)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let session = db::weave::create_session(
        &state.pool,
        &db::weave::NewWeaveSession {
            host_user_id: user_id,
            playlist_track_indexes: vec![0; playlists.len()],
            playlists,
            current_playlist_index: 0,
            current_track_uri: Some(first_track.clone()),
            queued_track_uri: preview_track,
        },
    )
    .await
    .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;

    stop_heartbeat(&state, session.id);
    spawn_heartbeat(&state, session.id);

    Ok(Json(session.into()))
}

async fn get_active_session(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Option<SessionResponse>> {
    let user_id = crate::routes::session::user_id_from_headers(&state.pool, &headers).await?;

    let session = db::weave::get_active_session(&state.pool, user_id)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;

    if let Some(ref s) = session {
        if !state.heartbeat_tasks.contains_key(&s.id) {
            spawn_heartbeat(&state, s.id);
        }
    }

    Ok(Json(session.map(Into::into)))
}

async fn skip_song(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
) -> ApiResult<SessionResponse> {
    let user_id = crate::routes::session::user_id_from_headers(&state.pool, &headers).await?;
    let session = get_verified_session(&state, session_id, user_id).await?;
    let access_token = get_access_token(&state, user_id).await?;

    let advance = weave::session::next_same_playlist(&session)
        .ok_or_else(|| err(StatusCode::UNPROCESSABLE_ENTITY, "playlist is empty"))?;
    let playback_uris = playback_uris(&advance.track_uri, None);

    spotify::player::start_tracks(&access_token, &playback_uris)
        .await
        .map_err(|e| err(StatusCode::BAD_GATEWAY, e))?;

    db::weave::update_position_and_track_and_clear_queue(
        &state.pool,
        session_id,
        advance.playlist_index,
        &advance.track_uri,
        &advance.track_indexes,
    )
    .await
    .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let updated = db::weave::get_session(&state.pool, session_id)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "session not found"))?;

    Ok(Json(updated.into()))
}

async fn skip_turn(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
) -> ApiResult<SessionResponse> {
    let user_id = crate::routes::session::user_id_from_headers(&state.pool, &headers).await?;
    let session = get_verified_session(&state, session_id, user_id).await?;
    let access_token = get_access_token(&state, user_id).await?;

    let advance = weave::session::next_playlist(&session)
        .ok_or_else(|| err(StatusCode::UNPROCESSABLE_ENTITY, "playlist is empty"))?;
    let playback_uris = playback_uris(&advance.track_uri, None);

    spotify::player::start_tracks(&access_token, &playback_uris)
        .await
        .map_err(|e| err(StatusCode::BAD_GATEWAY, e))?;

    db::weave::update_position_and_track_and_clear_queue(
        &state.pool,
        session_id,
        advance.playlist_index,
        &advance.track_uri,
        &advance.track_indexes,
    )
    .await
    .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let updated = db::weave::get_session(&state.pool, session_id)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "session not found"))?;

    Ok(Json(updated.into()))
}

async fn get_playback(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
) -> ApiResult<Option<PlaybackResponse>> {
    let user_id = crate::routes::session::user_id_from_headers(&state.pool, &headers).await?;
    get_verified_session(&state, session_id, user_id).await?;
    let access_token = get_access_token(&state, user_id).await?;

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
    get_verified_session(&state, session_id, user_id).await?;
    let access_token = get_access_token(&state, user_id).await?;

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
    get_verified_session(&state, session_id, user_id).await?;
    let access_token = get_access_token(&state, user_id).await?;

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
    get_verified_session(&state, session_id, user_id).await?;
    let access_token = get_access_token(&state, user_id).await?;

    spotify::player::seek_to_start(&access_token)
        .await
        .map_err(|e| err(StatusCode::BAD_GATEWAY, e))?;

    let playback = spotify::player::get_playback_state(&access_token)
        .await
        .map_err(|e| err(StatusCode::BAD_GATEWAY, e))?;

    Ok(Json(playback.map(Into::into)))
}

async fn get_queue(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
) -> ApiResult<QueueResponse> {
    let user_id = crate::routes::session::user_id_from_headers(&state.pool, &headers).await?;
    let session = get_verified_session(&state, session_id, user_id).await?;

    Ok(Json(build_queue_response(&session)))
}

async fn search_queue_tracks(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    Query(query): Query<SearchQueueQuery>,
) -> ApiResult<TrackSearchResponse> {
    let user_id = crate::routes::session::user_id_from_headers(&state.pool, &headers).await?;
    let session = get_verified_session(&state, session_id, user_id).await?;
    let term = query.q.trim();
    if term.len() < 2 {
        return Ok(Json(TrackSearchResponse {
            scope: query.scope.unwrap_or_else(|| "local".to_string()),
            results: Vec::new(),
        }));
    }

    let scope = query.scope.unwrap_or_else(|| "local".to_string());
    let results = if scope == "spotify" {
        let access_token = get_access_token(&state, user_id).await?;
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
        search_session_tracks(&session, term)
    };

    Ok(Json(TrackSearchResponse { scope, results }))
}

async fn add_queue_track(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    Json(body): Json<AddQueueTrackBody>,
) -> ApiResult<QueueResponse> {
    let user_id = crate::routes::session::user_id_from_headers(&state.pool, &headers).await?;
    let mut session = get_verified_session(&state, session_id, user_id).await?;

    let Some(playlist) = session.playlists.0.get_mut(body.playlist_index) else {
        return Err(err(StatusCode::NOT_FOUND, "playlist not found"));
    };

    let current_index = session
        .playlist_track_indexes
        .get(body.playlist_index)
        .copied()
        .unwrap_or(0);
    let track = db::weave::PlaylistTrack {
        uri: body.track.uri,
        name: body.track.name,
        artist: body.track.artist,
        album_art_url: body.track.album_art_url,
        duration_ms: body.track.duration_ms,
    };
    let new_current_index = insert_track_after_current(playlist, current_index, track);

    if session.playlist_track_indexes.len() < session.playlists.0.len() {
        session
            .playlist_track_indexes
            .resize(session.playlists.0.len(), 0);
    }
    session.playlist_track_indexes[body.playlist_index] = new_current_index;

    db::weave::update_playlists_and_track_indexes(
        &state.pool,
        session_id,
        &session.playlists.0,
        &session.playlist_track_indexes,
    )
    .await
    .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let updated = db::weave::get_session(&state.pool, session_id)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "session not found"))?;

    Ok(Json(build_queue_response(&updated)))
}

async fn reorder_playlist_queue(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((session_id, playlist_index)): Path<(Uuid, usize)>,
    Json(body): Json<ReorderQueueBody>,
) -> ApiResult<QueueResponse> {
    let user_id = crate::routes::session::user_id_from_headers(&state.pool, &headers).await?;
    let mut session = get_verified_session(&state, session_id, user_id).await?;

    let Some(playlist) = session.playlists.0.get_mut(playlist_index) else {
        return Err(err(StatusCode::NOT_FOUND, "playlist not found"));
    };

    let positions = queue_positions(
        playlist.order.len(),
        session
            .playlist_track_indexes
            .get(playlist_index)
            .copied()
            .unwrap_or(0),
    );

    let Some(from_abs) = positions.get(body.from_position).copied() else {
        return Err(err(
            StatusCode::BAD_REQUEST,
            "invalid source queue position",
        ));
    };
    let Some(to_abs) = positions.get(body.to_position).copied() else {
        return Err(err(
            StatusCode::BAD_REQUEST,
            "invalid target queue position",
        ));
    };

    let item = playlist.order.remove(from_abs);
    playlist.order.insert(to_abs, item);

    db::weave::update_playlists(&state.pool, session_id, &session.playlists.0)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let updated = db::weave::get_session(&state.pool, session_id)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "session not found"))?;

    Ok(Json(build_queue_response(&updated)))
}

async fn end_session(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
) -> ApiResult<Value> {
    let user_id = crate::routes::session::user_id_from_headers(&state.pool, &headers).await?;
    tracing::debug!("end_session: session_id={session_id} user_id={user_id}");
    get_verified_session(&state, session_id, user_id).await?;

    db::weave::end_session(&state.pool, session_id)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;

    stop_heartbeat(&state, session_id);

    Ok(Json(serde_json::json!({ "ok": true })))
}

#[derive(serde::Serialize)]
struct PlaylistSummaryResponse {
    id: String,
    name: String,
    track_count: u32,
    image_url: Option<String>,
}

async fn get_playlists(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Vec<PlaylistSummaryResponse>> {
    let user_id = crate::routes::session::user_id_from_headers(&state.pool, &headers).await?;
    let access_token = get_access_token(&state, user_id).await?;

    let playlists = spotify::playlist::get_user_playlists(&access_token)
        .await
        .map_err(|e| err(StatusCode::BAD_GATEWAY, e))?;

    Ok(Json(
        playlists
            .into_iter()
            .map(|p| PlaylistSummaryResponse {
                id: p.id,
                name: p.name,
                track_count: p.track_count,
                image_url: p.image_url,
            })
            .collect(),
    ))
}

#[derive(serde::Serialize)]
struct TrackResponse {
    name: String,
    artist: String,
    album_art_url: Option<String>,
    duration_ms: u64,
}

async fn get_track(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(uri): Path<String>,
) -> ApiResult<TrackResponse> {
    let user_id = crate::routes::session::user_id_from_headers(&state.pool, &headers).await?;
    let access_token = get_access_token(&state, user_id).await?;

    // Extract track ID from URI: "spotify:track:<id>"
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

async fn get_verified_session(
    state: &AppState,
    session_id: Uuid,
    user_id: Uuid,
) -> Result<db::weave::WeaveSession, (StatusCode, Json<Value>)> {
    let session = db::weave::get_session(&state.pool, session_id)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "session not found"))?;

    if session.host_user_id != user_id {
        return Err(err(StatusCode::FORBIDDEN, "not your session"));
    }

    Ok(session)
}

async fn playlist_name(access_token: &str, playlist_id: &str) -> anyhow::Result<String> {
    #[derive(serde::Deserialize)]
    struct Resp {
        name: String,
    }
    let resp = reqwest::Client::new()
        .get(format!(
            "https://api.spotify.com/v1/playlists/{playlist_id}?fields=name"
        ))
        .bearer_auth(access_token)
        .send()
        .await?
        .error_for_status()?
        .json::<Resp>()
        .await?;
    Ok(resp.name)
}

fn playback_uris(track_uri: &str, queued_track_uri: Option<&str>) -> Vec<String> {
    let mut uris = vec![track_uri.to_string()];
    if let Some(uri) = queued_track_uri {
        uris.push(uri.to_string());
    }
    uris
}

fn current_playlist(session: &db::weave::WeaveSession) -> Option<&db::weave::PlaylistState> {
    let index = usize::try_from(session.current_playlist_index).ok()?;
    session.playlists().get(index)
}

fn build_queue_response(session: &db::weave::WeaveSession) -> QueueResponse {
    let playlists = session
        .playlists()
        .iter()
        .enumerate()
        .map(|(playlist_index, playlist)| PlaylistQueueResponse {
            playlist_index,
            playlist_id: playlist.id.clone(),
            playlist_name: playlist.name.clone(),
            items: playlist_queue_items(session, playlist_index, 12),
        })
        .collect::<Vec<_>>();

    QueueResponse {
        unified: unified_queue_items(session, 12),
        playlists,
    }
}

fn unified_queue_items(session: &db::weave::WeaveSession, limit: usize) -> Vec<QueueItemResponse> {
    let playlist_count = session.playlists().len();
    let Some(current_index) = usize::try_from(session.current_playlist_index)
        .ok()
        .filter(|index| *index < playlist_count)
    else {
        return Vec::new();
    };

    let mut items = Vec::new();
    let mut seen_by_playlist = vec![0usize; playlist_count];
    let mut next_playlist_index = current_index;

    for _ in 0..(limit * playlist_count.max(1)) {
        next_playlist_index = (next_playlist_index + 1) % playlist_count;
        let playlist_items = playlist_queue_items(
            session,
            next_playlist_index,
            seen_by_playlist[next_playlist_index] + 1,
        );
        let Some(item) = playlist_items.last().cloned() else {
            continue;
        };
        seen_by_playlist[next_playlist_index] += 1;
        items.push(item);
        if items.len() >= limit {
            break;
        }
    }

    items
}

fn playlist_queue_items(
    session: &db::weave::WeaveSession,
    playlist_index: usize,
    limit: usize,
) -> Vec<QueueItemResponse> {
    let Some(playlist) = session.playlists().get(playlist_index) else {
        return Vec::new();
    };

    queue_positions(
        playlist.order.len(),
        session
            .playlist_track_indexes
            .get(playlist_index)
            .copied()
            .unwrap_or(0),
    )
    .into_iter()
    .take(limit)
    .enumerate()
    .map(|(position, track_index)| QueueItemResponse {
        uri: playlist.order[track_index].uri.clone(),
        name: playlist.order[track_index].name.clone(),
        artist: playlist.order[track_index].artist.clone(),
        album_art_url: playlist.order[track_index].album_art_url.clone(),
        duration_ms: playlist.order[track_index].duration_ms,
        playlist_index,
        playlist_id: playlist.id.clone(),
        playlist_name: playlist.name.clone(),
        position,
    })
    .collect()
}

fn search_session_tracks(
    session: &db::weave::WeaveSession,
    term: &str,
) -> Vec<TrackSearchResultResponse> {
    let needle = term.to_lowercase();
    let mut results = Vec::new();

    for (playlist_index, playlist) in session.playlists().iter().enumerate() {
        for track in &playlist.order {
            let name = track.name.as_deref().unwrap_or("");
            let artist = track.artist.as_deref().unwrap_or("");
            if !name.to_lowercase().contains(&needle) && !artist.to_lowercase().contains(&needle) {
                continue;
            }

            if results.iter().any(|r: &TrackSearchResultResponse| {
                r.uri == track.uri && r.playlist_index == Some(playlist_index)
            }) {
                continue;
            }

            results.push(TrackSearchResultResponse {
                uri: track.uri.clone(),
                name: track.name.clone(),
                artist: track.artist.clone(),
                album_art_url: track.album_art_url.clone(),
                duration_ms: track.duration_ms,
                playlist_index: Some(playlist_index),
                playlist_id: Some(playlist.id.clone()),
                playlist_name: Some(playlist.name.clone()),
            });

            if results.len() >= 20 {
                return results;
            }
        }
    }

    results
}

fn insert_track_after_current(
    playlist: &mut db::weave::PlaylistState,
    current_index: i32,
    track: db::weave::PlaylistTrack,
) -> i32 {
    let current = usize::try_from(current_index)
        .ok()
        .filter(|index| *index < playlist.order.len())
        .unwrap_or(0);

    let source_index = queue_positions(playlist.order.len(), current_index)
        .into_iter()
        .find(|index| playlist.order[*index].uri == track.uri);

    let mut adjusted_current = current;
    let item = if let Some(source) = source_index {
        let removed = playlist.order.remove(source);
        if source < adjusted_current {
            adjusted_current -= 1;
        }
        removed
    } else {
        track
    };

    let insert_at = (adjusted_current + 1).min(playlist.order.len());
    playlist.order.insert(insert_at, item);
    adjusted_current as i32
}

fn queue_positions(len: usize, current_index: i32) -> Vec<usize> {
    if len == 0 {
        return Vec::new();
    }

    let current = usize::try_from(current_index)
        .ok()
        .filter(|index| *index < len)
        .unwrap_or(0);

    (1..len).map(|offset| (current + offset) % len).collect()
}
