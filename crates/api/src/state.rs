use sqlx::PgPool;
use std::sync::Arc;
use open_tappd_webauthn::WebAuthn;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub encryption_key: [u8; 32],
    pub jwt_secret: String,
    pub webauthn: Arc<WebAuthn>,
}
