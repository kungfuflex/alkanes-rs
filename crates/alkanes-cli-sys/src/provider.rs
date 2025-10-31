//! RPC provider wrapper that bridges concrete RPC client with trait abstractions
//! 
//! This module provides a concrete implementation of the JsonRpcProvider trait
//! that wraps our existing RPC client to work with the trait-based core logic.

use std::sync::Arc;
use async_trait::async_trait;
use serde_json::Value as JsonValue;

use deezel_common::traits::JsonRpcProvider;
use deezel_common::DeezelError;
use crate::rpc::RpcClient;

/// Concrete RPC provider that wraps our RPC client
pub struct ConcreteRpcProvider {
    rpc_client: Arc<RpcClient>,
}

impl ConcreteRpcProvider {
    pub fn new(rpc_client: Arc<RpcClient>) -> Self {
        Self { rpc_client }
    }
}

#[async_trait]
impl JsonRpcProvider for ConcreteRpcProvider {
    async fn call(
        &self,
        _url: &str,
        method: &str,
        params: JsonValue,
        _id: u64,
    ) -> Result<JsonValue, DeezelError> {
        // Use the concrete RPC client's _call method and convert error
        self.rpc_client._call(method, params).await
            .map_err(|e| DeezelError::RpcError(e.to_string()))
    }

    async fn get_bytecode(&self, block: &str, tx: &str) -> Result<String, DeezelError> {
        // Use the concrete RPC client's get_bytecode method and convert error
        self.rpc_client.get_bytecode(block, tx).await
            .map_err(|e| DeezelError::RpcError(e.to_string()))
    }
}