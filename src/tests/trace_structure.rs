use crate::tests::std::alkanes_std_test_build;
use alkanes_support::cellpack::Cellpack;
use alkanes_support::id::AlkaneId;
use alkanes_support::trace::{Trace, TraceEvent};
use anyhow::Result;
use bitcoin::OutPoint;

use crate::index_block;
use crate::tests::helpers::{self as alkane_helpers};
use crate::view;
use alkane_helpers::clear;
#[allow(unused_imports)]
use metashrew_core::{
    println,
    stdio::{stdout, Write},
};
use wasm_bindgen_test::wasm_bindgen_test;

#[wasm_bindgen_test]
fn test_trace_structure_basic() -> Result<()> {
    clear();
    let block_height = 0;

    // Create a simple cellpack that mints some tokens
    let mint_cellpack = Cellpack {
        target: AlkaneId { block: 1, tx: 0 },
        inputs: vec![30, 2, 1, 100], // Mint 100 tokens
    };

    // Initialize the contract and execute the cellpack
    let test_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
        vec![alkanes_std_test_build::get_bytes()],
        vec![mint_cellpack],
    );

    index_block(&test_block, block_height)?;

    // Get the trace for the transaction
    let last_tx = &test_block.txdata[test_block.txdata.len() - 1];
    let outpoint = OutPoint {
        txid: last_tx.compute_txid(),
        vout: 3, // The protostone vout (virtual output)
    };

    let trace_data: Trace = view::trace(&outpoint)?.try_into()?;
    let trace_events = trace_data.0.lock().expect("Mutex poisoned");

    println!("Total trace events: {}", trace_events.len());
    for (i, event) in trace_events.iter().enumerate() {
        println!("Event {}: {:?}", i, event);
    }

    // Verify trace structure
    assert!(
        trace_events.len() >= 3,
        "Trace should have at least 3 events (ReceiveIntent, EnterCall, ReturnContext/RevertContext)"
    );

    // First event should be ReceiveIntent
    match &trace_events[0] {
        TraceEvent::ReceiveIntent { incoming_alkanes } => {
            println!(
                "✓ First event is ReceiveIntent with {} transfers",
                incoming_alkanes.0.len()
            );
            assert_eq!(incoming_alkanes.0.len(), 0);
        }
        _ => panic!(
            "First trace event should be ReceiveIntent, got: {:?}",
            trace_events[0]
        ),
    }

    // Second event should be EnterCall
    match &trace_events[1] {
        TraceEvent::EnterCall(context) => {
            println!(
                "✓ Second event is EnterCall for target: {:?}",
                context.target
            );
        }
        _ => panic!(
            "Second trace event should be EnterCall, got: {:?}",
            trace_events[1]
        ),
    }

    // Third event should be ReturnContext or RevertContext
    match &trace_events[2] {
        TraceEvent::ReturnContext(_) => {
            println!("✓ Third event is ReturnContext (success)");
        }
        TraceEvent::RevertContext(_) => {
            println!("✓ Third event is RevertContext (revert)");
        }
        _ => panic!(
            "Third trace event should be ReturnContext or RevertContext, got: {:?}",
            trace_events[2]
        ),
    }

    // Last event should be ValueTransfer
    let last_event = trace_events.last().unwrap();
    match last_event {
        TraceEvent::ValueTransfer {
            transfers,
            redirect_to,
        } => {
            println!("✓ Last event is ValueTransfer (success)");
            assert_eq!(transfers.len(), 1);
            assert_eq!(transfers[0].value, 100);
            assert_eq!(*redirect_to, 0);
        }
        _ => panic!(
            "Last trace event should be ValueTransfer, got: {:?}",
            last_event
        ),
    }

    // Check if there's a ValueTransfer event (should be present after minting)
    let has_value_transfer = trace_events
        .iter()
        .any(|event| matches!(event, TraceEvent::ValueTransfer { .. }));

    if has_value_transfer {
        println!("✓ Trace contains ValueTransfer event");
    } else {
        println!("⚠ No ValueTransfer event found (might be expected if no transfers occurred)");
    }

    Ok(())
}
