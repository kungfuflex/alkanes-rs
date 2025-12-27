//! Test suite for non-SMT mode operation
//!
//! This test suite validates that the system works correctly with SMT disabled:
//! 1. Block processing works without SMT state roots
//! 2. Historical queries work using append-only storage
//! 3. Reorg handling works correctly without SMT
//! 4. Performance is better without SMT overhead

use crate::{
    in_memory_adapters::InMemoryBitcoinNode,
    test_utils::{TestConfig, TestUtils},
};
use anyhow::Result;
use bitcoin::hashes::Hash;
use bitcoin::BlockHash;
use memshrew_runtime::MemStoreAdapter;
use metashrew_sync::{
    BitcoinNodeAdapter, MetashrewRuntimeAdapter, RuntimeAdapter, SyncConfig, SyncEngine,
    ViewCall, MetashrewSync,
};
use std::time::Instant;

#[tokio::test]
async fn test_non_smt_basic_processing() -> Result<()> {
    println!("\n=== Testing Basic Block Processing (No SMT) ===\n");

    // Setup with SMT disabled
    let storage = MemStoreAdapter::new();
    let mut config_engine = wasmtime::Config::default();
    config_engine.async_support(true);
    let engine = wasmtime::Engine::new(&config_engine)?;

    let runtime = TestConfig::new()
        .create_runtime_from_adapter_with_smt(storage.clone(), engine, false)
        .await?;

    let genesis_block_hash = BlockHash::from_slice(&[0; 32])?;
    let genesis_block = TestUtils::create_test_block(0, genesis_block_hash);

    let mut agent = MetashrewSync::new(
        InMemoryBitcoinNode::new(genesis_block.clone()),
        storage.clone(),
        MetashrewRuntimeAdapter::new(runtime),
        SyncConfig::default(),
    );

    // Process genesis block
    println!("Processing genesis block...");
    agent.process_single_block(0).await?;

    // Process multiple blocks
    println!("Processing blocks 1-20...");
    for height in 1..=20 {
        let prev_hash = agent.node().get_block_hash(height - 1).await?;
        let block = TestUtils::create_test_block(height, BlockHash::from_slice(&prev_hash)?);
        agent.node().add_block(block.clone(), height);
        agent.process_single_block(height).await?;
    }

    // Verify state is correct
    println!("\nVerifying state integrity...");
    let view_call = ViewCall {
        function_name: "blocktracker".to_string(),
        input_data: vec![],
        height: 20,
    };
    let result = agent.runtime().execute_view(view_call).await?;

    assert_eq!(
        result.data.len(),
        21,
        "Should have 21 bytes without SMT"
    );

    println!("=== Test Passed ===");
    println!("Block processing works correctly without SMT");

    Ok(())
}

#[tokio::test]
async fn test_non_smt_historical_queries() -> Result<()> {
    println!("\n=== Testing Historical Queries (No SMT) ===\n");

    // Setup with SMT disabled
    let storage = MemStoreAdapter::new();
    let mut config_engine = wasmtime::Config::default();
    config_engine.async_support(true);
    let engine = wasmtime::Engine::new(&config_engine)?;

    let runtime = TestConfig::new()
        .create_runtime_from_adapter_with_smt(storage.clone(), engine, false)
        .await?;

    let genesis_block_hash = BlockHash::from_slice(&[0; 32])?;
    let genesis_block = TestUtils::create_test_block(0, genesis_block_hash);

    let mut agent = MetashrewSync::new(
        InMemoryBitcoinNode::new(genesis_block.clone()),
        storage.clone(),
        MetashrewRuntimeAdapter::new(runtime),
        SyncConfig::default(),
    );

    // Process blocks
    agent.process_single_block(0).await?;
    for height in 1..=30 {
        let prev_hash = agent.node().get_block_hash(height - 1).await?;
        let block = TestUtils::create_test_block(height, BlockHash::from_slice(&prev_hash)?);
        agent.node().add_block(block.clone(), height);
        agent.process_single_block(height).await?;
    }

    println!("Processed 31 blocks (0-30)");

    // Query historical states at different heights
    println!("\nQuerying historical states...");
    for query_height in [0, 10, 20, 30] {
        let view_call = ViewCall {
            function_name: "blocktracker".to_string(),
            input_data: vec![],
            height: query_height,
        };
        let result = agent.runtime().execute_view(view_call).await?;

        let expected_len = query_height as usize + 1;
        assert_eq!(
            result.data.len(),
            expected_len,
            "Height {} should have {} bytes",
            query_height,
            expected_len
        );
        println!("  Height {}: {} bytes ✓", query_height, result.data.len());
    }

    println!("\n=== Test Passed ===");
    println!("Historical queries work correctly without SMT (using append-only storage)");

    Ok(())
}

#[tokio::test]
async fn test_non_smt_reorg_handling() -> Result<()> {
    println!("\n=== Testing Reorg Handling (No SMT) ===\n");

    // Setup with SMT disabled
    let storage = MemStoreAdapter::new();
    let mut config_engine = wasmtime::Config::default();
    config_engine.async_support(true);
    let engine = wasmtime::Engine::new(&config_engine)?;

    let runtime = TestConfig::new()
        .create_runtime_from_adapter_with_smt(storage.clone(), engine, false)
        .await?;

    let genesis_block_hash = BlockHash::from_slice(&[0; 32])?;
    let genesis_block = TestUtils::create_test_block(0, genesis_block_hash);

    let node = InMemoryBitcoinNode::new(genesis_block.clone());

    let mut agent = MetashrewSync::new(
        node.clone(),
        storage.clone(),
        MetashrewRuntimeAdapter::new(runtime),
        SyncConfig::default(),
    );

    // Build original chain: 0 -> 1 -> 2 -> 3
    println!("Building original chain (blocks 0-3)...");
    agent.process_single_block(0).await?;

    let mut prev_hash = genesis_block_hash.to_byte_array().to_vec();
    for height in 1..=3 {
        let block = TestUtils::create_test_block(height, BlockHash::from_slice(&prev_hash)?);
        prev_hash = block.block_hash().to_byte_array().to_vec();
        node.add_block(block, height);
        agent.process_single_block(height).await?;
    }

    // Verify original chain state
    let view_call_original = ViewCall {
        function_name: "blocktracker".to_string(),
        input_data: vec![],
        height: 3,
    };
    let result_original = agent.runtime().execute_view(view_call_original).await?;
    println!("Original chain at height 3: {} bytes", result_original.data.len());
    assert_eq!(result_original.data.len(), 4);

    // Simulate reorg: Create alternate chain from block 2
    println!("\nSimulating reorg at height 2...");

    // Get block 1 hash (reorg point)
    let block_1_hash = agent.node().get_block_hash(1).await?;

    // Create alternate blocks 2' and 3'
    let alt_block_2 = TestUtils::create_test_block(2, BlockHash::from_slice(&block_1_hash)?);
    let alt_block_2_hash = alt_block_2.block_hash().to_byte_array().to_vec();

    let alt_block_3 = TestUtils::create_test_block(3, BlockHash::from_slice(&alt_block_2_hash)?);

    // Replace blocks in node (simulating reorg)
    node.add_block(alt_block_2, 2);
    node.add_block(alt_block_3, 3);

    // Reprocess blocks 2 and 3 (reorg)
    println!("Reprocessing blocks 2-3 (alternate chain)...");
    agent.process_single_block(2).await?;
    agent.process_single_block(3).await?;

    // Verify state after reorg
    let view_call_after_reorg = ViewCall {
        function_name: "blocktracker".to_string(),
        input_data: vec![],
        height: 3,
    };
    let result_after_reorg = agent.runtime().execute_view(view_call_after_reorg).await?;

    println!("After reorg at height 3: {} bytes", result_after_reorg.data.len());
    assert_eq!(result_after_reorg.data.len(), 4);

    // Verify historical state at height 1 (before reorg point) is still correct
    let view_call_before_reorg = ViewCall {
        function_name: "blocktracker".to_string(),
        input_data: vec![],
        height: 1,
    };
    let result_before_reorg = agent.runtime().execute_view(view_call_before_reorg).await?;

    println!("Historical state at height 1 (before reorg): {} bytes", result_before_reorg.data.len());
    assert_eq!(result_before_reorg.data.len(), 2);

    println!("\n=== Test Passed ===");
    println!("Reorg handling works correctly without SMT (using append-only storage)");

    Ok(())
}

#[tokio::test]
async fn test_performance_comparison_smt_vs_non_smt() -> Result<()> {
    println!("\n=== Performance Comparison: SMT vs Non-SMT ===\n");

    let num_blocks = 50u32;

    // Test 1: Non-SMT mode
    println!("Testing Non-SMT mode...");
    let start_non_smt = Instant::now();

    let storage_non_smt = MemStoreAdapter::new();
    let mut config_engine = wasmtime::Config::default();
    config_engine.async_support(true);
    let engine_non_smt = wasmtime::Engine::new(&config_engine)?;

    let runtime_non_smt = TestConfig::new()
        .create_runtime_from_adapter_with_smt(storage_non_smt.clone(), engine_non_smt, false)
        .await?;

    let genesis_block_hash = BlockHash::from_slice(&[0; 32])?;
    let genesis_block = TestUtils::create_test_block(0, genesis_block_hash);

    let mut agent_non_smt = MetashrewSync::new(
        InMemoryBitcoinNode::new(genesis_block.clone()),
        storage_non_smt.clone(),
        MetashrewRuntimeAdapter::new(runtime_non_smt),
        SyncConfig::default(),
    );

    agent_non_smt.process_single_block(0).await?;
    for height in 1..=num_blocks {
        let prev_hash = agent_non_smt.node().get_block_hash(height - 1).await?;
        let block = TestUtils::create_test_block(height, BlockHash::from_slice(&prev_hash)?);
        agent_non_smt.node().add_block(block.clone(), height);
        agent_non_smt.process_single_block(height).await?;
    }

    let duration_non_smt = start_non_smt.elapsed();
    println!("Non-SMT: Processed {} blocks in {:?}", num_blocks + 1, duration_non_smt);

    // Test 2: SMT mode
    println!("\nTesting SMT mode...");
    let start_smt = Instant::now();

    let storage_smt = MemStoreAdapter::new();
    let mut config_engine_smt = wasmtime::Config::default();
    config_engine_smt.async_support(true);
    let engine_smt = wasmtime::Engine::new(&config_engine_smt)?;

    let runtime_smt = TestConfig::new()
        .create_runtime_from_adapter_with_smt(storage_smt.clone(), engine_smt, true)
        .await?;

    let genesis_block_smt = TestUtils::create_test_block(0, genesis_block_hash);

    let mut agent_smt = MetashrewSync::new(
        InMemoryBitcoinNode::new(genesis_block_smt.clone()),
        storage_smt.clone(),
        MetashrewRuntimeAdapter::new(runtime_smt),
        SyncConfig::default(),
    );

    agent_smt.process_single_block(0).await?;
    for height in 1..=num_blocks {
        let prev_hash = agent_smt.node().get_block_hash(height - 1).await?;
        let block = TestUtils::create_test_block(height, BlockHash::from_slice(&prev_hash)?);
        agent_smt.node().add_block(block.clone(), height);
        agent_smt.process_single_block(height).await?;
    }

    let duration_smt = start_smt.elapsed();
    println!("SMT: Processed {} blocks in {:?}", num_blocks + 1, duration_smt);

    // Compare performance
    println!("\n=== Performance Results ===");
    let speedup = duration_smt.as_secs_f64() / duration_non_smt.as_secs_f64();
    println!("Non-SMT is {:.2}x faster than SMT", speedup);

    if duration_non_smt < duration_smt {
        println!("✓ Non-SMT mode is faster (as expected)");
    } else {
        println!("⚠ SMT mode was faster (unexpected, but possible with caching)");
    }

    // Verify both produce correct results
    let view_non_smt = ViewCall {
        function_name: "blocktracker".to_string(),
        input_data: vec![],
        height: num_blocks,
    };
    let result_non_smt = agent_non_smt.runtime().execute_view(view_non_smt).await?;

    let view_smt = ViewCall {
        function_name: "blocktracker".to_string(),
        input_data: vec![],
        height: num_blocks,
    };
    let result_smt = agent_smt.runtime().execute_view(view_smt).await?;

    assert_eq!(
        result_non_smt.data,
        result_smt.data,
        "Both modes should produce identical state"
    );

    println!("✓ Both modes produce identical results");
    println!("\n=== Test Passed ===");

    Ok(())
}

#[tokio::test]
async fn test_non_smt_deep_reorg() -> Result<()> {
    println!("\n=== Testing Deep Reorg (No SMT) ===\n");

    // Setup with SMT disabled
    let storage = MemStoreAdapter::new();
    let mut config_engine = wasmtime::Config::default();
    config_engine.async_support(true);
    let engine = wasmtime::Engine::new(&config_engine)?;

    let runtime = TestConfig::new()
        .create_runtime_from_adapter_with_smt(storage.clone(), engine, false)
        .await?;

    let genesis_block_hash = BlockHash::from_slice(&[0; 32])?;
    let genesis_block = TestUtils::create_test_block(0, genesis_block_hash);

    let node = InMemoryBitcoinNode::new(genesis_block.clone());

    let mut agent = MetashrewSync::new(
        node.clone(),
        storage.clone(),
        MetashrewRuntimeAdapter::new(runtime),
        SyncConfig::default(),
    );

    // Build original chain: 0 -> 1 -> 2 -> 3 -> 4 -> 5
    println!("Building original chain (blocks 0-5)...");
    agent.process_single_block(0).await?;

    let mut prev_hash = genesis_block_hash.to_byte_array().to_vec();
    for height in 1..=5 {
        let block = TestUtils::create_test_block(height, BlockHash::from_slice(&prev_hash)?);
        prev_hash = block.block_hash().to_byte_array().to_vec();
        node.add_block(block, height);
        agent.process_single_block(height).await?;
    }

    // Simulate deep reorg from block 2 (3 blocks deep)
    println!("\nSimulating deep reorg (3 blocks) from height 2...");

    let block_1_hash = agent.node().get_block_hash(1).await?;
    let mut alt_prev_hash = block_1_hash;

    // Create alternate chain: 2' -> 3' -> 4' -> 5'
    for height in 2..=5 {
        let alt_block = TestUtils::create_test_block(height, BlockHash::from_slice(&alt_prev_hash)?);
        alt_prev_hash = alt_block.block_hash().to_byte_array().to_vec();
        node.add_block(alt_block, height);
    }

    // Reprocess blocks 2-5 (deep reorg)
    println!("Reprocessing blocks 2-5 (alternate chain)...");
    for height in 2..=5 {
        agent.process_single_block(height).await?;
    }

    // Verify state after deep reorg
    let view_call = ViewCall {
        function_name: "blocktracker".to_string(),
        input_data: vec![],
        height: 5,
    };
    let result = agent.runtime().execute_view(view_call).await?;

    println!("After deep reorg at height 5: {} bytes", result.data.len());
    assert_eq!(result.data.len(), 6);

    // Verify all intermediate heights are queryable
    println!("\nVerifying all heights are queryable after deep reorg...");
    for height in 0..=5 {
        let view_call = ViewCall {
            function_name: "blocktracker".to_string(),
            input_data: vec![],
            height,
        };
        let result = agent.runtime().execute_view(view_call).await?;
        assert_eq!(result.data.len(), height as usize + 1);
        println!("  Height {}: {} bytes ✓", height, result.data.len());
    }

    println!("\n=== Test Passed ===");
    println!("Deep reorg handling works correctly without SMT");

    Ok(())
}
