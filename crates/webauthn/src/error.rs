use thiserror::Error;

#[derive(Debug, Error)]
pub enum WebAuthnError {
    #[error("base64url decode error: {0}")]
    Base64Decode(String),

    #[error("CBOR decode error: {0}")]
    CborDecode(String),

    #[error("JSON decode error: {0}")]
    JsonDecode(String),

    #[error("invalid client data type: expected {expected}, got {got}")]
    InvalidClientDataType { expected: String, got: String },

    #[error("challenge mismatch")]
    ChallengeMismatch,

    #[error("origin mismatch: expected {expected}, got {got}")]
    OriginMismatch { expected: String, got: String },

    #[error("RP ID hash mismatch")]
    RpIdHashMismatch,

    #[error("user present flag not set")]
    UserNotPresent,

    #[error("user verification required but not performed")]
    UserNotVerified,

    #[error("invalid backup flags: BE=0 but BS=1")]
    InvalidBackupFlags,

    #[error("unsupported attestation format: {0}")]
    UnsupportedAttestationFormat(String),

    #[error("unsupported COSE algorithm: {0}")]
    UnsupportedAlgorithm(i64),

    #[error("unsupported COSE key type: {0}")]
    UnsupportedKeyType(i64),

    #[error("invalid COSE key: {0}")]
    InvalidCoseKey(String),

    #[error("signature verification failed")]
    SignatureVerificationFailed,

    #[error("credential not found")]
    CredentialNotFound,

    #[error("possible credential cloning: counter did not increase")]
    CredentialPossibleCompromise,

    #[error("auth data too short: expected at least {expected} bytes, got {got}")]
    AuthDataTooShort { expected: usize, got: usize },

    #[error("credential ID too long")]
    CredentialIdTooLong,

    #[error("internal error: {0}")]
    Internal(String),
}
