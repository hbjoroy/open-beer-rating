use axum::extract::State;
use axum::Json;
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::auth::jwt::AuthUser;
use crate::db;
use crate::errors::ApiError;
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct BadgeResponse {
    pub name: String,
    pub description: String,
    pub icon_url: Option<String>,
    pub earned_at: chrono::DateTime<chrono::Utc>,
}

/// GET /api/users/me/badges — get own badges (authenticated)
pub async fn get_my_badges(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<Vec<BadgeResponse>>, ApiError> {
    let badges = db::badges::get_user_badges(&state.pool, auth.user_id).await?;

    Ok(Json(
        badges
            .into_iter()
            .map(|b| BadgeResponse {
                name: b.badge_name,
                description: b.badge_description,
                icon_url: b.badge_icon_url,
                earned_at: b.earned_at,
            })
            .collect(),
    ))
}

/// Evaluate and award badges after a tasting.
/// Called internally after each tasting submission.
pub async fn evaluate_badges(pool: &PgPool, user_id: Uuid) -> Result<(), ApiError> {
    let all_badges = db::badges::get_all_badges(pool).await?;

    for badge in &all_badges {
        // Skip if already earned
        if db::badges::has_badge(pool, user_id, badge.id).await? {
            continue;
        }

        let qualifies = match badge.criteria_type.as_str() {
            "total_ratings" => {
                let count = db::tastings::count_user_tastings(pool, user_id).await?;
                count >= badge.criteria_value as i64
            }
            "unique_styles" => {
                let count = db::tastings::count_user_unique_styles(pool, user_id).await?;
                count >= badge.criteria_value as i64
            }
            "same_brewery" => {
                let count = db::tastings::max_tastings_same_brewery(pool, user_id).await?;
                count >= badge.criteria_value as i64
            }
            _ => false,
        };

        if qualifies {
            db::badges::award_badge(pool, user_id, badge.id).await?;
            tracing::info!(
                "Awarded badge '{}' to user {}",
                badge.name,
                user_id
            );
        }
    }

    Ok(())
}
