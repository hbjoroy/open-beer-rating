use axum::extract::State;
use axum::Json;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::jwt;
use crate::db;
use crate::errors::ApiError;
use crate::state::AppState;
use open_tappd_domain::crypto;
use open_tappd_domain::validation;

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub email: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RegisterResponse {
    pub user: UserResponse,
    pub recovery_key: String,
    pub token: String,
}

#[derive(Debug, Serialize)]
pub struct UserResponse {
    pub id: Uuid,
    pub username: String,
    pub display_name: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub recovery_key: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub user: UserResponse,
}

/// POST /api/users/register — create account with system-generated recovery key
pub async fn register(
    State(state): State<AppState>,
    Json(req): Json<RegisterRequest>,
) -> Result<(axum::http::StatusCode, Json<RegisterResponse>), ApiError> {
    validation::validate_username(&req.username)?;

    let email_encrypted = if let Some(ref email) = req.email {
        validation::validate_email(email)?;
        Some(crypto::encrypt_field(email, &state.encryption_key)?)
    } else {
        None
    };

    // Generate a cryptographic recovery key (24 chars in groups of 4)
    let recovery_key = generate_recovery_key();
    let recovery_key_hash = hash_recovery_key(&recovery_key)?;

    let user = db::users::create_user(
        &state.pool,
        &req.username,
        email_encrypted.as_deref(),
        &recovery_key_hash,
    )
    .await?;

    let token = jwt::create_token(user.id, &user.username, &state.jwt_secret)?;

    Ok((
        axum::http::StatusCode::CREATED,
        Json(RegisterResponse {
            user: UserResponse {
                id: user.id,
                username: user.username,
                display_name: user.display_name,
                created_at: user.created_at,
            },
            recovery_key,
            token,
        }),
    ))
}

/// POST /api/users/login — recovery login with username + recovery key
pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, ApiError> {
    let user = db::users::find_user_by_username(&state.pool, &req.username)
        .await?
        .ok_or_else(|| ApiError::Unauthorized("Invalid username or recovery key".into()))?;

    verify_recovery_key(&req.recovery_key, &user.recovery_key_hash)?;

    let token = jwt::create_token(user.id, &user.username, &state.jwt_secret)?;

    Ok(Json(LoginResponse {
        token,
        user: UserResponse {
            id: user.id,
            username: user.username,
            display_name: user.display_name,
            created_at: user.created_at,
        },
    }))
}

/// Generate a 24-character recovery key in groups of 4: ABCD-EFGH-IJKL-MNOP-QRST-UVWX
fn generate_recovery_key() -> String {
    use argon2::password_hash::rand_core::{OsRng, RngCore};

    const CHARSET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789"; // no I, O, 0, 1 (avoid confusion)
    let mut key = String::with_capacity(29); // 24 chars + 5 dashes
    let mut rng = OsRng;

    for i in 0..24 {
        if i > 0 && i % 4 == 0 {
            key.push('-');
        }
        let idx = (rng.next_u32() % CHARSET.len() as u32) as usize;
        key.push(CHARSET[idx] as char);
    }
    key
}

fn hash_recovery_key(key: &str) -> Result<String, ApiError> {
    use argon2::{Argon2, PasswordHasher, password_hash::SaltString};
    use argon2::password_hash::rand_core::OsRng;

    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();

    argon2
        .hash_password(key.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|e| ApiError::Internal(format!("Recovery key hashing failed: {e}")))
}

fn verify_recovery_key(key: &str, hash: &str) -> Result<(), ApiError> {
    use argon2::{Argon2, PasswordVerifier, PasswordHash};

    let parsed_hash = PasswordHash::new(hash)
        .map_err(|e| ApiError::Internal(format!("Invalid recovery key hash: {e}")))?;

    Argon2::default()
        .verify_password(key.as_bytes(), &parsed_hash)
        .map_err(|_| ApiError::Unauthorized("Invalid username or recovery key".into()))
}
