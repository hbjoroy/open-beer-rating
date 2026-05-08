use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::auth::jwt::AuthUser;
use crate::db;
use crate::errors::ApiError;
use crate::state::AppState;
use open_tappd_domain::crypto;

#[derive(Debug, Serialize)]
pub struct PrivacySettingsResponse {
    pub profile_visibility: String,
    pub show_ratings: bool,
    pub show_badges: bool,
    pub show_stats: bool,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePrivacyRequest {
    pub profile_visibility: String,
    pub show_ratings: bool,
    pub show_badges: bool,
    pub show_stats: bool,
}

#[derive(Debug, Serialize)]
pub struct DataExportResponse {
    pub user: DataExportUser,
    pub ratings: Vec<DataExportRating>,
    pub badges: Vec<DataExportBadge>,
    pub privacy_settings: PrivacySettingsResponse,
}

#[derive(Debug, Serialize)]
pub struct DataExportUser {
    pub username: String,
    pub email: Option<String>,
    pub display_name: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DataExportRating {
    pub beer_name: String,
    pub brewery_name: String,
    pub score: i32,
    pub notes: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
pub struct DataExportBadge {
    pub name: String,
    pub description: String,
    pub earned_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct DeleteAccountRequest {
    pub password: String,
}

/// GET /api/users/me/privacy
pub async fn get_privacy_settings(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<PrivacySettingsResponse>, ApiError> {
    let settings = db::privacy::get_privacy_settings(&state.pool, auth.user_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("Privacy settings not found".into()))?;

    Ok(Json(PrivacySettingsResponse {
        profile_visibility: settings.profile_visibility,
        show_ratings: settings.show_ratings,
        show_badges: settings.show_badges,
        show_stats: settings.show_stats,
    }))
}

/// PUT /api/users/me/privacy
pub async fn update_privacy_settings(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<UpdatePrivacyRequest>,
) -> Result<Json<PrivacySettingsResponse>, ApiError> {
    // Validate visibility value
    match req.profile_visibility.as_str() {
        "public" | "private" | "friends" => {}
        _ => {
            return Err(ApiError::Validation(
                "profile_visibility must be 'public', 'private', or 'friends'".into(),
            ))
        }
    }

    let settings = db::privacy::update_privacy_settings(
        &state.pool,
        auth.user_id,
        &req.profile_visibility,
        req.show_ratings,
        req.show_badges,
        req.show_stats,
    )
    .await?;

    Ok(Json(PrivacySettingsResponse {
        profile_visibility: settings.profile_visibility,
        show_ratings: settings.show_ratings,
        show_badges: settings.show_badges,
        show_stats: settings.show_stats,
    }))
}

/// GET /api/users/me/data-export
pub async fn export_data(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<DataExportResponse>, ApiError> {
    let user = db::users::find_user_by_id(&state.pool, auth.user_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("User not found".into()))?;

    // Decrypt email if present
    let email = if let Some(ref enc) = user.email_encrypted {
        Some(
            crypto::decrypt_field(enc, &state.encryption_key)
                .unwrap_or_else(|_| "[decryption error]".into()),
        )
    } else {
        None
    };

    // Get all ratings (no pagination — full export)
    let ratings = db::ratings::get_user_ratings(&state.pool, auth.user_id, 10000, 0).await?;
    let export_ratings: Vec<DataExportRating> = ratings
        .into_iter()
        .map(|r| {
            let notes = r.notes_encrypted.as_ref().and_then(|enc| {
                crypto::decrypt_field(enc, &state.encryption_key).ok()
            });
            DataExportRating {
                beer_name: r.beer_name,
                brewery_name: r.brewery_name,
                score: r.score,
                notes,
                created_at: r.created_at,
            }
        })
        .collect();

    // Get badges
    let badges = db::badges::get_user_badges(&state.pool, auth.user_id).await?;
    let export_badges: Vec<DataExportBadge> = badges
        .into_iter()
        .map(|b| DataExportBadge {
            name: b.badge_name,
            description: b.badge_description,
            earned_at: b.earned_at,
        })
        .collect();

    // Get privacy settings
    let settings = db::privacy::get_privacy_settings(&state.pool, auth.user_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("Privacy settings not found".into()))?;

    Ok(Json(DataExportResponse {
        user: DataExportUser {
            username: user.username,
            email,
            display_name: user.display_name,
        },
        ratings: export_ratings,
        badges: export_badges,
        privacy_settings: PrivacySettingsResponse {
            profile_visibility: settings.profile_visibility,
            show_ratings: settings.show_ratings,
            show_badges: settings.show_badges,
            show_stats: settings.show_stats,
        },
    }))
}

/// DELETE /api/users/me
pub async fn delete_account(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<DeleteAccountRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    // Verify password before deletion
    let user = db::users::find_user_by_id(&state.pool, auth.user_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("User not found".into()))?;

    use argon2::{Argon2, PasswordHash, PasswordVerifier};
    let parsed_hash = PasswordHash::new(&user.password_hash)
        .map_err(|e| ApiError::Internal(format!("Invalid password hash: {e}")))?;
    Argon2::default()
        .verify_password(req.password.as_bytes(), &parsed_hash)
        .map_err(|_| ApiError::Unauthorized("Invalid password".into()))?;

    // Hard delete all user data (cascades via FK constraints)
    db::privacy::delete_user_data(&state.pool, auth.user_id).await?;

    tracing::info!("User {} deleted their account", auth.user_id);

    Ok(Json(serde_json::json!({
        "message": "Account and all associated data permanently deleted"
    })))
}
