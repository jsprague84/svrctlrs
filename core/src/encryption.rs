//! AES-256-GCM encryption for credentials at rest
//!
//! Encryption key is loaded from:
//! 1. `ENCRYPTION_KEY` environment variable (64 hex chars = 32 bytes)
//! 2. `data/encryption.key` file (64 hex chars)
//! 3. If neither exists, a random key is generated and written to `data/encryption.key`

use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Nonce};
use rand::RngCore;
use std::path::Path;
use std::sync::OnceLock;
use tracing::{info, warn};

use crate::{Error, Result};

/// Global encryption key, initialized once at startup
static ENCRYPTION_KEY: OnceLock<[u8; 32]> = OnceLock::new();

/// Nonce size for AES-256-GCM (96 bits = 12 bytes)
const NONCE_SIZE: usize = 12;

/// Initialize the global encryption key.
///
/// Loads from `ENCRYPTION_KEY` env var (hex-encoded, 64 chars) or
/// `data/encryption.key` file. If neither exists, generates a random
/// key and writes it to the file with restrictive permissions.
pub fn init_encryption_key() -> Result<()> {
    let key = load_or_generate_key()?;
    ENCRYPTION_KEY
        .set(key)
        .map_err(|_| Error::ConfigError("Encryption key already initialized".to_string()))?;
    info!("Encryption key initialized successfully");
    Ok(())
}

/// Check if the encryption key has been initialized
pub fn is_initialized() -> bool {
    ENCRYPTION_KEY.get().is_some()
}

/// Encrypt plaintext using AES-256-GCM.
///
/// Returns base64-encoded string containing: nonce (12 bytes) || ciphertext+tag
pub fn encrypt(plaintext: &str) -> Result<String> {
    let key_bytes = ENCRYPTION_KEY
        .get()
        .ok_or_else(|| Error::ConfigError("Encryption key not initialized".to_string()))?;

    let cipher = Aes256Gcm::new(key_bytes.into());

    // Generate random nonce
    let mut nonce_bytes = [0u8; NONCE_SIZE];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    // Encrypt
    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| Error::Other(format!("Encryption failed: {e}")))?;

    // Combine nonce + ciphertext and encode as base64
    let mut combined = Vec::with_capacity(NONCE_SIZE + ciphertext.len());
    combined.extend_from_slice(&nonce_bytes);
    combined.extend_from_slice(&ciphertext);

    Ok(base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        &combined,
    ))
}

/// Decrypt a base64-encoded ciphertext produced by [`encrypt`].
///
/// Expects base64 string containing: nonce (12 bytes) || ciphertext+tag
pub fn decrypt(encoded: &str) -> Result<String> {
    let key_bytes = ENCRYPTION_KEY
        .get()
        .ok_or_else(|| Error::ConfigError("Encryption key not initialized".to_string()))?;

    let cipher = Aes256Gcm::new(key_bytes.into());

    // Decode base64
    let combined = base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        encoded,
    )
    .map_err(|e| Error::Other(format!("Base64 decode failed: {e}")))?;

    if combined.len() < NONCE_SIZE {
        return Err(Error::Other("Ciphertext too short".to_string()));
    }

    // Split nonce and ciphertext
    let (nonce_bytes, ciphertext) = combined.split_at(NONCE_SIZE);
    let nonce = Nonce::from_slice(nonce_bytes);

    // Decrypt
    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| Error::Other(format!("Decryption failed: {e}")))?;

    String::from_utf8(plaintext).map_err(|e| Error::Other(format!("Invalid UTF-8: {e}")))
}

/// Load encryption key from environment or file, or generate a new one.
fn load_or_generate_key() -> Result<[u8; 32]> {
    // 1. Try ENCRYPTION_KEY environment variable
    if let Ok(hex_key) = std::env::var("ENCRYPTION_KEY") {
        let key = parse_hex_key(&hex_key)?;
        info!("Encryption key loaded from ENCRYPTION_KEY environment variable");
        return Ok(key);
    }

    // 2. Try data/encryption.key file
    let key_path = Path::new("data/encryption.key");
    if key_path.exists() {
        let contents = std::fs::read_to_string(key_path)
            .map_err(|e| Error::ConfigError(format!("Failed to read encryption key file: {e}")))?;
        let key = parse_hex_key(contents.trim())?;
        info!("Encryption key loaded from {}", key_path.display());
        return Ok(key);
    }

    // 3. Generate a new key and write to file
    warn!(
        "ENCRYPTION_KEY not set and {} not found — generating random encryption key",
        key_path.display()
    );

    let mut key = [0u8; 32];
    OsRng.fill_bytes(&mut key);

    // Ensure data/ directory exists
    if let Some(parent) = key_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            Error::ConfigError(format!("Failed to create data directory: {e}"))
        })?;
    }

    // Write hex-encoded key
    let hex_key = hex::encode(key);
    std::fs::write(key_path, &hex_key).map_err(|e| {
        Error::ConfigError(format!("Failed to write encryption key file: {e}"))
    })?;

    // Set restrictive permissions (0600) on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(key_path, std::fs::Permissions::from_mode(0o600)).map_err(
            |e| Error::ConfigError(format!("Failed to set key file permissions: {e}")),
        )?;
    }

    warn!(
        "Generated encryption key written to {} — back up this file to avoid data loss",
        key_path.display()
    );

    Ok(key)
}

/// Parse a 64-character hex string into a 32-byte key.
fn parse_hex_key(hex_str: &str) -> Result<[u8; 32]> {
    let bytes = hex::decode(hex_str.trim())
        .map_err(|e| Error::ConfigError(format!("Invalid hex in encryption key: {e}")))?;

    if bytes.len() != 32 {
        return Err(Error::ConfigError(format!(
            "Encryption key must be 32 bytes (64 hex chars), got {} bytes",
            bytes.len()
        )));
    }

    let mut key = [0u8; 32];
    key.copy_from_slice(&bytes);
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to initialize key for tests (uses a fixed test key)
    fn init_test_key() {
        let test_key = [0x42u8; 32]; // Fixed test key
        let _ = ENCRYPTION_KEY.set(test_key); // Ignore if already set
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        init_test_key();

        let plaintext = "my-secret-ssh-key-content";
        let encrypted = encrypt(plaintext).unwrap();

        // Encrypted value should be different from plaintext
        assert_ne!(encrypted, plaintext);

        // Should be valid base64
        assert!(base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            &encrypted
        )
        .is_ok());

        let decrypted = decrypt(&encrypted).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_encrypt_produces_different_ciphertexts() {
        init_test_key();

        let plaintext = "same-plaintext";
        let enc1 = encrypt(plaintext).unwrap();
        let enc2 = encrypt(plaintext).unwrap();

        // Different nonces should produce different ciphertexts
        assert_ne!(enc1, enc2);

        // Both should decrypt to the same plaintext
        assert_eq!(decrypt(&enc1).unwrap(), plaintext);
        assert_eq!(decrypt(&enc2).unwrap(), plaintext);
    }

    #[test]
    fn test_decrypt_invalid_base64() {
        init_test_key();
        assert!(decrypt("not-valid-base64!!!").is_err());
    }

    #[test]
    fn test_decrypt_too_short() {
        init_test_key();
        // Valid base64 but too short (less than 12 bytes nonce)
        let short = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            &[0u8; 5],
        );
        assert!(decrypt(&short).is_err());
    }

    #[test]
    fn test_decrypt_tampered_ciphertext() {
        init_test_key();

        let encrypted = encrypt("secret").unwrap();
        let mut bytes = base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            &encrypted,
        )
        .unwrap();

        // Tamper with the ciphertext (flip a byte after the nonce)
        if bytes.len() > NONCE_SIZE {
            bytes[NONCE_SIZE] ^= 0xFF;
        }

        let tampered = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            &bytes,
        );
        assert!(decrypt(&tampered).is_err());
    }

    #[test]
    fn test_parse_hex_key_valid() {
        let hex = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let key = parse_hex_key(hex).unwrap();
        assert_eq!(key.len(), 32);
    }

    #[test]
    fn test_parse_hex_key_wrong_length() {
        let hex = "0123456789abcdef"; // Only 8 bytes
        assert!(parse_hex_key(hex).is_err());
    }

    #[test]
    fn test_parse_hex_key_invalid_hex() {
        let hex = "ZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZ";
        assert!(parse_hex_key(hex).is_err());
    }

    #[test]
    fn test_empty_string_encrypt_decrypt() {
        init_test_key();

        let plaintext = "";
        let encrypted = encrypt(plaintext).unwrap();
        let decrypted = decrypt(&encrypted).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_unicode_encrypt_decrypt() {
        init_test_key();

        let plaintext = "пароль 密码 🔑";
        let encrypted = encrypt(plaintext).unwrap();
        let decrypted = decrypt(&encrypted).unwrap();
        assert_eq!(decrypted, plaintext);
    }
}
