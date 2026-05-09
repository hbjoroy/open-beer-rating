//! Authentication ceremony implementation.

use crate::base64url;
use crate::config::WebAuthnConfig;
use crate::credential::{StoredCredential, parse_auth_data};
use crate::crypto;
use crate::error::WebAuthnError;
use crate::proto::*;

/// Server-side state for a pending authentication ceremony.
#[derive(Debug)]
pub(crate) struct AuthenticationState {
    pub challenge: Vec<u8>,
    pub rp_id: String,
    pub origin: String,
}

/// Begin authentication: generate challenge and request options.
/// Uses discoverable credentials (empty allowCredentials).
pub(crate) fn start_authentication(
    config: &WebAuthnConfig,
) -> Result<(RequestChallengeResponse, AuthenticationState), WebAuthnError> {
    let challenge = crypto::generate_challenge();

    let response = RequestChallengeResponse {
        challenge: base64url::encode(&challenge),
        timeout: Some(300_000),
        rp_id: config.rp_id.clone(),
        allow_credentials: Some(vec![]), // empty = discoverable
        user_verification: "required".into(),
    };

    let state = AuthenticationState {
        challenge,
        rp_id: config.rp_id.clone(),
        origin: config.origin.clone(),
    };

    Ok((response, state))
}

/// Complete authentication: validate the assertion response.
pub(crate) fn finish_authentication(
    _config: &WebAuthnConfig,
    state: &AuthenticationState,
    response: &AuthenticatePublicKeyCredential,
    credentials: &[StoredCredential],
) -> Result<AuthenticationResult, WebAuthnError> {
    // Step 1: Find matching credential by ID
    let cred_id_bytes = base64url::decode(&response.raw_id)?;
    let stored_cred = credentials
        .iter()
        .find(|c| c.credential_id == cred_id_bytes)
        .ok_or(WebAuthnError::CredentialNotFound)?;

    // Step 3-6: Parse and validate clientDataJSON
    let (client_data, client_data_raw) =
        parse_client_data_raw(&response.response.client_data_json)?;

    if client_data.type_ != "webauthn.get" {
        return Err(WebAuthnError::InvalidClientDataType {
            expected: "webauthn.get".into(),
            got: client_data.type_.clone(),
        });
    }

    let challenge_bytes = base64url::decode(&client_data.challenge)?;
    if challenge_bytes != state.challenge {
        return Err(WebAuthnError::ChallengeMismatch);
    }

    if client_data.origin != state.origin {
        return Err(WebAuthnError::OriginMismatch {
            expected: state.origin.clone(),
            got: client_data.origin.clone(),
        });
    }

    // Step 7-10: Parse and validate authData
    let auth_data_bytes = base64url::decode(&response.response.authenticator_data)?;
    let auth_data = parse_auth_data(&auth_data_bytes)?;

    let expected_rp_id_hash = crypto::sha256(state.rp_id.as_bytes());
    if auth_data.rp_id_hash != expected_rp_id_hash {
        return Err(WebAuthnError::RpIdHashMismatch);
    }

    if !auth_data.flags.user_present {
        return Err(WebAuthnError::UserNotPresent);
    }

    if !auth_data.flags.user_verified {
        return Err(WebAuthnError::UserNotVerified);
    }

    // Step 13: Backup flags
    if !auth_data.flags.backup_eligible && auth_data.flags.backup_state {
        return Err(WebAuthnError::InvalidBackupFlags);
    }

    // Step 14: Compute verification data = authData || SHA-256(clientDataJSON)
    let client_data_hash = crypto::sha256(&client_data_raw);
    let mut verification_data = auth_data_bytes.clone();
    verification_data.extend_from_slice(&client_data_hash);

    // Step 15: Verify signature
    let signature_bytes = base64url::decode(&response.response.signature)?;
    crypto::verify_es256(&stored_cred.public_key, &verification_data, &signature_bytes)?;

    // Step 16: Counter validation
    if auth_data.counter > 0 || stored_cred.counter > 0 {
        if auth_data.counter <= stored_cred.counter {
            return Err(WebAuthnError::CredentialPossibleCompromise);
        }
    }

    Ok(AuthenticationResult {
        credential_id: cred_id_bytes,
        user_handle: response.response.user_handle.clone(),
        new_counter: auth_data.counter,
        backup_state: auth_data.flags.backup_state,
    })
}
