use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::config::Config;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlkaneId {
    pub block: String,
    pub tx: String,
}

impl AlkaneId {
    pub fn serialize(&self) -> String {
        format!("{}:{}", self.block, self.tx)
    }

    pub fn parse(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() == 2 {
            Some(AlkaneId {
                block: parts[0].to_string(),
                tx: parts[1].to_string(),
            })
        } else {
            None
        }
    }
}

impl From<&crate::models::AlkaneId> for AlkaneId {
    fn from(id: &crate::models::AlkaneId) -> Self {
        AlkaneId {
            block: id.block.clone(),
            tx: id.tx.clone(),
        }
    }
}

impl From<&crate::models::PoolId> for AlkaneId {
    fn from(id: &crate::models::PoolId) -> Self {
        AlkaneId {
            block: id.block.clone(),
            tx: id.tx.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct RpcRequest {
    jsonrpc: String,
    method: String,
    params: Value,
    id: u64,
}

#[derive(Debug, Clone, Deserialize)]
struct RpcResponse {
    jsonrpc: String,
    result: Option<Value>,
    error: Option<Value>,
    id: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AlkanesOutpoint {
    pub outpoint: Outpoint,
    pub output: Output,
    pub runes: Vec<RuneBalance>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Outpoint {
    pub txid: String,
    pub vout: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Output {
    pub script: String,
    pub value: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RuneBalance {
    pub balance: String,
    pub rune: RuneInfo,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RuneInfo {
    pub id: AlkaneId,
    pub name: String,
    pub symbol: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SimulateRequest {
    pub target: AlkaneId,
    pub inputs: Vec<Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SimulateResponse {
    pub status: i32,
    pub parsed: Option<Value>,
}

#[derive(Clone)]
pub struct AlkanesRpcClient {
    client: Client,
    url: String,
    request_id: Arc<AtomicU64>,
}

use std::sync::Arc;

impl AlkanesRpcClient {
    pub fn new(config: &Config) -> Result<Self> {
        Ok(Self {
            client: Client::new(),
            url: config.sandshrew_url.clone(),
            request_id: Arc::new(AtomicU64::new(1)),
        })
    }

    fn next_id(&self) -> u64 {
        self.request_id.fetch_add(1, Ordering::SeqCst)
    }

    async fn call(&self, method: &str, params: Value) -> Result<Value> {
        let request = RpcRequest {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params,
            id: self.next_id(),
        };

        let response = self
            .client
            .post(&self.url)
            .json(&request)
            .send()
            .await
            .context("Failed to send RPC request")?;

        let rpc_response: RpcResponse = response
            .json()
            .await
            .context("Failed to parse RPC response")?;

        if let Some(error) = rpc_response.error {
            return Err(anyhow::anyhow!("RPC error: {:?}", error));
        }

        rpc_response
            .result
            .ok_or_else(|| anyhow::anyhow!("No result in RPC response"))
    }

    /// Get alkanes by address
    pub async fn get_alkanes_by_address(&self, address: &str) -> Result<Vec<AlkanesOutpoint>> {
        let result = self
            .call("alkanes.getAlkanesByAddress", json!([{ "address": address }]))
            .await?;

        serde_json::from_value(result).context("Failed to parse alkanes by address response")
    }

    /// Get block count
    pub async fn get_block_count(&self) -> Result<u64> {
        let result = self.call("getblockcount", json!([])).await?;
        result
            .as_u64()
            .ok_or_else(|| anyhow::anyhow!("Invalid block count response"))
    }

    /// Simulate alkane contract call
    pub async fn simulate(&self, request: &SimulateRequest) -> Result<SimulateResponse> {
        let result = self
            .call("alkanes.simulate", json!([request]))
            .await?;

        serde_json::from_value(result).context("Failed to parse simulate response")
    }

    /// Get blockchain info
    pub async fn get_blockchain_info(&self) -> Result<Value> {
        self.call("getblockchaininfo", json!([])).await
    }

    /// Get address UTXOs (via esplora)
    pub async fn get_address_utxos(&self, address: &str) -> Result<Value> {
        self.call("esplora.address.utxo", json!([address])).await
    }

    /// Get transaction
    pub async fn get_transaction(&self, txid: &str) -> Result<Value> {
        self.call("esplora.tx", json!([txid])).await
    }

    /// Get address transactions
    pub async fn get_address_txs(&self, address: &str) -> Result<Value> {
        self.call("esplora.address.txs", json!([address])).await
    }
}
