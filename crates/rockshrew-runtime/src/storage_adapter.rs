//! RocksDB-specific implementation of the `StorageAdapter` trait.

use async_trait::async_trait;
use log::{info, warn};
use metashrew_runtime::KeyValueStoreLike;
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
        info!("Starting comprehensive rollback to height {}", height);
        let current_height = self.get_indexed_height().await?;
        if height >= current_height {
            return Ok(());
        }

        // --- Part 1: Delete metadata for heights above rollback ---
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

        // --- Part 2: Rollback append-only data ---
        // Scan for all keys ending with "/length" to find base keys
        let length_suffix = b"/length";
        let mut base_keys_to_process = Vec::new();

        let iter = self.db.iterator(rocksdb::IteratorMode::Start);
        for item in iter {
            if let Ok((key, _)) = item {
                if key.ends_with(length_suffix) {
                    // Extract base key (everything before "/length")
                    let base_key = key[..key.len() - length_suffix.len()].to_vec();
                    base_keys_to_process.push(base_key);
                }
            }
        }

        info!("Found {} append-only keys to check for rollback", base_keys_to_process.len());

        for base_key in base_keys_to_process {
            let length_key = {
                let mut key = base_key.clone();
                key.extend_from_slice(length_suffix);
                key
            };

            // Get current length
            let old_length = match self.db.get(&length_key) {
                Ok(Some(length_bytes)) => {
                    String::from_utf8_lossy(&length_bytes).parse::<u32>().unwrap_or(0)
                }
                _ => continue,
            };

            // Collect valid updates (those at or before rollback height)
            let mut valid_updates = Vec::new();
            for i in 0..old_length {
                let update_key = {
                    let mut key = base_key.clone();
                    key.extend_from_slice(format!("/{}", i).as_bytes());
                    key
                };

                if let Ok(Some(update_data)) = self.db.get(&update_key) {
                    let update_str = String::from_utf8_lossy(&update_data);
                    if let Some(colon_pos) = update_str.find(':') {
                        let height_str = &update_str[..colon_pos];
                        if let Ok(update_height) = height_str.parse::<u32>() {
                            if update_height <= height {
                                valid_updates.push(update_data.clone());
                            }
                        }
                    }
                }
            }

            // If we removed some updates, rewrite the key
            if valid_updates.len() < old_length as usize {
                // Delete all old entries
                for i in 0..old_length {
                    let update_key = {
                        let mut key = base_key.clone();
                        key.extend_from_slice(format!("/{}", i).as_bytes());
                        key
                    };
                    let _ = self.db.delete(&update_key);
                }

                // Reinsert valid entries
                for (i, update_data) in valid_updates.iter().enumerate() {
                    let update_key = {
                        let mut key = base_key.clone();
                        key.extend_from_slice(format!("/{}", i).as_bytes());
                        key
                    };
                    let _ = self.db.put(&update_key, update_data);
                }

                // Update length
                let new_length = valid_updates.len() as u32;
                if new_length > 0 {
                    let _ = self.db.put(&length_key, new_length.to_string().as_bytes());
                } else {
                    let _ = self.db.delete(&length_key);
                }
            }
        }

        // --- Part 3: Clean up orphaned SMT nodes ---
        // Note: SMT nodes don't have height embedded, so we can't selectively delete them.
        // The orphaned nodes will be naturally unreferenced after rollback since:
        // 1. State roots above rollback height are deleted
        // 2. New block processing will create new nodes from the valid root
        // For now, we accept some orphaned data. A full GC would require tracking node creation heights.
        info!("Note: Orphaned SMT nodes may remain but will be unreferenced");

        self.set_indexed_height(height).await?;
        info!("Successfully completed comprehensive rollback to height {}", height);
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