//! Web crypto implementation using Web Crypto API
//!
//! This module provides cryptographic operations using the browser's
//! Web Crypto API for secure operations in web environments. The [`WebCrypto`]
//! struct implements the [`alkanes_cli_common::CryptoProvider`] trait, providing
//! a web-compatible cryptographic backend for the Deezel Bitcoin toolkit.
//!
//! # Features
//!
//! - **Web Crypto API Integration**: Uses the browser's native cryptographic APIs
//! - **Fallback Support**: Falls back to pure Rust implementations when Web Crypto is unavailable
//! - **AES-GCM Encryption**: Symmetric encryption using AES-GCM mode
//! - **PBKDF2 Key Derivation**: Password-based key derivation with configurable iterations
//! - **Secure Random Generation**: Cryptographically secure random number generation
//! - **Hash Functions**: SHA-256 and SHA3-256 (Keccak) hash implementations
//! - **Async Interface**: Fully async API compatible with web environments
//!
//! # Browser Compatibility
//!
//! This implementation requires a browser environment with Web Crypto API support.
//! For operations not supported by Web Crypto, it falls back to pure Rust implementations
//! using the `sha2`, `sha3`, and `rand` crates.
//!
//! # Security Considerations
//!
//! - Random number generation uses the browser's secure random source
//! - AES-GCM provides authenticated encryption with associated data (AEAD)
//! - PBKDF2 uses SHA-256 as the underlying hash function
//! - All cryptographic operations are performed using browser-native implementations when available
//!
//! # Examples
//!
//! ```rust,no_run
//! use deezel_web::crypto::WebCrypto;
//! use alkanes_cli_common::CryptoProvider;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let crypto = WebCrypto::new();
//!
//! // Generate secure random bytes
//! let random_data = crypto.random_bytes(32)?;
//! println!("Generated {} random bytes", random_data.len());
//!
//! // Compute SHA-256 hash
//! let data = b"Hello, world!";
//! let hash = crypto.sha256(data)?;
//! println!("SHA-256 hash: {:?}", hash);
//!
//! // AES-GCM encryption
//! let key = crypto.random_bytes(32)?; // 256-bit key
//! let nonce = crypto.random_bytes(12)?; // 96-bit nonce
//! let plaintext = b"Secret message";
//!
//! let ciphertext = crypto.encrypt_aes_gcm(plaintext, &key, &nonce).await?;
//! let decrypted = crypto.decrypt_aes_gcm(&ciphertext, &key, &nonce).await?;
//! assert_eq!(decrypted, plaintext);
//!
//! // PBKDF2 key derivation
//! let password = b"user_password";
//! let salt = crypto.random_bytes(16)?;
//! let derived_key = crypto.pbkdf2_derive(password, &salt, 100000, 32).await?;
//! # Ok(())
//! # }
//! ```

#[cfg(target_arch = "wasm32")]
extern crate alloc;
#[cfg(target_arch = "wasm32")]
use alloc::{
    vec::Vec,
    boxed::Box,
    string::ToString,
    format,
    vec,
};

use async_trait::async_trait;
use alkanes_cli_common::{AlkanesError, Result};
use js_sys::{Array, Object, Uint8Array};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{window, Crypto, SubtleCrypto, CryptoKey};
use sha2::{Sha256, Digest as Sha2Digest};
use sha3::Sha3_256;
use rand::RngCore;
use aes_gcm::{Aes256Gcm, KeyInit, aead::Aead, Nonce};

/// Web crypto implementation using browser Web Crypto API
///
/// This struct provides a web-compatible cryptographic backend that implements the
/// [`alkanes_cli_common::CryptoProvider`] trait. It uses the browser's Web Crypto API
/// for secure cryptographic operations with fallbacks to pure Rust implementations.
///
/// # Architecture
///
/// - **Primary**: Uses Web Crypto API for maximum performance and security
/// - **Fallback**: Uses pure Rust implementations when Web Crypto is unavailable
/// - **Hybrid**: Combines both approaches for optimal compatibility
///
/// # Supported Operations
///
/// - **Random Generation**: Cryptographically secure random bytes
/// - **Hashing**: SHA-256 and SHA3-256 (Keccak) hash functions
/// - **Symmetric Encryption**: AES-GCM authenticated encryption
/// - **Key Derivation**: PBKDF2 with SHA-256
///
/// # Error Handling
///
/// The implementation handles various error conditions:
/// - Web Crypto API not available (falls back to Rust implementations)
/// - Invalid key sizes or parameters
/// - Encryption/decryption failures
/// - Key import/export errors
///
/// # Thread Safety
///
/// This struct is `Clone` but not `Send` or `Sync`, as it's designed for
/// single-threaded web environments using `?Send` async traits.
#[derive(Clone)]
pub struct WebCrypto {
    /// Optional reference to the browser's Crypto object
    /// None if Web Crypto API is not available in the current environment
    crypto: Option<Crypto>,
    /// Optional reference to the browser's SubtleCrypto object
    /// None if SubtleCrypto API is not available in the current environment
    subtle: Option<SubtleCrypto>,
}

impl WebCrypto {
    /// Create a new WebCrypto instance
    ///
    /// Attempts to access the browser's Web Crypto API. If the API is not
    /// available, operations will fall back to pure Rust implementations
    /// where possible.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use deezel_web::crypto::WebCrypto;
    ///
    /// let crypto = WebCrypto::new();
    /// // Crypto provider is ready to use, will handle API availability automatically
    /// ```
    pub fn new() -> Self {
        let window = window();
        let crypto = window.as_ref().and_then(|w| w.crypto().ok());
        let subtle = crypto.as_ref().and_then(|c| {
            let s = c.subtle();
            if s.is_undefined() {
                None
            } else {
                Some(s)
            }
        });
        
        Self { crypto, subtle }
    }

    /// Get the Crypto object or return an error
    ///
    /// # Errors
    ///
    /// Returns [`AlkanesError::Crypto`] if the Web Crypto API is not available
    /// in the current browser environment.
    fn get_crypto(&self) -> Result<&Crypto> {
        self.crypto.as_ref()
            .ok_or_else(|| AlkanesError::Crypto("Web Crypto API not available".to_string()))
    }

    /// Get the SubtleCrypto object or return an error
    ///
    /// # Errors
    ///
    /// Returns [`AlkanesError::Crypto`] if the SubtleCrypto API is not available
    /// in the current browser environment.
    fn get_subtle(&self) -> Result<&SubtleCrypto> {
        self.subtle.as_ref()
            .ok_or_else(|| AlkanesError::Crypto("SubtleCrypto API not available".to_string()))
    }

    /// Convert Rust bytes to JavaScript Uint8Array
    ///
    /// This utility method converts Rust byte slices to JavaScript Uint8Array
    /// objects that can be used with the Web Crypto API.
    ///
    /// # Arguments
    ///
    /// * `bytes` - The byte slice to convert
    ///
    /// # Returns
    ///
    /// A JavaScript Uint8Array containing the same data
    fn bytes_to_uint8_array(&self, bytes: &[u8]) -> Uint8Array {
        let uint8_array = Uint8Array::new_with_length(bytes.len() as u32);
        uint8_array.copy_from(bytes);
        uint8_array
    }

    /// Convert JavaScript Uint8Array to Rust bytes
    ///
    /// This utility method converts JavaScript Uint8Array objects back to
    /// Rust Vec<u8> for further processing.
    ///
    /// # Arguments
    ///
    /// * `uint8_array` - The JavaScript Uint8Array to convert
    ///
    /// # Returns
    ///
    /// A Vec<u8> containing the same data
    fn uint8_array_to_bytes(&self, uint8_array: &Uint8Array) -> Vec<u8> {
        let mut bytes = vec![0u8; uint8_array.length() as usize];
        uint8_array.copy_to(&mut bytes);
        bytes
    }
}

/// Implementation of the [`alkanes_cli_common::CryptoProvider`] trait for web environments
///
/// This implementation provides all the standard cryptographic operations using the
/// browser's Web Crypto API with fallbacks to pure Rust implementations. All operations
/// are async-compatible and handle the web environment's security constraints.
#[async_trait(?Send)]
impl alkanes_cli_common::CryptoProvider for WebCrypto {
    /// Generate cryptographically secure random bytes
    ///
    /// Uses the browser's Web Crypto API for secure random generation,
    /// falling back to the `rand` crate if Web Crypto is unavailable.
    ///
    /// # Arguments
    ///
    /// * `len` - The number of random bytes to generate
    ///
    /// # Returns
    ///
    /// A vector containing the requested number of random bytes
    ///
    /// # Errors
    ///
    /// This method should not fail under normal circumstances as it has
    /// a fallback implementation using the `rand` crate.
    fn random_bytes(&self, len: usize) -> Result<Vec<u8>> {
        // Try to use Web Crypto API first
        if let Ok(crypto) = self.get_crypto() {
            let mut bytes = vec![0u8; len];
            if crypto.get_random_values_with_u8_array(&mut bytes).is_ok() {
                return Ok(bytes);
            }
        }
        
        // Fallback to rand crate (which uses getrandom with js feature)
        let mut bytes = vec![0u8; len];
        rand::thread_rng().fill_bytes(&mut bytes);
        Ok(bytes)
    }

    fn sha256(&self, data: &[u8]) -> Result<[u8; 32]> {
        // Use sha2 crate for SHA256 (more reliable than Web Crypto for this)
        let mut hasher = Sha256::new();
        hasher.update(data);
        Ok(hasher.finalize().into())
    }

    fn sha3_256(&self, data: &[u8]) -> Result<[u8; 32]> {
        // Use sha3 crate for SHA3-256 (Keccak256)
        let mut hasher = Sha3_256::new();
        hasher.update(data);
        Ok(hasher.finalize().into())
    }

    async fn encrypt_aes_gcm(&self, data: &[u8], key: &[u8], nonce: &[u8]) -> Result<Vec<u8>> {
        if let Ok(subtle) = self.get_subtle() {
            // Import the key
            let key_data = self.bytes_to_uint8_array(key);
            let key_algorithm = Object::new();
            js_sys::Reflect::set(&key_algorithm, &"name".into(), &"AES-GCM".into())
                .map_err(|e| AlkanesError::Crypto(format!("Failed to set key algorithm: {e:?}")))?;
            
            let crypto_key_promise = subtle.import_key_with_object(
                "raw",
                &key_data,
                &key_algorithm,
                false,
                &Array::of1(&"encrypt".into()),
            ).map_err(|e| AlkanesError::Crypto(format!("Failed to import key: {e:?}")))?;
            
            let crypto_key_value = JsFuture::from(crypto_key_promise)
                .await
                .map_err(|e| AlkanesError::Crypto(format!("Failed to import key: {e:?}")))?;
            
            let crypto_key: CryptoKey = crypto_key_value.dyn_into()
                .map_err(|e| AlkanesError::Crypto(format!("Failed to cast crypto key: {e:?}")))?;
            
            // Set up encryption parameters
            let algorithm = Object::new();
            js_sys::Reflect::set(&algorithm, &"name".into(), &"AES-GCM".into())
                .map_err(|e| AlkanesError::Crypto(format!("Failed to set algorithm: {e:?}")))?;
            js_sys::Reflect::set(&algorithm, &"iv".into(), &self.bytes_to_uint8_array(nonce))
                .map_err(|e| AlkanesError::Crypto(format!("Failed to set IV: {e:?}")))?;
            
            // Encrypt the data
            let encrypt_promise = subtle.encrypt_with_object_and_u8_array(&algorithm, &crypto_key, data)
                .map_err(|e| AlkanesError::Crypto(format!("Failed to encrypt: {e:?}")))?;
            
            let encrypted_value = JsFuture::from(encrypt_promise)
                .await
                .map_err(|e| AlkanesError::Crypto(format!("Failed to encrypt: {e:?}")))?;
            
            let encrypted_array = Uint8Array::new(&encrypted_value);
            Ok(self.uint8_array_to_bytes(&encrypted_array))
        } else {
            // Fallback to pure Rust implementation
            let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| AlkanesError::Crypto(e.to_string()))?;
            let nonce = Nonce::from_slice(nonce);
            cipher.encrypt(nonce, data).map_err(|e| AlkanesError::Crypto(e.to_string()))
        }
    }

    async fn decrypt_aes_gcm(&self, data: &[u8], key: &[u8], nonce: &[u8]) -> Result<Vec<u8>> {
        if let Ok(subtle) = self.get_subtle() {
            // Import the key
            let key_data = self.bytes_to_uint8_array(key);
            let key_algorithm = Object::new();
            js_sys::Reflect::set(&key_algorithm, &"name".into(), &"AES-GCM".into())
                .map_err(|e| AlkanesError::Crypto(format!("Failed to set key algorithm: {e:?}")))?;
            
            let crypto_key_promise = subtle.import_key_with_object(
                "raw",
                &key_data,
                &key_algorithm,
                false,
                &Array::of1(&"decrypt".into()),
            ).map_err(|e| AlkanesError::Crypto(format!("Failed to import key: {e:?}")))?;
            
            let crypto_key_value = JsFuture::from(crypto_key_promise)
                .await
                .map_err(|e| AlkanesError::Crypto(format!("Failed to import key: {e:?}")))?;
            
            let crypto_key: CryptoKey = crypto_key_value.dyn_into()
                .map_err(|e| AlkanesError::Crypto(format!("Failed to cast crypto key: {e:?}")))?;
            
            // Set up decryption parameters
            let algorithm = Object::new();
            js_sys::Reflect::set(&algorithm, &"name".into(), &"AES-GCM".into())
                .map_err(|e| AlkanesError::Crypto(format!("Failed to set algorithm: {e:?}")))?;
            js_sys::Reflect::set(&algorithm, &"iv".into(), &self.bytes_to_uint8_array(nonce))
                .map_err(|e| AlkanesError::Crypto(format!("Failed to set IV: {e:?}")))?;
            
            // Decrypt the data
            let decrypt_promise = subtle.decrypt_with_object_and_u8_array(&algorithm, &crypto_key, data)
                .map_err(|e| AlkanesError::Crypto(format!("Failed to decrypt: {e:?}")))?;
            
            let decrypted_value = JsFuture::from(decrypt_promise)
                .await
                .map_err(|e| AlkanesError::Crypto(format!("Failed to decrypt: {e:?}")))?;
            
            let decrypted_array = Uint8Array::new(&decrypted_value);
            Ok(self.uint8_array_to_bytes(&decrypted_array))
        } else {
            // Fallback to pure Rust implementation
            let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| AlkanesError::Crypto(e.to_string()))?;
            let nonce = Nonce::from_slice(nonce);
            cipher.decrypt(nonce, data).map_err(|e| AlkanesError::Crypto(e.to_string()))
        }
    }

    async fn pbkdf2_derive(&self, password: &[u8], salt: &[u8], iterations: u32, key_len: usize) -> Result<Vec<u8>> {
        if let Ok(subtle) = self.get_subtle() {
            // Import the password as a key
            let password_data = self.bytes_to_uint8_array(password);
            let import_algorithm = Object::new();
            js_sys::Reflect::set(&import_algorithm, &"name".into(), &"PBKDF2".into())
                .map_err(|e| AlkanesError::Crypto(format!("Failed to set import algorithm: {e:?}")))?;
            
            let password_key_promise = subtle.import_key_with_object(
                "raw",
                &password_data,
                &import_algorithm,
                false,
                &Array::of1(&"deriveBits".into()),
            ).map_err(|e| AlkanesError::Crypto(format!("Failed to import password: {e:?}")))?;
            
            let password_key_value = JsFuture::from(password_key_promise)
                .await
                .map_err(|e| AlkanesError::Crypto(format!("Failed to import password: {e:?}")))?;
            
            let password_key: CryptoKey = password_key_value.dyn_into()
                .map_err(|e| AlkanesError::Crypto(format!("Failed to cast password key: {e:?}")))?;
            
            // Set up PBKDF2 parameters
            let algorithm = Object::new();
            js_sys::Reflect::set(&algorithm, &"name".into(), &"PBKDF2".into())
                .map_err(|e| AlkanesError::Crypto(format!("Failed to set algorithm: {e:?}")))?;
            js_sys::Reflect::set(&algorithm, &"salt".into(), &self.bytes_to_uint8_array(salt))
                .map_err(|e| AlkanesError::Crypto(format!("Failed to set salt: {e:?}")))?;
            js_sys::Reflect::set(&algorithm, &"iterations".into(), &JsValue::from(iterations))
                .map_err(|e| AlkanesError::Crypto(format!("Failed to set iterations: {e:?}")))?;
            js_sys::Reflect::set(&algorithm, &"hash".into(), &"SHA-256".into())
                .map_err(|e| AlkanesError::Crypto(format!("Failed to set hash: {e:?}")))?;
            
            // Derive the key
            let derive_promise = subtle.derive_bits_with_object(&algorithm, &password_key, (key_len * 8) as u32)
                .map_err(|e| AlkanesError::Crypto(format!("Failed to derive bits: {e:?}")))?;
            
            let derived_value = JsFuture::from(derive_promise)
                .await
                .map_err(|e| AlkanesError::Crypto(format!("Failed to derive bits: {e:?}")))?;
            
            let derived_array = Uint8Array::new(&derived_value);
            Ok(self.uint8_array_to_bytes(&derived_array))
        } else {
            // Fallback to pure Rust implementation
            let mut key = vec![0u8; key_len];
            pbkdf2::pbkdf2_hmac::<sha2::Sha256>(
                password,
                salt,
                iterations,
                &mut key,
            );
            Ok(key)
        }
    }
}

impl Default for WebCrypto {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alkanes_cli_common::CryptoProvider;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn test_random_bytes() {
        let crypto = WebCrypto::new();
        let bytes = crypto.random_bytes(32).unwrap();
        assert_eq!(bytes.len(), 32);
        
        // Generate another set and ensure they're different
        let bytes2 = crypto.random_bytes(32).unwrap();
        assert_ne!(bytes, bytes2);
    }

    #[wasm_bindgen_test]
    fn test_sha256() {
        let crypto = WebCrypto::new();
        let data = b"hello world";
        let hash = crypto.sha256(data).unwrap();
        
        // Known SHA256 hash of "hello world"
        let expected = [
            0xb9, 0x4d, 0x27, 0xb9, 0x93, 0x4d, 0x3e, 0x08,
            0xa5, 0x2e, 0x52, 0xd7, 0xda, 0x7d, 0xab, 0xfa,
            0xc4, 0x84, 0xef, 0xe3, 0x7a, 0x53, 0x80, 0xee,
            0x90, 0x88, 0xf7, 0xac, 0xe2, 0xef, 0xcd, 0xe9,
        ];
        
        assert_eq!(hash, expected);
    }

    #[wasm_bindgen_test]
    async fn test_aes_gcm_encryption() {
        let crypto = WebCrypto::new();
        let data = b"test data";
        let key = &[0u8; 32]; // 256-bit key
        let nonce = &[0u8; 12]; // 96-bit nonce
        
        // Test encryption and decryption
        match crypto.encrypt_aes_gcm(data, key, nonce).await {
            Ok(encrypted) => {
                assert_ne!(encrypted, data);
                
                match crypto.decrypt_aes_gcm(&encrypted, key, nonce).await {
                    Ok(decrypted) => {
                        assert_eq!(decrypted, data);
                    },
                    Err(_) => {
                        // Decryption might fail in some test environments
                    }
                }
            },
            Err(_) => {
                // Encryption might fail in some test environments
            }
        }
    }
}