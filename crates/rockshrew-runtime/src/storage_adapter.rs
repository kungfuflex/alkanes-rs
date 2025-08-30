//! RocksDB-specific implementation of the `StorageAdapter` trait.

use async_trait::async_trait;
use log::{info, warn};
use metashrew_runtime::KeyValueStoreLike;
use metashrew_sync::{StorageAdapter, StorageStats, SyncError, SyncResult};
use rocksdb::DB;
use std::sync::Arc;

use crate::adapter::RocksDBRuntimeAdapter;
use crate::retry::with_retry;

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
        with_retry(&self.db, |db| db.put(&height_key, &height_bytes))
            .map_err(|e| SyncError::Storage(format!("Failed to store height: {}", e)))
    }

    async fn store_block_hash(&mut self, height: u32, hash: &[u8]) -> SyncResult<()> {
        let blockhash_key = format!("/__INTERNAL/height-to-hash/{}", height).into_bytes();
        with_retry(&self.db, |db| db.put(&blockhash_key, hash))
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
        info!("Starting rollback to height {}", height);
        let current_height = self.get_indexed_height().await?;
        if height >= current_height {
            return Ok(());
        }
        for h in (height + 1)..=current_height {
            let blockhash_key = format!("/__INTERNAL/height-to-hash/{}", h).into_bytes();
            if let Err(e) = self.db.delete(&blockhash_key) {
                warn!("Failed to delete blockhash for height {}: {}", h, e);
            }
            let root_key = format!("smt:root:{}", h).into_bytes();
            if let Err(e) = self.db.delete(&root_key) {
                warn!("Failed to delete state root for height {}: {}", h, e);
            }
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
