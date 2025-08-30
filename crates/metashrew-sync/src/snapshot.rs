//! # Snapshot and Repository Mode Infrastructure
//!
//! This module provides comprehensive infrastructure for snapshot-based synchronization,
//! enabling fast bootstrap and distributed indexing architectures. The system supports
//! both snapshot creation and consumption, allowing one instance to create periodic
//! snapshots while other instances can rapidly sync from those snapshots.
//!
//! ## Core Concepts
//!
//! ### Snapshot-Based Synchronization
//! Traditional blockchain synchronization requires processing every block from genesis,
//! which can take days or weeks for Bitcoin. Snapshot-based sync enables:
//! - **Fast Bootstrap**: Start indexing from a recent snapshot instead of genesis
//! - **Distributed Architecture**: Multiple indexers sharing snapshot data
//! - **Backup and Recovery**: Reliable state backup and restoration
//! - **Development Efficiency**: Quick setup for testing and development
//!
//! ### Repository Mode
//! Repository mode enables a distributed architecture where:
//! - **Snapshot Providers**: Create and serve snapshots at regular intervals
//! - **Snapshot Consumers**: Download and apply snapshots for fast sync
//! - **Hybrid Operation**: Instances can both create and consume snapshots
//! - **Automatic Fallback**: Seamless transition to normal sync after catching up
//!
//! ## Architecture Overview
//!
//! ### Snapshot Creation Pipeline
//! 1. **State Capture**: Extract current indexed state at specific heights
//! 2. **Compression**: Compress state data for efficient storage and transfer
//! 3. **Metadata Generation**: Create checksums and verification data
//! 4. **Storage**: Store snapshots locally or serve via HTTP
//! 5. **Cleanup**: Remove old snapshots based on retention policies
//!
//! ### Snapshot Consumption Pipeline
//! 1. **Discovery**: Find available snapshots from repositories
//! 2. **Selection**: Choose optimal snapshot based on current state
//! 3. **Download**: Retrieve snapshot data with integrity verification
//! 4. **Verification**: Validate checksums and state consistency
//! 5. **Application**: Apply snapshot to local storage
//! 6. **Sync Continuation**: Resume normal sync from snapshot height
//!
//! ## Usage Examples
//!
//! ### Creating Snapshots
//! ```rust,ignore
//! use metashrew_sync::snapshot::*;
//!
//! // Configure snapshot creation
//! let config = SnapshotConfig {
//!     snapshot_interval: 1000,  // Every 1000 blocks
//!     max_snapshots: 10,        // Keep 10 snapshots
//!     compression_level: 6,     // Balanced compression
//!     reorg_buffer_size: 100,   // 100 blocks for reorg detection
//! };
//!
//! // Create snapshot provider
//! let mut provider = MySnapshotProvider::new(config);
//!
//! // Check if snapshot should be created
//! if provider.should_create_snapshot(current_height) {
//!     let metadata = provider.create_snapshot(current_height).await?;
//!     println!("Created snapshot at height {}", metadata.height);
//! }
//! ```
//!
//! ### Consuming Snapshots
//! ```rust,ignore
//! use metashrew_sync::snapshot::*;
//!
//! // Configure repository mode
//! let config = RepoConfig {
//!     repo_url: "https://snapshots.example.com".to_string(),
//!     check_interval: 300,      // Check every 5 minutes
//!     max_snapshot_age: 86400,  // Accept snapshots up to 24 hours old
//!     continue_sync: true,      // Continue normal sync after catching up
//!     min_blocks_behind: 100,   // Use snapshots if >100 blocks behind
//! };
//!
//! // Create snapshot consumer
//! let mut consumer = MySnapshotConsumer::new(config);
//!
//! // Check if snapshots should be used
//! if consumer.should_use_snapshots(current_height, tip_height).await? {
//!     if let Some(metadata) = consumer.get_best_snapshot(current_height, tip_height).await? {
//!         consumer.apply_snapshot(&metadata).await?;
//!         println!("Applied snapshot from height {}", metadata.height);
//!     }
//! }
//! ```
//!
//! ### Serving Snapshots
//! ```rust,ignore
//! use metashrew_sync::snapshot::*;
//!
//! // Create snapshot server
//! let mut server = MySnapshotServer::new();
//! server.start().await?;
//!
//! // Register new snapshots
//! server.register_snapshot(metadata, snapshot_data).await?;
//!
//! // Server automatically handles HTTP requests for snapshot data
//! ```
//!
//! ## Key Features
//!
//! ### Data Integrity
//! - **Cryptographic Checksums**: SHA-256 verification of snapshot data
//! - **State Root Validation**: Consistency checks using SMT state roots
//! - **WASM Module Verification**: Ensure snapshots match indexer version
//! - **Reorg Protection**: Include recent block hashes for fork detection
//!
//! ### Performance Optimization
//! - **Compression**: Configurable compression levels for size/speed tradeoffs
//! - **Streaming**: Large snapshots can be streamed for memory efficiency
//! - **Parallel Processing**: Concurrent snapshot creation and consumption
//! - **Incremental Updates**: Delta snapshots for efficient updates
//!
//! ### Operational Features
//! - **Automatic Cleanup**: Configurable retention policies for old snapshots
//! - **Health Monitoring**: Status reporting and availability checks
//! - **Error Recovery**: Robust error handling and retry mechanisms
//! - **Metrics Collection**: Comprehensive statistics for monitoring
//!
//! ## Integration Patterns
//!
//! ### Production Deployment
//! - **Primary Indexer**: Creates snapshots while processing new blocks
//! - **Replica Indexers**: Bootstrap from snapshots for fast deployment
//! - **CDN Distribution**: Serve snapshots via content delivery networks
//! - **Backup Strategy**: Regular snapshots for disaster recovery
//!
//! ### Development Workflow
//! - **Quick Setup**: Developers can start from recent snapshots
//! - **Testing**: Consistent test environments using snapshot data
//! - **Debugging**: Reproduce issues from specific blockchain states
//! - **Performance Testing**: Benchmark indexers with realistic data

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{SyncError, SyncResult};

/// Snapshot metadata containing information about a checkpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMetadata {
    /// Height at which this snapshot was taken
    pub height: u32,
    /// Block hash at this height
    pub block_hash: Vec<u8>,
    /// State root at this height
    pub state_root: Vec<u8>,
    /// Timestamp when snapshot was created
    pub timestamp: u64,
    /// Size of the snapshot data in bytes
    pub size_bytes: u64,
    /// Checksum of the snapshot data
    pub checksum: String,
    /// WASM module hash used for this snapshot
    pub wasm_hash: String,
}

/// Snapshot data containing the actual state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotData {
    /// Metadata about this snapshot
    pub metadata: SnapshotMetadata,
    /// Raw state data (compressed)
    pub state_data: Vec<u8>,
    /// Block hashes for recent blocks (for reorg detection)
    pub recent_block_hashes: HashMap<u32, Vec<u8>>,
}

/// Configuration for snapshot creation
#[derive(Debug, Clone)]
pub struct SnapshotConfig {
    /// Interval between snapshots (in blocks)
    pub snapshot_interval: u32,
    /// Maximum number of snapshots to keep
    pub max_snapshots: usize,
    /// Compression level (0-9)
    pub compression_level: u32,
    /// Number of recent blocks to include for reorg detection
    pub reorg_buffer_size: u32,
}

impl Default for SnapshotConfig {
    fn default() -> Self {
        Self {
            snapshot_interval: 1000,
            max_snapshots: 10,
            compression_level: 6,
            reorg_buffer_size: 100,
        }
    }
}

/// Configuration for repository mode
#[derive(Debug, Clone)]
pub struct RepoConfig {
    /// Base URL or path for the snapshot repository
    pub repo_url: String,
    /// Interval to check for new snapshots (in seconds)
    pub check_interval: u64,
    /// Maximum age of snapshots to consider (in seconds)
    pub max_snapshot_age: u64,
    /// Whether to continue syncing after catching up to snapshots
    pub continue_sync: bool,
    /// Minimum blocks behind before using snapshots
    pub min_blocks_behind: u32,
}

impl Default for RepoConfig {
    fn default() -> Self {
        Self {
            repo_url: "http://localhost:8080/snapshots".to_string(),
            check_interval: 300,     // 5 minutes
            max_snapshot_age: 86400, // 24 hours
            continue_sync: true,
            min_blocks_behind: 100,
        }
    }
}

/// Trait for creating and managing snapshots
#[async_trait]
pub trait SnapshotProvider: Send + Sync {
    /// Create a snapshot at the current height
    async fn create_snapshot(&mut self, height: u32) -> SyncResult<SnapshotMetadata>;

    /// Get available snapshots
    async fn list_snapshots(&self) -> SyncResult<Vec<SnapshotMetadata>>;

    /// Get a specific snapshot by height
    async fn get_snapshot(&self, height: u32) -> SyncResult<Option<SnapshotData>>;

    /// Get the latest snapshot
    async fn get_latest_snapshot(&self) -> SyncResult<Option<SnapshotData>>;

    /// Delete old snapshots beyond the configured limit
    async fn cleanup_snapshots(&mut self) -> SyncResult<usize>;

    /// Check if a snapshot should be created at this height
    fn should_create_snapshot(&self, height: u32) -> bool;
}

/// Trait for consuming snapshots in repository mode
#[async_trait]
pub trait SnapshotConsumer: Send + Sync {
    /// Check for available snapshots from the repository
    async fn check_available_snapshots(&self) -> SyncResult<Vec<SnapshotMetadata>>;

    /// Download and apply a snapshot
    async fn apply_snapshot(&mut self, metadata: &SnapshotMetadata) -> SyncResult<()>;

    /// Get the best snapshot to use for catching up
    async fn get_best_snapshot(
        &self,
        current_height: u32,
        tip_height: u32,
    ) -> SyncResult<Option<SnapshotMetadata>>;

    /// Verify a snapshot's integrity
    async fn verify_snapshot(&self, data: &SnapshotData) -> SyncResult<bool>;

    /// Check if we should use snapshots given current state
    async fn should_use_snapshots(&self, current_height: u32, tip_height: u32) -> SyncResult<bool>;
}

/// Trait for serving snapshots over HTTP or filesystem
#[async_trait]
pub trait SnapshotServer: Send + Sync {
    /// Start the snapshot server
    async fn start(&mut self) -> SyncResult<()>;

    /// Stop the snapshot server
    async fn stop(&mut self) -> SyncResult<()>;

    /// Get server status
    async fn get_status(&self) -> SyncResult<SnapshotServerStatus>;

    /// Register a new snapshot
    async fn register_snapshot(
        &mut self,
        metadata: SnapshotMetadata,
        data: Vec<u8>,
    ) -> SyncResult<()>;

    /// Get snapshot metadata by height
    async fn get_snapshot_metadata(&self, height: u32) -> SyncResult<Option<SnapshotMetadata>>;

    /// Get snapshot data by height
    async fn get_snapshot_data(&self, height: u32) -> SyncResult<Option<Vec<u8>>>;

    /// List all available snapshots
    async fn list_available_snapshots(&self) -> SyncResult<Vec<SnapshotMetadata>>;
}

/// Status of a snapshot server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotServerStatus {
    pub is_running: bool,
    pub total_snapshots: usize,
    pub latest_snapshot_height: Option<u32>,
    pub total_size_bytes: u64,
    pub uptime_seconds: u64,
}

/// Trait for HTTP client to download snapshots
#[async_trait]
pub trait SnapshotClient: Send + Sync {
    /// Download snapshot metadata from URL
    async fn download_metadata(&self, url: &str) -> SyncResult<SnapshotMetadata>;

    /// Download snapshot data from URL
    async fn download_data(&self, url: &str) -> SyncResult<Vec<u8>>;

    /// List available snapshots from repository
    async fn list_remote_snapshots(&self, base_url: &str) -> SyncResult<Vec<SnapshotMetadata>>;

    /// Check if repository is available
    async fn check_repository(&self, base_url: &str) -> SyncResult<bool>;
}

/// Combined sync mode that can operate in both snapshot and repo modes
#[derive(Debug, Clone)]
pub enum SyncMode {
    /// Normal synchronization mode
    Normal,
    /// Snapshot creation mode
    Snapshot(SnapshotConfig),
    /// Repository consumption mode
    Repo(RepoConfig),
    /// Combined mode (create snapshots and serve them)
    SnapshotServer(SnapshotConfig),
}

/// Sync engine that supports snapshot and repository modes
#[async_trait]
pub trait SnapshotSyncEngine: Send + Sync {
    /// Get current sync mode
    fn get_sync_mode(&self) -> &SyncMode;

    /// Switch sync mode
    async fn set_sync_mode(&mut self, mode: SyncMode) -> SyncResult<()>;

    /// Process a block with snapshot considerations
    async fn process_block_with_snapshots(
        &mut self,
        height: u32,
        block_data: &[u8],
    ) -> SyncResult<()>;

    /// Check and apply snapshots if in repo mode
    async fn check_and_apply_snapshots(&mut self) -> SyncResult<bool>;

    /// Create snapshot if in snapshot mode
    async fn create_snapshot_if_needed(&mut self, height: u32) -> SyncResult<bool>;

    /// Get sync statistics including snapshot info
    async fn get_snapshot_stats(&self) -> SyncResult<SnapshotSyncStats>;

    /// Process the next block in the sync process
    async fn process_next_block(&mut self) -> SyncResult<Option<u32>>;
}

/// Statistics for snapshot-enabled sync
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotSyncStats {
    pub current_height: u32,
    pub tip_height: u32,
    pub sync_mode: String,
    pub snapshots_created: u32,
    pub snapshots_applied: u32,
    pub last_snapshot_height: Option<u32>,
    pub blocks_synced_normally: u32,
    pub blocks_synced_from_snapshots: u32,
}

/// Error types specific to snapshot operations
#[derive(Debug, thiserror::Error)]
pub enum SnapshotError {
    #[error("Snapshot not found at height {height}")]
    SnapshotNotFound { height: u32 },

    #[error("Invalid snapshot data: {reason}")]
    InvalidSnapshot { reason: String },

    #[error("Snapshot verification failed: {reason}")]
    VerificationFailed { reason: String },

    #[error("Repository unavailable: {url}")]
    RepositoryUnavailable { url: String },

    #[error("Compression error: {message}")]
    CompressionError { message: String },

    #[error("Network error: {message}")]
    NetworkError { message: String },

    #[error("Filesystem error: {message}")]
    FilesystemError { message: String },
}

impl From<SnapshotError> for SyncError {
    fn from(err: SnapshotError) -> Self {
        SyncError::Runtime(err.to_string())
    }
}
