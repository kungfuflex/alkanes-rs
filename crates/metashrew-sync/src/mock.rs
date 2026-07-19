//! Mock implementations for testing

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tokio::sync::Mutex;

use crate::{
    AtomicBlockResult, BitcoinNodeAdapter, BlockInfo, ChainTip, PreviewCall, RuntimeAdapter,
    RuntimeStats, StorageAdapter, StorageStats, SyncError, SyncResult, ViewCall, ViewResult,
};

/// Mock Bitcoin node adapter for testing
#[derive(Debug, Clone)]
pub struct MockBitcoinNode {
    blocks: Arc<RwLock<HashMap<u32, BlockInfo>>>,
    tip_height: Arc<RwLock<u32>>,
    connected: Arc<RwLock<bool>>,
}

impl MockBitcoinNode {
    pub fn new() -> Self {
        Self {
            blocks: Arc::new(RwLock::new(HashMap::new())),
            tip_height: Arc::new(RwLock::new(0)),
            connected: Arc::new(RwLock::new(true)),
        }
    }

    pub fn add_block(&self, height: u32, hash: Vec<u8>, data: Vec<u8>) {
        let block_info = BlockInfo { height, hash, data };
        let mut blocks = self.blocks.write().unwrap();
        blocks.insert(height, block_info);

        let mut tip = self.tip_height.write().unwrap();
        if height > *tip {
            *tip = height;
        }
    }

    pub fn set_connected(&self, connected: bool) {
        let mut conn = self.connected.write().unwrap();
        *conn = connected;
    }

    pub fn simulate_reorg(&self, from_height: u32, new_blocks: Vec<(u32, Vec<u8>, Vec<u8>)>) {
        let mut blocks = self.blocks.write().unwrap();
        blocks.retain(|&height, _| height < from_height);
        for (height, hash, data) in new_blocks {
            blocks.insert(height, BlockInfo { height, hash, data });
        }
        if let Some(&max_height) = blocks.keys().max() {
            let mut tip = self.tip_height.write().unwrap();
            *tip = max_height;
        }
    }
}

#[async_trait]
impl BitcoinNodeAdapter for MockBitcoinNode {
    async fn get_tip_height(&self) -> SyncResult<u32> {
        if !*self.connected.read().unwrap() {
            return Err(SyncError::BitcoinNode("Node not connected".to_string()));
        }
        Ok(*self.tip_height.read().unwrap())
    }

    async fn get_block_hash(&self, height: u32) -> SyncResult<Vec<u8>> {
        if !*self.connected.read().unwrap() {
            return Err(SyncError::BitcoinNode("Node not connected".to_string()));
        }
        let blocks = self.blocks.read().unwrap();
        match blocks.get(&height) {
            Some(block) => Ok(block.hash.clone()),
            None => Err(SyncError::BitcoinNode(format!(
                "Block {} not found",
                height
            ))),
        }
    }

    async fn get_block_data(&self, height: u32) -> SyncResult<Vec<u8>> {
        if !*self.connected.read().unwrap() {
            return Err(SyncError::BitcoinNode("Node not connected".to_string()));
        }
        let blocks = self.blocks.read().unwrap();
        match blocks.get(&height) {
            Some(block) => Ok(block.data.clone()),
            None => Err(SyncError::BitcoinNode(format!(
                "Block {} not found",
                height
            ))),
        }
    }

    async fn get_block_info(&self, height: u32) -> SyncResult<BlockInfo> {
        if !*self.connected.read().unwrap() {
            return Err(SyncError::BitcoinNode("Node not connected".to_string()));
        }
        let blocks = self.blocks.read().unwrap();
        match blocks.get(&height) {
            Some(block) => Ok(block.clone()),
            None => Err(SyncError::BitcoinNode(format!(
                "Block {} not found",
                height
            ))),
        }
    }

    async fn get_chain_tip(&self) -> SyncResult<ChainTip> {
        if !*self.connected.read().unwrap() {
            return Err(SyncError::BitcoinNode("Node not connected".to_string()));
        }
        let tip_height = *self.tip_height.read().unwrap();
        let blocks = self.blocks.read().unwrap();
        match blocks.get(&tip_height) {
            Some(block) => Ok(ChainTip {
                height: tip_height,
                hash: block.hash.clone(),
            }),
            None => Err(SyncError::BitcoinNode("Tip block not found".to_string())),
        }
    }

    async fn is_connected(&self) -> bool {
        *self.connected.read().unwrap()
    }
}

/// Mock storage adapter for testing
#[derive(Debug, Clone)]
pub struct MockStorage {
    indexed_height: Arc<Mutex<u32>>,
    block_hashes: Arc<Mutex<HashMap<u32, Vec<u8>>>>,
    state_roots: Arc<Mutex<HashMap<u32, Vec<u8>>>>,
    available: Arc<RwLock<bool>>,
}

impl MockStorage {
    pub fn new() -> Self {
        Self {
            indexed_height: Arc::new(Mutex::new(0)),
            block_hashes: Arc::new(Mutex::new(HashMap::new())),
            state_roots: Arc::new(Mutex::new(HashMap::new())),
            available: Arc::new(RwLock::new(true)),
        }
    }

    pub fn set_available(&self, available: bool) {
        let mut avail = self.available.write().unwrap();
        *avail = available;
    }
}

#[async_trait]
impl StorageAdapter for MockStorage {
    async fn get_indexed_height(&self) -> SyncResult<u32> {
        if !*self.available.read().unwrap() {
            return Err(SyncError::Storage("Storage not available".to_string()));
        }
        Ok(*self.indexed_height.lock().await)
    }

    async fn set_indexed_height(&mut self, height: u32) -> SyncResult<()> {
        if !*self.available.read().unwrap() {
            return Err(SyncError::Storage("Storage not available".to_string()));
        }
        let mut h = self.indexed_height.lock().await;
        *h = height;
        Ok(())
    }

    async fn store_block_hash(&mut self, height: u32, hash: &[u8]) -> SyncResult<()> {
        if !*self.available.read().unwrap() {
            return Err(SyncError::Storage("Storage not available".to_string()));
        }
        let mut hashes = self.block_hashes.lock().await;
        hashes.insert(height, hash.to_vec());
        Ok(())
    }

    async fn get_block_hash(&self, height: u32) -> SyncResult<Option<Vec<u8>>> {
        if !*self.available.read().unwrap() {
            return Err(SyncError::Storage("Storage not available".to_string()));
        }
        let hashes = self.block_hashes.lock().await;
        Ok(hashes.get(&height).cloned())
    }

    async fn store_state_root(&mut self, height: u32, root: &[u8]) -> SyncResult<()> {
        if !*self.available.read().unwrap() {
            return Err(SyncError::Storage("Storage not available".to_string()));
        }
        let mut roots = self.state_roots.lock().await;
        roots.insert(height, root.to_vec());
        Ok(())
    }

    async fn get_state_root(&self, height: u32) -> SyncResult<Option<Vec<u8>>> {
        if !*self.available.read().unwrap() {
            return Err(SyncError::Storage("Storage not available".to_string()));
        }
        let roots = self.state_roots.lock().await;
        Ok(roots.get(&height).cloned())
    }

    async fn rollback_to_height(&mut self, height: u32) -> SyncResult<()> {
        if !*self.available.read().unwrap() {
            return Err(SyncError::Storage("Storage not available".to_string()));
        }
        {
            let mut hashes = self.block_hashes.lock().await;
            hashes.retain(|&h, _| h <= height);
        }
        {
            let mut roots = self.state_roots.lock().await;
            roots.retain(|&h, _| h <= height);
        }
        let mut indexed = self.indexed_height.lock().await;
        *indexed = height;
        Ok(())
    }

    async fn is_available(&self) -> bool {
        *self.available.read().unwrap()
    }

    async fn get_stats(&self) -> SyncResult<StorageStats> {
        if !*self.available.read().unwrap() {
            return Err(SyncError::Storage("Storage not available".to_string()));
        }
        let indexed_height = *self.indexed_height.lock().await;
        let hashes = self.block_hashes.lock().await;
        let roots = self.state_roots.lock().await;
        let total_entries = hashes.len() + roots.len();
        Ok(StorageStats {
            total_entries,
            indexed_height,
            storage_size_bytes: Some((total_entries * 64) as u64),
        })
    }
}

/// Mock runtime adapter for testing
#[derive(Debug, Clone)]
pub struct MockRuntime {
    blocks_processed: Arc<Mutex<u32>>,
    ready: Arc<RwLock<bool>>,
    memory_usage: Arc<Mutex<usize>>,
}

impl MockRuntime {
    pub fn new() -> Self {
        Self {
            blocks_processed: Arc::new(Mutex::new(0)),
            ready: Arc::new(RwLock::new(true)),
            memory_usage: Arc::new(Mutex::new(1024 * 1024)),
        }
    }

    pub fn set_ready(&self, ready: bool) {
        let mut r = self.ready.write().unwrap();
        *r = ready;
    }

    pub async fn get_blocks_processed(&self) -> u32 {
        *self.blocks_processed.lock().await
    }
}

#[async_trait]
impl RuntimeAdapter for MockRuntime {
    async fn process_block(&self, _height: u32, _block_data: &[u8]) -> SyncResult<()> {
        let mut processed = self.blocks_processed.lock().await;
        *processed += 1;
        Ok(())
    }

    async fn process_block_atomic(
        &self,
        height: u32,
        _block_data: &[u8],
        block_hash: &[u8],
    ) -> SyncResult<AtomicBlockResult> {
        let mut processed = self.blocks_processed.lock().await;
        *processed += 1;
        Ok(AtomicBlockResult {
            state_root: vec![0; 32],
            batch_data: vec![],
            height,
            block_hash: block_hash.to_vec(),
        })
    }

    async fn execute_view(&self, call: ViewCall) -> SyncResult<ViewResult> {
        if !*self.ready.read().unwrap() {
            return Err(SyncError::Runtime("Runtime not ready".to_string()));
        }
        let mut result_data = call.input_data;
        result_data.extend_from_slice(&call.height.to_le_bytes());
        result_data.extend_from_slice(call.function_name.as_bytes());
        Ok(ViewResult { data: result_data })
    }

    async fn execute_preview(&self, call: PreviewCall) -> SyncResult<ViewResult> {
        if !*self.ready.read().unwrap() {
            return Err(SyncError::Runtime("Runtime not ready".to_string()));
        }
        let mut result_data = call.block_data;
        result_data.extend_from_slice(&call.input_data);
        result_data.extend_from_slice(&call.height.to_le_bytes());
        result_data.extend_from_slice(call.function_name.as_bytes());
        Ok(ViewResult { data: result_data })
    }

    async fn get_state_root(&self, height: u32) -> SyncResult<Vec<u8>> {
        if !*self.ready.read().unwrap() {
            return Err(SyncError::Runtime("Runtime not ready".to_string()));
        }
        let mut state_root = vec![0u8; 32];
        let height_bytes = height.to_le_bytes();
        for i in 0..8 {
            state_root[i] = height_bytes[i % 4];
            state_root[i + 8] = height_bytes[i % 4];
            state_root[i + 16] = height_bytes[i % 4];
            state_root[i + 24] = height_bytes[i % 4];
        }
        Ok(state_root)
    }

    async fn refresh_memory(&self) -> SyncResult<()> {
        if !*self.ready.read().unwrap() {
            return Err(SyncError::Runtime("Runtime not ready".to_string()));
        }
        let mut memory = self.memory_usage.lock().await;
        *memory = 1024 * 1024;
        log::debug!("Mock runtime memory refreshed");
        Ok(())
    }

    async fn is_ready(&self) -> bool {
        *self.ready.read().unwrap()
    }

    async fn get_stats(&self) -> SyncResult<RuntimeStats> {
        if !*self.ready.read().unwrap() {
            return Err(SyncError::Runtime("Runtime not ready".to_string()));
        }
        let memory_usage = *self.memory_usage.lock().await;
        let blocks_processed = *self.blocks_processed.lock().await;
        Ok(RuntimeStats {
            memory_usage_bytes: memory_usage,
            blocks_processed,
            last_refresh_height: Some(blocks_processed),
        })
    }
}