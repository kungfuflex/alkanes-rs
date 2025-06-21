use crate::index_block;
use crate::tests::helpers::{self as alkane_helpers};
use crate::tests::std::alkanes_std_test_build;
use crate::vm::utils::{get_memory, read_arraybuffer};
use alkanes_support::cellpack::Cellpack;
use alkanes_support::id::AlkaneId;
use alkanes_support::parcel::{AlkaneTransfer, AlkaneTransferParcel};
use alkanes_support::storage::StorageMap;
use alkanes_support::trace::{Trace, TraceEvent};
use anyhow::Result;
use bitcoin::{OutPoint, Witness};
use protorune_support::balance_sheet::BalanceSheetOperations;
use std::io::Cursor;
use wasm_bindgen_test::wasm_bindgen_test;

// Helper function to create a malformed cellpack with extremely large inputs
fn create_malformed_cellpack_large_inputs() -> Cellpack {
    Cellpack {
        target: AlkaneId { block: 1, tx: 0 },
        inputs: vec![u128::MAX], // Extremely large inputs
    }
}

#[wasm_bindgen_test]
fn test_integer_overflow_in_memory_operations() -> Result<()> {
    alkane_helpers::clear();
    let block_height = 840_000;

    // Create a cellpack with extremely large inputs
    let overflow_cellpack = create_malformed_cellpack_large_inputs();

    // Initialize the contract and execute the cellpack
    let mut test_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
        [alkanes_std_test_build::get_bytes()].into(),
        [overflow_cellpack].into(),
    );

    // This should not crash the indexer, but should fail gracefully
    index_block(&test_block, block_height)?;

    // Check that the operation failed by examining the trace
    let outpoint = OutPoint {
        txid: test_block.txdata.last().unwrap().compute_txid(),
        vout: 3,
    };

    alkane_helpers::assert_revert_context(&outpoint, "Unrecognized opcode")?;

    Ok(())
}

#[wasm_bindgen_test]
fn test_malformed_transfer_parcel() -> Result<()> {
    alkane_helpers::clear();
    let block_height = 840_000;

    // Create a normal cellpack first
    let init_cellpack = Cellpack {
        target: AlkaneId { block: 1, tx: 0 },
        inputs: vec![50], // Initialize the contract
    };

    // Initialize the contract
    let mut test_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
        [alkanes_std_test_build::get_bytes()].into(),
        [init_cellpack].into(),
    );

    let test_contract_id = AlkaneId { block: 2, tx: 1 };

    // Now create a cellpack that will attempt to use extcall with a malformed transfer parcel
    let large_transfer_cellpack = Cellpack {
        target: test_contract_id,
        inputs: vec![40],
    };

    // Add the malformed transfer parcel to the witness
    test_block
        .txdata
        .push(alkane_helpers::create_multiple_cellpack_with_witness(
            Witness::new(),
            vec![large_transfer_cellpack],
            false,
        ));

    // This should not crash the indexer, but should fail gracefully
    index_block(&test_block, block_height)?;

    let sheet = alkane_helpers::get_last_outpoint_sheet(&test_block)?;
    assert_eq!(sheet.get(&test_contract_id.into()), u128::MAX);

    Ok(())
}

#[wasm_bindgen_test]
fn test_malformed_transfer_parcel_extcall() -> Result<()> {
    alkane_helpers::clear();
    let block_height = 840_000;

    // Create a normal cellpack first
    let init_cellpack = Cellpack {
        target: AlkaneId { block: 1, tx: 0 },
        inputs: vec![50], // Initialize the contract
    };

    // Initialize the contract
    let mut test_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
        [alkanes_std_test_build::get_bytes()].into(),
        [init_cellpack].into(),
    );
    let test_contract_id = AlkaneId { block: 2, tx: 1 };

    // Now create a cellpack that will attempt to use extcall with a malformed transfer parcel
    let large_transfer_extcall_cellpack = Cellpack {
        target: test_contract_id,
        inputs: vec![41],
    };

    // Add the malformed transfer parcel to the witness
    test_block
        .txdata
        .push(alkane_helpers::create_multiple_cellpack_with_witness(
            Witness::new(),
            vec![large_transfer_extcall_cellpack],
            false,
        ));

    // This should not crash the indexer, but should fail gracefully
    index_block(&test_block, block_height)?;

    let sheet = alkane_helpers::get_last_outpoint_sheet(&test_block)?;
    assert_eq!(sheet.get(&test_contract_id.into()), u128::MAX);

    Ok(())
}

// #[wasm_bindgen_test]
// fn test_malformed_storage_map() -> Result<()> {
//     alkane_helpers::clear();
//     let block_height = 840_000;

//     // Create a normal cellpack first
//     let init_cellpack = Cellpack {
//         target: AlkaneId { block: 1, tx: 0 },
//         inputs: vec![50], // Initialize the contract
//     };

//     // Initialize the contract
//     let mut test_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
//         [alkanes_std_test_build::get_bytes()].into(),
//         [init_cellpack].into(),
//     );

//     // Now create a cellpack that will attempt to use extcall with a malformed storage map
//     let extcall_cellpack = Cellpack {
//         target: AlkaneId { block: 2, tx: 1 },
//         inputs: vec![31, 2, 0, 3], // Call the TestExtCall function with target = {2,0}
//     };

//     // Add the cellpack with a malformed storage map to the witness
//     test_block
//         .txdata
//         .push(alkane_helpers::create_cellpack_with_malformed_storage_map(
//             extcall_cellpack,
//         ));

//     // This should not crash the indexer, but should fail gracefully
//     index_block(&test_block, block_height)?;

//     // Check that the operation failed by examining the trace
//     let outpoint = OutPoint {
//         txid: test_block.txdata.last().unwrap().compute_txid(),
//         vout: 3,
//     };

//     let trace_data: Trace = alkanes::view::trace(&outpoint)?.try_into()?;
//     let trace_events = trace_data.0.lock().expect("Mutex poisoned");

//     // Find a revert event in the trace
//     let has_revert = trace_events
//         .iter()
//         .any(|event| matches!(event, TraceEvent::RevertContext(_)));

//     assert!(
//         has_revert,
//         "Expected a revert event due to malformed storage map"
//     );

//     Ok(())
// }
