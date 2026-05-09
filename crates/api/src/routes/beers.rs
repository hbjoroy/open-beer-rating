use axum::extract::{Path, Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::jwt::AuthUser;
use crate::db;
use crate::errors::ApiError;
use crate::state::AppState;
use open_tappd_domain::validation;

#[derive(Debug, Deserialize)]
pub struct CreateBeerRequest {
    pub brewery_id: Uuid,
    pub name: String,
    pub style: Option<String>,
    pub abv: Option<f64>,
    pub description: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct BeerResponse {
    pub id: Uuid,
    pub brewery_id: Uuid,
    pub name: String,
    pub style: Option<String>,
    pub abv: Option<f64>,
    pub description: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct BeerDetailResponse {
    pub id: Uuid,
    pub brewery_id: Uuid,
    pub name: String,
    pub style: Option<String>,
    pub abv: Option<f64>,
    pub description: Option<String>,
    pub average_score: Option<f64>,
    pub rating_count: i64,
}

#[derive(Debug, Deserialize)]
pub struct BeerListParams {
    pub brewery_id: Option<Uuid>,
    pub style: Option<String>,
    pub search: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

pub async fn create_beer(
    State(state): State<AppState>,
    _auth: AuthUser,
    Json(req): Json<CreateBeerRequest>,
) -> Result<(axum::http::StatusCode, Json<BeerResponse>), ApiError> {
    if req.name.is_empty() || req.name.len() > 200 {
        return Err(ApiError::Validation("Beer name must be 1-200 characters".into()));
    }
    if let Some(abv) = req.abv {
        validation::validate_abv(abv)?;
    }

    // Verify brewery exists
    db::breweries::get_brewery(&state.pool, req.brewery_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("Brewery not found".into()))?;

    let beer = db::beers::create_beer(
        &state.pool,
        req.brewery_id,
        &req.name,
        req.style.as_deref(),
        req.abv,
        req.description.as_deref(),
    )
    .await?;

    Ok((
        axum::http::StatusCode::CREATED,
        Json(BeerResponse {
            id: beer.id,
            brewery_id: beer.brewery_id,
            name: beer.name,
            style: beer.style,
            abv: beer.abv,
            description: beer.description,
        }),
    ))
}

pub async fn list_beers(
    State(state): State<AppState>,
    Query(params): Query<BeerListParams>,
) -> Result<Json<Vec<BeerResponse>>, ApiError> {
    let limit = params.limit.unwrap_or(20).min(100);
    let offset = params.offset.unwrap_or(0);

    let beers = db::beers::list_beers(
        &state.pool,
        params.brewery_id,
        params.style.as_deref(),
        params.search.as_deref(),
        limit,
        offset,
    )
    .await?;

    Ok(Json(
        beers
            .into_iter()
            .map(|b| BeerResponse {
                id: b.id,
                brewery_id: b.brewery_id,
                name: b.name,
                style: b.style,
                abv: b.abv,
                description: b.description,
            })
            .collect(),
    ))
}

pub async fn get_beer(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<BeerDetailResponse>, ApiError> {
    let beer = db::beers::get_beer(&state.pool, id)
        .await?
        .ok_or_else(|| ApiError::NotFound("Beer not found".into()))?;

    let aggregate = db::ratings::get_beer_aggregate(&state.pool, id).await?;

    Ok(Json(BeerDetailResponse {
        id: beer.id,
        brewery_id: beer.brewery_id,
        name: beer.name,
        style: beer.style,
        abv: beer.abv,
        description: beer.description,
        average_score: aggregate.average_score,
        rating_count: aggregate.rating_count.unwrap_or(0),
    }))
}
