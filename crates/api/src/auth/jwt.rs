use axum::{
    extract::FromRequestParts,
    http::request::Parts,
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD as B64};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use uuid::Uuid;

use crate::errors::ApiError;
use crate::state::AppState;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: Uuid,
    pub username: String,
    pub exp: u64,
    pub iat: u64,
}

const JWT_HEADER: &str = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9"; // {"alg":"HS256","typ":"JWT"}

pub fn create_token(user_id: Uuid, username: &str, secret: &str) -> Result<String, ApiError> {
    let now = chrono::Utc::now().timestamp() as u64;
    let claims = Claims {
        sub: user_id,
        username: username.to_string(),
        exp: now + 24 * 3600,
        iat: now,
    };

    let payload = serde_json::to_vec(&claims)
        .map_err(|e| ApiError::Internal(format!("JWT serialization failed: {e}")))?;
    let payload_b64 = B64.encode(&payload);

    let signing_input = format!("{JWT_HEADER}.{payload_b64}");

    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .map_err(|e| ApiError::Internal(format!("HMAC key error: {e}")))?;
    mac.update(signing_input.as_bytes());
    let signature = B64.encode(mac.finalize().into_bytes());

    Ok(format!("{signing_input}.{signature}"))
}

pub fn verify_token(token: &str, secret: &str) -> Result<Claims, ApiError> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return Err(ApiError::Unauthorized("Invalid token format".into()));
    }

    let signing_input = format!("{}.{}", parts[0], parts[1]);

    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .map_err(|e| ApiError::Internal(format!("HMAC key error: {e}")))?;
    mac.update(signing_input.as_bytes());

    let signature = B64
        .decode(parts[2])
        .map_err(|_| ApiError::Unauthorized("Invalid token signature encoding".into()))?;

    mac.verify_slice(&signature)
        .map_err(|_| ApiError::Unauthorized("Invalid token signature".into()))?;

    let payload = B64
        .decode(parts[1])
        .map_err(|_| ApiError::Unauthorized("Invalid token payload encoding".into()))?;

    let claims: Claims = serde_json::from_slice(&payload)
        .map_err(|_| ApiError::Unauthorized("Invalid token payload".into()))?;

    let now = chrono::Utc::now().timestamp() as u64;
    if claims.exp < now {
        return Err(ApiError::Unauthorized("Token expired".into()));
    }

    Ok(claims)
}

/// Extractor that validates JWT from the Authorization header.
#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user_id: Uuid,
    pub username: String,
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let auth_header = parts
            .headers
            .get("Authorization")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| ApiError::Unauthorized("Missing Authorization header".into()))?;

        let token = auth_header
            .strip_prefix("Bearer ")
            .ok_or_else(|| ApiError::Unauthorized("Invalid Authorization header format".into()))?;

        let claims = verify_token(token, &state.jwt_secret)?;

        Ok(AuthUser {
            user_id: claims.sub,
            username: claims.username,
        })
    }
}
