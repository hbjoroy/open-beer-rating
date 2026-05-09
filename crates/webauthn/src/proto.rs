//! Protocol types matching the WebAuthn JSON API.
//! These structs serialize/deserialize to/from the JSON the browser expects.

use serde::{Deserialize, Serialize};

use crate::base64url;

// --- Registration (navigator.credentials.create) ---

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreationChallengeResponse {
    pub rp: RelyingParty,
    pub user: UserEntity,
    pub challenge: String,
    pub pub_key_cred_params: Vec<PubKeyCredParam>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclude_credentials: Option<Vec<CredentialDescriptor>>,
    pub authenticator_selection: AuthenticatorSelection,
    pub attestation: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RelyingParty {
    pub name: String,
    pub id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserEntity {
    pub id: String,
    pub name: String,
    pub display_name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PubKeyCredParam {
    #[serde(rename = "type")]
    pub type_: String,
    pub alg: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialDescriptor {
    #[serde(rename = "type")]
    pub type_: String,
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transports: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthenticatorSelection {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authenticator_attachment: Option<String>,
    pub resident_key: String,
    pub require_resident_key: bool,
    pub user_verification: String,
}

// --- Client response types (from browser → server) ---

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterPublicKeyCredential {
    pub id: String,
    pub raw_id: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub response: AuthenticatorAttestationResponse,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthenticatorAttestationResponse {
    pub client_data_json: String,
    pub attestation_object: String,
    #[serde(default)]
    pub transports: Option<Vec<String>>,
}

// --- Authentication (navigator.credentials.get) ---

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestChallengeResponse {
    pub challenge: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u64>,
    pub rp_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_credentials: Option<Vec<CredentialDescriptor>>,
    pub user_verification: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthenticatePublicKeyCredential {
    pub id: String,
    pub raw_id: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub response: AuthenticatorAssertionResponse,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthenticatorAssertionResponse {
    pub authenticator_data: String,
    pub client_data_json: String,
    pub signature: String,
    #[serde(default)]
    pub user_handle: Option<Vec<u8>>,
}

// --- Shared ---

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectedClientData {
    #[serde(rename = "type")]
    pub type_: String,
    pub challenge: String,
    pub origin: String,
    #[serde(default)]
    pub cross_origin: Option<bool>,
}

/// Result of a successful authentication
#[derive(Debug, Clone)]
pub struct AuthenticationResult {
    pub credential_id: Vec<u8>,
    pub user_handle: Option<Vec<u8>>,
    pub new_counter: u32,
    pub backup_state: bool,
}

/// Parse the base64url-encoded clientDataJSON into CollectedClientData.
/// Also returns the raw bytes (needed for SHA-256 hashing).
pub fn parse_client_data(client_data_json_b64: &str) -> Result<CollectedClientData, crate::error::WebAuthnError> {
    let raw_bytes = base64url::decode(client_data_json_b64)?;
    let client_data: CollectedClientData = serde_json::from_slice(&raw_bytes)
        .map_err(|e| crate::error::WebAuthnError::JsonDecode(e.to_string()))?;
    Ok(client_data)
}

/// Decode clientDataJSON and return both parsed struct and raw bytes.
pub fn parse_client_data_raw(client_data_json_b64: &str) -> Result<(CollectedClientData, Vec<u8>), crate::error::WebAuthnError> {
    let raw_bytes = base64url::decode(client_data_json_b64)?;
    let client_data: CollectedClientData = serde_json::from_slice(&raw_bytes)
        .map_err(|e| crate::error::WebAuthnError::JsonDecode(e.to_string()))?;
    Ok((client_data, raw_bytes))
}
