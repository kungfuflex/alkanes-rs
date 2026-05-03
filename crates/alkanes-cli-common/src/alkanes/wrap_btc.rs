// Alkanes wrap-btc functionality for frBTC synthetic Bitcoin
// Uses opcode 77 to call exchange() on frBTC alkane {32, 0}
// Then locks the minted frBTC in vault {4, 3032615708} using opcode 1

use crate::{AlkanesError, DeezelProvider, Result};
use crate::alkanes::execute::{EnhancedAlkanesExecutor, EnhancedExecuteParams, EnhancedExecuteResult};
use crate::alkanes::types::{InputRequirement, OutputTarget, ProtostoneSpec, AlkaneId};
use crate::alkanes::protostone::ProtostoneEdict;
#[cfg(not(feature = "std"))]
use alloc::{vec, vec::Vec, string::{String, ToString}, format};
#[cfg(feature = "std")]
use std::{vec, vec::Vec, string::{String, ToString}, format};
use bitcoin::Address;
use core::str::FromStr;

/// Parameters for wrap-btc operation
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WrapBtcParams {
    /// Amount of BTC (in satoshis) to wrap
    pub amount: u64,
    /// Address to receive the frBTC tokens
    pub to_address: String,
    /// Addresses to source UTXOs from
    pub from_addresses: Option<Vec<String>>,
    /// Change address
    pub change_address: Option<String>,
    /// Fee rate in sat/vB
    pub fee_rate: Option<f32>,
    /// Show raw JSON output
    pub raw_output: bool,
    /// Enable transaction tracing
    pub trace_enabled: bool,
    /// Mine a block after broadcasting (regtest only)
    pub mine_enabled: bool,
    /// Automatically confirm the transaction preview
    pub auto_confirm: bool,
}

/// Constants for frBTC wrapping
pub const FRBTC_ALKANE_BLOCK: u64 = 32;
pub const FRBTC_ALKANE_TX: u64 = 0;
pub const FRBTC_WRAP_OPCODE: u128 = 77;

pub const BRC20_VAULT_BLOCK: u64 = 4;
pub const BRC20_VAULT_TX: u64 = 3032615708;
pub const VAULT_LOCK_OPCODE: u128 = 1;

/// Executor for wrap-btc operations
pub struct WrapBtcExecutor<'a> {
    pub provider: &'a mut dyn DeezelProvider,
}

impl<'a> WrapBtcExecutor<'a> {
    /// Create a new wrap-btc executor
    pub fn new(provider: &'a mut dyn DeezelProvider) -> Self {
        Self { provider }
    }

    /// Execute wrap-btc operation
    /// This creates a single-protostone transaction: [32,0,77]:v0:v0
    /// - Sends BTC to subfrost signer address (output 0)
    /// - Mints frBTC to recipient address (output 1, via pointer v0)
    /// - Change goes to change address (output 2 or 1 if no change)
    pub async fn wrap_btc(&mut self, params: WrapBtcParams) -> Result<EnhancedExecuteResult> {
        log::info!("Starting wrap-btc operation for {} sats", params.amount);

        // Fetch the subfrost signer address to send BTC to
        let subfrost_address = self.fetch_subfrost_address().await?;
        log::info!("Subfrost signer address: {}", subfrost_address);

        // Output structure:
        // Output 0: Subfrost address (receives BTC payment, amount specified in protostone)
        // Output 1: Recipient address (receives frBTC via pointer v0)
        // Output 2+: Change address (if needed)
        let to_addresses = vec![
            subfrost_address,       // Output 0: BTC to subfrost
            params.to_address,      // Output 1: frBTC recipient (v0 points here)
        ];

        // Build the single protostone: [32,0,77]:v0:v0
        let protostones = self.build_wrap_protostone(params.amount)?;

        // Build execute params
        let execute_params = EnhancedExecuteParams {
            alkanes_change_address: None,
            input_requirements: vec![
                InputRequirement::Bitcoin { amount: params.amount },
            ],
            to_addresses,
            from_addresses: params.from_addresses,
            change_address: params.change_address,
            fee_rate: params.fee_rate,
            envelope_data: None, // No contract deployment
            protostones,
            raw_output: params.raw_output,
            trace_enabled: params.trace_enabled,
            mine_enabled: params.mine_enabled,
            auto_confirm: params.auto_confirm,
            ordinals_strategy: crate::alkanes::types::OrdinalsStrategy::default(),
            mempool_indexer: false,
            split_transactions: false,
            known_pending_tx_hexes: Vec::new(),
        };

        // Execute using the enhanced alkanes executor
        let mut executor = EnhancedAlkanesExecutor::new(self.provider);
        let state = executor.execute(execute_params.clone()).await?;

        // Extract the final result
        match state {
            crate::alkanes::types::ExecutionState::ReadyToSign(ready) => {
                executor.resume_execution(ready, &execute_params).await
            }
            _ => Err(AlkanesError::Other("Unexpected execution state".to_string())),
        }
    }

    /// Fetch the subfrost signer address using the existing subfrost module
    async fn fetch_subfrost_address(&self) -> Result<String> {
        use crate::subfrost::get_subfrost_address;
        
        log::info!("Fetching subfrost signer address from frBTC alkane {{32, 0}}");
        
        let alkane_id = AlkaneId {
            block: FRBTC_ALKANE_BLOCK,
            tx: FRBTC_ALKANE_TX,
        };
        
        let address = get_subfrost_address(self.provider, &alkane_id).await?;
        log::info!("Subfrost signer address: {}", address);
        
        Ok(address)
    }

    /// Build the single protostone for wrapping BTC: [32,0,77]:v0:v0
    /// - Cellpack: Call frBTC {32, 0} opcode 77 (exchange/wrap)
    /// - Bitcoin transfer: Send specified amount to output 0 (subfrost address)
    /// - Pointer: v0 (output 1) - where minted frBTC goes
    /// - Refund: v0 (output 1) - unused frBTC goes back here
    fn build_wrap_protostone(&self, amount: u64) -> Result<Vec<ProtostoneSpec>> {
        use alkanes_support::id::AlkaneId as SupportAlkaneId;
        
        let protostone = ProtostoneSpec {
            cellpack: Some(alkanes_support::cellpack::Cellpack {
                target: SupportAlkaneId {
                    block: FRBTC_ALKANE_BLOCK as u128,
                    tx: FRBTC_ALKANE_TX as u128,
                },
                inputs: vec![FRBTC_WRAP_OPCODE], // Opcode 77: exchange()
            }),
            edicts: vec![], // No edicts, minted frBTC goes to pointer destination
            bitcoin_transfer: Some(crate::alkanes::types::BitcoinTransfer {
                amount,
                target: OutputTarget::Output(0), // Send BTC to subfrost address (output 0)
            }),
            pointer: Some(OutputTarget::Output(1)),  // Minted frBTC goes to output 1 (recipient)
            refund: Some(OutputTarget::Output(1)),   // Refund unused frBTC to output 1
        };

        Ok(vec![protostone])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wrap_protostones_structure() {
        use alkanes_support::id::AlkaneId as SupportAlkaneId;

        // Mock provider not needed for this test
        let protostones = vec![
            ProtostoneSpec {
                cellpack: Some(alkanes_support::cellpack::Cellpack {
                    target: SupportAlkaneId {
                        block: FRBTC_ALKANE_BLOCK as u128,
                        tx: FRBTC_ALKANE_TX as u128,
                    },
                    inputs: vec![FRBTC_WRAP_OPCODE],
                }),
                edicts: vec![],
                bitcoin_transfer: None,
                pointer: None,
                refund: None,
            },
            ProtostoneSpec {
                cellpack: Some(alkanes_support::cellpack::Cellpack {
                    target: SupportAlkaneId {
                        block: BRC20_VAULT_BLOCK as u128,
                        tx: BRC20_VAULT_TX as u128,
                    },
                    inputs: vec![VAULT_LOCK_OPCODE],
                }),
                edicts: vec![],
                bitcoin_transfer: None,
                pointer: None,
                refund: None,
            },
        ];

        // Verify first protostone targets frBTC
        assert_eq!(protostones[0].cellpack.as_ref().unwrap().target.block, FRBTC_ALKANE_BLOCK as u128);
        assert_eq!(protostones[0].cellpack.as_ref().unwrap().target.tx, FRBTC_ALKANE_TX as u128);
        assert_eq!(protostones[0].cellpack.as_ref().unwrap().inputs[0], FRBTC_WRAP_OPCODE);

        // Verify second protostone targets vault
        assert_eq!(protostones[1].cellpack.as_ref().unwrap().target.block, BRC20_VAULT_BLOCK as u128);
        assert_eq!(protostones[1].cellpack.as_ref().unwrap().target.tx, BRC20_VAULT_TX as u128);
        assert_eq!(protostones[1].cellpack.as_ref().unwrap().inputs[0], VAULT_LOCK_OPCODE);
    }
}
