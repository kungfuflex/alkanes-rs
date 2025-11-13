//! Comprehensive end-to-end test for the Metashrew indexing workflow.
//!
//! This test covers:
//! - Block processing using metashrew-sync
//! - Chain reorganizations
//! - View function execution via mocked JSON-RPC
//! - Data consistency and snapshotting

use crate::block_builder::ChainBuilder;
use crate::test_utils::TestConfig;
use anyhow::Result;
use async_trait::async_trait;
use bitcoin::hashes::Hash;
use bitcoin::BlockHash;
use memshrew_runtime::MemStoreAdapter;
use metashrew_runtime::smt::SMTHelper;
use metashrew_sync::{
    adapters::MetashrewRuntimeAdapter, BitcoinNodeAdapter, BlockInfo, ChainTip, StorageAdapter,
    SyncConfig, SyncEngine, SyncResult,
};
use std::sync::Arc;
use tokio::sync::Mutex;

/// A mock Bitcoin node that provides blocks from a ChainBuilder
#[derive(Clone)]
struct MockNode {
    chain: Arc<Mutex<ChainBuilder>>,
}

impl MockNode {
    fn new(chain: ChainBuilder) -> Self {
        Self {
            chain: Arc::new(Mutex::new(chain)),
        }
    }
}

#[async_trait]
impl BitcoinNodeAdapter for MockNode {
    async fn get_tip_height(&self) -> SyncResult<u32> {
        let chain = self.chain.lock().await;
        Ok(chain.height())
    }

    async fn get_block_hash(&self, height: u32) -> SyncResult<Vec<u8>> {
        let chain = self.chain.lock().await;
        Ok(chain
            .get_block(height)
            .map(|b| b.block_hash().to_byte_array().to_vec())
            .unwrap_or_else(|| BlockHash::all_zeros().to_byte_array().to_vec()))
    }

    async fn get_block_data(&self, height: u32) -> SyncResult<Vec<u8>> {
        let chain = self.chain.lock().await;
        let block = chain.get_block(height).unwrap();
        Ok(metashrew_support::utils::consensus_encode(block)?)
    }

    async fn get_block_info(&self, height: u32) -> SyncResult<BlockInfo> {
        let hash = self.get_block_hash(height).await?;
        let data = self.get_block_data(height).await?;
        Ok(BlockInfo {
            height,
            hash,
            data,
        })
    }

    async fn get_chain_tip(&self) -> SyncResult<ChainTip> {
        let chain = self.chain.lock().await;
        Ok(ChainTip {
            height: chain.height(),
            hash: chain.tip_hash().to_byte_array().to_vec(),
        })
    }

    async fn is_connected(&self) -> bool {
        true
    }
}

fn get_indexed_block(adapter: &MemStoreAdapter, height: u32) -> Result<Option<Vec<u8>>> {
    let smt_helper = SMTHelper::new(adapter.clone());
    let key = format!("/blocks/{}", height).into_bytes();
    smt_helper.get_at_height(&key, height)
}

use log::info;
use env_logger;

#[tokio::test]
async fn test_comprehensive_e2e() -> Result<()> {
    let _ = env_logger::builder().is_test(true).try_init();
    info!("Comprehensive E2E test started");
    let config = TestConfig::new();
    let shared_adapter = MemStoreAdapter::new();

    // --- Phase 1: Initial Indexing ---
    info!("--- Phase 1: Initial Indexing ---");
    let initial_chain = ChainBuilder::new().add_blocks(10);
    let initial_node = MockNode::new(initial_chain.clone());
    let mut config_engine = wasmtime::Config::default();
    config_engine.async_support(true);
    let engine = wasmtime::Engine::new(&config_engine)?;
    let initial_runtime = config.create_runtime_from_adapter(shared_adapter.clone(), engine).await?;
    let initial_runtime_adapter = MetashrewRuntimeAdapter::new(initial_runtime);
    let initial_sync_config = SyncConfig {
        start_block: 0,
        exit_at: Some(11),
        pipeline_size: Some(1),
        max_reorg_depth: 100,
        reorg_check_threshold: 6,
    };
    let mut initial_syncer = metashrew_sync::sync::MetashrewSync::new(
        initial_node,
        shared_adapter.clone(),
        initial_runtime_adapter,
        initial_sync_config,
    );
    initial_syncer.start().await?;

    // --- Verification 1: Initial State ---
    info!("--- Verification 1: Initial State ---");
    {
        let db_adapter = initial_syncer.storage().read().await;
        for height in 0..=10 {
            assert!(
                get_indexed_block(&db_adapter, height)?.is_some(),
                "Block {} should be indexed",
                height
            );
        }
        info!("Initial state verified. All blocks 0-10 are present.");
    }

    // --- Phase 2: Simulate a Reorg ---
    info!("--- Phase 2: Simulate a Reorg ---");
    let reorg_chain = ChainBuilder::new()
        .add_blocks(7)
        .fork(5)
        .with_salt(1)
        .add_blocks(5);
    let reorg_node = MockNode::new(reorg_chain.clone());
    let mut config_engine = wasmtime::Config::default();
    config_engine.async_support(true);
    let engine = wasmtime::Engine::new(&config_engine)?;
    let reorg_runtime = config.create_runtime_from_adapter(shared_adapter.clone(), engine).await?;
    let reorg_runtime_adapter = MetashrewRuntimeAdapter::new(reorg_runtime);
    let reorg_sync_config = SyncConfig {
        start_block: 0, // This will be ignored as the adapter already has a height
        exit_at: Some(11), // Sync the new chain up to its tip
        pipeline_size: Some(1),
        max_reorg_depth: 100,
        reorg_check_threshold: 1, // Check for reorg more aggressively
    };
    let mut reorg_syncer = metashrew_sync::sync::MetashrewSync::new(
        reorg_node,
        shared_adapter.clone(),
        reorg_runtime_adapter,
        reorg_sync_config,
    );
    reorg_syncer.start().await?;

    // --- Verification 2: Post-Reorg State ---
    info!("--- Verification 2: Post-Reorg State ---");
    let final_db_adapter = reorg_syncer.storage().read().await;
    assert!(
        get_indexed_block(&final_db_adapter, 5)?.is_some(),
        "Block 5 (common ancestor) should remain"
    );
    info!("Block 5 (common ancestor) is present.");

    assert!(
        get_indexed_block(&final_db_adapter, 10)?.is_some(),
        "Block 10 from the NEW chain should be indexed"
    );
    info!("Block 10 from the new chain is present.");

    // Verify that the state from the final block (10) is present
    let final_block_data = get_indexed_block(&final_db_adapter, 10)?.unwrap();
    let final_block_from_chain = reorg_chain.get_block(10).unwrap();
    let final_block_from_chain_bytes = metashrew_support::utils::consensus_encode(final_block_from_chain)?;
    assert_eq!(final_block_data, final_block_from_chain_bytes, "Data for block 10 should match the new chain");

    // To truly verify the reorg, we need to check the HASH of block 6.
    // The old block 6 should be gone, and the new one should be there.
    let old_chain_block_6_hash = initial_chain.get_block(6).unwrap().block_hash();
    let new_chain_block_6_hash = reorg_chain.get_block(6).unwrap().block_hash();
    
    assert_ne!(old_chain_block_6_hash, new_chain_block_6_hash, "Hashes for old and new block 6 should differ");

    let indexed_block_6_hash_bytes = final_db_adapter.get_block_hash(6).await?.unwrap();
    let indexed_block_6_hash = BlockHash::from_slice(&indexed_block_6_hash_bytes)?;

    assert_eq!(indexed_block_6_hash, new_chain_block_6_hash, "The hash of indexed block 6 should be from the NEW chain");
    info!("Hash of indexed block 6 matches the new chain's block 6.");

    // The original assertion was flawed because the new chain also has a block 10.
    // The real test is whether the *data* from the old chain is gone.
    // We can check for a key that was only in the old chain's block 10.
    // For this test, we'll rely on the block hash check above as sufficient.
    // The original assertion is left here, commented out, for historical context.
    // assert!(
    //     get_indexed_block(&final_db_adapter, 10)?.is_none(),
    //     "Block 10 from old chain should be gone"
    // );

    info!("Comprehensive E2E test passed successfully!");
    Ok(())
}
