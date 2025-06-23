//! Generic traits for synchronization systems

use anyhow::Result;
use async_trait::async_trait;

/// Generic trait for blockchain synchronization
#[async_trait]
pub trait Synchronizer {
    /// Start synchronization from the given height
    async fn start_sync(&mut self, start_height: u32) -> Result<()>;
    
    /// Stop synchronization
    async fn stop_sync(&mut self) -> Result<()>;
    
    /// Get current synchronization status
    async fn get_sync_status(&self) -> Result<SyncStatus>;
}

/// Synchronization status information
#[derive(Debug, Clone)]
pub struct SyncStatus {
    pub current_height: u32,
    pub target_height: u32,
    pub is_syncing: bool,
    pub blocks_per_second: f64,
}

/// Generic trait for block processing
#[async_trait]
pub trait BlockProcessor {
    /// Process a single block
    async fn process_block(&mut self, height: u32, block_data: &[u8]) -> Result<()>;
    
    /// Handle blockchain reorganization
    async fn handle_reorg(&mut self, from_height: u32, to_height: u32) -> Result<()>;
}

/// Generic trait for data storage
#[async_trait]
pub trait DataStore {
    /// Store data at the given key
    async fn store(&mut self, key: &[u8], value: &[u8]) -> Result<()>;
    
    /// Retrieve data for the given key
    async fn retrieve(&self, key: &[u8]) -> Result<Option<Vec<u8>>>;
    
    /// Delete data for the given key
    async fn delete(&mut self, key: &[u8]) -> Result<()>;
    
    /// Begin a transaction
    async fn begin_transaction(&mut self) -> Result<()>;
    
    /// Commit the current transaction
    async fn commit_transaction(&mut self) -> Result<()>;
    
    /// Rollback the current transaction
    async fn rollback_transaction(&mut self) -> Result<()>;
}