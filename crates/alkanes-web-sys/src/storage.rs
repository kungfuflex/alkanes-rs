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
use web_sys::{window, Storage};
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

/// Web storage implementation using browser localStorage
///
/// This struct provides a web-compatible storage backend that implements the
/// [`alkanes_cli_common::StorageProvider`] trait. It uses the browser's localStorage
/// API for persistent data storage across browser sessions.
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
    /// Optional reference to the browser's localStorage object
    /// None if localStorage is not available in the current environment
    storage: Option<Storage>,
}

impl WebStorage {
    /// Create a new WebStorage instance
    ///
    /// Attempts to access the browser's localStorage API. If localStorage
    /// is not available (e.g., in private browsing mode or non-browser
    /// environments), the storage field will be None and operations will
    /// return appropriate errors.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use deezel_web::storage::WebStorage;
    ///
    /// let storage = WebStorage::new();
    /// // Storage is ready to use, operations will handle localStorage availability
    /// ```
    pub fn new() -> Self {
        let storage = window()
            .and_then(|w| w.local_storage().ok())
            .flatten();
        
        Self { storage }
    }

    /// Get the localStorage object or return an error
    ///
    /// # Errors
    ///
    /// Returns [`AlkanesError::Storage`] if localStorage is not available
    /// in the current browser environment.
    fn get_storage(&self) -> Result<&Storage> {
        self.storage.as_ref()
            .ok_or_else(|| AlkanesError::Storage("localStorage not available".to_string()))
    }

    /// Encode binary data as base64 for safe text storage
    ///
    /// Uses standard base64 encoding to convert binary data into a text
    /// format suitable for localStorage, which only supports string values.
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

    /// Get the namespaced key for localStorage operations
    ///
    /// Prefixes the provided key with "alkanes:" to create a namespaced
    /// key that avoids conflicts with other applications using localStorage.
    ///
    /// # Arguments
    ///
    /// * `key` - The original key to namespace
    ///
    /// # Returns
    ///
    /// A prefixed key in the format "alkanes:{key}"
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use deezel_web::storage::WebStorage;
    /// let storage = WebStorage::new();
    /// // This would return "alkanes:wallet_data"
    /// // let prefixed = storage.get_prefixed_key("wallet_data");
    /// ```
    fn get_prefixed_key(&self, key: &str) -> String {
        format!("alkanes:{key}")
    }
}

/// Implementation of the [`alkanes_cli_common::StorageProvider`] trait for web environments
///
/// This implementation provides all the standard storage operations using the
/// browser's localStorage API. All operations are async-compatible and handle
/// the web environment's constraints.
#[async_trait(?Send)]
impl alkanes_cli_common::StorageProvider for WebStorage {
    /// Read data from localStorage by key
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
    /// * [`AlkanesError::Storage`] if localStorage is not available
    /// * [`AlkanesError::Storage`] if the key is not found
    /// * [`AlkanesError::Storage`] if the stored data is not valid base64
    /// * [`AlkanesError::Storage`] if localStorage access fails
    async fn read(&self, key: &str) -> Result<Vec<u8>> {
        let storage = self.get_storage()?;
        let prefixed_key = self.get_prefixed_key(key);
        
        let value = storage.get_item(&prefixed_key)
            .map_err(|e| AlkanesError::Storage(format!("Failed to read from localStorage: {e:?}")))?
            .ok_or_else(|| AlkanesError::Storage(format!("Key not found: {key}")))?;
        
        self.decode_data(&value)
    }

    /// Write data to localStorage
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
    /// * [`AlkanesError::Storage`] if localStorage is not available
    /// * [`AlkanesError::Storage`] if localStorage is full (quota exceeded)
    /// * [`AlkanesError::Storage`] if localStorage access fails
    async fn write(&self, key: &str, data: &[u8]) -> Result<()> {
        let storage = self.get_storage()?;
        let prefixed_key = self.get_prefixed_key(key);
        let encoded_data = self.encode_data(data);
        
        storage.set_item(&prefixed_key, &encoded_data)
            .map_err(|e| AlkanesError::Storage(format!("Failed to write to localStorage: {e:?}")))?;
        
        Ok(())
    }

    /// Check if a key exists in localStorage
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
    ///
    /// # Errors
    ///
    /// * [`AlkanesError::Storage`] if localStorage is not available
    /// * [`AlkanesError::Storage`] if localStorage access fails
    async fn exists(&self, key: &str) -> Result<bool> {
        let storage = self.get_storage()?;
        let prefixed_key = self.get_prefixed_key(key);
        
        let exists = storage.get_item(&prefixed_key)
            .map_err(|e| AlkanesError::Storage(format!("Failed to check localStorage: {e:?}")))?
            .is_some();
        
        Ok(exists)
    }

    /// Delete a key from localStorage
    ///
    /// Removes the specified key and its associated value from storage.
    ///
    /// # Arguments
    ///
    /// * `key` - The key to delete (will be automatically prefixed with "alkanes:")
    ///
    /// # Errors
    ///
    /// * [`AlkanesError::Storage`] if localStorage is not available
    /// * [`AlkanesError::Storage`] if localStorage access fails
    ///
    /// # Note
    ///
    /// This operation succeeds even if the key doesn't exist.
    async fn delete(&self, key: &str) -> Result<()> {
        let storage = self.get_storage()?;
        let prefixed_key = self.get_prefixed_key(key);
        
        storage.remove_item(&prefixed_key)
            .map_err(|e| AlkanesError::Storage(format!("Failed to delete from localStorage: {e:?}")))?;
        
        Ok(())
    }

    /// List all keys matching a prefix
    ///
    /// Returns all keys in storage that start with the given prefix.
    /// The returned keys have the "alkanes:" namespace prefix removed.
    ///
    /// # Arguments
    ///
    /// * `prefix` - The prefix to match against (will be automatically prefixed with "alkanes:")
    ///
    /// # Returns
    ///
    /// A vector of keys (without the "alkanes:" prefix) that match the prefix
    ///
    /// # Errors
    ///
    /// * [`AlkanesError::Storage`] if localStorage is not available
    /// * [`AlkanesError::Storage`] if localStorage access fails
    ///
    /// # Performance
    ///
    /// This operation iterates through all keys in localStorage, so performance
    /// may degrade with large numbers of stored items.
    async fn list_keys(&self, prefix: &str) -> Result<Vec<String>> {
        let storage = self.get_storage()?;
        let full_prefix = self.get_prefixed_key(prefix);
        let mut keys = Vec::new();
        
        // Get the length of localStorage
        let length = storage.length()
            .map_err(|e| AlkanesError::Storage(format!("Failed to get localStorage length: {e:?}")))?;
        
        // Iterate through all keys
        for i in 0..length {
            if let Ok(Some(key)) = storage.key(i) {
                if key.starts_with(&full_prefix) {
                    // Remove the "alkanes:" prefix to return the original key
                    if let Some(original_key) = key.strip_prefix("alkanes:") {
                        keys.push(original_key.to_string());
                    }
                }
            }
        }
        
        Ok(keys)
    }

    /// Get the storage type identifier
    ///
    /// Returns a string identifier for this storage backend type.
    ///
    /// # Returns
    ///
    /// Always returns "localStorage" for this implementation
    fn storage_type(&self) -> &'static str {
        "localStorage"
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