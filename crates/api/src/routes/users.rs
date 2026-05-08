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
    pub password: String,
    pub email: Option<String>,
    pub display_name: Option<String>,
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
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub user: UserResponse,
}

pub async fn register(
    State(state): State<AppState>,
    Json(req): Json<RegisterRequest>,
) -> Result<(axum::http::StatusCode, Json<UserResponse>), ApiError> {
    validation::validate_username(&req.username)?;
    validation::validate_password(&req.password)?;

    let email_encrypted = if let Some(ref email) = req.email {
        validation::validate_email(email)?;
        Some(crypto::encrypt_field(email, &state.encryption_key)?)
    } else {
        None
    };

    let password_hash = hash_password(&req.password)?;

    let user = db::users::create_user(
        &state.pool,
        &req.username,
        email_encrypted.as_deref(),
        &password_hash,
    )
    .await?;

    Ok((
        axum::http::StatusCode::CREATED,
        Json(UserResponse {
            id: user.id,
            username: user.username,
            display_name: user.display_name,
            created_at: user.created_at,
        }),
    ))
}

pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, ApiError> {
    let user = db::users::find_user_by_username(&state.pool, &req.username)
        .await?
        .ok_or_else(|| ApiError::Unauthorized("Invalid username or password".into()))?;

    verify_password(&req.password, &user.password_hash)?;

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

fn hash_password(password: &str) -> Result<String, ApiError> {
    use argon2::{Argon2, PasswordHasher, password_hash::SaltString};
    use argon2::password_hash::rand_core::OsRng;

    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();

    argon2
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|e| ApiError::Internal(format!("Password hashing failed: {e}")))
}

fn verify_password(password: &str, hash: &str) -> Result<(), ApiError> {
    use argon2::{Argon2, PasswordVerifier, PasswordHash};

    let parsed_hash = PasswordHash::new(hash)
        .map_err(|e| ApiError::Internal(format!("Invalid password hash: {e}")))?;

    Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .map_err(|_| ApiError::Unauthorized("Invalid username or password".into()))
}
