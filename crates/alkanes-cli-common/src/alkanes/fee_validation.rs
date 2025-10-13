//! Fee validation module for alkanes transactions
//!
//! This module provides fee rate validation to prevent "absurdly high fee rate" errors
//! from Bitcoin Core when broadcasting transactions with large witness data.

use anyhow::{anyhow, Result};
use bitcoin::Transaction;
use log::{info, warn};

use crate::{ToString, format};

#[cfg(not(target_arch = "wasm32"))]
use std::{vec::Vec, string::String};
#[cfg(target_arch = "wasm32")]
use alloc::{vec::Vec, string::String};

// Conditional print macros for WASM compatibility
#[cfg(target_arch = "wasm32")]
macro_rules! eprintln {
    ($($arg:tt)*) => {
        // In WASM, we can use web_sys::console::log or just ignore
        // For now, we'll just ignore the output
    };
}


/// Maximum allowed fee rate in sat/vB (1000 sat/vB = ~$40 at $40k BTC)
const MAX_FEE_RATE_SAT_VB: f64 = 1000.0;

/// Maximum allowed absolute fee in satoshis (0.01 BTC = 1,000,000 sats)
const MAX_ABSOLUTE_FEE_SATS: u64 = 1_000_000;

/// Fee analysis result
#[derive(Debug, Clone)]
pub struct FeeAnalysis {
    pub total_input_value: u64,
    pub total_output_value: u64,
    pub calculated_fee: u64,
    pub vsize: usize,
    pub weight: usize,
    pub fee_rate_sat_vb: f64,
    pub is_valid: bool,
    pub validation_errors: Vec<String>,
}

/// Validate transaction fee rate before broadcasting
pub fn validate_transaction_fee_rate(
    tx: &Transaction,
    input_values: &[u64],
) -> Result<FeeAnalysis> {
    eprintln!("🔍 VALIDATING TRANSACTION FEE RATE BEFORE BROADCAST");
    eprintln!("═══════════════════════════════════════════════════");
    
    // DEBUG: Dump complete transaction details before fee calculation
    eprintln!("🔍 TRANSACTION DEBUG DUMP:");
    eprintln!("  Transaction ID: {}", tx.compute_txid());
    eprintln!("  Version: {}", tx.version);
    eprintln!("  Lock Time: {}", tx.lock_time);
    eprintln!("  Transaction Hex: {}", hex::encode(bitcoin::consensus::serialize(tx)));
    
    // DEBUG: Dump all inputs with their values
    eprintln!("  📥 INPUTS ({}):", tx.input.len());
    for (i, input) in tx.input.iter().enumerate() {
        let input_value = input_values.get(i).copied().unwrap_or(0);
        eprintln!("    Input {}: {}:{} = {} sats", i, input.previous_output.txid, input.previous_output.vout, input_value);
        eprintln!("      Script Sig: {} bytes", input.script_sig.len());
        eprintln!("      Witness: {} items, {} bytes total",
              input.witness.len(),
              input.witness.iter().map(|item| item.len()).sum::<usize>());
        if !input.witness.is_empty() {
            for (j, witness_item) in input.witness.iter().enumerate() {
                eprintln!("        Witness item {}: {} bytes", j, witness_item.len());
                if witness_item.len() > 1000 {
                    eprintln!("        ⚠️  LARGE WITNESS ITEM DETECTED! This will significantly increase transaction size");
                }
            }
        }
        eprintln!("      Sequence: {}", input.sequence);
    }
    
    // DEBUG: Dump all outputs with their values
    eprintln!("  📤 OUTPUTS ({}):", tx.output.len());
    for (i, output) in tx.output.iter().enumerate() {
        eprintln!("    Output {}: {} sats", i, output.value.to_sat());
        eprintln!("      Script: {} bytes", output.script_pubkey.len());
        if output.script_pubkey.is_op_return() {
            eprintln!("      Type: OP_RETURN");
            if output.script_pubkey.len() > 100 {
                eprintln!("      ⚠️  LARGE OP_RETURN DETECTED! {} bytes", output.script_pubkey.len());
            }
        } else {
            eprintln!("      Type: Regular output");
        }
    }
    
    // DEBUG: Dump transaction size metrics
    eprintln!("  📊 SIZE METRICS:");
    eprintln!("    Base Size: {} bytes", tx.base_size());
    eprintln!("    Total Size: {} bytes", tx.total_size());
    eprintln!("    Weight: {} WU", tx.weight());
    eprintln!("    VSize: {} vbytes", tx.vsize());
    
    // Calculate witness data size breakdown
    let total_witness_size: usize = tx.input.iter()
        .map(|input| input.witness.iter().map(|item| item.len()).sum::<usize>())
        .sum();
    eprintln!("    Total Witness Data: {} bytes", total_witness_size);
    if total_witness_size > 50_000 {
        eprintln!("    ⚠️  EXTREMELY LARGE WITNESS DATA! This is likely causing the high fee rate");
    }
    
    // Calculate total input and output values
    let total_input_value: u64 = input_values.iter().sum();
    let total_output_value: u64 = tx.output.iter().map(|out| out.value.to_sat()).sum();
    
    // DEBUG: Show the calculation breakdown
    eprintln!("  💰 VALUE CALCULATION:");
    eprintln!("    Total Input Value: {} sats", total_input_value);
    eprintln!("    Total Output Value: {} sats", total_output_value);
    eprintln!("    Calculated Fee: {} sats", total_input_value.saturating_sub(total_output_value));
    
    // Calculate fee
    let calculated_fee = total_input_value.saturating_sub(total_output_value);
    
    // Get transaction size metrics
    let vsize = tx.vsize();
    let weight = tx.weight().to_wu() as usize;
    
    // Calculate fee rate
    let fee_rate_sat_vb = if vsize > 0 {
        calculated_fee as f64 / vsize as f64
    } else {
        0.0
    };
    
    // DEBUG: Show the final fee rate calculation
    eprintln!("  🧮 FEE RATE CALCULATION:");
    eprintln!("    Fee Rate: {:.2} sat/vB ({} sats ÷ {} vbytes)", fee_rate_sat_vb, calculated_fee, vsize);
    eprintln!("    Maximum Allowed: {:.2} sat/vB", MAX_FEE_RATE_SAT_VB);
    if fee_rate_sat_vb > MAX_FEE_RATE_SAT_VB {
        eprintln!("    ❌ EXCEEDS MAXIMUM BY: {:.2}x", fee_rate_sat_vb / MAX_FEE_RATE_SAT_VB);
    }
    eprintln!("═══════════════════════════════════════════════════");
    
    let mut validation_errors = Vec::new();
    
    // Validation checks
    if calculated_fee > MAX_ABSOLUTE_FEE_SATS {
        validation_errors.push(format!(
            "Absolute fee too high: {} sats > {} sats maximum",
            calculated_fee, MAX_ABSOLUTE_FEE_SATS
        ));
    }
    
    if fee_rate_sat_vb > MAX_FEE_RATE_SAT_VB {
        validation_errors.push(format!(
            "Fee rate too high: {:.2} sat/vB > {:.2} sat/vB maximum",
            fee_rate_sat_vb, MAX_FEE_RATE_SAT_VB
        ));
    }
    
    if total_input_value == 0 {
        validation_errors.push("Total input value is zero - inputs may not be properly fetched".to_string());
    }
    
    if calculated_fee == 0 && total_output_value > 0 {
        validation_errors.push("Calculated fee is zero but outputs exist - possible input/output mismatch".to_string());
    }
    
    let is_valid = validation_errors.is_empty();
    
    // Log analysis results
    info!("💰 Fee Analysis Results:");
    info!("  Total Input Value: {} sats", total_input_value);
    info!("  Total Output Value: {} sats", total_output_value);
    info!("  Calculated Fee: {} sats", calculated_fee);
    info!("  Transaction VSize: {} vbytes", vsize);
    info!("  Transaction Weight: {} WU", weight);
    info!("  Fee Rate: {:.2} sat/vB", fee_rate_sat_vb);
    
    if !is_valid {
        warn!("❌ Fee validation failed:");
        for error in &validation_errors {
            warn!("  - {}", error);
        }
    } else {
        info!("✅ Fee validation passed");
    }
    
    let analysis = FeeAnalysis {
        total_input_value,
        total_output_value,
        calculated_fee,
        vsize,
        weight,
        fee_rate_sat_vb,
        is_valid,
        validation_errors,
    };
    
    Ok(analysis)
}

/// Suggest fee adjustments for invalid transactions
pub fn suggest_fee_adjustments(analysis: &FeeAnalysis) -> Vec<String> {
    let mut suggestions = Vec::new();
    
    if analysis.fee_rate_sat_vb > MAX_FEE_RATE_SAT_VB {
        suggestions.push(format!(
            "Reduce fee to maximum allowed: {} sats (target rate: {:.2} sat/vB)",
            (MAX_FEE_RATE_SAT_VB * analysis.vsize as f64).ceil() as u64,
            MAX_FEE_RATE_SAT_VB
        ));
    }
    
    if analysis.calculated_fee > MAX_ABSOLUTE_FEE_SATS {
        suggestions.push(format!(
            "Reduce absolute fee to maximum: {} sats",
            MAX_ABSOLUTE_FEE_SATS
        ));
    }
    
    if analysis.total_input_value == 0 {
        suggestions.push("Check UTXO selection - ensure input values are properly fetched".to_string());
    }
    
    if analysis.vsize > 100_000 {
        suggestions.push("Consider splitting large transactions or reducing witness data size".to_string());
    }
    
    suggestions
}

/// Create a fee-adjusted transaction by modifying outputs
pub fn create_fee_adjusted_transaction(
    mut tx: Transaction,
    target_fee: u64,
    input_values: &[u64],
) -> Result<Transaction> {
    info!("🔧 Creating fee-adjusted transaction with target fee: {} sats", target_fee);
    
    let total_input_value: u64 = input_values.iter().sum();
    let current_output_value: u64 = tx.output.iter().map(|out| out.value.to_sat()).sum();
    let current_fee = total_input_value.saturating_sub(current_output_value);
    
    if target_fee >= current_fee {
        // Need to reduce output values
        let fee_increase = target_fee - current_fee;
        
        // Find the largest non-OP_RETURN output to adjust
        let mut largest_output_index = None;
        let mut largest_value = 0u64;
        
        for (i, output) in tx.output.iter().enumerate() {
            if !output.script_pubkey.is_op_return() && output.value.to_sat() > largest_value {
                largest_value = output.value.to_sat();
                largest_output_index = Some(i);
            }
        }
        
        if let Some(index) = largest_output_index {
            let new_value = largest_value.saturating_sub(fee_increase);
            if new_value >= 546 { // Dust threshold
                tx.output[index].value = bitcoin::Amount::from_sat(new_value);
                info!("Adjusted output {} from {} to {} sats (fee increase: {} sats)",
                      index, largest_value, new_value, fee_increase);
            } else {
                return Err(anyhow!("Cannot adjust fee: would create dust output"));
            }
        } else {
            return Err(anyhow!("Cannot adjust fee: no suitable outputs found"));
        }
    }
    
    // Validate the adjusted transaction
    let analysis = validate_transaction_fee_rate(&tx, input_values)?;
    if !analysis.is_valid {
        return Err(anyhow!("Fee adjustment failed: {:?}", analysis.validation_errors));
    }
    
    info!("✅ Fee-adjusted transaction created successfully");
    Ok(tx)
}

#[cfg(test)]
mod tests {
    use alloc::vec;
    use super::*;
    use bitcoin::{Transaction, TxOut, Amount, ScriptBuf};
    
    #[test]
    fn test_validate_reasonable_fee_rate() {
        let tx = Transaction {
            version: bitcoin::transaction::Version(2),
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: vec![],
            output: vec![
                TxOut {
                    value: Amount::from_sat(100_000),
                    script_pubkey: ScriptBuf::new(),
                }
            ],
        };
        
        let input_values = vec![105_000]; // 5000 sat fee
        let analysis = validate_transaction_fee_rate(&tx, &input_values).unwrap();
        
        assert!(analysis.is_valid);
        assert_eq!(analysis.calculated_fee, 5_000);
        assert!(analysis.fee_rate_sat_vb < MAX_FEE_RATE_SAT_VB);
    }
    
    #[test]
    fn test_validate_high_fee_rate() {
        let tx = Transaction {
            version: bitcoin::transaction::Version(2),
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: vec![],
            output: vec![
                TxOut {
                    value: Amount::from_sat(1_000),
                    script_pubkey: ScriptBuf::new(),
                }
            ],
        };
        
        let input_values = vec![2_000_000]; // Very high fee
        let analysis = validate_transaction_fee_rate(&tx, &input_values).unwrap();
        
        assert!(!analysis.is_valid);
        assert!(analysis.fee_rate_sat_vb > MAX_FEE_RATE_SAT_VB);
        assert!(!analysis.validation_errors.is_empty());
    }
}