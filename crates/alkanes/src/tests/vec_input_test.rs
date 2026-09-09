use crate::{message::AlkaneMessageContext, tests::std::alkanes_std_test_build};
use alkanes_support::cellpack::Cellpack;
use alkanes_support::id::AlkaneId;
use anyhow::{anyhow, Result};
use bitcoin::OutPoint;
use metashrew_support::utils::consensus_encode;

use crate::index_block;
use crate::tests::helpers::{self as alkane_helpers, assert_binary_deployed_to_id};
use alkane_helpers::clear;
use alkanes::view;
use bitcoin::Witness;
#[allow(unused_imports)]
use metashrew_core::{
    println,
    stdio::{stdout, Write},
};
use alkanes_support::proto::alkanes as alkanes_proto;
use prost::Message;
use wasm_bindgen_test::wasm_bindgen_test;

/// Decode the u128 an alkane returned, from the LAST exit context in a trace.
///
/// This used to be `trace_bytes[trace_bytes.len() - 16]` — a raw byte offset
/// into the serialized protobuf, which only worked while `response.data` was
/// the final thing on the wire. `AlkanesExitContext` gained `fuel_used`
/// (field 3, additive) on 2026-07-28; that field now encodes AFTER the
/// response, so `len - 16` stopped pointing at the return value and this test
/// began failing. It went unnoticed because CI could not run at all between
/// 2026-07-19 and 2026-08-06 (`cargo install wasm-bindgen-cli` was broken by
/// an MSRV bump in a transitive dep).
///
/// Decoding the message instead of counting backwards from the end makes the
/// assertion immune to any further additive proto field — which is the whole
/// point of the fields being additive.
fn returned_u128(trace_bytes: &[u8]) -> Result<u128> {
    let trace = alkanes_proto::AlkanesTrace::decode(trace_bytes)
        .map_err(|e| anyhow!("failed to decode AlkanesTrace: {e}"))?;

    let data = trace
        .events
        .iter()
        .rev()
        .find_map(|event| match &event.event {
            Some(alkanes_proto::alkanes_trace_event::Event::ExitContext(exit)) => {
                exit.response.as_ref().map(|r| r.data.clone())
            }
            _ => None,
        })
        .ok_or(anyhow!("trace contains no exit context with a response"))?;

    if data.len() < 16 {
        return Err(anyhow!(
            "expected at least a 16-byte u128 return value, got {} bytes",
            data.len()
        ));
    }
    // Alkanes returns u128 little-endian; take the leading value.
    let mut le = [0u8; 16];
    le.copy_from_slice(&data[..16]);
    Ok(u128::from_le_bytes(le))
}

#[wasm_bindgen_test]
fn test_vec_inputs() -> Result<()> {
    clear();
    let block_height = 0;
    // Get the LoggerAlkane ID
    let logger_alkane_id = AlkaneId { block: 2, tx: 1 };

    // Create a cellpack to call the process_numbers method (opcode 11)
    let process_numbers_cellpack = Cellpack {
        target: AlkaneId { block: 1, tx: 0 },
        inputs: vec![
            11, // opcode for process_numbers
            4,  // length of the vector
            10, // first element
            20, // second element
            30, // third element
            40, // fourth element
        ],
    };

    // Create a cellpack to call the process_strings method (opcode 12)
    // For "hello" and "world" strings with null terminators
    let hello_bytes = u128::from_le_bytes(*b"hello\0\0\0\0\0\0\0\0\0\0\0");
    let world_bytes = u128::from_le_bytes(*b"world\0\0\0\0\0\0\0\0\0\0\0");

    let process_strings_cellpack = Cellpack {
        target: logger_alkane_id.clone(),
        inputs: vec![
            12,          // opcode for process_strings
            2,           // length of the vector
            hello_bytes, // "hello" string
            world_bytes, // "world" string
        ],
    };

    // Create a cellpack to call the process_nested_vec method (opcode 15)
    let process_nested_vec_cellpack = Cellpack {
        target: logger_alkane_id.clone(),
        inputs: vec![
            13, // opcode for process_nested_vec
            2,  // length of the outer vector
            3,  // length of first inner vector
            1, 2, 3, // elements of first inner vector
            2, // length of second inner vector
            4, 5, // elements of second inner vector
        ],
    };

    // Initialize the contract and execute the cellpacks
    let mut test_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
        [alkanes_std_test_build::get_bytes()].into(),
        [process_numbers_cellpack].into(),
    );

    // Add a transaction with the remaining cellpacks
    test_block.txdata.push(
        alkane_helpers::create_multiple_cellpack_with_witness_and_in(
            Witness::new(),
            vec![process_strings_cellpack, process_nested_vec_cellpack],
            OutPoint {
                txid: test_block
                    .txdata
                    .last()
                    .ok_or(anyhow!("no last el"))?
                    .compute_txid(),
                vout: 0,
            },
            false,
        ),
    );

    index_block(&test_block, block_height)?;

    // Verify the binary was deployed correctly
    let _ = assert_binary_deployed_to_id(
        logger_alkane_id.clone(),
        alkanes_std_test_build::get_bytes(),
    );

    // Get the trace data from the transaction for process_numbers
    let outpoint_process_numbers = OutPoint {
        txid: test_block.txdata[1].compute_txid(),
        vout: 3,
    };

    let trace_data_process_numbers = view::trace(&outpoint_process_numbers)?;
    println!("process_numbers trace: {:?}", trace_data_process_numbers);

    // process_numbers sums the vector: 10 + 20 + 30 + 40 = 100.
    assert_eq!(returned_u128(&trace_data_process_numbers)?, 100);

    // Get the trace data from the transaction for get_strings
    let outpoint_get_strings = OutPoint {
        txid: test_block
            .txdata
            .last()
            .ok_or(anyhow!("no last el"))?
            .compute_txid(),
        vout: 3,
    };

    let trace_data_get_strings = view::trace(&outpoint_get_strings)?;
    let trace_str = String::from_utf8_lossy(&trace_data_get_strings);
    println!("get_strings trace: {:?}", trace_str);
    let expected_name = "hello,world";

    // Verify the get_strings result contains the expected values
    // The result should be a vector with ["hello", "world"]
    assert!(
        trace_str.contains(expected_name),
        "Trace data should contain the name '{}', but it doesn't",
        expected_name
    );

    // Get the trace data from the transaction for process_nested_vec
    let outpoint_process_nested_vec = OutPoint {
        txid: test_block
            .txdata
            .last()
            .ok_or(anyhow!("no last el"))?
            .compute_txid(),
        vout: 4,
    };

    let trace_data_process_nested_vec = view::trace(&outpoint_process_nested_vec)?;
    println!(
        "process_nested_vec trace: {:?}",
        trace_data_process_nested_vec
    );

    // The result should be the total number of elements: 3 + 2 = 5
    assert_eq!(returned_u128(&trace_data_process_nested_vec)?, 5);

    Ok(())
}
