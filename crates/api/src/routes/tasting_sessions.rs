use axum::extract::{Path, Query, State};
use axum::Json;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::jwt::AuthUser;
use crate::db;
use crate::errors::ApiError;
use crate::state::AppState;
use open_tappd_domain::models::tasting_session::SessionVisibility;
use open_tappd_domain::validation;

#[derive(Debug, Deserialize)]
pub struct CreateSessionRequest {
    pub name: String,
    pub description: Option<String>,
    pub location_id: Option<Uuid>,
    pub visibility: Option<SessionVisibility>,
    pub auto_end_minutes: Option<i32>,
    pub planned_start: Option<DateTime<Utc>>,
    pub planned_end: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct JoinByCodeRequest {
    pub code: String,
}

#[derive(Debug, Serialize)]
pub struct SessionResponse {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub location_id: Option<Uuid>,
    pub created_by: Uuid,
    pub join_code: String,
    pub visibility: SessionVisibility,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub planned_start: Option<DateTime<Utc>>,
    pub planned_end: Option<DateTime<Utc>>,
    pub auto_end_minutes: i32,
    pub participants: Option<Vec<ParticipantResponse>>,
}

#[derive(Debug, Serialize)]
pub struct ParticipantResponse {
    pub user_id: Uuid,
    pub username: String,
    pub joined_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct ListSessionsParams {
    pub active_only: Option<bool>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// POST /api/tasting-sessions — create session (auto-joins creator)
pub async fn create_session(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<CreateSessionRequest>,
) -> Result<(axum::http::StatusCode, Json<SessionResponse>), ApiError> {
    validation::validate_session_name(&req.name)?;

    let auto_end = req.auto_end_minutes.unwrap_or(180);
    validation::validate_auto_end_minutes(auto_end)?;

    if let Some(loc_id) = req.location_id {
        db::locations::get_location(&state.pool, loc_id)
            .await?
            .ok_or_else(|| ApiError::NotFound("Location not found".into()))?;
    }

    let visibility = req.visibility.unwrap_or(SessionVisibility::Participants);

    let row = db::tasting_sessions::create_session(
        &state.pool,
        &req.name,
        req.description.as_deref(),
        req.location_id,
        auth.user_id,
        visibility,
        auto_end,
        req.planned_start,
        req.planned_end,
    )
    .await?;

    // Auto-join the creator
    db::tasting_sessions::join_session(&state.pool, row.id, auth.user_id).await?;

    Ok((
        axum::http::StatusCode::CREATED,
        Json(session_to_response(row, None)),
    ))
}

/// GET /api/tasting-sessions — list sessions visible to user
pub async fn list_sessions(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(params): Query<ListSessionsParams>,
) -> Result<Json<Vec<SessionResponse>>, ApiError> {
    let active_only = params.active_only.unwrap_or(false);
    let limit = params.limit.unwrap_or(20).min(100);
    let offset = params.offset.unwrap_or(0);

    let rows =
        db::tasting_sessions::list_sessions(&state.pool, auth.user_id, active_only, limit, offset)
            .await?;

    let results: Vec<SessionResponse> = rows
        .into_iter()
        .map(|r| session_to_response(r, None))
        .collect();

    Ok(Json(results))
}

/// GET /api/tasting-sessions/:id — session details with participants
pub async fn get_session(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<SessionResponse>, ApiError> {
    let row = db::tasting_sessions::get_session(&state.pool, id)
        .await?
        .ok_or_else(|| ApiError::NotFound("Session not found".into()))?;

    // Visibility check
    check_session_access(&state, &row, auth.user_id).await?;

    // Include participants if visibility allows
    let participants = if row.visibility != SessionVisibility::Private || row.created_by == auth.user_id
    {
        let parts = db::tasting_sessions::get_participants(&state.pool, id).await?;
        Some(
            parts
                .into_iter()
                .map(|p| ParticipantResponse {
                    user_id: p.user_id,
                    username: p.username,
                    joined_at: p.joined_at,
                })
                .collect(),
        )
    } else {
        None
    };

    Ok(Json(session_to_response(row, participants)))
}

/// POST /api/tasting-sessions/:id/join — join by ID
pub async fn join_session(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<axum::http::StatusCode, ApiError> {
    let session = db::tasting_sessions::get_session(&state.pool, id)
        .await?
        .ok_or_else(|| ApiError::NotFound("Session not found".into()))?;

    if session.ended_at.is_some() {
        return Err(ApiError::Validation("Session has already ended".into()));
    }

    // Non-public sessions require being the creator or already a participant (or using join code)
    if session.visibility != SessionVisibility::Public && session.created_by != auth.user_id {
        return Err(ApiError::NotFound("Session not found".into()));
    }

    db::tasting_sessions::join_session(&state.pool, id, auth.user_id).await?;
    Ok(axum::http::StatusCode::OK)
}

/// POST /api/tasting-sessions/join — join by code
pub async fn join_session_by_code(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<JoinByCodeRequest>,
) -> Result<Json<SessionResponse>, ApiError> {
    let code = req.code.trim().to_uppercase();
    let session = db::tasting_sessions::get_session_by_code(&state.pool, &code)
        .await?
        .ok_or_else(|| ApiError::NotFound("Invalid join code".into()))?;

    if session.ended_at.is_some() {
        return Err(ApiError::Validation("Session has already ended".into()));
    }

    db::tasting_sessions::join_session(&state.pool, session.id, auth.user_id).await?;

    Ok(Json(session_to_response(session, None)))
}

/// POST /api/tasting-sessions/:id/leave — leave session
pub async fn leave_session(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<axum::http::StatusCode, ApiError> {
    let left = db::tasting_sessions::leave_session(&state.pool, id, auth.user_id).await?;
    if left {
        Ok(axum::http::StatusCode::OK)
    } else {
        Err(ApiError::NotFound("Not a participant".into()))
    }
}

/// POST /api/tasting-sessions/:id/end — end session (creator only)
pub async fn end_session(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<axum::http::StatusCode, ApiError> {
    let ended = db::tasting_sessions::end_session(&state.pool, id, auth.user_id).await?;
    if ended {
        Ok(axum::http::StatusCode::OK)
    } else {
        Err(ApiError::NotFound(
            "Session not found, not yours, or already ended".into(),
        ))
    }
}

/// GET /api/tasting-sessions/:id/tastings — tastings in a session
pub async fn get_session_tastings(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Query(params): Query<super::tastings::PaginationParams>,
) -> Result<Json<Vec<super::tastings::TastingResponse>>, ApiError> {
    let session = db::tasting_sessions::get_session(&state.pool, id)
        .await?
        .ok_or_else(|| ApiError::NotFound("Session not found".into()))?;

    check_session_access(&state, &session, auth.user_id).await?;

    let limit = params.limit.unwrap_or(50).min(100);
    let offset = params.offset.unwrap_or(0);

    let rows = db::tastings::get_session_tastings(&state.pool, id, limit, offset).await?;

    let results: Vec<super::tastings::TastingResponse> = rows
        .into_iter()
        .map(|r| {
            let notes = if r.user_id == auth.user_id {
                super::tastings::decrypt_notes_pub(r.notes_encrypted.as_deref(), &state.encryption_key)
            } else {
                None // Don't expose other users' notes
            };
            super::tastings::TastingResponse {
                id: r.id,
                beer_id: r.beer_id,
                beer_name: Some(r.beer_name),
                brewery_name: Some(r.brewery_name),
                score: r.score,
                serving_style: r.serving_style,
                notes,
                location_id: r.location_id,
                location_name: r.location_name,
                session_id: r.session_id,
                session_name: r.session_name,
                tasted_at: r.tasted_at,
                created_at: r.created_at,
            }
        })
        .collect();

    Ok(Json(results))
}

async fn check_session_access(
    state: &AppState,
    session: &db::tasting_sessions::TastingSessionRow,
    user_id: Uuid,
) -> Result<(), ApiError> {
    match session.visibility {
        SessionVisibility::Public => Ok(()),
        SessionVisibility::Participants | SessionVisibility::Private => {
            if session.created_by == user_id {
                return Ok(());
            }
            if db::tasting_sessions::is_participant(&state.pool, session.id, user_id).await? {
                Ok(())
            } else {
                Err(ApiError::NotFound("Session not found".into()))
            }
        }
    }
}

fn session_to_response(
    row: db::tasting_sessions::TastingSessionRow,
    participants: Option<Vec<ParticipantResponse>>,
) -> SessionResponse {
    SessionResponse {
        id: row.id,
        name: row.name,
        description: row.description,
        location_id: row.location_id,
        created_by: row.created_by,
        join_code: row.join_code,
        visibility: row.visibility,
        started_at: row.started_at,
        ended_at: row.ended_at,
        planned_start: row.planned_start,
        planned_end: row.planned_end,
        auto_end_minutes: row.auto_end_minutes,
        participants,
    }
}
