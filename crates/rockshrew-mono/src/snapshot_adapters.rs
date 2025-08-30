//! Production implementations of snapshot traits for rockshrew-mono

use anyhow::Result;
use async_trait::async_trait;
use log::{debug, error, info};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

use metashrew_sync::{
    SnapshotClient, SnapshotConsumer, SnapshotData, SnapshotMetadata as GenericMetadata,
    SnapshotProvider, SnapshotServer, SnapshotServerStatus, StorageAdapter, SyncError, SyncResult,
};

use crate::adapters::{MetashrewRuntimeAdapter};
use rockshrew_runtime::fork_adapter::ForkAdapter;
use crate::snapshot::{RepoIndex, SnapshotConfig, SnapshotManager};
use rockshrew_runtime::RocksDBStorageAdapter;

/// Production snapshot provider using the existing SnapshotManager
pub struct RockshrewSnapshotProvider {
    manager: Arc<RwLock<SnapshotManager>>,
    storage: Arc<RwLock<RocksDBStorageAdapter>>,
    runtime_adapter: Option<Arc<RwLock<MetashrewRuntimeAdapter<ForkAdapter>>>>,
}

impl RockshrewSnapshotProvider {
    #[allow(dead_code)]
    pub fn new(config: SnapshotConfig, storage: Arc<RwLock<RocksDBStorageAdapter>>) -> Self {
        let manager = Arc::new(RwLock::new(SnapshotManager::new(config)));
        Self {
            manager,
            storage,
            runtime_adapter: None,
        }
    }

    /// Set the runtime adapter to get tracked changes from
    #[allow(dead_code)]
    pub fn set_runtime_adapter(&mut self, runtime_adapter: Arc<RwLock<MetashrewRuntimeAdapter<ForkAdapter>>>) {
        self.runtime_adapter = Some(runtime_adapter);
    }

    #[allow(dead_code)]
    pub async fn initialize(&self, current_height: u32) -> Result<()> {
        let mut manager = self.manager.write().await;

        // Initialize the directory structure first
        manager.initialize().await?;

        // Set the last_snapshot_height to the current height without opening the database
        manager.last_snapshot_height = current_height;
        info!(
            "Set last snapshot height to {} for snapshot provider",
            current_height
        );

        Ok(())
    }

    #[allow(dead_code)]
    pub async fn set_current_wasm(&self, wasm_path: PathBuf) -> Result<()> {
        let mut manager = self.manager.write().await;
        manager.set_current_wasm(wasm_path)
    }
}

#[async_trait]
impl SnapshotProvider for RockshrewSnapshotProvider {
    /// Create a snapshot at the current height
    async fn create_snapshot(&mut self, height: u32) -> SyncResult<GenericMetadata> {
        info!("Creating snapshot at height {}", height);

        // Get state root from storage, with fallback to direct database access
        let state_root = {
            let storage = self.storage.read().await;
            match storage.get_state_root(height).await? {
                Some(root) => root,
                None => {
                    // Fallback: try to get state root directly from database
                    let db = storage.get_db_handle().await?;
                    let root_key = format!("smt:root:{}", height).into_bytes();
                    db.get(&root_key)
                        .map_err(|e| SyncError::Runtime(format!("Database error: {}", e)))?
                        .ok_or_else(|| {
                            SyncError::Runtime(format!("No state root found for height {} (tried both storage adapter and direct DB access)", height))
                        })?
                }
            }
        };

        // Get tracked changes from the runtime adapter if available
        let actual_size = if let Some(runtime_adapter) = &self.runtime_adapter {
            // Get the snapshot manager with tracked changes from the runtime adapter
            if let Some(runtime_manager_arc) =
                runtime_adapter.read().await.get_snapshot_manager().await
            {
                let mut manager = self.manager.write().await;
                let start_height = manager.last_snapshot_height;

                info!("Creating snapshot for interval {}-{}", start_height, height);

                // Access the runtime manager through the Arc<RwLock<>>
                let runtime_manager = runtime_manager_arc.read().await;
                info!(
                    "Using {} tracked key-value changes from runtime adapter",
                    runtime_manager.key_changes.len()
                );

                // Get tracking stats for debugging
                let (logical_updates, raw_operations, logical_size, raw_size) =
                    runtime_manager.get_tracking_stats();
                info!("Snapshot tracking stats: {} logical updates ({} bytes), {} raw operations ({} bytes)",
                      logical_updates, logical_size, raw_operations, raw_size);

                // Copy the tracked changes from the runtime manager to our manager
                manager.key_changes = runtime_manager.key_changes.clone();
                manager.raw_operations = runtime_manager.raw_operations.clone();
                manager.key_change_heights = runtime_manager.key_change_heights.clone();
                manager.last_snapshot_height = start_height;

                // Release the runtime manager lock before creating snapshot
                drop(runtime_manager);

                // Create the snapshot with the tracked changes
                manager
                    .create_snapshot(height, &state_root)
                    .await
                    .map_err(|e| SyncError::Runtime(format!("Failed to create snapshot: {}", e)))?;

                // Calculate the actual size of the snapshot data
                let mut total_size = 0u64;
                for (key, value) in &manager.key_changes {
                    total_size += 8; // 4 bytes for key length + 4 bytes for value length
                    total_size += key.len() as u64;
                    total_size += value.len() as u64;
                }

                info!(
                    "Calculated snapshot data size: {} bytes from {} key-value pairs",
                    total_size,
                    manager.key_changes.len()
                );
                total_size
            } else {
                info!("No snapshot manager found in runtime adapter, creating empty snapshot");
                let mut manager = self.manager.write().await;
                manager
                    .create_snapshot(height, &state_root)
                    .await
                    .map_err(|e| SyncError::Runtime(format!("Failed to create snapshot: {}", e)))?;
                0
            }
        } else {
            // Fallback to the old behavior if no runtime adapter is set
            let mut manager = self.manager.write().await;
            let start_height = manager.last_snapshot_height;

            info!(
                "Creating snapshot for interval {}-{} (no runtime adapter)",
                start_height, height
            );
            info!(
                "Using {} tracked key-value changes",
                manager.key_changes.len()
            );

            // Get tracking stats for debugging
            let (logical_updates, raw_operations, logical_size, raw_size) =
                manager.get_tracking_stats();
            info!("Snapshot tracking stats: {} logical updates ({} bytes), {} raw operations ({} bytes)",
                  logical_updates, logical_size, raw_operations, raw_size);

            // Create the snapshot with the tracked changes
            manager
                .create_snapshot(height, &state_root)
                .await
                .map_err(|e| SyncError::Runtime(format!("Failed to create snapshot: {}", e)))?;

            // Calculate the actual size of the snapshot data
            let mut total_size = 0u64;
            for (key, value) in &manager.key_changes {
                total_size += 8; // 4 bytes for key length + 4 bytes for value length
                total_size += key.len() as u64;
                total_size += value.len() as u64;
            }

            total_size
        };

        // Get block hash from storage
        let block_hash = {
            let storage = self.storage.read().await;
            storage
                .get_block_hash(height)
                .await?
                .unwrap_or_else(|| vec![0u8; 32])
        };

        info!(
            "Successfully created snapshot for height {} with {} bytes of data",
            height, actual_size
        );

        // Return metadata in the format expected by the trait
        Ok(GenericMetadata {
            height,
            block_hash,
            state_root,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            size_bytes: actual_size,
            checksum: "".to_string(),  // TODO: Calculate checksum
            wasm_hash: "".to_string(), // TODO: Get WASM hash
        })
    }

    /// Get available snapshots
    async fn list_snapshots(&self) -> SyncResult<Vec<GenericMetadata>> {
        let manager = self.manager.read().await;

        // Read the index.json file
        let index_path = manager.config.directory.join("index.json");
        if !index_path.exists() {
            return Ok(Vec::new());
        }

        let index_content = tokio::fs::read(&index_path)
            .await
            .map_err(|e| SyncError::Runtime(format!("Failed to read index: {}", e)))?;

        let index: RepoIndex = serde_json::from_slice(&index_content)
            .map_err(|e| SyncError::Runtime(format!("Failed to parse index: {}", e)))?;

        let mut metadata_list = Vec::new();
        for interval in index.intervals {
            metadata_list.push(GenericMetadata {
                height: interval.end_height,
                block_hash: vec![0u8; 32], // TODO: Get actual block hash
                state_root: hex::decode(&interval.state_root)
                    .map_err(|e| SyncError::Runtime(format!("Invalid state root hex: {}", e)))?,
                timestamp: interval.created_at,
                size_bytes: 0,            // TODO: Get actual size
                checksum: "".to_string(), // TODO: Calculate checksum
                wasm_hash: interval.wasm_hash,
            });
        }

        Ok(metadata_list)
    }

    /// Get a specific snapshot by height
    async fn get_snapshot(&self, height: u32) -> SyncResult<Option<SnapshotData>> {
        let manager = self.manager.read().await;

        // Find the interval containing this height
        let index_path = manager.config.directory.join("index.json");
        if !index_path.exists() {
            return Ok(None);
        }

        let index_content = tokio::fs::read(&index_path)
            .await
            .map_err(|e| SyncError::Runtime(format!("Failed to read index: {}", e)))?;

        let index: RepoIndex = serde_json::from_slice(&index_content)
            .map_err(|e| SyncError::Runtime(format!("Failed to parse index: {}", e)))?;

        for interval in index.intervals {
            if interval.end_height == height {
                let diff_path = manager.config.directory.join(&interval.diff_file);
                if diff_path.exists() {
                    let state_data = tokio::fs::read(&diff_path).await.map_err(|e| {
                        SyncError::Runtime(format!("Failed to read snapshot: {}", e))
                    })?;

                    let metadata = GenericMetadata {
                        height: interval.end_height,
                        block_hash: vec![0u8; 32], // TODO: Get actual block hash
                        state_root: hex::decode(&interval.state_root).map_err(|e| {
                            SyncError::Runtime(format!("Invalid state root hex: {}", e))
                        })?,
                        timestamp: interval.created_at,
                        size_bytes: state_data.len() as u64,
                        checksum: "".to_string(), // TODO: Calculate checksum
                        wasm_hash: interval.wasm_hash,
                    };

                    return Ok(Some(SnapshotData {
                        metadata,
                        state_data,
                        recent_block_hashes: HashMap::new(), // TODO: Populate with recent blocks
                    }));
                }
            }
        }

        Ok(None)
    }

    /// Get the latest snapshot
    async fn get_latest_snapshot(&self) -> SyncResult<Option<SnapshotData>> {
        let snapshots = self.list_snapshots().await?;
        if let Some(latest) = snapshots.into_iter().max_by_key(|s| s.height) {
            self.get_snapshot(latest.height).await
        } else {
            Ok(None)
        }
    }

    /// Delete old snapshots beyond the configured limit
    ///
    /// NOTE: For snapshot repositories, we should maintain ALL snapshots to provide
    /// a complete history. This method should only be used for local cleanup when
    /// disk space is a concern, and should respect the max_snapshots configuration.
    async fn cleanup_snapshots(&mut self) -> SyncResult<usize> {
        info!("Checking for snapshot cleanup (maintaining complete snapshot history)");

        // For snapshot repositories, we should NOT automatically delete old snapshots
        // as they provide the complete history needed for syncing from any point.
        // Only clean up if explicitly configured with a max_snapshots limit.

        let manager = self.manager.read().await;
        let intervals_dir = manager.config.directory.join("intervals");

        if !intervals_dir.exists() {
            return Ok(0);
        }

        // Check if we have a max_snapshots configuration
        // For now, we'll disable automatic cleanup to maintain complete snapshot history
        // This can be made configurable in the future if needed
        let max_snapshots = None; // TODO: Add max_snapshots to SnapshotConfig if needed

        if max_snapshots.is_none() {
            info!("No max_snapshots limit configured - maintaining complete snapshot history");
            return Ok(0);
        }

        // If max_snapshots is configured, proceed with cleanup
        let max_snapshots = max_snapshots.unwrap();

        // Read all interval directories
        let mut entries = tokio::fs::read_dir(&intervals_dir).await.map_err(|e| {
            SyncError::Runtime(format!("Failed to read intervals directory: {}", e))
        })?;

        let mut intervals = Vec::new();
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| SyncError::Runtime(format!("Failed to read directory entry: {}", e)))?
        {
            if entry
                .file_type()
                .await
                .map_err(|e| SyncError::Runtime(format!("Failed to get file type: {}", e)))?
                .is_dir()
            {
                if let Some(name) = entry.file_name().to_str() {
                    if let Some((_start, end)) = parse_interval_name(name) {
                        intervals.push((end, entry.path()));
                    }
                }
            }
        }

        // Sort by end height (descending) and keep only the most recent max_snapshots
        intervals.sort_by(|a, b| b.0.cmp(&a.0));

        let mut removed_count = 0;
        for (_, path) in intervals.into_iter().skip(max_snapshots) {
            info!("Removing old snapshot directory: {:?}", path);
            if let Err(e) = tokio::fs::remove_dir_all(&path).await {
                error!("Failed to remove snapshot directory {:?}: {}", path, e);
            } else {
                removed_count += 1;
            }
        }

        if removed_count > 0 {
            info!(
                "Cleaned up {} old snapshots (keeping {} most recent)",
                removed_count, max_snapshots
            );
        }

        Ok(removed_count)
    }

    /// Check if a snapshot should be created at this height
    fn should_create_snapshot(&self, height: u32) -> bool {
        // We need to access the config synchronously, so we'll need to restructure this
        // For now, let's use a blocking approach to get the interval
        if height == 0 {
            return false;
        }

        // Try to get the interval from the manager config
        // This is not ideal but works for the current architecture
        if let Ok(manager) = self.manager.try_read() {
            height % manager.config.interval == 0
        } else {
            // Fallback to a reasonable default if we can't access the config
            height % 100 == 0
        }
    }
}

/// Production snapshot consumer using the existing SnapshotManager
pub struct RockshrewSnapshotConsumer {
    #[allow(dead_code)]
    manager: Arc<RwLock<SnapshotManager>>,
    #[allow(dead_code)]
    storage: Arc<RwLock<RocksDBStorageAdapter>>,
}

impl RockshrewSnapshotConsumer {
    #[allow(dead_code)]
    pub fn new(config: SnapshotConfig, storage: Arc<RwLock<RocksDBStorageAdapter>>) -> Self {
        let manager = Arc::new(RwLock::new(SnapshotManager::new(config)));
        Self { manager, storage }
    }
}

#[async_trait]
impl SnapshotConsumer for RockshrewSnapshotConsumer {
    /// Check for available snapshots from the repository
    async fn check_available_snapshots(&self) -> SyncResult<Vec<GenericMetadata>> {
        // This would typically check a remote repository
        // For now, return empty list
        Ok(Vec::new())
    }

    /// Download and apply a snapshot
    async fn apply_snapshot(&mut self, metadata: &GenericMetadata) -> SyncResult<()> {
        info!("Applying snapshot for height {}", metadata.height);

        // For now, this is a placeholder implementation
        // In a real implementation, this would download and apply the snapshot data

        Ok(())
    }

    /// Get the best snapshot to use for catching up
    async fn get_best_snapshot(
        &self,
        current_height: u32,
        tip_height: u32,
    ) -> SyncResult<Option<GenericMetadata>> {
        let available = self.check_available_snapshots().await?;

        // Find the best snapshot between current_height and tip_height
        let best = available
            .into_iter()
            .filter(|s| s.height > current_height && s.height <= tip_height)
            .max_by_key(|s| s.height);

        Ok(best)
    }

    /// Verify a snapshot's integrity
    async fn verify_snapshot(&self, data: &SnapshotData) -> SyncResult<bool> {
        info!("Verifying snapshot for height {}", data.metadata.height);

        // Basic verification - check if we can decompress the data
        match zstd::decode_all(data.state_data.as_slice()) {
            Ok(_) => {
                debug!("Snapshot decompression successful");
                Ok(true)
            }
            Err(e) => {
                error!("Snapshot decompression failed: {}", e);
                Ok(false)
            }
        }
    }

    /// Check if we should use snapshots given current state
    async fn should_use_snapshots(&self, current_height: u32, tip_height: u32) -> SyncResult<bool> {
        // Use snapshots if we're more than 100 blocks behind
        Ok(tip_height > current_height + 100)
    }
}

/// HTTP-based snapshot server implementation
pub struct RockshrewSnapshotServer {
    provider: Arc<RwLock<RockshrewSnapshotProvider>>,
    status: Arc<RwLock<SnapshotServerStatus>>,
    snapshots: Arc<RwLock<HashMap<u32, Vec<u8>>>>,
}

impl RockshrewSnapshotServer {
    #[allow(dead_code)]
    pub fn new(provider: RockshrewSnapshotProvider) -> Self {
        let status = SnapshotServerStatus {
            is_running: false,
            total_snapshots: 0,
            latest_snapshot_height: None,
            total_size_bytes: 0,
            uptime_seconds: 0,
        };

        Self {
            provider: Arc::new(RwLock::new(provider)),
            status: Arc::new(RwLock::new(status)),
            snapshots: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl SnapshotServer for RockshrewSnapshotServer {
    /// Start the snapshot server
    async fn start(&mut self) -> SyncResult<()> {
        let mut status = self.status.write().await;
        status.is_running = true;
        info!("Snapshot server started");
        Ok(())
    }

    /// Stop the snapshot server
    async fn stop(&mut self) -> SyncResult<()> {
        let mut status = self.status.write().await;
        status.is_running = false;
        info!("Snapshot server stopped");
        Ok(())
    }

    /// Get server status
    async fn get_status(&self) -> SyncResult<SnapshotServerStatus> {
        let status = self.status.read().await;
        Ok(status.clone())
    }

    /// Register a new snapshot
    async fn register_snapshot(
        &mut self,
        metadata: GenericMetadata,
        data: Vec<u8>,
    ) -> SyncResult<()> {
        let mut snapshots = self.snapshots.write().await;
        snapshots.insert(metadata.height, data);

        let mut status = self.status.write().await;
        status.total_snapshots = snapshots.len();
        status.latest_snapshot_height = Some(metadata.height);

        info!("Registered snapshot for height {}", metadata.height);
        Ok(())
    }

    /// Get snapshot metadata by height
    async fn get_snapshot_metadata(&self, height: u32) -> SyncResult<Option<GenericMetadata>> {
        let provider = self.provider.read().await;
        let snapshots = provider.list_snapshots().await?;
        Ok(snapshots.into_iter().find(|s| s.height == height))
    }

    /// Get snapshot data by height
    async fn get_snapshot_data(&self, height: u32) -> SyncResult<Option<Vec<u8>>> {
        let snapshots = self.snapshots.read().await;
        Ok(snapshots.get(&height).cloned())
    }

    /// List all available snapshots
    async fn list_available_snapshots(&self) -> SyncResult<Vec<GenericMetadata>> {
        let provider = self.provider.read().await;
        provider.list_snapshots().await
    }
}

/// HTTP-based snapshot client implementation
pub struct RockshrewSnapshotClient {
    #[allow(dead_code)]
    base_url: String,
    client: reqwest::Client,
}

impl RockshrewSnapshotClient {
    #[allow(dead_code)]
    pub fn new(base_url: String) -> Self {
        Self {
            base_url,
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl SnapshotClient for RockshrewSnapshotClient {
    /// Download snapshot metadata from URL
    async fn download_metadata(&self, url: &str) -> SyncResult<GenericMetadata> {
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| SyncError::Network(format!("Failed to download metadata: {}", e)))?;

        if !response.status().is_success() {
            return Err(SyncError::Network(format!(
                "HTTP error: {}",
                response.status()
            )));
        }

        let metadata: GenericMetadata = response
            .json()
            .await
            .map_err(|e| SyncError::Network(format!("Failed to parse metadata: {}", e)))?;

        Ok(metadata)
    }

    /// Download snapshot data from URL
    async fn download_data(&self, url: &str) -> SyncResult<Vec<u8>> {
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| SyncError::Network(format!("Failed to download data: {}", e)))?;

        if !response.status().is_success() {
            return Err(SyncError::Network(format!(
                "HTTP error: {}",
                response.status()
            )));
        }

        let data = response
            .bytes()
            .await
            .map_err(|e| SyncError::Network(format!("Failed to read data: {}", e)))?
            .to_vec();

        Ok(data)
    }

    /// List available snapshots from repository
    async fn list_remote_snapshots(&self, base_url: &str) -> SyncResult<Vec<GenericMetadata>> {
        let url = format!("{}/index.json", base_url.trim_end_matches('/'));

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| SyncError::Network(format!("Failed to fetch index: {}", e)))?;

        if !response.status().is_success() {
            return Err(SyncError::Network(format!(
                "HTTP error: {}",
                response.status()
            )));
        }

        let index_json = response
            .text()
            .await
            .map_err(|e| SyncError::Network(format!("Failed to read response: {}", e)))?;

        let index: RepoIndex = serde_json::from_str(&index_json)
            .map_err(|e| SyncError::Runtime(format!("Failed to parse index: {}", e)))?;

        let mut metadata_list = Vec::new();
        for interval in index.intervals {
            metadata_list.push(GenericMetadata {
                height: interval.end_height,
                block_hash: vec![0u8; 32], // TODO: Get actual block hash
                state_root: hex::decode(&interval.state_root)
                    .map_err(|e| SyncError::Runtime(format!("Invalid state root hex: {}", e)))?,
                timestamp: interval.created_at,
                size_bytes: 0,            // We don't know the size without fetching
                checksum: "".to_string(), // TODO: Calculate checksum
                wasm_hash: interval.wasm_hash,
            });
        }

        Ok(metadata_list)
    }

    /// Check if repository is available
    async fn check_repository(&self, base_url: &str) -> SyncResult<bool> {
        let url = format!("{}/index.json", base_url.trim_end_matches('/'));

        match self.client.head(&url).send().await {
            Ok(response) if response.status().is_success() => Ok(true),
            _ => Ok(false),
        }
    }
}

// Helper functions
fn parse_interval_name(name: &str) -> Option<(u32, u32)> {
    let parts: Vec<&str> = name.split('-').collect();
    if parts.len() == 2 {
        if let (Ok(start), Ok(end)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>()) {
            return Some((start, end));
        }
    }
    None
}
