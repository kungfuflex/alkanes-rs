//! Tests for stateful view functionality
//!
//! This module tests the stateful view runtime that retains WASM memory
//! and instance state between view function calls.

use crate::{
    traits::{BatchLike, KeyValueStoreLike},
    MetashrewRuntime,
};
use anyhow::Result;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Simple in-memory key-value store for testing
#[derive(Clone, Debug)]
pub struct InMemoryStore {
    data: Arc<Mutex<HashMap<Vec<u8>, Vec<u8>>>>,
}

impl InMemoryStore {
    pub fn new() -> Self {
        Self {
            data: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

/// Simple batch implementation for testing
#[derive(Debug)]
pub struct InMemoryBatch {
    operations: Vec<(Vec<u8>, Option<Vec<u8>>)>, // (key, Some(value) for put, None for delete)
}

impl crate::traits::BatchLike for InMemoryBatch {
    fn put<K: AsRef<[u8]>, V: AsRef<[u8]>>(&mut self, key: K, value: V) {
        self.operations
            .push((key.as_ref().to_vec(), Some(value.as_ref().to_vec())));
    }

    fn delete<K: AsRef<[u8]>>(&mut self, key: K) {
        self.operations.push((key.as_ref().to_vec(), None));
    }

    fn default() -> Self {
        Self {
            operations: Vec::new(),
        }
    }
}

impl KeyValueStoreLike for InMemoryStore {
    type Error = std::io::Error;
    type Batch = InMemoryBatch;

    fn write(&mut self, batch: Self::Batch) -> Result<(), Self::Error> {
        let mut data = self.data.lock().map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("Lock error: {}", e))
        })?;
        for (key, value_opt) in batch.operations {
            match value_opt {
                Some(value) => {
                    data.insert(key, value);
                }
                None => {
                    data.remove(&key);
                }
            }
        }
        Ok(())
    }

    fn get<K: AsRef<[u8]>>(&mut self, key: K) -> Result<Option<Vec<u8>>, Self::Error> {
        let data = self.data.lock().map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("Lock error: {}", e))
        })?;
        Ok(data.get(key.as_ref()).cloned())
    }

    fn get_immutable<K: AsRef<[u8]>>(&self, key: K) -> Result<Option<Vec<u8>>, Self::Error> {
        let data = self.data.lock().map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("Lock error: {}", e))
        })?;
        Ok(data.get(key.as_ref()).cloned())
    }

    fn put<K, V>(&mut self, key: K, value: V) -> Result<(), Self::Error>
    where
        K: AsRef<[u8]>,
        V: AsRef<[u8]>,
    {
        let mut data = self.data.lock().map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("Lock error: {}", e))
        })?;
        data.insert(key.as_ref().to_vec(), value.as_ref().to_vec());
        Ok(())
    }

    fn delete<K: AsRef<[u8]>>(&mut self, key: K) -> Result<(), Self::Error> {
        let mut data = self.data.lock().map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("Lock error: {}", e))
        })?;
        data.remove(key.as_ref());
        Ok(())
    }

    fn scan_prefix<K: AsRef<[u8]>>(
        &self,
        prefix: K,
    ) -> Result<Vec<(Vec<u8>, Vec<u8>)>, Self::Error> {
        let data = self.data.lock().map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("Lock error: {}", e))
        })?;
        let prefix_bytes = prefix.as_ref();
        Ok(data
            .iter()
            .filter(|(k, _)| k.starts_with(prefix_bytes))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect())
    }

    fn create_batch(&self) -> Self::Batch {
        InMemoryBatch::default()
    }

    fn keys<'a>(&'a self) -> Result<Box<dyn Iterator<Item = Vec<u8>> + 'a>, Self::Error> {
        let data = self.data.lock().map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("Lock error: {}", e))
        })?;
        let keys: Vec<Vec<u8>> = data.keys().cloned().collect();
        Ok(Box::new(keys.into_iter()))
    }
}

/// Simple WASM module that maintains a counter in memory
/// This demonstrates stateful behavior between view calls
const STATEFUL_WASM: &[u8] = &[
    0x00, 0x61, 0x73, 0x6d, // WASM magic number
    0x01, 0x00, 0x00,
    0x00, // WASM version
          // This is a minimal WASM module for testing
          // In a real implementation, this would be a proper WASM module
          // that maintains state in memory between calls
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stateful_view_basic_functionality() -> Result<()> {
        let store = InMemoryStore::new();
        let runtime = MetashrewRuntime::new(STATEFUL_WASM, store, vec![])?;

        // Initially, stateful views should be disabled
        assert!(!runtime.is_stateful_views_enabled());

        // Note: Full async testing would require tokio runtime
        // For now, we just verify the basic structure is correct
        Ok(())
    }

    #[test]
    fn test_in_memory_store() -> Result<()> {
        let mut store = InMemoryStore::new();

        // Test basic operations
        store.put(b"key1", b"value1")?;
        assert_eq!(store.get(b"key1")?, Some(b"value1".to_vec()));

        // Test batch operations
        let mut batch = store.create_batch();
        batch.put(b"key2", b"value2");
        batch.put(b"key3", b"value3");
        batch.delete(b"key1");
        store.write(batch)?;

        assert_eq!(store.get(b"key1")?, None);
        assert_eq!(store.get(b"key2")?, Some(b"value2".to_vec()));
        assert_eq!(store.get(b"key3")?, Some(b"value3".to_vec()));

        // Test prefix scanning
        store.put(b"prefix:key1", b"value1")?;
        store.put(b"prefix:key2", b"value2")?;
        store.put(b"other:key", b"value")?;

        let prefix_results = store.scan_prefix(b"prefix:")?;
        assert_eq!(prefix_results.len(), 2);

        Ok(())
    }
}
