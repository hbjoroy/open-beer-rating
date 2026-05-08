use axum::extract::{Path, Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::jwt::AuthUser;
use crate::db;
use crate::errors::ApiError;
use crate::state::AppState;
use open_tappd_domain::{crypto, validation};

#[derive(Debug, Deserialize)]
pub struct RateBeerRequest {
    pub score: i32,
    pub notes: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RatingResponse {
    pub id: Uuid,
    pub beer_id: Uuid,
    pub score: i32,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct BeerAggregateResponse {
    pub beer_id: Uuid,
    pub average_score: Option<f64>,
    pub rating_count: i64,
}

#[derive(Debug, Serialize)]
pub struct UserRatingResponse {
    pub id: Uuid,
    pub beer_id: Uuid,
    pub beer_name: String,
    pub brewery_name: String,
    pub score: i32,
    pub notes: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct PaginationParams {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// POST /api/beers/:id/ratings — rate a beer (authenticated)
pub async fn rate_beer(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(beer_id): Path<Uuid>,
    Json(req): Json<RateBeerRequest>,
) -> Result<(axum::http::StatusCode, Json<RatingResponse>), ApiError> {
    validation::validate_score(req.score)?;

    // Verify beer exists
    db::beers::get_beer(&state.pool, beer_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("Beer not found".into()))?;

    // Encrypt notes if provided
    let notes_encrypted = if let Some(ref notes) = req.notes {
        Some(crypto::encrypt_field(notes, &state.encryption_key)?)
    } else {
        None
    };

    let rating_id = db::ratings::upsert_rating(
        &state.pool,
        auth.user_id,
        beer_id,
        req.score,
        notes_encrypted.as_deref(),
    )
    .await?;

    // Evaluate badges after rating
    crate::routes::badges::evaluate_badges(&state.pool, auth.user_id).await?;

    Ok((
        axum::http::StatusCode::CREATED,
        Json(RatingResponse {
            id: rating_id,
            beer_id,
            score: req.score,
            message: "Rating submitted".into(),
        }),
    ))
}

/// GET /api/beers/:id/ratings — aggregate only (privacy: no individual attribution)
pub async fn get_beer_ratings(
    State(state): State<AppState>,
    Path(beer_id): Path<Uuid>,
) -> Result<Json<BeerAggregateResponse>, ApiError> {
    // Verify beer exists
    db::beers::get_beer(&state.pool, beer_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("Beer not found".into()))?;

    let aggregate = db::ratings::get_beer_aggregate(&state.pool, beer_id).await?;

    Ok(Json(BeerAggregateResponse {
        beer_id,
        average_score: aggregate.average_score,
        rating_count: aggregate.rating_count.unwrap_or(0),
    }))
}

/// GET /api/users/me/ratings — own ratings only (authenticated)
pub async fn get_my_ratings(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(params): Query<PaginationParams>,
) -> Result<Json<Vec<UserRatingResponse>>, ApiError> {
    let limit = params.limit.unwrap_or(20).min(100);
    let offset = params.offset.unwrap_or(0);

    let ratings = db::ratings::get_user_ratings(&state.pool, auth.user_id, limit, offset).await?;

    let mut results = Vec::with_capacity(ratings.len());
    for r in ratings {
        let notes = if let Some(ref enc) = r.notes_encrypted {
            Some(
                crypto::decrypt_field(enc, &state.encryption_key)
                    .unwrap_or_else(|_| "[decryption error]".into()),
            )
        } else {
            None
        };

        results.push(UserRatingResponse {
            id: r.id,
            beer_id: r.beer_id,
            beer_name: r.beer_name,
            brewery_name: r.brewery_name,
            score: r.score,
            notes,
            created_at: r.created_at,
        });
    }

    Ok(Json(results))
}
