//! Alkanes implementation of MetaprotocolUnwrap
//!
//! Uses metashrew_view RPC to query the alkanes indexer for pending unwraps

use crate::{AlkanesError, Result};
use crate::alkanes::PendingUnwrap;
use crate::traits::{DeezelProvider, MetashrewRpcProvider, JsonRpcProvider};
use crate::unwrap::MetaprotocolUnwrap;
use async_trait::async_trait;
use bitcoin::consensus::Decodable;
use bitcoin::hashes::Hash;
use prost::Message;
use std::io::Cursor;

/// Alkanes unwrap implementation using metashrew_view
pub struct AlkanesUnwrap {
    // No state needed for now, but we keep the struct for future extensibility
}

impl AlkanesUnwrap {
    /// Create a new AlkanesUnwrap instance
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for AlkanesUnwrap {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait(?Send)]
impl MetaprotocolUnwrap for AlkanesUnwrap {
    async fn get_pending_unwraps(
        &self,
        provider: &dyn DeezelProvider,
        confirmations_required: u64,
    ) -> Result<Vec<PendingUnwrap>> {
        log::info!("[AlkanesUnwrap] Fetching unwraps with {} confirmations required", confirmations_required);
        
        // Get current height
        let current_height = provider.get_height().await?;
        
        // Calculate query height based on confirmations
        let query_height = if current_height >= confirmations_required {
            current_height - confirmations_required
        } else {
            log::warn!("[AlkanesUnwrap] Current height {} is less than confirmations_required {}, using 0",
                current_height, confirmations_required);
            0
        };
        
        log::info!("[AlkanesUnwrap] Querying unwraps at height {} (current: {}, confirmations: {})",
            query_height, current_height, confirmations_required);
        
        // Call metashrew_view with the alkanes indexer format
        let params = serde_json::json!(["unwrap", "0x", query_height]);
        let response = provider.call(
            &provider.get_metashrew_rpc_url().ok_or_else(|| {
                AlkanesError::Configuration("metashrew_rpc_url not configured".to_string())
            })?,
            "metashrew_view",
            params,
            1,
        ).await?;
        
        // Parse the hex response
        let hex_data = response.as_str()
            .ok_or_else(|| AlkanesError::RpcError("metashrew_view result is not a string".to_string()))?;
        let hex_data_stripped = hex_data.strip_prefix("0x").unwrap_or(hex_data);
        let response_bytes = hex::decode(hex_data_stripped)
            .map_err(|e| AlkanesError::RpcError(format!("Failed to decode hex response: {}", e)))?;
        
        if response_bytes.is_empty() {
            log::info!("[AlkanesUnwrap] No pending unwraps found in indexer");
            return Ok(vec![]);
        }
        
        // Decode the protobuf response
        let unwraps_response = crate::proto::alkanes::PendingUnwrapsResponse::decode(response_bytes.as_slice())?;
        
        if unwraps_response.payments.is_empty() {
            log::info!("[AlkanesUnwrap] No payments in unwraps response");
            return Ok(vec![]);
        }
        
        log::info!("[AlkanesUnwrap] Found {} unwraps from indexer", unwraps_response.payments.len());
        
        // Convert proto payments to PendingUnwrap structs (no filtering here)
        let mut result = Vec::new();
        for payment in unwraps_response.payments {
            let spendable = payment.spendable.ok_or_else(|| {
                AlkanesError::RpcError("Payment missing spendable field".to_string())
            })?;
            
            let txid_bytes = spendable.txid.clone();
            let txid = bitcoin::Txid::from_byte_array(
                txid_bytes.try_into()
                    .map_err(|_| AlkanesError::RpcError("Invalid txid length in spendable".to_string()))?
            );
            let vout = spendable.vout;
            
            // Decode the TxOut from the output bytes
            let mut cursor = Cursor::new(payment.output);
            let tx_out = bitcoin::TxOut::consensus_decode(&mut cursor)
                .map_err(|e| AlkanesError::RpcError(format!("Failed to decode TxOut: {}", e)))?;
            
            let amount = tx_out.value.to_sat();
            
            // Try to extract address from script_pubkey
            let network = match provider.get_network() {
                bitcoin::Network::Bitcoin => bitcoin::Network::Bitcoin,
                bitcoin::Network::Testnet => bitcoin::Network::Testnet,
                bitcoin::Network::Signet => bitcoin::Network::Signet,
                bitcoin::Network::Regtest => bitcoin::Network::Regtest,
                _ => bitcoin::Network::Regtest,
            };
            
            let address = bitcoin::Address::from_script(&tx_out.script_pubkey, network)
                .ok()
                .map(|a| a.to_string());
            
            result.push(PendingUnwrap {
                txid: txid.to_string(),
                vout,
                amount,
                address,
                fulfilled: payment.fulfilled,
            });
        }
        
        log::info!("[AlkanesUnwrap] Returning {} unfiltered unwraps", result.len());
        Ok(result)
    }
    
    fn protocol_name(&self) -> &'static str {
        "alkanes"
    }
}
