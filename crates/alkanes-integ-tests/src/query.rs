//! State query helpers using alkanes.wasm view functions.
//!
//! View functions expect `[height_le32][protobuf_request]` as input
//! and return protobuf-encoded responses in ArrayBuffer format.

use anyhow::{Context, Result};
use bitcoin::{Block, OutPoint};
use prost::Message;
use protorune_support::proto::protorune;

use crate::runtime::TestRuntime;

/// Encode a u128 as the protobuf uint128 message (lo/hi split).
fn encode_u128(v: u128) -> protorune::Uint128 {
    protorune::Uint128 {
        lo: v as u64,
        hi: (v >> 64) as u64,
    }
}

/// Decode a protobuf uint128 message to a Rust u128.
fn decode_u128(v: &Option<protorune::Uint128>) -> u128 {
    match v {
        Some(v) => v.lo as u128 | ((v.hi as u128) << 64),
        None => 0,
    }
}

/// Get the alkane balance sheet for a specific outpoint.
///
/// Calls `protorunesbyoutpoint` view function.
/// Returns a map of (block, tx) → balance.
pub fn get_balance_for_outpoint(
    runtime: &TestRuntime,
    outpoint: &OutPoint,
    height: u32,
) -> Result<Vec<(u128, u128, u128)>> {
    // Build OutpointWithProtocol protobuf
    let txid_bytes: Vec<u8> = bitcoin::consensus::serialize(&outpoint.txid);
    let request = protorune::OutpointWithProtocol {
        txid: txid_bytes,
        vout: outpoint.vout,
        protocol: Some(encode_u128(1)), // alkanes protocol tag = 1
    };

    let request_bytes = request.encode_to_vec();
    let response_bytes = runtime
        .alkanes_view("protorunesbyoutpoint", &request_bytes, height)
        .context("protorunesbyoutpoint view call failed")?;

    let response = protorune::OutpointResponse::decode(response_bytes.as_slice())
        .context("failed to decode OutpointResponse")?;

    let mut balances = Vec::new();
    if let Some(sheet) = &response.balances {
        for entry in &sheet.entries {
            if let Some(rune) = &entry.rune {
                if let Some(id) = &rune.rune_id {
                    let block = decode_u128(&id.height);
                    let tx = decode_u128(&id.txindex);
                    let balance = decode_u128(&entry.balance);
                    balances.push((block, tx, balance));
                }
            }
        }
    }

    Ok(balances)
}

/// Get the balance of a specific alkane at an outpoint.
pub fn get_alkane_balance(
    runtime: &TestRuntime,
    outpoint: &OutPoint,
    alkane_block: u128,
    alkane_tx: u128,
    height: u32,
) -> Result<u128> {
    let balances = get_balance_for_outpoint(runtime, outpoint, height)?;
    Ok(balances
        .iter()
        .find(|(b, t, _)| *b == alkane_block && *t == alkane_tx)
        .map(|(_, _, bal)| *bal)
        .unwrap_or(0))
}

/// Helper: get balance at the last tx's first output in a block.
pub fn get_last_outpoint_balance(
    runtime: &TestRuntime,
    block: &Block,
    alkane_block: u128,
    alkane_tx: u128,
    height: u32,
) -> Result<u128> {
    let last_tx = block
        .txdata
        .last()
        .context("block has no transactions")?;
    let outpoint = OutPoint {
        txid: last_tx.compute_txid(),
        vout: 0,
    };
    get_alkane_balance(runtime, &outpoint, alkane_block, alkane_tx, height)
}

/// Call alkanes `simulate` view function.
///
/// Input is a MessageContextParcel protobuf (from alkanes_support::proto).
pub fn simulate_raw(
    runtime: &TestRuntime,
    request_bytes: &[u8],
    height: u32,
) -> Result<Vec<u8>> {
    runtime
        .alkanes_view("simulate", request_bytes, height)
        .context("simulate view call failed")
}

/// Call alkanes `trace` view function.
pub fn trace_raw(
    runtime: &TestRuntime,
    outpoint_bytes: &[u8],
    height: u32,
) -> Result<Vec<u8>> {
    runtime
        .alkanes_view("trace", outpoint_bytes, height)
        .context("trace view call failed")
}

/// Call alkanes `sequence` view function.
pub fn get_sequence(runtime: &TestRuntime, height: u32) -> Result<Vec<u8>> {
    runtime
        .alkanes_view("sequence", &[], height)
        .context("sequence view call failed")
}
