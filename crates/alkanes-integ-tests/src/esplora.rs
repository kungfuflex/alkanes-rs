//! Esplora view function wrappers.
//!
//! esplorashrew.wasm exports view functions matching the Esplora REST API.
//! Each view takes its input as raw bytes and returns JSON in an ArrayBuffer.

use anyhow::Result;

use crate::runtime::TestRuntime;

/// Call esplorashrew's `txhex` view — returns hex-encoded transaction.
pub fn get_tx_hex(runtime: &TestRuntime, txid_bytes: &[u8], height: u32) -> Result<Vec<u8>> {
    runtime.esplora_view("txhex", txid_bytes, height)
}

/// Call esplorashrew's `tx` view — returns JSON transaction details.
pub fn get_tx(runtime: &TestRuntime, txid_bytes: &[u8], height: u32) -> Result<Vec<u8>> {
    runtime.esplora_view("tx", txid_bytes, height)
}

/// Call esplorashrew's `utxosbyscripthash` view.
pub fn get_utxos_by_scripthash(
    runtime: &TestRuntime,
    scripthash: &[u8],
    height: u32,
) -> Result<Vec<u8>> {
    runtime.esplora_view("utxosbyscripthash", scripthash, height)
}

/// Call esplorashrew's `tipheight` view.
pub fn get_tip_height(runtime: &TestRuntime, height: u32) -> Result<Vec<u8>> {
    runtime.esplora_view("tipheight", &[], height)
}

/// Call esplorashrew's `blockheight` view.
pub fn get_block_at_height(
    runtime: &TestRuntime,
    block_height_bytes: &[u8],
    tip_height: u32,
) -> Result<Vec<u8>> {
    runtime.esplora_view("blockheight", block_height_bytes, tip_height)
}
