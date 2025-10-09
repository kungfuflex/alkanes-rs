//! Transaction construction and signing functionality
//!
//! This module provides comprehensive transaction functionality including:
//! - Transaction construction
//! - Fee estimation and validation
//! - Transaction signing
//! - PSBT (Partially Signed Bitcoin Transaction) support
//! - Envelope and cellpack patterns for alkanes

use crate::{Result, AlkanesError};
use alloc::{string::{String, ToString}, vec::Vec, str::FromStr, format};
use crate::traits::*;
use bitcoin::{Transaction, TxOut, TxIn, OutPoint, ScriptBuf, Witness, Amount, Address};
use serde::{Deserialize, Serialize};


/// Transaction constructor that works with any provider
pub struct TransactionConstructor<P: AlkanesProvider> {
    provider: P,
}

impl<P: AlkanesProvider> TransactionConstructor<P> {
    /// Create a new transaction constructor
    pub fn new(provider: P) -> Self {
        Self { provider }
    }
    
    /// Create a simple send transaction
    pub async fn create_send_transaction(&self, params: SendTransactionParams) -> Result<Transaction> {
        // Get UTXOs for the transaction
        let utxos_with_outpoints = self.select_utxos(&params).await?;
        
        // Calculate fees
        let fee_rate = params.fee_rate.unwrap_or(1.0);
        let utxos: Vec<UtxoInfo> = utxos_with_outpoints.iter().map(|(_, info)| info.clone()).collect();
        let estimated_size = self.estimate_transaction_size(&utxos, &params.outputs)?;
        let fee = (estimated_size as f32 * fee_rate) as u64;
        
        // Build transaction
        let mut tx = Transaction {
            version: bitcoin::transaction::Version::TWO,
            lock_time: bitcoin::locktime::absolute::LockTime::ZERO,
            input: Vec::new(),
            output: Vec::new(),
        };
        
        // Add inputs
        let mut _total_input = 0u64;
        for (outpoint, utxo) in &utxos_with_outpoints {
            tx.input.push(TxIn {
                previous_output: *outpoint,
                script_sig: ScriptBuf::new(),
                sequence: bitcoin::Sequence::ENABLE_RBF_NO_LOCKTIME,
                witness: Witness::new(),
            });
            _total_input += utxo.amount;
        }
        
        // Add outputs
        let mut total_output = 0u64;
        for output in &params.outputs {
            tx.output.push(TxOut {
                value: Amount::from_sat(output.amount),
                script_pubkey: output.script_pubkey.clone(),
            });
            total_output += output.amount;
        }
        
        // Add change output if needed
        if _total_input > total_output + fee {
            let change_amount = _total_input - total_output - fee;
            if change_amount >= 546 { // Dust threshold
                let change_script = self.get_change_script(&params).await?;
                tx.output.push(TxOut {
                    value: Amount::from_sat(change_amount),
                    script_pubkey: change_script,
                });
            }
        }
        
        Ok(tx)
    }
    
    /// Create an envelope transaction for alkanes
    pub async fn create_envelope_transaction(&self, params: EnvelopeTransactionParams) -> Result<Transaction> {
        let mut tx = Transaction {
            version: bitcoin::transaction::Version::TWO,
            lock_time: bitcoin::locktime::absolute::LockTime::ZERO,
            input: Vec::new(),
            output: Vec::new(),
        };
        
        // Add inputs
        let mut total_input = 0u64;
        for utxo in &params.utxos {
            tx.input.push(TxIn {
                previous_output: OutPoint {
                    txid: utxo.txid.parse().map_err(|_| AlkanesError::Parse("Invalid TXID".to_string()))?,
                    vout: utxo.vout,
                },
                script_sig: ScriptBuf::new(),
                sequence: bitcoin::Sequence::ENABLE_RBF_NO_LOCKTIME,
                witness: Witness::new(),
            });
            total_input += utxo.amount;
        }
        
        // Add envelope output (OP_RETURN with envelope data)
        if let Some(envelope_data) = &params.envelope_data {
            let mut script = ScriptBuf::new();
            script.push_opcode(bitcoin::opcodes::all::OP_RETURN);
            script.push_slice(bitcoin::script::PushBytesBuf::try_from(envelope_data.clone()).unwrap().as_push_bytes());
            
            tx.output.push(TxOut {
                value: Amount::ZERO,
                script_pubkey: script,
            });
        }
        
        // Add recipient outputs
        let mut total_output = 0u64;
        for output in &params.outputs {
            tx.output.push(TxOut {
                value: Amount::from_sat(output.amount),
                script_pubkey: output.script_pubkey.clone(),
            });
            total_output += output.amount;
        }
        
        // Add change output
        let fee = params.fee.unwrap_or(1000); // Default fee
        if total_input > total_output + fee {
            let change_amount = total_input - total_output - fee;
            if change_amount >= 546 {
                tx.output.push(TxOut {
                    value: Amount::from_sat(change_amount),
                    script_pubkey: params.change_script.clone(),
                });
            }
        }
        
        Ok(tx)
    }
    
    /// Create a cellpack transaction for alkanes execution
    pub async fn create_cellpack_transaction(&self, params: CellpackTransactionParams) -> Result<Transaction> {
        let mut tx = Transaction {
            version: bitcoin::transaction::Version::TWO,
            lock_time: bitcoin::locktime::absolute::LockTime::ZERO,
            input: Vec::new(),
            output: Vec::new(),
        };
        
        // Add inputs
        let mut _total_input = 0u64;
        for utxo in &params.utxos {
            tx.input.push(TxIn {
                previous_output: OutPoint {
                    txid: utxo.txid.parse().map_err(|_| AlkanesError::Parse("Invalid TXID".to_string()))?,
                    vout: utxo.vout,
                },
                script_sig: ScriptBuf::new(),
                sequence: bitcoin::Sequence::ENABLE_RBF_NO_LOCKTIME,
                witness: Witness::new(),
            });
            _total_input += utxo.amount;
        }
        
        // Add cellpack output with witness data
        for output in &params.outputs {
            tx.output.push(TxOut {
                value: Amount::from_sat(output.amount),
                script_pubkey: output.script_pubkey.clone(),
            });
        }
        
        // Add cellpack data to witness of first input if present
        if !params.cellpack_data.is_empty() && !tx.input.is_empty() {
            let mut witness = Witness::new();
            witness.push(&params.cellpack_data);
            tx.input[0].witness = witness;
        }
        
        Ok(tx)
    }
    
    /// Estimate transaction size in bytes
    pub fn estimate_transaction_size(&self, utxos: &[UtxoInfo], outputs: &[TransactionOutput]) -> Result<usize> {
        // Base transaction size
        let mut size = 10; // version (4) + input count (1) + output count (1) + locktime (4)
        
        // Input sizes (varies by script type)
        for utxo in utxos {
            size += 32; // previous output hash
            size += 4;  // previous output index
            size += 4;  // sequence
            
            // Script sig size (depends on address type)
            if utxo.address.starts_with("bc1") || utxo.address.starts_with("tb1") || utxo.address.starts_with("bcrt1") {
                // SegWit input
                size += 1; // empty script sig
                size += 27; // witness data (approximate)
            } else {
                // Legacy input
                size += 107; // script sig (approximate)
            }
        }
        
        // Output sizes
        for output in outputs {
            size += 8; // value
            size += 1; // script length
            size += output.script_pubkey.len();
        }
        
        // Add change output estimate
        size += 34; // typical P2WPKH output
        
        Ok(size)
    }
    
    /// Select UTXOs for transaction
    async fn select_utxos(&self, params: &SendTransactionParams) -> Result<Vec<(OutPoint, UtxoInfo)>> {
        let available_utxos = self.provider.get_utxos(false, params.from_addresses.clone()).await?;
        
        let total_needed = params.outputs.iter().map(|o| o.amount).sum::<u64>() + 1000; // Add fee estimate
        
        // Simple UTXO selection (largest first)
        let mut selected = Vec::new();
        let mut total_selected = 0u64;
        
        let mut sorted_utxos = available_utxos;
        sorted_utxos.sort_by(|a, b| b.1.amount.cmp(&a.1.amount));
        
        for (outpoint, utxo) in sorted_utxos {
            if utxo.frozen {
                continue;
            }
            
            selected.push((outpoint, utxo.clone()));
            total_selected += utxo.amount;
            
            if total_selected >= total_needed {
                break;
            }
        }
        
        if total_selected < total_needed {
            return Err(AlkanesError::Transaction("Insufficient funds".to_string()));
        }
        
        Ok(selected)
    }
    
    /// Get change script
    async fn get_change_script(&self, params: &SendTransactionParams) -> Result<ScriptBuf> {
        if let Some(change_address) = &params.change_address {
            // Parse change address to script
            let network = self.provider.get_network();
            let address = Address::from_str(change_address)
                .map_err(|e| AlkanesError::AddressResolution(e.to_string()))?
                .require_network(network)
                .map_err(|e| AlkanesError::AddressResolution(e.to_string()))?;
            Ok(address.script_pubkey())
        } else {
            // Use default wallet address
            let address_str = WalletProvider::get_address(&self.provider).await?;
            let network = self.provider.get_network();
            let address = Address::from_str(&address_str)
                .map_err(|e| AlkanesError::AddressResolution(e.to_string()))?
                .require_network(network)
                .map_err(|e| AlkanesError::AddressResolution(e.to_string()))?;
            Ok(address.script_pubkey())
        }
    }
    
    /// Sign transaction
    pub async fn sign_transaction(&mut self, tx: Transaction) -> Result<Transaction> {
        let tx_hex = bitcoin::consensus::encode::serialize_hex(&tx);
        let signed_hex = self.provider.sign_transaction(tx_hex).await?;
        
        let signed_bytes = hex::decode(signed_hex)
            .map_err(|e| AlkanesError::Parse(format!("Invalid signed transaction hex: {e}")))?;
        
        bitcoin::consensus::encode::deserialize(&signed_bytes)
            .map_err(|e| AlkanesError::Transaction(format!("Failed to deserialize signed transaction: {e}")))
    }
    
    /// Broadcast transaction
    pub async fn broadcast_transaction(&self, tx: &Transaction) -> Result<String> {
        let tx_hex = bitcoin::consensus::encode::serialize_hex(tx);
        self.provider.broadcast_transaction(tx_hex).await
    }
}

/// Send transaction parameters
#[derive(Debug, Clone)]
pub struct SendTransactionParams {
    pub outputs: Vec<TransactionOutput>,
    pub fee_rate: Option<f32>,
    pub from_addresses: Option<Vec<String>>,
    pub change_address: Option<String>,
}

/// Transaction output
#[derive(Debug, Clone)]
pub struct TransactionOutput {
    pub amount: u64,
    pub script_pubkey: ScriptBuf,
}

/// Envelope transaction parameters
#[derive(Debug, Clone)]
pub struct EnvelopeTransactionParams {
    pub utxos: Vec<crate::traits::UtxoInfo>,
    pub outputs: Vec<TransactionOutput>,
    pub envelope_data: Option<Vec<u8>>,
    pub change_script: ScriptBuf,
    pub fee: Option<u64>,
}

/// Cellpack transaction parameters
#[derive(Debug, Clone)]
pub struct CellpackTransactionParams {
    pub utxos: Vec<UtxoInfo>,
    pub outputs: Vec<TransactionOutput>,
    pub cellpack_data: Vec<u8>,
}

/// Fee validation utilities
pub mod fee_validation {
    use super::*;
    
    /// Validate transaction fee
    pub fn validate_fee(tx: &Transaction, fee_rate: f32, utxos: &[UtxoInfo]) -> Result<()> {
        let tx_size = bitcoin::consensus::encode::serialize(tx).len();
        let calculated_fee = (tx_size as f32 * fee_rate) as u64;
        
        let total_input: u64 = utxos.iter().map(|u| u.amount).sum();
        let total_output: u64 = tx.output.iter().map(|o| o.value.to_sat()).sum();
        let actual_fee = total_input.saturating_sub(total_output);
        
        // Check if fee is reasonable (not too high or too low)
        let min_fee = calculated_fee / 2; // Allow 50% below calculated
        let max_fee = calculated_fee * 10; // Allow 10x above calculated
        
        if actual_fee < min_fee {
            return Err(AlkanesError::Transaction(format!(
                "Fee too low: {actual_fee} sats (minimum: {min_fee} sats)"
            )));
        }
        
        if actual_fee > max_fee {
            return Err(AlkanesError::Transaction(format!(
                "Fee too high: {actual_fee} sats (maximum: {max_fee} sats)"
            )));
        }
        
        Ok(())
    }
    
    /// Calculate recommended fee
    pub fn calculate_recommended_fee(tx_size: usize, fee_rate: f32) -> u64 {
        (tx_size as f32 * fee_rate) as u64
    }
    
    /// Get fee rate recommendations
    pub fn get_fee_rate_recommendations() -> FeeRateRecommendations {
        FeeRateRecommendations {
            fast: 20.0,    // ~1 block
            medium: 10.0,  // ~3 blocks
            slow: 5.0,     // ~6 blocks
            minimum: 1.0,  // Minimum relay fee
        }
    }
}

/// Fee rate recommendations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeRateRecommendations {
    pub fast: f32,
    pub medium: f32,
    pub slow: f32,
    pub minimum: f32,
}

/// Transaction analysis utilities
pub mod analysis {
    use super::*;
    
    /// Analyze transaction
    pub fn analyze_transaction(tx: &Transaction) -> TransactionAnalysis {
        let mut analysis = TransactionAnalysis {
            txid: tx.compute_txid().to_string(),
            size: bitcoin::consensus::encode::serialize(tx).len(),
            weight: tx.weight().to_wu() as usize,
            input_count: tx.input.len(),
            output_count: tx.output.len(),
            total_input_value: 0,
            total_output_value: tx.output.iter().map(|o| o.value.to_sat()).sum(),
            fee: 0,
            fee_rate: 0.0,
            has_witness: false,
            has_op_return: false,
            op_return_data: Vec::new(),
        };
        
        // Check for witness data
        analysis.has_witness = tx.input.iter().any(|input| !input.witness.is_empty());
        
        // Check for OP_RETURN outputs
        for output in &tx.output {
            if output.script_pubkey.is_op_return() {
                analysis.has_op_return = true;
                // Extract OP_RETURN data
                let script_bytes = output.script_pubkey.as_bytes();
                if script_bytes.len() > 2 && script_bytes[0] == 0x6a {
                    let data_len = script_bytes[1] as usize;
                    if script_bytes.len() >= 2 + data_len {
                        analysis.op_return_data.push(script_bytes[2..2 + data_len].to_vec());
                    }
                }
            }
        }
        
        analysis
    }
    
    /// Check if transaction is RBF (Replace-By-Fee) enabled
    pub fn is_rbf_enabled(tx: &Transaction) -> bool {
        tx.input.iter().any(|input| input.sequence.is_rbf())
    }
    
    /// Check if transaction is a coinbase transaction
    pub fn is_coinbase(tx: &Transaction) -> bool {
        tx.input.len() == 1 && tx.input[0].previous_output.is_null()
    }
}

/// Transaction analysis result
use alkanes_pretty_print_macro::PrettyPrint;

#[derive(Debug, Clone, Serialize, Deserialize, PrettyPrint)]
pub struct TransactionAnalysis {
    pub txid: String,
    pub size: usize,
    pub weight: usize,
    pub input_count: usize,
    pub output_count: usize,
    pub total_input_value: u64,
    pub total_output_value: u64,
    pub fee: u64,
    pub fee_rate: f32,
    pub has_witness: bool,
    pub has_op_return: bool,
    pub op_return_data: Vec<Vec<u8>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use bitcoin::{Amount, ScriptBuf};
    
    #[test]
    fn test_fee_validation() {
        let tx = Transaction {
            version: bitcoin::transaction::Version::TWO,
            lock_time: bitcoin::locktime::absolute::LockTime::ZERO,
            input: vec![],
            output: vec![
                bitcoin::TxOut {
                    value: Amount::from_sat(100000),
                    script_pubkey: ScriptBuf::new(),
                }
            ],
        };
        
        let utxos = vec![
            UtxoInfo {
                txid: "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
                vout: 0,
                amount: 101000,
                address: "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4".to_string(),
                script_pubkey: Some(ScriptBuf::new()),
                confirmations: 6,
                frozen: false,
                freeze_reason: None,
                block_height: Some(100),
                has_inscriptions: false,
                has_runes: false,
                has_alkanes: false,
                is_coinbase: false,
            }
        ];
        
        // This should pass with reasonable fee
        assert!(fee_validation::validate_fee(&tx, 10.0, &utxos).is_ok());
    }
    
    #[test]
    fn test_transaction_analysis() {
        let tx = Transaction {
            version: bitcoin::transaction::Version::TWO,
            lock_time: bitcoin::locktime::absolute::LockTime::ZERO,
            input: vec![],
            output: vec![
                bitcoin::TxOut {
                    value: Amount::from_sat(100000),
                    script_pubkey: ScriptBuf::new(),
                }
            ],
        };
        
        let analysis = analysis::analyze_transaction(&tx);
        assert_eq!(analysis.output_count, 1);
        assert_eq!(analysis.total_output_value, 100000);
        assert!(!analysis.has_witness);
        assert!(!analysis.has_op_return);
    }
    
    #[test]
    fn test_fee_rate_recommendations() {
        let recommendations = fee_validation::get_fee_rate_recommendations();
        assert!(recommendations.fast > recommendations.medium);
        assert!(recommendations.medium > recommendations.slow);
        assert!(recommendations.slow >= recommendations.minimum);
    }
}