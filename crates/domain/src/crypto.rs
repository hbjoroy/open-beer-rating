use aes_gcm::{
    Aes256Gcm, Key, Nonce,
    aead::{Aead, KeyInit, OsRng, rand_core::RngCore},
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

use crate::errors::DomainError;

const NONCE_SIZE: usize = 12;

/// Encrypts plaintext using AES-256-GCM.
/// Returns bytes formatted as: [12-byte nonce][ciphertext+tag]
pub fn encrypt_field(plaintext: &str, key_bytes: &[u8; 32]) -> Result<Vec<u8>, DomainError> {
    let key = Key::<Aes256Gcm>::from_slice(key_bytes);
    let cipher = Aes256Gcm::new(key);

    let mut nonce_bytes = [0u8; NONCE_SIZE];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| DomainError::Encryption(format!("Encryption failed: {e}")))?;

    let mut result = Vec::with_capacity(NONCE_SIZE + ciphertext.len());
    result.extend_from_slice(&nonce_bytes);
    result.extend_from_slice(&ciphertext);
    Ok(result)
}

/// Decrypts data produced by `encrypt_field`.
/// Expects bytes formatted as: [12-byte nonce][ciphertext+tag]
pub fn decrypt_field(encrypted: &[u8], key_bytes: &[u8; 32]) -> Result<String, DomainError> {
    if encrypted.len() < NONCE_SIZE {
        return Err(DomainError::Encryption("Encrypted data too short".into()));
    }

    let key = Key::<Aes256Gcm>::from_slice(key_bytes);
    let cipher = Aes256Gcm::new(key);

    let (nonce_bytes, ciphertext) = encrypted.split_at(NONCE_SIZE);
    let nonce = Nonce::from_slice(nonce_bytes);

    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| DomainError::Encryption(format!("Decryption failed: {e}")))?;

    String::from_utf8(plaintext)
        .map_err(|e| DomainError::Encryption(format!("Invalid UTF-8 after decryption: {e}")))
}

/// Parses a base64-encoded encryption key into a 32-byte array.
pub fn parse_encryption_key(key_b64: &str) -> Result<[u8; 32], DomainError> {
    let decoded = BASE64
        .decode(key_b64.trim())
        .map_err(|e| DomainError::Encryption(format!("Invalid base64 key: {e}")))?;

    decoded
        .try_into()
        .map_err(|v: Vec<u8>| {
            DomainError::Encryption(format!(
                "Encryption key must be 32 bytes, got {}",
                v.len()
            ))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_key() -> [u8; 32] {
        let mut key = [0u8; 32];
        OsRng.fill_bytes(&mut key);
        key
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let key = test_key();
        let plaintext = "user@example.com";

        let encrypted = encrypt_field(plaintext, &key).unwrap();
        let decrypted = decrypt_field(&encrypted, &key).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_different_encryptions_produce_different_output() {
        let key = test_key();
        let plaintext = "same text";

        let enc1 = encrypt_field(plaintext, &key).unwrap();
        let enc2 = encrypt_field(plaintext, &key).unwrap();

        // Different nonces should produce different ciphertext
        assert_ne!(enc1, enc2);

        // But both decrypt to the same value
        assert_eq!(decrypt_field(&enc1, &key).unwrap(), plaintext);
        assert_eq!(decrypt_field(&enc2, &key).unwrap(), plaintext);
    }

    #[test]
    fn test_wrong_key_fails() {
        let key1 = test_key();
        let key2 = test_key();

        let encrypted = encrypt_field("secret", &key1).unwrap();
        assert!(decrypt_field(&encrypted, &key2).is_err());
    }

    #[test]
    fn test_empty_string() {
        let key = test_key();
        let encrypted = encrypt_field("", &key).unwrap();
        assert_eq!(decrypt_field(&encrypted, &key).unwrap(), "");
    }

    #[test]
    fn test_unicode() {
        let key = test_key();
        let plaintext = "Hefeweizen 🍺 Kölsch 🇩🇪";
        let encrypted = encrypt_field(plaintext, &key).unwrap();
        assert_eq!(decrypt_field(&encrypted, &key).unwrap(), plaintext);
    }

    #[test]
    fn test_parse_encryption_key() {
        let key = test_key();
        let b64 = BASE64.encode(key);
        let parsed = parse_encryption_key(&b64).unwrap();
        assert_eq!(parsed, key);
    }

    #[test]
    fn test_parse_encryption_key_wrong_size() {
        let b64 = BASE64.encode([0u8; 16]); // 16 bytes, not 32
        assert!(parse_encryption_key(&b64).is_err());
    }

    #[test]
    fn test_truncated_data_fails() {
        assert!(decrypt_field(&[0u8; 5], &test_key()).is_err());
    }
}
