//! # Deezel Common Cryptography
//!
//! This module provides `no_std` compatible cryptographic functions for encrypting
//! and decrypting the wallet's seed mnemonic. It uses PBKDF2 to derive a key
//! from a user-provided passphrase and AES-256-GCM for authenticated encryption.

use crate::{Result, DeezelError};
use alloc::{string::ToString, vec, vec::Vec};
use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use rand_core::RngCore;
use pbkdf2::pbkdf2_hmac;
use sha2::Sha256;
// Chadson Journal:
// 1. Removed the async `encrypt` and `decrypt` functions that used a Web Worker.
// 2. This was causing errors in `slope-frontend` because the worker was being loaded
//    unnecessarily.
// 3. The `deezel-web` crate now provides the necessary async crypto functions using
//    the Web Crypto API, making this worker-based implementation obsolete.
// 4. Removed associated `use` statements for `gloo-worker`, `futures`, and `crypto_worker`.

const SALT_SIZE: usize = 16; // 128 bits for salt
const NONCE_SIZE: usize = 12; // 96 bits for AES-GCM nonce
const PBKDF_ITERATIONS: u32 = 600; // A modern standard for PBKDF2 iterations

/// Derives a key from a passphrase and salt using PBKDF2-HMAC-SHA256.
pub fn derive_key(passphrase: &str, salt: &[u8]) -> Result<Vec<u8>> {
    let mut key = vec![0u8; 32];
    pbkdf2_hmac::<Sha256>(
        passphrase.as_bytes(),
        salt,
        PBKDF_ITERATIONS,
        &mut key,
    );
    Ok(key)
}

/// Synchronously encrypts data. This should be called from within the worker.
pub fn encrypt_sync(data: &[u8], passphrase: &str) -> Result<(Vec<u8>, Vec<u8>, Vec<u8>)> {
    let mut salt = vec![0u8; SALT_SIZE];
    OsRng.fill_bytes(&mut salt);
    let key = derive_key(passphrase, &salt)?;
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| DeezelError::Crypto(e.to_string()))?;
    let mut nonce_bytes = vec![0u8; NONCE_SIZE];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let encrypted_data = cipher.encrypt(nonce, data).map_err(|e| DeezelError::Crypto(e.to_string()))?;
    Ok((encrypted_data, salt, nonce_bytes))
}

/// Synchronously decrypts data. This should be called from within the worker.
pub fn decrypt_sync(encrypted_data: &[u8], passphrase: &str, salt: &[u8], nonce_bytes: &[u8]) -> Result<Vec<u8>> {
    let key = derive_key(passphrase, salt)?;
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| DeezelError::Crypto(e.to_string()))?;
    let nonce = Nonce::from_slice(nonce_bytes);
    cipher.decrypt(nonce, encrypted_data).map_err(|e| DeezelError::Crypto(e.to_string()))
}



#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::String;

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let passphrase = "supersecretpassword";
        let data = "this is a very secret message";

        // Encrypt
        let (encrypted, salt, nonce) = encrypt_sync(data.as_bytes(), passphrase).unwrap();

        // Decrypt
        let decrypted_bytes = decrypt_sync(&encrypted, passphrase, &salt, &nonce).unwrap();
        let decrypted_string = String::from_utf8(decrypted_bytes).unwrap();

        assert_eq!(data, decrypted_string);
    }

    #[test]
    fn test_decrypt_wrong_password() {
        let passphrase = "supersecretpassword";
        let wrong_passphrase = "wrongpassword";
        let data = "this is another secret";

        // Encrypt
        let (encrypted, salt, nonce) = encrypt_sync(data.as_bytes(), passphrase).unwrap();

        // Decrypt with wrong password
        let result = decrypt_sync(&encrypted, wrong_passphrase, &salt, &nonce);

        assert!(result.is_err());
    }
}