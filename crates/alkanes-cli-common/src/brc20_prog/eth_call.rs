//! Ethereum call helpers for BRC20-Prog contracts
//!
//! Provides utilities for making eth_call queries to BRC20-Prog contracts using alloy-rs

use crate::{AlkanesError, Result};
use crate::traits::JsonRpcProvider;
use alloy_sol_types::{SolCall, SolValue};
use alloy_primitives::{U256, Bytes as AlloyBytes};

/// RPC call structure for eth_call
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Brc20ProgRpcCall {
    pub to: Option<String>,
    pub data: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gas: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gas_price: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

/// Define FrBTC contract interface using alloy sol! macro
alloy_sol_types::sol! {
    /// FrBTC contract interface
    interface IFrBTC {
        /// Payment struct from FrBTC.sol
        struct Payment {
            bytes32 txid;
            uint256 vout;
            uint256 value;
            bytes recipient;
            uint256 height;
        }
        
        /// Get the length of the payments array
        function getPaymentsLength() external view returns (uint256);
        
        /// Get the signer address (p2tr script_pubkey)
        function getSignerAddress() external view returns (bytes memory);
        
        /// Get a payment by index
        function payments(uint256 index) external view returns (Payment memory);
    }
}

/// Execute an eth_call to a BRC20-Prog contract
///
/// # Arguments
/// * `provider` - RPC provider
/// * `rpc_url` - BRC20-Prog RPC URL
/// * `contract_address` - Contract address (0x prefixed)
/// * `calldata` - Hex-encoded calldata (0x prefixed)
///
/// # Returns
/// * Hex-encoded return data (0x prefixed)
pub async fn eth_call(
    provider: &dyn JsonRpcProvider,
    rpc_url: &str,
    contract_address: &str,
    calldata: &str,
) -> Result<String> {
    let call = Brc20ProgRpcCall {
        to: Some(contract_address.to_string()),
        data: Some(calldata.to_string()),
        from: None,
        gas: None,
        gas_price: None,
        value: None,
    };
    
    let params = serde_json::json!([call, "latest"]);
    let response = provider.call(rpc_url, "eth_call", params, 1).await?;
    
    response.as_str()
        .ok_or_else(|| AlkanesError::RpcError("eth_call response is not a string".to_string()))
        .map(|s| s.to_string())
}

/// Get payments length from FrBTC contract
pub async fn get_payments_length(
    provider: &dyn JsonRpcProvider,
    rpc_url: &str,
    contract_address: &str,
) -> Result<u64> {
    let call = IFrBTC::getPaymentsLengthCall {};
    let calldata = hex::encode(call.abi_encode());
    
    let response = eth_call(provider, rpc_url, contract_address, &format!("0x{}", calldata)).await?;
    
    let hex_stripped = response.strip_prefix("0x").unwrap_or(&response);
    let bytes = hex::decode(hex_stripped)
        .map_err(|e| AlkanesError::RpcError(format!("Failed to decode response: {}", e)))?;
    
    let length = U256::abi_decode(&bytes, true)
        .map_err(|e| AlkanesError::RpcError(format!("Failed to decode uint256: {}", e)))?;
    
    Ok(length.to::<u64>())
}

/// Get signer address (p2tr script_pubkey) from FrBTC contract
pub async fn get_signer_address(
    provider: &dyn JsonRpcProvider,
    rpc_url: &str,
    contract_address: &str,
) -> Result<Vec<u8>> {
    let call = IFrBTC::getSignerAddressCall {};
    let calldata = hex::encode(call.abi_encode());
    
    let response = eth_call(provider, rpc_url, contract_address, &format!("0x{}", calldata)).await?;
    
    let hex_stripped = response.strip_prefix("0x").unwrap_or(&response);
    let bytes = hex::decode(hex_stripped)
        .map_err(|e| AlkanesError::RpcError(format!("Failed to decode response: {}", e)))?;
    
    let alloy_bytes = AlloyBytes::abi_decode(&bytes, true)
        .map_err(|e| AlkanesError::RpcError(format!("Failed to decode bytes: {}", e)))?;
    
    Ok(alloy_bytes.to_vec())
}

/// Get a payment by index from FrBTC contract
pub async fn get_payment(
    provider: &dyn JsonRpcProvider,
    rpc_url: &str,
    contract_address: &str,
    index: u64,
) -> Result<Payment> {
    let call = IFrBTC::paymentsCall {
        index: U256::from(index),
    };
    let calldata = hex::encode(call.abi_encode());
    
    let response = eth_call(provider, rpc_url, contract_address, &format!("0x{}", calldata)).await?;
    
    let hex_stripped = response.strip_prefix("0x").unwrap_or(&response);
    let bytes = hex::decode(hex_stripped)
        .map_err(|e| AlkanesError::RpcError(format!("Failed to decode response hex: {}", e)))?;
    
    log::debug!("[get_payment] Response length: {} bytes", bytes.len());
    
    // Manual ABI decoding since alloy has issues with this struct format
    // The response format for a struct with dynamic bytes is:
    // [txid(32)] [vout(32)] [value(32)] [offset_to_recipient(32)] [height(32)] [recipient_length(32)] [recipient_data...] [padding]
    
    if bytes.len() < 192 {
        return Err(AlkanesError::RpcError(format!("Response too short: {} bytes", bytes.len())));
    }
    
    // Parse fixed fields
    let mut txid = [0u8; 32];
    txid.copy_from_slice(&bytes[0..32]);
    
    let vout = U256::from_be_slice(&bytes[32..64]).to::<u32>();
    let value = U256::from_be_slice(&bytes[64..96]).to::<u64>();
    let recipient_offset = U256::from_be_slice(&bytes[96..128]).to::<usize>();
    let height = U256::from_be_slice(&bytes[128..160]).to::<u64>();
    
    // Parse dynamic bytes field
    if bytes.len() < recipient_offset + 32 {
        return Err(AlkanesError::RpcError(format!(
            "Response too short for recipient at offset {}: {} bytes",
            recipient_offset, bytes.len()
        )));
    }
    
    let recipient_length = U256::from_be_slice(&bytes[recipient_offset..recipient_offset + 32]).to::<usize>();
    let recipient_start = recipient_offset + 32;
    let recipient_end = recipient_start + recipient_length;
    
    if bytes.len() < recipient_end {
        return Err(AlkanesError::RpcError(format!(
            "Response too short for recipient data: need {}, have {} bytes",
            recipient_end, bytes.len()
        )));
    }
    
    let recipient = bytes[recipient_start..recipient_end].to_vec();
    
    log::debug!("[get_payment] Decoded: vout={}, value={}, height={}, recipient_len={}", 
        vout, value, height, recipient.len());
    
    Ok(Payment {
        txid,
        vout,
        value,
        recipient,
        height,
    })
}

/// Payment struct matching FrBTC.sol
#[derive(Debug, Clone)]
pub struct Payment {
    pub txid: [u8; 32],
    pub vout: u32,
    pub value: u64,
    pub recipient: Vec<u8>,
    pub height: u64,
}
