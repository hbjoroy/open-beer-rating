//! Stored credential type and auth data parsing.

use serde::{Deserialize, Serialize};

/// Opaque credential ID bytes.
pub type CredentialId = Vec<u8>;

/// A stored WebAuthn credential — persisted in the database per user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredCredential {
    /// Raw credential ID bytes
    pub credential_id: CredentialId,
    /// COSE public key (parsed representation)
    pub public_key: CoseKey,
    /// Sign counter (monotonically increasing, or 0 for passkeys)
    pub counter: u32,
    /// Transports hint (e.g. "internal", "hybrid", "usb")
    pub transports: Option<Vec<String>>,
    /// Whether user verification was performed at registration
    pub user_verified: bool,
    /// Backup eligible flag from registration
    pub backup_eligible: bool,
    /// Current backup state
    pub backup_state: bool,
}

/// Parsed COSE public key
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CoseKey {
    /// P-256 ECDSA (ES256, alg=-7)
    EC2 {
        /// X coordinate, 32 bytes
        x: Vec<u8>,
        /// Y coordinate, 32 bytes
        y: Vec<u8>,
    },
}

/// Parsed authenticator data
#[derive(Debug)]
pub(crate) struct AuthData {
    pub rp_id_hash: [u8; 32],
    pub flags: AuthDataFlags,
    pub counter: u32,
    pub attested_credential_data: Option<AttestedCredentialData>,
}

#[derive(Debug)]
pub(crate) struct AuthDataFlags {
    pub user_present: bool,
    pub user_verified: bool,
    pub backup_eligible: bool,
    pub backup_state: bool,
    pub attested_data_present: bool,
    pub extension_data_present: bool,
}

impl AuthDataFlags {
    pub fn from_byte(b: u8) -> Self {
        Self {
            user_present: b & 0x01 != 0,
            user_verified: b & 0x04 != 0,
            backup_eligible: b & 0x08 != 0,
            backup_state: b & 0x10 != 0,
            attested_data_present: b & 0x40 != 0,
            extension_data_present: b & 0x80 != 0,
        }
    }
}

#[derive(Debug)]
pub(crate) struct AttestedCredentialData {
    pub aaguid: [u8; 16],
    pub credential_id: Vec<u8>,
    pub public_key: CoseKey,
}

/// Parse the binary authData per W3C §6.1
pub(crate) fn parse_auth_data(data: &[u8]) -> Result<AuthData, crate::error::WebAuthnError> {
    use crate::error::WebAuthnError;

    if data.len() < 37 {
        return Err(WebAuthnError::AuthDataTooShort { expected: 37, got: data.len() });
    }

    let mut rp_id_hash = [0u8; 32];
    rp_id_hash.copy_from_slice(&data[0..32]);

    let flags = AuthDataFlags::from_byte(data[32]);

    let counter = u32::from_be_bytes([data[33], data[34], data[35], data[36]]);

    let attested_credential_data = if flags.attested_data_present {
        Some(parse_attested_credential_data(&data[37..])?)
    } else {
        None
    };

    Ok(AuthData {
        rp_id_hash,
        flags,
        counter,
        attested_credential_data,
    })
}

fn parse_attested_credential_data(data: &[u8]) -> Result<AttestedCredentialData, crate::error::WebAuthnError> {
    use crate::error::WebAuthnError;

    if data.len() < 18 {
        return Err(WebAuthnError::AuthDataTooShort { expected: 18, got: data.len() });
    }

    let mut aaguid = [0u8; 16];
    aaguid.copy_from_slice(&data[0..16]);

    let cred_id_len = u16::from_be_bytes([data[16], data[17]]) as usize;
    if cred_id_len > 1023 {
        return Err(WebAuthnError::CredentialIdTooLong);
    }

    let cred_id_end = 18 + cred_id_len;
    if data.len() < cred_id_end {
        return Err(WebAuthnError::AuthDataTooShort {
            expected: cred_id_end,
            got: data.len(),
        });
    }

    let credential_id = data[18..cred_id_end].to_vec();
    let public_key = crate::cbor::parse_cose_key(&data[cred_id_end..])?;

    Ok(AttestedCredentialData {
        aaguid,
        credential_id,
        public_key,
    })
}
