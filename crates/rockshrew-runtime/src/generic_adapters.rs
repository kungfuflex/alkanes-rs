//! RocksDB-specific implementations of generic adapter traits
//!
//! This module provides concrete implementations of the generic adapter traits
//! from metashrew-runtime for RocksDB storage backend.

use anyhow::Result;
use async_trait::async_trait;
use metashrew_runtime::adapters::{
    HeightTracker, StateRootManager, BatchProcessor,
    GenericHeightTracker, GenericStateRootManager,
};
use metashrew_runtime::adapters::traits::{BlockHashManager, StorageAdapterCore};
use metashrew_runtime::{KeyValueStoreLike, to_labeled_key, TIP_HEIGHT_KEY, BatchLike};
use rocksdb::DB;
use std::sync::Arc;

use crate::adapter::{RocksDBRuntimeAdapter, RocksDBBatch};

/// RocksDB-specific height tracker implementation
pub struct RocksDBHeightTracker {
    db: Arc<DB>,
}

impl RocksDBHeightTracker {
    pub fn new(db: Arc<DB>) -> Self {
        Self { db }
    }
}

#[async_trait]
impl HeightTracker for RocksDBHeightTracker {
    async fn get_current_height(&self) -> Result<u32> {
        let height_key = TIP_HEIGHT_KEY.as_bytes().to_vec();
        let bytes = match self.db.get(&to_labeled_key(&height_key))? {
            Some(v) => v,
            None => return Ok(0),
        };
        if bytes.is_empty() {
            return Ok(0);
        }
        let bytes_ref: &[u8] = &bytes;
        Ok(u32::from_le_bytes(bytes_ref.try_into().unwrap_or([0; 4])))
    }

    async fn set_current_height(&mut self, height: u32) -> Result<()> {
        let height_key = TIP_HEIGHT_KEY.as_bytes().to_vec();
        let height_bytes = height.to_le_bytes().to_vec();
        self.db.put(&to_labeled_key(&height_key), &height_bytes)?;
        Ok(())
    }

    async fn get_indexed_height(&self) -> Result<u32> {
        // For RocksDB, indexed height is the same as current height
        self.get_current_height().await
    }

    async fn set_indexed_height(&mut self, height: u32) -> Result<()> {
        // For RocksDB, indexed height is the same as current height
        self.set_current_height(height).await
    }
}

/// RocksDB-specific state root manager implementation
pub struct RocksDBStateRootManager {
    db: Arc<DB>,
}

impl RocksDBStateRootManager {
    pub fn new(db: Arc<DB>) -> Self {
        Self { db }
    }
}

#[async_trait]
impl StateRootManager for RocksDBStateRootManager {
    async fn store_state_root(&self, height: u32, root: &[u8]) -> Result<()> {
        let root_key = format!("state_root_{}", height);
        self.db.put(&to_labeled_key(&root_key.as_bytes().to_vec()), root)?;
        Ok(())
    }

    async fn get_state_root(&self, height: u32) -> Result<Option<Vec<u8>>> {
        let root_key = format!("state_root_{}", height);
        match self.db.get(&to_labeled_key(&root_key.as_bytes().to_vec()))? {
            Some(root) => Ok(Some(root.to_vec())),
            None => Ok(None),
        }
    }

    async fn get_latest_state_root(&self) -> Result<Option<(u32, Vec<u8>)>> {
        // This is a simplified implementation - in practice you'd want to iterate
        // through state root keys to find the latest one
        // For now, we'll return None as a placeholder
        Ok(None)
    }
}

/// RocksDB-specific batch processor implementation
pub struct RocksDBBatchProcessor {
    adapter: RocksDBRuntimeAdapter,
}

impl RocksDBBatchProcessor {
    pub fn new(adapter: RocksDBRuntimeAdapter) -> Self {
        Self { adapter }
    }
}

impl BatchProcessor<RocksDBBatch> for RocksDBBatchProcessor {
    fn create_batch(&self) -> RocksDBBatch {
        self.adapter.create_batch()
    }

    fn write_batch(&mut self, batch: RocksDBBatch) -> Result<()> {
        let mut adapter = self.adapter.clone();
        adapter.write(batch).map_err(|e| anyhow::anyhow!("RocksDB write error: {}", e))?;
        Ok(())
    }

    fn create_atomic_batch(&self, operations: RocksDBBatch, new_height: u32) -> RocksDBBatch {
        let mut batch = operations;
        let height_key = TIP_HEIGHT_KEY.as_bytes().to_vec();
        batch.put(&to_labeled_key(&height_key), &new_height.to_le_bytes());
        batch
    }
}

/// RocksDB-specific block hash manager implementation
pub struct RocksDBBlockHashManager {
    db: Arc<DB>,
}

impl RocksDBBlockHashManager {
    pub fn new(db: Arc<DB>) -> Self {
        Self { db }
    }
}

#[async_trait]
impl BlockHashManager for RocksDBBlockHashManager {
    async fn store_block_hash(&self, height: u32, hash: &[u8]) -> Result<()> {
        let hash_key = format!("block_hash_{}", height);
        self.db.put(&to_labeled_key(&hash_key.as_bytes().to_vec()), hash)?;
        Ok(())
    }

    async fn get_block_hash(&self, height: u32) -> Result<Option<Vec<u8>>> {
        let hash_key = format!("block_hash_{}", height);
        match self.db.get(&to_labeled_key(&hash_key.as_bytes().to_vec()))? {
            Some(hash) => Ok(Some(hash.to_vec())),
            None => Ok(None),
        }
    }

    async fn remove_block_hashes_after(&self, height: u32) -> Result<()> {
        // This is a simplified implementation - in practice you'd want to iterate
        // through keys and remove those with height > specified height
        // For now, we'll implement a basic version
        use rocksdb::WriteBatch;
        let mut batch = WriteBatch::default();
        
        // In a real implementation, you'd iterate through all block hash keys
        // and remove those with height > specified height
        // This is a placeholder implementation
        for h in (height + 1)..=height + 1000 {
            let hash_key = format!("block_hash_{}", h);
            batch.delete(&to_labeled_key(&hash_key.as_bytes().to_vec()));
        }
        
        self.db.write(batch)?;
        Ok(())
    }
}

/// RocksDB-specific storage adapter core implementation
pub struct RocksDBStorageAdapterCore {
    height_tracker: RocksDBHeightTracker,
    state_root_manager: RocksDBStateRootManager,
    block_hash_manager: RocksDBBlockHashManager,
}

impl RocksDBStorageAdapterCore {
    pub fn new(adapter: RocksDBRuntimeAdapter) -> Self {
        Self {
            height_tracker: RocksDBHeightTracker::new(adapter.db.clone()),
            state_root_manager: RocksDBStateRootManager::new(adapter.db.clone()),
            block_hash_manager: RocksDBBlockHashManager::new(adapter.db.clone()),
        }
    }
}

#[async_trait]
impl HeightTracker for RocksDBStorageAdapterCore {
    async fn get_current_height(&self) -> Result<u32> {
        self.height_tracker.get_current_height().await
    }

    async fn set_current_height(&mut self, height: u32) -> Result<()> {
        // Note: This is a limitation - we can't mutate the inner tracker
        // In practice, you'd want to use interior mutability or a different design
        let key = TIP_HEIGHT_KEY.as_bytes().to_vec();
        self.height_tracker.db.put(&to_labeled_key(&key), &height.to_le_bytes())?;
        Ok(())
    }

    async fn get_indexed_height(&self) -> Result<u32> {
        self.height_tracker.get_indexed_height().await
    }

    async fn set_indexed_height(&mut self, height: u32) -> Result<()> {
        let key = TIP_HEIGHT_KEY.as_bytes().to_vec();
        self.height_tracker.db.put(&to_labeled_key(&key), &height.to_le_bytes())?;
        Ok(())
    }
}

#[async_trait]
impl StateRootManager for RocksDBStorageAdapterCore {
    async fn store_state_root(&self, height: u32, root: &[u8]) -> Result<()> {
        self.state_root_manager.store_state_root(height, root).await
    }

    async fn get_state_root(&self, height: u32) -> Result<Option<Vec<u8>>> {
        self.state_root_manager.get_state_root(height).await
    }

    async fn get_latest_state_root(&self) -> Result<Option<(u32, Vec<u8>)>> {
        self.state_root_manager.get_latest_state_root().await
    }
}

#[async_trait]
impl BlockHashManager for RocksDBStorageAdapterCore {
    async fn store_block_hash(&self, height: u32, hash: &[u8]) -> Result<()> {
        self.block_hash_manager.store_block_hash(height, hash).await
    }

    async fn get_block_hash(&self, height: u32) -> Result<Option<Vec<u8>>> {
        self.block_hash_manager.get_block_hash(height).await
    }

    async fn remove_block_hashes_after(&self, height: u32) -> Result<()> {
        self.block_hash_manager.remove_block_hashes_after(height).await
    }
}

#[async_trait]
impl StorageAdapterCore for RocksDBStorageAdapterCore {
    async fn is_available(&self) -> bool {
        // Simple availability check - in practice you might want more sophisticated checks
        true
    }
}

/// Convenience functions to create generic implementations using RocksDB
impl RocksDBRuntimeAdapter {
    /// Create a generic height tracker using this RocksDB adapter
    pub fn create_height_tracker(&self) -> GenericHeightTracker<Self> {
        GenericHeightTracker::new(self.clone())
    }

    /// Create a generic state root manager using this RocksDB adapter
    pub fn create_state_root_manager(&self) -> GenericStateRootManager<Self> {
        GenericStateRootManager::new(self.clone())
    }

    /// Create RocksDB-specific height tracker
    pub fn create_rocksdb_height_tracker(&self) -> RocksDBHeightTracker {
        RocksDBHeightTracker::new(self.db.clone())
    }

    /// Create RocksDB-specific state root manager
    pub fn create_rocksdb_state_root_manager(&self) -> RocksDBStateRootManager {
        RocksDBStateRootManager::new(self.db.clone())
    }

    /// Create RocksDB-specific batch processor
    pub fn create_batch_processor(&self) -> RocksDBBatchProcessor {
        RocksDBBatchProcessor::new(self.clone())
    }

    /// Create RocksDB-specific block hash manager
    pub fn create_block_hash_manager(&self) -> RocksDBBlockHashManager {
        RocksDBBlockHashManager::new(self.db.clone())
    }

    /// Create RocksDB-specific storage adapter core
    pub fn create_storage_adapter_core(&self) -> RocksDBStorageAdapterCore {
        RocksDBStorageAdapterCore::new(self.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rocksdb::Options;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_rocksdb_height_tracker() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let mut opts = Options::default();
        opts.create_if_missing(true);
        
        let adapter = RocksDBRuntimeAdapter::open(temp_dir.path().to_string_lossy().to_string(), opts)?;
        let mut tracker = adapter.create_rocksdb_height_tracker();

        // Test height operations
        assert_eq!(tracker.get_current_height().await?, 0);
        
        tracker.set_current_height(42).await?;
        assert_eq!(tracker.get_current_height().await?, 42);

        Ok(())
    }

    #[tokio::test]
    async fn test_rocksdb_state_root_manager() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let mut opts = Options::default();
        opts.create_if_missing(true);
        
        let adapter = RocksDBRuntimeAdapter::open(temp_dir.path().to_string_lossy().to_string(), opts)?;
        let mut manager = adapter.create_rocksdb_state_root_manager();

        // Test state root operations
        assert_eq!(manager.get_state_root(100).await?, None);
        
        let root = b"test_root_hash";
        manager.set_state_root(100, root).await?;
        assert_eq!(manager.get_state_root(100).await?, Some(root.to_vec()));

        manager.delete_state_root(100).await?;
        assert_eq!(manager.get_state_root(100).await?, None);

        Ok(())
    }

    #[tokio::test]
    async fn test_generic_vs_specific_implementations() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let mut opts = Options::default();
        opts.create_if_missing(true);
        
        let adapter = RocksDBRuntimeAdapter::open(temp_dir.path().to_string_lossy().to_string(), opts)?;
        
        // Test that both generic and specific implementations work
        let mut generic_tracker = adapter.create_height_tracker();
        let mut specific_tracker = adapter.create_rocksdb_height_tracker();

        // Both should start at 0
        assert_eq!(generic_tracker.get_current_height().await?, 0);
        assert_eq!(specific_tracker.get_current_height().await?, 0);

        // Set height using generic implementation
        generic_tracker.set_current_height(50).await?;
        
        // Both should see the same value
        assert_eq!(generic_tracker.get_current_height().await?, 50);
        assert_eq!(specific_tracker.get_current_height().await?, 50);

        Ok(())
    }
}