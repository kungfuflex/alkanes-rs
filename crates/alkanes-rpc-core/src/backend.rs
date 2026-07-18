use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

use crate::types::{JsonRpcRequest, JsonRpcResponse};

/// Backend for Bitcoin Core RPC calls.
#[async_trait(?Send)]
pub trait BitcoinBackend {
    /// Call a Bitcoin Core RPC method (method name without namespace prefix).
    async fn call(&self, method: &str, params: Vec<Value>, id: Value) -> Result<JsonRpcResponse>;
}

/// Backend for Metashrew indexer JSON-RPC calls.
#[async_trait(?Send)]
pub trait MetashrewBackend {
    /// Forward a full JSON-RPC request to metashrew.
    async fn forward(&self, request: &JsonRpcRequest) -> Result<JsonRpcResponse>;

    /// Forward many requests, ideally CONCURRENTLY, returning results in the
    /// same order. The default is a serial fallback (correct but not
    /// parallel); backends whose transport can issue concurrent outbound
    /// requests (e.g. the edge's wasi:http client) override this so a fan-out
    /// costs one round-trip instead of N. Used by the `protorunesbyaddress` /
    /// `sandshrew_balances` per-outpoint fan-out.
    async fn forward_batch(
        &self,
        requests: &[JsonRpcRequest],
    ) -> Result<Vec<JsonRpcResponse>> {
        let mut out = Vec::with_capacity(requests.len());
        for r in requests {
            out.push(self.forward(r).await?);
        }
        Ok(out)
    }
}

/// Backend for Esplora REST API calls.
#[async_trait(?Send)]
pub trait EsploraBackend {
    /// Fetch a path from esplora (e.g. "/address/{addr}/utxo").
    async fn fetch(&self, path: &str) -> Result<Value>;
}

/// Backend for Ord REST API calls.
#[async_trait(?Send)]
pub trait OrdBackend {
    /// Fetch a JSON endpoint from ord.
    async fn fetch(&self, path: &str) -> Result<Value>;

    /// Fetch binary content from ord (for inscriptions).
    async fn fetch_content(&self, inscription_id: &str) -> Result<Vec<u8>>;
}

/// No-op Ord backend that returns empty/null results.
pub struct NoOrd;

#[async_trait(?Send)]
impl OrdBackend for NoOrd {
    async fn fetch(&self, _path: &str) -> Result<Value> {
        Ok(Value::Null)
    }

    async fn fetch_content(&self, _inscription_id: &str) -> Result<Vec<u8>> {
        Ok(vec![])
    }
}
