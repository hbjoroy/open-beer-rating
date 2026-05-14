//! # open-tappd-webauthn
//!
//! Pure-Rust WebAuthn Relying Party library.
//!
//! No OpenSSL dependency — uses `p256` for ECDSA, `sha2` for hashing,
//! `ciborium` for CBOR, and `rand` for challenge generation.
//!
//! ## Supported features
//! - Registration ceremony (attestation: "none" and "self")
//! - Authentication ceremony (discoverable + non-discoverable credentials)
//! - ES256 (P-256 ECDSA with SHA-256) signature verification
//!
//! ## Example
//! ```no_run
//! use open_tappd_webauthn::{WebAuthn, WebAuthnConfig};
//!
//! let config = WebAuthnConfig::new("localhost", "http://localhost:8080");
//! let webauthn = WebAuthn::new(config);
//! ```

mod base64url;
mod cbor;
mod config;
mod credential;
mod crypto;
mod error;
mod proto;
mod registration;
mod authentication;

pub use config::WebAuthnConfig;
pub use credential::{StoredCredential, CredentialId};
pub use error::WebAuthnError;
pub use proto::*;

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

const CHALLENGE_TTL_SECS: u64 = 300;

/// WebAuthn Relying Party instance.
pub struct WebAuthn {
    config: WebAuthnConfig,
    reg_challenges: Mutex<HashMap<Vec<u8>, (registration::RegistrationState, Instant)>>,
    auth_challenges: Mutex<HashMap<Vec<u8>, (authentication::AuthenticationState, Instant)>>,
}

impl WebAuthn {
    pub fn new(config: WebAuthnConfig) -> Self {
        Self {
            config,
            reg_challenges: Mutex::new(HashMap::new()),
            auth_challenges: Mutex::new(HashMap::new()),
        }
    }

    /// Start a passkey registration ceremony.
    pub fn start_registration(
        &self,
        user_id: uuid::Uuid,
        username: &str,
        display_name: &str,
        exclude_credentials: Vec<CredentialId>,
    ) -> Result<CreationChallengeResponse, WebAuthnError> {
        self.cleanup_expired();
        let (challenge_response, state) =
            registration::start_registration(&self.config, user_id, username, display_name, exclude_credentials)?;

        let mut cache = self.reg_challenges.lock().map_err(|_| WebAuthnError::Internal("lock poisoned".into()))?;
        cache.insert(state.challenge.clone(), (state, Instant::now()));

        Ok(challenge_response)
    }

    /// Complete a passkey registration ceremony.
    pub fn finish_registration(
        &self,
        response: &RegisterPublicKeyCredential,
    ) -> Result<StoredCredential, WebAuthnError> {
        let client_data = proto::parse_client_data(&response.response.client_data_json)?;
        let challenge_bytes = base64url::decode(&client_data.challenge)?;

        let state = {
            let mut cache = self.reg_challenges.lock().map_err(|_| WebAuthnError::Internal("lock poisoned".into()))?;
            cache
                .remove(&challenge_bytes)
                .ok_or(WebAuthnError::ChallengeMismatch)?
                .0
        };

        registration::finish_registration(&self.config, &state, response)
    }

    /// Start a passkey authentication ceremony (discoverable credentials).
    /// If `allow_credentials` is non-empty, only those credential IDs are offered to the user.
    pub fn start_authentication(&self, allow_credentials: Vec<CredentialId>) -> Result<(RequestChallengeResponse, Vec<u8>), WebAuthnError> {
        self.cleanup_expired();
        let (challenge_response, state) = authentication::start_authentication(&self.config, allow_credentials)?;
        let challenge_key = state.challenge.clone();

        let mut cache = self.auth_challenges.lock().map_err(|_| WebAuthnError::Internal("lock poisoned".into()))?;
        cache.insert(challenge_key.clone(), (state, Instant::now()));

        Ok((challenge_response, challenge_key))
    }

    /// Complete a passkey authentication ceremony.
    ///
    /// `credentials` should be the stored credentials for the user identified
    /// by the `userHandle` in the assertion response.
    pub fn finish_authentication(
        &self,
        response: &AuthenticatePublicKeyCredential,
        credentials: &[StoredCredential],
    ) -> Result<AuthenticationResult, WebAuthnError> {
        let client_data = proto::parse_client_data(&response.response.client_data_json)?;
        let challenge_bytes = base64url::decode(&client_data.challenge)?;

        let state = {
            let mut cache = self.auth_challenges.lock().map_err(|_| WebAuthnError::Internal("lock poisoned".into()))?;
            cache
                .remove(&challenge_bytes)
                .ok_or(WebAuthnError::ChallengeMismatch)?
                .0
        };

        authentication::finish_authentication(&self.config, &state, response, credentials)
    }

    /// Extract the userHandle from an authentication response (for discoverable credentials).
    pub fn get_user_handle(response: &AuthenticatePublicKeyCredential) -> Option<&[u8]> {
        response.response.user_handle.as_deref()
    }

    fn cleanup_expired(&self) {
        let cutoff = Instant::now() - std::time::Duration::from_secs(CHALLENGE_TTL_SECS);
        if let Ok(mut cache) = self.reg_challenges.lock() {
            cache.retain(|_, (_, ts)| *ts > cutoff);
        }
        if let Ok(mut cache) = self.auth_challenges.lock() {
            cache.retain(|_, (_, ts)| *ts > cutoff);
        }
    }
}
