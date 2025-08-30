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
}

pub type SyncResult<T> = Result<T, SyncError>;
