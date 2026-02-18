//! OP_NET client for querying opshrew-indexed state via metashrew_view.
//!
//! Each method corresponds to an opshrew WASM view function export
//! and maps to a canonical OP_NET JSON-RPC method (btc_*).

use crate::provider::ConcreteProvider;
use crate::Result;
use super::types::*;

/// Client for OP_NET opshrew view functions.
///
/// Uses `metashrew_view_call` under the hood, encoding parameters
/// as raw bytes (hex) matching opshrew's binary view protocol.
pub struct OpnetClient<'a> {
    provider: &'a ConcreteProvider,
}

impl<'a> OpnetClient<'a> {
    pub fn new(provider: &'a ConcreteProvider) -> Self {
        Self { provider }
    }

    // ── Block methods ──────────────────────────────────────────────────

    /// btc_blockNumber — get latest indexed block height.
    pub async fn block_number(&self) -> Result<u32> {
        let bytes = self.provider.metashrew_view_call("blocknumber", "", "latest").await?;
        if bytes.len() >= 4 {
            Ok(u32::from_le_bytes(bytes[..4].try_into().unwrap()))
        } else {
            Ok(0)
        }
    }

    /// btc_getBlockByNumber — get block info by height.
    pub async fn get_block_by_number(&self, height: u32) -> Result<Option<OpnetBlockInfo>> {
        let params = hex::encode(height.to_le_bytes());
        let bytes = self.provider.metashrew_view_call("getblockbynumber", &params, "latest").await?;
        if bytes.len() < 56 {
            return Ok(None);
        }
        Ok(Some(decode_block_by_number(&bytes, height)))
    }

    /// btc_getBlockByHash — get block info by hash.
    pub async fn get_block_by_hash(&self, hash: &[u8; 32]) -> Result<Option<OpnetBlockInfo>> {
        let params = hex::encode(hash);
        let bytes = self.provider.metashrew_view_call("getblockbyhash", &params, "latest").await?;
        if bytes.len() < 28 {
            return Ok(None);
        }
        Ok(Some(decode_block_by_hash(&bytes, hash)))
    }

    // ── State methods ──────────────────────────────────────────────────

    /// btc_getStorageAt — get contract storage slot value.
    pub async fn get_storage_at(&self, contract_address: &[u8; 32], key: &[u8; 32]) -> Result<[u8; 32]> {
        let mut params_bytes = Vec::with_capacity(64);
        params_bytes.extend_from_slice(contract_address);
        params_bytes.extend_from_slice(key);
        let params = hex::encode(&params_bytes);
        let bytes = self.provider.metashrew_view_call("getstorageat", &params, "latest").await?;
        let mut result = [0u8; 32];
        if bytes.len() >= 32 {
            result.copy_from_slice(&bytes[..32]);
        }
        Ok(result)
    }

    /// btc_getCode — get contract bytecode.
    pub async fn get_code(&self, contract_address: &[u8; 32]) -> Result<Vec<u8>> {
        let params = hex::encode(contract_address);
        self.provider.metashrew_view_call("getcode", &params, "latest").await
    }

    /// btc_call — simulate contract execution (read-only).
    pub async fn call(&self, contract_address: &[u8; 32], calldata: &[u8]) -> Result<Vec<u8>> {
        let mut params_bytes = Vec::with_capacity(32 + calldata.len());
        params_bytes.extend_from_slice(contract_address);
        params_bytes.extend_from_slice(calldata);
        let params = hex::encode(&params_bytes);
        self.provider.metashrew_view_call("simulate", &params, "latest").await
    }

    // ── Transaction methods ────────────────────────────────────────────

    /// btc_getTransactionByHash — get transaction info.
    pub async fn get_transaction_by_hash(&self, tx_hash: &[u8; 32]) -> Result<Option<OpnetTxInfo>> {
        let params = hex::encode(tx_hash);
        let bytes = self.provider.metashrew_view_call("gettransactionbyhash", &params, "latest").await?;
        if bytes.len() < 9 {
            return Ok(None);
        }
        let height = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
        let tx_index = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
        let tx_type = OpnetTxType::from(bytes[8]);
        let contract_address = hex::encode(&bytes[9..]);
        Ok(Some(OpnetTxInfo {
            height,
            tx_index,
            tx_type,
            contract_address,
        }))
    }

    /// btc_getTransactionReceipt — get transaction receipt.
    pub async fn get_transaction_receipt(&self, tx_hash: &[u8; 32]) -> Result<Option<OpnetReceipt>> {
        let params = hex::encode(tx_hash);
        let bytes = self.provider.metashrew_view_call("gettransactionreceipt", &params, "latest").await?;
        if bytes.len() < 13 {
            return Ok(None);
        }
        let success = bytes[0] == 1;
        let gas_used = u64::from_le_bytes(bytes[1..9].try_into().unwrap());
        let exit_data_len = u32::from_le_bytes(bytes[9..13].try_into().unwrap()) as usize;
        let exit_data = if bytes.len() >= 13 + exit_data_len {
            bytes[13..13 + exit_data_len].to_vec()
        } else {
            Vec::new()
        };
        let event_count = if bytes.len() >= 13 + exit_data_len + 4 {
            u32::from_le_bytes(bytes[13 + exit_data_len..13 + exit_data_len + 4].try_into().unwrap())
        } else {
            0
        };
        Ok(Some(OpnetReceipt {
            success,
            gas_used,
            exit_data,
            event_count,
        }))
    }

    // ── Chain methods ──────────────────────────────────────────────────

    /// btc_chainId — get chain ID.
    pub async fn chain_id(&self) -> Result<Vec<u8>> {
        self.provider.metashrew_view_call("chainid", "", "latest").await
    }

    /// btc_gas — get gas information for a block.
    pub async fn gas(&self, height: Option<u32>) -> Result<OpnetGasInfo> {
        let params = match height {
            Some(h) => hex::encode(h.to_le_bytes()),
            None => String::new(),
        };
        let bytes = self.provider.metashrew_view_call("gas", &params, "latest").await?;
        if bytes.len() < 12 {
            return Ok(OpnetGasInfo {
                height: 0,
                gas_used: 0,
                block_hash: String::new(),
            });
        }
        let h = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
        let gas_used = u64::from_le_bytes(bytes[4..12].try_into().unwrap());
        let block_hash = if bytes.len() >= 44 {
            hex::encode(&bytes[12..44])
        } else {
            String::new()
        };
        Ok(OpnetGasInfo {
            height: h,
            gas_used,
            block_hash,
        })
    }

    // ── Contract enumeration ───────────────────────────────────────────

    /// Get deployer pubkey for a contract.
    pub async fn get_deployer(&self, contract_address: &[u8; 32]) -> Result<Vec<u8>> {
        let params = hex::encode(contract_address);
        self.provider.metashrew_view_call("getdeployer", &params, "latest").await
    }

    /// Get total number of deployed contracts.
    pub async fn contract_count(&self) -> Result<u64> {
        let bytes = self.provider.metashrew_view_call("contractcount", "", "latest").await?;
        if bytes.len() >= 8 {
            Ok(u64::from_le_bytes(bytes[..8].try_into().unwrap()))
        } else {
            Ok(0)
        }
    }

    /// Get contract address by list index.
    pub async fn contract_at_index(&self, index: u64) -> Result<Vec<u8>> {
        let params = hex::encode(index.to_le_bytes());
        self.provider.metashrew_view_call("contractatindex", &params, "latest").await
    }
}

// ── Native OP_NET JSON-RPC client ──────────────────────────────────────

/// Client for native OP_NET JSON-RPC endpoints (e.g. regtest.opnet.org).
///
/// Unlike [`OpnetClient`] which routes through metashrew view calls,
/// this client speaks standard JSON-RPC 2.0 directly to an OP_NET node.
#[cfg(feature = "std")]
pub struct NativeOpnetClient {
    url: String,
    client: reqwest::Client,
}

#[cfg(feature = "std")]
impl NativeOpnetClient {
    pub fn new(url: &str) -> Self {
        Self {
            url: url.to_string(),
            client: reqwest::Client::new(),
        }
    }

    async fn rpc_call(&self, method: &str, params: serde_json::Value) -> crate::Result<serde_json::Value> {
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params,
        });
        let resp = self.client.post(&self.url)
            .json(&body)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("OP_NET RPC request failed: {}", e))?;
        let json: serde_json::Value = resp.json().await
            .map_err(|e| anyhow::anyhow!("OP_NET RPC response parse failed: {}", e))?;
        if let Some(err) = json.get("error") {
            return Err(anyhow::anyhow!("OP_NET RPC error: {}", err));
        }
        Ok(json["result"].clone())
    }

    /// getBlockNumber — get latest block height from the OP_NET node.
    pub async fn block_number(&self) -> crate::Result<u32> {
        let result = self.rpc_call("getBlockNumber", serde_json::json!([])).await?;
        Ok(result.as_u64().unwrap_or(0) as u32)
    }

    /// getStorageAt — get contract storage slot value.
    pub async fn get_storage_at(&self, contract_address: &str, key: &str) -> crate::Result<String> {
        let result = self.rpc_call("getStorageAt", serde_json::json!([contract_address, key])).await?;
        Ok(result.as_str().unwrap_or("").to_string())
    }

    /// getCode — get contract bytecode.
    pub async fn get_code(&self, contract_address: &str) -> crate::Result<String> {
        let result = self.rpc_call("getCode", serde_json::json!([contract_address])).await?;
        Ok(result.as_str().unwrap_or("").to_string())
    }

    /// call — simulate contract execution (read-only).
    pub async fn call(&self, contract_address: &str, calldata: &str) -> crate::Result<String> {
        let result = self.rpc_call("call", serde_json::json!([contract_address, calldata])).await?;
        Ok(result.as_str().unwrap_or("").to_string())
    }

    /// getBalance — get balance for an address.
    pub async fn get_balance(&self, address: &str) -> crate::Result<String> {
        let result = self.rpc_call("getBalance", serde_json::json!([address])).await?;
        Ok(result.as_str().unwrap_or("").to_string())
    }

    /// sendRawTransaction — broadcast a raw transaction.
    pub async fn send_raw_transaction(&self, tx_hex: &str) -> crate::Result<String> {
        let result = self.rpc_call("sendRawTransaction", serde_json::json!([tx_hex])).await?;
        Ok(result.as_str().unwrap_or("").to_string())
    }

    /// getChainId — get the chain ID.
    pub async fn chain_id(&self) -> crate::Result<String> {
        let result = self.rpc_call("getChainId", serde_json::json!([])).await?;
        Ok(result.as_str().unwrap_or("").to_string())
    }
}

// ── Helpers ────────────────────────────────────────────────────────────

fn decode_block_by_number(bytes: &[u8], height: u32) -> OpnetBlockInfo {
    let hash = hex::encode(&bytes[0..32]);
    let timestamp = u64::from_le_bytes(bytes[32..40].try_into().unwrap());
    let tx_count = u32::from_le_bytes(bytes[40..44].try_into().unwrap());
    let opnet_tx_count = u32::from_le_bytes(bytes[44..48].try_into().unwrap());
    let gas_used = u64::from_le_bytes(bytes[48..56].try_into().unwrap());
    OpnetBlockInfo {
        height,
        hash,
        timestamp,
        tx_count,
        opnet_tx_count,
        gas_used,
    }
}

fn decode_block_by_hash(bytes: &[u8], hash: &[u8; 32]) -> OpnetBlockInfo {
    let height = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
    let timestamp = u64::from_le_bytes(bytes[4..12].try_into().unwrap());
    let tx_count = u32::from_le_bytes(bytes[12..16].try_into().unwrap());
    let opnet_tx_count = u32::from_le_bytes(bytes[16..20].try_into().unwrap());
    let gas_used = u64::from_le_bytes(bytes[20..28].try_into().unwrap());
    OpnetBlockInfo {
        height,
        hash: hex::encode(hash),
        timestamp,
        tx_count,
        opnet_tx_count,
        gas_used,
    }
}
