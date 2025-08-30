//! Mock implementations for snapshot testing
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::{
    SnapshotConsumer, SnapshotData, SnapshotMetadata, SnapshotProvider, SnapshotServer,
    SnapshotServerStatus, SyncResult,
};

/// Mock snapshot provider for testing
#[derive(Debug, Clone)]
pub struct MockSnapshotProvider {
    snapshots: Arc<RwLock<HashMap<u32, SnapshotData>>>,
    interval: u32,
}

impl MockSnapshotProvider {
    pub fn new(interval: u32) -> Self {
        Self {
            snapshots: Arc::new(RwLock::new(HashMap::new())),
            interval,
        }
    }
}

#[async_trait]
impl SnapshotProvider for MockSnapshotProvider {
    async fn create_snapshot(&mut self, height: u32) -> SyncResult<SnapshotMetadata> {
        let metadata = SnapshotMetadata {
            height,
            block_hash: vec![height as u8; 32],
            state_root: vec![height as u8; 32],
            timestamp: 0,
            size_bytes: 1024,
            checksum: "mock_checksum".to_string(),
            wasm_hash: "mock_wasm_hash".to_string(),
        };
        let data = SnapshotData {
            metadata: metadata.clone(),
            state_data: vec![height as u8; 1024],
            recent_block_hashes: HashMap::new(),
        };
        self.snapshots.write().unwrap().insert(height, data);
        Ok(metadata)
    }

    async fn list_snapshots(&self) -> SyncResult<Vec<SnapshotMetadata>> {
        Ok(self
            .snapshots
            .read()
            .unwrap()
            .values()
            .map(|d| d.metadata.clone())
            .collect())
    }

    async fn get_snapshot(&self, height: u32) -> SyncResult<Option<SnapshotData>> {
        Ok(self.snapshots.read().unwrap().get(&height).cloned())
    }

    async fn get_latest_snapshot(&self) -> SyncResult<Option<SnapshotData>> {
        let snapshots = self.snapshots.read().unwrap();
        let latest_height = snapshots.keys().max().cloned();
        Ok(latest_height.and_then(|h| snapshots.get(&h).cloned()))
    }

    async fn cleanup_snapshots(&mut self) -> SyncResult<usize> {
        Ok(0)
    }

    fn should_create_snapshot(&self, height: u32) -> bool {
        height % self.interval == 0
    }
}

/// Mock snapshot consumer for testing
#[derive(Debug, Clone)]
pub struct MockSnapshotConsumer {
    provider: MockSnapshotProvider,
}

impl MockSnapshotConsumer {
    pub fn new(provider: MockSnapshotProvider) -> Self {
        Self { provider }
    }
}

#[async_trait]
impl SnapshotConsumer for MockSnapshotConsumer {
    async fn check_available_snapshots(&self) -> SyncResult<Vec<SnapshotMetadata>> {
        self.provider.list_snapshots().await
    }

    async fn apply_snapshot(&mut self, _metadata: &SnapshotMetadata) -> SyncResult<()> {
        Ok(())
    }

    async fn get_best_snapshot(
        &self,
        _current_height: u32,
        _tip_height: u32,
    ) -> SyncResult<Option<SnapshotMetadata>> {
        Ok(self
            .provider
            .get_latest_snapshot()
            .await?
            .map(|d| d.metadata))
    }

    async fn verify_snapshot(&self, _data: &SnapshotData) -> SyncResult<bool> {
        Ok(true)
    }

    async fn should_use_snapshots(&self, current_height: u32, tip_height: u32) -> SyncResult<bool> {
        Ok(tip_height > current_height + 100)
    }
}

/// Mock snapshot server for testing
#[derive(Debug, Clone)]
pub struct MockSnapshotServer;

#[async_trait]
impl SnapshotServer for MockSnapshotServer {
    async fn start(&mut self) -> SyncResult<()> {
        Ok(())
    }
    async fn stop(&mut self) -> SyncResult<()> {
        Ok(())
    }
    async fn get_status(&self) -> SyncResult<SnapshotServerStatus> {
        Ok(SnapshotServerStatus {
            is_running: true,
            total_snapshots: 0,
            latest_snapshot_height: None,
            total_size_bytes: 0,
            uptime_seconds: 0,
        })
    }
    async fn register_snapshot(
        &mut self,
        _metadata: SnapshotMetadata,
        _data: Vec<u8>,
    ) -> SyncResult<()> {
        Ok(())
    }
    async fn get_snapshot_metadata(&self, _height: u32) -> SyncResult<Option<SnapshotMetadata>> {
        Ok(None)
    }
    async fn get_snapshot_data(&self, _height: u32) -> SyncResult<Option<Vec<u8>>> {
        Ok(None)
    }
    async fn list_available_snapshots(&self) -> SyncResult<Vec<SnapshotMetadata>> {
        Ok(vec![])
    }
}
