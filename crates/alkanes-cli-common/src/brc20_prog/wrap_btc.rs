// BRC20-Prog wrap-btc: Dual protocol transaction combining alkanes frBTC wrap with brc20-prog wrapAndExecute2
// This creates a unified frBTC asset that exists in both alkanes and brc20-prog protocols

use crate::{AlkanesError, DeezelProvider, Result};
use crate::traits::WalletProvider;
use crate::alkanes::wrap_btc::{WrapBtcExecutor as AlkanesWrapExecutor, WrapBtcParams as AlkanesWrapParams};
use crate::brc20_prog::execute::Brc20ProgExecutor;
use crate::brc20_prog::types::Brc20ProgExecuteParams;
use crate::brc20_prog::types::{Brc20ProgCallInscription, Brc20ProgExecuteResult};
use crate::brc20_prog::calldata::encode_function_call;
#[cfg(not(feature = "std"))]
use alloc::{vec, vec::Vec, string::{String, ToString}, format};
#[cfg(feature = "std")]
use std::{vec, vec::Vec, string::{String, ToString}, format};

/// Parameters for brc20-prog wrap-btc operation
#[derive(Debug, Clone)]
pub struct Brc20ProgWrapBtcParams {
    /// Amount of BTC (in satoshis) to wrap
    pub amount: u64,
    /// Target contract address for wrapAndExecute2
    pub target_address: String,
    /// Calldata to pass to the target contract
    pub calldata: Vec<u8>,
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

/// Dummy FrBTC contract address (placeholder until deployed)
/// TODO: Update this with the actual deployed contract address
pub const FRBTC_CONTRACT_ADDRESS: &str = "0x0000000000000000000000000000000000000000";

/// Executor for brc20-prog wrap-btc operations
pub struct Brc20ProgWrapBtcExecutor<'a> {
    pub provider: &'a mut dyn DeezelProvider,
}

impl<'a> Brc20ProgWrapBtcExecutor<'a> {
    /// Create a new brc20-prog wrap-btc executor
    pub fn new(provider: &'a mut dyn DeezelProvider) -> Self {
        Self { provider }
    }

    /// Execute brc20-prog wrap-btc operation
    /// This creates a dual-protocol transaction:
    /// 1. Alkanes OP_RETURN with two protostones (wrap frBTC + lock in vault)
    /// 2. BRC20-prog inscription calling wrapAndExecute2(address,bytes)
    ///
    /// The result is frBTC that exists in both:
    /// - Alkanes protocol (locked in vault {4, 3032615708})
    /// - BRC20-prog protocol (via wrapAndExecute2)
    pub async fn wrap_btc(&mut self, params: Brc20ProgWrapBtcParams) -> Result<Brc20ProgExecuteResult> {
        log::info!("Starting brc20-prog wrap-btc operation for {} sats", params.amount);
        log::info!("Target contract: {}", params.target_address);

        // Step 1: Build the brc20-prog inscription for wrapAndExecute2
        let inscription_json = self.build_wrap_and_execute_inscription(
            &params.target_address,
            &params.calldata,
        )?;

        log::info!("BRC20-prog inscription: {}", inscription_json);

        // Step 2: Execute the brc20-prog transaction
        // This will create the commit-reveal with the inscription
        // The reveal transaction will also include the alkanes OP_RETURN
        let brc20_params = Brc20ProgExecuteParams {
            inscription_content: inscription_json,
            from_addresses: params.from_addresses.clone(),
            change_address: params.change_address.clone(),
            fee_rate: params.fee_rate,
            raw_output: params.raw_output,
            trace_enabled: params.trace_enabled,
            mine_enabled: params.mine_enabled,
            auto_confirm: params.auto_confirm,
        };

        let mut brc20_executor = Brc20ProgExecutor::new(self.provider);
        let result = brc20_executor.execute(brc20_params).await?;

        log::info!("✅ BRC20-prog wrap-btc transaction completed");
        log::info!("Commit TXID: {}", result.commit_txid);
        log::info!("Reveal TXID: {}", result.reveal_txid);

        // TODO: Extend the reveal transaction to include alkanes protostones
        // This requires modifying the Brc20ProgExecutor to support dual-protocol transactions
        log::warn!("Note: Alkanes protostone integration not yet implemented");
        log::warn!("This transaction only creates the brc20-prog side");
        log::warn!("Full dual-protocol support requires additional development");

        Ok(result)
    }

    /// Build the JSON inscription for wrapAndExecute2 call
    /// Format: {"p":"brc20-prog","op":"call","c":"<frbtc-contract>","d":"<calldata>"}
    fn build_wrap_and_execute_inscription(
        &self,
        target_address: &str,
        calldata: &[u8],
    ) -> Result<String> {
        // Encode the wrapAndExecute2(address,bytes) call
        // Function signature: wrapAndExecute2(address,bytes)
        let calldata_hex = hex::encode(calldata);
        
        // Build the function call with target address and calldata
        let function_calldata = encode_function_call(
            "wrapAndExecute2(address,bytes)",
            &format!("{},{}", target_address, calldata_hex),
        )?;

        // Create the BRC20-prog call inscription
        let inscription = Brc20ProgCallInscription::new(
            FRBTC_CONTRACT_ADDRESS.to_string(),
            function_calldata,
        );

        // Serialize to JSON
        serde_json::to_string(&inscription)
            .map_err(|e| AlkanesError::Other(format!("Failed to serialize inscription: {}", e)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wrap_and_execute_inscription_format() {
        // This test just verifies the inscription structure
        let target = "0x1234567890abcdef1234567890abcdef12345678";
        let calldata = vec![0x12, 0x34, 0x56, 0x78];
        
        let inscription = Brc20ProgCallInscription {
            p: "brc20-prog".to_string(),
            op: "call".to_string(),
            c: Some(FRBTC_CONTRACT_ADDRESS.to_string()),
            i: None,
            d: Some("0x...".to_string()), // Placeholder
            b: None,
        };

        let json = serde_json::to_string(&inscription).unwrap();
        assert!(json.contains("brc20-prog"));
        assert!(json.contains("call"));
    }

    #[test]
    fn test_frbtc_contract_address() {
        // Verify the placeholder address is valid hex
        assert!(FRBTC_CONTRACT_ADDRESS.starts_with("0x"));
        assert_eq!(FRBTC_CONTRACT_ADDRESS.len(), 42); // 0x + 40 hex chars
    }
}
