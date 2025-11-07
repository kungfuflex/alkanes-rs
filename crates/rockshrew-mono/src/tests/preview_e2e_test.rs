//! End-to-end test for the `metashrew_preview` functionality.
//!
//! This test verifies that the `execute_preview` function can correctly
//! execute a view function against a provided block without modifying
//! the underlying database state.

use bitcoin::hashes::Hash;
use crate::in_memory_adapters::{InMemoryBitcoinNode, InMemoryRuntime};
use crate::test_utils::WASM;
use metashrew_sync::{PreviewCall, RuntimeAdapter};

#[tokio::test]
async fn test_preview_e2e() {
    // 1. Setup
    let wasm_bytes = WASM;
    let mut runtime = InMemoryRuntime::new(&wasm_bytes).await;
    let mut adapter = runtime.new_runtime_adapter();
    let genesis_block = crate::test_utils::TestUtils::create_test_block(0, bitcoin::BlockHash::all_zeros());
    let block1 = crate::test_utils::TestUtils::create_test_block(1, genesis_block.block_hash());
    let block2 = crate::test_utils::TestUtils::create_test_block(2, block1.block_hash());
    let block3 = crate::test_utils::TestUtils::create_test_block(3, block2.block_hash());

    // 2. Index blocks 0, 1, and 2
    adapter.process_block(0, &metashrew_support::utils::consensus_encode(&genesis_block).unwrap()).await.unwrap();
    adapter.process_block(1, &metashrew_support::utils::consensus_encode(&block1).unwrap()).await.unwrap();
    adapter.process_block(2, &metashrew_support::utils::consensus_encode(&block2).unwrap()).await.unwrap();

    // 3. Get the initial blocktracker state
    let initial_blocktracker_result = adapter.execute_view(metashrew_sync::ViewCall {
        function_name: "blocktracker".to_string(),
        input_data: vec![],
        height: 2,
    }).await.unwrap();
    let initial_blocktracker_state = initial_blocktracker_result.data;
    assert_eq!(initial_blocktracker_state.len(), 3, "Initial blocktracker state should have 3 bytes");

    // 4. Define the preview call for block 3
    let call = PreviewCall {
        block_data: metashrew_support::utils::consensus_encode(&block3).unwrap(),
        function_name: "blocktracker".to_string(),
        input_data: vec![],
        height: 3,
    };

    // 5. Execute the preview
    let preview_result = adapter.execute_preview(call).await.unwrap();
    let preview_blocktracker_state = preview_result.data;
    assert_eq!(preview_blocktracker_state.len(), 4, "Preview blocktracker state should have 4 bytes");

    // 6. Verify that the database state was not modified
    let final_blocktracker_result = adapter.execute_view(metashrew_sync::ViewCall {
        function_name: "blocktracker".to_string(),
        input_data: vec![],
        height: 2,
    }).await.unwrap();
    let final_blocktracker_state = final_blocktracker_result.data;
    assert_eq!(initial_blocktracker_state, final_blocktracker_state, "Database state should not be modified by a preview call");
}
