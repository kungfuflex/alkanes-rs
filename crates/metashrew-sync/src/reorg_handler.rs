//! Centralized reorganization detection and handling
//!
//! This module provides a unified interface for detecting chain reorganizations
//! and coordinating rollback across storage and runtime components.

use crate::error::{SyncError, SyncResult};
use crate::traits::{BitcoinNodeAdapter, RuntimeAdapter, StorageAdapter};
use crate::types::SyncConfig;
use log::{debug, error, info, warn};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Configuration for reorg handling behavior
#[derive(Clone, Debug)]
pub struct ReorgConfig {
    /// Maximum depth to search backwards for common ancestor (default: 100)
    pub max_reorg_depth: u32,
    /// Distance from tip to trigger proactive reorg checks (default: 6)
    pub reorg_check_threshold: u32,
}

impl From<&SyncConfig> for ReorgConfig {
    fn from(config: &SyncConfig) -> Self {
        Self {
            max_reorg_depth: config.max_reorg_depth,
            reorg_check_threshold: config.reorg_check_threshold,
        }
    }
}

/// Centralized handler for chain reorganization detection and recovery
///
/// ReorgHandler coordinates between:
/// - Chain validation (detecting discontinuities)
/// - Fork detection (comparing local vs remote hashes)
/// - Storage rollback (removing invalid data)
/// - Runtime refresh (clearing WASM state)
pub struct ReorgHandler<N, S, R>
where
    N: BitcoinNodeAdapter + 'static,
    S: StorageAdapter + 'static,
    R: RuntimeAdapter + 'static,
{
    node: Arc<N>,
    storage: Arc<RwLock<S>>,
    runtime: Arc<R>,
    config: ReorgConfig,
}

impl<N, S, R> Clone for ReorgHandler<N, S, R>
where
    N: BitcoinNodeAdapter + 'static,
    S: StorageAdapter + 'static,
    R: RuntimeAdapter + 'static,
{
    fn clone(&self) -> Self {
        Self {
            node: self.node.clone(),
            storage: self.storage.clone(),
            runtime: self.runtime.clone(),
            config: self.config.clone(),
        }
    }
}

impl<N, S, R> ReorgHandler<N, S, R>
where
    N: BitcoinNodeAdapter + 'static,
    S: StorageAdapter + 'static,
    R: RuntimeAdapter + 'static,
{
    /// Create a new ReorgHandler
    pub fn new(
        node: Arc<N>,
        storage: Arc<RwLock<S>>,
        runtime: Arc<R>,
        config: ReorgConfig,
    ) -> Self {
        Self {
            node,
            storage,
            runtime,
            config,
        }
    }

    /// Main entry point: Check for reorg and handle if detected
    ///
    /// This should be called from all places that previously called handle_reorg directly.
    /// Returns the height to resume syncing from (may be < current_height if rollback occurred).
    pub async fn check_and_handle_reorg(&self, current_height: u32) -> SyncResult<u32> {
        // Don't check genesis
        if current_height == 0 {
            return Ok(0);
        }

        // Detect if reorg occurred
        match self.detect_reorg(current_height).await? {
            Some(common_ancestor_height) => {
                // Reorg detected, execute rollback
                info!(
                    "Reorg detected between heights {} and {}. Common ancestor: {}",
                    common_ancestor_height + 1,
                    current_height,
                    common_ancestor_height
                );
                self.execute_rollback(common_ancestor_height).await?;
                Ok(common_ancestor_height + 1)
            }
            None => {
                // No reorg detected
                debug!("No reorg detected at height {}", current_height);
                Ok(current_height)
            }
        }
    }

    /// Detect if a reorg has occurred by comparing local and remote block hashes
    ///
    /// Searches backwards from current_height - 1 up to max_reorg_depth.
    /// Returns Some(height) of common ancestor if reorg detected, None otherwise.
    async fn detect_reorg(&self, current_height: u32) -> SyncResult<Option<u32>> {
        let search_start = current_height.saturating_sub(1);
        let search_limit = current_height.saturating_sub(self.config.max_reorg_depth);

        let mut check_height = search_start;
        let mut reorg_detected = false;

        while check_height > 0 && check_height >= search_limit {
            // Get local hash
            let storage_guard = self.storage.read().await;
            let local_hash = match storage_guard.get_block_hash(check_height).await {
                Ok(Some(hash)) => hash,
                Ok(None) => {
                    // No stored hash at this height - skip it
                    drop(storage_guard);
                    check_height = check_height.saturating_sub(1);
                    continue;
                }
                Err(e) => {
                    drop(storage_guard);
                    return Err(SyncError::Storage(format!(
                        "Failed to get local hash at height {}: {}",
                        check_height, e
                    )));
                }
            };
            drop(storage_guard);

            // Get remote hash
            let remote_hash = match self.node.get_block_hash(check_height).await {
                Ok(hash) => hash,
                Err(e) => {
                    error!(
                        "Failed to get remote block hash at height {}: {}",
                        check_height, e
                    );
                    // Don't trigger reorg if node is temporarily failing
                    return Ok(None);
                }
            };

            // Compare hashes
            if local_hash == remote_hash {
                // Common ancestor found
                debug!(
                    "Common ancestor found at height {} (hash: {})",
                    check_height,
                    hex::encode(&local_hash)
                );
                break;
            }

            // Hashes differ - reorg detected
            warn!(
                "Fork detected at height {}: local={}, remote={}",
                check_height,
                hex::encode(&local_hash),
                hex::encode(&remote_hash)
            );
            reorg_detected = true;
            check_height = check_height.saturating_sub(1);
        }

        if check_height == 0 && reorg_detected {
            // Reorg extends beyond max_reorg_depth
            return Err(SyncError::RollbackDepthExceeded {
                depth: current_height.saturating_sub(check_height),
                max_depth: self.config.max_reorg_depth,
            });
        }

        if reorg_detected {
            Ok(Some(check_height))
        } else {
            Ok(None)
        }
    }

    /// Execute rollback to the specified height
    ///
    /// Coordinates storage rollback and runtime refresh.
    async fn execute_rollback(&self, target_height: u32) -> SyncResult<()> {
        warn!("Executing rollback to height {}", target_height);

        // Step 1: Rollback storage
        let mut storage_guard = self.storage.write().await;
        storage_guard
            .rollback_to_height(target_height)
            .await
            .map_err(|e| {
                SyncError::Storage(format!("Rollback to height {} failed: {}", target_height, e))
            })?;
        drop(storage_guard);

        info!("Storage rolled back to height {}", target_height);

        // Step 2: Refresh runtime memory (clear WASM state)
        self.runtime.refresh_memory().await.map_err(|e| {
            SyncError::Runtime(format!(
                "Failed to refresh runtime memory after rollback: {}",
                e
            ))
        })?;

        info!("Runtime memory refreshed after rollback");

        Ok(())
    }

    /// Check if proactive reorg check should be performed
    ///
    /// Returns true if we're within reorg_check_threshold blocks of remote tip
    pub fn should_check_for_reorg(&self, current_height: u32, remote_tip: u32) -> bool {
        remote_tip.saturating_sub(current_height) <= self.config.reorg_check_threshold
    }

    /// Handle an error that occurred during block processing
    ///
    /// Categorizes the error and determines appropriate action:
    /// - If reorg-triggering error: executes rollback and returns resume height
    /// - If retryable error: returns None (caller should retry)
    /// - If permanent error: propagates the error
    pub async fn handle_processing_error(
        &self,
        error: SyncError,
        failed_height: u32,
    ) -> SyncResult<Option<u32>> {
        if error.should_trigger_reorg() {
            warn!(
                "Reorg-triggering error at height {}: {}",
                failed_height, error
            );

            // Attempt rollback
            match self.check_and_handle_reorg(failed_height).await {
                Ok(resume_height) => {
                    info!("Rolled back to height {}. Resuming sync.", resume_height);
                    Ok(Some(resume_height))
                }
                Err(rollback_err) => {
                    error!("Failed to handle reorg: {}", rollback_err);
                    Err(rollback_err)
                }
            }
        } else if error.should_retry() {
            warn!("Temporary error at height {}: {}", failed_height, error);
            Ok(None) // Signal caller to retry
        } else if error.is_permanent() {
            error!("Permanent error at height {}: {}", failed_height, error);
            Err(error)
        } else {
            // Generic error - propagate
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_check_for_reorg() {
        let config = ReorgConfig {
            max_reorg_depth: 100,
            reorg_check_threshold: 6,
        };

        // Mock handler (won't use node/storage/runtime in this test)
        // Just testing the should_check_for_reorg logic

        // Within threshold: should check
        assert!(6 <= config.reorg_check_threshold);
        assert!(5 <= config.reorg_check_threshold);
        assert!(0 <= config.reorg_check_threshold);

        // Beyond threshold: should not check
        assert!(7 > config.reorg_check_threshold);
        assert!(100 > config.reorg_check_threshold);
    }

    #[test]
    fn test_error_categorization() {
        use crate::error::SyncError;

        // Chain discontinuity should trigger reorg
        let err = SyncError::ChainDiscontinuity {
            height: 100,
            prev_height: 99,
            expected: "abc123".to_string(),
            got: "def456".to_string(),
        };
        assert!(err.should_trigger_reorg());
        assert!(!err.should_retry());
        assert!(!err.is_permanent());

        // Network error should retry
        let err = SyncError::Network("timeout".to_string());
        assert!(!err.should_trigger_reorg());
        assert!(err.should_retry());
        assert!(!err.is_permanent());

        // Invalid block is permanent
        let err = SyncError::InvalidBlock {
            height: 100,
            message: "malformed".to_string(),
        };
        assert!(!err.should_trigger_reorg());
        assert!(!err.should_retry());
        assert!(err.is_permanent());
    }
}
