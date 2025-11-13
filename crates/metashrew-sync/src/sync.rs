//! # Core Synchronization Engine Implementation
//!
//! This module provides the main synchronization engine that coordinates Bitcoin blockchain
//! indexing using the adapter pattern. The [`MetashrewSync`] engine orchestrates the
//! interaction between Bitcoin nodes, storage backends, and WASM runtime environments
//! to provide reliable, high-performance blockchain indexing.
//!
//! ## Architecture Overview
//!
//! The synchronization engine implements a pipeline architecture with the following components:
//!
//! ### Pipeline Processing
//! - **Block Fetcher**: Asynchronously fetches blocks from Bitcoin nodes
//! - **Block Processor**: Processes blocks through WASM indexer modules
//! - **Result Handler**: Manages processing results and error recovery
//! - **Atomic Operations**: Ensures data consistency through atomic block processing
//!
//! ### Concurrency Model
//! - **Parallel Fetching**: Blocks are fetched in parallel to maximize throughput
//! - **Async Processing**: Non-blocking I/O operations throughout the pipeline
//! - **Thread Safety**: Safe concurrent access to shared state using atomic operations
//! - **Backpressure**: Automatic flow control to prevent memory exhaustion
//!
//! ### Error Recovery
//! - **Graceful Degradation**: Fallback from atomic to non-atomic processing
//! - **Retry Logic**: Automatic retry of failed operations with exponential backoff
//! - **Chain Reorganization**: Detection and handling of blockchain forks
//! - **State Consistency**: Rollback capabilities for maintaining data integrity
//!
//! ## Usage Examples
//!
//! ### Basic Synchronization
//! ```rust,ignore
//! use metashrew_sync::*;
//!
//! // Create adapters
//! let node_adapter = MyBitcoinNodeAdapter::new();
//! let storage_adapter = MyStorageAdapter::new();
//! let runtime_adapter = MyRuntimeAdapter::new();
//!
//! // Configure synchronization
//! let config = SyncConfig {
//!     start_block: 0,
//!     exit_at: None,
//!     pipeline_size: Some(10),
//!     max_reorg_depth: 100,
//!     reorg_check_threshold: 6,
//! };
//!
//! // Create and start sync engine
//! let mut sync_engine = MetashrewSync::new(
//!     node_adapter,
//!     storage_adapter,
//!     runtime_adapter,
//!     config
//! );
//!
//! sync_engine.start().await?;
//! ```
//!
//! ### JSON-RPC API Integration
//! ```rust,ignore
//! // The sync engine also implements JsonRpcProvider
//! let result = sync_engine.metashrew_view(
//!     "get_balance".to_string(),
//!     "0x1234...".to_string(),
//!     "latest".to_string()
//! ).await?;
//! ```
//!
//! ## Performance Characteristics
//!
//! ### Pipeline Optimization
//! - **Adaptive Pipeline Size**: Automatically adjusts based on CPU cores
//! - **Memory Management**: Controlled memory usage with bounded channels
//! - **Batch Operations**: Efficient database operations through batching
//! - **State Root Caching**: Optimized state root calculation and storage
//!
//! ### Monitoring and Observability
//! - **Real-time Metrics**: Blocks per second, processing latency, error rates
//! - **Status Reporting**: Current height, blocks behind, sync progress
//! - **Health Checks**: Component availability and connectivity monitoring
//! - **Detailed Logging**: Comprehensive logging for debugging and auditing
//!
//! ## Integration with Metashrew
//!
//! This engine serves as the foundation for:
//! - **rockshrew-mono**: Production Bitcoin indexer implementation
//! - **Custom indexers**: Application-specific blockchain data processing
//! - **Development tools**: Testing and prototyping of indexing strategies
//! - **API services**: JSON-RPC endpoints for accessing indexed data

use async_trait::async_trait;
use log::{debug, error, info, warn};
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::time::sleep;

use crate::{
    BitcoinNodeAdapter, BlockResult, JsonRpcProvider, PreviewCall, RuntimeAdapter, StorageAdapter,
    SyncConfig, SyncEngine, SyncError, SyncResult, SyncStatus, ViewCall,
};

/// Generic Bitcoin indexer synchronization engine
pub struct MetashrewSync<N, S, R>
where
    N: BitcoinNodeAdapter,
    S: StorageAdapter,
    R: RuntimeAdapter,
{
    node: Arc<N>,
    storage: Arc<RwLock<S>>,
    runtime: Arc<R>,
    pub config: SyncConfig,
    is_running: Arc<AtomicBool>,
    pub current_height: Arc<AtomicU32>,
    last_block_time: Arc<RwLock<Option<SystemTime>>>,
    blocks_processed: Arc<AtomicU32>,
    processing_heights: Arc<Mutex<HashSet<u32>>>,
}

impl<N, S, R> MetashrewSync<N, S, R>
where
    N: BitcoinNodeAdapter + 'static,
    S: StorageAdapter + 'static,
    R: RuntimeAdapter + 'static,
{
    /// Create a new sync engine
    pub fn new(node: N, storage: S, runtime: R, config: SyncConfig) -> Self {
        Self {
            node: Arc::new(node),
            storage: Arc::new(RwLock::new(storage)),
            runtime: Arc::new(runtime),
            config,
            is_running: Arc::new(AtomicBool::new(false)),
            current_height: Arc::new(AtomicU32::new(0)),
            last_block_time: Arc::new(RwLock::new(None)),
            blocks_processed: Arc::new(AtomicU32::new(0)),
            processing_heights: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    pub async fn init(&self) {
        let (indexed_height, start_height) = {
            let storage = self.storage.read().await;
            let indexed_height = storage.get_indexed_height().await.unwrap_or(0);
            let start_height = if self.config.start_block > 0 && self.config.start_block > indexed_height {
                self.config.start_block
            } else if indexed_height > 0 {
                indexed_height + 1
            } else {
                self.config.start_block
            };
            (indexed_height, start_height)
        };

        if indexed_height == 0 && self.config.start_block > 0 {
            let prev_height = self.config.start_block.saturating_sub(1);
            let state_root = {
                let storage = self.storage.read().await;
                storage.get_state_root(prev_height).await
            };
            if let Ok(None) = state_root {
                let empty_state_root = vec![0u8; 32];
                let mut storage = self.storage.write().await;
                storage.store_state_root(prev_height, &empty_state_root).await.unwrap();
            }
        }

        
        self.current_height.store(start_height, Ordering::SeqCst);
    }

    /// Get a reference to the storage adapter
    pub fn storage(&self) -> &Arc<RwLock<S>> {
        &self.storage
    }

    /// Get a reference to the node adapter
    pub fn node(&self) -> &Arc<N> {
        &self.node
    }

    /// Get a reference to the runtime adapter
    pub fn runtime(&self) -> &Arc<R> {
        &self.runtime
    }

    /// Run the sync engine (convenience method that calls start)
    pub async fn run(&mut self) -> SyncResult<()> {
        self.start().await
    }

    pub async fn get_next_block_data(&self) -> SyncResult<Option<(u32, Vec<u8>, Vec<u8>)>> {
        let mut current_height = self.current_height.load(Ordering::SeqCst);
        
        // Get remote tip
        let remote_tip = self.node.get_tip_height().await?;

        // Check for reorgs only when close to the tip
        if remote_tip.saturating_sub(current_height) <= self.config.reorg_check_threshold {
            match handle_reorg(
                current_height,
                self.node.clone(),
                self.storage.clone(),
                self.runtime.clone(),
                &self.config,
            )
            .await
            {
                Ok(new_height) => {
                    if new_height != current_height {
                        info!("Reorg handled. Resuming from height {}", new_height);
                    }
                    current_height = new_height;
                    self.current_height.store(current_height, Ordering::SeqCst);
                }
                Err(e) => {
                    error!("Error handling reorg: {}", e);
                    return Err(e);
                }
            }
        }

        // Check exit condition
        if let Some(exit_at) = self.config.exit_at {
            if current_height >= exit_at {
                info!("Fetcher reached exit height {}", exit_at);
                return Ok(None);
            }
        }

        // Check if we need to wait for new blocks
        if current_height > remote_tip {
            debug!(
                "Waiting for new blocks: current={}, tip={}",
                current_height, remote_tip
            );
            return Ok(None);
        }

        // Fetch block
        match self.node.get_block_info(current_height).await {
            Ok(block_info) => {
                info!(
                    "Fetched block {} ({} bytes)",
                    current_height,
                    block_info.data.len()
                );
                Ok(Some((current_height, block_info.data, block_info.hash)))
            }
            Err(e) => {
                error!("Failed to fetch block {}: {}", current_height, e);
                Err(e.into())
            }
        }
    }

    /// Initialize the sync engine by determining the starting height


    /// Process a single block atomically
    pub async fn process_block(&self, height: u32, block_data: Vec<u8>, block_hash: Vec<u8>) -> SyncResult<()> {
        info!(
            "Processing block {} ({} bytes) atomically",
            height,
            block_data.len()
        );

        // Try atomic processing first
        let atomic_result = self.runtime
            .process_block_atomic(height, &block_data, &block_hash)
            .await;

        match atomic_result {
            Ok(result) => {
                // Atomic processing succeeded - commit all operations at once
                info!("Atomic block processing succeeded for height {}", height);

                // Update storage with all metadata atomically
                {
                    let mut storage = self.storage.write().await;
                    storage.set_indexed_height(height).await?;
                    storage.store_block_hash(height, &result.block_hash).await?;
                    storage.store_state_root(height, &result.state_root).await?;
                }

                // Update metrics
                self.blocks_processed.fetch_add(1, Ordering::SeqCst);
                {
                    let mut last_time = self.last_block_time.write().await;
                    *last_time = Some(SystemTime::now());
                }

                info!(
                    "Successfully processed block {} atomically with state root",
                    height
                );
                Ok(())
            }
            Err(_) => {
                // Fallback to non-atomic processing
                warn!(
                    "Atomic processing failed for height {}, falling back to non-atomic",
                    height
                );

                // Process with runtime (non-atomic fallback)
                self.runtime
                    .process_block(height, &block_data)
                    .await
                    .map_err(|e| SyncError::BlockProcessing {
                        height,
                        message: e.to_string(),
                    })?;

                // Get state root after processing
                let state_root = self.runtime.get_state_root(height).await?;

                // Update storage with height, block hash, and state root
                {
                    let mut storage = self.storage.write().await;
                    storage.set_indexed_height(height).await?;
                    storage.store_block_hash(height, &block_hash).await?;
                    storage.store_state_root(height, &state_root).await?;
                }

                // Update metrics
                self.blocks_processed.fetch_add(1, Ordering::SeqCst);
                {
                    let mut last_time = self.last_block_time.write().await;
                    *last_time = Some(SystemTime::now());
                }

                info!(
                    "Successfully processed block {} with fallback method",
                    height
                );
                Ok(())
            }
        }
    }

    /// Run the sync pipeline with parallel fetching and processing
    async fn run_pipeline(&self) -> SyncResult<()> {
        // Determine pipeline size
        let pipeline_size = self.config.pipeline_size.unwrap_or_else(|| {
            let cpu_count = num_cpus::get();
            std::cmp::min(std::cmp::max(5, cpu_count / 2), 16)
        });

        info!("Starting sync pipeline with size {}", pipeline_size);

        // Create channels for the pipeline
        let (block_sender, mut block_receiver) = mpsc::channel::<(u32, Vec<u8>, Vec<u8>)>(pipeline_size);
        let (result_sender, mut result_receiver) = mpsc::channel::<BlockResult>(pipeline_size);

        // Spawn block fetcher task
        let fetcher_handle = {
            let self_clone = self.clone_for_processing();
            let block_sender = block_sender.clone();

            tokio::spawn(async move {
                loop {
                    if !self_clone.is_running.load(Ordering::SeqCst) {
                        break;
                    }

                    let mut current_height = self_clone.current_height.load(Ordering::SeqCst);

                    // Get remote tip
                    let remote_tip = match self_clone.node.get_tip_height().await {
                        Ok(tip) => tip,
                        Err(e) => {
                            error!("Failed to get tip height: {}", e);
                            sleep(Duration::from_secs(5)).await;
                            continue;
                        }
                    };

                    // Check for reorgs only when close to the tip
                    if remote_tip.saturating_sub(current_height) <= self_clone.config.reorg_check_threshold {
                        match handle_reorg(
                            current_height,
                            self_clone.node.clone(),
                            self_clone.storage.clone(),
                            self_clone.runtime.clone(),
                            &self_clone.config,
                        )
                        .await
                        {
                            Ok(new_height) => {
                                if new_height != current_height {
                                    info!("Reorg handled. Resuming from height {}", new_height);
                                    self_clone.current_height.store(new_height, Ordering::SeqCst);
                                }
                                current_height = new_height;
                            }
                            Err(e) => {
                                error!("Error handling reorg: {}", e);
                                sleep(Duration::from_secs(5)).await;
                                continue;
                            }
                        }
                    }

                    // Check exit condition
                    if let Some(exit_at) = self_clone.config.exit_at {
                        if current_height >= exit_at {
                            info!("Fetcher reached exit height {}", exit_at);
                            break;
                        }
                    }

                    // Check if we need to wait for new blocks
                    if current_height > remote_tip {
                        debug!(
                            "Waiting for new blocks: current={}, tip={}",
                            current_height, remote_tip
                        );
                        sleep(Duration::from_secs(3)).await;
                        continue;
                    }

                    // Check if already processing
                    {
                        let processing_heights = self_clone.processing_heights.lock().await;
                        if processing_heights.contains(&current_height) {
                            sleep(Duration::from_millis(100)).await;
                            continue;
                        }
                    }

                    // Fetch block
                    match self_clone.node.get_block_info(current_height).await {
                        Ok(block_info) => {
                            info!(
                                "Fetched block {} ({} bytes)",
                                current_height,
                                block_info.data.len()
                            );
                            {
                                let mut processing_heights = self_clone.processing_heights.lock().await;
                                processing_heights.insert(current_height);
                            }
                            if block_sender
                                .send((current_height, block_info.data, block_info.hash))
                                .await
                                .is_err()
                            {
                                break;
                            }
                        }
                        Err(e) => {
                            error!("Failed to fetch block {}: {}", current_height, e);
                            sleep(Duration::from_secs(1)).await;
                        }
                    }
                }

                debug!("Block fetcher task completed");
            })
        };

        // Spawn block processor task
        let processor_handle = {
            let sync_engine = self.clone_for_processing();
            let result_sender = result_sender.clone();

            tokio::spawn(async move {
                while let Some((block_height, block_data, block_hash)) = block_receiver.recv().await {
                    info!(
                        "Processing block {} ({} bytes)",
                        block_height,
                        block_data.len()
                    );

                    let result = match sync_engine.process_block(block_height, block_data, block_hash).await {
                        Ok(_) => BlockResult::Success(block_height),
                        Err(e) => BlockResult::Error(block_height, e.to_string()),
                    };

                    if result_sender.send(result).await.is_err() {
                        break;
                    }
                }

                debug!("Block processor task completed");
            })
        };

        // Main result handling loop
        while let Some(result) = result_receiver.recv().await {
            let height = match result {
                BlockResult::Success(processed_height) => {
                    info!("Block {} successfully processed", processed_height);
                    self.current_height.store(processed_height + 1, Ordering::SeqCst);
                    {
                        let mut processing_heights = self.processing_heights.lock().await;
                        processing_heights.remove(&processed_height);
                    }
                    processed_height + 1
                }
                BlockResult::Error(failed_height, error) => {
                    error!("Failed to process block {}: {}", failed_height, error);
                    {
                        let mut processing_heights = self.processing_heights.lock().await;
                        processing_heights.remove(&failed_height);
                    }
                    if error.contains("indexer exited unexpectedly") {
                        error!("Critical error: Indexer exited unexpectedly. Aborting.");
                        self.is_running.store(false, Ordering::SeqCst);
                        return Err(SyncError::BlockProcessing {
                            height: failed_height,
                            message: error,
                        });
                    }
                    sleep(Duration::from_secs(5)).await;
                    failed_height
                }
            };

            // Check exit condition
            if let Some(exit_at) = self.config.exit_at {
                if height >= exit_at {
                    info!("Reached exit height {}", exit_at);
                    break;
                }
            }

            if !self.is_running.load(Ordering::SeqCst) {
                break;
            }
        }

        // Cleanup
        drop(block_sender);
        drop(result_sender);

        // Wait for tasks to complete
        let _ = tokio::join!(fetcher_handle, processor_handle);

        Ok(())
    }

    /// Create a clone for processing (simplified for this example)
    fn clone_for_processing(&self) -> ProcessingClone<N, S, R> {
        ProcessingClone {
            node: self.node.clone(),
            storage: self.storage.clone(),
            runtime: self.runtime.clone(),
            config: self.config.clone(),
            is_running: self.is_running.clone(),
            current_height: self.current_height.clone(),
            processing_heights: self.processing_heights.clone(),
        }
    }
}

/// Simplified clone for processing tasks
#[derive(Clone)]
struct ProcessingClone<N, S, R>
where
    N: BitcoinNodeAdapter,
    S: StorageAdapter,
    R: RuntimeAdapter,
{
    node: Arc<N>,
    storage: Arc<RwLock<S>>,
    runtime: Arc<R>,
    config: SyncConfig,
    is_running: Arc<AtomicBool>,
    current_height: Arc<AtomicU32>,
    processing_heights: Arc<Mutex<HashSet<u32>>>,
}

impl<N, S, R> ProcessingClone<N, S, R>
where
    N: BitcoinNodeAdapter + 'static,
    S: StorageAdapter + 'static,
    R: RuntimeAdapter + 'static,
{
    async fn process_block(&self, height: u32, block_data: Vec<u8>, block_hash: Vec<u8>) -> SyncResult<()> {
        // Try atomic processing first
        let atomic_result = self.runtime
            .process_block_atomic(height, &block_data, &block_hash)
            .await;

        match atomic_result {
            Ok(result) => {
                // Atomic processing succeeded
                info!(
                    "Atomic block processing succeeded for height {} in pipeline",
                    height
                );

                // Update storage with all metadata atomically
                {
                    let mut storage = self.storage.write().await;
                    storage.set_indexed_height(height).await?;
                    storage.store_block_hash(height, &result.block_hash).await?;
                    storage.store_state_root(height, &result.state_root).await?;
                }

                Ok(())
            }
            Err(_) => {
                // Fallback to non-atomic processing
                warn!(
                    "Atomic processing failed for height {} in pipeline, falling back",
                    height
                );

                // Process with runtime (non-atomic fallback)
                self.runtime
                    .process_block(height, &block_data)
                    .await
                    .map_err(|e| SyncError::BlockProcessing {
                        height,
                        message: e.to_string(),
                    })?;

                // Get state root after processing
                let state_root = self.runtime.get_state_root(height).await?;

                // Update storage with height, block hash, and state root
                {
                    let mut storage = self.storage.write().await;
                    storage.set_indexed_height(height).await?;
                    storage.store_block_hash(height, &block_hash).await?;
                    storage.store_state_root(height, &state_root).await?;
                }

                Ok(())
            }
        }
    }
}

#[async_trait]
impl<N, S, R> SyncEngine for MetashrewSync<N, S, R>
where
    N: BitcoinNodeAdapter + 'static,
    S: StorageAdapter + 'static,
    R: RuntimeAdapter + 'static,
{
    async fn start(&mut self) -> SyncResult<()> {
        if self.is_running.load(Ordering::SeqCst) {
            return Err(SyncError::Config(
                "Sync engine is already running".to_string(),
            ));
        }

        info!("Starting Metashrew sync engine");
        self.is_running.store(true, Ordering::SeqCst);

        // Check connectivity
        if !self.node.is_connected().await {
            return Err(SyncError::BitcoinNode("Node is not connected".to_string()));
        }

        let storage = self.storage.read().await;
        if !storage.is_available().await {
            return Err(SyncError::Storage("Storage is not available".to_string()));
        }
        drop(storage);

        if !self.runtime.is_ready().await {
            return Err(SyncError::Runtime("Runtime is not ready".to_string()));
        }

        // Start the pipeline
        self.run_pipeline().await?;

        Ok(())
    }

    async fn stop(&mut self) -> SyncResult<()> {
        info!("Stopping Metashrew sync engine");
        self.is_running.store(false, Ordering::SeqCst);
        Ok(())
    }

    async fn get_status(&self) -> SyncResult<SyncStatus> {
        let current_height = self.current_height.load(Ordering::SeqCst);
        let tip_height = self.node.get_tip_height().await?;
        let blocks_behind = tip_height.saturating_sub(current_height);
        let last_block_time = *self.last_block_time.read().await;
        let blocks_processed = self.blocks_processed.load(Ordering::SeqCst);

        // Calculate blocks per second (simplified)
        let blocks_per_second = if let Some(last_time) = last_block_time {
            if let Ok(duration) = last_time.elapsed() {
                blocks_processed as f64 / duration.as_secs_f64()
            } else {
                0.0
            }
        } else {
            0.0
        };

        Ok(SyncStatus {
            is_running: self.is_running.load(Ordering::SeqCst),
            current_height,
            tip_height,
            blocks_behind,
            last_block_time,
            blocks_per_second,
        })
    }

    async fn process_single_block(&mut self, height: u32) -> SyncResult<()> {
        let block_data = self.node.get_block_data(height).await?;
        let block_hash = self.node.get_block_hash(height).await?;
        self.process_block(height, block_data, block_hash).await
    }

}

/// Handles chain reorganizations by finding the common ancestor and rolling back state.
pub async fn handle_reorg<N, S, R>(
    current_height: u32,
    node: Arc<N>,
    storage: Arc<RwLock<S>>,
    runtime: Arc<R>,
    config: &SyncConfig,
) -> SyncResult<u32>
where
    N: BitcoinNodeAdapter + 'static,
    S: StorageAdapter + 'static,
    R: RuntimeAdapter + 'static,
{
    let mut check_height = current_height.saturating_sub(1);
    let mut reorg_detected = false;
    if current_height == 0 {
        return Ok(0);
    }
    // Find the common ancestor
    while check_height > 0 && check_height >= current_height.saturating_sub(config.max_reorg_depth) {
        let storage_guard = storage.read().await;
        let local_hash = match storage_guard.get_block_hash(check_height).await {
            Ok(Some(hash)) => hash,
            _ => {
                check_height = check_height.saturating_sub(1);
                continue;
            }
        };
        drop(storage_guard);

        let remote_hash = match node.get_block_hash(check_height).await {
            Ok(hash) => hash,
            Err(e) => {
                error!("Failed to get remote block hash at height {}: {}", check_height, e);
                return Ok(current_height); // Don't reorg if node is failing
            }
        };

        if local_hash == remote_hash {
            break; // Common ancestor found
        }

        reorg_detected = true;
        check_height = check_height.saturating_sub(1);
    }

    if reorg_detected {
        let rollback_height = check_height;
        warn!("Reorg detected. Rolling back to height {}", rollback_height);

        // Rollback storage
        let mut storage_guard = storage.write().await;
        storage_guard.rollback_to_height(rollback_height).await?;
        drop(storage_guard);

        // Refresh runtime memory
        runtime.refresh_memory().await?;

        return Ok(rollback_height + 1);
    }

    Ok(current_height)
}

#[async_trait]
impl<N, S, R> JsonRpcProvider for MetashrewSync<N, S, R>
where
    N: BitcoinNodeAdapter + 'static,
    S: StorageAdapter + 'static,
    R: RuntimeAdapter + 'static,
{
    async fn metashrew_view(
        &self,
        function_name: String,
        input_hex: String,
        height: String,
    ) -> SyncResult<String> {
        let input_data = hex::decode(input_hex.trim_start_matches("0x"))
            .map_err(|e| SyncError::Serialization(format!("Invalid hex input: {}", e)))?;

        let height = if height == "latest" {
            self.current_height.load(Ordering::SeqCst).saturating_sub(1)
        } else {
            height
                .parse::<u32>()
                .map_err(|e| SyncError::Serialization(format!("Invalid height: {}", e)))?
        };

        let call = ViewCall {
            function_name,
            input_data,
            height,
        };

        let result = self.runtime.execute_view(call).await?;

        Ok(format!("0x{}", hex::encode(result.data)))
    }

    async fn metashrew_preview(
        &self,
        block_hex: String,
        function_name: String,
        input_hex: String,
        height: String,
    ) -> SyncResult<String> {
        let block_data = hex::decode(block_hex.trim_start_matches("0x"))
            .map_err(|e| SyncError::Serialization(format!("Invalid hex block data: {}", e)))?;

        let input_data = hex::decode(input_hex.trim_start_matches("0x"))
            .map_err(|e| SyncError::Serialization(format!("Invalid hex input: {}", e)))?;

        let height = if height == "latest" {
            self.current_height.load(Ordering::SeqCst).saturating_sub(1)
        } else {
            height
                .parse::<u32>()
                .map_err(|e| SyncError::Serialization(format!("Invalid height: {}", e)))?
        };

        let call = PreviewCall {
            block_data,
            function_name,
            input_data,
            height,
        };

        let result = self.runtime.execute_preview(call).await?;

        Ok(format!("0x{}", hex::encode(result.data)))
    }

    async fn metashrew_height(&self) -> SyncResult<u32> {
        // Use storage adapter to get the actual indexed height from database
        // This ensures consistency with the database state rather than sync engine's internal tracking
        let storage = self.storage.read().await;
        storage.get_indexed_height().await
    }

    async fn metashrew_getblockhash(&self, height: u32) -> SyncResult<String> {
        let storage = self.storage.read().await;
        match storage.get_block_hash(height).await? {
            Some(hash) => Ok(format!("0x{}", hex::encode(hash))),
            None => Err(SyncError::Storage(format!(
                "Block hash not found for height {}",
                height
            ))),
        }
    }

    async fn metashrew_stateroot(&self, height: String) -> SyncResult<String> {
        let height = if height == "latest" {
            self.current_height.load(Ordering::SeqCst).saturating_sub(1)
        } else {
            height
                .parse::<u32>()
                .map_err(|e| SyncError::Serialization(format!("Invalid height: {}", e)))?
        };

        let storage = self.storage.read().await;
        match storage.get_state_root(height).await? {
            Some(root) => Ok(format!("0x{}", hex::encode(root))),
            None => Err(SyncError::Storage(format!(
                "State root not found for height {}",
                height
            ))),
        }
    }

    async fn metashrew_snapshot(&self) -> SyncResult<serde_json::Value> {
        let storage = self.storage.read().await;
        let stats = storage.get_stats().await?;

        Ok(serde_json::json!({
            "enabled": true,
            "current_height": self.current_height.load(Ordering::SeqCst),
            "indexed_height": stats.indexed_height,
            "total_entries": stats.total_entries,
            "storage_size_bytes": stats.storage_size_bytes
        }))
    }
}