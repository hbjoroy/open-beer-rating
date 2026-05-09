use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use open_tappd_webauthn::{
    AuthenticatePublicKeyCredential, RegisterPublicKeyCredential,
    StoredCredential, WebAuthn,
};

use crate::auth::jwt::{self, AuthUser};
use crate::db;
use crate::errors::ApiError;
use crate::state::AppState;

// --- Registration (authenticated: add passkey to existing account) ---

#[derive(Debug, Serialize)]
pub struct PasskeyRegisterStartResponse {
    pub challenge: serde_json::Value,
}

/// POST /api/passkeys/register/start — begin passkey registration (requires auth)
pub async fn register_start(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<PasskeyRegisterStartResponse>, ApiError> {
    tracing::debug!("Passkey register/start for user {} ({})", auth.user_id, auth.username);

    let existing = db::passkeys::list_passkeys(&state.pool, auth.user_id).await?;
    let exclude: Vec<Vec<u8>> = existing.iter().map(|p| p.credential_id.clone()).collect();
    tracing::debug!("Excluding {} existing credentials", exclude.len());

    let ccr = state
        .webauthn
        .start_registration(auth.user_id, &auth.username, &auth.username, exclude)
        .map_err(|e| ApiError::Internal(format!("WebAuthn registration start failed: {e}")))?;

    let challenge_json = serde_json::to_value(&ccr)
        .map_err(|e| ApiError::Internal(format!("JSON serialize: {e}")))?;

    tracing::debug!("Passkey challenge response: {}", serde_json::to_string(&challenge_json).unwrap_or_default());

    Ok(Json(PasskeyRegisterStartResponse {
        challenge: challenge_json,
    }))
}

#[derive(Debug, Deserialize)]
pub struct PasskeyRegisterFinishRequest {
    #[serde(flatten)]
    pub credential: RegisterPublicKeyCredential,
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PasskeyRegisterFinishResponse {
    pub id: Uuid,
    pub name: String,
}

/// POST /api/passkeys/register/finish — complete passkey registration
pub async fn register_finish(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<PasskeyRegisterFinishRequest>,
) -> Result<(axum::http::StatusCode, Json<PasskeyRegisterFinishResponse>), ApiError> {
    tracing::debug!("Passkey register/finish for user {} — credential id: {}", auth.user_id, req.credential.id);

    let stored_cred = state
        .webauthn
        .finish_registration(&req.credential)
        .map_err(|e| ApiError::Validation(format!("Passkey registration failed: {e}")))?;

    // Serialize the StoredCredential to JSON for DB storage
    let cred_json = serde_json::to_vec(&stored_cred)
        .map_err(|e| ApiError::Internal(format!("serialize credential: {e}")))?;
    let transports_str = stored_cred.transports.as_ref().map(|t| t.join(","));
    let name = req.name.unwrap_or_else(|| "Passkey".to_string());

    let row = db::passkeys::store_passkey(
        &state.pool,
        auth.user_id,
        &stored_cred.credential_id,
        &cred_json,
        stored_cred.counter as i32,
        transports_str.as_deref(),
        &name,
    )
    .await?;

    Ok((
        axum::http::StatusCode::CREATED,
        Json(PasskeyRegisterFinishResponse {
            id: row.id,
            name: row.name,
        }),
    ))
}

// --- Authentication (unauthenticated: sign in with passkey) ---

#[derive(Debug, Serialize)]
pub struct PasskeyAuthStartResponse {
    pub challenge: serde_json::Value,
    pub challenge_id: String,
}

/// POST /api/passkeys/auth/start — begin passkey authentication
pub async fn auth_start(
    State(state): State<AppState>,
) -> Result<Json<PasskeyAuthStartResponse>, ApiError> {
    let (rcr, challenge_key) = state
        .webauthn
        .start_authentication()
        .map_err(|e| ApiError::Internal(format!("WebAuthn auth start failed: {e}")))?;

    let challenge_json = serde_json::to_value(&rcr)
        .map_err(|e| ApiError::Internal(format!("JSON serialize: {e}")))?;

    Ok(Json(PasskeyAuthStartResponse {
        challenge: challenge_json,
        challenge_id: base64::Engine::encode(
            &base64::engine::general_purpose::URL_SAFE_NO_PAD,
            &challenge_key,
        ),
    }))
}

/// POST /api/passkeys/auth/finish — complete passkey authentication, returns JWT
pub async fn auth_finish(
    State(state): State<AppState>,
    Json(response): Json<AuthenticatePublicKeyCredential>,
) -> Result<Json<crate::routes::users::LoginResponse>, ApiError> {
    // Extract userHandle to find the user
    let user_handle = WebAuthn::get_user_handle(&response)
        .ok_or_else(|| ApiError::Unauthorized("No userHandle in assertion".into()))?;

    // Parse user UUID from userHandle bytes
    let user_id = Uuid::from_slice(user_handle)
        .map_err(|_| ApiError::Unauthorized("Invalid userHandle format".into()))?;

    // Load user's stored credentials
    let passkey_rows = db::passkeys::list_passkeys(&state.pool, user_id).await?;
    if passkey_rows.is_empty() {
        return Err(ApiError::Unauthorized("No passkeys registered".into()));
    }

    let stored_creds: Vec<StoredCredential> = passkey_rows
        .iter()
        .filter_map(|row| serde_json::from_slice(&row.public_key_cbor).ok())
        .collect();

    if stored_creds.is_empty() {
        return Err(ApiError::Internal("Failed to deserialize stored credentials".into()));
    }

    let auth_result = state
        .webauthn
        .finish_authentication(&response, &stored_creds)
        .map_err(|e| ApiError::Unauthorized(format!("Passkey authentication failed: {e}")))?;

    // Update counter for the matched credential
    if let Some(row) = passkey_rows
        .iter()
        .find(|r| r.credential_id == auth_result.credential_id)
    {
        db::passkeys::update_passkey_counter(&state.pool, row.id, auth_result.new_counter as i32)
            .await?;
    }

    // Look up user and issue JWT
    let user = db::users::find_user_by_id(&state.pool, user_id)
        .await?
        .ok_or_else(|| ApiError::Internal("User not found for passkey".into()))?;

    let token = jwt::create_token(user.id, &user.username, &state.jwt_secret)?;

    Ok(Json(crate::routes::users::LoginResponse {
        token,
        user: crate::routes::users::UserResponse {
            id: user.id,
            username: user.username,
            display_name: user.display_name,
            created_at: user.created_at,
        },
    }))
}

// --- Management (authenticated) ---

#[derive(Debug, Serialize)]
pub struct PasskeyListItem {
    pub id: Uuid,
    pub name: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// GET /api/passkeys — list user's passkeys
pub async fn list_passkeys(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<Vec<PasskeyListItem>>, ApiError> {
    let passkeys = db::passkeys::list_passkeys(&state.pool, auth.user_id).await?;
    Ok(Json(
        passkeys
            .into_iter()
            .map(|p| PasskeyListItem {
                id: p.id,
                name: p.name,
                created_at: p.created_at,
            })
            .collect(),
    ))
}

/// DELETE /api/passkeys/:id — remove a passkey
pub async fn delete_passkey(
    State(state): State<AppState>,
    auth: AuthUser,
    axum::extract::Path(id): axum::extract::Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let count = db::passkeys::count_passkeys(&state.pool, auth.user_id).await?;
    if count <= 1 {
        return Err(ApiError::Validation(
            "Cannot remove your last passkey. Add another passkey first.".into(),
        ));
    }

    let deleted = db::passkeys::delete_passkey(&state.pool, id, auth.user_id).await?;
    if !deleted {
        return Err(ApiError::NotFound("Passkey not found".into()));
    }

    Ok(Json(serde_json::json!({ "message": "Passkey removed" })))
}
