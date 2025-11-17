//! BRC20-Prog RPC client for interacting with brc20-programmable-module
//!
//! This module provides a JSON-RPC client for querying the brc20-programmable-module
//! which implements Ethereum-compatible RPC methods like eth_getCode, eth_call, etc.

use crate::{AlkanesError, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[cfg(not(target_arch = "wasm32"))]
use alloc::string::{String, ToString};
#[cfg(not(target_arch = "wasm32"))]
use alloc::vec::Vec;

/// BRC20-Prog RPC client
#[derive(Clone, Debug)]
pub struct Brc20ProgRpcClient {
    url: String,
    client: reqwest::Client,
}

/// eth_call parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EthCallParams {
    pub to: String,
    pub data: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
}

impl Brc20ProgRpcClient {
    /// Create a new BRC20-Prog RPC client
    pub fn new(url: String) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| AlkanesError::Network(e.to_string()))?;
        
        Ok(Self { url, client })
    }

    /// Make a JSON-RPC call
    async fn call(&self, method: &str, params: Value) -> Result<Value> {
        let request_body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params
        });

        log::debug!("BRC20-Prog RPC request: {} -> {}", method, request_body);

        let response = self
            .client
            .post(&self.url)
            .json(&request_body)
            .send()
            .await
            .map_err(|e| AlkanesError::Network(format!("HTTP request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(AlkanesError::Network(format!(
                "HTTP error: {}",
                response.status()
            )));
        }

        let response_text = response
            .text()
            .await
            .map_err(|e| AlkanesError::Network(format!("Failed to read response: {}", e)))?;

        log::debug!("BRC20-Prog RPC response: {}", response_text);

        let response_json: serde_json::Value = serde_json::from_str(&response_text)
            .map_err(|e| AlkanesError::Parse(format!("Invalid JSON response: {}", e)))?;

        if let Some(error) = response_json.get("error") {
            return Err(AlkanesError::Rpc(format!("RPC error: {}", error)));
        }

        response_json
            .get("result")
            .cloned()
            .ok_or_else(|| AlkanesError::Rpc("Missing result field".to_string()))
    }

    /// Get contract bytecode (eth_getCode)
    pub async fn eth_get_code(&self, address: &str) -> Result<String> {
        // Ensure address is properly formatted
        let address = if address.starts_with("0x") {
            address.to_string()
        } else {
            format!("0x{}", address)
        };

        let result = self
            .call("eth_getCode", serde_json::json!([{"address": address}]))
            .await?;

        result
            .get("code")
            .or_else(|| result.as_str().map(|s| &serde_json::Value::String(s.to_string())))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| AlkanesError::Parse("Invalid eth_getCode response".to_string()))
    }

    /// Call a contract function (eth_call)
    pub async fn eth_call(&self, params: EthCallParams, block: Option<&str>) -> Result<String> {
        let call_params = serde_json::json!({
            "to": params.to,
            "data": params.data,
            "from": params.from
        });

        let rpc_params = if let Some(block_num) = block {
            serde_json::json!([call_params, block_num])
        } else {
            serde_json::json!([call_params])
        };

        let result = self.call("eth_call", rpc_params).await?;

        result
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| AlkanesError::Parse("Invalid eth_call response".to_string()))
    }

    /// Get balance (eth_getBalance)
    pub async fn eth_get_balance(&self, address: &str, block: &str) -> Result<String> {
        // Ensure address is properly formatted
        let address = if address.starts_with("0x") {
            address.to_string()
        } else {
            format!("0x{}", address)
        };

        let result = self
            .call("eth_getBalance", serde_json::json!([{"address": address}, block]))
            .await?;

        result
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| AlkanesError::Parse("Invalid eth_getBalance response".to_string()))
    }

    /// Get inscription ID by contract address
    pub async fn brc20_get_inscription_id_by_contract_address(
        &self,
        contract_address: &str,
    ) -> Result<Option<String>> {
        // Ensure address is properly formatted
        let address = if contract_address.starts_with("0x") {
            contract_address.to_string()
        } else {
            format!("0x{}", contract_address)
        };

        let result = self
            .call(
                "brc20_getInscriptionIdByContractAddress",
                serde_json::json!([{"address": address}]),
            )
            .await?;

        Ok(result.as_str().map(|s| s.to_string()))
    }

    /// Get contract address by inscription ID
    pub async fn brc20_get_contract_address_by_inscription_id(
        &self,
        inscription_id: &str,
    ) -> Result<Option<String>> {
        let result = self
            .call(
                "brc20_getContractAddressByInscriptionId",
                serde_json::json!([inscription_id]),
            )
            .await?;

        Ok(result.as_str().map(|s| s.to_string()))
    }

    /// Get current block number (eth_blockNumber)
    pub async fn eth_block_number(&self) -> Result<u64> {
        let result = self.call("eth_blockNumber", serde_json::json!([])).await?;

        let block_hex = result
            .as_str()
            .ok_or_else(|| AlkanesError::Parse("Invalid eth_blockNumber response".to_string()))?;

        let block_hex = block_hex.trim_start_matches("0x");
        u64::from_str_radix(block_hex, 16)
            .map_err(|e| AlkanesError::Parse(format!("Invalid block number: {}", e)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eth_call_params_serialization() {
        let params = EthCallParams {
            to: "0x1234".to_string(),
            data: "0xabcd".to_string(),
            from: Some("0x5678".to_string()),
        };

        let json = serde_json::to_value(&params).unwrap();
        assert_eq!(json["to"], "0x1234");
        assert_eq!(json["data"], "0xabcd");
        assert_eq!(json["from"], "0x5678");
    }
}
