use crate::index_block;
use crate::tests::helpers::{self as alkane_helpers};
use crate::tests::std::alkanes_std_test_build;
use crate::tests::test_runtime::TestRuntime;
use alkanes_support::cellpack::Cellpack;
use alkanes_support::id::AlkaneId;
use anyhow::Result;
use bitcoin::{OutPoint, Witness};
use metashrew_support::environment::RuntimeEnvironment;
use protorune_support::balance_sheet::BalanceSheetOperations;

// Helper function to create a malformed cellpack with extremely large inputs
fn create_malformed_cellpack_large_inputs() -> Cellpack {
    Cellpack {
        target: AlkaneId { block: 1, tx: 0 },
        inputs: vec![u128::MAX], // Extremely large inputs
    }
}

#[test]
fn test_integer_overflow_in_memory_operations() -> Result<()> {
    alkane_helpers::clear::<TestRuntime>();
    let block_height = 0;

    // Create a cellpack with extremely large inputs
    let overflow_cellpack = create_malformed_cellpack_large_inputs();

    // Initialize the contract and execute the cellpack
    let mut test_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
        [alkanes_std_test_build::get_bytes()].into(),
        [overflow_cellpack].into(),
    );

    // This should not crash the indexer, but should fail gracefully
    index_block::<TestRuntime>(&mut TestRuntime::default(), &test_block, block_height)?;

    // Check that the operation failed by examining the trace
    let outpoint = OutPoint {
        txid: test_block.txdata.last().unwrap().compute_txid(),
        vout: 3,
    };

    alkane_helpers::assert_revert_context(
        &outpoint,
        "Unrecognized opcode",
    )?;

    Ok(())
}