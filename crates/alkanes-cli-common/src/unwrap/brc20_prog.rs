//! BRC20-Prog implementation of MetaprotocolUnwrap
//!
//! Uses eth_call to query FrBTC.sol contract for pending unwraps

use crate::{AlkanesError, Result};
use crate::alkanes::PendingUnwrap;
use crate::traits::{DeezelProvider, JsonRpcProvider, EsploraProvider};
use crate::unwrap::MetaprotocolUnwrap;
use crate::brc20_prog::{get_frbtc_address, get_payments_length, get_signer_address, get_payment};
use async_trait::async_trait;

/// BRC20-Prog unwrap implementation using eth_call to FrBTC.sol contract
pub struct Brc20ProgUnwrap {
    /// Optional override for FrBTC contract address
    frbtc_address_override: Option<String>,
}

impl Brc20ProgUnwrap {
    /// Create a new Brc20ProgUnwrap instance
    pub fn new() -> Self {
        Self {
            frbtc_address_override: None,
        }
    }

    /// Create a new Brc20ProgUnwrap instance with a custom FrBTC address
    pub fn with_frbtc_address(address: &str) -> Self {
        Self {
            frbtc_address_override: Some(address.to_string()),
        }
    }

    /// Set the FrBTC contract address override
    pub fn set_frbtc_address(&mut self, address: Option<&str>) {
        self.frbtc_address_override = address.map(|s| s.to_string());
    }
}

impl Default for Brc20ProgUnwrap {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait(?Send)]
impl MetaprotocolUnwrap for Brc20ProgUnwrap {
    async fn get_pending_unwraps(
        &self,
        provider: &dyn DeezelProvider,
        confirmations_required: u64,
    ) -> Result<Vec<PendingUnwrap>> {
        log::info!("[Brc20ProgUnwrap] Fetching unwraps with {} confirmations required", confirmations_required);
        
        // Get current height
        let current_height = provider.get_block_count().await?;
        
        // Calculate cutoff height based on confirmations
        let cutoff_height = if current_height >= confirmations_required {
            current_height - confirmations_required
        } else {
            log::warn!("[Brc20ProgUnwrap] Current height {} is less than confirmations_required {}, using 0",
                current_height, confirmations_required);
            0
        };
        
        log::info!("[Brc20ProgUnwrap] Querying unwraps with cutoff height {} (current: {}, confirmations: {})",
            cutoff_height, current_height, confirmations_required);
        
        // Get FrBTC contract address (use override if provided, else default for network)
        let network = provider.get_network();
        let frbtc_address = self.frbtc_address_override.as_deref()
            .unwrap_or_else(|| get_frbtc_address(network));
        log::info!("[Brc20ProgUnwrap] Using FrBTC contract at {}", frbtc_address);
        
        // Get BRC20-Prog RPC URL
        let brc20_prog_rpc_url = provider.get_brc20_prog_rpc_url()
            .ok_or_else(|| AlkanesError::Configuration("brc20_prog_rpc_url not configured".to_string()))?;
        
        // Step 1: Get payments length
        let payments_length = get_payments_length(
            provider as &dyn JsonRpcProvider,
            &brc20_prog_rpc_url,
            frbtc_address,
        ).await? as usize;
        log::info!("[Brc20ProgUnwrap] Total payments in contract: {}", payments_length);
        
        if payments_length == 0 {
            log::info!("[Brc20ProgUnwrap] No payments found in contract");
            return Ok(vec![]);
        }
        
        // Step 2: Get signer address (p2tr script_pubkey)
        let signer_script = get_signer_address(
            provider as &dyn JsonRpcProvider,
            &brc20_prog_rpc_url,
            frbtc_address,
        ).await?;
        log::info!("[Brc20ProgUnwrap] Signer script_pubkey: 0x{}", hex::encode(&signer_script));
        
        // Convert script_pubkey to taproot address
        let signer_address = script_pubkey_to_address(&signer_script, network)?;
        log::info!("[Brc20ProgUnwrap] Signer taproot address: {}", signer_address);
        
        // Step 3: Get oldest 546 sat UTXO spendable by the signer
        let utxos_json = provider.get_address_utxo(&signer_address).await?;
        let oldest_utxo_height = find_oldest_546_sat_utxo(&utxos_json)?;
        log::info!("[Brc20ProgUnwrap] Oldest 546 sat UTXO at height: {:?}", oldest_utxo_height);
        
        // Step 4: Query payments backwards from length-1
        let mut result = Vec::new();
        let mut idx = payments_length - 1;
        
        loop {
            // Query payment at index
            let payment = get_payment(
                provider as &dyn JsonRpcProvider,
                &brc20_prog_rpc_url,
                frbtc_address,
                idx as u64,
            ).await?;
            log::debug!("[Brc20ProgUnwrap] Payment[{}]: height={}, value={}", idx, payment.height, payment.value);
            
            // Skip if payment is too new (doesn't have enough confirmations yet)
            if payment.height > cutoff_height {
                log::debug!("[Brc20ProgUnwrap] Skipping payment at height {} (too new, needs {} confirmations, cutoff is {})",
                    payment.height, confirmations_required, cutoff_height);
                if idx == 0 {
                    break;
                }
                idx -= 1;
                continue;
            }
            
            // Stop if we've reached a payment older than the oldest spendable UTXO
            // (This means we can't spend it even if we wanted to)
            if let Some(oldest_height) = oldest_utxo_height {
                if payment.height < oldest_height {
                    log::info!("[Brc20ProgUnwrap] Reached payment at height {} older than oldest spendable UTXO at height {}", 
                        payment.height, oldest_height);
                    break;
                }
            }
            
            // This payment has enough confirmations and is spendable, include it
            log::debug!("[Brc20ProgUnwrap] Including payment at height {} (has {} confirmations)", 
                payment.height, current_height.saturating_sub(payment.height));
            let pending_unwrap = payment_to_pending_unwrap(payment, network)?;
            result.push(pending_unwrap);
            
            // Break if we've processed all payments
            if idx == 0 {
                break;
            }
            idx -= 1;
        }
        
        log::info!("[Brc20ProgUnwrap] Returning {} unfiltered unwraps", result.len());
        Ok(result)
    }
    
    async fn get_total_supply(
        &self,
        provider: &dyn DeezelProvider,
    ) -> Result<u64> {
        log::info!("[Brc20ProgUnwrap] Fetching frBTC total supply");
        
        // Get FrBTC contract address for this network
        let network = provider.get_network();
        let frbtc_address = get_frbtc_address(network);
        
        // Get BRC20-Prog RPC URL
        let brc20_prog_rpc_url = provider.get_brc20_prog_rpc_url()
            .ok_or_else(|| AlkanesError::Configuration("brc20_prog_rpc_url not configured".to_string()))?;
        
        // Call totalSupply() on the FrBTC contract
        // Function selector for totalSupply() is 0x18160ddd
        let data = "0x18160ddd";
        
        let params = serde_json::json!([{
            "to": frbtc_address,
            "data": data
        }, "latest"]);
        
        let response = provider.call(
            &brc20_prog_rpc_url,
            "eth_call",
            params,
            1,
        ).await?;
        
        let hex_result = response.as_str()
            .ok_or_else(|| AlkanesError::RpcError("eth_call result is not a string".to_string()))?;
        let hex_stripped = hex_result.strip_prefix("0x").unwrap_or(hex_result);
        
        // Decode uint256 result (32 bytes)
        if hex_stripped.len() != 64 {
            return Err(AlkanesError::RpcError(format!(
                "Invalid totalSupply response length: expected 64 hex chars, got {}",
                hex_stripped.len()
            )));
        }
        
        let bytes = hex::decode(hex_stripped)
            .map_err(|e| AlkanesError::RpcError(format!("Failed to decode hex response: {}", e)))?;
        
        // Convert bytes to u64 (take last 8 bytes as total supply should fit in u64)
        let total_supply = u64::from_be_bytes(bytes[24..32].try_into().unwrap());
        
        log::info!("[Brc20ProgUnwrap] Total supply: {} sats", total_supply);
        Ok(total_supply)
    }
    
    fn protocol_name(&self) -> &'static str {
        "brc20-prog"
    }
}

/// ASM implementation - uses single eth_call with generated bytecode
impl Brc20ProgUnwrap {
    pub async fn get_pending_unwraps_experimental_asm(
        &self,
        provider: &dyn DeezelProvider,
        confirmations_required: u64,
        frbtc_address_override: Option<&str>,
        block_tag: Option<&str>,
    ) -> Result<Vec<PendingUnwrap>> {
        use crate::brc20_prog::generate_batch_payment_fetcher_bytecode;
        
        log::info!("[Brc20ProgUnwrap] 🚀 Using experimental ASM bytecode generator");
        
        // Get current height
        let current_height = provider.get_block_count().await?;
        let cutoff_height = if current_height >= confirmations_required {
            current_height - confirmations_required
        } else {
            0
        };
        
        // Get FrBTC contract address (use override if provided)
        let network = provider.get_network();
        let frbtc_address = frbtc_address_override
            .map(|s| s.to_string())
            .unwrap_or_else(|| get_frbtc_address(network).to_string());
        let brc20_prog_rpc_url = provider.get_brc20_prog_rpc_url()
            .ok_or_else(|| AlkanesError::Configuration("brc20_prog_rpc_url not configured".to_string()))?;
        
        // Get signer address and oldest UTXO
        let signer_script = get_signer_address(
            provider as &dyn JsonRpcProvider,
            &brc20_prog_rpc_url,
            &frbtc_address,
        ).await?;
        let signer_address = script_pubkey_to_address(&signer_script, network)?;
        let utxos_json = provider.get_address_utxo(&signer_address).await?;
        let oldest_utxo_height = find_oldest_546_sat_utxo(&utxos_json)?.unwrap_or(0);
        
        log::info!("[Brc20ProgUnwrap] Generating bytecode: cutoff={}, oldest={}",
                   cutoff_height, oldest_utxo_height);

        // Generate the bytecode
        let bytecode = generate_batch_payment_fetcher_bytecode(
            &frbtc_address,
            cutoff_height,
            oldest_utxo_height,
        )?;
        
        log::debug!("[Brc20ProgUnwrap] Generated {} bytes of bytecode", bytecode.len() / 2);

        // First, estimate gas to see what we actually need
        let tag = block_tag.unwrap_or("latest");
        let estimate_params = serde_json::json!([{
            "data": bytecode.clone()
        }, tag]);

        log::info!("[Brc20ProgUnwrap] Estimating gas for ASM bytecode...");
        let gas_estimate = provider.call(&brc20_prog_rpc_url, "eth_estimateGas", estimate_params, 1).await;

        let estimated_gas = match gas_estimate {
            Ok(estimate) => {
                // Parse the hex value
                let gas_hex = estimate.as_str()
                    .and_then(|s| s.strip_prefix("0x").or(Some(s)))
                    .unwrap_or("0");
                let gas_decimal = u64::from_str_radix(gas_hex, 16).unwrap_or(0);
                log::info!("[Brc20ProgUnwrap] ✅ Gas estimate successful: {} ({} gas)", estimate, gas_decimal);
                Some(gas_decimal)
            }
            Err(e) => {
                log::warn!("[Brc20ProgUnwrap] ⚠️  Gas estimation failed (will try with high limit): {}", e);
                None
            }
        };

        // Use estimated gas + 50% buffer, or fallback to very high limit
        // Maximum safe value is 0x5000000 (83,886,080 gas)
        let gas_limit = if let Some(est) = estimated_gas {
            let with_buffer = (est * 3) / 2;  // 150% of estimate
            let capped = with_buffer.min(0x5000000);
            format!("0x{:x}", capped)
        } else {
            "0x5000000".to_string()  // 83,886,080 gas - very high fallback
        };

        log::info!("[Brc20ProgUnwrap] Using gas limit: {} ({} gas)",
                   gas_limit,
                   u64::from_str_radix(gas_limit.strip_prefix("0x").unwrap_or(&gas_limit), 16).unwrap_or(0));

        // Make single eth_call with generated bytecode and calculated gas limit
        let params = serde_json::json!([{
            "data": bytecode,
            "gas": gas_limit.clone()
        }, tag]);

        log::info!("[Brc20ProgUnwrap] Making single eth_call with ASM bytecode (block_tag={}, gas={})...", tag, gas_limit);
        let response = provider.call(&brc20_prog_rpc_url, "eth_call", params, 1).await?;

        // Parse response - ABI-encoded Payment[] array
        let hex_stripped = response.as_str()
            .ok_or_else(|| AlkanesError::RpcError("eth_call result is not a string".to_string()))?
            .strip_prefix("0x").unwrap_or(response.as_str().unwrap());

        let bytes = hex::decode(hex_stripped)
            .map_err(|e| AlkanesError::RpcError(format!("Failed to decode response: {}", e)))?;

        log::info!("[Brc20ProgUnwrap ASM] Received {} bytes, parsing ABI-encoded Payment[] array", bytes.len());

        // DEBUG: Dump raw response hex for comparison
        log::debug!("[Brc20ProgUnwrap ASM] Raw response hex (first 1024 chars): {}",
                   if hex_stripped.len() > 1024 { &hex_stripped[..1024] } else { hex_stripped });
        log::debug!("[Brc20ProgUnwrap ASM] Full response length: {} hex chars = {} bytes",
                   hex_stripped.len(), bytes.len());

        // DEBUG: Detailed hex dump of response structure
        if bytes.len() >= 64 {
            log::debug!("[Brc20ProgUnwrap ASM] Response structure analysis:");
            log::debug!("  [0x00-0x20] Array offset:  0x{}", hex::encode(&bytes[0..32]));
            log::debug!("  [0x20-0x40] Array length:  0x{} = {} items",
                       hex::encode(&bytes[32..64]),
                       u64::from_be_bytes(bytes[56..64].try_into().unwrap_or([0u8; 8])));

            if bytes.len() >= 256 {
                log::debug!("  First 256 bytes hex dump:");
                for (i, chunk) in bytes[..256].chunks(32).enumerate() {
                    log::debug!("    [{:04x}] {}", i * 32, hex::encode(chunk));
                }
            }
        }

        // Parse ABI-encoded Payment[] array using helper function
        let payments = parse_abi_encoded_payments(&bytes)?;

        // Convert to PendingUnwrap
        let result: Vec<PendingUnwrap> = payments.into_iter().map(|payment| {
            // Convert recipient bytes to address
            let recipient_script = bitcoin::ScriptBuf::from_bytes(payment.recipient.clone());
            let address = bitcoin::Address::from_script(&recipient_script, network)
                .ok()
                .map(|a| a.to_string());

            // txid is already in display order (same as UTXO set), no need to reverse
            let txid = payment.txid;

            log::debug!("[Brc20ProgUnwrap] Parsed payment: txid={}, vout={}, value={}, height={}, recipient={:?}",
                       hex::encode(&txid[..8]), payment.vout, payment.value, payment.height, address);

            PendingUnwrap {
                txid: hex::encode(&txid),
                vout: payment.vout,
                amount: payment.value,
                address,
                fulfilled: false,
            }
        }).collect();

        log::info!("[Brc20ProgUnwrap] ✅ Parsed {} payments from bytecode execution (single eth_call!)", result.len());
        Ok(result)
    }

    /// Solidity implementation - uses pre-compiled FrBTCQuery.sol bytecode
    pub async fn get_pending_unwraps_experimental_sol(
        &self,
        provider: &dyn DeezelProvider,
        confirmations_required: u64,
        frbtc_address_override: Option<&str>,
        block_tag: Option<&str>,
    ) -> Result<Vec<PendingUnwrap>> {
        use crate::brc20_prog::generate_frbtc_query_bytecode;

        log::info!("[Brc20ProgUnwrap] 🔬 Using experimental Solidity-compiled bytecode");

        // Get current height
        let current_height = provider.get_block_count().await?;
        let cutoff_height = if current_height >= confirmations_required {
            current_height - confirmations_required
        } else {
            0
        };

        // Get FrBTC contract address (use override if provided)
        let network = provider.get_network();
        let frbtc_address = frbtc_address_override
            .map(|s| s.to_string())
            .unwrap_or_else(|| get_frbtc_address(network).to_string());
        let brc20_prog_rpc_url = provider.get_brc20_prog_rpc_url()
            .ok_or_else(|| AlkanesError::Configuration("brc20_prog_rpc_url not configured".to_string()))?;

        // Get signer address and oldest UTXO for filtering
        let signer_script = get_signer_address(
            provider as &dyn JsonRpcProvider,
            &brc20_prog_rpc_url,
            &frbtc_address,
        ).await?;
        let signer_address = script_pubkey_to_address(&signer_script, network)?;
        let utxos_json = provider.get_address_utxo(&signer_address).await?;
        let oldest_utxo_height = find_oldest_546_sat_utxo(&utxos_json)?.unwrap_or(0);

        log::info!("[Brc20ProgUnwrap] Generating Solidity bytecode for frbtc={}, cutoff={}, oldest={}",
                   frbtc_address, cutoff_height, oldest_utxo_height);

        // Generate the bytecode using pre-compiled Solidity contract with filtering
        let bytecode = generate_frbtc_query_bytecode(
            &frbtc_address,
            cutoff_height,
            oldest_utxo_height,
        )?;

        log::debug!("[Brc20ProgUnwrap] Generated {} bytes of bytecode", bytecode.len() / 2);

        // First, estimate gas to see what we actually need
        let tag = block_tag.unwrap_or("latest");
        let estimate_params = serde_json::json!([{
            "data": bytecode.clone()
        }, tag]);

        log::info!("[Brc20ProgUnwrap] Estimating gas for Solidity bytecode...");
        let gas_estimate = provider.call(&brc20_prog_rpc_url, "eth_estimateGas", estimate_params, 1).await;

        let estimated_gas = match gas_estimate {
            Ok(estimate) => {
                // Parse the hex value
                let gas_hex = estimate.as_str()
                    .and_then(|s| s.strip_prefix("0x").or(Some(s)))
                    .unwrap_or("0");
                let gas_decimal = u64::from_str_radix(gas_hex, 16).unwrap_or(0);
                log::info!("[Brc20ProgUnwrap] ✅ Gas estimate successful: {} ({} gas)", estimate, gas_decimal);
                Some(gas_decimal)
            }
            Err(e) => {
                log::warn!("[Brc20ProgUnwrap] ⚠️  Gas estimation failed (will try with high limit): {}", e);
                None
            }
        };

        // Use estimated gas + 50% buffer, or fallback to very high limit
        // Maximum safe value is 0x5000000 (83,886,080 gas)
        let gas_limit = if let Some(est) = estimated_gas {
            let with_buffer = (est * 3) / 2;  // 150% of estimate
            let capped = with_buffer.min(0x5000000);
            format!("0x{:x}", capped)
        } else {
            "0x5000000".to_string()  // 83,886,080 gas - very high fallback
        };

        log::info!("[Brc20ProgUnwrap] Using gas limit: {} ({} gas)",
                   gas_limit,
                   u64::from_str_radix(gas_limit.strip_prefix("0x").unwrap_or(&gas_limit), 16).unwrap_or(0));

        // Make single eth_call with generated bytecode and calculated gas limit
        let params = serde_json::json!([{
            "data": bytecode,
            "gas": gas_limit
        }, tag]);

        log::info!("[Brc20ProgUnwrap] Making single eth_call with Solidity bytecode (block_tag={}, gas={})...", tag, gas_limit);
        let response = provider.call(&brc20_prog_rpc_url, "eth_call", params, 1).await?;

        // Parse response - ABI-encoded Payment[] array
        let hex_stripped = response.as_str()
            .ok_or_else(|| AlkanesError::RpcError("eth_call result is not a string".to_string()))?
            .strip_prefix("0x").unwrap_or(response.as_str().unwrap());

        let bytes = hex::decode(hex_stripped)
            .map_err(|e| AlkanesError::RpcError(format!("Failed to decode response: {}", e)))?;

        log::info!("[Brc20ProgUnwrap SOL] Received {} bytes, parsing ABI-encoded Payment[] array", bytes.len());

        // DEBUG: Dump raw response hex for comparison
        log::debug!("[Brc20ProgUnwrap SOL] Raw response hex (first 1024 chars): {}",
                   if hex_stripped.len() > 1024 { &hex_stripped[..1024] } else { hex_stripped });
        log::debug!("[Brc20ProgUnwrap SOL] Full response length: {} hex chars = {} bytes",
                   hex_stripped.len(), bytes.len());

        // DEBUG: Detailed hex dump of response structure
        if bytes.len() >= 64 {
            log::debug!("[Brc20ProgUnwrap SOL] Response structure analysis:");
            log::debug!("  [0x00-0x20] Array offset:  0x{}", hex::encode(&bytes[0..32]));
            log::debug!("  [0x20-0x40] Array length:  0x{} = {} items",
                       hex::encode(&bytes[32..64]),
                       u64::from_be_bytes(bytes[56..64].try_into().unwrap_or([0u8; 8])));

            if bytes.len() >= 256 {
                log::debug!("  First 256 bytes hex dump:");
                for (i, chunk) in bytes[..256].chunks(32).enumerate() {
                    log::debug!("    [{:04x}] {}", i * 32, hex::encode(chunk));
                }
            }
        }

        // Parse ABI-encoded Payment[] array - Solidity already filtered by cutoff/oldest height
        let payments = parse_abi_encoded_payments_sol(&bytes)?;

        log::info!("[Brc20ProgUnwrap] Parsed {} filtered payments from Solidity bytecode (already filtered in contract)", payments.len());

        // Convert to PendingUnwrap - no additional filtering needed, Solidity did it
        let result: Vec<PendingUnwrap> = payments.into_iter().map(|payment| {
            // Convert recipient bytes to address
            let recipient_script = bitcoin::ScriptBuf::from_bytes(payment.recipient.clone());
            let address = bitcoin::Address::from_script(&recipient_script, network)
                .ok()
                .map(|a| a.to_string());

            // txid is already in display order from Solidity
            let txid = payment.txid;

            log::debug!("[Brc20ProgUnwrap] Parsed payment: txid={}, vout={}, value={}, height={}, recipient={:?}",
                       hex::encode(&txid[..8]), payment.vout, payment.value, payment.height, address);

            PendingUnwrap {
                txid: hex::encode(&txid),
                vout: payment.vout,
                amount: payment.value,
                address,
                fulfilled: false,
            }
        }).collect();

        log::info!("[Brc20ProgUnwrap] ✅ Parsed {} payments from Solidity bytecode execution (single eth_call!)", result.len());
        Ok(result)
    }
}

/// Parse ABI-encoded Payment[] array from Solidity bytecode execution result
/// Unlike the ASM version, Solidity returns in forward order (oldest first)
fn parse_abi_encoded_payments_sol(bytes: &[u8]) -> Result<Vec<crate::brc20_prog::Payment>> {
    if bytes.is_empty() {
        log::debug!("[parse_abi_encoded_payments_sol] Empty response, returning empty array");
        return Ok(vec![]);
    }

    log::debug!("[parse_abi_encoded_payments_sol SOL] Response length: {} bytes", bytes.len());

    // Debug: Print the first 512 bytes as hex dump to analyze structure
    if bytes.len() >= 512 {
        log::debug!("[parse_abi_encoded_payments_sol SOL] First 512 bytes hex dump:");
        for (i, chunk) in bytes[..512].chunks(32).enumerate() {
            log::debug!("  {:04x}: {}", i * 32, hex::encode(chunk));
        }
    } else if bytes.len() > 0 {
        log::debug!("[parse_abi_encoded_payments_sol SOL] Complete hex dump ({} bytes):", bytes.len());
        for (i, chunk) in bytes.chunks(32).enumerate() {
            log::debug!("  {:04x}: {}", i * 32, hex::encode(chunk));
        }
    }

    // Use alloy's ABI decoder
    log::debug!("[parse_abi_encoded_payments_sol SOL] Attempting to decode with alloy...");
    let payments = match try_alloy_decode(bytes) {
        Ok(p) => {
            log::debug!("[parse_abi_encoded_payments_sol SOL] Successfully decoded {} payments", p.len());
            p
        }
        Err(e) => {
            log::error!("[parse_abi_encoded_payments_sol SOL] DECODE FAILED: {}", e);
            log::error!("[parse_abi_encoded_payments_sol SOL] Dumping full response for analysis:");
            log::error!("  Full hex: 0x{}", hex::encode(bytes));
            return Err(AlkanesError::RpcError(format!("Failed to decode Payment[] ABI from Solidity: {}", e)));
        }
    };

    // Note: Unlike ASM version, Solidity returns in forward order, so we don't reverse
    log::info!("[parse_abi_encoded_payments_sol] Decoded {} payments via alloy (forward order)", payments.len());
    Ok(payments)
}

/// Convert p2tr script_pubkey bytes to Bitcoin address
pub fn script_pubkey_to_address(script: &[u8], network: bitcoin::Network) -> Result<String> {
    let script_buf = bitcoin::ScriptBuf::from_bytes(script.to_vec());
    let address = bitcoin::Address::from_script(&script_buf, network)
        .map_err(|e| AlkanesError::AddressResolution(format!("Failed to convert script to address: {}", e)))?;
    Ok(address.to_string())
}

/// Find the oldest 546 sat UTXO from Esplora UTXO response
pub fn find_oldest_546_sat_utxo(utxos_json: &serde_json::Value) -> Result<Option<u64>> {
    let utxos = utxos_json.as_array()
        .ok_or_else(|| AlkanesError::RpcError("UTXOs response is not an array".to_string()))?;
    
    let mut oldest_height: Option<u64> = None;
    
    for utxo in utxos {
        let value = utxo["value"].as_u64()
            .ok_or_else(|| AlkanesError::RpcError("UTXO missing value field".to_string()))?;
        
        // Consider dust UTXOs: 546 sats (legacy/P2PKH dust limit) or 330 sats (P2TR dust limit)
        if value <= 546 {
            if let Some(status) = utxo["status"].as_object() {
                if let Some(height) = status.get("block_height").and_then(|h| h.as_u64()) {
                    oldest_height = match oldest_height {
                        None => Some(height),
                        Some(current_oldest) => Some(current_oldest.min(height)),
                    };
                }
            }
        }
    }
    
    Ok(oldest_height)
}

/// Convert Payment struct to PendingUnwrap
pub fn payment_to_pending_unwrap(payment: crate::brc20_prog::Payment, network: bitcoin::Network) -> Result<PendingUnwrap> {
    // Convert txid bytes to hex string
    // Bitcoin txids are displayed in reverse byte order (little-endian display)
    let mut txid_bytes = payment.txid;
    txid_bytes.reverse();
    let txid = hex::encode(txid_bytes);

    // Convert recipient bytes to address
    let recipient_script = bitcoin::ScriptBuf::from_bytes(payment.recipient.clone());
    let address = bitcoin::Address::from_script(&recipient_script, network)
        .ok()
        .map(|a| a.to_string());

    Ok(PendingUnwrap {
        txid,
        vout: payment.vout,
        amount: payment.value,
        address,
        fulfilled: false, // BRC20-Prog doesn't track fulfilled status in the same way
    })
}

/// Parse ABI-encoded Payment[] array from bytecode execution result
///
/// The bytecode returns data in standard ABI format:
/// [offset_to_array=0x20][array_length][offset0][offset1]...[struct0][struct1]...
///
/// Each struct has the format:
/// [txid(32)][vout(32)][value(32)][offset_to_recipient(32)][height(32)][recipient_len(32)][recipient_data...]
pub fn parse_abi_encoded_payments(bytes: &[u8]) -> Result<Vec<crate::brc20_prog::Payment>> {
    if bytes.is_empty() {
        log::debug!("[parse_abi_encoded_payments] Empty response, returning empty array");
        return Ok(vec![]);
    }

    log::debug!("[parse_abi_encoded_payments ASM] Response length: {} bytes", bytes.len());

    // Debug: Print the first 512 bytes as hex dump to analyze structure
    if bytes.len() >= 512 {
        log::debug!("[parse_abi_encoded_payments ASM] First 512 bytes hex dump:");
        for (i, chunk) in bytes[..512].chunks(32).enumerate() {
            log::debug!("  {:04x}: {}", i * 32, hex::encode(chunk));
        }
    } else if bytes.len() > 0 {
        log::debug!("[parse_abi_encoded_payments ASM] Complete hex dump ({} bytes):", bytes.len());
        for (i, chunk) in bytes.chunks(32).enumerate() {
            log::debug!("  {:04x}: {}", i * 32, hex::encode(chunk));
        }
    }

    // Use alloy's ABI decoder
    log::debug!("[parse_abi_encoded_payments ASM] Attempting to decode with alloy...");
    let mut payments = match try_alloy_decode(bytes) {
        Ok(p) => {
            log::debug!("[parse_abi_encoded_payments ASM] Successfully decoded {} payments", p.len());
            p
        }
        Err(e) => {
            log::error!("[parse_abi_encoded_payments ASM] DECODE FAILED: {}", e);
            log::error!("[parse_abi_encoded_payments ASM] Dumping full response for analysis:");
            log::error!("  Full hex: 0x{}", hex::encode(bytes));
            return Err(e);
        }
    };

    // Reverse the payments to get chronological order (bytecode iterates backwards)
    payments.reverse();

    log::info!("[parse_abi_encoded_payments] Decoded {} payments via alloy (reversed to chronological order)", payments.len());
    Ok(payments)
}

/// Try to decode using alloy's ABI decoder
fn try_alloy_decode(bytes: &[u8]) -> Result<Vec<crate::brc20_prog::Payment>> {
    use alloy_sol_types::SolValue;
    use crate::brc20_prog::eth_call::IFrBTC;

    let decoded: Vec<IFrBTC::Payment> = Vec::<IFrBTC::Payment>::abi_decode(bytes, false)
        .map_err(|e| AlkanesError::RpcError(format!("Alloy decode failed: {}", e)))?;

    let payments: Vec<crate::brc20_prog::Payment> = decoded
        .into_iter()
        .map(|p| {
            let mut txid = [0u8; 32];
            txid.copy_from_slice(p.txid.as_slice());

            crate::brc20_prog::Payment {
                txid,
                vout: p.vout.try_into().unwrap_or(0),
                value: p.value.try_into().unwrap_or(0),
                recipient: p.recipient.to_vec(),
                height: p.height.try_into().unwrap_or(0),
            }
        })
        .collect();

    Ok(payments)
}
