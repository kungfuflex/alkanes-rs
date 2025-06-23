//! Common traits for adapter functionality

use anyhow::Result;
use async_trait::async_trait;

/// Trait for managing height tracking across different storage backends
#[async_trait]
pub trait HeightTracker {
    /// Get the current tip height from storage
    async fn get_current_height(&self) -> Result<u32>;
    
    /// Set the current tip height in storage
    async fn set_current_height(&mut self, height: u32) -> Result<()>;
    
    /// Get the indexed height (last processed height)
    async fn get_indexed_height(&self) -> Result<u32>;
    
    /// Set the indexed height
    async fn set_indexed_height(&mut self, height: u32) -> Result<()>;
}

/// Trait for managing state roots across different storage backends
#[async_trait]
pub trait StateRootManager {
    /// Store a state root for a given height
    async fn store_state_root(&self, height: u32, root: &[u8]) -> Result<()>;
    
    /// Get a state root for a given height
    async fn get_state_root(&self, height: u32) -> Result<Option<Vec<u8>>>;
    
    /// Get the latest state root
    async fn get_latest_state_root(&self) -> Result<Option<(u32, Vec<u8>)>>;
}

/// Trait for batch processing operations
pub trait BatchProcessor<B> {
    /// Create a new batch
    fn create_batch(&self) -> B;
    
    /// Write a batch to storage
    fn write_batch(&mut self, batch: B) -> Result<()>;
    
    /// Create an atomic batch that includes height updates
    fn create_atomic_batch(&self, operations: B, new_height: u32) -> B;
}

/// Trait for block hash management
#[async_trait]
pub trait BlockHashManager {
    /// Store a block hash for a given height
    async fn store_block_hash(&self, height: u32, hash: &[u8]) -> Result<()>;
    
    /// Get a block hash for a given height
    async fn get_block_hash(&self, height: u32) -> Result<Option<Vec<u8>>>;
    
    /// Remove block hashes after a given height (for rollbacks)
    async fn remove_block_hashes_after(&self, height: u32) -> Result<()>;
}

/// Combined trait that represents a complete storage adapter
#[async_trait]
pub trait StorageAdapterCore: HeightTracker + StateRootManager + BlockHashManager + Send + Sync {
    /// Check if the storage is available
    async fn is_available(&self) -> bool;
    
    /// Rollback storage to a specific height
    async fn rollback_to_height(&mut self, height: u32) -> Result<()> {
        // Default implementation that calls individual rollback methods
        self.remove_block_hashes_after(height).await?;
        self.set_indexed_height(height).await?;
        self.set_current_height(height).await?;
        Ok(())
    }
}