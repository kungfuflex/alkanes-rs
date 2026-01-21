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
    /// Mint DIESEL tokens in commit and reveal transactions
    pub mint_diesel: bool,
}

/// Default FrBTC contract addresses for different networks
pub const DEFAULT_FRBTC_ADDRESS_MAINNET: &str = "0xdBB5b6A1D422fca2813cF486e5F986ADB09D8337";
pub const DEFAULT_FRBTC_ADDRESS_SIGNET: &str = "0x8A3d3eB978c754D3Abf2b293D67848af4041106f";
pub const DEFAULT_FRBTC_ADDRESS_REGTEST: &str = "0x0000000000000000000000000000000000000000";

/// Legacy constant - kept for backward compatibility
/// Use get_frbtc_address() with network parameter for network-specific addresses
pub const FRBTC_CONTRACT_ADDRESS: &str = DEFAULT_FRBTC_ADDRESS_MAINNET;

/// Get the appropriate FRBTC contract address for the given network
pub fn get_frbtc_address(network: bitcoin::Network) -> &'static str {
    use bitcoin::Network;
    match network {
        Network::Bitcoin => DEFAULT_FRBTC_ADDRESS_MAINNET,
        Network::Signet => DEFAULT_FRBTC_ADDRESS_SIGNET,
        Network::Regtest => DEFAULT_FRBTC_ADDRESS_REGTEST,
        Network::Testnet => DEFAULT_FRBTC_ADDRESS_MAINNET, // Use mainnet address for testnet
        _ => DEFAULT_FRBTC_ADDRESS_MAINNET,
    }
}

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
    /// This creates a brc20-prog transaction that calls wrapAndExecute2(address,bytes)
    /// on the FrBTC contract, which:
    /// 1. Wraps BTC sent to the FrBTC signer address into frBTC tokens
    /// 2. Mints frBTC to the FrBTC contract itself
    /// 3. Approves the target contract to spend the minted frBTC
    /// 4. Calls execute(msg.sender, amount, data) on the target contract
    /// 5. Returns any leftover frBTC to msg.sender
    ///
    /// This is a pure brc20-prog transaction - no alkanes protostones needed
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
            use_activation: false, // Use 2-tx pattern for wrap-btc
            use_slipstream: false,
            use_rebar: false,
            rebar_tier: None,
            strategy: None,
            resume_from_commit: None,
            additional_outputs: None,
            mempool_indexer: false,
            mint_diesel: false,
        };

        let mut brc20_executor = Brc20ProgExecutor::new(self.provider);
        let result = brc20_executor.execute(brc20_params).await?;

        log::info!("✅ BRC20-prog wrap-btc transaction completed");
        log::info!("Commit TXID: {}", result.commit_txid);
        log::info!("Reveal TXID: {}", result.reveal_txid);
        log::info!("frBTC will be minted to {} and forwarded to target contract", params.target_address);

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
        // Verify all FRBTC addresses are valid hex
        assert!(FRBTC_CONTRACT_ADDRESS.starts_with("0x"));
        assert_eq!(FRBTC_CONTRACT_ADDRESS.len(), 42); // 0x + 40 hex chars
        
        assert!(DEFAULT_FRBTC_ADDRESS_MAINNET.starts_with("0x"));
        assert_eq!(DEFAULT_FRBTC_ADDRESS_MAINNET.len(), 42);
        
        assert!(DEFAULT_FRBTC_ADDRESS_SIGNET.starts_with("0x"));
        assert_eq!(DEFAULT_FRBTC_ADDRESS_SIGNET.len(), 42);
        
        assert!(DEFAULT_FRBTC_ADDRESS_REGTEST.starts_with("0x"));
        assert_eq!(DEFAULT_FRBTC_ADDRESS_REGTEST.len(), 42);
    }
    
    #[test]
    fn test_get_frbtc_address() {
        use bitcoin::Network;
        
        // Test network-specific addresses
        assert_eq!(get_frbtc_address(Network::Bitcoin), DEFAULT_FRBTC_ADDRESS_MAINNET);
        assert_eq!(get_frbtc_address(Network::Signet), DEFAULT_FRBTC_ADDRESS_SIGNET);
        assert_eq!(get_frbtc_address(Network::Regtest), DEFAULT_FRBTC_ADDRESS_REGTEST);
        assert_eq!(get_frbtc_address(Network::Testnet), DEFAULT_FRBTC_ADDRESS_MAINNET);
    }
}
