//! Error types for rockshrew-sync

use thiserror::Error;

#[derive(Error, Debug)]
pub enum SyncError {
    #[error("Bitcoin node error: {0}")]
    BitcoinNode(String),

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Runtime error: {0}")]
    Runtime(String),

    #[error("Chain reorganization error: {0}")]
    Reorg(String),

    #[error("Block processing error at height {height}: {message}")]
    BlockProcessing { height: u32, message: String },

    #[error("View function error: {0}")]
    ViewFunction(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Network error: {0}")]
    Network(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Generic error: {0}")]
    Generic(#[from] anyhow::Error),

    // ==================== Chain Validation Errors (trigger reorg) ====================
    /// Block's prev_blockhash doesn't match stored hash of previous block
    #[error("Chain discontinuity at height {height}: block's prev_blockhash {got} does not match stored hash {expected} of block {prev_height}")]
    ChainDiscontinuity {
        height: u32,
        prev_height: u32,
        expected: String,
        got: String,
    },

    /// Block hash from node doesn't match stored hash (detected during reorg check)
    #[error("Fork detected at height {height}: remote hash {remote_hash} differs from local hash {local_hash}")]
    ForkDetected {
        height: u32,
        local_hash: String,
        remote_hash: String,
    },

    /// Snapshot hash doesn't match remote node hash at same height
    #[error("Snapshot fork at height {height}: snapshot hash {snapshot_hash} doesn't match remote hash {remote_hash}")]
    SnapshotForkDetected {
        height: u32,
        snapshot_hash: String,
        remote_hash: String,
    },

    // ==================== Temporary Errors (should retry) ====================
    /// Temporary node connection failure
    #[error("Temporary node failure: {message}")]
    TemporaryNodeFailure { message: String },

    /// Network timeout
    #[error("Network timeout after {duration_ms}ms: {message}")]
    NetworkTimeout { duration_ms: u64, message: String },

    /// Block data temporarily unavailable
    #[error("Block {height} temporarily unavailable: {message}")]
    BlockTemporarilyUnavailable { height: u32, message: String },

    // ==================== Permanent Errors (should fail) ====================
    /// Invalid block data (malformed, can't deserialize)
    #[error("Invalid block at height {height}: {message}")]
    InvalidBlock { height: u32, message: String },

    /// Corrupted storage detected
    #[error("Corrupted storage at height {height}: {message}")]
    CorruptedStorage { height: u32, message: String },

    /// Configuration is invalid
    #[error("Invalid configuration: {message}")]
    InvalidConfig { message: String },

    /// Rollback depth exceeded max_reorg_depth
    #[error("Rollback depth {depth} exceeds max allowed depth {max_depth}")]
    RollbackDepthExceeded { depth: u32, max_depth: u32 },
}

impl SyncError {
    /// Returns true if this error should trigger reorg handling
    pub fn should_trigger_reorg(&self) -> bool {
        matches!(
            self,
            SyncError::ChainDiscontinuity { .. }
                | SyncError::ForkDetected { .. }
                | SyncError::SnapshotForkDetected { .. }
        )
    }

    /// Returns true if this error is temporary and should be retried
    pub fn should_retry(&self) -> bool {
        matches!(
            self,
            SyncError::TemporaryNodeFailure { .. }
                | SyncError::NetworkTimeout { .. }
                | SyncError::BlockTemporarilyUnavailable { .. }
                | SyncError::Network(_)
        )
    }

    /// Returns true if this error is permanent and sync should stop
    pub fn is_permanent(&self) -> bool {
        matches!(
            self,
            SyncError::InvalidBlock { .. }
                | SyncError::CorruptedStorage { .. }
                | SyncError::InvalidConfig { .. }
                | SyncError::RollbackDepthExceeded { .. }
        )
    }

    /// Extract the height from errors that have it
    pub fn height(&self) -> Option<u32> {
        match self {
            SyncError::BlockProcessing { height, .. }
            | SyncError::ChainDiscontinuity { height, .. }
            | SyncError::ForkDetected { height, .. }
            | SyncError::SnapshotForkDetected { height, .. }
            | SyncError::BlockTemporarilyUnavailable { height, .. }
            | SyncError::InvalidBlock { height, .. }
            | SyncError::CorruptedStorage { height, .. } => Some(*height),
            _ => None,
        }
    }
}

pub type SyncResult<T> = Result<T, SyncError>;