use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

#[derive(Debug)]
pub enum ApiError {
    Validation(String),
    Conflict(String),
    NotFound(String),
    Unauthorized(String),
    Internal(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            ApiError::Validation(msg) => (StatusCode::UNPROCESSABLE_ENTITY, msg),
            ApiError::Conflict(msg) => (StatusCode::CONFLICT, msg),
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            ApiError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, msg),
            ApiError::Internal(msg) => {
                tracing::error!("Internal error: {}", msg);
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".to_string())
            }
        };

        (status, Json(json!({ "error": message }))).into_response()
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(err: sqlx::Error) -> Self {
        match &err {
            sqlx::Error::Database(db_err) => {
                if let Some(constraint) = db_err.constraint() {
                    if constraint == "users_username_key" {
                        return ApiError::Conflict("Username already taken".into());
                    }
                }
                ApiError::Internal(err.to_string())
            }
            _ => ApiError::Internal(err.to_string()),
        }
    }
}

impl From<open_tappd_domain::errors::DomainError> for ApiError {
    fn from(err: open_tappd_domain::errors::DomainError) -> Self {
        match err {
            open_tappd_domain::errors::DomainError::Validation(msg) => ApiError::Validation(msg),
            open_tappd_domain::errors::DomainError::NotFound(msg) => ApiError::NotFound(msg),
            open_tappd_domain::errors::DomainError::Conflict(msg) => ApiError::Conflict(msg),
            open_tappd_domain::errors::DomainError::Encryption(msg) => ApiError::Internal(msg),
        }
    }
}
