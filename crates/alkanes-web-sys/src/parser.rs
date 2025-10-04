//! Block and transaction parsing functionality for deezel-web.
//!
//! This module provides functions for parsing raw block and transaction data
//! into structured formats, including headers, transactions, and protostones.

use wasm_bindgen::prelude::*;
use deezel_common::{
    self,
    JsonValue,
    Runestone,
};
use bitcoin::{
    Block,
    block::Header as BlockHeader,
    Transaction,
};
use crate::alloc::{vec::Vec, format};
use ordinals::Artifact;
use protorune_support::protostone::Protostone;

fn decipher_protostones(tx: &Transaction) -> Option<Vec<Protostone>> {
    let artifact = Runestone::decipher(tx);
    if let Some(Artifact::Runestone(runestone)) = artifact {
        if let Some(payload) = runestone.protocol {
            return Protostone::decipher(&payload).ok();
        }
    }
    None
}

#[wasm_bindgen]
pub fn parse_block(block_hex: &str) -> Result<JsValue, JsValue> {
    let block_bytes = hex::decode(block_hex)
        .map_err(|e| JsValue::from_str(&format!("Failed to decode block hex: {e}")))?;
    
    let block: Block = bitcoin::consensus::deserialize(&block_bytes)
        .map_err(|e| JsValue::from_str(&format!("Failed to deserialize block: {e}")))?;

    let header: &BlockHeader = &block.header;
    let transactions: Vec<JsonValue> = block.txdata.iter().map(|tx| {
        let protostones = decipher_protostones(tx);
        let artifact = Runestone::decipher(tx);
        let runestone_str = artifact.map(|a| {
            match a {
                Artifact::Runestone(r) => format!("{r:?}"),
                Artifact::Cenotaph(c) => format!("cenotaph: {c:?}"),
            }
        });
        serde_json::json!({
            "txid": tx.compute_txid(),
            "transaction": tx,
            "protostones": protostones.map(|p| format!("{p:?}")),
            "runestone": runestone_str,
        })
    }).collect();

    let result = serde_json::json!({
        "header": header,
        "transactions": transactions,
    });

    let js_result = serde_wasm_bindgen::to_value(&result)
        .map_err(|e| JsValue::from_str(&format!("Failed to serialize result: {e}")))?;

    Ok(js_result)
}