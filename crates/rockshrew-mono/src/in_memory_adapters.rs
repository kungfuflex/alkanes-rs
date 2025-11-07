//! In-memory adapters for comprehensive e2e testing
use anyhow::Error as AnyhowError;
use async_trait::async_trait;
use bitcoin::{Block, hashes::Hash};
use metashrew_runtime::{MetashrewRuntime};
use memshrew_runtime::MemStoreAdapter as MemStore;
use metashrew_sync::{
    BitcoinNodeAdapter, BlockInfo, ChainTip, SyncError, SyncResult,
    RuntimeAdapter, ViewCall, ViewResult, PreviewCall, AtomicBlockResult, RuntimeStats,
};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use wasmtime::Engine;

/// An in-memory Bitcoin node adapter for testing
#[derive(Clone)]
pub struct InMemoryBitcoinNode {
    blocks: Arc<RwLock<HashMap<u32, Block>>>,
    tip: Arc<RwLock<ChainTip>>,
}

impl InMemoryBitcoinNode {
    pub fn new(genesis_block: Block) -> Self {
        let mut blocks = HashMap::new();
        let tip = ChainTip {
            height: 0,
            hash: genesis_block.block_hash().to_byte_array().to_vec(),
        };
        blocks.insert(0, genesis_block);
        Self {
            blocks: Arc::new(RwLock::new(blocks)),
            tip: Arc::new(RwLock::new(tip)),
        }
    }

    pub fn add_block(&self, block: Block, height: u32) {
        let mut blocks = self.blocks.write().unwrap();
        let mut tip = self.tip.write().unwrap();
        tip.height = height;
        tip.hash = block.block_hash().to_byte_array().to_vec();
        blocks.insert(height, block);
    }

    pub fn get_block(&self, height: u32) -> Option<Block> {
        self.blocks.read().unwrap().get(&height).cloned()
    }
}

#[async_trait]
impl BitcoinNodeAdapter for InMemoryBitcoinNode {
    async fn get_tip_height(&self) -> SyncResult<u32> {
        Ok(self.tip.read().unwrap().height)
    }

    async fn get_block_hash(&self, height: u32) -> SyncResult<Vec<u8>> {
        self.blocks
            .read()
            .unwrap()
            .get(&height)
            .map(|b| b.block_hash().to_byte_array().to_vec())
            .ok_or_else(|| SyncError::BitcoinNode(format!("Block not found at height {}", height)))
    }

    async fn get_block_data(&self, height: u32) -> SyncResult<Vec<u8>> {
        self.blocks
            .read()
            .unwrap()
            .get(&height)
            .map(|b| metashrew_support::utils::consensus_encode(b).unwrap())
            .ok_or_else(|| SyncError::BitcoinNode(format!("Block not found at height {}", height)))
    }

    async fn get_block_info(&self, height: u32) -> SyncResult<BlockInfo> {
        let block = self
            .blocks
            .read()
            .unwrap()
            .get(&height)
            .cloned()
            .ok_or_else(|| SyncError::BitcoinNode(format!("Block not found at height {}", height)))?;
        Ok(BlockInfo {
            height,
            hash: block.block_hash().to_byte_array().to_vec(),
            data: metashrew_support::utils::consensus_encode(&block).unwrap(),
        })
    }

    async fn get_chain_tip(&self) -> SyncResult<ChainTip> {
        Ok(self.tip.read().unwrap().clone())
    }

    async fn is_connected(&self) -> bool {
        true
    }
}

pub struct InMemoryRuntime {
    runtime: MetashrewRuntime<MemStore>,
}

impl InMemoryRuntime {
    pub async fn new(wasm_bytes: &[u8]) -> Self {
        let store = MemStore::new();
        let mut config = wasmtime::Config::default();
        config.async_support(true);
        let engine = Engine::new(&config).unwrap();
        let runtime = MetashrewRuntime::new(wasm_bytes, store, engine).await.unwrap();
        Self { runtime }
    }

    pub fn new_runtime_adapter(self) -> Box<dyn RuntimeAdapter> {
        Box::new(self)
    }
}

#[async_trait]
impl RuntimeAdapter for InMemoryRuntime {
    async fn process_block(&self, height: u32, block_data: &[u8]) -> SyncResult<()> {
        self.runtime.process_block(height, block_data).await.map_err(|e| SyncError::Runtime(e.to_string()))
    }

    async fn process_block_atomic(&self, height: u32, block_data: &[u8], block_hash: &[u8]) -> SyncResult<AtomicBlockResult> {
        self.runtime.process_block_atomic(height, block_data, block_hash).await.map(|res| AtomicBlockResult {
            state_root: res.state_root,
            batch_data: res.batch_data,
            height: res.height,
            block_hash: res.block_hash,
        }).map_err(|e: AnyhowError| SyncError::Runtime(e.to_string()))
    }

    async fn execute_view(&self, call: ViewCall) -> SyncResult<ViewResult> {
        self.runtime.view(call.function_name, &call.input_data, call.height).await.map(|res| ViewResult { data: res }).map_err(|e| SyncError::Runtime(e.to_string()))
    }

    async fn execute_preview(&self, call: PreviewCall) -> SyncResult<ViewResult> {
        self.runtime.preview_async(&call.block_data, call.function_name, &call.input_data, call.height).await.map(|res| ViewResult { data: res }).map_err(|e| {
            eprintln!("Preview execution error: {:?}", e);
            SyncError::Runtime(format!("Preview failed: {:?}", e))
        })
    }

    async fn get_state_root(&self, height: u32) -> SyncResult<Vec<u8>> {
        self.runtime.get_state_root(height).await.map_err(|e| SyncError::Runtime(e.to_string()))
    }

    async fn refresh_memory(&self) -> SyncResult<()> {
        Ok(())
    }

    async fn is_ready(&self) -> bool {
        true
    }

    async fn get_stats(&self) -> SyncResult<RuntimeStats> {
        Ok(RuntimeStats {
            memory_usage_bytes: 0,
            blocks_processed: 0,
            last_refresh_height: None,
        })
    }
}
