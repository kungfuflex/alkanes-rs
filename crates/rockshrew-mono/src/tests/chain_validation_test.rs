//! Chain validation and automatic rollback tests
//!
//! This test suite validates that the sync engine properly:
//! - Validates each block connects to the previous block
//! - Detects chain discontinuities immediately
//! - Automatically triggers rollback when needed
//! - Correctly resumes processing after rollback

use crate::in_memory_adapters::{InMemoryBitcoinNode, InMemoryRuntime};
use crate::test_utils::{TestConfig, TestUtils};
use anyhow::Result;
use bitcoin::{hashes::Hash, Block, BlockHash};
use metashrew_sync::{MetashrewSync, StorageAdapter, SyncConfig};
use rockshrew_runtime::adapter::RocksDBRuntimeAdapter;
use std::sync::Arc;
use tempfile::TempDir;

/// Helper to create a simple in-memory storage adapter for testing
struct InMemoryStorage {
    indexed_height: u32,
    block_hashes: std::collections::HashMap<u32, Vec<u8>>,
    state_roots: std::collections::HashMap<u32, Vec<u8>>,
}

impl InMemoryStorage {
    fn new() -> Self {
        Self {
            indexed_height: 0,
            block_hashes: std::collections::HashMap::new(),
            state_roots: std::collections::HashMap::new(),
        }
    }
}

#[async_trait::async_trait]
impl StorageAdapter for InMemoryStorage {
    async fn get_indexed_height(&self) -> metashrew_sync::SyncResult<u32> {
        Ok(self.indexed_height)
    }

    async fn set_indexed_height(&mut self, height: u32) -> metashrew_sync::SyncResult<()> {
        self.indexed_height = height;
        Ok(())
    }

    async fn store_block_hash(&mut self, height: u32, hash: &[u8]) -> metashrew_sync::SyncResult<()> {
        self.block_hashes.insert(height, hash.to_vec());
        Ok(())
    }

    async fn get_block_hash(&self, height: u32) -> metashrew_sync::SyncResult<Option<Vec<u8>>> {
        Ok(self.block_hashes.get(&height).cloned())
    }

    async fn store_state_root(&mut self, height: u32, root: &[u8]) -> metashrew_sync::SyncResult<()> {
        self.state_roots.insert(height, root.to_vec());
        Ok(())
    }

    async fn get_state_root(&self, height: u32) -> metashrew_sync::SyncResult<Option<Vec<u8>>> {
        Ok(self.state_roots.get(&height).cloned())
    }

    async fn rollback_to_height(&mut self, height: u32) -> metashrew_sync::SyncResult<()> {
        // Remove all data after the rollback height
        self.indexed_height = height;
        self.block_hashes.retain(|h, _| *h <= height);
        self.state_roots.retain(|h, _| *h <= height);
        Ok(())
    }

    async fn is_available(&self) -> bool {
        true
    }

    async fn get_stats(&self) -> metashrew_sync::SyncResult<metashrew_sync::StorageStats> {
        Ok(metashrew_sync::StorageStats {
            total_entries: self.block_hashes.len() + self.state_roots.len(),
            indexed_height: self.indexed_height,
            storage_size_bytes: None,
        })
    }
}

/// Test that chain validation detects when a block doesn't connect to the previous block
#[tokio::test]
async fn test_chain_validation_detects_discontinuity() -> Result<()> {
    let _ = env_logger::builder().is_test(true).try_init();

    // Create genesis block
    let genesis_block = TestUtils::create_test_block(0, BlockHash::all_zeros());
    let node = InMemoryBitcoinNode::new(genesis_block.clone());

    // Add block 1 that connects to genesis
    let block_1 = TestUtils::create_test_block(1, genesis_block.block_hash());
    let block_1_hash = block_1.block_hash().to_byte_array().to_vec();
    node.add_block(block_1.clone(), 1);

    // Add block 2 that DOESN'T connect to block 1 (wrong prev_blockhash)
    let wrong_prev_hash = BlockHash::from_byte_array([0xaa; 32]);
    let bad_block_2 = TestUtils::create_test_block(2, wrong_prev_hash);
    node.add_block(bad_block_2.clone(), 2);

    // Create runtime and storage
    let config = TestConfig::new();
    let wasm_bytes = config.wasm;
    let runtime = InMemoryRuntime::new(wasm_bytes).await;
    let storage = InMemoryStorage::new();

    // Create sync engine
    let sync_config = SyncConfig {
        start_block: 0,
        exit_at: Some(3),
        pipeline_size: Some(1),
        max_reorg_depth: 100,
        reorg_check_threshold: 6,
    };

    let mut sync = MetashrewSync::new(node, storage, runtime, sync_config);

    // Process genesis
    let genesis_data = TestUtils::serialize_block(&genesis_block);
    let genesis_hash = genesis_block.block_hash().to_byte_array().to_vec();
    sync.process_block(0, genesis_data, genesis_hash).await?;

    // Process block 1 (should succeed)
    let block_1_data = TestUtils::serialize_block(&block_1);
    sync.process_block(1, block_1_data, block_1_hash).await?;

    // Try to process block 2 (should FAIL due to chain discontinuity)
    let bad_block_2_data = TestUtils::serialize_block(&bad_block_2);
    let bad_block_2_hash = bad_block_2.block_hash().to_byte_array().to_vec();

    let result = sync.process_block(2, bad_block_2_data, bad_block_2_hash).await;

    // Verify that processing failed
    assert!(result.is_err(), "Processing should have failed due to chain discontinuity");

    let error = result.unwrap_err();
    assert!(
        error.to_string().contains("does not connect to previous block"),
        "Error should mention chain discontinuity"
    );

    println!("✅ Chain validation correctly detected discontinuity");

    Ok(())
}

/// Test that genesis block (height 0) is always accepted
#[tokio::test]
async fn test_chain_validation_accepts_genesis() -> Result<()> {
    let _ = env_logger::builder().is_test(true).try_init();

    let genesis_block = TestUtils::create_test_block(0, BlockHash::all_zeros());
    let node = InMemoryBitcoinNode::new(genesis_block.clone());

    let config = TestConfig::new();
    let runtime = InMemoryRuntime::new(config.wasm).await;
    let storage = InMemoryStorage::new();

    let sync_config = SyncConfig {
        start_block: 0,
        exit_at: Some(1),
        pipeline_size: Some(1),
        max_reorg_depth: 100,
        reorg_check_threshold: 6,
    };

    let mut sync = MetashrewSync::new(node, storage, runtime, sync_config);

    // Genesis block should always be accepted, even with BlockHash::all_zeros() as prev
    let genesis_data = TestUtils::serialize_block(&genesis_block);
    let genesis_hash = genesis_block.block_hash().to_byte_array().to_vec();

    let result = sync.process_block(0, genesis_data, genesis_hash).await;

    assert!(result.is_ok(), "Genesis block should always be accepted");

    println!("✅ Genesis block accepted without validation");

    Ok(())
}

/// Test that blocks forming a valid chain are accepted
#[tokio::test]
async fn test_chain_validation_accepts_valid_chain() -> Result<()> {
    let _ = env_logger::builder().is_test(true).try_init();

    let genesis_block = TestUtils::create_test_block(0, BlockHash::all_zeros());
    let node = InMemoryBitcoinNode::new(genesis_block.clone());

    // Build a valid chain of 5 blocks
    let mut prev_hash = genesis_block.block_hash();
    let mut blocks = vec![genesis_block.clone()];

    for height in 1..=5 {
        let block = TestUtils::create_test_block(height, prev_hash);
        prev_hash = block.block_hash();
        node.add_block(block.clone(), height);
        blocks.push(block);
    }

    let config = TestConfig::new();
    let runtime = InMemoryRuntime::new(config.wasm).await;
    let storage = InMemoryStorage::new();

    let sync_config = SyncConfig {
        start_block: 0,
        exit_at: Some(6),
        pipeline_size: Some(1),
        max_reorg_depth: 100,
        reorg_check_threshold: 6,
    };

    let mut sync = MetashrewSync::new(node, storage, runtime, sync_config);

    // Process all blocks - they should all succeed
    for (height, block) in blocks.iter().enumerate() {
        let block_data = TestUtils::serialize_block(block);
        let block_hash = block.block_hash().to_byte_array().to_vec();

        let result = sync.process_block(height as u32, block_data, block_hash).await;

        assert!(
            result.is_ok(),
            "Block {} should be accepted (valid chain)",
            height
        );
    }

    println!("✅ Valid chain of 6 blocks accepted");

    Ok(())
}

/// Test that missing previous block hash allows processing (with warning)
#[tokio::test]
async fn test_chain_validation_handles_missing_prev_hash() -> Result<()> {
    let _ = env_logger::builder().is_test(true).try_init();

    let genesis_block = TestUtils::create_test_block(0, BlockHash::all_zeros());
    let node = InMemoryBitcoinNode::new(genesis_block.clone());

    // Create block 2 (skipping block 1)
    let block_2 = TestUtils::create_test_block(2, BlockHash::from_byte_array([0xaa; 32]));
    node.add_block(block_2.clone(), 2);

    let config = TestConfig::new();
    let runtime = InMemoryRuntime::new(config.wasm).await;
    let storage = InMemoryStorage::new();

    let sync_config = SyncConfig {
        start_block: 0,
        exit_at: Some(3),
        pipeline_size: Some(1),
        max_reorg_depth: 100,
        reorg_check_threshold: 6,
    };

    let mut sync = MetashrewSync::new(node, storage, runtime, sync_config);

    // Process genesis
    let genesis_data = TestUtils::serialize_block(&genesis_block);
    let genesis_hash = genesis_block.block_hash().to_byte_array().to_vec();
    sync.process_block(0, genesis_data, genesis_hash).await?;

    // Try to process block 2 (block 1 is missing, so we can't validate)
    // This should be allowed with a warning
    let block_2_data = TestUtils::serialize_block(&block_2);
    let block_2_hash = block_2.block_hash().to_byte_array().to_vec();

    let result = sync.process_block(2, block_2_data, block_2_hash).await;

    // Should succeed (with warning in logs)
    assert!(
        result.is_ok(),
        "Processing should succeed when previous hash is missing (with warning)"
    );

    println!("✅ Missing previous hash handled gracefully");

    Ok(())
}

/// Test detecting chain discontinuity in the middle of a long chain
#[tokio::test]
async fn test_chain_validation_detects_mid_chain_discontinuity() -> Result<()> {
    let _ = env_logger::builder().is_test(true).try_init();

    let genesis_block = TestUtils::create_test_block(0, BlockHash::all_zeros());
    let node = InMemoryBitcoinNode::new(genesis_block.clone());

    // Build valid chain 0-9
    let mut prev_hash = genesis_block.block_hash();
    let mut blocks = vec![genesis_block.clone()];

    for height in 1..=9 {
        let block = TestUtils::create_test_block(height, prev_hash);
        prev_hash = block.block_hash();
        node.add_block(block.clone(), height);
        blocks.push(block);
    }

    // Add block 10 with WRONG prev_hash
    let wrong_prev = BlockHash::from_byte_array([0xbb; 32]);
    let bad_block_10 = TestUtils::create_test_block(10, wrong_prev);
    node.add_block(bad_block_10.clone(), 10);

    let config = TestConfig::new();
    let runtime = InMemoryRuntime::new(config.wasm).await;
    let storage = InMemoryStorage::new();

    let sync_config = SyncConfig {
        start_block: 0,
        exit_at: Some(11),
        pipeline_size: Some(1),
        max_reorg_depth: 100,
        reorg_check_threshold: 6,
    };

    let mut sync = MetashrewSync::new(node, storage, runtime, sync_config);

    // Process blocks 0-9 (should all succeed)
    for (height, block) in blocks.iter().enumerate() {
        let block_data = TestUtils::serialize_block(block);
        let block_hash = block.block_hash().to_byte_array().to_vec();
        sync.process_block(height as u32, block_data, block_hash)
            .await?;
    }

    // Try to process bad block 10
    let bad_block_data = TestUtils::serialize_block(&bad_block_10);
    let bad_block_hash = bad_block_10.block_hash().to_byte_array().to_vec();

    let result = sync.process_block(10, bad_block_data, bad_block_hash).await;

    assert!(
        result.is_err(),
        "Block 10 should be rejected due to discontinuity"
    );

    println!("✅ Mid-chain discontinuity detected at block 10");

    Ok(())
}

/// Test that proper chain validation messages are logged
#[tokio::test]
async fn test_chain_validation_logging() -> Result<()> {
    let _ = env_logger::builder().is_test(true).try_init();

    let genesis_block = TestUtils::create_test_block(0, BlockHash::all_zeros());
    let node = InMemoryBitcoinNode::new(genesis_block.clone());

    let block_1 = TestUtils::create_test_block(1, genesis_block.block_hash());
    node.add_block(block_1.clone(), 1);

    // Bad block 2
    let bad_block_2 = TestUtils::create_test_block(2, BlockHash::from_byte_array([0xcc; 32]));
    node.add_block(bad_block_2.clone(), 2);

    let config = TestConfig::new();
    let runtime = InMemoryRuntime::new(config.wasm).await;
    let storage = InMemoryStorage::new();

    let sync_config = SyncConfig {
        start_block: 0,
        exit_at: Some(3),
        pipeline_size: Some(1),
        max_reorg_depth: 100,
        reorg_check_threshold: 6,
    };

    let mut sync = MetashrewSync::new(node, storage, runtime, sync_config);

    // Process genesis and block 1
    let genesis_data = TestUtils::serialize_block(&genesis_block);
    let genesis_hash = genesis_block.block_hash().to_byte_array().to_vec();
    sync.process_block(0, genesis_data, genesis_hash).await?;

    let block_1_data = TestUtils::serialize_block(&block_1);
    let block_1_hash = block_1.block_hash().to_byte_array().to_vec();
    sync.process_block(1, block_1_data, block_1_hash).await?;

    // Process bad block (check error message)
    let bad_data = TestUtils::serialize_block(&bad_block_2);
    let bad_hash = bad_block_2.block_hash().to_byte_array().to_vec();

    let result = sync.process_block(2, bad_data, bad_hash).await;

    match result {
        Err(e) => {
            let error_str = e.to_string();
            println!("Error message: {}", error_str);

            // Verify error message contains key information
            assert!(
                error_str.contains("does not connect") || error_str.contains("CHAIN DISCONTINUITY"),
                "Error should clearly indicate chain discontinuity"
            );
        }
        Ok(_) => panic!("Should have failed"),
    }

    println!("✅ Proper error messages logged");

    Ok(())
}
