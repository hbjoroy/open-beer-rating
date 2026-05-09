//! Cryptographic operations for WebAuthn.

use p256::ecdsa::{signature::Verifier, Signature, VerifyingKey};
use p256::EncodedPoint;
use sha2::{Digest, Sha256};

use crate::credential::CoseKey;
use crate::error::WebAuthnError;

/// SHA-256 hash
pub(crate) fn sha256(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&result);
    out
}

/// Verify an ES256 (P-256 ECDSA) signature.
///
/// `message` is the raw data signed: authData || SHA-256(clientDataJSON).
/// The `p256` crate's `verify` internally hashes with SHA-256, but for WebAuthn
/// the signed data is already `authData || hash(clientDataJSON)`, NOT pre-hashed.
/// We need to use `verify_prehash` or feed the raw message.
///
/// Actually, ECDSA in WebAuthn: the authenticator signs
/// `SHA-256(authData || clientDataHash)` — wait, no. Per the spec:
///   signature = ECDSA-SHA256(authData_bytes || client_data_hash)
/// The authenticator performs SHA-256 internally. So the RP must verify by
/// providing `authData_bytes || client_data_hash` as the message,
/// and the verifier hashes it with SHA-256 before checking.
pub(crate) fn verify_es256(
    key: &CoseKey,
    message: &[u8],
    signature_bytes: &[u8],
) -> Result<(), WebAuthnError> {
    let CoseKey::EC2 { x, y } = key;

    // Build uncompressed SEC1 point: 0x04 || x || y
    let encoded_point = EncodedPoint::from_affine_coordinates(
        p256::FieldBytes::from_slice(x),
        p256::FieldBytes::from_slice(y),
        false, // uncompressed
    );

    let verifying_key = VerifyingKey::from_encoded_point(&encoded_point)
        .map_err(|e| WebAuthnError::InvalidCoseKey(format!("P-256 key invalid: {e}")))?;

    let signature = Signature::from_der(signature_bytes)
        .map_err(|_| WebAuthnError::SignatureVerificationFailed)?;

    verifying_key
        .verify(message, &signature)
        .map_err(|_| WebAuthnError::SignatureVerificationFailed)
}

/// Generate cryptographically random challenge bytes.
pub(crate) fn generate_challenge() -> Vec<u8> {
    use rand::Rng;
    let mut rng = rand::rng();
    let mut challenge = vec![0u8; 32];
    rng.fill(&mut challenge[..]);
    challenge
}
