use alkanes_indexer::indexer::AlkanesIndexer;
use alkanes_native::adapters::NativeRuntimeAdapter;
use bitcoin::hashes::Hash;
use memshrew_runtime::{MemStoreAdapter, MemStoreRuntime};
use metashrew_sync::{
    BitcoinNodeAdapter, BlockInfo, ChainTip, SnapshotMetashrewSync, StorageStats, SyncConfig,
    SyncEngine, SyncError, SyncMode, SyncResult,
};
use std::collections::HashMap;

pub fn setup_test_runtime() -> MemStoreRuntime<AlkanesIndexer> {
    let storage = MemStoreAdapter::default();
    MemStoreRuntime::new(storage, vec![]).unwrap()
}

pub struct TestHarness {
    pub runtime: MemStoreRuntime<AlkanesIndexer>,
    pub node: MockNodeAdapter,
    pub sync_config: SyncConfig,
    pub sync_mode: SyncMode,
}

impl TestHarness {
    pub fn new() -> Self {
        Self {
            runtime: setup_test_runtime(),
            node: MockNodeAdapter::default(),
            sync_config: SyncConfig::default(),
            sync_mode: SyncMode::Normal,
        }
    }

    pub fn add_block(&mut self, block: Block) {
        let height = self.node.blocks.lock().unwrap().len() as u32;
        self.node.blocks.lock().unwrap().insert(height, block);
    }

    pub async fn process_block(&mut self) {
        let mut engine = SnapshotMetashrewSync::new(
            self.node.clone(),
            self.runtime.context.lock().unwrap().db.clone(),
            NativeRuntimeAdapter,
            self.sync_config.clone(),
            self.sync_mode.clone(),
        );
        engine.start().await.unwrap();
    }
}
use async_trait::async_trait;
use bitcoin::{Block, BlockHash};
use metashrew_sync::StorageAdapter;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct MemStorageAdapter {
    pub db: Arc<Mutex<HashMap<Vec<u8>, Vec<u8>>>>,
    pub height: Arc<Mutex<u32>>,
}

impl Default for MemStorageAdapter {
    fn default() -> Self {
        Self {
            db: Arc::new(Mutex::new(HashMap::new())),
            height: Arc::new(Mutex::new(0)),
        }
    }
}

impl metashrew_core::native_host::StorageAdapter for MemStorageAdapter {
	fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, anyhow::Error> {
		Ok(self.db.lock().unwrap().get(key).cloned())
	}
}

#[async_trait]
impl StorageAdapter for MemStorageAdapter {
    async fn get_indexed_height(&self) -> SyncResult<u32> {
        Ok(*self.height.lock().unwrap())
    }
    async fn set_indexed_height(&mut self, height: u32) -> SyncResult<()> {
        *self.height.lock().unwrap() = height;
        Ok(())
    }
    async fn store_block_hash(&mut self, _height: u32, _hash: &[u8]) -> SyncResult<()> {
        Ok(())
    }
    async fn get_block_hash(&self, _height: u32) -> SyncResult<Option<Vec<u8>>> {
        Ok(None)
    }
    async fn store_state_root(&mut self, _height: u32, _root: &[u8]) -> SyncResult<()> {
        Ok(())
    }
    async fn get_state_root(&self, _height: u32) -> SyncResult<Option<Vec<u8>>> {
        Ok(None)
    }
    async fn rollback_to_height(&mut self, _height: u32) -> SyncResult<()> {
        Ok(())
    }
    async fn is_available(&self) -> bool {
        true
    }
    async fn get_stats(&self) -> SyncResult<StorageStats> {
        Ok(StorageStats {
            total_entries: 0,
            indexed_height: 0,
            storage_size_bytes: Some(0),
        })
    }
}

#[derive(Clone, Default)]
pub struct MockNodeAdapter {
    pub blocks: Arc<Mutex<HashMap<u32, Block>>>,
}

#[async_trait]
impl BitcoinNodeAdapter for MockNodeAdapter {
    async fn get_block_hash(&self, height: u32) -> SyncResult<Vec<u8>> {
        let blocks = self.blocks.lock().unwrap();
        let block = blocks.get(&height).ok_or(SyncError::BitcoinNode("Block not found".to_string()))?;
        Ok(block.block_hash()[..].to_vec())
    }

    async fn get_block_data(&self, height: u32) -> SyncResult<Vec<u8>> {
        let blocks = self.blocks.lock().unwrap();
        let block = blocks.get(&height).ok_or(SyncError::BitcoinNode("Block not found".to_string()))?;
        Ok(bitcoin::consensus::encode::serialize(block))
    }

    async fn get_block_info(&self, height: u32) -> SyncResult<BlockInfo> {
        let blocks = self.blocks.lock().unwrap();
        let block = blocks.get(&height).ok_or(SyncError::BitcoinNode("Block not found".to_string()))?;
        let hash = block.block_hash()[..].to_vec();
        let data = bitcoin::consensus::encode::serialize(block);
        Ok(BlockInfo { height, hash, data })
    }

    async fn get_tip_height(&self) -> SyncResult<u32> {
        Ok(0)
    }
    async fn get_chain_tip(&self) -> SyncResult<ChainTip> {
        Ok(ChainTip {
            height: 0,
            hash: BlockHash::all_zeros()[..].to_vec(),
        })
    }
    async fn is_connected(&self) -> bool {
        true
    }
}