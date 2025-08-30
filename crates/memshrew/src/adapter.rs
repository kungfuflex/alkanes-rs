//! In-memory implementation of KeyValueStoreLike trait for fast testing

use metashrew_runtime::{to_labeled_key, BatchLike, KeyValueStoreLike, TIP_HEIGHT_KEY};
use std::collections::HashMap;
use std::io::{Error, Result};
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
pub struct MemStoreAdapter {
    pub db: Arc<Mutex<HashMap<Vec<u8>, Vec<u8>>>>,
    pub height: u32,
}

impl MemStoreAdapter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_data(data: HashMap<Vec<u8>, Vec<u8>>) -> Self {
        Self {
            db: Arc::new(Mutex::new(data)),
            height: 0,
        }
    }

    /// Get a snapshot of all data (useful for testing)
    pub fn get_all_data(&self) -> HashMap<Vec<u8>, Vec<u8>> {
        self.db.lock().unwrap().clone()
    }

    /// Clear all data (useful for testing)
    pub fn clear(&mut self) {
        self.db.lock().unwrap().clear();
        self.height = 0;
    }

    /// Get the number of keys stored
    pub fn len(&self) -> usize {
        self.db.lock().unwrap().len()
    }

    /// Check if the store is empty
    pub fn is_empty(&self) -> bool {
        self.db.lock().unwrap().is_empty()
    }

    /// Create a deep copy with isolated data (useful for preview operations)
    pub fn deep_copy(&self) -> Self {
        let data = self.get_all_data();
        Self {
            db: Arc::new(Mutex::new(data)),
            height: self.height,
        }
    }
}

pub struct MemStoreBatch {
    operations: Vec<BatchOperation>,
}

#[derive(Clone)]
enum BatchOperation {
    Put(Vec<u8>, Vec<u8>),
    Delete(Vec<u8>),
}

impl MemStoreBatch {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the number of operations in this batch
    pub fn len(&self) -> usize {
        self.operations.len()
    }

    /// Check if the batch is empty
    pub fn is_empty(&self) -> bool {
        self.operations.is_empty()
    }
}

impl BatchLike for MemStoreBatch {
    fn put<K: AsRef<[u8]>, V: AsRef<[u8]>>(&mut self, key: K, value: V) {
        self.operations.push(BatchOperation::Put(
            key.as_ref().to_vec(),
            value.as_ref().to_vec(),
        ));
    }

    fn delete<K: AsRef<[u8]>>(&mut self, key: K) {
        self.operations
            .push(BatchOperation::Delete(key.as_ref().to_vec()));
    }

    fn default() -> Self {
        Self {
            operations: Vec::new(),
        }
    }
}

impl KeyValueStoreLike for MemStoreAdapter {
    type Batch = MemStoreBatch;
    type Error = Error;

    fn write(&mut self, batch: Self::Batch) -> Result<()> {
        let mut db = self.db.lock().unwrap();
        for operation in batch.operations {
            match operation {
                BatchOperation::Put(key, value) => {
                    db.insert(to_labeled_key(&key), value);
                }
                BatchOperation::Delete(key) => {
                    db.remove(&to_labeled_key(&key));
                }
            }
        }
        Ok(())
    }

    fn get<K: AsRef<[u8]>>(&mut self, key: K) -> Result<Option<Vec<u8>>> {
        let db = self.db.lock().unwrap();
        Ok(db.get(&to_labeled_key(&key.as_ref().to_vec())).cloned())
    }

    fn get_immutable<K: AsRef<[u8]>>(&self, key: K) -> Result<Option<Vec<u8>>> {
        let db = self.db.lock().unwrap();
        Ok(db.get(&to_labeled_key(&key.as_ref().to_vec())).cloned())
    }

    fn put<K, V>(&mut self, key: K, value: V) -> Result<()>
    where
        K: AsRef<[u8]>,
        V: AsRef<[u8]>,
    {
        let mut db = self.db.lock().unwrap();
        db.insert(
            to_labeled_key(&key.as_ref().to_vec()),
            value.as_ref().to_vec(),
        );
        Ok(())
    }

    fn delete<K: AsRef<[u8]>>(&mut self, key: K) -> Result<()> {
        let mut db = self.db.lock().unwrap();
        db.remove(&to_labeled_key(&key.as_ref().to_vec()));
        Ok(())
    }

    fn scan_prefix<K: AsRef<[u8]>>(&self, prefix: K) -> Result<Vec<(Vec<u8>, Vec<u8>)>> {
        let db = self.db.lock().unwrap();
        let prefix_bytes = to_labeled_key(&prefix.as_ref().to_vec());
        let mut results = Vec::new();

        for (key, value) in db.iter() {
            if key.starts_with(&prefix_bytes) {
                results.push((key.clone(), value.clone()));
            }
        }

        // Sort results by key for consistent ordering
        results.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(results)
    }

    fn create_batch(&self) -> Self::Batch {
        <MemStoreBatch as BatchLike>::default()
    }

    fn keys<'a>(&'a self) -> Result<Box<dyn Iterator<Item = Vec<u8>> + 'a>> {
        let db = self.db.lock().unwrap();
        let keys = db.keys().cloned().collect::<Vec<Vec<u8>>>();
        Ok(Box::new(keys.into_iter()))
    }

    fn is_open(&self) -> bool {
        true // In-memory store is always "open"
    }

    fn set_height(&mut self, height: u32) {
        self.height = height;
    }

    fn get_height(&self) -> u32 {
        self.height
    }

    fn track_kv_update(&mut self, _key: Vec<u8>, _value: Vec<u8>) {
        // In-memory implementation doesn't need tracking by default
        // This can be extended if needed for testing purposes
    }

    fn create_isolated_copy(&self) -> Self {
        self.deep_copy()
    }

    fn clear(&mut self) -> Result<()> {
        self.db.lock().unwrap().clear();
        self.height = 0;
        Ok(())
    }
}
use async_trait::async_trait;
use metashrew_sync::{StorageAdapter, StorageStats, SyncResult};

#[async_trait]
impl StorageAdapter for MemStoreAdapter {
    async fn get_indexed_height(&self) -> SyncResult<u32> {
        Ok(self.get_height())
    }
    async fn set_indexed_height(&mut self, height: u32) -> SyncResult<()> {
        self.set_height(height);
        Ok(())
    }
    async fn store_block_hash(&mut self, height: u32, hash: &[u8]) -> SyncResult<()> {
        self.put(format!("block_hash_{}", height).as_bytes(), hash)
            .unwrap();
        Ok(())
    }
    async fn get_block_hash(&self, height: u32) -> SyncResult<Option<Vec<u8>>> {
        Ok(self
            .get_immutable(format!("block_hash_{}", height).as_bytes())
            .unwrap())
    }
    async fn store_state_root(&mut self, height: u32, root: &[u8]) -> SyncResult<()> {
        self.put(format!("state_root_{}", height).as_bytes(), root)
            .unwrap();
        Ok(())
    }
    async fn get_state_root(&self, height: u32) -> SyncResult<Option<Vec<u8>>> {
        Ok(self
            .get_immutable(format!("state_root_{}", height).as_bytes())
            .unwrap())
    }
    async fn rollback_to_height(&mut self, height: u32) -> SyncResult<()> {
        // This is now handled by the MetashrewRuntime::rollback function.
        // This function is kept to satisfy the StorageAdapter trait.
        self.set_height(height);
        Ok(())
    }
    async fn is_available(&self) -> bool {
        true
    }
    async fn get_stats(&self) -> SyncResult<StorageStats> {
        Ok(StorageStats {
            total_entries: self.len(),
            indexed_height: self.get_height(),
            storage_size_bytes: Some(0),
        })
    }
}

/// Query height from in-memory store
pub async fn query_height(adapter: &MemStoreAdapter, start_block: u32) -> anyhow::Result<u32> {
    let height_key = TIP_HEIGHT_KEY.as_bytes().to_vec();
    let db = adapter.db.lock().unwrap();
    let bytes = match db.get(&to_labeled_key(&height_key)) {
        Some(v) => v,
        None => {
            return Ok(start_block);
        }
    };
    if bytes.len() == 0 {
        return Ok(start_block);
    }
    let bytes_ref: &[u8] = &bytes;
    Ok(u32::from_le_bytes(bytes_ref.try_into().unwrap()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memstore_basic_operations() {
        let mut store = MemStoreAdapter::new();

        // Test put and get
        store.put(b"key1", b"value1").unwrap();
        assert_eq!(store.get(b"key1").unwrap(), Some(b"value1".to_vec()));

        // Test delete
        store.delete(b"key1").unwrap();
        assert_eq!(store.get(b"key1").unwrap(), None);
    }

    #[test]
    fn test_memstore_batch_operations() {
        let mut store = MemStoreAdapter::new();
        let mut batch = MemStoreBatch::new();

        batch.put(b"key1", b"value1");
        batch.put(b"key2", b"value2");

        store.write(batch).unwrap();

        assert_eq!(store.get(b"key1").unwrap(), Some(b"value1".to_vec()));
        assert_eq!(store.get(b"key2").unwrap(), Some(b"value2".to_vec()));
    }

    #[test]
    fn test_memstore_height_tracking() {
        let mut store = MemStoreAdapter::new();

        assert_eq!(store.get_height(), 0);

        store.set_height(42);
        assert_eq!(store.get_height(), 42);
    }
}
