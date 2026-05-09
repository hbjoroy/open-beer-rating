use axum::extract::{Path, Query, State};
use axum::Json;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::jwt::AuthUser;
use crate::db;
use crate::errors::ApiError;
use crate::state::AppState;
use open_tappd_domain::models::location::LocationType;
use open_tappd_domain::validation;

#[derive(Debug, Deserialize)]
pub struct CreateLocationRequest {
    pub name: String,
    pub location_type: LocationType,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateLocationRequest {
    pub name: String,
    pub location_type: LocationType,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct LocationResponse {
    pub id: Uuid,
    pub name: String,
    pub location_type: LocationType,
    pub metadata: serde_json::Value,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct ListLocationsParams {
    pub location_type: Option<LocationType>,
    pub search: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// POST /api/locations — create location (authenticated)
pub async fn create_location(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<CreateLocationRequest>,
) -> Result<(axum::http::StatusCode, Json<LocationResponse>), ApiError> {
    validation::validate_location_name(&req.name)?;

    let metadata = req.metadata.unwrap_or(serde_json::json!({}));
    validation::validate_location_metadata(req.location_type, &metadata)?;

    let row = db::locations::create_location(
        &state.pool,
        &req.name,
        req.location_type,
        &metadata,
        auth.user_id,
    )
    .await?;

    Ok((
        axum::http::StatusCode::CREATED,
        Json(location_to_response(row)),
    ))
}

/// GET /api/locations — list active locations
pub async fn list_locations(
    State(state): State<AppState>,
    Query(params): Query<ListLocationsParams>,
) -> Result<Json<Vec<LocationResponse>>, ApiError> {
    let limit = params.limit.unwrap_or(20).min(100);
    let offset = params.offset.unwrap_or(0);

    let rows = db::locations::list_locations(
        &state.pool,
        params.location_type,
        params.search.as_deref(),
        limit,
        offset,
    )
    .await?;

    Ok(Json(rows.into_iter().map(location_to_response).collect()))
}

/// GET /api/locations/:id — get location details
pub async fn get_location(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<LocationResponse>, ApiError> {
    let row = db::locations::get_location(&state.pool, id)
        .await?
        .ok_or_else(|| ApiError::NotFound("Location not found".into()))?;

    Ok(Json(location_to_response(row)))
}

/// PUT /api/locations/:id — update location (creator only)
pub async fn update_location(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateLocationRequest>,
) -> Result<Json<LocationResponse>, ApiError> {
    validation::validate_location_name(&req.name)?;

    let metadata = req.metadata.unwrap_or(serde_json::json!({}));
    validation::validate_location_metadata(req.location_type, &metadata)?;

    let row = db::locations::update_location(
        &state.pool,
        id,
        auth.user_id,
        &req.name,
        req.location_type,
        &metadata,
    )
    .await?
    .ok_or_else(|| ApiError::NotFound("Location not found or not yours".into()))?;

    Ok(Json(location_to_response(row)))
}

/// DELETE /api/locations/:id — soft-delete (creator only)
pub async fn delete_location(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<axum::http::StatusCode, ApiError> {
    let deleted = db::locations::soft_delete_location(&state.pool, id, auth.user_id).await?;
    if deleted {
        Ok(axum::http::StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::NotFound("Location not found or not yours".into()))
    }
}

fn location_to_response(row: db::locations::LocationRow) -> LocationResponse {
    LocationResponse {
        id: row.id,
        name: row.name,
        location_type: row.location_type,
        metadata: row.metadata,
        is_active: row.is_active,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}
