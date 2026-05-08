use axum::extract::{Path, Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::jwt::AuthUser;
use crate::db;
use crate::errors::ApiError;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct CreateBreweryRequest {
    pub name: String,
    pub country: Option<String>,
    pub city: Option<String>,
    pub website: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct BreweryResponse {
    pub id: Uuid,
    pub name: String,
    pub country: Option<String>,
    pub city: Option<String>,
    pub website: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PaginationParams {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

pub async fn create_brewery(
    State(state): State<AppState>,
    _auth: AuthUser,
    Json(req): Json<CreateBreweryRequest>,
) -> Result<(axum::http::StatusCode, Json<BreweryResponse>), ApiError> {
    if req.name.is_empty() || req.name.len() > 200 {
        return Err(ApiError::Validation("Brewery name must be 1-200 characters".into()));
    }

    let brewery = db::breweries::create_brewery(
        &state.pool,
        &req.name,
        req.country.as_deref(),
        req.city.as_deref(),
        req.website.as_deref(),
    )
    .await?;

    Ok((
        axum::http::StatusCode::CREATED,
        Json(BreweryResponse {
            id: brewery.id,
            name: brewery.name,
            country: brewery.country,
            city: brewery.city,
            website: brewery.website,
        }),
    ))
}

pub async fn list_breweries(
    State(state): State<AppState>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<Vec<BreweryResponse>>, ApiError> {
    let limit = params.limit.unwrap_or(20).min(100);
    let offset = params.offset.unwrap_or(0);

    let breweries = db::breweries::list_breweries(&state.pool, limit, offset).await?;

    Ok(Json(
        breweries
            .into_iter()
            .map(|b| BreweryResponse {
                id: b.id,
                name: b.name,
                country: b.country,
                city: b.city,
                website: b.website,
            })
            .collect(),
    ))
}

pub async fn get_brewery(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<BreweryResponse>, ApiError> {
    let brewery = db::breweries::get_brewery(&state.pool, id)
        .await?
        .ok_or_else(|| ApiError::NotFound("Brewery not found".into()))?;

    Ok(Json(BreweryResponse {
        id: brewery.id,
        name: brewery.name,
        country: brewery.country,
        city: brewery.city,
        website: brewery.website,
    }))
}
