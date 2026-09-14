//! RocksDB-specific implementation of the `StorageAdapter` trait.

use async_trait::async_trait;
use log::{info, warn};
use metashrew_runtime::{rollback::SmtRollback, KeyValueStoreLike};
use metashrew_sync::{StorageAdapter, StorageStats, SyncError, SyncResult};
use rocksdb::DB;
use std::sync::Arc;

use crate::adapter::RocksDBRuntimeAdapter;

/// RocksDB storage adapter for persistent storage.
#[derive(Clone)]
pub struct RocksDBStorageAdapter {
    db: Arc<DB>,
}

impl RocksDBStorageAdapter {
    pub fn new(db: Arc<DB>) -> Self {
        Self { db }
    }
}

// Implement SmtRollback trait for proper reorg handling
impl SmtRollback for RocksDBStorageAdapter {
    fn iter_keys<F>(&self, mut callback: F) -> anyhow::Result<()>
    where
        F: FnMut(&[u8]) -> anyhow::Result<()>,
    {
        let iter = self.db.iterator(rocksdb::IteratorMode::Start);
        for item in iter {
            match item {
                Ok((key, _)) => callback(&key)?,
                Err(e) => return Err(anyhow::anyhow!("Failed to iterate keys: {}", e)),
            }
        }
        Ok(())
    }

    fn delete_key(&mut self, key: &[u8]) -> anyhow::Result<()> {
        self.db.delete(key)
            .map_err(|e| anyhow::anyhow!("Failed to delete key: {}", e))
    }

    fn put_key(&mut self, key: &[u8], value: &[u8]) -> anyhow::Result<()> {
        self.db.put(key, value)
            .map_err(|e| anyhow::anyhow!("Failed to put key: {}", e))
    }

    fn get_value(&self, key: &[u8]) -> anyhow::Result<Option<Vec<u8>>> {
        self.db.get(key)
            .map_err(|e| anyhow::anyhow!("Failed to get value: {}", e))
    }
}

#[async_trait]
impl StorageAdapter for RocksDBStorageAdapter {
    async fn get_indexed_height(&self) -> SyncResult<u32> {
        let height_key = b"__INTERNAL/height".to_vec();
        match self.db.get(&height_key) {
            Ok(Some(value)) => {
                if value.len() >= 4 {
                    let height_bytes: [u8; 4] = value[..4]
                        .try_into()
                        .map_err(|_| SyncError::Storage("Invalid height data".to_string()))?;
                    Ok(u32::from_le_bytes(height_bytes))
                } else {
                    Ok(0)
                }
            }
            Ok(None) => Ok(0),
            Err(e) => Err(SyncError::Storage(format!("Database error: {}", e))),
        }
    }

    async fn set_indexed_height(&mut self, height: u32) -> SyncResult<()> {
        let height_key = b"__INTERNAL/height".to_vec();
        let height_bytes = height.to_le_bytes();
        self.db
            .put(&height_key, &height_bytes)
            .map_err(|e| SyncError::Storage(format!("Failed to store height: {}", e)))
    }

    async fn store_block_hash(&mut self, height: u32, hash: &[u8]) -> SyncResult<()> {
        let blockhash_key = format!("/__INTERNAL/height-to-hash/{}", height).into_bytes();
        self.db
            .put(&blockhash_key, hash)
            .map_err(|e| SyncError::Storage(format!("Failed to store blockhash: {}", e)))
    }

    async fn get_block_hash(&self, height: u32) -> SyncResult<Option<Vec<u8>>> {
        let blockhash_key = format!("/__INTERNAL/height-to-hash/{}", height).into_bytes();
        match self.db.get(&blockhash_key) {
            Ok(Some(value)) => Ok(Some(value)),
            Ok(None) => Ok(None),
            Err(e) => Err(SyncError::Storage(format!("Database error: {}", e))),
        }
    }

    async fn store_state_root(&mut self, height: u32, root: &[u8]) -> SyncResult<()> {
        let adapter = RocksDBRuntimeAdapter::new(self.db.clone());
        let mut smt_helper = metashrew_runtime::smt::SMTHelper::new(adapter);
        let root_key = format!("smt:root:{}", height).into_bytes();
        smt_helper
            .storage
            .put(&root_key, root)
            .map_err(|e| SyncError::Storage(format!("Failed to store state root: {}", e)))
    }

    async fn get_state_root(&self, height: u32) -> SyncResult<Option<Vec<u8>>> {
        let adapter = RocksDBRuntimeAdapter::new(self.db.clone());
        let smt_helper = metashrew_runtime::smt::SMTHelper::new(adapter);
        match smt_helper.get_smt_root_at_height(height) {
            Ok(root) => Ok(Some(root.to_vec())),
            Err(_) => Ok(None),
        }
    }

    async fn rollback_to_height(&mut self, height: u32) -> SyncResult<()> {
        use metashrew_runtime::rollback::{rollback_smt_data, rollback_with_manifests};

        info!("Starting rollback to height {}", height);
        let current_height = self.get_indexed_height().await?;

        // Try fast manifest-based rollback first, fall back to full scan
        let used_fast = rollback_with_manifests(self, height, current_height)
            .map_err(|e| SyncError::Storage(format!("Manifest rollback failed: {}", e)))?;

        if !used_fast {
            warn!("Using full SMT rollback (no manifests for rollback range)");
            rollback_smt_data(self, height, current_height)
                .map_err(|e| SyncError::Storage(format!("SMT rollback failed: {}", e)))?;
        }

        self.set_indexed_height(height).await?;
        info!("Successfully completed rollback to height {}", height);
        Ok(())
    }

    async fn is_available(&self) -> bool {
        self.db.get(b"__test").is_ok()
    }

    async fn get_stats(&self) -> SyncResult<StorageStats> {
        let indexed_height = self.get_indexed_height().await?;
        Ok(StorageStats {
            total_entries: 0,
            indexed_height,
            storage_size_bytes: None,
        })
    }

    async fn get_db_handle(&self) -> SyncResult<Arc<DB>> {
        Ok(self.db.clone())
    }
}