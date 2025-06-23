//! Generic JSON-RPC method handlers

use super::types::{JsonRpcRequest, JsonRpcResult, JsonRpcError};
use crate::{StorageAdapter, RuntimeAdapter, ViewCall, PreviewCall};
use serde_json::Value;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use log::{debug, error, info};

/// Generic JSON-RPC handlers that work with any storage and runtime adapters
pub struct JsonRpcHandlers<S, R> 
where 
    S: StorageAdapter,
    R: RuntimeAdapter,
{
    storage: Arc<RwLock<S>>,
    runtime: Arc<RwLock<R>>,
    current_height: Arc<AtomicU32>,
}

impl<S, R> JsonRpcHandlers<S, R>
where
    S: StorageAdapter + 'static,
    R: RuntimeAdapter + 'static,
{
    pub fn new(
        storage: Arc<RwLock<S>>,
        runtime: Arc<RwLock<R>>,
        current_height: Arc<AtomicU32>,
    ) -> Self {
        Self {
            storage,
            runtime,
            current_height,
        }
    }

    /// Handle metashrew_view RPC method
    pub async fn handle_view(&self, request: &JsonRpcRequest) -> Result<JsonRpcResult, JsonRpcError> {
        debug!("Handling metashrew_view request");

        if request.params.len() < 3 {
            return Err(JsonRpcError::invalid_params(
                request.id,
                "Invalid params: requires [view_name, input_data, height]".to_string(),
            ));
        }

        let view_name = request.params[0].as_str()
            .ok_or_else(|| JsonRpcError::invalid_params(
                request.id,
                "Invalid params: view_name must be a string".to_string(),
            ))?
            .to_string();

        let input_hex = request.params[1].as_str()
            .ok_or_else(|| JsonRpcError::invalid_params(
                request.id,
                "Invalid params: input_data must be a hex string".to_string(),
            ))?
            .to_string();

        let height = self.parse_height(&request.params[2], request.id)?;

        let input_data = hex::decode(input_hex.trim_start_matches("0x"))
            .map_err(|_| JsonRpcError::invalid_params(
                request.id,
                "Invalid hex input data".to_string(),
            ))?;

        let call = ViewCall {
            function_name: view_name,
            input_data,
            height,
        };

        match self.runtime.read().await.execute_view(call).await {
            Ok(result) => Ok(JsonRpcResult::success(
                request.id,
                format!("0x{}", hex::encode(result.data)),
            )),
            Err(err) => Err(JsonRpcError::internal_error(
                request.id,
                err.to_string(),
            )),
        }
    }

    /// Handle metashrew_preview RPC method
    pub async fn handle_preview(&self, request: &JsonRpcRequest) -> Result<JsonRpcResult, JsonRpcError> {
        debug!("Handling metashrew_preview request");

        if request.params.len() < 4 {
            return Err(JsonRpcError::invalid_params(
                request.id,
                "Invalid params: requires [block_data, view_name, input_data, height]".to_string(),
            ));
        }

        let block_hex = request.params[0].as_str()
            .ok_or_else(|| JsonRpcError::invalid_params(
                request.id,
                "Invalid params: block_data must be a hex string".to_string(),
            ))?
            .to_string();

        let view_name = request.params[1].as_str()
            .ok_or_else(|| JsonRpcError::invalid_params(
                request.id,
                "Invalid params: view_name must be a string".to_string(),
            ))?
            .to_string();

        let input_hex = request.params[2].as_str()
            .ok_or_else(|| JsonRpcError::invalid_params(
                request.id,
                "Invalid params: input_data must be a hex string".to_string(),
            ))?
            .to_string();

        let height = self.parse_height(&request.params[3], request.id)?;

        let block_data = hex::decode(block_hex.trim_start_matches("0x"))
            .map_err(|_| JsonRpcError::invalid_params(
                request.id,
                "Invalid hex block data".to_string(),
            ))?;

        let input_data = hex::decode(input_hex.trim_start_matches("0x"))
            .map_err(|_| JsonRpcError::invalid_params(
                request.id,
                "Invalid hex input data".to_string(),
            ))?;

        let call = PreviewCall {
            block_data,
            function_name: view_name,
            input_data,
            height,
        };

        match self.runtime.read().await.execute_preview(call).await {
            Ok(result) => Ok(JsonRpcResult::success(
                request.id,
                format!("0x{}", hex::encode(result.data)),
            )),
            Err(err) => Err(JsonRpcError::internal_error(
                request.id,
                err.to_string(),
            )),
        }
    }

    /// Handle metashrew_height RPC method
    pub async fn handle_height(&self, request: &JsonRpcRequest) -> Result<JsonRpcResult, JsonRpcError> {
        debug!("Handling metashrew_height request");

        let current_height = self.current_height.load(Ordering::SeqCst);
        let height = current_height.saturating_sub(1); // Same logic as sync engine

        Ok(JsonRpcResult::success_json(
            request.id,
            serde_json::json!(height),
        ))
    }

    /// Handle metashrew_getblockhash RPC method
    pub async fn handle_getblockhash(&self, request: &JsonRpcRequest) -> Result<JsonRpcResult, JsonRpcError> {
        debug!("Handling metashrew_getblockhash request");

        if request.params.len() != 1 {
            return Err(JsonRpcError::invalid_params(
                request.id,
                "Invalid params: requires [block_number]".to_string(),
            ));
        }

        let height = request.params[0].as_u64()
            .ok_or_else(|| JsonRpcError::invalid_params(
                request.id,
                "Invalid params: block_number must be a number".to_string(),
            ))? as u32;

        match self.storage.read().await.get_block_hash(height).await {
            Ok(Some(hash)) => Ok(JsonRpcResult::success(
                request.id,
                format!("0x{}", hex::encode(hash)),
            )),
            Ok(None) => Err(JsonRpcError::internal_error(
                request.id,
                "Block hash not found".to_string(),
            )),
            Err(err) => Err(JsonRpcError::internal_error(
                request.id,
                format!("Storage error: {}", err),
            )),
        }
    }

    /// Handle metashrew_stateroot RPC method
    pub async fn handle_stateroot(&self, request: &JsonRpcRequest) -> Result<JsonRpcResult, JsonRpcError> {
        debug!("Handling metashrew_stateroot request");

        let height = if request.params.is_empty() {
            // Default to latest height if no params provided
            let current_height = self.current_height.load(Ordering::SeqCst);
            current_height.saturating_sub(1)
        } else {
            self.parse_height(&request.params[0], request.id)?
        };

        info!("metashrew_stateroot called with height: {}", height);

        match self.storage.read().await.get_state_root(height).await {
            Ok(Some(root)) => {
                info!(
                    "Successfully retrieved state root for height {}: 0x{}",
                    height,
                    hex::encode(&root)
                );
                Ok(JsonRpcResult::success(
                    request.id,
                    format!("0x{}", hex::encode(root)),
                ))
            }
            Ok(None) => {
                error!("No state root found for height {}", height);
                Err(JsonRpcError::internal_error(
                    request.id,
                    format!("No state root found for height {}", height),
                ))
            }
            Err(e) => {
                error!("Failed to get stateroot for height {}: {}", height, e);
                Err(JsonRpcError::internal_error(
                    request.id,
                    format!("Failed to get stateroot: {}", e),
                ))
            }
        }
    }

    /// Handle metashrew_snapshot RPC method
    pub async fn handle_snapshot(&self, request: &JsonRpcRequest) -> Result<JsonRpcResult, JsonRpcError> {
        debug!("Handling metashrew_snapshot request");

        match self.storage.read().await.get_stats().await {
            Ok(stats) => {
                let snapshot_info = serde_json::json!({
                    "enabled": true,
                    "current_height": self.current_height.load(Ordering::SeqCst),
                    "indexed_height": stats.indexed_height,
                    "total_entries": stats.total_entries,
                    "storage_size_bytes": stats.storage_size_bytes
                });
                Ok(JsonRpcResult::success(
                    request.id,
                    snapshot_info.to_string(),
                ))
            }
            Err(err) => Err(JsonRpcError::internal_error(
                request.id,
                err.to_string(),
            )),
        }
    }

    /// Parse height parameter (supports "latest" and numeric values)
    fn parse_height(&self, param: &Value, request_id: u32) -> Result<u32, JsonRpcError> {
        match param {
            Value::String(s) if s == "latest" => {
                let current_height = self.current_height.load(Ordering::SeqCst);
                Ok(current_height.saturating_sub(1))
            }
            Value::Number(n) => Ok(n.as_u64().unwrap_or(0) as u32),
            _ => Err(JsonRpcError::invalid_params(
                request_id,
                "Invalid params: height must be a number or 'latest'".to_string(),
            )),
        }
    }

    /// Dispatch request to appropriate handler
    pub async fn handle_request(&self, request: JsonRpcRequest) -> Result<JsonRpcResult, JsonRpcError> {
        match request.method.as_str() {
            "metashrew_view" => self.handle_view(&request).await,
            "metashrew_preview" => self.handle_preview(&request).await,
            "metashrew_height" => self.handle_height(&request).await,
            "metashrew_getblockhash" => self.handle_getblockhash(&request).await,
            "metashrew_stateroot" => self.handle_stateroot(&request).await,
            "metashrew_snapshot" => self.handle_snapshot(&request).await,
            _ => Err(JsonRpcError::method_not_found(request.id, request.method)),
        }
    }
}