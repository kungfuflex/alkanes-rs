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
fn test_trace_with_receive_intent_and_value_transfer() -> Result<()> {
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

    println!("✅ Total trace events: {}", trace_events.len());
    for (i, event) in trace_events.iter().enumerate() {
        println!("  Event {}: {:?}", i, event);
    }

    // Verify trace structure - protorune traces have ReceiveIntent and ValueTransfer
    // The alkanes runtime events (EnterCall, ReturnContext) are in separate traces
    assert!(
        trace_events.len() >= 2,
        "Trace should have at least 2 events (ReceiveIntent, ValueTransfer)"
    );

    // First event should be ReceiveIntent
    match &trace_events[0] {
        TraceEvent::ReceiveIntent { incoming_alkanes } => {
            println!(
                "✅ First event is ReceiveIntent with {} transfers",
                incoming_alkanes.0.len()
            );
            // For a mint operation, we don't have incoming alkanes initially
            assert_eq!(incoming_alkanes.0.len(), 0);
        }
        _ => panic!(
            "❌ First trace event should be ReceiveIntent, got: {:?}",
            trace_events[0]
        ),
    }

    // Last event should be ValueTransfer
    let last_event = trace_events.last().unwrap();
    match last_event {
        TraceEvent::ValueTransfer {
            transfers,
            redirect_to,
        } => {
            println!("✅ Last event is ValueTransfer");
            println!("   - transfers: {}", transfers.len());
            println!("   - redirect_to: vout {}", redirect_to);
            assert_eq!(transfers.len(), 1, "Expected 1 transfer");
            assert_eq!(transfers[0].value, 100, "Expected transfer value of 100");
            assert_eq!(*redirect_to, 0, "Expected redirect to vout 0");
            println!("   - transfer[0].id: [{}:{}]", transfers[0].id.block, transfers[0].id.tx);
            println!("   - transfer[0].value: {}", transfers[0].value);
        }
        _ => panic!(
            "❌ Last trace event should be ValueTransfer, got: {:?}",
            last_event
        ),
    }

    // Verify that ValueTransfer events exist
    let value_transfer_count = trace_events
        .iter()
        .filter(|event| matches!(event, TraceEvent::ValueTransfer { .. }))
        .count();

    assert!(
        value_transfer_count > 0,
        "❌ Trace should contain at least one ValueTransfer event"
    );
    println!("✅ Trace contains {} ValueTransfer event(s)", value_transfer_count);

    // Verify that ReceiveIntent events exist
    let receive_intent_count = trace_events
        .iter()
        .filter(|event| matches!(event, TraceEvent::ReceiveIntent { .. }))
        .count();

    assert!(
        receive_intent_count > 0,
        "❌ Trace should contain at least one ReceiveIntent event"
    );
    println!("✅ Trace contains {} ReceiveIntent event(s)", receive_intent_count);

    println!("\n🎉 All trace structure assertions passed!");
    println!("   - ReceiveIntent events: ✅");
    println!("   - ValueTransfer events: ✅");
    println!("   - Proper event ordering: ✅");

    Ok(())
}
