//! # Metashrew Provider
//!
//! This module provides an implementation of the `MetashrewProvider` trait.

use crate::{
    traits::{JsonRpcProvider, MetashrewProvider},
    Result,
};
use async_trait::async_trait;
use serde_json::{json, Value};

#[derive(Clone)]
pub struct MetashrewProviderImpl<R: JsonRpcProvider> {
    rpc_provider: R,
    url: String,
}

impl<R: JsonRpcProvider> MetashrewProviderImpl<R> {
    pub fn new(rpc_provider: R, url: String) -> Self {
        Self { rpc_provider, url }
    }
}

#[async_trait(?Send)]
impl<R: JsonRpcProvider + Send + Sync> MetashrewProvider for MetashrewProviderImpl<R> {
    async fn get_height(&self) -> Result<u64> {
        let result = self
            .rpc_provider
            .call(&self.url, "getblockcount", json!([]), 1)
            .await?;
        let height = result.as_u64().unwrap_or(0);
        Ok(height)
    }

    async fn get_block_hash(&self, height: u64) -> Result<String> {
        let result = self
            .rpc_provider
            .call(&self.url, "getblockhash", json!([height]), 1)
            .await?;
        let hash = result.as_str().unwrap_or_default().to_string();
        Ok(hash)
    }

    async fn get_state_root(&self, height: Value) -> Result<String> {
        let result = self
            .rpc_provider
            .call(&self.url, "getstateroot", json!([height]), 1)
            .await?;
        let root = result.as_str().unwrap_or_default().to_string();
        Ok(root)
    }
}