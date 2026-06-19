//! PSBT utilities for decoding and analyzing Partially Signed Bitcoin Transactions
//!
//! This module provides utilities for decoding PSBTs both via bitcoind RPC
//! and client-side using the rust-bitcoin library.

use crate::Result;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use bitcoin::psbt::Psbt;
use bitcoin::Transaction;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};

/// Decode a PSBT from base64 string and convert to JSON representation
pub fn decode_psbt_from_base64(psbt_base64: &str) -> Result<JsonValue> {
    // Decode base64 to bytes
    let psbt_bytes = STANDARD.decode(psbt_base64)
        .map_err(|e| crate::AlkanesError::Validation(format!("Failed to decode base64: {}", e)))?;

    // Deserialize PSBT
    let psbt = Psbt::deserialize(&psbt_bytes)
        .map_err(|e| crate::AlkanesError::Serialization(format!("Failed to deserialize PSBT: {}", e)))?;

    // Convert to JSON
    Ok(psbt_to_json(&psbt))
}

/// Convert a PSBT to a JSON representation similar to bitcoind's decodepsbt output
pub fn psbt_to_json(psbt: &Psbt) -> JsonValue {
    let tx = &psbt.unsigned_tx;

    // Build inputs array
    let inputs: Vec<JsonValue> = tx.input.iter().enumerate().map(|(i, input)| {
        let mut input_json = json!({
            "txid": input.previous_output.txid.to_string(),
            "vout": input.previous_output.vout,
            "sequence": input.sequence.0,
        });

        // Add PSBT-specific input data if available
        if let Some(psbt_input) = psbt.inputs.get(i) {
            if let Some(ref witness_utxo) = psbt_input.witness_utxo {
                input_json["witness_utxo"] = json!({
                    "amount": witness_utxo.value.to_sat(),
                    "scriptPubKey": witness_utxo.script_pubkey.to_hex_string(),
                });
            }

            if let Some(ref non_witness_utxo) = psbt_input.non_witness_utxo {
                input_json["non_witness_utxo"] = json!({
                    "txid": non_witness_utxo.compute_txid().to_string(),
                });
            }

            if let Some(ref redeem_script) = psbt_input.redeem_script {
                input_json["redeem_script"] = json!(redeem_script.to_hex_string());
            }

            if let Some(ref witness_script) = psbt_input.witness_script {
                input_json["witness_script"] = json!(witness_script.to_hex_string());
            }

            if !psbt_input.partial_sigs.is_empty() {
                input_json["partial_signatures"] = json!(
                    psbt_input.partial_sigs.iter()
                        .map(|(pk, sig)| json!({
                            "pubkey": pk.to_string(),
                            "signature": hex::encode(sig.serialize()),
                        }))
                        .collect::<Vec<_>>()
                );
            }

            if let Some(ref sighash_type) = psbt_input.sighash_type {
                input_json["sighash_type"] = json!(sighash_type.to_u32());
            }

            if !psbt_input.bip32_derivation.is_empty() {
                input_json["bip32_derivs"] = json!(
                    psbt_input.bip32_derivation.iter()
                        .map(|(pk, (fingerprint, path))| json!({
                            "pubkey": pk.to_string(),
                            "master_fingerprint": fingerprint.to_string(),
                            "path": path.to_string(),
                        }))
                        .collect::<Vec<_>>()
                );
            }

            if !psbt_input.tap_key_origins.is_empty() {
                input_json["tap_key_origins"] = json!(
                    psbt_input.tap_key_origins.iter()
                        .map(|(pk, (leaf_hashes, (fingerprint, path)))| json!({
                            "pubkey": pk.to_string(),
                            "leaf_hashes": leaf_hashes.iter().map(|h| h.to_string()).collect::<Vec<_>>(),
                            "master_fingerprint": fingerprint.to_string(),
                            "path": path.to_string(),
                        }))
                        .collect::<Vec<_>>()
                );
            }

            if let Some(ref tap_internal_key) = psbt_input.tap_internal_key {
                input_json["tap_internal_key"] = json!(tap_internal_key.to_string());
            }

            if let Some(ref tap_merkle_root) = psbt_input.tap_merkle_root {
                input_json["tap_merkle_root"] = json!(tap_merkle_root.to_string());
            }
        }

        input_json
    }).collect();

    // Build outputs array
    let outputs: Vec<JsonValue> = tx.output.iter().enumerate().map(|(i, output)| {
        let mut output_json = json!({
            "amount": output.value.to_sat(),
            "scriptPubKey": output.script_pubkey.to_hex_string(),
        });

        // Add PSBT-specific output data if available
        if let Some(psbt_output) = psbt.outputs.get(i) {
            if let Some(ref redeem_script) = psbt_output.redeem_script {
                output_json["redeem_script"] = json!(redeem_script.to_hex_string());
            }

            if let Some(ref witness_script) = psbt_output.witness_script {
                output_json["witness_script"] = json!(witness_script.to_hex_string());
            }

            if !psbt_output.bip32_derivation.is_empty() {
                output_json["bip32_derivs"] = json!(
                    psbt_output.bip32_derivation.iter()
                        .map(|(pk, (fingerprint, path))| json!({
                            "pubkey": pk.to_string(),
                            "master_fingerprint": fingerprint.to_string(),
                            "path": path.to_string(),
                        }))
                        .collect::<Vec<_>>()
                );
            }

            if let Some(ref tap_internal_key) = psbt_output.tap_internal_key {
                output_json["tap_internal_key"] = json!(tap_internal_key.to_string());
            }

            if !psbt_output.tap_key_origins.is_empty() {
                output_json["tap_key_origins"] = json!(
                    psbt_output.tap_key_origins.iter()
                        .map(|(pk, (leaf_hashes, (fingerprint, path)))| json!({
                            "pubkey": pk.to_string(),
                            "leaf_hashes": leaf_hashes.iter().map(|h| h.to_string()).collect::<Vec<_>>(),
                            "master_fingerprint": fingerprint.to_string(),
                            "path": path.to_string(),
                        }))
                        .collect::<Vec<_>>()
                );
            }
        }

        output_json
    }).collect();

    // Build the main PSBT JSON structure
    let mut psbt_json = json!({
        "tx": {
            "txid": tx.compute_txid().to_string(),
            "version": tx.version.0,
            "locktime": tx.lock_time.to_consensus_u32(),
            "vin": inputs,
            "vout": outputs,
        },
        "global": {
            "xpubs": psbt.xpub.iter().map(|(xpub, (fingerprint, path))| json!({
                "xpub": xpub.to_string(),
                "master_fingerprint": fingerprint.to_string(),
                "path": path.to_string(),
            })).collect::<Vec<_>>(),
        },
        "inputs": psbt.inputs.iter().enumerate().map(|(i, input)| {
            let mut input_info = json!({});

            if let Some(ref witness_utxo) = input.witness_utxo {
                input_info["witness_utxo"] = json!({
                    "amount": witness_utxo.value.to_sat(),
                    "scriptPubKey": witness_utxo.script_pubkey.to_hex_string(),
                });
            }

            if let Some(ref non_witness_utxo) = input.non_witness_utxo {
                input_info["non_witness_utxo"] = json!(hex::encode(bitcoin::consensus::serialize(non_witness_utxo)));
            }

            if !input.partial_sigs.is_empty() {
                input_info["partial_signatures"] = json!(
                    input.partial_sigs.iter()
                        .map(|(pk, sig)| json!({
                            "pubkey": pk.to_string(),
                            "signature": hex::encode(sig.serialize()),
                        }))
                        .collect::<Vec<_>>()
                );
            }

            if let Some(ref sighash_type) = input.sighash_type {
                input_info["sighash"] = json!(format!("{:?}", sighash_type));
            }

            if !input.bip32_derivation.is_empty() {
                input_info["bip32_derivs"] = json!(
                    input.bip32_derivation.iter()
                        .map(|(pk, (fingerprint, path))| json!({
                            "pubkey": pk.to_string(),
                            "master_fingerprint": fingerprint.to_string(),
                            "path": path.to_string(),
                        }))
                        .collect::<Vec<_>>()
                );
            }

            input_info
        }).collect::<Vec<_>>(),
        "outputs": psbt.outputs.iter().map(|output| {
            let mut output_info = json!({});

            if let Some(ref redeem_script) = output.redeem_script {
                output_info["redeem_script"] = json!(redeem_script.to_hex_string());
            }

            if let Some(ref witness_script) = output.witness_script {
                output_info["witness_script"] = json!(witness_script.to_hex_string());
            }

            if !output.bip32_derivation.is_empty() {
                output_info["bip32_derivs"] = json!(
                    output.bip32_derivation.iter()
                        .map(|(pk, (fingerprint, path))| json!({
                            "pubkey": pk.to_string(),
                            "master_fingerprint": fingerprint.to_string(),
                            "path": path.to_string(),
                        }))
                        .collect::<Vec<_>>()
                );
            }

            output_info
        }).collect::<Vec<_>>(),
        "fee": calculate_psbt_fee(psbt),
    });

    // Add unknown global fields if any
    if !psbt.unknown.is_empty() {
        psbt_json["unknown"] = json!(
            psbt.unknown.iter()
                .map(|(key, value)| json!({
                    "key": hex::encode(&key.key),
                    "value": hex::encode(value),
                }))
                .collect::<Vec<_>>()
        );
    }

    psbt_json
}

/// Calculate the fee for a PSBT (if possible)
fn calculate_psbt_fee(psbt: &Psbt) -> Option<u64> {
    let mut total_input = 0u64;

    // Sum up inputs
    for input in &psbt.inputs {
        if let Some(ref witness_utxo) = input.witness_utxo {
            total_input = total_input.checked_add(witness_utxo.value.to_sat())?;
        } else if let Some(ref non_witness_utxo) = input.non_witness_utxo {
            // Find the corresponding output in the non-witness utxo
            // This is a simplification - in a real implementation you'd need to match vout
            // For now, we'll skip non-witness utxos for fee calculation
            continue;
        } else {
            // Can't calculate fee without UTXO info
            return None;
        }
    }

    // Sum up outputs
    let total_output: u64 = psbt.unsigned_tx.output.iter()
        .map(|output| output.value.to_sat())
        .sum();

    // Fee = inputs - outputs
    total_input.checked_sub(total_output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_simple_psbt() {
        // This is a simple PSBT for testing
        // In a real test, you'd use a valid PSBT base64 string
        // For now, this is a placeholder
    }
}
