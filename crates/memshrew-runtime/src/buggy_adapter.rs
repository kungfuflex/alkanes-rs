//! Buggy rollback implementation for reproducing the reorg bug
//!
//! This module contains the OLD buggy rollback that only deletes metadata,
//! not the actual SMT data. We use this to prove the bug exists before fixing it.

use crate::adapter::MemStoreAdapter;
use metashrew_runtime::to_labeled_key;
use metashrew_sync::{StorageAdapter, SyncResult};
use async_trait::async_trait;

/// Wrapper around MemStoreAdapter that uses the BUGGY rollback
#[derive(Clone)]
pub struct BuggyMemStoreAdapter {
    inner: MemStoreAdapter,
}

impl BuggyMemStoreAdapter {
    pub fn new() -> Self {
        Self {
            inner: MemStoreAdapter::new(),
        }
    }

    pub fn inner(&self) -> &MemStoreAdapter {
        &self.inner
    }
}

// Forward all KeyValueStoreLike methods to inner
impl metashrew_runtime::KeyValueStoreLike for BuggyMemStoreAdapter {
    type Batch = <MemStoreAdapter as metashrew_runtime::KeyValueStoreLike>::Batch;
    type Error = <MemStoreAdapter as metashrew_runtime::KeyValueStoreLike>::Error;

    fn write(&mut self, batch: Self::Batch) -> Result<(), Self::Error> {
        self.inner.write(batch)
    }

    fn get<K: AsRef<[u8]>>(&mut self, key: K) -> Result<Option<Vec<u8>>, Self::Error> {
        self.inner.get(key)
    }

    fn get_immutable<K: AsRef<[u8]>>(&self, key: K) -> Result<Option<Vec<u8>>, Self::Error> {
        self.inner.get_immutable(key)
    }

    fn put<K, V>(&mut self, key: K, value: V) -> Result<(), Self::Error>
    where
        K: AsRef<[u8]>,
        V: AsRef<[u8]>,
    {
        self.inner.put(key, value)
    }

    fn delete<K: AsRef<[u8]>>(&mut self, key: K) -> Result<(), Self::Error> {
        self.inner.delete(key)
    }

    fn scan_prefix<K: AsRef<[u8]>>(&self, prefix: K) -> Result<Vec<(Vec<u8>, Vec<u8>)>, Self::Error> {
        self.inner.scan_prefix(prefix)
    }

    fn create_batch(&self) -> Self::Batch {
        self.inner.create_batch()
    }

    fn keys<'a>(&'a self) -> Result<Box<dyn Iterator<Item = Vec<u8>> + 'a>, Self::Error> {
        self.inner.keys()
    }

    fn is_open(&self) -> bool {
        self.inner.is_open()
    }

    fn set_height(&mut self, height: u32) {
        self.inner.set_height(height)
    }

    fn get_height(&self) -> u32 {
        self.inner.get_height()
    }

    fn track_kv_update(&mut self, key: Vec<u8>, value: Vec<u8>) {
        self.inner.track_kv_update(key, value)
    }

    fn create_isolated_copy(&self) -> Self {
        Self {
            inner: self.inner.create_isolated_copy(),
        }
    }
}

#[async_trait]
impl StorageAdapter for BuggyMemStoreAdapter {
    async fn get_indexed_height(&self) -> SyncResult<u32> {
        self.inner.get_indexed_height().await
    }

    async fn set_indexed_height(&mut self, height: u32) -> SyncResult<()> {
        self.inner.set_indexed_height(height).await
    }

    async fn store_block_hash(&mut self, height: u32, hash: &[u8]) -> SyncResult<()> {
        self.inner.store_block_hash(height, hash).await
    }

    async fn get_block_hash(&self, height: u32) -> SyncResult<Option<Vec<u8>>> {
        self.inner.get_block_hash(height).await
    }

    async fn store_state_root(&mut self, height: u32, root: &[u8]) -> SyncResult<()> {
        self.inner.store_state_root(height, root).await
    }

    async fn get_state_root(&self, height: u32) -> SyncResult<Option<Vec<u8>>> {
        self.inner.get_state_root(height).await
    }

    /// BUGGY IMPLEMENTATION - Only deletes metadata, NOT SMT data!
    /// This mimics the old RocksDB bug.
    async fn rollback_to_height(&mut self, height: u32) -> SyncResult<()> {
        use metashrew_runtime::{to_labeled_key, KeyValueStoreLike};

        let current_height = self.inner.get_height();

        if height >= current_height {
            return Ok(());
        }

        let db = &mut self.inner.db;
        let mut db_guard = db.lock().unwrap();

        // Only delete metadata for heights > rollback height
        // This mimics RocksDB's storage_adapter.rs lines 93-102 (OLD BUGGY VERSION)
        for h in (height + 1)..=current_height {
            // Delete block hash metadata
            let blockhash_key = to_labeled_key(&format!("block_hash_{}", h).as_bytes().to_vec());
            db_guard.remove(&blockhash_key);

            // Delete state root metadata
            let state_root_key = to_labeled_key(&format!("state_root_{}", h).as_bytes().to_vec());
            db_guard.remove(&state_root_key);
            let smt_root_key = to_labeled_key(&format!("smt:root:{}", h).as_bytes().to_vec());
            db_guard.remove(&smt_root_key);
        }

        // CRITICAL BUG: We do NOT delete the actual SMT data written by the WASM indexer
        // This means keys like:
        //   - /blocks/{height}
        //   - /block-hashes/{height}
        //   - /blocktracker updates
        //   - Any other WASM-indexed data
        // ...are NOT cleaned up during rollback!
        //
        // This causes reorg failures because when we try to index a different block
        // at the same height, the old data is still present in the SMT.

        drop(db_guard);
        self.inner.set_height(height);
        Ok(())
    }

    async fn is_available(&self) -> bool {
        self.inner.is_available().await
    }

    async fn get_stats(&self) -> SyncResult<metashrew_sync::StorageStats> {
        self.inner.get_stats().await
    }
}
