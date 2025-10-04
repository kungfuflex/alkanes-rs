//! Utility functions for the deezel-sys library

use anyhow::{anyhow, Result};
use deezel_common::provider::ConcreteProvider;
use deezel_common::traits::AddressResolver;
use std::str::FromStr;

/// Resolve a single address identifier string (e.g., "p2tr:0" or a concrete address)
pub async fn resolve_address_identifiers(input: &str, provider: &ConcreteProvider) -> Result<String> {
    // If it's not a valid address, assume it's an identifier
    if bitcoin::Address::from_str(input).is_err() {
        // Convert shorthand to full format and resolve
        let full_identifier = format!("[self:{input}]");
        return provider.resolve_all_identifiers(&full_identifier).await.map_err(|e| anyhow!("{}", e));
    }
    
    // No identifiers found, return as-is
    Ok(input.to_string())
}

use anyhow::Context;
use bitcoin::{consensus::deserialize, Transaction};
use deezel_common::traits::RunestoneProvider;

/// Decode a transaction from hex
pub fn decode_transaction_hex(hex_str: &str) -> Result<Transaction> {
    let tx_bytes = hex::decode(hex_str.trim_start_matches("0x"))
        .context("Failed to decode transaction hex")?;
    
    let tx: Transaction = deserialize(&tx_bytes)
        .context("Failed to deserialize transaction")?;
    
    Ok(tx)
}

/// Analyze a transaction for Runestone data
pub async fn analyze_runestone_tx(tx: &Transaction, raw_output: bool, provider: &ConcreteProvider) -> Result<()> {
    // Use the enhanced format_runestone_with_decoded_messages function
    match provider.format_runestone_with_decoded_messages(tx).await {
        Ok(result) => {
            if raw_output {
                // Raw JSON output for scripting
                println!("{}", serde_json::to_string_pretty(&result).unwrap_or_else(|_| "Error formatting result".to_string()));
            } else {
                // Human-readable styled output
                print_human_readable_runestone(tx, &result);
            }
        },
        Err(e) => {
            if raw_output {
                eprintln!("Error decoding runestone: {e}");
            } else {
                println!("❌ Error decoding runestone: {e}");
            }
        }
    }
    Ok(())
}

/// Print human-readable runestone information
pub fn print_human_readable_runestone(tx: &Transaction, result: &serde_json::Value) {
    println!("🪨 Runestone Analysis");
    println!("═══════════════════");
    println!("🔗 Transaction: {}", tx.compute_txid());
    
    if let Some(runestone) = result.get("runestone") {
        if let Some(edicts) = runestone.get("edicts") {
            if let Some(edicts_array) = edicts.as_array() {
                if !edicts_array.is_empty() {
                    println!("📜 Edicts: {} found", edicts_array.len());
                    for (i, edict) in edicts_array.iter().enumerate() {
                        println!("  {}. {}", i + 1, serde_json::to_string_pretty(edict).unwrap_or_default());
                    }
                }
            }
        }
        
        if let Some(etching) = runestone.get("etching") {
            println!("🎨 Etching: {}", serde_json::to_string_pretty(etching).unwrap_or_default());
        }
        
        if let Some(mint) = runestone.get("mint") {
            println!("🪙 Mint: {}", serde_json::to_string_pretty(mint).unwrap_or_default());
        }
    }
    
    if let Some(decoded_messages) = result.get("decoded_messages") {
        println!("📋 Decoded Messages: {}", serde_json::to_string_pretty(decoded_messages).unwrap_or_default());
    }
}

/// Parse outpoint from string (format: txid:vout)
pub fn parse_outpoint(outpoint: &str) -> Result<(String, u32)> {
    let parts: Vec<&str> = outpoint.split(':').collect();
    if parts.len() != 2 {
        return Err(anyhow!("Invalid outpoint format. Expected 'txid:vout'"));
    }
    
    let txid = parts[0].to_string();
    let vout = parts[1].parse::<u32>()
        .context("Invalid vout in outpoint")?;
    
    Ok((txid, vout))
}

/// Parse contract ID from string (format: txid:vout)
#[allow(dead_code)]
pub fn parse_contract_id(contract_id: &str) -> Result<(String, String)> {
    let parts: Vec<&str> = contract_id.split(':').collect();
    if parts.len() != 2 {
        return Err(anyhow!("Invalid contract ID format. Expected 'txid:vout'"));
    }
    
    Ok((parts[0].to_string(), parts[1].to_string()))
}