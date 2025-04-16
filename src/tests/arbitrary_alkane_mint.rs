use crate::tests::std::alkanes_std_test_build;
use alkanes_support::cellpack::Cellpack;
use alkanes_support::id::AlkaneId;
use alkanes_support::trace::{Trace, TraceEvent};
use anyhow::Result;
use bitcoin::{OutPoint, Witness};

use crate::index_block;
use crate::tests::helpers::{self as alkane_helpers};
use alkane_helpers::clear;
use alkanes::view;
#[allow(unused_imports)]
use metashrew_core::{
    println,
    stdio::{stdout, Write},
};
use protorune_support::balance_sheet::ProtoruneRuneId;
use wasm_bindgen_test::wasm_bindgen_test;

#[wasm_bindgen_test]
fn test_arbitrary_mint() -> Result<()> {
    clear();
    let block_height = 840_000;

    // Create a cellpack to call the process_numbers method (opcode 11)
    let arb_mint_cellpack = Cellpack {
        target: AlkaneId { block: 1, tx: 0 },
        inputs: vec![30, 2, 0],
    };

    // Initialize the contract and execute the cellpacks
    let mut test_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
        [alkanes_std_test_build::get_bytes()].into(),
        [arb_mint_cellpack].into(),
    );

    index_block(&test_block, block_height)?;

    let sheet = alkane_helpers::get_last_outpoint_sheet(&test_block)?;

    println!("Last sheet: {:?}", sheet);

    assert_eq!(sheet.get_cached(&ProtoruneRuneId { block: 2, tx: 0 }), 0);
    assert_eq!(sheet.get_cached(&ProtoruneRuneId { block: 2, tx: 1 }), 0);

    let outpoint = OutPoint {
        txid: test_block.txdata.last().unwrap().compute_txid(),
        vout: 3,
    };

    let trace_data: Trace = view::trace(&outpoint)?.try_into()?;
    let trace_events = trace_data.0.lock().expect("Mutex poisoned");
    let last_trace_event = trace_events[trace_events.len() - 1].clone();
    match last_trace_event {
        TraceEvent::RevertContext(trace_response) => {
            // Now we have the TraceResponse, access the data field
            let data = String::from_utf8_lossy(&trace_response.inner.data);
            assert!(data.contains("overflow error"));
        }
        _ => panic!("Expected RevertContext variant, but got a different variant"),
    }

    Ok(())
}

#[wasm_bindgen_test]
fn test_extcall_mint() -> Result<()> {
    clear();
    let block_height = 840_000;

    // Create a cellpack to call the process_numbers method (opcode 11)
    let init_cellpack = Cellpack {
        target: AlkaneId { block: 1, tx: 0 },
        inputs: vec![50],
    };

    // Initialize the contract and execute the cellpacks
    let mut test_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
        [alkanes_std_test_build::get_bytes()].into(),
        [init_cellpack].into(),
    );

    // Create a cellpack to call the process_numbers method (opcode 11)
    let arb_mint_cellpack = Cellpack {
        target: AlkaneId { block: 2, tx: 1 },
        inputs: vec![31, 2, 1, 3, 30, 2, 0],
    };

    test_block
        .txdata
        .push(alkane_helpers::create_multiple_cellpack_with_witness(
            Witness::new(),
            vec![arb_mint_cellpack],
            false,
        ));

    index_block(&test_block, block_height)?;

    let sheet = alkane_helpers::get_last_outpoint_sheet(&test_block)?;

    println!("Last sheet: {:?}", sheet);

    assert_eq!(sheet.get_cached(&ProtoruneRuneId { block: 2, tx: 0 }), 0);
    assert_eq!(sheet.get_cached(&ProtoruneRuneId { block: 2, tx: 1 }), 0);

    let outpoint = OutPoint {
        txid: test_block.txdata.last().unwrap().compute_txid(),
        vout: 3,
    };

    let trace_data: Trace = view::trace(&outpoint)?.try_into()?;
    let trace_events = trace_data.0.lock().expect("Mutex poisoned");
    let last_trace_event = trace_events[trace_events.len() - 1].clone();
    match last_trace_event {
        TraceEvent::RevertContext(trace_response) => {
            // Now we have the TraceResponse, access the data field
            let data = String::from_utf8_lossy(&trace_response.inner.data);
            assert!(data.contains("ALKANES: revert: wasm `unreachable` instruction executed"));
        }
        _ => panic!("Expected RevertContext variant, but got a different variant"),
    }

    Ok(())
}
