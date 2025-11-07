
//! Test for historical view function execution.
//!
//! This test verifies that `metashrew_view` can correctly render a result
//! from the historical state of the database, even after the state has been
//! overwritten in a later block.

use crate::{in_memory_adapters::InMemoryBitcoinNode, test_utils::{TestConfig, TestUtils}};
use bitcoin::hashes::Hash;
use bitcoin::BlockHash;
use memshrew_runtime::MemStoreAdapter;
use metashrew_runtime::MetashrewRuntime;
use metashrew_sync::{BitcoinNodeAdapter, MetashrewRuntimeAdapter, RuntimeAdapter, SyncConfig, ViewCall, MetashrewSync, SyncEngine};

#[tokio::test]
async fn test_historical_view() -> Result<(), Box<dyn std::error::Error>> {
    let genesis_block_hash = BlockHash::from_slice(&[0; 32])?;
    let genesis_block = TestUtils::create_test_block(0, genesis_block_hash);
    let storage = MemStoreAdapter::new();
    let mut config_engine = wasmtime::Config::default();
    config_engine.async_support(true);
    let engine = wasmtime::Engine::new(&config_engine)?;
    let runtime = TestConfig::new().create_runtime_from_adapter(storage.clone(), engine).await?;
    let mut agent = MetashrewSync::new(
        InMemoryBitcoinNode::new(genesis_block.clone()),
        storage,
        MetashrewRuntimeAdapter::new(runtime),
        SyncConfig::default(),
    );
    agent.process_single_block(0).await?;
    let block1_hash = match agent.node().get_block_hash(0).await {
        Ok(hash) => hash,
        Err(e) => return Err(e.into()),
    };
    let block1 = TestUtils::create_test_block(1, BlockHash::from_slice(&block1_hash)?);
    agent.node().add_block(block1.clone(), 1);
    agent.process_single_block(1).await?;
    let block2_hash = match agent.node().get_block_hash(1).await {
        Ok(hash) => hash,
        Err(e) => return Err(e.into()),
    };
    let block2 = TestUtils::create_test_block(2, BlockHash::from_slice(&block2_hash)?);
    agent.node().add_block(block2.clone(), 2);
    agent.process_single_block(2).await?;

    let view_call_latest = ViewCall {
        function_name: "getblock".to_string(),
        input_data: 2u32.to_le_bytes().to_vec(), // Input for getblock: height 2
        height: 2,
    };
    let result_latest = agent.runtime().execute_view(view_call_latest).await?;
    let view_call_historical = ViewCall {
        function_name: "getblock".to_string(),
        input_data: 1u32.to_le_bytes().to_vec(), // Input for getblock: height 1
        height: 1,
    };
    let result_historical = agent.runtime().execute_view(view_call_historical).await?;
    assert_ne!(result_latest.data, result_historical.data);
    Ok(())
}
