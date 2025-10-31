// Chadson's Journal
// Date: 2025-08-04
//
// Task: Fix wallet connection issues in slope-frontend.
//
// Current Status:
// I've been stuck on a circular compilation error in `deezel-web`.
// The root cause is that `lib.rs` was not declaring the crate's modules correctly.
// It contained a lot of old, conflicting code.
//
// Plan:
// 1.  Overwrite `lib.rs` to properly declare all public modules.
// 2.  This should resolve the `unresolved import` errors.
// 3.  Re-compile the project.

use wasm_bindgen::prelude::*;
use bitcoin::psbt::Psbt;
use alkanes_cli_common::runestone_enhanced::format_runestone_with_decoded_messages;
use alkanes_cli_common::alkanes::inspector::analysis::perform_fuzzing_analysis;
use alkanes_cli_common::alkanes::types::AlkaneId;
use js_sys::Promise;
use wasm_bindgen_futures::future_to_promise;
pub use crate::provider::WebProvider;
use alkanes_cli_common::AlkanesProvider;
use base64::{engine::general_purpose::STANDARD, Engine as _};


pub mod crypto;
pub mod keystore;
pub mod logging;
pub mod network;
pub mod provider;
pub mod storage;
pub mod time;
pub mod utils;
pub mod wallet_provider;
pub mod keystore_wallet;

#[wasm_bindgen]
pub fn analyze_psbt(psbt_base64: &str) -> Result<String, JsValue> {
    let psbt_bytes = STANDARD.decode(psbt_base64)
        .map_err(|e| JsValue::from_str(&format!("base64 decode error: {}", e)))?;
    let psbt: Psbt = Psbt::deserialize(&psbt_bytes)
        .map_err(|e| JsValue::from_str(&format!("PSBT deserialize error: {}", e)))?;

    let tx = psbt.extract_tx()
        .map_err(|e| JsValue::from_str(&format!("PSBT extract_tx error: {}", e)))?;

    let analysis = format_runestone_with_decoded_messages(&tx)
        .map_err(|e| JsValue::from_str(&format!("Runestone analysis error: {}", e)))?;

    serde_json::to_string(&analysis)
        .map_err(|e| JsValue::from_str(&format!("JSON serialization error: {}", e)))
}

#[wasm_bindgen]
pub fn simulate_alkane_call(alkane_id_str: &str, wasm_hex: &str, cellpack_hex: &str) -> Promise {
    let wasm_bytes = match hex::decode(wasm_hex.strip_prefix("0x").unwrap_or(wasm_hex)) {
        Ok(bytes) => bytes,
        Err(e) => return future_to_promise(async move { Err(JsValue::from_str(&format!("WASM hex decode error: {}", e))) }),
    };

    let _cellpack_bytes = match hex::decode(cellpack_hex.strip_prefix("0x").unwrap_or(cellpack_hex)) {
        Ok(bytes) => bytes,
        Err(e) => return future_to_promise(async move { Err(JsValue::from_str(&format!("Cellpack hex decode error: {}", e))) }),
    };

    // The inspector's fuzzing analysis function is perfect for this.
    // We can treat the cellpack as a single "opcode" to test.
    // The `perform_fuzzing_analysis` function expects opcodes as u128.
    // We need to get the opcode from the cellpack.
    // For now, let's assume the first element in the cellpack is the opcode.
    // This part needs to be more robust based on actual cellpack structure.
    let alkane_id: AlkaneId = match serde_json::from_str(alkane_id_str) {
        Ok(id) => id,
        Err(e) => return future_to_promise(async move { Err(JsValue::from_str(&format!("AlkaneId deserialize error: {}", e))) }),
    };
    
    future_to_promise(async move {
        let fuzz_ranges = "0-1"; // Placeholder
        match perform_fuzzing_analysis(&alkane_id, &wasm_bytes, Some(fuzz_ranges)).await {
            Ok(fuzz_result) => {
                let result_json = serde_json::to_string(&fuzz_result)
                    .map_err(|e| JsValue::from_str(&format!("Fuzz result serialization error: {}", e)))?;
                Ok(JsValue::from_str(&result_json))
            }
            Err(e) => Err(JsValue::from_str(&format!("Alkane simulation error: {}", e))),
        }
    })
}

#[wasm_bindgen]
pub fn get_alkane_bytecode(network: &str, block: f64, tx: f64, block_tag: &str) -> Promise {
    let network_str = network.to_string();
    let alkane_id = format!("{}:{}", block as u64, tx as u32);
    let block_tag_opt = if block_tag.is_empty() {
        None
    } else {
        Some(block_tag.to_string())
    };

    future_to_promise(async move {
        let provider = WebProvider::new(network_str).await
            .map_err(|e| JsValue::from_str(&format!("Failed to create provider: {:?}", e)))?;

        match provider.get_bytecode(&alkane_id, block_tag_opt).await {
            Ok(bytecode_hex) => {
                Ok(JsValue::from_str(&bytecode_hex))
            }
            Err(e) => Err(JsValue::from_str(&format!("get_bytecode failed: {:?}", e))),
        }
    })
}
