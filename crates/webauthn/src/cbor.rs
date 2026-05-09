//! CBOR parsing for WebAuthn attestationObject and COSE keys.

use ciborium::Value;

use crate::credential::CoseKey;
use crate::error::WebAuthnError;

/// Attestation object parsed from CBOR
pub(crate) struct AttestationObject {
    pub fmt: String,
    pub auth_data: Vec<u8>,
    // attStmt is not stored — we only support "none" and "self"
}

/// Parse the attestation object from CBOR bytes.
pub(crate) fn parse_attestation_object(data: &[u8]) -> Result<AttestationObject, WebAuthnError> {
    let value: Value =
        ciborium::from_reader(data).map_err(|e| WebAuthnError::CborDecode(e.to_string()))?;

    let map = match value {
        Value::Map(m) => m,
        _ => return Err(WebAuthnError::CborDecode("attestationObject is not a CBOR map".into())),
    };

    let mut fmt = None;
    let mut auth_data = None;

    for (k, v) in &map {
        match k {
            Value::Text(key) if key == "fmt" => {
                if let Value::Text(f) = v {
                    fmt = Some(f.clone());
                }
            }
            Value::Text(key) if key == "authData" => {
                if let Value::Bytes(b) = v {
                    auth_data = Some(b.clone());
                }
            }
            _ => {} // skip attStmt, extensions, etc.
        }
    }

    Ok(AttestationObject {
        fmt: fmt.ok_or_else(|| WebAuthnError::CborDecode("missing 'fmt' in attestationObject".into()))?,
        auth_data: auth_data
            .ok_or_else(|| WebAuthnError::CborDecode("missing 'authData' in attestationObject".into()))?,
    })
}

/// Parse a COSE public key from CBOR bytes.
/// Currently supports EC2 (P-256, ES256) only.
pub(crate) fn parse_cose_key(data: &[u8]) -> Result<CoseKey, WebAuthnError> {
    let value: Value =
        ciborium::from_reader(data).map_err(|e| WebAuthnError::CborDecode(format!("COSE key CBOR: {e}")))?;

    let map = match value {
        Value::Map(m) => m,
        _ => return Err(WebAuthnError::InvalidCoseKey("not a CBOR map".into())),
    };

    // Extract integer-keyed fields
    let mut kty: Option<i64> = None;
    let mut alg: Option<i64> = None;
    let mut crv: Option<i64> = None;
    let mut x: Option<Vec<u8>> = None;
    let mut y: Option<Vec<u8>> = None;

    for (k, v) in &map {
        let key_int = cbor_to_i64(k);
        match key_int {
            Some(1) => kty = cbor_to_i64(v),
            Some(3) => alg = cbor_to_i64(v),
            Some(-1) => crv = cbor_to_i64(v),
            Some(-2) => x = cbor_to_bytes(v),
            Some(-3) => y = cbor_to_bytes(v),
            _ => {}
        }
    }

    let kty = kty.ok_or_else(|| WebAuthnError::InvalidCoseKey("missing kty (key 1)".into()))?;
    let alg = alg.ok_or_else(|| WebAuthnError::InvalidCoseKey("missing alg (key 3)".into()))?;

    match kty {
        2 => {
            // EC2 key
            if alg != -7 {
                return Err(WebAuthnError::UnsupportedAlgorithm(alg));
            }
            let crv = crv.ok_or_else(|| WebAuthnError::InvalidCoseKey("missing crv (key -1)".into()))?;
            if crv != 1 {
                // Only P-256 (crv=1) supported
                return Err(WebAuthnError::InvalidCoseKey(format!("unsupported curve: {crv}")));
            }
            let x = x.ok_or_else(|| WebAuthnError::InvalidCoseKey("missing x (key -2)".into()))?;
            let y = y.ok_or_else(|| WebAuthnError::InvalidCoseKey("missing y (key -3)".into()))?;
            if x.len() != 32 || y.len() != 32 {
                return Err(WebAuthnError::InvalidCoseKey(format!(
                    "EC2 P-256 key coordinates wrong length: x={}, y={}",
                    x.len(),
                    y.len()
                )));
            }
            Ok(CoseKey::EC2 { x, y })
        }
        _ => Err(WebAuthnError::UnsupportedKeyType(kty)),
    }
}

fn cbor_to_i64(v: &Value) -> Option<i64> {
    match v {
        Value::Integer(i) => {
            let val: i128 = (*i).into();
            i64::try_from(val).ok()
        }
        _ => None,
    }
}

fn cbor_to_bytes(v: &Value) -> Option<Vec<u8>> {
    match v {
        Value::Bytes(b) => Some(b.clone()),
        _ => None,
    }
}
