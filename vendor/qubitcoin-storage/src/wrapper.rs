//! `DbWrapper`: typed serialization + XOR obfuscation layer.
//!
//! Maps to: `CDBWrapper` in Bitcoin Core's `src/dbwrapper.h`.
//!
//! This module wraps a raw `Database` with:
//! 1. Typed key/value serialization via `Encodable`/`Decodable`.
//! 2. XOR obfuscation of stored values (matching Bitcoin Core).
//!
//! The obfuscation key is persisted in the database under a well-known key
//! so that values remain readable across restarts.

use crate::traits::{Database, DbBatch};
use qubitcoin_serialize::{Decodable, Encodable, Error as SerError};
use std::io::Cursor;

/// Database wrapper providing typed serialization and optional XOR obfuscation.
///
/// Wraps a raw `Database` implementation. When obfuscation is enabled, all
/// values are XOR-ed with a random 8-byte key before being written and
/// un-XOR-ed on read. The obfuscation key itself is stored unobfuscated
/// under the `OBFUSCATION_KEY_KEY` constant.
///
/// Equivalent to `CDBWrapper` in Bitcoin Core.
pub struct DbWrapper<D: Database> {
    /// The underlying raw database.
    db: D,
    /// The XOR obfuscation key (empty if obfuscation is disabled).
    obfuscation_key: Vec<u8>,
}

/// The database key under which the obfuscation key is stored.
const OBFUSCATION_KEY_KEY: &[u8] = b"\x0e\x00obfuscate_key";

/// Length of the obfuscation key in bytes (8).
const OBFUSCATION_KEY_LEN: usize = 8;

impl<D: Database> DbWrapper<D> {
    /// Create a new DbWrapper. If `obfuscate` is true and no obfuscation key
    /// exists in the database, a random one is generated and stored.
    pub fn new(db: D, obfuscate: bool) -> Self {
        let obfuscation_key = if obfuscate {
            // Try to read existing obfuscation key
            if let Ok(Some(key)) = db.read(OBFUSCATION_KEY_KEY) {
                key
            } else {
                // Generate new key
                #[cfg(feature = "rand")]
                let key: Vec<u8> = (0..OBFUSCATION_KEY_LEN)
                    .map(|_| rand::random::<u8>())
                    .collect();
                #[cfg(not(feature = "rand"))]
                let key: Vec<u8> = vec![0x1a, 0x2b, 0x3c, 0x4d, 0x5e, 0x6f, 0x70, 0x81];
                // Store it
                let mut batch = db.new_batch();
                batch.put(OBFUSCATION_KEY_KEY, &key);
                let _ = db.write_batch(batch, true);
                key
            }
        } else {
            vec![]
        };

        DbWrapper {
            db,
            obfuscation_key,
        }
    }

    /// Create without obfuscation (for testing).
    pub fn new_unobfuscated(db: D) -> Self {
        DbWrapper {
            db,
            obfuscation_key: vec![],
        }
    }

    /// Read a typed value from the database.
    pub fn read<K: AsRef<[u8]>, V: Decodable>(&self, key: K) -> Result<Option<V>, DbWrapperError> {
        match self
            .db
            .read(key.as_ref())
            .map_err(|e| DbWrapperError::Db(e.to_string()))?
        {
            Some(mut raw) => {
                self.xor_bytes(&mut raw);
                let value =
                    V::decode(&mut Cursor::new(&raw)).map_err(|e| DbWrapperError::Serialize(e))?;
                Ok(Some(value))
            }
            None => Ok(None),
        }
    }

    /// Read multiple typed values in a single batch operation.
    pub fn multi_read<V: Decodable>(&self, keys: &[Vec<u8>]) -> Vec<Result<Option<V>, DbWrapperError>> {
        let key_refs: Vec<&[u8]> = keys.iter().map(|k| k.as_slice()).collect();
        self.db
            .multi_read(&key_refs)
            .into_iter()
            .map(|result| {
                match result.map_err(|e| DbWrapperError::Db(e.to_string()))? {
                    Some(mut raw) => {
                        self.xor_bytes(&mut raw);
                        let value = V::decode(&mut Cursor::new(&raw))
                            .map_err(DbWrapperError::Serialize)?;
                        Ok(Some(value))
                    }
                    None => Ok(None),
                }
            })
            .collect()
    }

    /// Check if a key exists in the database.
    pub fn exists<K: AsRef<[u8]>>(&self, key: K) -> Result<bool, DbWrapperError> {
        self.db
            .exists(key.as_ref())
            .map_err(|e| DbWrapperError::Db(e.to_string()))
    }

    /// Write a typed key-value pair.
    pub fn write<K: AsRef<[u8]>, V: Encodable>(
        &self,
        key: K,
        value: &V,
        sync: bool,
    ) -> Result<(), DbWrapperError> {
        let mut batch = self.db.new_batch();
        let mut serialized = Vec::new();
        value
            .encode(&mut serialized)
            .map_err(|e| DbWrapperError::Serialize(e))?;
        self.xor_bytes(&mut serialized);
        batch.put(key.as_ref(), &serialized);
        self.db
            .write_batch(batch, sync)
            .map_err(|e| DbWrapperError::Db(e.to_string()))
    }

    /// Delete a key.
    pub fn erase<K: AsRef<[u8]>>(&self, key: K, sync: bool) -> Result<(), DbWrapperError> {
        let mut batch = self.db.new_batch();
        batch.delete(key.as_ref());
        self.db
            .write_batch(batch, sync)
            .map_err(|e| DbWrapperError::Db(e.to_string()))
    }

    /// Get a reference to the underlying raw [`Database`].
    pub fn inner(&self) -> &D {
        &self.db
    }

    /// Serialize and XOR-obfuscate a value, returning the raw bytes
    /// ready for insertion into a [`DbBatch`].
    pub fn serialize_value<V: Encodable>(&self, value: &V) -> Result<Vec<u8>, DbWrapperError> {
        let mut serialized = Vec::new();
        value
            .encode(&mut serialized)
            .map_err(|e| DbWrapperError::Serialize(e))?;
        self.xor_bytes(&mut serialized);
        Ok(serialized)
    }

    /// Create a new write batch on the underlying database.
    pub fn new_batch(&self) -> D::Batch {
        self.db.new_batch()
    }

    /// Commit a pre-built write batch.
    pub fn write_batch(&self, batch: D::Batch, sync: bool) -> Result<(), DbWrapperError> {
        self.db
            .write_batch(batch, sync)
            .map_err(|e| DbWrapperError::Db(e.to_string()))
    }

    /// XOR obfuscation: XOR each byte with the obfuscation key (repeating).
    fn xor_bytes(&self, data: &mut [u8]) {
        if self.obfuscation_key.is_empty() {
            return;
        }
        for (i, byte) in data.iter_mut().enumerate() {
            *byte ^= self.obfuscation_key[i % self.obfuscation_key.len()];
        }
    }
}

/// Error type for [`DbWrapper`] operations.
#[derive(Debug, thiserror::Error)]
pub enum DbWrapperError {
    /// An error from the underlying database backend.
    #[error("Database error: {0}")]
    Db(String),
    /// A serialization or deserialization error.
    #[error("Serialization error: {0}")]
    Serialize(SerError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::MemoryDb;

    #[test]
    fn test_typed_read_write() {
        let db = MemoryDb::new();
        let wrapper = DbWrapper::new_unobfuscated(db);

        let value: u32 = 42;
        wrapper.write(b"test_key", &value, false).unwrap();

        let read_back: u32 = wrapper.read::<_, u32>(b"test_key").unwrap().unwrap();
        assert_eq!(read_back, 42);
    }

    #[test]
    fn test_exists_and_erase() {
        let db = MemoryDb::new();
        let wrapper = DbWrapper::new_unobfuscated(db);

        wrapper.write(b"key", &100u32, false).unwrap();
        assert!(wrapper.exists(b"key").unwrap());

        wrapper.erase(b"key", false).unwrap();
        assert!(!wrapper.exists(b"key").unwrap());
    }

    #[test]
    fn test_obfuscation() {
        let db = MemoryDb::new();
        let wrapper = DbWrapper::new(db, true);

        let value: u64 = 0xdeadbeef12345678;
        wrapper.write(b"obf_key", &value, false).unwrap();

        // Reading through wrapper should give back original value
        let read_back: u64 = wrapper.read::<_, u64>(b"obf_key").unwrap().unwrap();
        assert_eq!(read_back, value);

        // Reading raw bytes from underlying db should be XOR'd (not original)
        let raw = wrapper.inner().read(b"obf_key").unwrap().unwrap();
        let mut original_bytes = Vec::new();
        value.encode(&mut original_bytes).unwrap();
        // The raw bytes should differ from the original (unless obfuscation key is all zeros, which is extremely unlikely)
        assert_ne!(raw, original_bytes);
    }

    #[test]
    fn test_missing_key() {
        let db = MemoryDb::new();
        let wrapper = DbWrapper::new_unobfuscated(db);

        let result: Option<u32> = wrapper.read(b"nonexistent").unwrap();
        assert!(result.is_none());
    }
}
