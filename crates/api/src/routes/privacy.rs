use axum::Json;
use axum::extract::State;
use serde::{Deserialize, Serialize};

use crate::auth::jwt::AuthUser;
use crate::db;
use crate::errors::ApiError;
use crate::state::AppState;
use open_tappd_domain::crypto;
use base64::Engine;

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
    pub identity: Option<crate::identity::LinkedIdentity>,
    // Compatibility summary: the latest canonical tasting for each beer.
    pub ratings: Vec<DataExportRating>,
    pub tastings: Vec<DataExportTasting>,
    pub locations: Vec<db::privacy::ExportLocationRow>,
    pub created_sessions: Vec<db::privacy::ExportSessionRow>,
    pub session_memberships: Vec<db::privacy::ExportMembershipRow>,
    pub badges: Vec<DataExportBadge>,
    pub passkeys: Vec<db::privacy::ExportPasskeyRow>,
    pub privacy_settings: DataExportPrivacySettings,
}

#[derive(Debug, Serialize)]
pub struct DataExportPrivacySettings {
    pub profile_visibility: String,
    pub show_ratings: bool,
    pub show_badges: bool,
    pub show_stats: bool,
    pub show_tasting_sessions: bool,
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
pub struct DataExportTasting {
    pub id: uuid::Uuid,
    pub beer_id: uuid::Uuid,
    pub beer_name: String,
    pub brewery_name: String,
    pub score: i32,
    pub serving_style: Option<open_tappd_domain::models::tasting::ServingStyle>,
    pub notes: Option<String>,
    pub location_id: Option<uuid::Uuid>,
    pub location_name: Option<String>,
    pub session_id: Option<uuid::Uuid>,
    pub session_name: Option<String>,
    pub tasted_at: chrono::DateTime<chrono::Utc>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub photo: Option<DataExportPhoto>,
}

#[derive(Debug, Serialize)]
pub struct DataExportPhoto {
    pub content_type: &'static str,
    pub thumb_base64: String,
    pub large_base64: String,
}

#[derive(Debug, Serialize)]
pub struct DataExportBadge {
    pub name: String,
    pub description: String,
    pub earned_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct DeleteAccountRequest {
    pub recovery_key: String,
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
            ));
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
    let export = db::privacy::export_user_data(&state.pool, auth.user_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("User not found".into()))?;
    let email = export.user.email_encrypted.as_ref().map(|enc| {
        crypto::decrypt_field(enc, &state.encryption_key)
            .unwrap_or_else(|_| "[decryption error]".into())
    });

    let tastings: Vec<DataExportTasting> = export
        .tastings
        .into_iter()
        .map(|t| DataExportTasting {
            id: t.id,
            beer_id: t.beer_id,
            beer_name: t.beer_name,
            brewery_name: t.brewery_name,
            score: t.score,
            serving_style: t.serving_style,
            notes: t.notes_encrypted.as_ref().map(|enc| {
                crypto::decrypt_field(enc, &state.encryption_key)
                    .unwrap_or_else(|_| "[decryption error]".into())
            }),
            location_id: t.location_id,
            location_name: t.location_name,
            session_id: t.session_id,
            session_name: t.session_name,
            tasted_at: t.tasted_at,
            created_at: t.created_at,
            updated_at: t.updated_at,
            photo: t.photo_thumb_jpeg.zip(t.photo_large_jpeg).map(|(thumb, large)| DataExportPhoto {
                content_type: "image/jpeg",
                thumb_base64: base64::engine::general_purpose::STANDARD.encode(thumb),
                large_base64: base64::engine::general_purpose::STANDARD.encode(large),
            }),
        })
        .collect();
    // The snapshot is sorted newest first, with deterministic tie breakers.
    // Preserve the old response shape without omitting history from `tastings`.
    let mut seen_beers = std::collections::HashSet::new();
    let ratings = tastings
        .iter()
        .filter(|t| seen_beers.insert(t.beer_id))
        .map(|t| DataExportRating {
            beer_name: t.beer_name.clone(),
            brewery_name: t.brewery_name.clone(),
            score: t.score,
            notes: t.notes.clone(),
            created_at: t.tasted_at,
        })
        .collect();
    let badges = export
        .badges
        .into_iter()
        .map(|b| DataExportBadge {
            name: b.badge_name,
            description: b.badge_description,
            earned_at: b.earned_at,
        })
        .collect();

    Ok(Json(DataExportResponse {
        identity: export.identity,
        user: DataExportUser {
            username: export.user.username,
            email,
            display_name: export.user.display_name,
        },
        ratings,
        tastings,
        locations: export.locations,
        created_sessions: export.created_sessions,
        session_memberships: export.session_memberships,
        badges,
        passkeys: export.passkeys,
        privacy_settings: DataExportPrivacySettings {
            profile_visibility: export.settings.profile_visibility,
            show_ratings: export.settings.show_ratings,
            show_badges: export.settings.show_badges,
            show_stats: export.settings.show_stats,
            show_tasting_sessions: export.settings.show_tasting_sessions,
        },
    }))
}

/// DELETE /api/users/me
pub async fn delete_account(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<DeleteAccountRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    // Verify recovery key before deletion
    let user = db::users::find_user_by_id(&state.pool, auth.user_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("User not found".into()))?;

    use argon2::{Argon2, PasswordHash, PasswordVerifier};
    let parsed_hash = PasswordHash::new(&user.recovery_key_hash)
        .map_err(|e| ApiError::Internal(format!("Invalid recovery key hash: {e}")))?;
    let normalized_key = req.recovery_key.to_uppercase();
    Argon2::default()
        .verify_password(normalized_key.as_bytes(), &parsed_hash)
        .map_err(|_| ApiError::Unauthorized("Invalid recovery key".into()))?;

    // Remove owned context and detach shared references in one transaction.
    db::privacy::delete_user_data(&state.pool, auth.user_id).await?;

    tracing::info!("User {} deleted their account", auth.user_id);

    Ok(Json(serde_json::json!({
        "message": "Account and all associated data permanently deleted"
    })))
}
