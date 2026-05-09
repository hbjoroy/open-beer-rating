//! Registration ceremony implementation.

use crate::base64url;
use crate::cbor;
use crate::config::WebAuthnConfig;
use crate::credential::{StoredCredential, CredentialId, parse_auth_data};
use crate::crypto;
use crate::error::WebAuthnError;
use crate::proto::*;

/// Server-side state for a pending registration ceremony.
#[derive(Debug)]
pub(crate) struct RegistrationState {
    pub challenge: Vec<u8>,
    pub user_id: uuid::Uuid,
    pub rp_id: String,
    pub origin: String,
}

/// Begin registration: generate challenge and creation options.
pub(crate) fn start_registration(
    config: &WebAuthnConfig,
    user_id: uuid::Uuid,
    username: &str,
    display_name: &str,
    exclude_credentials: Vec<CredentialId>,
) -> Result<(CreationChallengeResponse, RegistrationState), WebAuthnError> {
    let challenge = crypto::generate_challenge();

    let exclude = if exclude_credentials.is_empty() {
        None
    } else {
        Some(
            exclude_credentials
                .iter()
                .map(|id| CredentialDescriptor {
                    type_: "public-key".into(),
                    id: base64url::encode(id),
                    transports: None,
                })
                .collect(),
        )
    };

    let response = CreationChallengeResponse {
        rp: RelyingParty {
            name: config.rp_name.clone(),
            id: config.rp_id.clone(),
        },
        user: UserEntity {
            id: base64url::encode(user_id.as_bytes()),
            name: username.to_string(),
            display_name: display_name.to_string(),
        },
        challenge: base64url::encode(&challenge),
        pub_key_cred_params: vec![PubKeyCredParam {
            type_: "public-key".into(),
            alg: -7, // ES256
        }],
        timeout: Some(300_000),
        exclude_credentials: exclude,
        authenticator_selection: AuthenticatorSelection {
            authenticator_attachment: None,
            resident_key: "required".into(),
            require_resident_key: true,
            user_verification: "required".into(),
        },
        attestation: "none".into(),
    };

    let state = RegistrationState {
        challenge,
        user_id,
        rp_id: config.rp_id.clone(),
        origin: config.origin.clone(),
    };

    Ok((response, state))
}

/// Complete registration: validate the authenticator response.
pub(crate) fn finish_registration(
    _config: &WebAuthnConfig,
    state: &RegistrationState,
    response: &RegisterPublicKeyCredential,
) -> Result<StoredCredential, WebAuthnError> {
    // Step 1-2: Parse and validate clientDataJSON
    let (client_data, client_data_raw) =
        parse_client_data_raw(&response.response.client_data_json)?;

    if client_data.type_ != "webauthn.create" {
        return Err(WebAuthnError::InvalidClientDataType {
            expected: "webauthn.create".into(),
            got: client_data.type_.clone(),
        });
    }

    // Step 3: Verify challenge
    let challenge_bytes = base64url::decode(&client_data.challenge)?;
    if challenge_bytes != state.challenge {
        return Err(WebAuthnError::ChallengeMismatch);
    }

    // Step 4: Verify origin
    if client_data.origin != state.origin {
        return Err(WebAuthnError::OriginMismatch {
            expected: state.origin.clone(),
            got: client_data.origin.clone(),
        });
    }

    // Step 5: crossOrigin check
    if client_data.cross_origin == Some(true) {
        return Err(WebAuthnError::OriginMismatch {
            expected: state.origin.clone(),
            got: format!("{} (cross-origin)", client_data.origin),
        });
    }

    // Step 7: Hash clientDataJSON
    let _client_data_hash = crypto::sha256(&client_data_raw);

    // Step 8: Decode attestationObject
    let att_obj_bytes = base64url::decode(&response.response.attestation_object)?;
    let att_obj = cbor::parse_attestation_object(&att_obj_bytes)?;

    // Step 9: Parse authData
    let auth_data = parse_auth_data(&att_obj.auth_data)?;

    // Step 10: Verify rpIdHash
    let expected_rp_id_hash = crypto::sha256(state.rp_id.as_bytes());
    if auth_data.rp_id_hash != expected_rp_id_hash {
        return Err(WebAuthnError::RpIdHashMismatch);
    }

    // Step 11: Verify UP
    if !auth_data.flags.user_present {
        return Err(WebAuthnError::UserNotPresent);
    }

    // Step 12: Verify UV (we require it)
    if !auth_data.flags.user_verified {
        return Err(WebAuthnError::UserNotVerified);
    }

    // Step 13: Backup flags consistency
    if !auth_data.flags.backup_eligible && auth_data.flags.backup_state {
        return Err(WebAuthnError::InvalidBackupFlags);
    }

    // Step 15-16: Attestation format — we only accept "none" and "packed" self-attestation
    match att_obj.fmt.as_str() {
        "none" => {
            // No attestation statement to verify
        }
        "packed" => {
            // Accept packed without verifying x5c chain (self-attestation)
            // For full security, you'd verify the attestation cert chain here
        }
        fmt => {
            return Err(WebAuthnError::UnsupportedAttestationFormat(fmt.to_string()));
        }
    }

    // Step 17-18: Extract credential from ACD
    let acd = auth_data
        .attested_credential_data
        .ok_or_else(|| WebAuthnError::AuthDataTooShort { expected: 55, got: 37 })?;

    // Step 19: Verify algorithm (we only support ES256)
    // Already validated during COSE key parsing

    let credential = StoredCredential {
        credential_id: acd.credential_id,
        public_key: acd.public_key,
        counter: auth_data.counter,
        transports: response.response.transports.clone(),
        user_verified: auth_data.flags.user_verified,
        backup_eligible: auth_data.flags.backup_eligible,
        backup_state: auth_data.flags.backup_state,
    };

    Ok(credential)
}
