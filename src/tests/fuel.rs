use crate::tests::std::alkanes_std_test_build;
use alkanes_support::cellpack::Cellpack;
use alkanes_support::id::AlkaneId;
use alkanes_support::trace::{Trace, TraceEvent};
use anyhow::Result;
use bitcoin::OutPoint;

use crate::index_block;
use crate::tests::helpers::{self as alkane_helpers};
use alkane_helpers::clear;
use alkanes::view;
#[allow(unused_imports)]
use metashrew_core::{
    println,
    stdio::{stdout, Write},
};
use wasm_bindgen_test::wasm_bindgen_test;

#[wasm_bindgen_test]
fn test_infinite_loop() -> Result<()> {
    clear();
    let block_height = 840_000;

    // Create a cellpack to call the process_numbers method (opcode 11)
    let infinite_exec_cellpack = Cellpack {
        target: AlkaneId { block: 1, tx: 0 },
        inputs: vec![20],
    };

    // Initialize the contract and execute the cellpacks
    let mut test_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
        [alkanes_std_test_build::get_bytes()].into(),
        [infinite_exec_cellpack].into(),
    );

    index_block(&test_block, block_height)?;

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
            assert!(data.contains("ALKANES: revert: all fuel consumed by WebAssembly"));
        }
        _ => panic!("Expected RevertContext variant, but got a different variant"),
    }

    Ok(())
}

#[wasm_bindgen_test]
fn test_infinite_extcall_loop() -> Result<()> {
    clear();
    let block_height = 840_000;

    // Create a cellpack to call the process_numbers method (opcode 11)
    let infinite_exec_cellpack = Cellpack {
        target: AlkaneId { block: 1, tx: 0 },
        inputs: vec![21],
    };

    // Initialize the contract and execute the cellpacks
    let mut test_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
        [alkanes_std_test_build::get_bytes()].into(),
        [infinite_exec_cellpack].into(),
    );

    index_block(&test_block, block_height)?;

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
            assert!(data
                .contains("Possible infinite recursion encountered: checkpoint depth too large"));
        }
        _ => panic!("Expected RevertContext variant, but got a different variant"),
    }

    Ok(())
}
