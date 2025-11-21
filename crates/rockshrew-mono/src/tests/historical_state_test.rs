//! Test for historical state queries with the same key updated across multiple blocks.
//!
//! This test verifies that `metashrew_view` can correctly retrieve historical values
//! of a key that has been updated multiple times across different blocks.
//!
//! Specifically, this test:
//! 1. Indexes multiple blocks that each update the "/blocktracker" key
//! 2. Queries the key at different historical heights
//! 3. Verifies that each query returns the correct historical value for that height

use crate::{
    in_memory_adapters::InMemoryBitcoinNode,
    test_utils::{TestConfig, TestUtils, WASM},
};
use anyhow::Result;
use bitcoin::hashes::Hash;
use bitcoin::BlockHash;
use memshrew_runtime::MemStoreAdapter;
use metashrew_sync::{
    BitcoinNodeAdapter, MetashrewRuntimeAdapter, RuntimeAdapter, SyncConfig, SyncEngine,
    ViewCall, MetashrewSync,
};

#[tokio::test]
async fn test_historical_state_queries() -> Result<()> {
    println!("=== Testing Historical State Queries ===");

    // Setup
    let storage = MemStoreAdapter::new();
    let mut config_engine = wasmtime::Config::default();
    config_engine.async_support(true);
    let engine = wasmtime::Engine::new(&config_engine)?;
    
    let runtime = TestConfig::new()
        .create_runtime_from_adapter(storage.clone(), engine)
        .await?;

    let genesis_block_hash = BlockHash::from_slice(&[0; 32])?;
    let genesis_block = TestUtils::create_test_block(0, genesis_block_hash);

    let mut agent = MetashrewSync::new(
        InMemoryBitcoinNode::new(genesis_block.clone()),
        storage,
        MetashrewRuntimeAdapter::new(runtime),
        SyncConfig::default(),
    );

    // Index genesis block (height 0)
    println!("Processing genesis block at height 0");
    agent.process_single_block(0).await?;

    // Index blocks 1-5, each updating the "/blocktracker" key
    for height in 1..=5 {
        let prev_hash = match agent.node().get_block_hash(height - 1).await {
            Ok(hash) => hash,
            Err(e) => return Err(e.into()),
        };
        let block = TestUtils::create_test_block(height, BlockHash::from_slice(&prev_hash)?);
        agent.node().add_block(block.clone(), height);
        
        println!("Processing block at height {}", height);
        agent.process_single_block(height).await?;
    }

    println!("\n=== Querying Historical States ===");

    // Query the blocktracker at different heights
    // The metashrew-minimal WASM appends the first byte of each block hash to /blocktracker
    for query_height in 0..=5 {
        let view_call = ViewCall {
            function_name: "blocktracker".to_string(),
            input_data: vec![], // blocktracker takes no input
            height: query_height,
        };

        let result = agent.runtime().execute_view(view_call).await?;
        
        // At height 0, blocktracker should have 1 byte (from genesis)
        // At height N, blocktracker should have N+1 bytes
        let expected_len = query_height as usize + 1;
        
        println!(
            "Height {}: blocktracker has {} bytes (expected {})",
            query_height,
            result.data.len(),
            expected_len
        );

        if result.data.len() != expected_len {
            println!("FAIL: Expected {} bytes at height {}, got {}", 
                expected_len, query_height, result.data.len());
            println!("Data: {:?}", hex::encode(&result.data));
            
            // This is the bug we're testing for!
            // If historical queries don't work, we'll get the latest state at all heights
            return Err(anyhow::anyhow!(
                "Historical query returned wrong data length at height {}: expected {}, got {}",
                query_height,
                expected_len,
                result.data.len()
            ));
        }
    }

    println!("\n=== Test Passed ===");
    println!("Historical state queries are working correctly!");
    
    Ok(())
}

#[tokio::test]
async fn test_historical_query_after_multiple_updates() -> Result<()> {
    println!("=== Testing Historical Query After Multiple Updates to Same Key ===");

    // Setup
    let storage = MemStoreAdapter::new();
    let mut config_engine = wasmtime::Config::default();
    config_engine.async_support(true);
    let engine = wasmtime::Engine::new(&config_engine)?;
    
    let runtime = TestConfig::new()
        .create_runtime_from_adapter(storage.clone(), engine)
        .await?;

    let genesis_block_hash = BlockHash::from_slice(&[0; 32])?;
    let genesis_block = TestUtils::create_test_block(0, genesis_block_hash);

    let mut agent = MetashrewSync::new(
        InMemoryBitcoinNode::new(genesis_block.clone()),
        storage,
        MetashrewRuntimeAdapter::new(runtime),
        SyncConfig::default(),
    );

    // Process 10 blocks
    agent.process_single_block(0).await?;
    
    for height in 1..=10 {
        let prev_hash = agent.node().get_block_hash(height - 1).await?;
        let block = TestUtils::create_test_block(height, BlockHash::from_slice(&prev_hash)?);
        agent.node().add_block(block.clone(), height);
        agent.process_single_block(height).await?;
    }

    println!("Indexed 11 blocks (0-10)");

    // Now query at height 5 (middle of the chain)
    let view_call_mid = ViewCall {
        function_name: "blocktracker".to_string(),
        input_data: vec![],
        height: 5,
    };
    let result_mid = agent.runtime().execute_view(view_call_mid).await?;

    // Query at height 10 (latest)
    let view_call_latest = ViewCall {
        function_name: "blocktracker".to_string(),
        input_data: vec![],
        height: 10,
    };
    let result_latest = agent.runtime().execute_view(view_call_latest).await?;

    println!("Height 5: {} bytes", result_mid.data.len());
    println!("Height 10: {} bytes", result_latest.data.len());

    // At height 5, should have 6 bytes (blocks 0-5)
    // At height 10, should have 11 bytes (blocks 0-10)
    assert_eq!(
        result_mid.data.len(),
        6,
        "Height 5 should have 6 bytes, got {}",
        result_mid.data.len()
    );
    assert_eq!(
        result_latest.data.len(),
        11,
        "Height 10 should have 11 bytes, got {}",
        result_latest.data.len()
    );

    // The data should be different
    assert_ne!(
        result_mid.data, result_latest.data,
        "Historical query should return different data than latest"
    );

    // The first 6 bytes should match
    assert_eq!(
        &result_latest.data[0..6],
        &result_mid.data[..],
        "First 6 bytes of latest should match height 5 data"
    );

    println!("=== Test Passed ===");
    Ok(())
}
