use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;

use crate::error::WebAuthnError;

pub fn encode(data: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(data)
}

pub fn decode(s: &str) -> Result<Vec<u8>, WebAuthnError> {
    URL_SAFE_NO_PAD
        .decode(s)
        .map_err(|e| WebAuthnError::Base64Decode(e.to_string()))
}
