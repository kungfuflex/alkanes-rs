use crate::tests::std::alkanes_std_test_build;
use alkanes_support::cellpack::Cellpack;
use alkanes_support::id::AlkaneId;
use alkanes_support::trace::{Trace, TraceEvent};
use anyhow::Result;
use bitcoin::block::Header;
use bitcoin::{OutPoint, ScriptBuf, Sequence, TxIn, Witness};
use protorune::test_helpers::create_coinbase_transaction;
use protorune_support::protostone::ProtostoneEdict;
use protorune_support::utils::consensus_decode;

use crate::index_block;
use crate::tests::helpers::{self as alkane_helpers, get_sheet_for_runtime};
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
fn test_special_extcall() -> Result<()> {
    clear();
    let block_height = 840_000;

    // Create a cellpack to call the process_numbers method (opcode 11)
    let special_cellpack = Cellpack {
        target: AlkaneId { block: 1, tx: 0 },
        inputs: vec![101],
    };

    // Initialize the contract and execute the cellpacks
    let mut test_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
        [alkanes_std_test_build::get_bytes()].into(),
        [special_cellpack].into(),
    );

    for i in 0..5000 {
        test_block
            .txdata
            .push(create_coinbase_transaction(block_height));
    }

    index_block(&test_block, block_height)?;

    let outpoint_3 = OutPoint {
        txid: test_block.txdata[1].compute_txid(),
        vout: 3,
    };

    let raw_trace_data = view::trace(&outpoint_3)?;
    let trace_data: Trace = raw_trace_data.clone().try_into()?;

    let last_trace_event = trace_data.0.lock().expect("Mutex poisoned").last().cloned();

    // Access the data field from the trace response
    if let Some(return_context) = last_trace_event {
        // Use pattern matching to extract the data field from the TraceEvent enum
        match return_context {
            TraceEvent::ReturnContext(trace_response) => {
                // Now we have the TraceResponse, access the data field
                let data = consensus_decode::<Header>(&mut std::io::Cursor::new(
                    trace_response.inner.data,
                ))?;

                println!("{:?}", data);
            }
            _ => panic!("Expected ReturnContext variant, but got a different variant"),
        }
    } else {
        panic!("Failed to get last_trace_event from trace data");
    }

    Ok(())
}
