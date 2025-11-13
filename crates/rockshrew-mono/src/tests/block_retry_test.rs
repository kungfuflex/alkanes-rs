//! Test to verify that the block skipping fix works correctly
//!
//! This test verifies that failed blocks are properly retried instead of skipped,
//! ensuring that all blocks are eventually indexed even with intermittent failures.

use crate::block_builder::ChainBuilder;
use crate::test_utils::TestConfig;
use anyhow::Result;
use async_trait::async_trait;
use bitcoin::hashes::Hash;
use bitcoin::BlockHash;
use log::{info, warn};
use memshrew_runtime::MemStoreAdapter;
use metashrew_runtime::smt::SMTHelper;
use metashrew_sync::{
    adapters::MetashrewRuntimeAdapter, BitcoinNodeAdapter, BlockInfo, ChainTip, RuntimeAdapter, AtomicBlockResult, ViewCall, PreviewCall, ViewResult, SyncConfig, SyncEngine, SyncError, SyncResult, RuntimeStats,
};
use std::sync::Arc;
use tokio::sync::Mutex;
use std::collections::HashMap;

/// A mock Bitcoin node that fails a few times then succeeds
#[derive(Clone)]
struct RetryingMockNode {
    chain: Arc<Mutex<ChainBuilder>>,
    failure_counts: Arc<Mutex<std::collections::HashMap<u32, u32>>>,
    max_failures_per_block: u32,
}

impl RetryingMockNode {
    fn new(chain: ChainBuilder, max_failures_per_block: u32) -> Self {
        Self {
            chain: Arc::new(Mutex::new(chain)),
            failure_counts: Arc::new(Mutex::new(std::collections::HashMap::new())),
            max_failures_per_block,
        }
    }

    async fn should_fail(&self, height: u32) -> bool {
        let mut counts = self.failure_counts.lock().await;
        let current_failures = counts.entry(height).or_insert(0);
        
        if *current_failures < self.max_failures_per_block {
            *current_failures += 1;
            warn!("Simulating failure #{} for block {}", *current_failures, height);
            true
        } else {
            info!("Block {} will succeed after {} failures", height, *current_failures);
            false
        }
    }
}

#[async_trait]
impl BitcoinNodeAdapter for RetryingMockNode {
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
        if self.should_fail(height).await {
            return Err(metashrew_sync::SyncError::BitcoinNode(
                format!("Simulated temporary failure for block {}", height)
            ));
        }
        let hash = self.get_block_hash(height).await?;
        let chain = self.chain.lock().await;
        let block = chain.get_block(height).unwrap();
        let data = metashrew_support::utils::consensus_encode(block)?;
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

#[tokio::test]
async fn test_block_retry_fix() -> Result<()> {
    let _ = env_logger::builder().is_test(true).try_init();
    info!("Block retry fix test started");
    
    let config = TestConfig::new();
    let shared_adapter = MemStoreAdapter::new();

    // Create a chain with 10 blocks
    let chain = ChainBuilder::new().add_blocks(10);
    
    // Create a node that fails 2 times per block before succeeding
    let retrying_node = RetryingMockNode::new(chain.clone(), 2);
    
    let mut config_engine = wasmtime::Config::default();
    config_engine.async_support(true);
    let engine = wasmtime::Engine::new(&config_engine)?;
    
    let runtime = config.create_runtime_from_adapter(shared_adapter.clone(), engine).await?;
    let runtime_adapter = MetashrewRuntimeAdapter::new(runtime);
    let counting_adapter = CountingRuntimeAdapter::new(runtime_adapter);
    
    let sync_config = SyncConfig {
        start_block: 0,
        exit_at: Some(11), // Process blocks 0-10
        pipeline_size: Some(4), // Use a larger pipeline to test concurrency
        max_reorg_depth: 100,
        reorg_check_threshold: 6,
    };

    let mut syncer = metashrew_sync::sync::MetashrewSync::new(
        retrying_node,
        shared_adapter.clone(),
        counting_adapter.clone(),
        sync_config,
    );

    // This should now work correctly - all blocks should be indexed despite failures
    info!("Starting sync with retrying failures...");
    syncer.start().await?;

    // Check that ALL blocks were indexed
    info!("Checking that all blocks were indexed...");
    let mut indexed_blocks = Vec::new();
    let mut missing_blocks = Vec::new();
    
    for height in 0..=10 {
        if get_indexed_block(&shared_adapter, height)?.is_some() {
            indexed_blocks.push(height);
            info!("Block {} was indexed ✓", height);
        } else {
            missing_blocks.push(height);
            info!("Block {} is MISSING ✗", height);
        }
    }

    info!("Indexed blocks: {:?}", indexed_blocks);
    info!("Missing blocks: {:?}", missing_blocks);
    info!("Total indexed: {}/11", indexed_blocks.len());

    // With the fix, ALL blocks should be indexed
    assert!(
        missing_blocks.is_empty(),
        "Expected all blocks to be indexed with the fix, but missing blocks: {:?}",
        missing_blocks
    );

    assert_eq!(
        indexed_blocks.len(),
        11,
        "Expected all 11 blocks (0-10) to be indexed"
    );

    // Check that each block was processed exactly once
    let counts = counting_adapter.call_counts.lock().await;
    for height in 0..=10 {
        let count = counts.get(&height).unwrap_or(&0);
        assert_eq!(*count, 1, "Block {} should be processed exactly once, but was processed {} times", height, count);
    }

    info!("Block retry fix test passed! All blocks were successfully indexed despite failures and processed exactly once.");
    
    Ok(())
}

#[tokio::test]
async fn test_snapshot_sync_retry_fix() -> Result<()> {
    let _ = env_logger::builder().is_test(true).try_init();
    info!("Snapshot sync retry fix test started");
    
    let config = TestConfig::new();
    let shared_adapter = MemStoreAdapter::new();

    // Create a chain with 10 blocks
    let chain = ChainBuilder::new().add_blocks(10);
    
    // Create a node that fails 2 times per block before succeeding
    let retrying_node = RetryingMockNode::new(chain.clone(), 2);
    let mut config_engine = wasmtime::Config::default();
    config_engine.async_support(true);
    let engine = wasmtime::Engine::new(&config_engine)?;
    let runtime = config.create_runtime_from_adapter(shared_adapter.clone(), engine).await?;
    let runtime_adapter = MetashrewRuntimeAdapter::new(runtime);
    
    let sync_config = SyncConfig {
        start_block: 0,
        exit_at: Some(11), // Process blocks 0-10
        pipeline_size: Some(1), // Single-threaded to make the test deterministic
        max_reorg_depth: 100,
        reorg_check_threshold: 6,
    };

    let mut syncer = metashrew_sync::snapshot_sync::SnapshotMetashrewSync::new(
        retrying_node,
        shared_adapter.clone(),
        runtime_adapter,
        sync_config,
        metashrew_sync::SyncMode::Normal,
    );

    // This should now work correctly with snapshot sync too
    info!("Starting snapshot sync with retrying failures...");
    syncer.start().await?;

    // Check that ALL blocks were indexed
    info!("Checking that all blocks were indexed...");
    let mut indexed_blocks = Vec::new();
    let mut missing_blocks = Vec::new();
    
    for height in 0..=10 {
        if get_indexed_block(&shared_adapter, height)?.is_some() {
            indexed_blocks.push(height);
            info!("Block {} was indexed ✓", height);
        } else {
            missing_blocks.push(height);
            info!("Block {} is MISSING ✗", height);
        }
    }

    info!("Indexed blocks: {:?}", indexed_blocks);
    info!("Missing blocks: {:?}", missing_blocks);
    info!("Total indexed: {}/11", indexed_blocks.len());

    // With the fix, ALL blocks should be indexed
    assert!(
        missing_blocks.is_empty(),
        "Expected all blocks to be indexed with snapshot sync fix, but missing blocks: {:?}",
        missing_blocks
    );

    assert_eq!(
        indexed_blocks.len(),
        11,
        "Expected all 11 blocks (0-10) to be indexed"
    );

    info!("Snapshot sync retry fix test passed! All blocks were successfully indexed despite failures.");
    
    Ok(())
}


#[derive(Clone)]
struct CountingRuntimeAdapter<T: RuntimeAdapter> {
    inner: T,
    call_counts: Arc<Mutex<HashMap<u32, u32>>>,
}

impl<T: RuntimeAdapter> CountingRuntimeAdapter<T> {
    fn new(inner: T) -> Self {
        Self {
            inner,
            call_counts: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl<T: RuntimeAdapter + Clone + Send + Sync> RuntimeAdapter for CountingRuntimeAdapter<T> {
    async fn process_block(&self, height: u32, block_data: &[u8]) -> SyncResult<()> {
        let mut counts = self.call_counts.lock().await;
        *counts.entry(height).or_insert(0) += 1;
        self.inner.process_block(height, block_data).await
    }

    async fn process_block_atomic(
        &self,
        height: u32,
        block_data: &[u8],
        block_hash: &[u8],
    ) -> SyncResult<AtomicBlockResult> {
        let mut counts = self.call_counts.lock().await;
        *counts.entry(height).or_insert(0) += 1;
        self.inner.process_block_atomic(height, block_data, block_hash).await
    }

    async fn get_state_root(&self, height: u32) -> SyncResult<Vec<u8>> {
        self.inner.get_state_root(height).await
    }
    
    async fn execute_view(&self, call: ViewCall) -> SyncResult<ViewResult> {
        self.inner.execute_view(call).await
    }

    async fn execute_preview(&self, call: PreviewCall) -> SyncResult<ViewResult> {
        self.inner.execute_preview(call).await
    }
    
    async fn refresh_memory(&self) -> SyncResult<()> {
        self.inner.refresh_memory().await
    }

    async fn is_ready(&self) -> bool {
        self.inner.is_ready().await
    }

    async fn get_stats(&self) -> SyncResult<RuntimeStats> {
        Ok(RuntimeStats {
            memory_usage_bytes: 0,
            blocks_processed: 0,
            last_refresh_height: None,
        })
    }
}

#[derive(Clone)]
struct CrashingRuntimeAdapter {
    crash_at_height: u32,
    processed_heights: Arc<Mutex<Vec<u32>>>,
}

impl CrashingRuntimeAdapter {
    fn new(crash_at_height: u32) -> Self {
        Self {
            crash_at_height,
            processed_heights: Arc::new(Mutex::new(vec![])),
        }
    }
}

#[async_trait]
impl RuntimeAdapter for CrashingRuntimeAdapter {
    async fn process_block(&self, height: u32, _block_data: &[u8]) -> SyncResult<()> {
        if height == self.crash_at_height {
            return Err(SyncError::Runtime("indexer exited unexpectedly".to_string()));
        }
        let mut processed = self.processed_heights.lock().await;
        if !processed.contains(&height) {
            processed.push(height);
        }
        Ok(())
    }

    async fn process_block_atomic(
        &self,
        height: u32,
        _block_data: &[u8],
        block_hash: &[u8],
    ) -> SyncResult<AtomicBlockResult> {
        if height == self.crash_at_height {
            return Err(SyncError::Runtime("indexer exited unexpectedly".to_string()));
        }
        let mut processed = self.processed_heights.lock().await;
        if !processed.contains(&height) {
            processed.push(height);
        }
        Ok(AtomicBlockResult {
            state_root: vec![0; 32],
            batch_data: vec![],
            height,
            block_hash: block_hash.to_vec(),
        })
    }

    async fn get_state_root(&self, _height: u32) -> SyncResult<Vec<u8>> {
        Ok(vec![0; 32])
    }
    
    async fn execute_view(&self, _call: ViewCall) -> SyncResult<ViewResult> {
        unimplemented!()
    }

    async fn execute_preview(&self, _call: PreviewCall) -> SyncResult<ViewResult> {
        unimplemented!()
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

#[tokio::test]
async fn test_abort_on_critical_failure() -> Result<()> {
    let _ = env_logger::builder().is_test(true).try_init();
    info!("Abort on critical failure test started");

    let chain = ChainBuilder::new().add_blocks(10);
    let node = RetryingMockNode::new(chain.clone(), 0); // No retries, should succeed
    let storage = MemStoreAdapter::new();
    
    let crash_height = 5;
    let runtime_adapter = CrashingRuntimeAdapter::new(crash_height);

    let sync_config = SyncConfig {
        start_block: 0,
        exit_at: Some(11),
        pipeline_size: Some(1),
        max_reorg_depth: 100,
        reorg_check_threshold: 6,
    };

    let mut syncer = metashrew_sync::sync::MetashrewSync::new(
        node,
        storage.clone(),
        runtime_adapter.clone(),
        sync_config,
    );

    info!("Starting sync with crashing runtime...");
    let result = syncer.start().await;

    assert!(result.is_err(), "Expected syncer to return an error");

    if let Err(e) = result {
        let error_message = e.to_string();
        assert!(
            error_message.contains("indexer exited unexpectedly"),
            "Error message should indicate critical failure"
        );
    }

    // Check that blocks after the crash were not processed
    let processed_heights = runtime_adapter.processed_heights.lock().await;
    assert!(!processed_heights.contains(&(crash_height)), "Block at crash height should not be processed");
    assert!(!processed_heights.contains(&(crash_height + 1)), "Blocks after crash height should not be processed");
    
    // Check that blocks before the crash were processed
    for height in 0..crash_height {
        assert!(processed_heights.contains(&height), "Block {} should have been processed", height);
    }

    info!("Abort on critical failure test passed!");

    Ok(())
}
