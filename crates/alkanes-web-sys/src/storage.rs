//! Web storage implementation using localStorage
//!
//! This module provides a storage implementation that uses the browser's
//! localStorage API for persistent data storage in web environments.
//!
//! The [`WebStorage`] struct implements the [`alkanes_cli_common::StorageProvider`] trait,
//! providing a web-compatible storage backend for the Deezel Bitcoin toolkit.
//! All data is automatically base64-encoded for safe storage in localStorage
//! and namespaced with a "alkanes:" prefix to avoid conflicts.
//!
//! # Features
//!
//! - **Persistent Storage**: Uses browser localStorage for data persistence across sessions
//! - **Base64 Encoding**: Automatically encodes binary data for safe text storage
//! - **Namespacing**: All keys are prefixed with "alkanes:" to avoid conflicts
//! - **Error Handling**: Comprehensive error handling for storage operations
//! - **Async Interface**: Fully async API compatible with web environments
//!
//! # Browser Compatibility
//!
//! This implementation requires a browser environment with localStorage support.
//! It will gracefully handle cases where localStorage is not available.
//!
//! # Examples
//!
//! ```rust,no_run
//! use deezel_web::storage::WebStorage;
//! use alkanes_cli_common::StorageProvider;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let storage = WebStorage::new();
//!
//! // Store some data
//! let data = b"Hello, world!";
//! storage.write("greeting", data).await?;
//!
//! // Read it back
//! let retrieved = storage.read("greeting").await?;
//! assert_eq!(retrieved, data);
//!
//! // Check if key exists
//! assert!(storage.exists("greeting").await?);
//!
//! // List keys with prefix
//! let keys = storage.list_keys("greet").await?;
//! assert!(keys.contains(&"greeting".to_string()));
//!
//! // Clean up
//! storage.delete("greeting").await?;
//! # Ok(())
//! # }
//! ```

use async_trait::async_trait;
use alkanes_cli_common::{AlkanesError, Result};
use crate::platform::{self, PlatformStorage};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
#[cfg(target_arch = "wasm32")]
extern crate alloc;
#[cfg(target_arch = "wasm32")]
use alloc::{
    boxed::Box,
    format,
    string::{String, ToString},
    vec::Vec,
};

/// Web storage implementation using platform-agnostic storage
///
/// This struct provides a storage backend that implements the
/// [`alkanes_cli_common::StorageProvider`] trait. It works in both browser
/// (using localStorage) and Node.js (using in-memory storage) environments.
///
/// # Storage Format
///
/// - All data is base64-encoded before storage to handle binary data safely
/// - Keys are prefixed with "alkanes:" to avoid conflicts with other applications
/// - Storage operations are async-compatible for web environments
///
/// # Error Handling
///
/// The implementation handles various error conditions:
/// - localStorage not available (e.g., in private browsing mode)
/// - Storage quota exceeded
/// - Invalid base64 data during decoding
/// - Network or browser-specific storage errors
///
/// # Thread Safety
///
/// This struct is `Clone` but not `Send` or `Sync`, as it's designed for
/// single-threaded web environments using `?Send` async traits.
#[derive(Clone)]
pub struct WebStorage {
    /// Platform-agnostic storage backend
    storage: PlatformStorage,
}

impl WebStorage {
    /// Create a new WebStorage instance
    ///
    /// Creates a storage backend that works in both browser and Node.js environments.
    /// In browser, it uses localStorage. In Node.js, it uses in-memory storage.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use deezel_web::storage::WebStorage;
    ///
    /// let storage = WebStorage::new();
    /// // Storage is ready to use in any environment
    /// ```
    pub fn new() -> Self {
        Self {
            storage: PlatformStorage::new()
        }
    }

    /// Encode binary data as base64 for safe text storage
    ///
    /// Uses standard base64 encoding to convert binary data into a text
    /// format suitable for storage, which only supports string values.
    ///
    /// # Arguments
    ///
    /// * `data` - The binary data to encode
    ///
    /// # Returns
    ///
    /// A base64-encoded string representation of the input data
    fn encode_data(&self, data: &[u8]) -> String {
        BASE64.encode(data)
    }

    /// Decode base64 data back to binary format
    ///
    /// Converts base64-encoded strings back to their original binary format.
    ///
    /// # Arguments
    ///
    /// * `encoded` - The base64-encoded string to decode
    ///
    /// # Returns
    ///
    /// The decoded binary data
    ///
    /// # Errors
    ///
    /// Returns [`AlkanesError::Storage`] if the input is not valid base64
    fn decode_data(&self, encoded: &str) -> Result<Vec<u8>> {
        BASE64.decode(encoded)
            .map_err(|e| AlkanesError::Storage(format!("Failed to decode base64 data: {e}")))
    }
}

/// Implementation of the [`alkanes_cli_common::StorageProvider`] trait for web environments
///
/// This implementation provides all the standard storage operations using a
/// platform-agnostic storage backend. All operations are async-compatible and handle
/// different environments (browser and Node.js).
#[async_trait(?Send)]
impl alkanes_cli_common::StorageProvider for WebStorage {
    /// Read data from storage by key
    ///
    /// Retrieves the value associated with the given key, automatically
    /// decoding it from base64 format back to binary data.
    ///
    /// # Arguments
    ///
    /// * `key` - The key to read (will be automatically prefixed with "alkanes:")
    ///
    /// # Returns
    ///
    /// The binary data associated with the key
    ///
    /// # Errors
    ///
    /// * [`AlkanesError::Storage`] if the key is not found
    /// * [`AlkanesError::Storage`] if the stored data is not valid base64
    async fn read(&self, key: &str) -> Result<Vec<u8>> {
        let value = self.storage.get(key)
            .ok_or_else(|| AlkanesError::Storage(format!("Key not found: {key}")))?;

        self.decode_data(&value)
    }

    /// Write data to storage
    ///
    /// Stores the given binary data under the specified key, automatically
    /// encoding it as base64 for safe text storage.
    ///
    /// # Arguments
    ///
    /// * `key` - The key to store under (will be automatically prefixed with "alkanes:")
    /// * `data` - The binary data to store
    ///
    /// # Errors
    ///
    /// * [`AlkanesError::Storage`] if storage is full (quota exceeded)
    /// * [`AlkanesError::Storage`] if storage access fails
    async fn write(&self, key: &str, data: &[u8]) -> Result<()> {
        let encoded_data = self.encode_data(data);
        self.storage.set(key, &encoded_data)
    }

    /// Check if a key exists in storage
    ///
    /// Determines whether the specified key has an associated value in storage.
    ///
    /// # Arguments
    ///
    /// * `key` - The key to check (will be automatically prefixed with "alkanes:")
    ///
    /// # Returns
    ///
    /// `true` if the key exists, `false` otherwise
    async fn exists(&self, key: &str) -> Result<bool> {
        Ok(self.storage.exists(key))
    }

    /// Delete a key from storage
    ///
    /// Removes the specified key and its associated value from storage.
    ///
    /// # Arguments
    ///
    /// * `key` - The key to delete (will be automatically prefixed with "alkanes:")
    ///
    /// # Note
    ///
    /// This operation succeeds even if the key doesn't exist.
    async fn delete(&self, key: &str) -> Result<()> {
        self.storage.remove(key)
    }

    /// List all keys matching a prefix
    ///
    /// Returns all keys in storage that start with the given prefix.
    /// The returned keys have the "alkanes:" namespace prefix removed.
    ///
    /// # Arguments
    ///
    /// * `prefix` - The prefix to match against
    ///
    /// # Returns
    ///
    /// A vector of keys (without the "alkanes:" prefix) that match the prefix
    async fn list_keys(&self, prefix: &str) -> Result<Vec<String>> {
        Ok(self.storage.list_keys(prefix))
    }

    /// Get the storage type identifier
    ///
    /// Returns a string identifier for this storage backend type.
    ///
    /// # Returns
    ///
    /// "localStorage" in browser, "memory" in Node.js
    fn storage_type(&self) -> &'static str {
        if platform::is_browser() {
            "localStorage"
        } else {
            "memory"
        }
    }
}

impl Default for WebStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alkanes_cli_common::StorageProvider;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    async fn test_storage_operations() {
        let storage = WebStorage::new();
        let test_key = "test_key";
        let test_data = b"test data";

        // Test write
        assert!(storage.write(test_key, test_data).await.is_ok());

        // Test exists
        assert!(storage.exists(test_key).await.unwrap());

        // Test read
        let read_data = storage.read(test_key).await.unwrap();
        assert_eq!(read_data, test_data);

        // Test delete
        assert!(storage.delete(test_key).await.is_ok());
        assert!(!storage.exists(test_key).await.unwrap());
    }

    #[wasm_bindgen_test]
    async fn test_list_keys() {
        let storage = WebStorage::new();
        
        // Write some test data
        storage.write("prefix:key1", b"data1").await.unwrap();
        storage.write("prefix:key2", b"data2").await.unwrap();
        storage.write("other:key3", b"data3").await.unwrap();

        // List keys with prefix
        let keys = storage.list_keys("prefix:").await.unwrap();
        assert!(keys.contains(&"prefix:key1".to_string()));
        assert!(keys.contains(&"prefix:key2".to_string()));
        assert!(!keys.contains(&"other:key3".to_string()));

        // Clean up
        storage.delete("prefix:key1").await.unwrap();
        storage.delete("prefix:key2").await.unwrap();
        storage.delete("other:key3").await.unwrap();
    }
}