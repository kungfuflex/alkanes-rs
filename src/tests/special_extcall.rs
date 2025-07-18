use crate::index_block;
use crate::tests::helpers::{self as alkane_helpers};
use crate::tests::std::alkanes_std_test_build;
use alkane_helpers::clear;
use alkanes::view;
use alkanes_support::cellpack::Cellpack;
use alkanes_support::id::AlkaneId;
use alkanes_support::trace::{Trace, TraceEvent};
use anyhow::Result;
use bitcoin::block::Header;
use bitcoin::OutPoint;
use bitcoin::Transaction;
#[allow(unused_imports)]
use metashrew_core::{
    println,
    stdio::{stdout, Write},
};
use protorune::test_helpers::create_coinbase_transaction;
use protorune_support::utils::consensus_decode;
use wasm_bindgen_test::wasm_bindgen_test;

#[wasm_bindgen_test]
fn test_special_extcall() -> Result<()> {
    clear();
    let block_height = 0;

    let get_header = Cellpack {
        target: AlkaneId { block: 1, tx: 0 },
        inputs: vec![101],
    };
    let coinbase_tx = Cellpack {
        target: AlkaneId { block: 2, tx: 1 },
        inputs: vec![102],
    };

    // Initialize the contract and execute the cellpacks
    let mut test_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
        [alkanes_std_test_build::get_bytes(), [].into()].into(),
        [get_header, coinbase_tx].into(),
    );

    for i in 0..5000 {
        test_block
            .txdata
            .push(create_coinbase_transaction(block_height));
    }

    index_block(&test_block, block_height)?;

    let outpoint_1 = OutPoint {
        txid: test_block.txdata[1].compute_txid(),
        vout: 3,
    };

    let raw_trace_data = view::trace(&outpoint_1)?;
    let trace_data: Trace = raw_trace_data.clone().try_into()?;

    let trace_event_1 = trace_data.0.lock().expect("Mutex poisoned").last().cloned();

    // Access the data field from the trace response
    if let Some(return_context) = trace_event_1 {
        // Use pattern matching to extract the data field from the TraceEvent enum
        match return_context {
            TraceEvent::ReturnContext(trace_response) => {
                // Now we have the TraceResponse, access the data field
                let data = consensus_decode::<Header>(&mut std::io::Cursor::new(
                    trace_response.inner.data,
                ))?;

                println!("{:?}", data);
                assert_eq!(data.time, 1231006505);
            }
            _ => panic!("Expected ReturnContext variant, but got a different variant"),
        }
    } else {
        panic!("Failed to get trace_event_1 from trace data");
    }

    let outpoint_2 = OutPoint {
        txid: test_block.txdata[2].compute_txid(),
        vout: 3,
    };

    let raw_trace_data = view::trace(&outpoint_2)?;
    let trace_data: Trace = raw_trace_data.clone().try_into()?;

    let trace_event_1 = trace_data.0.lock().expect("Mutex poisoned").last().cloned();

    // Access the data field from the trace response
    if let Some(return_context) = trace_event_1 {
        // Use pattern matching to extract the data field from the TraceEvent enum
        match return_context {
            TraceEvent::ReturnContext(trace_response) => {
                // Now we have the TraceResponse, access the data field
                let data = consensus_decode::<Transaction>(&mut std::io::Cursor::new(
                    trace_response.inner.data,
                ))?;

                println!("{:?}", data);
                assert_eq!(data.version, bitcoin::transaction::Version(2));
            }
            _ => panic!("Expected ReturnContext variant, but got a different variant"),
        }
    } else {
        panic!("Failed to get trace_event_1 from trace data");
    }

    Ok(())
}

#[wasm_bindgen_test]
fn test_special_extcall_number_diesel_mints() -> Result<()> {
    clear();
    let block_height = 0;

    let diesel_mint = Cellpack {
        target: AlkaneId { block: 2, tx: 0 },
        inputs: vec![77],
    };

    let get_num_diesel = Cellpack {
        target: AlkaneId { block: 1, tx: 0 },
        inputs: vec![106],
    };

    // Initialize the contract and execute the cellpacks
    let mut test_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
        [
            [].into(),
            [].into(),
            [].into(),
            [].into(),
            [].into(),
            alkanes_std_test_build::get_bytes(),
        ]
        .into(),
        [
            diesel_mint.clone(),
            diesel_mint.clone(),
            diesel_mint.clone(),
            diesel_mint.clone(),
            diesel_mint.clone(),
            get_num_diesel,
        ]
        .into(),
    );

    index_block(&test_block, block_height)?;

    let outpoint_1 = OutPoint {
        txid: test_block.txdata.last().unwrap().compute_txid(),
        vout: 3,
    };

    let raw_trace_data = view::trace(&outpoint_1)?;
    let trace_data: Trace = raw_trace_data.clone().try_into()?;

    let trace_event_1 = trace_data.0.lock().expect("Mutex poisoned").last().cloned();

    // Access the data field from the trace response
    if let Some(return_context) = trace_event_1 {
        // Use pattern matching to extract the data field from the TraceEvent enum
        match return_context {
            TraceEvent::ReturnContext(trace_response) => {
                // Now we have the TraceResponse, access the data field
                let data = u128::from_le_bytes(trace_response.inner.data[0..16].try_into()?);

                println!("{:?}", data);
                assert_eq!(data, 5);
            }
            _ => panic!("Expected ReturnContext variant, but got a different variant"),
        }
    } else {
        panic!("Failed to get trace_event_1 from trace data");
    }

    Ok(())
}

#[wasm_bindgen_test]
fn test_special_extcall_total_miner_fees() -> Result<()> {
    clear();
    let block_height = 0;

    let get_miner_fee = Cellpack {
        target: AlkaneId { block: 1, tx: 0 },
        inputs: vec![107],
    };

    // Initialize the contract and execute the cellpacks
    let mut test_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
        [alkanes_std_test_build::get_bytes()].into(),
        [get_miner_fee].into(),
    );

    index_block(&test_block, block_height)?;

    let outpoint_1 = OutPoint {
        txid: test_block.txdata.last().unwrap().compute_txid(),
        vout: 3,
    };

    let raw_trace_data = view::trace(&outpoint_1)?;
    let trace_data: Trace = raw_trace_data.clone().try_into()?;

    let trace_event_1 = trace_data.0.lock().expect("Mutex poisoned").last().cloned();

    // Access the data field from the trace response
    if let Some(return_context) = trace_event_1 {
        // Use pattern matching to extract the data field from the TraceEvent enum
        match return_context {
            TraceEvent::ReturnContext(trace_response) => {
                // Now we have the TraceResponse, access the data field
                let data = u128::from_le_bytes(trace_response.inner.data[0..16].try_into()?);

                println!("{:?}", data);
                assert_eq!(data, 50_000_000 * 7);
            }
            _ => panic!("Expected ReturnContext variant, but got a different variant"),
        }
    } else {
        panic!("Failed to get trace_event_1 from trace data");
    }

    Ok(())
}
