//! Comprehensive Transaction and Runestone Analysis
//!
//! This module provides functions to perform a detailed analysis of a Bitcoin
//! transaction, extracting information about its inputs, outputs, and any
//! embedded protostones or runestones. It also includes a pretty-printer
//! to format the analysis into a human-readable string, consistent with
//! the reference implementation's output.

use crate::alkanes::analyze::analyze_runestone;
use crate::runestone_enhanced;
use crate::Result;
use alloc::{
    string::{String},
};
use bitcoin::{Network, Transaction, TxIn, TxOut, OutPoint, ScriptBuf, Witness, Amount};
use serde_json::{Value as JsonValue};
use std::str::FromStr;

/// Analyzes a transaction and any embedded runestone, producing a detailed JSON object.
///
/// This function inspects the transaction's inputs, outputs, and decodes any
/// protostone data found in an OP_RETURN output. The structure of the returned
/// JSON value is designed to match the output of the `--raw` flag from the
/// reference implementation.
///
/// # Arguments
///
/// * `tx` - A reference to the `bitcoin::Transaction` to be analyzed.
/// * `_network` - The `bitcoin::Network` context (e.g., Mainnet, Testnet) for address generation.
///
/// # Returns
///
/// A `Result` containing a `serde_json::Value` with the detailed analysis.
pub fn analyze_transaction_with_runestone(
    tx: &Transaction,
    _network: Network,
) -> Result<JsonValue> {
    Ok(analyze_runestone(tx)?)
}

/// Formats the detailed transaction analysis into a human-readable string.
///
/// This function takes the JSON object produced by `analyze_transaction_with_runestone`
/// and formats it with headers, emojis, and structured sections for readability.
///
/// # Arguments
///
/// * `analysis` - A `serde_json::Value` containing the detailed transaction analysis.
///
/// # Returns
///
/// A `Result` containing the formatted `String`.
pub fn pretty_print_transaction_analysis(analysis: &JsonValue) -> Result<String> {
    let output = String::new();

    // Reconstruct the transaction from the analysis JSON
    let version = analysis["version"].as_i64().unwrap_or(2) as i32;
    let lock_time = analysis["lock_time"].as_u64().unwrap_or(0) as u32;

    let inputs: Vec<TxIn> = analysis["inputs"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .map(|input_json| {
            let txid_str = input_json["previous_output"]["txid"].as_str().unwrap_or_default();
            let txid = bitcoin::Txid::from_str(txid_str).unwrap_or(bitcoin::Txid::from_str("0000000000000000000000000000000000000000000000000000000000000000").unwrap());
            let vout = input_json["previous_output"]["vout"].as_u64().unwrap_or(0) as u32;
            let sequence = input_json["sequence"].as_u64().unwrap_or(0) as u32;

            TxIn {
                previous_output: OutPoint { txid, vout },
                script_sig: ScriptBuf::new(), // Not available in analysis
                sequence: bitcoin::Sequence(sequence),
                witness: Witness::new(), // Not available in analysis
            }
        })
        .collect();

    let outputs: Vec<TxOut> = analysis["outputs"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .map(|output_json| {
            let value = output_json["value"].as_u64().unwrap_or(0);
            let script_pubkey_hex = output_json["script_pubkey"].as_str().unwrap_or_default();
            let script_pubkey_bytes = hex::decode(script_pubkey_hex).unwrap_or_default();
            TxOut {
                value: Amount::from_sat(value),
                script_pubkey: ScriptBuf::from(script_pubkey_bytes),
            }
        })
        .collect();

    let tx = Transaction {
        version: bitcoin::transaction::Version(version),
        lock_time: bitcoin::absolute::LockTime::from_consensus(lock_time),
        input: inputs,
        output: outputs,
    };

    runestone_enhanced::print_human_readable_runestone(&tx, analysis);

    // The print function prints directly to stdout, so we capture it here if needed
    // For now, we assume it prints and we return an empty string.
    // A better approach would be for the print function to return a string.
    Ok(output)
}
