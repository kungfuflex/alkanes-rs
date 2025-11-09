//! Minimal test to isolate alkane deployment issue

use crate::index_block;
use crate::tests::helpers::{self as alkane_helpers, assert_binary_deployed_to_id};
use crate::tests::std::alkanes_std_test_build;
use alkane_helpers::clear;
use alkanes_support::cellpack::Cellpack;
use alkanes_support::id::AlkaneId;
use anyhow::Result;
use wasm_bindgen_test::wasm_bindgen_test;

#[wasm_bindgen_test]
fn test_minimal_deploy() -> Result<()> {
    clear();
    
    // Create a simple deployment cellpack
    let deploy_cellpack = Cellpack {
        target: AlkaneId { block: 1, tx: 0 },
        inputs: vec![0], // opcode 0 = deploy
    };

    // Initialize with the test alkane binary
    let test_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
        vec![alkanes_std_test_build::get_bytes()],
        vec![deploy_cellpack],
    );

    println!("Test block has {} transactions", test_block.txdata.len());
    println!("Test block vfsize: {}", {
        use bitcoin::consensus::Encodable;
        test_block.txdata.iter().map(|tx| {
            let mut buf = Vec::new();
            tx.consensus_encode(&mut buf).unwrap();
            buf.len() as u64
        }).sum::<u64>()
    });

    // Index the block at height 0
    index_block(&test_block, 0)?;

    // Verify the binary was deployed
    let expected_id = AlkaneId { block: 1, tx: 0 };
    let binary = alkanes_std_test_build::get_bytes();
    
    println!("Expected binary length: {}", binary.len());
    
    assert_binary_deployed_to_id(expected_id, binary)?;

    println!("✅ Deployment successful!");
    Ok(())
}
