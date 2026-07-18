//! BRC20-Prog RPC client for interacting with brc20-programmable-module
//!
//! This module provides a comprehensive JSON-RPC client for querying the brc20-programmable-module
//! which implements Ethereum-compatible RPC methods

use crate::{AlkanesError, Result};
use crate::brc20_prog_rpc_types::*;
use serde_json::Value;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};

#[cfg(not(target_arch = "wasm32"))]
use alloc::string::{String, ToString};
#[cfg(not(target_arch = "wasm32"))]
use alloc::vec::Vec;

/// BRC20-Prog RPC client
#[derive(Clone, Debug)]
pub struct Brc20ProgRpcClient {
    url: String,
    client: reqwest::Client,
    headers: HeaderMap,
}

impl Brc20ProgRpcClient {
    /// Create a new BRC20-Prog RPC client
    pub fn new(url: String) -> Result<Self> {
        Self::with_headers(url, Vec::new())
    }

    /// Create a new BRC20-Prog RPC client with custom headers
    pub fn with_headers(url: String, headers: Vec<(String, String)>) -> Result<Self> {
        #[cfg(not(target_arch = "wasm32"))]
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| AlkanesError::Network(e.to_string()))?;

        #[cfg(target_arch = "wasm32")]
        let client = reqwest::Client::builder()
            .build()
            .map_err(|e| AlkanesError::Network(e.to_string()))?;

        let mut header_map = HeaderMap::new();
        for (name, value) in headers {
            let header_name = HeaderName::from_bytes(name.as_bytes())
                .map_err(|e| AlkanesError::Network(format!("Invalid header name '{}': {}", name, e)))?;
            let header_value = HeaderValue::from_str(&value)
                .map_err(|e| AlkanesError::Network(format!("Invalid header value '{}': {}", value, e)))?;
            header_map.insert(header_name, header_value);
        }

        Ok(Self { url, client, headers: header_map })
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

        let mut request = self
            .client
            .post(&self.url)
            .json(&request_body);

        // Add custom headers
        for (name, value) in self.headers.iter() {
            request = request.header(name.clone(), value.clone());
        }

        let response = request
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
            return Err(AlkanesError::Network(format!("RPC error: {}", error)));
        }

        response_json
            .get("result")
            .cloned()
            .ok_or_else(|| AlkanesError::Network("Missing result field".to_string()))
    }

    // ============================================================================
    // ETH JSON-RPC Methods
    // ============================================================================

    /// Get current block number (eth_blockNumber)
    pub async fn eth_block_number(&self) -> Result<String> {
        let result = self.call("eth_blockNumber", serde_json::json!([])).await?;
        result
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| AlkanesError::Parse("Invalid eth_blockNumber response".to_string()))
    }

    /// Get block by number (eth_getBlockByNumber)
    pub async fn eth_get_block_by_number(&self, block: &str, full_tx: bool) -> Result<BlockInfo> {
        let result = self
            .call("eth_getBlockByNumber", serde_json::json!([block, full_tx]))
            .await?;
        serde_json::from_value(result)
            .map_err(|e| AlkanesError::Parse(format!("Invalid block response: {}", e)))
    }

    /// Get block by hash (eth_getBlockByHash)
    pub async fn eth_get_block_by_hash(&self, hash: &str, full_tx: bool) -> Result<BlockInfo> {
        let result = self
            .call("eth_getBlockByHash", serde_json::json!([hash, full_tx]))
            .await?;
        serde_json::from_value(result)
            .map_err(|e| AlkanesError::Parse(format!("Invalid block response: {}", e)))
    }

    /// Get transaction count (eth_getTransactionCount / nonce)
    pub async fn eth_get_transaction_count(&self, address: &str, block: &str) -> Result<String> {
        let result = self
            .call("eth_getTransactionCount", serde_json::json!([address, block]))
            .await?;
        result
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| AlkanesError::Parse("Invalid transaction count response".to_string()))
    }

    /// Get block transaction count by number (eth_getBlockTransactionCountByNumber)
    pub async fn eth_get_block_transaction_count_by_number(&self, block: &str) -> Result<String> {
        let result = self
            .call("eth_getBlockTransactionCountByNumber", serde_json::json!([block]))
            .await?;
        result
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| AlkanesError::Parse("Invalid transaction count response".to_string()))
    }

    /// Get block transaction count by hash (eth_getBlockTransactionCountByHash)
    pub async fn eth_get_block_transaction_count_by_hash(&self, hash: &str) -> Result<String> {
        let result = self
            .call("eth_getBlockTransactionCountByHash", serde_json::json!([hash]))
            .await?;
        result
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| AlkanesError::Parse("Invalid transaction count response".to_string()))
    }

    /// Get contract bytecode (eth_getCode)
    pub async fn eth_get_code(&self, address: &str) -> Result<String> {
        let result = self
            .call("eth_getCode", serde_json::json!([address]))
            .await?;

        // Handle both string and object responses
        if let Some(code_val) = result.get("code") {
            code_val
                .as_str()
                .map(|s| s.to_string())
                .ok_or_else(|| AlkanesError::Parse("Invalid eth_getCode response".to_string()))
        } else {
            result
                .as_str()
                .map(|s| s.to_string())
                .ok_or_else(|| AlkanesError::Parse("Invalid eth_getCode response".to_string()))
        }
    }

    /// Call a contract function (eth_call)
    pub async fn eth_call(&self, params: EthCallParams, block: Option<&str>) -> Result<String> {
        let rpc_params = if let Some(block_num) = block {
            serde_json::json!([params, block_num])
        } else {
            serde_json::json!([params])
        };

        let result = self.call("eth_call", rpc_params).await?;
        result
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| AlkanesError::Parse("Invalid eth_call response".to_string()))
    }

    /// Estimate gas for a transaction (eth_estimateGas)
    pub async fn eth_estimate_gas(&self, params: EthCallParams, block: Option<&str>) -> Result<String> {
        let rpc_params = if let Some(block_num) = block {
            serde_json::json!([params, block_num])
        } else {
            serde_json::json!([params])
        };

        let result = self.call("eth_estimateGas", rpc_params).await?;
        result
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| AlkanesError::Parse("Invalid eth_estimateGas response".to_string()))
    }

    /// Get storage at a specific location (eth_getStorageAt)
    pub async fn eth_get_storage_at(&self, address: &str, position: &str) -> Result<String> {
        let result = self
            .call("eth_getStorageAt", serde_json::json!([address, position]))
            .await?;
        result
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| AlkanesError::Parse("Invalid eth_getStorageAt response".to_string()))
    }

    /// Get balance (eth_getBalance)
    pub async fn eth_get_balance(&self, address: &str, block: &str) -> Result<String> {
        let result = self
            .call("eth_getBalance", serde_json::json!([address, block]))
            .await?;
        result
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| AlkanesError::Parse("Invalid eth_getBalance response".to_string()))
    }

    /// Get transaction receipt (eth_getTransactionReceipt)
    pub async fn eth_get_transaction_receipt(&self, tx_hash: &str) -> Result<Option<TransactionReceipt>> {
        let result = self
            .call("eth_getTransactionReceipt", serde_json::json!([tx_hash]))
            .await?;
        
        if result.is_null() {
            return Ok(None);
        }

        serde_json::from_value(result)
            .map(Some)
            .map_err(|e| AlkanesError::Parse(format!("Invalid transaction receipt: {}", e)))
    }

    /// Get transaction by hash (eth_getTransactionByHash)
    pub async fn eth_get_transaction_by_hash(&self, tx_hash: &str) -> Result<Option<TransactionInfo>> {
        let result = self
            .call("eth_getTransactionByHash", serde_json::json!([tx_hash]))
            .await?;
        
        if result.is_null() {
            return Ok(None);
        }

        serde_json::from_value(result)
            .map(Some)
            .map_err(|e| AlkanesError::Parse(format!("Invalid transaction info: {}", e)))
    }

    /// Get transaction by block number and index (eth_getTransactionByBlockNumberAndIndex)
    pub async fn eth_get_transaction_by_block_number_and_index(
        &self,
        block: &str,
        index: u64,
    ) -> Result<Option<TransactionInfo>> {
        let result = self
            .call(
                "eth_getTransactionByBlockNumberAndIndex",
                serde_json::json!([block, format!("0x{:x}", index)]),
            )
            .await?;
        
        if result.is_null() {
            return Ok(None);
        }

        serde_json::from_value(result)
            .map(Some)
            .map_err(|e| AlkanesError::Parse(format!("Invalid transaction info: {}", e)))
    }

    /// Get transaction by block hash and index (eth_getTransactionByBlockHashAndIndex)
    pub async fn eth_get_transaction_by_block_hash_and_index(
        &self,
        hash: &str,
        index: u64,
    ) -> Result<Option<TransactionInfo>> {
        let result = self
            .call(
                "eth_getTransactionByBlockHashAndIndex",
                serde_json::json!([hash, format!("0x{:x}", index)]),
            )
            .await?;
        
        if result.is_null() {
            return Ok(None);
        }

        serde_json::from_value(result)
            .map(Some)
            .map_err(|e| AlkanesError::Parse(format!("Invalid transaction info: {}", e)))
    }

    /// Get logs (eth_getLogs)
    pub async fn eth_get_logs(&self, filter: GetLogsFilter) -> Result<Vec<LogEntry>> {
        let result = self
            .call("eth_getLogs", serde_json::json!([filter]))
            .await?;
        serde_json::from_value(result)
            .map_err(|e| AlkanesError::Parse(format!("Invalid logs response: {}", e)))
    }

    /// Get chain ID (eth_chainId)
    pub async fn eth_chain_id(&self) -> Result<String> {
        let result = self.call("eth_chainId", serde_json::json!([])).await?;
        result
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| AlkanesError::Parse("Invalid eth_chainId response".to_string()))
    }

    /// Get gas price (eth_gasPrice)
    pub async fn eth_gas_price(&self) -> Result<String> {
        let result = self.call("eth_gasPrice", serde_json::json!([])).await?;
        result
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| AlkanesError::Parse("Invalid eth_gasPrice response".to_string()))
    }

    /// Get max priority fee per gas (eth_maxPriorityFeePerGas)
    pub async fn eth_max_priority_fee_per_gas(&self) -> Result<String> {
        let result = self.call("eth_maxPriorityFeePerGas", serde_json::json!([])).await?;
        result
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| AlkanesError::Parse("Invalid eth_maxPriorityFeePerGas response".to_string()))
    }

    /// Get syncing status (eth_syncing)
    pub async fn eth_syncing(&self) -> Result<bool> {
        let result = self.call("eth_syncing", serde_json::json!([])).await?;
        result
            .as_bool()
            .ok_or_else(|| AlkanesError::Parse("Invalid eth_syncing response".to_string()))
    }

    /// Get accounts (eth_accounts)
    pub async fn eth_accounts(&self) -> Result<Vec<String>> {
        let result = self.call("eth_accounts", serde_json::json!([])).await?;
        serde_json::from_value(result)
            .map_err(|e| AlkanesError::Parse(format!("Invalid eth_accounts response: {}", e)))
    }

    // ============================================================================
    // BRC20-Prog Specific Methods
    // ============================================================================

    /// Get BRC20-Prog version (brc20_version)
    pub async fn brc20_version(&self) -> Result<String> {
        let result = self.call("brc20_version", serde_json::json!([])).await?;
        result
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| AlkanesError::Parse("Invalid brc20_version response".to_string()))
    }

    /// Get transaction receipt by inscription ID (brc20_getTxReceiptByInscriptionId)
    pub async fn brc20_get_tx_receipt_by_inscription_id(
        &self,
        inscription_id: &str,
    ) -> Result<Option<TransactionReceipt>> {
        let result = self
            .call(
                "brc20_getTxReceiptByInscriptionId",
                serde_json::json!([inscription_id]),
            )
            .await?;
        
        if result.is_null() {
            return Ok(None);
        }

        serde_json::from_value(result)
            .map(Some)
            .map_err(|e| AlkanesError::Parse(format!("Invalid transaction receipt: {}", e)))
    }

    /// Get inscription ID by transaction hash (brc20_getInscriptionIdByTxHash)
    pub async fn brc20_get_inscription_id_by_tx_hash(&self, tx_hash: &str) -> Result<Option<String>> {
        let result = self
            .call(
                "brc20_getInscriptionIdByTxHash",
                serde_json::json!([tx_hash]),
            )
            .await?;
        Ok(result.as_str().map(|s| s.to_string()))
    }

    /// Get inscription ID by contract address (brc20_getInscriptionIdByContractAddress)
    pub async fn brc20_get_inscription_id_by_contract_address(
        &self,
        contract_address: &str,
    ) -> Result<Option<String>> {
        let result = self
            .call(
                "brc20_getInscriptionIdByContractAddress",
                serde_json::json!([contract_address]),
            )
            .await?;
        Ok(result.as_str().map(|s| s.to_string()))
    }

    /// Get BRC20 balance (brc20_balance)
    pub async fn brc20_balance(&self, pkscript: &str, ticker: &str) -> Result<String> {
        let result = self
            .call("brc20_balance", serde_json::json!([pkscript, ticker]))
            .await?;
        result
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| AlkanesError::Parse("Invalid brc20_balance response".to_string()))
    }

    // ============================================================================
    // Debug Methods
    // ============================================================================

    /// Get transaction trace (debug_traceTransaction)
    pub async fn debug_trace_transaction(&self, tx_hash: &str) -> Result<Option<TraceInfo>> {
        let result = self
            .call("debug_traceTransaction", serde_json::json!([tx_hash]))
            .await?;
        
        if result.is_null() {
            return Ok(None);
        }

        serde_json::from_value(result)
            .map(Some)
            .map_err(|e| AlkanesError::Parse(format!("Invalid trace response: {}", e)))
    }

    /// Get raw header (debug_getRawHeader)
    pub async fn debug_get_raw_header(&self, block: &str) -> Result<Option<String>> {
        let result = self
            .call("debug_getRawHeader", serde_json::json!([block]))
            .await?;
        Ok(result.as_str().map(|s| s.to_string()))
    }

    /// Get raw block (debug_getRawBlock)
    pub async fn debug_get_raw_block(&self, block: &str) -> Result<Option<String>> {
        let result = self
            .call("debug_getRawBlock", serde_json::json!([block]))
            .await?;
        Ok(result.as_str().map(|s| s.to_string()))
    }

    /// Get raw receipts (debug_getRawReceipts)
    pub async fn debug_get_raw_receipts(&self, block: &str) -> Result<Option<Vec<String>>> {
        let result = self
            .call("debug_getRawReceipts", serde_json::json!([block]))
            .await?;
        
        if result.is_null() {
            return Ok(None);
        }

        serde_json::from_value(result)
            .map(Some)
            .map_err(|e| AlkanesError::Parse(format!("Invalid raw receipts response: {}", e)))
    }

    // ============================================================================
    // Txpool Methods
    // ============================================================================

    /// Get txpool content (txpool_content)
    pub async fn txpool_content(&self) -> Result<TxpoolContent> {
        let result = self.call("txpool_content", serde_json::json!([])).await?;
        serde_json::from_value(result)
            .map_err(|e| AlkanesError::Parse(format!("Invalid txpool_content response: {}", e)))
    }

    /// Get txpool content from address (txpool_contentFrom)
    pub async fn txpool_content_from(&self, address: &str) -> Result<TxpoolContent> {
        let result = self
            .call("txpool_contentFrom", serde_json::json!([address]))
            .await?;
        serde_json::from_value(result)
            .map_err(|e| AlkanesError::Parse(format!("Invalid txpool_contentFrom response: {}", e)))
    }

    // ============================================================================
    // Web3 Methods
    // ============================================================================

    /// Get client version (web3_clientVersion)
    pub async fn web3_client_version(&self) -> Result<String> {
        let result = self.call("web3_clientVersion", serde_json::json!([])).await?;
        result
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| AlkanesError::Parse("Invalid web3_clientVersion response".to_string()))
    }

    /// Calculate keccak256 hash (web3_sha3)
    pub async fn web3_sha3(&self, data: &str) -> Result<String> {
        let result = self
            .call("web3_sha3", serde_json::json!([data]))
            .await?;
        result
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| AlkanesError::Parse("Invalid web3_sha3 response".to_string()))
    }

    /// Get network version (net_version)
    pub async fn net_version(&self) -> Result<String> {
        let result = self.call("net_version", serde_json::json!([])).await?;
        result
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| AlkanesError::Parse("Invalid net_version response".to_string()))
    }
}
