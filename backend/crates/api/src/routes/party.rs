//! Party mode endpoints.

use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{delete, get, post},
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::{collections::HashMap, collections::HashSet, str::FromStr};
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
        .route("/sessions/{id}/settings", post(update_settings))
        .route("/sessions/{id}/queue", get(get_queue))
        .route("/sessions/{id}/queue/add", post(add_queue_track))
        .route(
            "/sessions/{id}/queue/add-playlist",
            post(add_queue_playlist),
        )
        .route("/sessions/{id}/source-queue", get(get_source_queue))
        .route(
            "/sessions/{id}/source-queue/{item_id}/disabled",
            post(set_source_queue_item_disabled),
        )
        .route("/sessions/{id}/queue/search", get(search_queue_tracks))
        .route("/sessions/{id}/queue/reorder", post(reorder_queue))
        .route("/sessions/{id}/queue/remove", post(remove_queue_track))
        .route("/sessions/{id}/queue/{item_id}/vote", post(vote_queue_item))
        .route(
            "/sessions/{id}/queue/{item_id}/vote",
            delete(unvote_queue_item),
        )
        .route(
            "/sessions/{id}/queue/{item_id}/pin",
            delete(unpin_queue_item),
        )
        .route("/sessions/{id}/export", get(get_export_preview))
        .route("/sessions/{id}/export/playlist", post(export_playlist))
        .route("/sessions/{id}/export/csv", get(export_csv))
        .route("/sessions/{id}/end", post(end_session))
        .route("/library/tracks", get(get_library_tracks))
        .route("/track/{uri}", get(get_track))
}

fn spawn_heartbeat(state: &AppState, session_id: Uuid) {
    let params = party::heartbeat::HeartbeatParams {
        session_id,
        pool: state.pool.clone(),
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
    allow_guest_playlist_adds: bool,
    source_min_queue_size: i32,
    add_added_tracks_to_source: bool,
    show_queue_attribution: bool,
    current_track_uri: Option<String>,
    is_host: bool,
    is_guest: bool,
    display_name: Option<String>,
    session_token: Option<String>,
}

#[derive(serde::Deserialize)]
struct CreateSessionBody {
    source_playlist_id: Option<String>,
    source_min_queue_size: Option<i32>,
    add_added_tracks_to_source: Option<bool>,
}

#[derive(serde::Deserialize)]
struct JoinSessionBody {
    room_code: String,
    display_name: Option<String>,
}

#[derive(serde::Serialize)]
struct PlaybackResponse {
    track_uri: String,
    progress_ms: u64,
    duration_ms: u64,
    is_playing: bool,
    observed_at_ms: i64,
}

#[derive(serde::Serialize)]
struct QueueResponse {
    items: Vec<QueueItemResponse>,
}

#[derive(serde::Serialize)]
struct SourceQueueResponse {
    items: Vec<SourceQueueItemResponse>,
}

#[derive(serde::Serialize)]
struct ExportPreviewResponse {
    mode: String,
    items: Vec<ExportItemResponse>,
}

#[derive(serde::Serialize, Clone)]
struct ExportItemResponse {
    id: String,
    source: String,
    session_id: Uuid,
    uri: String,
    name: Option<String>,
    artist: Option<String>,
    album_art_url: Option<String>,
    duration_ms: Option<u64>,
    position: usize,
    play_order: Option<i32>,
    source_position: Option<i32>,
    added_by_user_id: Option<Uuid>,
    added_by_display_name: Option<String>,
    created_at: Option<DateTime<Utc>>,
}

#[derive(serde::Serialize, Clone)]
struct VoterResponse {
    user_id: String,
    display_name: Option<String>,
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
    pin_position: Option<i32>,
    vote_count: i64,
    user_voted: bool,
    voters: Vec<VoterResponse>,
    added_by_user_id: Option<Uuid>,
    added_by_display_name: Option<String>,
}

#[derive(serde::Serialize, Clone)]
struct SourceQueueItemResponse {
    id: Uuid,
    uri: String,
    name: Option<String>,
    artist: Option<String>,
    album_art_url: Option<String>,
    duration_ms: Option<u64>,
    position: usize,
    deferred: bool,
    disabled: bool,
    added_by_user_id: Option<Uuid>,
    added_by_display_name: Option<String>,
}

#[derive(serde::Deserialize)]
struct SourceQueueDisabledBody {
    disabled: bool,
}

#[derive(serde::Deserialize)]
struct AddQueueTrackBody {
    track: AddQueueTrack,
}

#[derive(serde::Deserialize)]
struct AddQueuePlaylistBody {
    playlist_id: String,
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
    playlists: Vec<PlaylistSearchResultResponse>,
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

#[derive(serde::Serialize, Clone)]
struct PlaylistSearchResultResponse {
    id: String,
    name: String,
    track_count: u32,
    image_url: Option<String>,
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
struct ExportQuery {
    mode: Option<String>,
}

#[derive(serde::Deserialize)]
struct ExportPlaylistBody {
    mode: Option<String>,
    name: Option<String>,
}

#[derive(serde::Serialize)]
struct ExportPlaylistResponse {
    playlist_id: String,
    url: String,
    track_count: usize,
}

#[derive(serde::Deserialize)]
struct VoteQueueItemBody {
    vote: bool,
}

#[derive(serde::Deserialize)]
struct UpdateModeBody {
    mode: String,
}

#[derive(serde::Deserialize)]
struct UpdateSettingsBody {
    allow_guest_playlist_adds: Option<bool>,
    source_min_queue_size: Option<i32>,
    add_added_tracks_to_source: Option<bool>,
    show_queue_attribution: Option<bool>,
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
            observed_at_ms: chrono::Utc::now().timestamp_millis(),
        }
    }
}

#[derive(Clone)]
enum PartyActor {
    User {
        id: Uuid,
        display_name: String,
    },
    Guest {
        id: Uuid,
        session_id: Uuid,
        display_name: String,
    },
}

impl PartyActor {
    fn user_id(&self) -> Option<Uuid> {
        match self {
            Self::User { id, .. } => Some(*id),
            Self::Guest { .. } => None,
        }
    }

    fn guest_id(&self) -> Option<Uuid> {
        match self {
            Self::User { .. } => None,
            Self::Guest { id, .. } => Some(*id),
        }
    }

    fn voter_id(&self) -> String {
        match self {
            Self::User { id, .. } | Self::Guest { id, .. } => id.to_string(),
        }
    }

    fn display_name(&self) -> &str {
        match self {
            Self::User { display_name, .. } | Self::Guest { display_name, .. } => display_name,
        }
    }

    fn is_guest(&self) -> bool {
        matches!(self, Self::Guest { .. })
    }

    fn kind(&self) -> &'static str {
        match self {
            Self::User { .. } => "user",
            Self::Guest { .. } => "guest",
        }
    }

    fn id(&self) -> Uuid {
        match self {
            Self::User { id, .. } | Self::Guest { id, .. } => *id,
        }
    }

    fn guest_session_id(&self) -> Option<Uuid> {
        match self {
            Self::User { .. } => None,
            Self::Guest { session_id, .. } => Some(*session_id),
        }
    }

    fn can_access_session(&self, session_id: Uuid) -> bool {
        match self {
            Self::User { .. } => true,
            Self::Guest {
                session_id: guest_session_id,
                ..
            } => *guest_session_id == session_id,
        }
    }
}

async fn actor_from_headers(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<PartyActor, (StatusCode, Json<Value>)> {
    let token = crate::routes::session::bearer_token(headers)?;
    if let Some(user_id) = db::users::user_id_for_session(&state.pool, token)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?
    {
        let user = db::users::get_user(&state.pool, user_id)
            .await
            .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?
            .ok_or_else(|| err(StatusCode::UNAUTHORIZED, "invalid or expired session"))?;
        return Ok(PartyActor::User {
            id: user.id,
            display_name: user.display_name,
        });
    }

    if let Some(guest) = db::party::guest_for_session_token(&state.pool, token)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?
    {
        return Ok(PartyActor::Guest {
            id: guest.id,
            session_id: guest.session_id,
            display_name: guest.display_name,
        });
    }

    Err(err(StatusCode::UNAUTHORIZED, "invalid or expired session"))
}

async fn user_actor_from_headers(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<PartyActor, (StatusCode, Json<Value>)> {
    match actor_from_headers(state, headers).await? {
        actor @ PartyActor::User { .. } => Ok(actor),
        PartyActor::Guest { .. } => Err(err(StatusCode::FORBIDDEN, "Spotify login required")),
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
        let client_id = user.spotify_client_id.as_deref().ok_or_else(|| {
            err(
                StatusCode::UNPROCESSABLE_ENTITY,
                "Spotify client ID is missing; reconnect Spotify",
            )
        })?;
        let tokens = spotify::auth::refresh_token_pkce(&user.refresh_token, client_id)
            .await
            .map_err(|e| err(StatusCode::BAD_GATEWAY, e))?;

        let new_expires_at =
            chrono::Utc::now() + chrono::Duration::seconds(tokens.expires_in as i64);
        db::users::update_tokens(
            &state.pool,
            user_id,
            &tokens.access_token,
            tokens.refresh_token.as_deref(),
            new_expires_at,
        )
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
    Json(body): Json<CreateSessionBody>,
) -> ApiResult<SessionResponse> {
    let actor = user_actor_from_headers(&state, &headers).await?;
    let user_id = actor.user_id().expect("user actor should have user id");
    let source_min_queue_size = body.source_min_queue_size.unwrap_or(0).clamp(0, 25);

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
                source_min_queue_size,
                add_added_tracks_to_source: body.add_added_tracks_to_source.unwrap_or(false),
            },
        )
        .await
        {
            Ok(session) => {
                if let Some(playlist_id) = body.source_playlist_id.as_deref() {
                    seed_source_playlist(&state, user_id, session.id, playlist_id).await?;
                    db::party::refill_queue_from_source(&state.pool, session.id)
                        .await
                        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
                }
                stop_heartbeat(&state, session.id);
                spawn_heartbeat(&state, session.id);
                let updated = get_existing_session(&state, session.id).await?;
                return Ok(Json(session_response(updated, &actor, None)));
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
    let actor = user_actor_from_headers(&state, &headers).await?;
    let user_id = actor.user_id().expect("user actor should have user id");
    let session = db::party::get_active_session(&state.pool, user_id)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;

    if let Some(ref s) = session {
        ensure_heartbeat(&state, s);
    }

    Ok(Json(session.map(|s| session_response(s, &actor, None))))
}

async fn join_session(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<JoinSessionBody>,
) -> ApiResult<SessionResponse> {
    let code = normalize_room_code(&body.room_code);
    let session = db::party::get_session_by_room_code(&state.pool, &code)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "party room not found"))?;

    ensure_heartbeat(&state, &session);

    if let Ok(actor @ PartyActor::User { .. }) = actor_from_headers(&state, &headers).await {
        return Ok(Json(session_response(session, &actor, None)));
    }

    let display_name = body
        .display_name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .ok_or_else(|| err(StatusCode::BAD_REQUEST, "guest display name is required"))?;
    let session_token = format!(
        "ens_guest_{}{}",
        Uuid::new_v4().simple(),
        Uuid::new_v4().simple()
    );
    let expires_at = Utc::now() + chrono::Duration::days(1);
    let guest = db::party::create_guest_session(
        &state.pool,
        session.id,
        display_name,
        &session_token,
        expires_at,
    )
    .await
    .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let actor = PartyActor::Guest {
        id: guest.id,
        session_id: guest.session_id,
        display_name: guest.display_name,
    };

    Ok(Json(session_response(session, &actor, Some(session_token))))
}

async fn get_session(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
) -> ApiResult<SessionResponse> {
    let actor = actor_from_headers(&state, &headers).await?;
    let session = get_existing_session(&state, session_id).await?;
    if !actor.can_access_session(session.id) {
        return Err(err(StatusCode::FORBIDDEN, "not your party session"));
    }
    ensure_heartbeat(&state, &session);
    Ok(Json(session_response(session, &actor, None)))
}

async fn get_playback(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
) -> ApiResult<Option<PlaybackResponse>> {
    let actor = actor_from_headers(&state, &headers).await?;
    let session = get_existing_session(&state, session_id).await?;
    if !actor.can_access_session(session.id) {
        return Err(err(StatusCode::FORBIDDEN, "not your party session"));
    }

    Ok(Json(playback_from_session(&session)))
}

fn playback_from_session(session: &db::party::PartySession) -> Option<PlaybackResponse> {
    Some(PlaybackResponse {
        track_uri: session.playback_track_uri.clone()?,
        progress_ms: session.playback_progress_ms? as u64,
        duration_ms: session.playback_duration_ms? as u64,
        is_playing: session.playback_is_playing?,
        observed_at_ms: session.playback_updated_at?.timestamp_millis(),
    })
}

async fn pause_session(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
) -> ApiResult<Option<PlaybackResponse>> {
    let user_id = user_actor_from_headers(&state, &headers)
        .await?
        .user_id()
        .expect("user actor should have user id");
    let session = get_host_session(&state, session_id, user_id).await?;
    let access_token = get_access_token(&state, session.host_user_id).await?;

    spotify::player::pause_playback(&access_token)
        .await
        .map_err(|e| err(StatusCode::BAD_GATEWAY, e))?;

    let playback = spotify::player::get_playback_state(&access_token)
        .await
        .map_err(|e| err(StatusCode::BAD_GATEWAY, e))?;

    if let Some(ref p) = playback {
        if let Err(e) = db::party::update_playback_state(
            &state.pool,
            session_id,
            &p.track_uri,
            p.progress_ms as i64,
            p.duration_ms as i64,
            p.is_playing,
        )
        .await
        {
            tracing::warn!("failed to cache playback state after pause: {e:#}");
        }
    }

    Ok(Json(playback.map(Into::into)))
}

async fn resume_session(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
) -> ApiResult<Option<PlaybackResponse>> {
    let user_id = user_actor_from_headers(&state, &headers)
        .await?
        .user_id()
        .expect("user actor should have user id");
    let session = get_host_session(&state, session_id, user_id).await?;
    let access_token = get_access_token(&state, session.host_user_id).await?;

    spotify::player::resume_playback(&access_token)
        .await
        .map_err(|e| err(StatusCode::BAD_GATEWAY, e))?;

    let playback = spotify::player::get_playback_state(&access_token)
        .await
        .map_err(|e| err(StatusCode::BAD_GATEWAY, e))?;

    if let Some(ref p) = playback {
        if let Err(e) = db::party::update_playback_state(
            &state.pool,
            session_id,
            &p.track_uri,
            p.progress_ms as i64,
            p.duration_ms as i64,
            p.is_playing,
        )
        .await
        {
            tracing::warn!("failed to cache playback state after resume: {e:#}");
        }
    }

    Ok(Json(playback.map(Into::into)))
}

async fn restart_session(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
) -> ApiResult<Option<PlaybackResponse>> {
    let user_id = user_actor_from_headers(&state, &headers)
        .await?
        .user_id()
        .expect("user actor should have user id");
    let session = get_host_session(&state, session_id, user_id).await?;
    let access_token = get_access_token(&state, session.host_user_id).await?;

    spotify::player::seek_to_start(&access_token)
        .await
        .map_err(|e| err(StatusCode::BAD_GATEWAY, e))?;

    let playback = spotify::player::get_playback_state(&access_token)
        .await
        .map_err(|e| err(StatusCode::BAD_GATEWAY, e))?;

    if let Some(ref p) = playback {
        if let Err(e) = db::party::update_playback_state(
            &state.pool,
            session_id,
            &p.track_uri,
            p.progress_ms as i64,
            p.duration_ms as i64,
            p.is_playing,
        )
        .await
        {
            tracing::warn!("failed to cache playback state after restart: {e:#}");
        }
    }

    Ok(Json(playback.map(Into::into)))
}

async fn skip_to_next(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
) -> ApiResult<SessionResponse> {
    let actor = user_actor_from_headers(&state, &headers).await?;
    let user_id = actor.user_id().expect("user actor should have user id");
    let session = get_host_session(&state, session_id, user_id).await?;
    let access_token = get_access_token(&state, session.host_user_id).await?;
    db::party::refill_queue_from_source(&state.pool, session_id)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
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
    db::party::add_played_track(
        &state.pool,
        &db::party::NewPartyPlayedTrack {
            session_id,
            track: item.track.0,
            added_by_user_id: item.added_by_user_id,
            added_by_guest_id: item.added_by_guest_id,
        },
    )
    .await
    .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    db::party::refill_queue_from_source(&state.pool, session_id)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let updated = get_existing_session(&state, session_id).await?;
    Ok(Json(session_response(updated, &actor, None)))
}

async fn get_queue(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
) -> ApiResult<QueueResponse> {
    let actor = actor_from_headers(&state, &headers).await?;
    let session = get_existing_session(&state, session_id).await?;
    if !actor.can_access_session(session.id) {
        return Err(err(StatusCode::FORBIDDEN, "not your party session"));
    }
    db::party::refill_queue_from_source(&state.pool, session_id)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(
        build_queue_response(&state, session_id, &actor).await?,
    ))
}

async fn update_mode(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    Json(body): Json<UpdateModeBody>,
) -> ApiResult<SessionResponse> {
    let actor = user_actor_from_headers(&state, &headers).await?;
    let user_id = actor.user_id().expect("user actor should have user id");
    let current = get_host_session(&state, session_id, user_id).await?;
    let mode =
        db::party::PartyMode::from_str(&body.mode).map_err(|e| err(StatusCode::BAD_REQUEST, e))?;

    if mode == db::party::PartyMode::VotedQueue {
        db::party::sort_voted_queue(&state.pool, session_id)
            .await
            .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    } else if current.mode == db::party::PartyMode::VotedQueue.as_str() {
        db::party::clear_all_pins(&state.pool, session_id)
            .await
            .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    }

    let session = db::party::set_mode(&state.pool, session_id, mode)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(session_response(session, &actor, None)))
}

async fn update_settings(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    Json(body): Json<UpdateSettingsBody>,
) -> ApiResult<SessionResponse> {
    let actor = user_actor_from_headers(&state, &headers).await?;
    let user_id = actor.user_id().expect("user actor should have user id");
    get_host_session(&state, session_id, user_id).await?;
    if let Some(allow_guest_playlist_adds) = body.allow_guest_playlist_adds {
        db::party::set_allow_guest_playlist_adds(
            &state.pool,
            session_id,
            allow_guest_playlist_adds,
        )
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    }
    if body.source_min_queue_size.is_some() || body.add_added_tracks_to_source.is_some() {
        let existing = get_existing_session(&state, session_id).await?;
        db::party::set_source_settings(
            &state.pool,
            session_id,
            body.source_min_queue_size
                .unwrap_or(existing.source_min_queue_size)
                .clamp(0, 25),
            body.add_added_tracks_to_source
                .unwrap_or(existing.add_added_tracks_to_source),
        )
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
        db::party::refill_queue_from_source(&state.pool, session_id)
            .await
            .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    }
    if let Some(show_queue_attribution) = body.show_queue_attribution {
        db::party::set_show_queue_attribution(&state.pool, session_id, show_queue_attribution)
            .await
            .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    }

    let updated = get_existing_session(&state, session_id).await?;
    Ok(Json(session_response(updated, &actor, None)))
}

async fn search_queue_tracks(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    Query(query): Query<SearchQueueQuery>,
) -> ApiResult<TrackSearchResponse> {
    let actor = actor_from_headers(&state, &headers).await?;
    let session = get_existing_session(&state, session_id).await?;
    if !actor.can_access_session(session.id) {
        return Err(err(StatusCode::FORBIDDEN, "not your party session"));
    }

    let term = query.q.trim();
    let scope = query.scope.unwrap_or_else(|| "local".to_string());
    tracing::info!(
        session_id = %session_id,
        host_user_id = %session.host_user_id,
        actor_kind = actor.kind(),
        actor_id = %actor.id(),
        actor_guest_session_id = ?actor.guest_session_id(),
        scope = %scope,
        term = %term,
        "party search: request"
    );
    if term.len() < 2 {
        return Ok(Json(TrackSearchResponse {
            results: Vec::new(),
            playlists: Vec::new(),
        }));
    }

    let results = if scope == "spotify" {
        let access_token = match &actor {
            PartyActor::User { id, .. } => get_access_token(&state, *id).await?,
            PartyActor::Guest { .. } => get_access_token(&state, session.host_user_id).await?,
        };
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
        search_party_cached_tracks(&state, session_id, term, &actor).await?
    };
    let playlists = Vec::new();

    Ok(Json(TrackSearchResponse { results, playlists }))
}

async fn add_queue_track(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    Json(body): Json<AddQueueTrackBody>,
) -> ApiResult<QueueResponse> {
    let actor = actor_from_headers(&state, &headers).await?;
    let session = get_existing_session(&state, session_id).await?;
    if !actor.can_access_session(session.id) {
        return Err(err(StatusCode::FORBIDDEN, "not your party session"));
    }
    if !session.is_active {
        return Err(err(StatusCode::GONE, "party session has ended"));
    }

    let position = db::party::next_position(&state.pool, session_id)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let track = db::party::PartyTrack {
        uri: body.track.uri,
        name: body.track.name,
        artist: body.track.artist,
        album_art_url: body.track.album_art_url,
        duration_ms: body.track.duration_ms,
    };

    let item = db::party::add_queue_item(
        &state.pool,
        &db::party::NewPartyQueueItem {
            session_id,
            position,
            added_by_user_id: actor.user_id(),
            added_by_guest_id: actor.guest_id(),
            track: track.clone(),
        },
    )
    .await
    .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;

    if session.add_added_tracks_to_source {
        db::party::defer_source_queue_track(
            &state.pool,
            session_id,
            &track,
            actor.user_id(),
            actor.guest_id(),
        )
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    }

    if session.mode == db::party::PartyMode::VotedQueue.as_str() {
        db::party::vote_queue_item(
            &state.pool,
            session_id,
            item.id,
            actor.user_id(),
            actor.guest_id(),
        )
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
        db::party::sort_voted_queue(&state.pool, session_id)
            .await
            .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    }

    Ok(Json(
        build_queue_response(&state, session_id, &actor).await?,
    ))
}

async fn add_queue_playlist(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    Json(body): Json<AddQueuePlaylistBody>,
) -> ApiResult<QueueResponse> {
    let actor = user_actor_from_headers(&state, &headers).await?;
    let user_id = actor.user_id().expect("user actor should have user id");
    let session = get_playlist_adder_session(&state, session_id, user_id).await?;
    if !session.is_active {
        return Err(err(StatusCode::GONE, "party session has ended"));
    }

    let access_token = get_access_token(&state, user_id).await?;
    let tracks = spotify::playlist::get_tracks(&access_token, &body.playlist_id)
        .await
        .map_err(|e| err(StatusCode::BAD_GATEWAY, e))?
        .into_iter()
        .map(|track| db::party::PartyTrack {
            uri: track.uri,
            name: Some(track.name),
            artist: Some(track.artist),
            album_art_url: track.album_art_url,
            duration_ms: Some(track.duration_ms),
        })
        .collect::<Vec<_>>();

    if tracks.is_empty() {
        return Err(err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "playlist has no playable Spotify tracks",
        ));
    }

    let mut tracks = tracks;
    weave::session::shuffle(&mut tracks);
    db::party::append_source_queue_items(&state.pool, session_id, &tracks, Some(user_id), None)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    db::party::refill_queue_from_source(&state.pool, session_id)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(
        build_queue_response(&state, session_id, &actor).await?,
    ))
}

async fn get_source_queue(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
) -> ApiResult<SourceQueueResponse> {
    let user_id = user_actor_from_headers(&state, &headers)
        .await?
        .user_id()
        .expect("user actor should have user id");
    get_host_session(&state, session_id, user_id).await?;
    Ok(Json(build_source_queue_response(&state, session_id).await?))
}

async fn set_source_queue_item_disabled(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((session_id, item_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<SourceQueueDisabledBody>,
) -> ApiResult<SourceQueueResponse> {
    let user_id = user_actor_from_headers(&state, &headers)
        .await?
        .user_id()
        .expect("user actor should have user id");
    get_host_session(&state, session_id, user_id).await?;

    db::party::set_source_queue_item_disabled(&state.pool, session_id, item_id, body.disabled)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    db::party::refill_queue_from_source(&state.pool, session_id)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(build_source_queue_response(&state, session_id).await?))
}

async fn reorder_queue(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    Json(body): Json<ReorderQueueBody>,
) -> ApiResult<QueueResponse> {
    let actor = actor_from_headers(&state, &headers).await?;
    let session = get_queue_editor_session(&state, session_id, &actor).await?;

    if session.mode == db::party::PartyMode::VotedQueue.as_str() {
        let pin_position = i32::try_from(body.to_position).unwrap_or(i32::MAX);
        db::party::set_queue_item_pin(&state.pool, session_id, body.item_id, pin_position)
            .await
            .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
        db::party::sort_voted_queue(&state.pool, session_id)
            .await
            .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    } else {
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
    }

    Ok(Json(
        build_queue_response(&state, session_id, &actor).await?,
    ))
}

async fn remove_queue_track(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    Json(body): Json<RemoveQueueTrackBody>,
) -> ApiResult<QueueResponse> {
    let actor = actor_from_headers(&state, &headers).await?;
    let session = get_queue_editor_session(&state, session_id, &actor).await?;

    db::party::remove_queue_item(&state.pool, session_id, body.item_id)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    db::party::refill_queue_from_source(&state.pool, session_id)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;

    if session.mode == db::party::PartyMode::VotedQueue.as_str() {
        db::party::sort_voted_queue(&state.pool, session_id)
            .await
            .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    }

    Ok(Json(
        build_queue_response(&state, session_id, &actor).await?,
    ))
}

async fn vote_queue_item(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((session_id, item_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<VoteQueueItemBody>,
) -> ApiResult<QueueResponse> {
    let actor = actor_from_headers(&state, &headers).await?;
    let session = get_existing_session(&state, session_id).await?;
    if !actor.can_access_session(session.id) {
        return Err(err(StatusCode::FORBIDDEN, "not your party session"));
    }

    if body.vote {
        db::party::vote_queue_item(
            &state.pool,
            session_id,
            item_id,
            actor.user_id(),
            actor.guest_id(),
        )
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    } else {
        db::party::unvote_queue_item(
            &state.pool,
            session_id,
            item_id,
            actor.user_id(),
            actor.guest_id(),
        )
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    }

    db::party::sort_voted_queue(&state.pool, session_id)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(
        build_queue_response(&state, session_id, &actor).await?,
    ))
}

async fn unvote_queue_item(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((session_id, item_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<QueueResponse> {
    let actor = actor_from_headers(&state, &headers).await?;
    let session = get_existing_session(&state, session_id).await?;
    if !actor.can_access_session(session.id) {
        return Err(err(StatusCode::FORBIDDEN, "not your party session"));
    }

    db::party::unvote_queue_item(
        &state.pool,
        session_id,
        item_id,
        actor.user_id(),
        actor.guest_id(),
    )
    .await
    .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    db::party::sort_voted_queue(&state.pool, session_id)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(
        build_queue_response(&state, session_id, &actor).await?,
    ))
}

async fn unpin_queue_item(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((session_id, item_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<QueueResponse> {
    let user_id = user_actor_from_headers(&state, &headers)
        .await?
        .user_id()
        .expect("user actor should have user id");
    get_host_session(&state, session_id, user_id).await?;

    db::party::clear_queue_item_pin(&state.pool, session_id, item_id)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    db::party::sort_voted_queue(&state.pool, session_id)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let actor = PartyActor::User {
        id: user_id,
        display_name: String::new(),
    };
    Ok(Json(
        build_queue_response(&state, session_id, &actor).await?,
    ))
}

async fn get_export_preview(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    Query(query): Query<ExportQuery>,
) -> ApiResult<ExportPreviewResponse> {
    let actor = actor_from_headers(&state, &headers).await?;
    let session = get_existing_session(&state, session_id).await?;
    if !actor.can_access_session(session.id) {
        return Err(err(StatusCode::FORBIDDEN, "not your party session"));
    }
    let mode = normalize_export_mode(query.mode.as_deref());
    let items = build_export_items(&state, session_id, mode).await?;

    Ok(Json(ExportPreviewResponse {
        mode: mode.to_string(),
        items,
    }))
}

async fn export_playlist(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    Json(body): Json<ExportPlaylistBody>,
) -> ApiResult<ExportPlaylistResponse> {
    let user_id = user_actor_from_headers(&state, &headers)
        .await?
        .user_id()
        .expect("user actor should have user id");
    let session = get_existing_session(&state, session_id).await?;
    if session.host_user_id != user_id {
        return Err(err(
            StatusCode::FORBIDDEN,
            "Spotify export requires a signed-in host",
        ));
    }
    let mode = normalize_export_mode(body.mode.as_deref());
    let items = build_export_items(&state, session_id, mode).await?;
    if items.is_empty() {
        return Err(err(StatusCode::UNPROCESSABLE_ENTITY, "nothing to export"));
    }

    let access_token = get_access_token(&state, user_id).await?;
    let playlist_name = body
        .name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or("Ensemble Party Export");
    let playlist = spotify::playlist::create_playlist(
        &access_token,
        playlist_name,
        "Exported from an Ensemble party session.",
    )
    .await
    .map_err(|e| err(StatusCode::BAD_GATEWAY, e))?;
    let uris = items
        .iter()
        .map(|item| item.uri.clone())
        .collect::<Vec<_>>();
    spotify::playlist::add_items_to_playlist(&access_token, &playlist.id, &uris)
        .await
        .map_err(|e| err(StatusCode::BAD_GATEWAY, e))?;

    Ok(Json(ExportPlaylistResponse {
        playlist_id: playlist.id,
        url: playlist.url,
        track_count: uris.len(),
    }))
}

async fn export_csv(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    Query(query): Query<ExportQuery>,
) -> Result<impl IntoResponse, (StatusCode, Json<Value>)> {
    let actor = actor_from_headers(&state, &headers).await?;
    let session = get_existing_session(&state, session_id).await?;
    if !actor.can_access_session(session.id) {
        return Err(err(StatusCode::FORBIDDEN, "not your party session"));
    }
    let mode = normalize_export_mode(query.mode.as_deref());
    let items = build_export_items(&state, session_id, mode).await?;
    let csv = export_items_csv(&items);
    let filename = format!("ensemble-party-{mode}.csv");

    Ok((
        [
            (header::CONTENT_TYPE, "text/csv; charset=utf-8".to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{filename}\""),
            ),
        ],
        csv,
    ))
}

async fn get_library_tracks(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<LibraryTracksQuery>,
) -> ApiResult<TrackSearchResponse> {
    const DEFAULT_LIMIT: usize = 1_500;
    const MAX_LIMIT: usize = 3_000;

    let user_id = user_actor_from_headers(&state, &headers)
        .await?
        .user_id()
        .expect("user actor should have user id");
    let access_token = get_access_token(&state, user_id).await?;
    let limit = query.limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT);
    let playlists = user_playlists(&access_token)
        .await
        .map_err(|e| err(StatusCode::BAD_GATEWAY, e))?;
    let results = if limit == 0 {
        Vec::new()
    } else {
        user_playlist_tracks(&access_token, &playlists, limit).await
    };

    Ok(Json(TrackSearchResponse { results, playlists }))
}

async fn end_session(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
) -> ApiResult<Value> {
    let user_id = user_actor_from_headers(&state, &headers)
        .await?
        .user_id()
        .expect("user actor should have user id");
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
    let actor = actor_from_headers(&state, &headers).await?;
    let user_id = match actor {
        PartyActor::User { id, .. } => id,
        PartyActor::Guest { session_id, .. } => {
            get_existing_session(&state, session_id).await?.host_user_id
        }
    };
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
    actor: &PartyActor,
) -> Result<QueueResponse, (StatusCode, Json<Value>)> {
    let items = db::party::queue_items(&state.pool, session_id)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let raw_votes = db::party::votes_for_queue_items(&state.pool, session_id)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let mut vote_map: HashMap<Uuid, Vec<VoterResponse>> = HashMap::new();
    for vote in raw_votes {
        vote_map
            .entry(vote.queue_item_id)
            .or_default()
            .push(VoterResponse {
                user_id: vote
                    .user_id
                    .or(vote.guest_id)
                    .map(|id| id.to_string())
                    .unwrap_or_default(),
                display_name: vote.display_name,
            });
    }

    let items = items
        .into_iter()
        .enumerate()
        .map(|(position, item)| {
            let voters = vote_map.get(&item.id).cloned().unwrap_or_default();
            let vote_count = voters.len() as i64;
            let actor_id = actor.voter_id();
            let user_voted = voters.iter().any(|v| v.user_id == actor_id);
            QueueItemResponse {
                id: item.id,
                uri: item.track.uri.clone(),
                name: item.track.name.clone(),
                artist: item.track.artist.clone(),
                album_art_url: item.track.album_art_url.clone(),
                duration_ms: item.track.duration_ms,
                position,
                pin_position: item.pin_position,
                vote_count,
                user_voted,
                voters,
                added_by_user_id: item.added_by_user_id,
                added_by_display_name: item.added_by_display_name,
            }
        })
        .collect();

    Ok(QueueResponse { items })
}

async fn build_source_queue_response(
    state: &AppState,
    session_id: Uuid,
) -> Result<SourceQueueResponse, (StatusCode, Json<Value>)> {
    let items = db::party::source_queue_items(&state.pool, session_id)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?
        .into_iter()
        .enumerate()
        .map(|(position, item)| SourceQueueItemResponse {
            id: item.id,
            uri: item.track.uri.clone(),
            name: item.track.name.clone(),
            artist: item.track.artist.clone(),
            album_art_url: item.track.album_art_url.clone(),
            duration_ms: item.track.duration_ms,
            position,
            deferred: item.position < 0,
            disabled: item.disabled,
            added_by_user_id: item.added_by_user_id,
            added_by_display_name: item.added_by_display_name,
        })
        .collect();

    Ok(SourceQueueResponse { items })
}

async fn build_export_items(
    state: &AppState,
    session_id: Uuid,
    mode: &str,
) -> Result<Vec<ExportItemResponse>, (StatusCode, Json<Value>)> {
    let mut rows = Vec::new();

    match mode {
        "played" => {
            rows.extend(played_export_items(state, session_id).await?);
        }
        "played_plus_queue" => {
            rows.extend(played_export_items(state, session_id).await?);
            rows.extend(queue_export_items(state, session_id).await?);
        }
        "played_plus_source" => {
            rows.extend(played_export_items(state, session_id).await?);
            rows.extend(source_export_items(state, session_id).await?);
        }
        "source_pool" => {
            rows.extend(source_export_items(state, session_id).await?);
        }
        _ => unreachable!("export mode should be normalized"),
    }

    Ok(distinct_export_items(rows))
}

async fn played_export_items(
    state: &AppState,
    session_id: Uuid,
) -> Result<Vec<ExportItemResponse>, (StatusCode, Json<Value>)> {
    Ok(db::party::played_tracks(&state.pool, session_id)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?
        .into_iter()
        .map(|item| ExportItemResponse {
            id: item.id.to_string(),
            source: "played".to_string(),
            session_id: item.session_id,
            uri: item.track.uri.clone(),
            name: item.track.name.clone(),
            artist: item.track.artist.clone(),
            album_art_url: item.track.album_art_url.clone(),
            duration_ms: item.track.duration_ms,
            position: usize::try_from(item.play_order).unwrap_or(0),
            play_order: Some(item.play_order),
            source_position: None,
            added_by_user_id: item.added_by_user_id,
            added_by_display_name: item.added_by_display_name,
            created_at: Some(item.created_at),
        })
        .collect())
}

async fn queue_export_items(
    state: &AppState,
    session_id: Uuid,
) -> Result<Vec<ExportItemResponse>, (StatusCode, Json<Value>)> {
    Ok(db::party::queue_items(&state.pool, session_id)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?
        .into_iter()
        .enumerate()
        .map(|(position, item)| ExportItemResponse {
            id: item.id.to_string(),
            source: "queue".to_string(),
            session_id: item.session_id,
            uri: item.track.uri.clone(),
            name: item.track.name.clone(),
            artist: item.track.artist.clone(),
            album_art_url: item.track.album_art_url.clone(),
            duration_ms: item.track.duration_ms,
            position,
            play_order: None,
            source_position: Some(item.position),
            added_by_user_id: item.added_by_user_id,
            added_by_display_name: item.added_by_display_name,
            created_at: Some(item.created_at),
        })
        .collect())
}

async fn source_export_items(
    state: &AppState,
    session_id: Uuid,
) -> Result<Vec<ExportItemResponse>, (StatusCode, Json<Value>)> {
    Ok(db::party::source_queue_items(&state.pool, session_id)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?
        .into_iter()
        .filter(|item| !item.disabled)
        .enumerate()
        .map(|(position, item)| ExportItemResponse {
            id: item.id.to_string(),
            source: "source".to_string(),
            session_id: item.session_id,
            uri: item.track.uri.clone(),
            name: item.track.name.clone(),
            artist: item.track.artist.clone(),
            album_art_url: item.track.album_art_url.clone(),
            duration_ms: item.track.duration_ms,
            position,
            play_order: None,
            source_position: Some(item.position),
            added_by_user_id: item.added_by_user_id,
            added_by_display_name: item.added_by_display_name,
            created_at: Some(item.created_at),
        })
        .collect())
}

fn distinct_export_items(items: Vec<ExportItemResponse>) -> Vec<ExportItemResponse> {
    let mut seen = HashSet::new();
    items
        .into_iter()
        .filter(|item| seen.insert(item.uri.clone()))
        .enumerate()
        .map(|(position, item)| ExportItemResponse { position, ..item })
        .collect()
}

fn normalize_export_mode(mode: Option<&str>) -> &'static str {
    match mode {
        Some("played_plus_queue") => "played_plus_queue",
        Some("played_plus_source") => "played_plus_source",
        Some("source_pool") => "source_pool",
        _ => "played",
    }
}

fn export_items_csv(items: &[ExportItemResponse]) -> String {
    let mut csv = String::from(
        "position,source,session_id,source_id,uri,name,artist,album_art_url,duration_ms,play_order,source_position,added_by_user_id,added_by_display_name,created_at\n",
    );

    for item in items {
        let fields = [
            item.position.to_string(),
            item.source.clone(),
            item.session_id.to_string(),
            item.id.clone(),
            item.uri.clone(),
            item.name.clone().unwrap_or_default(),
            item.artist.clone().unwrap_or_default(),
            item.album_art_url.clone().unwrap_or_default(),
            item.duration_ms.map(|v| v.to_string()).unwrap_or_default(),
            item.play_order.map(|v| v.to_string()).unwrap_or_default(),
            item.source_position
                .map(|v| v.to_string())
                .unwrap_or_default(),
            item.added_by_user_id
                .map(|v| v.to_string())
                .unwrap_or_default(),
            item.added_by_display_name.clone().unwrap_or_default(),
            item.created_at.map(|v| v.to_rfc3339()).unwrap_or_default(),
        ];
        csv.push_str(
            &fields
                .into_iter()
                .map(csv_field)
                .collect::<Vec<_>>()
                .join(","),
        );
        csv.push('\n');
    }

    csv
}

fn csv_field(value: String) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') || value.contains('\r') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value
    }
}

async fn seed_source_playlist(
    state: &AppState,
    user_id: Uuid,
    session_id: Uuid,
    playlist_id: &str,
) -> Result<(), (StatusCode, Json<Value>)> {
    let access_token = get_access_token(state, user_id).await?;
    let mut tracks = spotify::playlist::get_tracks(&access_token, playlist_id)
        .await
        .map_err(|e| err(StatusCode::BAD_GATEWAY, e))?
        .into_iter()
        .map(|track| db::party::PartyTrack {
            uri: track.uri,
            name: Some(track.name),
            artist: Some(track.artist),
            album_art_url: track.album_art_url,
            duration_ms: Some(track.duration_ms),
        })
        .collect::<Vec<_>>();

    if tracks.is_empty() {
        return Err(err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "playlist has no playable Spotify tracks",
        ));
    }

    weave::session::shuffle(&mut tracks);
    db::party::append_source_queue_items(&state.pool, session_id, &tracks, Some(user_id), None)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))
}

async fn search_party_cached_tracks(
    state: &AppState,
    session_id: Uuid,
    term: &str,
    actor: &PartyActor,
) -> Result<Vec<TrackSearchResultResponse>, (StatusCode, Json<Value>)> {
    let needle = term.to_lowercase();
    let mut seen = HashSet::new();
    let mut results = Vec::new();

    let source_items = db::party::source_queue_items(&state.pool, session_id)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let queue_items = db::party::queue_items(&state.pool, session_id)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let source_count = source_items.len();
    let queue_count = queue_items.len();

    for track in source_items
        .into_iter()
        .map(|item| item.track.0)
        .chain(queue_items.into_iter().map(|item| item.track.0))
    {
        let name = track.name.as_deref().unwrap_or("");
        let artist = track.artist.as_deref().unwrap_or("");
        if !name.to_lowercase().contains(&needle) && !artist.to_lowercase().contains(&needle) {
            continue;
        }
        if !seen.insert(track.uri.clone()) {
            continue;
        }
        results.push(TrackSearchResultResponse {
            uri: track.uri,
            name: track.name,
            artist: track.artist,
            album_art_url: track.album_art_url,
            duration_ms: track.duration_ms,
            playlist_index: None,
            playlist_id: None,
            playlist_name: None,
        });
        if results.len() >= 20 {
            break;
        }
    }

    tracing::info!(
        session_id = %session_id,
        actor_kind = actor.kind(),
        actor_id = %actor.id(),
        actor_guest_session_id = ?actor.guest_session_id(),
        term = %term,
        source_count,
        queue_count,
        match_count = results.len(),
        "party search: cached local results"
    );

    Ok(results)
}

async fn user_playlists(access_token: &str) -> anyhow::Result<Vec<PlaylistSearchResultResponse>> {
    Ok(spotify::playlist::get_user_playlists(access_token)
        .await?
        .into_iter()
        .map(|playlist| PlaylistSearchResultResponse {
            id: playlist.id,
            name: playlist.name,
            track_count: playlist.track_count,
            image_url: playlist.image_url,
        })
        .collect())
}

async fn user_playlist_tracks(
    access_token: &str,
    playlists: &[PlaylistSearchResultResponse],
    limit: usize,
) -> Vec<TrackSearchResultResponse> {
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

    results
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
    actor: &PartyActor,
) -> Result<db::party::PartySession, (StatusCode, Json<Value>)> {
    let session = get_existing_session(state, session_id).await?;
    if !actor.can_access_session(session.id) {
        return Err(err(StatusCode::FORBIDDEN, "not your party session"));
    }
    if !session.is_active {
        return Err(err(StatusCode::GONE, "party session has ended"));
    }
    if actor.user_id() == Some(session.host_user_id)
        || session.mode == db::party::PartyMode::SharedQueue.as_str()
    {
        return Ok(session);
    }

    Err(err(StatusCode::FORBIDDEN, "host controls only"))
}

async fn get_playlist_adder_session(
    state: &AppState,
    session_id: Uuid,
    user_id: Uuid,
) -> Result<db::party::PartySession, (StatusCode, Json<Value>)> {
    let session = get_existing_session(state, session_id).await?;
    if session.host_user_id == user_id || session.allow_guest_playlist_adds {
        return Ok(session);
    }

    Err(err(StatusCode::FORBIDDEN, "host controls only"))
}

fn session_response(
    session: db::party::PartySession,
    actor: &PartyActor,
    session_token: Option<String>,
) -> SessionResponse {
    let is_host = actor.user_id() == Some(session.host_user_id);
    SessionResponse {
        id: session.id,
        host_user_id: session.host_user_id,
        room_code: session.room_code,
        mode: session.mode,
        allow_guest_playlist_adds: session.allow_guest_playlist_adds,
        source_min_queue_size: session.source_min_queue_size,
        add_added_tracks_to_source: session.add_added_tracks_to_source,
        show_queue_attribution: session.show_queue_attribution,
        current_track_uri: session.current_track_uri,
        is_host,
        is_guest: actor.is_guest(),
        display_name: Some(actor.display_name().to_string()),
        session_token,
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
