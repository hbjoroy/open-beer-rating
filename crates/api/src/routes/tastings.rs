use axum::extract::{Path, Query, State};
use axum::Json;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::jwt::AuthUser;
use crate::db;
use crate::errors::ApiError;
use crate::state::AppState;
use open_tappd_domain::{crypto, validation};

#[derive(Debug, Deserialize)]
pub struct CreateTastingRequest {
    pub beer_id: Uuid,
    pub score: i32,
    pub notes: Option<String>,
    pub location_id: Option<Uuid>,
    pub session_id: Option<Uuid>,
    pub tasted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateTastingRequest {
    pub score: i32,
    pub notes: Option<String>,
    pub location_id: Option<Uuid>,
}

#[derive(Debug, Serialize)]
pub struct TastingResponse {
    pub id: Uuid,
    pub beer_id: Uuid,
    pub beer_name: Option<String>,
    pub brewery_name: Option<String>,
    pub score: i32,
    pub notes: Option<String>,
    pub location_id: Option<Uuid>,
    pub location_name: Option<String>,
    pub session_id: Option<Uuid>,
    pub session_name: Option<String>,
    pub tasted_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct TastingCreatedResponse {
    pub id: Uuid,
    pub beer_id: Uuid,
    pub score: i32,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct BeerTastingAggregateResponse {
    pub beer_id: Uuid,
    pub average_score: Option<f64>,
    pub unique_tasters: i64,
    pub total_tastings: i64,
}

#[derive(Debug, Deserialize)]
pub struct PaginationParams {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// POST /api/tastings — create a tasting (authenticated)
pub async fn create_tasting(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<CreateTastingRequest>,
) -> Result<(axum::http::StatusCode, Json<TastingCreatedResponse>), ApiError> {
    validation::validate_score(req.score)?;

    // Verify beer exists
    db::beers::get_beer(&state.pool, req.beer_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("Beer not found".into()))?;

    // Verify location exists if provided
    if let Some(loc_id) = req.location_id {
        db::locations::get_location(&state.pool, loc_id)
            .await?
            .ok_or_else(|| ApiError::NotFound("Location not found".into()))?;
    }

    // Verify session exists and user is participant if provided
    if let Some(sess_id) = req.session_id {
        let session = db::tasting_sessions::get_session(&state.pool, sess_id)
            .await?
            .ok_or_else(|| ApiError::NotFound("Session not found".into()))?;

        if session.ended_at.is_some() {
            return Err(ApiError::Validation("Session has already ended".into()));
        }

        if !db::tasting_sessions::is_participant(&state.pool, sess_id, auth.user_id).await? {
            return Err(ApiError::Validation(
                "You must join the session before adding tastings".into(),
            ));
        }
    }

    let notes_encrypted = if let Some(ref notes) = req.notes {
        Some(crypto::encrypt_field(notes, &state.encryption_key)?)
    } else {
        None
    };

    let tasted_at = req.tasted_at.unwrap_or_else(Utc::now);

    let row = db::tastings::create_tasting(
        &state.pool,
        auth.user_id,
        req.beer_id,
        req.score,
        notes_encrypted.as_deref(),
        req.location_id,
        req.session_id,
        tasted_at,
    )
    .await?;

    // Evaluate badges after tasting
    crate::routes::badges::evaluate_badges(&state.pool, auth.user_id).await?;

    Ok((
        axum::http::StatusCode::CREATED,
        Json(TastingCreatedResponse {
            id: row.id,
            beer_id: row.beer_id,
            score: row.score,
            message: "Tasting recorded".into(),
        }),
    ))
}

/// GET /api/tastings — list own tastings (authenticated)
pub async fn list_my_tastings(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(params): Query<PaginationParams>,
) -> Result<Json<Vec<TastingResponse>>, ApiError> {
    let limit = params.limit.unwrap_or(20).min(100);
    let offset = params.offset.unwrap_or(0);

    let rows = db::tastings::get_user_tastings(&state.pool, auth.user_id, limit, offset).await?;

    let mut results = Vec::with_capacity(rows.len());
    for r in rows {
        let notes = decrypt_notes(r.notes_encrypted.as_deref(), &state.encryption_key);
        results.push(TastingResponse {
            id: r.id,
            beer_id: r.beer_id,
            beer_name: Some(r.beer_name),
            brewery_name: Some(r.brewery_name),
            score: r.score,
            notes,
            location_id: r.location_id,
            location_name: r.location_name,
            session_id: r.session_id,
            session_name: r.session_name,
            tasted_at: r.tasted_at,
            created_at: r.created_at,
        });
    }

    Ok(Json(results))
}

/// GET /api/tastings/:id — get specific tasting (own only)
pub async fn get_tasting(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<TastingResponse>, ApiError> {
    let row = db::tastings::get_tasting(&state.pool, id)
        .await?
        .ok_or_else(|| ApiError::NotFound("Tasting not found".into()))?;

    if row.user_id != auth.user_id {
        return Err(ApiError::NotFound("Tasting not found".into()));
    }

    let notes = decrypt_notes(row.notes_encrypted.as_deref(), &state.encryption_key);

    Ok(Json(TastingResponse {
        id: row.id,
        beer_id: row.beer_id,
        beer_name: None,
        brewery_name: None,
        score: row.score,
        notes,
        location_id: row.location_id,
        location_name: None,
        session_id: row.session_id,
        session_name: None,
        tasted_at: row.tasted_at,
        created_at: row.created_at,
    }))
}

/// PUT /api/tastings/:id — update own tasting
pub async fn update_tasting(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateTastingRequest>,
) -> Result<Json<TastingResponse>, ApiError> {
    validation::validate_score(req.score)?;

    if let Some(loc_id) = req.location_id {
        db::locations::get_location(&state.pool, loc_id)
            .await?
            .ok_or_else(|| ApiError::NotFound("Location not found".into()))?;
    }

    let notes_encrypted = if let Some(ref notes) = req.notes {
        Some(crypto::encrypt_field(notes, &state.encryption_key)?)
    } else {
        None
    };

    let row = db::tastings::update_tasting(
        &state.pool,
        id,
        auth.user_id,
        req.score,
        notes_encrypted.as_deref(),
        req.location_id,
    )
    .await?
    .ok_or_else(|| ApiError::NotFound("Tasting not found or not yours".into()))?;

    let notes = decrypt_notes(row.notes_encrypted.as_deref(), &state.encryption_key);

    Ok(Json(TastingResponse {
        id: row.id,
        beer_id: row.beer_id,
        beer_name: None,
        brewery_name: None,
        score: row.score,
        notes,
        location_id: row.location_id,
        location_name: None,
        session_id: row.session_id,
        session_name: None,
        tasted_at: row.tasted_at,
        created_at: row.created_at,
    }))
}

/// DELETE /api/tastings/:id — delete own tasting
pub async fn delete_tasting(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<axum::http::StatusCode, ApiError> {
    let deleted = db::tastings::delete_tasting(&state.pool, id, auth.user_id).await?;
    if deleted {
        Ok(axum::http::StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::NotFound("Tasting not found or not yours".into()))
    }
}

/// GET /api/beers/:id/tastings — public aggregate for a beer
pub async fn get_beer_tastings(
    State(state): State<AppState>,
    Path(beer_id): Path<Uuid>,
) -> Result<Json<BeerTastingAggregateResponse>, ApiError> {
    db::beers::get_beer(&state.pool, beer_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("Beer not found".into()))?;

    let agg = db::tastings::get_beer_aggregate(&state.pool, beer_id).await?;
    let total = db::tastings::count_beer_tastings(&state.pool, beer_id).await?;

    Ok(Json(BeerTastingAggregateResponse {
        beer_id,
        average_score: agg.average_score,
        unique_tasters: agg.tasting_count.unwrap_or(0),
        total_tastings: total,
    }))
}

fn decrypt_notes(encrypted: Option<&[u8]>, key: &[u8; 32]) -> Option<String> {
    encrypted.map(|enc| {
        crypto::decrypt_field(enc, key).unwrap_or_else(|_| "[decryption error]".into())
    })
}

/// Public version for cross-module access
pub fn decrypt_notes_pub(encrypted: Option<&[u8]>, key: &[u8; 32]) -> Option<String> {
    decrypt_notes(encrypted, key)
}
