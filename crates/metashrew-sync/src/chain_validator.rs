//! Chain validation logic
//!
//! This module provides validation for blockchain continuity and integrity.

use crate::error::{SyncError, SyncResult};
use crate::traits::{BitcoinNodeAdapter, StorageAdapter};
use bitcoin::consensus::deserialize;
use bitcoin::hashes::Hash;
use bitcoin::Block;
use log::{debug, error, warn};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Validates blockchain continuity and integrity
///
/// ChainValidator ensures that:
/// - Each block connects to its predecessor (prev_blockhash validation)
/// - Block ranges form valid chains
/// - Snapshots match the remote chain
pub struct ChainValidator<S>
where
    S: StorageAdapter + 'static,
{
    storage: Arc<RwLock<S>>,
}

impl<S> ChainValidator<S>
where
    S: StorageAdapter + 'static,
{
    /// Create a new ChainValidator
    pub fn new(storage: Arc<RwLock<S>>) -> Self {
        Self { storage }
    }

    /// Validate that a single block connects to the previous block
    ///
    /// Returns Ok(()) if valid, or ChainDiscontinuity error if not.
    /// Genesis block (height 0) is always considered valid.
    pub async fn validate_single_block(
        &self,
        height: u32,
        block_data: &[u8],
    ) -> SyncResult<()> {
        // Genesis block has no previous block
        if height == 0 {
            return Ok(());
        }

        // Deserialize block
        let block: Block = deserialize(block_data).map_err(|e| SyncError::InvalidBlock {
            height,
            message: format!("Failed to deserialize block: {}", e),
        })?;

        let block_prev_hash_bytes = block.header.prev_blockhash.to_byte_array().to_vec();

        // Get stored hash of previous block
        let storage = self.storage.read().await;
        let stored_prev_hash = storage.get_block_hash(height - 1).await.map_err(|e| {
            SyncError::Storage(format!(
                "Failed to get block hash for height {}: {}",
                height - 1,
                e
            ))
        })?;
        drop(storage);

        match stored_prev_hash {
            Some(stored_hash) => {
                if stored_hash != block_prev_hash_bytes {
                    // Chain discontinuity detected
                    error!(
                        "⚠ CHAIN DISCONTINUITY at height {}: Block's prev_blockhash {} does not match stored hash {} of block {}",
                        height,
                        hex::encode(&block_prev_hash_bytes),
                        hex::encode(&stored_hash),
                        height - 1
                    );

                    return Err(SyncError::ChainDiscontinuity {
                        height,
                        prev_height: height - 1,
                        expected: hex::encode(&stored_hash),
                        got: hex::encode(&block_prev_hash_bytes),
                    });
                }

                debug!(
                    "✓ Block {} connects to previous block {} (hash: {})",
                    height,
                    height - 1,
                    hex::encode(&block_prev_hash_bytes[..8])
                );

                Ok(())
            }
            None => {
                // Previous block hash not found in storage
                warn!(
                    "No stored hash for block {} - unable to validate chain continuity for block {}",
                    height - 1, height
                );

                // Allow processing to continue (this can happen during catch-up)
                // but caller should be aware validation was skipped
                Ok(())
            }
        }
    }

    /// Validate that a range of blocks forms a valid chain
    ///
    /// Checks that each block in [from_height, to_height] connects to its predecessor.
    /// Returns Ok(()) if entire range is valid, or the first error encountered.
    pub async fn validate_chain_range(
        &self,
        blocks: &[(u32, Vec<u8>)], // (height, block_data) pairs
    ) -> SyncResult<()> {
        if blocks.is_empty() {
            return Ok(());
        }

        // Validate each block in sequence
        for (height, block_data) in blocks {
            self.validate_single_block(*height, block_data).await?;
        }

        debug!(
            "✓ Validated chain range: {} blocks from {} to {}",
            blocks.len(),
            blocks.first().unwrap().0,
            blocks.last().unwrap().0
        );

        Ok(())
    }

    /// Validate that a snapshot's block hash matches the remote node's hash
    ///
    /// This prevents applying a snapshot that's on a different fork.
    pub async fn validate_snapshot_fork<N>(
        &self,
        snapshot_height: u32,
        snapshot_hash: &[u8],
        node: &N,
    ) -> SyncResult<()>
    where
        N: BitcoinNodeAdapter,
    {
        // Get remote hash at snapshot height
        let remote_hash = node
            .get_block_hash(snapshot_height)
            .await
            .map_err(|e| SyncError::BitcoinNode(format!("Failed to get remote hash: {}", e)))?;

        if snapshot_hash != remote_hash {
            error!(
                "Snapshot fork detected at height {}: snapshot={}, remote={}",
                snapshot_height,
                hex::encode(snapshot_hash),
                hex::encode(&remote_hash)
            );

            return Err(SyncError::SnapshotForkDetected {
                height: snapshot_height,
                snapshot_hash: hex::encode(snapshot_hash),
                remote_hash: hex::encode(&remote_hash),
            });
        }

        debug!(
            "✓ Snapshot at height {} matches remote chain (hash: {})",
            snapshot_height,
            hex::encode(&snapshot_hash[..8])
        );

        Ok(())
    }

    /// Check if a block is internally valid (deserializes correctly)
    ///
    /// This is a lightweight check that doesn't require storage access.
    pub fn validate_block_structure(height: u32, block_data: &[u8]) -> SyncResult<Block> {
        deserialize(block_data).map_err(|e| SyncError::InvalidBlock {
            height,
            message: format!("Malformed block data: {}", e),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::BlockHash;

    #[test]
    fn test_validate_block_structure() {
        // Test with invalid data
        let invalid_data = vec![0u8; 10];
        let result = ChainValidator::<DummyStorage>::validate_block_structure(100, &invalid_data);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), SyncError::InvalidBlock { .. }));
    }

    // Dummy storage for testing static methods
    struct DummyStorage;

    #[async_trait::async_trait]
    impl StorageAdapter for DummyStorage {
        async fn get_indexed_height(&self) -> SyncResult<u32> {
            unimplemented!()
        }
        async fn set_indexed_height(&mut self, _height: u32) -> SyncResult<()> {
            unimplemented!()
        }
        async fn store_block_hash(&mut self, _height: u32, _hash: &[u8]) -> SyncResult<()> {
            unimplemented!()
        }
        async fn get_block_hash(&self, _height: u32) -> SyncResult<Option<Vec<u8>>> {
            unimplemented!()
        }
        async fn store_state_root(&mut self, _height: u32, _root: &[u8]) -> SyncResult<()> {
            unimplemented!()
        }
        async fn get_state_root(&self, _height: u32) -> SyncResult<Option<Vec<u8>>> {
            unimplemented!()
        }
        async fn rollback_to_height(&mut self, _height: u32) -> SyncResult<()> {
            unimplemented!()
        }
        async fn is_available(&self) -> bool {
            unimplemented!()
        }
        async fn get_stats(&self) -> SyncResult<crate::types::StorageStats> {
            unimplemented!()
        }
    }
}
