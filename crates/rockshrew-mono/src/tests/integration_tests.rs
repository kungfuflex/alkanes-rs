//! Integration tests for the complete Metashrew indexing workflow
//!
//! These tests verify end-to-end functionality including block processing,
//! view function execution, and data consistency across multiple operations.

use crate::block_builder::ChainBuilder;
use crate::test_utils::TestConfig;
use anyhow::Result;
use memshrew_runtime::MemStoreAdapter;
use metashrew_runtime::smt::SMTHelper;
use metashrew_support::utils;
use std::collections::HashMap;

// Helper functions for append-only store access
fn get_blocktracker(adapter: &MemStoreAdapter, height: u32) -> Result<Vec<u8>> {
    let smt_helper = SMTHelper::new(adapter.clone());
    let key = b"/blocktracker".to_vec();
    Ok(smt_helper
        .get_at_height(&key, height)?
        .unwrap_or_default())
}

fn get_indexed_block(adapter: &MemStoreAdapter, height: u32) -> Result<Option<Vec<u8>>> {
    let smt_helper = SMTHelper::new(adapter.clone());
    let key = format!("/blocks/{}", height).into_bytes();
    Ok(smt_helper.get_at_height(&key, height)?)
}

/// Test complete indexing workflow - comprehensive E2E test
#[tokio::test]
async fn test_complete_indexing_workflow() -> Result<()> {
    let config = TestConfig::new();
    let mut config_engine = wasmtime::Config::default();
    config_engine.async_support(true);
    let engine = wasmtime::Engine::new(&config_engine)?;
    let runtime = config.create_runtime(engine).await?;

    // Create a realistic chain of blocks
    let chain = ChainBuilder::new().add_blocks(10).blocks();

    // Process all blocks in sequence
    for (height, block) in chain.iter().enumerate() {
        let block_bytes = utils::consensus_encode(block)?;

        {
            let mut context = runtime.context.lock().await;
            context.block = block_bytes;
            context.height = height as u32;
        }

        runtime.run().await?;
        runtime.refresh_memory().await?;
    }

    // Verify final state using direct database access
    let adapter = &runtime.context.lock().await.db;

    // Check that all blocks are stored using append-only access
    for height in 0..chain.len() {
        let stored_block = get_indexed_block(adapter, height as u32)?;
        assert!(stored_block.is_some(), "Block {} should be stored", height);
    }

    // Check blocktracker has correct length (should be number of blocks processed)
    let final_height = (chain.len() - 1) as u32;
    let blocktracker_result = get_blocktracker(adapter, final_height)?;
    assert_eq!(
        blocktracker_result.len(),
        chain.len(),
        "Blocktracker should track all blocks"
    );

    Ok(())
}

/// Test database state consistency during indexing
#[tokio::test]
async fn test_database_state_consistency() -> Result<()> {
    let config = TestConfig::new();
    let mut config_engine = wasmtime::Config::default();
    config_engine.async_support(true);
    let engine = wasmtime::Engine::new(&config_engine)?;
    let runtime = config.create_runtime(engine).await?;

    // Process some blocks
    let chain = ChainBuilder::new().add_blocks(3).blocks();

    let mut snapshots: Vec<HashMap<Vec<u8>, Vec<u8>>> = Vec::new();

    // Take snapshots after each block
    for (height, block) in chain.iter().enumerate() {
        let block_bytes = utils::consensus_encode(block)?;

        {
            let mut context = runtime.context.lock().await;
            context.block = block_bytes;
            context.height = height as u32;
        }

        runtime.run().await?;
        runtime.refresh_memory().await?;

        // Take a snapshot of the database state
        let adapter = &runtime.context.lock().await.db;
        snapshots.push(adapter.get_all_data());
    }

    // Verify that each snapshot contains the previous data plus new data
    for i in 1..snapshots.len() {
        let prev_snapshot = &snapshots[i - 1];
        let curr_snapshot = &snapshots[i];

        // Current snapshot should have more or equal keys
        assert!(
            curr_snapshot.len() >= prev_snapshot.len(),
            "Database should only grow, not shrink"
        );
    }

    Ok(())
}
