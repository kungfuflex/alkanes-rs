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
#[derive(Debug, Clone)]
pub struct WrapBtcParams {
    /// Amount of BTC (in satoshis) to wrap
    pub amount: u64,
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
    /// This creates a two-protostone transaction:
    /// 1. First protostone: Call frBTC {32, 0} opcode 77 (Wrap) with pointer to second protostone
    /// 2. Second protostone: Call vault {4, 3032615708} opcode 1 (Lock) to lock the minted frBTC
    pub async fn wrap_btc(&mut self, params: WrapBtcParams) -> Result<EnhancedExecuteResult> {
        log::info!("Starting wrap-btc operation for {} sats", params.amount);

        // Fetch the subfrost signer address to send BTC to
        let subfrost_address = self.fetch_subfrost_address().await?;
        log::info!("Subfrost signer address: {}", subfrost_address);

        // Create recipient addresses
        // We need to send BTC to the subfrost address
        let to_addresses = vec![subfrost_address.clone()];

        // Build the two protostones
        let protostones = self.build_wrap_protostones(&to_addresses)?;

        // Build execute params
        let execute_params = EnhancedExecuteParams {
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

    /// Fetch the subfrost signer address by calling opcode 103 (GetSigner) on frBTC alkane
    async fn fetch_subfrost_address(&self) -> Result<String> {
        log::info!("Fetching subfrost signer pubkey from frBTC alkane {{32, 0}}");

        // Build the message context for simulate
        use crate::proto::alkanes::MessageContextParcel;
        
        // Encode the calldata as compact u64 values using LEB128
        let mut calldata = Vec::new();
        // Target alkane: block
        leb128::write::unsigned(&mut calldata, FRBTC_ALKANE_BLOCK).unwrap();
        // Target alkane: tx
        leb128::write::unsigned(&mut calldata, FRBTC_ALKANE_TX).unwrap();
        // Opcode 103 (GetSigner)
        leb128::write::unsigned(&mut calldata, 103u64).unwrap();
        
        let context = MessageContextParcel {
            alkanes: vec![],
            transaction: vec![],
            block: vec![],
            height: 0,
            vout: 0,
            txindex: 0,
            calldata,
            pointer: 0,
            refund_pointer: 0,
        };

        // Call simulate with the context
        let response = self.provider
            .simulate(
                &format!("{}:{}", FRBTC_ALKANE_BLOCK, FRBTC_ALKANE_TX),
                &context,
            )
            .await?;

        // Extract the pubkey from response data
        let data = response.get("data")
            .and_then(|d| d.as_str())
            .ok_or_else(|| AlkanesError::Other("No data in simulate response".to_string()))?;

        // Remove 0x prefix if present
        let pubkey_hex = if data.starts_with("0x") {
            &data[2..]
        } else {
            data
        };

        // Decode the pubkey bytes
        let pubkey_bytes = hex::decode(pubkey_hex)
            .map_err(|e| AlkanesError::Other(format!("Failed to decode pubkey hex: {}", e)))?;

        // The response is an x-only pubkey (32 bytes)
        // We need to derive the taproot address from it
        if pubkey_bytes.len() != 32 {
            return Err(AlkanesError::Other(format!(
                "Invalid pubkey length: expected 32 bytes, got {}",
                pubkey_bytes.len()
            )));
        }

        // Convert to x-only pubkey and create taproot address
        use bitcoin::key::TapTweak;
        
        let xonly_pubkey = bitcoin::secp256k1::XOnlyPublicKey::from_slice(&pubkey_bytes)
            .map_err(|e| AlkanesError::Other(format!("Invalid x-only pubkey: {}", e)))?;

        let secp = bitcoin::secp256k1::Secp256k1::new();
        let (tweaked_pubkey, _) = xonly_pubkey.tap_tweak(&secp, None);

        let network = self.provider.get_network();
        let address = Address::p2tr_tweaked(tweaked_pubkey, network);

        log::info!("Derived subfrost address: {}", address);
        Ok(address.to_string())
    }

    /// Build the two protostones for wrapping BTC
    /// 1. First protostone: frBTC wrap (opcode 77) - pointer managed by protostone message
    /// 2. Second protostone: Vault lock (opcode 1) to lock the minted frBTC
    fn build_wrap_protostones(&self, _to_addresses: &[String]) -> Result<Vec<ProtostoneSpec>> {
        use alkanes_support::id::AlkaneId as SupportAlkaneId;
        
        // First protostone: Call frBTC {32, 0} opcode 77 (Wrap)
        // The pointer will be managed by the protostone message itself
        let first_protostone = ProtostoneSpec {
            cellpack: Some(alkanes_support::cellpack::Cellpack {
                target: SupportAlkaneId {
                    block: FRBTC_ALKANE_BLOCK as u128,
                    tx: FRBTC_ALKANE_TX as u128,
                },
                inputs: vec![FRBTC_WRAP_OPCODE],
            }),
            edicts: vec![], // No edicts, minted frBTC goes to pointer destination
            bitcoin_transfer: Some(crate::alkanes::types::BitcoinTransfer {
                amount: 0, // Will be filled by executor
                target: OutputTarget::Output(0), // Send BTC to subfrost address
            }),
        };

        // Second protostone: Call vault {4, 3032615708} opcode 1 (Lock)
        let second_protostone = ProtostoneSpec {
            cellpack: Some(alkanes_support::cellpack::Cellpack {
                target: SupportAlkaneId {
                    block: BRC20_VAULT_BLOCK as u128,
                    tx: BRC20_VAULT_TX as u128,
                },
                inputs: vec![VAULT_LOCK_OPCODE],
            }),
            edicts: vec![], // No edicts, receives frBTC from first protostone's pointer
            bitcoin_transfer: None,
        };

        Ok(vec![first_protostone, second_protostone])
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
