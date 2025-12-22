// FrBTC operations for BRC20-Prog
// Implements wrap, unwrap, wrapAndExecute, wrapAndExecute2 for FrBTC on mainnet and signet

use crate::{AlkanesError, DeezelProvider, Result};
use crate::traits::{WalletProvider, JsonRpcProvider};
use crate::brc20_prog::execute::Brc20ProgExecutor;
use crate::brc20_prog::types::{Brc20ProgExecuteParams, Brc20ProgExecuteResult, Brc20ProgCallInscription, AdditionalOutput};
use crate::brc20_prog::calldata::encode_function_call;
use crate::brc20_prog::eth_call::get_signer_address;
use crate::unwrap::brc20_prog::script_pubkey_to_address;

#[cfg(not(feature = "std"))]
use alloc::{vec, vec::Vec, string::{String, ToString}, format};
#[cfg(feature = "std")]
use std::{vec, vec::Vec, string::{String, ToString}, format};

use serde::{Serialize, Deserialize};

// ============================================================================
// CONSTANTS - Canonical FrBTC addresses for each network
// ============================================================================

/// FrBTC contract address on mainnet
pub const FRBTC_ADDRESS_MAINNET: &str = "0xdBB5b6A1D422fca2813cF486e5F986ADB09D8337";

/// FrBTC contract address on signet
pub const FRBTC_ADDRESS_SIGNET: &str = "0x8A3d3eB978c754D3Abf2b293D67848af4041106f";

/// FrBTC contract address on regtest (placeholder)
pub const FRBTC_ADDRESS_REGTEST: &str = "0x0000000000000000000000000000000000000000";

/// Get the FrBTC contract address for the given network
pub fn get_frbtc_contract_address(network: bitcoin::Network) -> &'static str {
    match network {
        bitcoin::Network::Bitcoin => FRBTC_ADDRESS_MAINNET,
        bitcoin::Network::Signet => FRBTC_ADDRESS_SIGNET,
        bitcoin::Network::Regtest => FRBTC_ADDRESS_REGTEST,
        bitcoin::Network::Testnet => FRBTC_ADDRESS_MAINNET,
        _ => FRBTC_ADDRESS_MAINNET,
    }
}

// ============================================================================
// TYPES
// ============================================================================

/// Parameters for simple wrap (wrap BTC to frBTC)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrBtcWrapParams {
    /// Amount of BTC to wrap (in satoshis)
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
    /// Use MARA Slipstream service for broadcasting
    pub use_slipstream: bool,
    /// Use Rebar Shield for private transaction relay
    pub use_rebar: bool,
    /// Rebar fee tier (1 or 2)
    pub rebar_tier: Option<u8>,
    /// Resume from existing commit transaction
    pub resume_from_commit: Option<String>,
}

/// Parameters for unwrap (burn frBTC to get BTC)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrBtcUnwrapParams {
    /// Amount of frBTC to unwrap (in satoshis, same as BTC amount)
    pub amount: u64,
    /// Vout index where the inscription output will be (used by contract)
    pub vout: u64,
    /// Recipient address for the unwrapped BTC
    pub recipient_address: String,
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
    /// Use MARA Slipstream service for broadcasting
    pub use_slipstream: bool,
    /// Use Rebar Shield for private transaction relay
    pub use_rebar: bool,
    /// Rebar fee tier (1 or 2)
    pub rebar_tier: Option<u8>,
    /// Resume from existing commit transaction
    pub resume_from_commit: Option<String>,
}

/// Parameters for wrapAndExecute (wrap BTC and deploy+execute a script)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrBtcWrapAndExecuteParams {
    /// Amount of BTC to wrap (in satoshis)
    pub amount: u64,
    /// Script bytecode to deploy and execute (hex-encoded)
    pub script_bytecode: String,
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
    /// Use MARA Slipstream service for broadcasting
    pub use_slipstream: bool,
    /// Use Rebar Shield for private transaction relay
    pub use_rebar: bool,
    /// Rebar fee tier (1 or 2)
    pub rebar_tier: Option<u8>,
    /// Resume from existing commit transaction
    pub resume_from_commit: Option<String>,
}

/// Parameters for wrapAndExecute2 (wrap BTC and call existing contract)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrBtcWrapAndExecute2Params {
    /// Amount of BTC to wrap (in satoshis)
    pub amount: u64,
    /// Target contract address to call
    pub target_address: String,
    /// Function signature (e.g., "deposit()")
    pub signature: String,
    /// Calldata arguments (comma-separated)
    pub calldata_args: String,
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
    /// Use MARA Slipstream service for broadcasting
    pub use_slipstream: bool,
    /// Use Rebar Shield for private transaction relay
    pub use_rebar: bool,
    /// Rebar fee tier (1 or 2)
    pub rebar_tier: Option<u8>,
    /// Resume from existing commit transaction
    pub resume_from_commit: Option<String>,
}

// ============================================================================
// EXECUTOR
// ============================================================================

/// Executor for FrBTC operations on BRC20-Prog
pub struct FrBtcExecutor<'a> {
    provider: &'a mut dyn DeezelProvider,
}

impl<'a> FrBtcExecutor<'a> {
    /// Create a new FrBTC executor
    pub fn new(provider: &'a mut dyn DeezelProvider) -> Self {
        Self { provider }
    }

    /// Get the FrBTC signer address (p2tr) for the current network
    /// This calls getSignerAddress() on the FrBTC contract
    pub async fn get_signer_address(&self) -> Result<String> {
        let network = self.provider.get_network();
        let frbtc_address = get_frbtc_contract_address(network);

        let brc20_prog_rpc_url = self.provider.get_brc20_prog_rpc_url()
            .ok_or_else(|| AlkanesError::Configuration("brc20_prog_rpc_url not configured".to_string()))?;

        log::info!("Fetching FrBTC signer address from contract {}", frbtc_address);

        let signer_script = get_signer_address(
            self.provider as &dyn JsonRpcProvider,
            &brc20_prog_rpc_url,
            frbtc_address,
        ).await?;

        let signer_address = script_pubkey_to_address(&signer_script, network)?;
        log::info!("FrBTC signer address: {}", signer_address);

        Ok(signer_address)
    }

    /// Execute wrap() - wrap BTC to frBTC
    ///
    /// This creates a brc20-prog transaction that:
    /// 1. Sends BTC to the FrBTC signer address
    /// 2. Calls wrap() on the FrBTC contract
    /// 3. frBTC is minted to the sender (msg.sender in brc20-prog context)
    pub async fn wrap(&mut self, params: FrBtcWrapParams) -> Result<Brc20ProgExecuteResult> {
        log::info!("Starting FrBTC wrap operation for {} sats", params.amount);

        let network = self.provider.get_network();
        let frbtc_address = get_frbtc_contract_address(network);

        // Get the signer address to send BTC to
        let signer_address = self.get_signer_address().await?;
        log::info!("Will send {} sats to signer address: {}", params.amount, signer_address);

        // Build the wrap() calldata
        // wrap() has no parameters, so just the function selector
        let calldata = encode_function_call("wrap()", "")?;

        // Create the inscription
        let inscription = Brc20ProgCallInscription::new(
            frbtc_address.to_string(),
            calldata,
        );
        let inscription_json = serde_json::to_string(&inscription)
            .map_err(|e| AlkanesError::Other(format!("Failed to serialize inscription: {}", e)))?;

        log::info!("BRC20-prog inscription: {}", inscription_json);

        // Build execute params with the signer address as additional output
        // The activation tx will include an output sending params.amount to the signer
        use crate::brc20_prog::AdditionalOutput;
        let additional_outputs = vec![AdditionalOutput {
            address: signer_address.clone(),
            amount: params.amount,
        }];

        let execute_params = Brc20ProgExecuteParams {
            inscription_content: inscription_json,
            from_addresses: params.from_addresses,
            change_address: params.change_address,
            fee_rate: params.fee_rate,
            raw_output: params.raw_output,
            trace_enabled: params.trace_enabled,
            mine_enabled: params.mine_enabled,
            auto_confirm: params.auto_confirm,
            use_activation: true, // Must use 3-tx pattern for wrap
            use_slipstream: params.use_slipstream,
            use_rebar: params.use_rebar,
            rebar_tier: params.rebar_tier,
            strategy: None,
            resume_from_commit: params.resume_from_commit,
            additional_outputs: Some(additional_outputs),
            mempool_indexer: false,
        };

        let mut executor = Brc20ProgExecutor::new(self.provider);
        let result = executor.execute(execute_params).await?;

        log::info!("✅ FrBTC wrap completed");
        log::info!("   Commit TXID: {}", result.commit_txid);
        log::info!("   Reveal TXID: {}", result.reveal_txid);
        if let Some(ref activation_txid) = result.activation_txid {
            log::info!("   Activation TXID: {} (sent {} sats to {})", activation_txid, params.amount, signer_address);
        }

        Ok(result)
    }

    /// Execute unwrap() - burn frBTC to queue BTC withdrawal
    ///
    /// This creates a brc20-prog transaction that:
    /// 1. Sends a dust output (546 sats) to the FrBTC signer address (required by contract)
    /// 2. Calls unwrap2(amount, vout, pkscriptRecipient) on the FrBTC contract
    /// 3. Burns frBTC and queues a payment to the recipient
    ///
    /// Note: The actual BTC is paid out by the subfrost operator, not in this transaction
    pub async fn unwrap(&mut self, params: FrBtcUnwrapParams) -> Result<Brc20ProgExecuteResult> {
        log::info!("Starting FrBTC unwrap operation for {} sats", params.amount);

        let network = self.provider.get_network();
        let frbtc_address = get_frbtc_contract_address(network);

        // Get the signer address for the dust output
        let signer_address = self.get_signer_address().await?;
        log::info!("Signer address for dust output: {}", signer_address);

        // Parse the recipient address and get its script_pubkey
        let recipient_address = bitcoin::Address::from_str(&params.recipient_address)
            .map_err(|e| AlkanesError::AddressResolution(format!("Invalid recipient address: {}", e)))?
            .require_network(network)
            .map_err(|e| AlkanesError::AddressResolution(format!("Address network mismatch: {}", e)))?;

        let recipient_script = recipient_address.script_pubkey();
        let recipient_script_hex = hex::encode(recipient_script.as_bytes());

        log::info!("Unwrapping {} sats to {} (script: 0x{})",
            params.amount, params.recipient_address, recipient_script_hex);

        // The dust output will be at vout 1 in activation tx (after OP_RETURN at vout 0)
        // This is the index the contract expects for validation
        let dust_vout = 1u64;

        // Build the unwrap2(uint256 amount, uint256 vout, bytes calldata pkscriptRecipient) calldata
        let calldata = encode_function_call(
            "unwrap2(uint256,uint256,bytes)",
            &format!("{},{},0x{}", params.amount, dust_vout, recipient_script_hex),
        )?;

        // Create the inscription
        let inscription = Brc20ProgCallInscription::new(
            frbtc_address.to_string(),
            calldata,
        );
        let inscription_json = serde_json::to_string(&inscription)
            .map_err(|e| AlkanesError::Other(format!("Failed to serialize inscription: {}", e)))?;

        log::info!("BRC20-prog inscription: {}", inscription_json);

        // Add dust output to signer address (546 sats is the activation marker)
        use crate::brc20_prog::AdditionalOutput;
        let additional_outputs = vec![AdditionalOutput {
            address: signer_address.clone(),
            amount: 546, // Dust output for activation
        }];

        let execute_params = Brc20ProgExecuteParams {
            inscription_content: inscription_json,
            from_addresses: params.from_addresses,
            change_address: params.change_address,
            fee_rate: params.fee_rate,
            raw_output: params.raw_output,
            trace_enabled: params.trace_enabled,
            mine_enabled: params.mine_enabled,
            auto_confirm: params.auto_confirm,
            use_activation: true, // Must use 3-tx pattern for unwrap
            use_slipstream: params.use_slipstream,
            use_rebar: params.use_rebar,
            rebar_tier: params.rebar_tier,
            strategy: None,
            resume_from_commit: params.resume_from_commit,
            additional_outputs: Some(additional_outputs),
            mempool_indexer: false,
        };

        let mut executor = Brc20ProgExecutor::new(self.provider);
        let result = executor.execute(execute_params).await?;

        log::info!("✅ FrBTC unwrap queued");
        log::info!("   Commit TXID: {}", result.commit_txid);
        log::info!("   Reveal TXID: {}", result.reveal_txid);
        if let Some(ref activation_txid) = result.activation_txid {
            log::info!("   Activation TXID: {} (dust output to {})", activation_txid, signer_address);
        }
        log::info!("   Recipient will receive {} sats at {}", params.amount, params.recipient_address);

        Ok(result)
    }

    /// Execute wrapAndExecute() - wrap BTC and deploy+execute a script
    ///
    /// This creates a brc20-prog transaction that:
    /// 1. Sends BTC to the FrBTC signer address
    /// 2. Calls wrapAndExecute(bytes script) on the FrBTC contract
    /// 3. Mints frBTC, deploys the script via CREATE2, and calls execute(sender, amount) on it
    pub async fn wrap_and_execute(&mut self, params: FrBtcWrapAndExecuteParams) -> Result<Brc20ProgExecuteResult> {
        log::info!("Starting FrBTC wrapAndExecute operation for {} sats", params.amount);

        let network = self.provider.get_network();
        let frbtc_address = get_frbtc_contract_address(network);

        // Get the signer address for reference
        let signer_address = self.get_signer_address().await?;
        log::info!("FrBTC signer address: {}", signer_address);

        // Build the wrapAndExecute(bytes memory script) calldata
        let script_hex = if params.script_bytecode.starts_with("0x") {
            params.script_bytecode.clone()
        } else {
            format!("0x{}", params.script_bytecode)
        };

        let calldata = encode_function_call(
            "wrapAndExecute(bytes)",
            &script_hex,
        )?;

        // Create the inscription
        let inscription = Brc20ProgCallInscription::new(
            frbtc_address.to_string(),
            calldata,
        );
        let inscription_json = serde_json::to_string(&inscription)
            .map_err(|e| AlkanesError::Other(format!("Failed to serialize inscription: {}", e)))?;

        log::info!("BRC20-prog inscription: {}", inscription_json);

        // Additional output sends the wrap amount to signer address
        let additional_outputs = vec![AdditionalOutput {
            address: signer_address.clone(),
            amount: params.amount,
        }];

        let execute_params = Brc20ProgExecuteParams {
            inscription_content: inscription_json,
            from_addresses: params.from_addresses,
            change_address: params.change_address,
            fee_rate: params.fee_rate,
            raw_output: params.raw_output,
            trace_enabled: params.trace_enabled,
            mine_enabled: params.mine_enabled,
            auto_confirm: params.auto_confirm,
            use_activation: true, // Need activation to send BTC to signer
            use_slipstream: params.use_slipstream,
            use_rebar: params.use_rebar,
            rebar_tier: params.rebar_tier,
            strategy: None,
            resume_from_commit: params.resume_from_commit,
            additional_outputs: Some(additional_outputs),
            mempool_indexer: false,
        };

        let mut executor = Brc20ProgExecutor::new(self.provider);
        let result = executor.execute(execute_params).await?;

        log::info!("✅ FrBTC wrapAndExecute completed");
        log::info!("   Commit TXID: {}", result.commit_txid);
        log::info!("   Reveal TXID: {}", result.reveal_txid);
        if let Some(ref activation_txid) = result.activation_txid {
            log::info!("   Activation TXID: {} ({} sats to {})", activation_txid, params.amount, signer_address);
        }

        Ok(result)
    }

    /// Execute wrapAndExecute2() - wrap BTC and call an existing contract
    ///
    /// This creates a brc20-prog transaction that:
    /// 1. Sends BTC to the FrBTC signer address
    /// 2. Calls wrapAndExecute2(address target, bytes data) on the FrBTC contract
    /// 3. Mints frBTC, approves target, and calls execute(sender, amount, data) on target
    pub async fn wrap_and_execute2(&mut self, params: FrBtcWrapAndExecute2Params) -> Result<Brc20ProgExecuteResult> {
        log::info!("Starting FrBTC wrapAndExecute2 operation for {} sats", params.amount);
        log::info!("Target contract: {}", params.target_address);

        let network = self.provider.get_network();
        let frbtc_address = get_frbtc_contract_address(network);

        // Get the signer address for reference
        let signer_address = self.get_signer_address().await?;
        log::info!("FrBTC signer address: {}", signer_address);

        // Build the inner calldata (the data passed to the target contract)
        let inner_calldata = if params.calldata_args.is_empty() {
            encode_function_call(&params.signature, "")?
        } else {
            encode_function_call(&params.signature, &params.calldata_args)?
        };

        // Build the wrapAndExecute2(address target, bytes memory data) calldata
        let calldata = encode_function_call(
            "wrapAndExecute2(address,bytes)",
            &format!("{},{}", params.target_address, inner_calldata),
        )?;

        // Create the inscription
        let inscription = Brc20ProgCallInscription::new(
            frbtc_address.to_string(),
            calldata,
        );
        let inscription_json = serde_json::to_string(&inscription)
            .map_err(|e| AlkanesError::Other(format!("Failed to serialize inscription: {}", e)))?;

        log::info!("BRC20-prog inscription: {}", inscription_json);

        // Additional output sends the wrap amount to signer address
        let additional_outputs = vec![AdditionalOutput {
            address: signer_address.clone(),
            amount: params.amount,
        }];

        let execute_params = Brc20ProgExecuteParams {
            inscription_content: inscription_json,
            from_addresses: params.from_addresses,
            change_address: params.change_address,
            fee_rate: params.fee_rate,
            raw_output: params.raw_output,
            trace_enabled: params.trace_enabled,
            mine_enabled: params.mine_enabled,
            auto_confirm: params.auto_confirm,
            use_activation: true, // Need activation to send BTC to signer
            use_slipstream: params.use_slipstream,
            use_rebar: params.use_rebar,
            rebar_tier: params.rebar_tier,
            strategy: None,
            resume_from_commit: params.resume_from_commit,
            additional_outputs: Some(additional_outputs),
            mempool_indexer: false,
        };

        let mut executor = Brc20ProgExecutor::new(self.provider);
        let result = executor.execute(execute_params).await?;

        log::info!("✅ FrBTC wrapAndExecute2 completed");
        log::info!("   Commit TXID: {}", result.commit_txid);
        log::info!("   Reveal TXID: {}", result.reveal_txid);
        if let Some(ref activation_txid) = result.activation_txid {
            log::info!("   Activation TXID: {} ({} sats to {})", activation_txid, params.amount, signer_address);
        }

        Ok(result)
    }
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

use core::str::FromStr;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frbtc_addresses() {
        assert!(FRBTC_ADDRESS_MAINNET.starts_with("0x"));
        assert_eq!(FRBTC_ADDRESS_MAINNET.len(), 42);

        assert!(FRBTC_ADDRESS_SIGNET.starts_with("0x"));
        assert_eq!(FRBTC_ADDRESS_SIGNET.len(), 42);
    }

    #[test]
    fn test_get_frbtc_contract_address() {
        assert_eq!(get_frbtc_contract_address(bitcoin::Network::Bitcoin), FRBTC_ADDRESS_MAINNET);
        assert_eq!(get_frbtc_contract_address(bitcoin::Network::Signet), FRBTC_ADDRESS_SIGNET);
        assert_eq!(get_frbtc_contract_address(bitcoin::Network::Regtest), FRBTC_ADDRESS_REGTEST);
    }
}
