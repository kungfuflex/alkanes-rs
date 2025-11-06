//! Reorganization-focused end-to-end tests
//!
//! This test suite validates the chain reorganization handling of the Metashrew indexer.
//! It ensures that when a fork occurs, the indexer correctly rolls back the state
//! of the old chain and applies the state of the new, longer chain.

use crate::test_utils::{TestConfig, TestUtils};
use anyhow::Result;
use bitcoin::{hashes::Hash, BlockHash};

/// Test to validate correct state rollback during a chain reorganization.
#[tokio::test]
async fn test_reorg_handling() -> Result<()> {
    let config = TestConfig::new();
    let mut config_engine = wasmtime::Config::default();
    config_engine.async_support(true);
    let engine = wasmtime::Engine::new(&config_engine)?;
    let runtime = config.create_runtime(engine).await?;

    // 1. Process an initial chain of 5 blocks
    let mut chain_a_hashes = vec![BlockHash::all_zeros()];
    for height in 0..5 {
        let prev_hash = chain_a_hashes.last().unwrap().clone();
        let block = TestUtils::create_test_block(height, prev_hash);
        chain_a_hashes.push(block.block_hash());
        let block_bytes = TestUtils::serialize_block(&block);
        {
            let mut context = runtime.context.lock().await;
            context.block = block_bytes;
            context.height = height;
        }
        runtime.run().await?;
        runtime.refresh_memory().await?;
    }

    // Verify state of chain A at height 4
    let view_input = vec![];
    let blocktracker_data_a = runtime
        .view("blocktracker".to_string(), &view_input, 4)
        .await?;
    assert_eq!(blocktracker_data_a.len(), 5, "Chain A should have 5 blocks");

    // 2. Create a fork from height 2 (a new chain B)
    let mut chain_b_hashes = vec![chain_a_hashes[2]];
    for i in 0..4 {
        // Create a longer chain
        let height = 2 + i;
        let prev_hash = chain_b_hashes.last().unwrap().clone();
        // Create a different block by changing the nonce
        let mut block = TestUtils::create_test_block(height, prev_hash);
        block.header.nonce += 1; // Ensure block hash is different
        chain_b_hashes.push(block.block_hash());
        let block_bytes = TestUtils::serialize_block(&block);
        {
            let mut context = runtime.context.lock().await;
            context.block = block_bytes;
            context.height = height;
        }
        runtime.run().await?;
        runtime.refresh_memory().await?;
    }

    // 3. Verify the state now reflects chain B
    let blocktracker_data_b = runtime
        .view("blocktracker".to_string(), &view_input, 5) // Chain B is at height 5
        .await?;
    assert_eq!(
        blocktracker_data_b.len(),
        6,
        "Chain B should have 6 blocks"
    );

    // 4. Verify that the state from the original chain (A) after the fork point is gone.
    // Querying at height 4 should now show the state from chain B.
    let blocktracker_data_b_at_4 = runtime
        .view("blocktracker".to_string(), &view_input, 4)
        .await?;
    assert_ne!(
        blocktracker_data_a, blocktracker_data_b_at_4,
        "State at height 4 should have changed after reorg"
    );

    println!("✅ Reorganization handling test passed!");

    Ok(())
}
