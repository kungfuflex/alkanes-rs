//! Concrete adapter implementations for the `rockshrew-mono` binary.
//!
//! This module provides the specific implementations of the generic adapter traits
//! from `metashrew-sync` that are required to run the production indexer. This
//! includes the `BitcoinRpcAdapter` for connecting to Bitcoin Core and the
//! `MetashrewRuntimeAdapter`, which is aware of the snapshotting process.

use anyhow::Result;
use async_trait::async_trait;
use hex;
use metashrew_runtime::{
    KeyValueStoreLike, MetashrewRuntime, ViewPoolConfig, ViewPoolStats, ViewPoolSupport,
    ViewRuntimePool,
};
use metashrew_sync::{
    AtomicBlockResult, BitcoinNodeAdapter, BlockInfo, ChainTip, PreviewCall, RuntimeAdapter,
    RuntimeStats, SyncError, SyncResult, ViewCall, ViewResult,
};
use serde::{Deserialize, Serialize};
use serde_json::{Number, Value};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

use crate::ssh_tunnel::{make_request_with_tunnel, SshTunnel, SshTunnelConfig, TunneledResponse};

// JSON-RPC request/response structs for BitcoinRpcAdapter
#[derive(Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub id: u32,
    pub jsonrpc: String,
    pub method: String,
    pub params: Vec<Value>,
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub struct BlockCountResponse {
    pub id: u32,
    pub result: Option<u32>,
    pub error: Option<Value>,
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub struct BlockHashResponse {
    pub id: u32,
    pub result: Option<String>,
    pub error: Option<Value>,
}

/// Bitcoin node adapter that connects to a real Bitcoin node via RPC.
#[derive(Clone)]
pub struct BitcoinRpcAdapter {
    rpc_url: String,
    auth: Option<String>,
    bypass_ssl: bool,
    tunnel_config: Option<SshTunnelConfig>,
    active_tunnel: Arc<tokio::sync::Mutex<Option<SshTunnel>>>,
}

impl BitcoinRpcAdapter {
    pub fn new(
        rpc_url: String,
        auth: Option<String>,
        bypass_ssl: bool,
        tunnel_config: Option<SshTunnelConfig>,
    ) -> Self {
        Self {
            rpc_url,
            auth,
            bypass_ssl,
            tunnel_config,
            active_tunnel: Arc::new(tokio::sync::Mutex::new(None)),
        }
    }

    async fn post(&self, body: String) -> Result<TunneledResponse> {
        // Implementation with retry logic...
        let max_retries = 5;
        let mut retry_delay = Duration::from_millis(500);
        let max_delay = Duration::from_secs(16);

        let mut active_tunnel_guard = if self.tunnel_config.is_some() {
            Some(self.active_tunnel.lock().await)
        } else {
            None
        };

        for attempt in 0..max_retries {
            let existing_tunnel: Option<SshTunnel> = if let Some(guard) = &active_tunnel_guard {
                (**guard).clone()
            } else {
                None
            };

            match make_request_with_tunnel(
                &self.rpc_url,
                body.clone(),
                self.auth.clone(),
                self.tunnel_config.clone(),
                self.bypass_ssl,
                existing_tunnel,
            )
            .await
            {
                Ok(tunneled_response) => {
                    if let Some(guard) = &mut active_tunnel_guard {
                        if guard.is_none() {
                            if let Some(tunnel) = tunneled_response._tunnel.clone() {
                                **guard = Some(tunnel);
                            }
                        }
                    }
                    return Ok(tunneled_response);
                }
                Err(e) => {
                    log::warn!(
                        "Request failed (attempt {}): {}. Retrying in {:?}...",
                        attempt + 1,
                        e,
                        retry_delay
                    );
                    if let Some(guard) = &mut active_tunnel_guard {
                        **guard = None;
                    }
                    let jitter = {
                        use rand::Rng;
                        rand::thread_rng().gen_range(0..=100) as u64
                    };
                    retry_delay =
                        std::cmp::min(max_delay, retry_delay * 2 + Duration::from_millis(jitter));
                    tokio::time::sleep(retry_delay).await;
                }
            }
        }
        Err(anyhow::anyhow!("Max retries exceeded"))
    }
}

#[async_trait]
impl BitcoinNodeAdapter for BitcoinRpcAdapter {
    async fn get_tip_height(&self) -> SyncResult<u32> {
        let request_body = serde_json::to_string(&JsonRpcRequest {
            id: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|e| SyncError::BitcoinNode(format!("Time error: {}", e)))?
                .as_secs() as u32,
            jsonrpc: "2.0".to_string(),
            method: "getblockcount".to_string(),
            params: vec![],
        })
        .map_err(|e| SyncError::BitcoinNode(format!("JSON serialization error: {}", e)))?;
        let tunneled_response = self.post(request_body).await?;
        let result: BlockCountResponse = tunneled_response
            .json()
            .await
            .map_err(|e| SyncError::BitcoinNode(format!("JSON parsing error: {}", e)))?;
        result
            .result
            .ok_or_else(|| SyncError::BitcoinNode("missing result".to_string()))
    }

    async fn get_block_hash(&self, height: u32) -> SyncResult<Vec<u8>> {
        let request_body = serde_json::to_string(&JsonRpcRequest {
            id: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|e| SyncError::BitcoinNode(format!("Time error: {}", e)))?
                .as_secs() as u32,
            jsonrpc: "2.0".to_string(),
            method: "getblockhash".to_string(),
            params: vec![Value::Number(Number::from(height))],
        })
        .map_err(|e| SyncError::BitcoinNode(format!("JSON serialization error: {}", e)))?;
        let tunneled_response = self.post(request_body).await?;
        let result: BlockHashResponse = tunneled_response
            .json()
            .await
            .map_err(|e| SyncError::BitcoinNode(format!("JSON parsing error: {}", e)))?;
        let blockhash = result
            .result
            .ok_or_else(|| SyncError::BitcoinNode("missing result".to_string()))?;
        hex::decode(blockhash)
            .map_err(|e| SyncError::BitcoinNode(format!("Hex decode error: {}", e)))
    }

    async fn get_block_data(&self, height: u32) -> SyncResult<Vec<u8>> {
        let blockhash = self.get_block_hash(height).await?;
        let request_body = serde_json::to_string(&JsonRpcRequest {
            id: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|e| SyncError::BitcoinNode(format!("Time error: {}", e)))?
                .as_secs() as u32,
            jsonrpc: "2.0".to_string(),
            method: "getblock".to_string(),
            params: vec![
                Value::String(hex::encode(&blockhash)),
                Value::Number(Number::from(0)),
            ],
        })
        .map_err(|e| SyncError::BitcoinNode(format!("JSON serialization error: {}", e)))?;
        let tunneled_response = self.post(request_body).await?;
        let result: BlockHashResponse = tunneled_response
            .json()
            .await
            .map_err(|e| SyncError::BitcoinNode(format!("JSON parsing error: {}", e)))?;
        let block_hex = result
            .result
            .ok_or_else(|| SyncError::BitcoinNode("missing result".to_string()))?;
        hex::decode(block_hex)
            .map_err(|e| SyncError::BitcoinNode(format!("Hex decode error: {}", e)))
    }

    async fn get_block_info(&self, height: u32) -> SyncResult<BlockInfo> {
        let hash = self.get_block_hash(height).await?;
        let data = self.get_block_data(height).await?;
        Ok(BlockInfo { height, hash, data })
    }

    async fn get_chain_tip(&self) -> SyncResult<ChainTip> {
        let height = self.get_tip_height().await?;
        let hash = self.get_block_hash(height).await?;
        Ok(ChainTip { height, hash })
    }

    async fn is_connected(&self) -> bool {
        self.get_tip_height().await.is_ok()
    }
}

/// MetashrewRuntime adapter that wraps the actual MetashrewRuntime and is snapshot-aware.
/// Now includes a parallelized view pool for concurrent view execution.
#[derive(Clone)]
pub struct MetashrewRuntimeAdapter<T: KeyValueStoreLike + Clone + Send + Sync + 'static> {
    runtime: Arc<RwLock<MetashrewRuntime<T>>>,
    snapshot_manager: Arc<RwLock<Option<Arc<RwLock<crate::snapshot::SnapshotManager>>>>>,
    view_pool: Arc<RwLock<Option<ViewRuntimePool<T>>>>,
    disable_lru_cache: Arc<std::sync::atomic::AtomicBool>,
}

impl<T: KeyValueStoreLike + Clone + Send + Sync + 'static> MetashrewRuntimeAdapter<T> {
    pub fn new(runtime: Arc<RwLock<MetashrewRuntime<T>>>) -> Self {
        Self {
            runtime,
            snapshot_manager: Arc::new(RwLock::new(None)),
            view_pool: Arc::new(RwLock::new(None)),
            disable_lru_cache: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// Initialize the view pool with the specified configuration
    pub async fn initialize_view_pool(&self, config: ViewPoolConfig) -> Result<()> {
        let runtime = self.runtime.read().await;
        let pool = runtime
            .create_view_pool(config)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to create view pool: {}", e))?;

        let mut view_pool_guard = self.view_pool.write().await;
        *view_pool_guard = Some(pool);

        log::info!("View pool initialized successfully");
        Ok(())
    }

    /// Get view pool statistics for monitoring
    pub async fn get_view_pool_stats(&self) -> Option<ViewPoolStats> {
        if let Some(pool) = self.view_pool.read().await.as_ref() {
            Some(pool.get_stats().await)
        } else {
            None
        }
    }

    /// Disable stateful views to use non-stateful async wasmtime
    pub async fn disable_stateful_views(&self) {
        let mut runtime = self.runtime.write().await;
        runtime.disable_stateful_views();
        log::info!("Stateful views disabled - will use non-stateful async wasmtime");
    }

    /// Check if stateful views are enabled
    pub async fn is_stateful_views_enabled(&self) -> bool {
        let runtime = self.runtime.read().await;
        runtime.is_stateful_views_enabled()
    }

    /// Set the disable LRU cache flag
    pub fn set_disable_lru_cache(&self, disable: bool) {
        self.disable_lru_cache.store(disable, std::sync::atomic::Ordering::SeqCst);
        if disable {
            log::info!("LRU cache disabled - will refresh memory for each WASM invocation");
        } else {
            log::info!("LRU cache enabled - memory will persist between blocks");
        }
    }

    /// Check if LRU cache is disabled
    pub fn is_lru_cache_disabled(&self) -> bool {
        self.disable_lru_cache.load(std::sync::atomic::Ordering::SeqCst)
    }

    pub async fn set_snapshot_manager(
        &self,
        manager: Arc<RwLock<crate::snapshot::SnapshotManager>>,
    ) {
        let mut snapshot_manager = self.snapshot_manager.write().await;
        *snapshot_manager = Some(manager);
    }

    pub async fn get_snapshot_manager(
        &self,
    ) -> Option<Arc<RwLock<crate::snapshot::SnapshotManager>>> {
        self.snapshot_manager.read().await.as_ref().cloned()
    }
}

#[async_trait]
impl<T: KeyValueStoreLike + Clone + Send + Sync + 'static> RuntimeAdapter for MetashrewRuntimeAdapter<T> {
    async fn process_block(&mut self, height: u32, block_data: &[u8]) -> SyncResult<()> {
        if let Some(manager_arc) = self.get_snapshot_manager().await {
            {
                let mut manager = manager_arc.write().await;
                manager.set_current_height(height);
            }
            let mut runtime = self.runtime.write().await;
            {
                let mut context = runtime
                    .context
                    .lock()
                    .map_err(|e| SyncError::Runtime(format!("Failed to lock context: {}", e)))?;
                let manager_arc_clone = manager_arc.clone();
                let tracker_fn: metashrew_runtime::KVTrackerFn =
                    Box::new(move |key: Vec<u8>, value: Vec<u8>| {
                        tokio::task::block_in_place(|| {
                            tokio::runtime::Handle::current().block_on(async {
                                if let Ok(mut manager) = manager_arc_clone.try_write() {
                                    manager.track_key_change(key, value);
                                }
                            })
                        });
                    });
                context.db.set_kv_tracker(Some(tracker_fn));
                context.block = block_data.to_vec();
                context.height = height;
                context.db.set_height(height);
            }
            runtime
                .run()
                .map_err(|e| SyncError::Runtime(format!("Runtime execution failed: {}", e)))?;
            {
                let mut context = runtime
                    .context
                    .lock()
                    .map_err(|e| SyncError::Runtime(format!("Failed to lock context: {}", e)))?;
                context.db.set_kv_tracker(None);
            }
            
            // Refresh memory after each block if LRU cache is disabled
            if self.is_lru_cache_disabled() {
                log::debug!("Refreshing WASM memory after block {} (LRU cache disabled)", height);
                runtime
                    .refresh_memory()
                    .map_err(|e| SyncError::Runtime(format!("Failed to refresh memory after block: {}", e)))?;
            }
        } else {
            let mut runtime = self.runtime.write().await;
            {
                let mut context = runtime
                    .context
                    .lock()
                    .map_err(|e| SyncError::Runtime(format!("Failed to lock context: {}", e)))?;
                context.block = block_data.to_vec();
                context.height = height;
                context.db.set_height(height);
            }
            runtime
                .run()
                .map_err(|e| SyncError::Runtime(format!("Runtime execution failed: {}", e)))?;
            
            // Refresh memory after each block if LRU cache is disabled
            if self.is_lru_cache_disabled() {
                log::debug!("Refreshing WASM memory after block {} (LRU cache disabled)", height);
                runtime
                    .refresh_memory()
                    .map_err(|e| SyncError::Runtime(format!("Failed to refresh memory after block: {}", e)))?;
            }
        }
        Ok(())
    }

    async fn process_block_atomic(
        &mut self,
        height: u32,
        block_data: &[u8],
        block_hash: &[u8],
    ) -> SyncResult<AtomicBlockResult> {
        let mut runtime = self.runtime.write().await;
        runtime
            .process_block_atomic(height, block_data, block_hash)
            .await
            .map(|res| AtomicBlockResult {
                state_root: res.state_root,
                batch_data: res.batch_data,
                height: res.height,
                block_hash: res.block_hash,
            })
            .map_err(|e| SyncError::Runtime(format!("Atomic block processing failed: {}", e)))
    }

    async fn get_state_root(&self, height: u32) -> SyncResult<Vec<u8>> {
        let runtime = self.runtime.read().await;
        let context = runtime
            .context
            .lock()
            .map_err(|e| SyncError::Runtime(format!("Failed to lock context: {}", e)))?;
        let adapter = context.db.clone();
        let smt_helper = metashrew_runtime::smt::SMTHelper::new(adapter);
        match smt_helper.get_smt_root_at_height(height) {
            Ok(root) => Ok(root.to_vec()),
            Err(e) => Err(SyncError::Runtime(format!(
                "Failed to get state root for height {}: {}",
                height, e
            ))),
        }
    }

    async fn execute_view(&self, call: ViewCall) -> SyncResult<ViewResult> {
        // Try to use view pool first if available
        if let Some(pool) = self.view_pool.read().await.as_ref() {
            log::debug!("Using view pool for function: {}", call.function_name);
            let result = pool
                .view(call.function_name, &call.input_data, call.height)
                .await
                .map_err(|e| {
                    SyncError::ViewFunction(format!("View pool execution failed: {}", e))
                })?;
            Ok(ViewResult { data: result })
        } else {
            // Fallback to direct runtime execution
            log::debug!("Using direct runtime for function: {}", call.function_name);
            let runtime = self.runtime.read().await;
            let result = runtime
                .view(call.function_name, &call.input_data, call.height)
                .await
                .map_err(|e| SyncError::ViewFunction(format!("View function failed: {}", e)))?;
            Ok(ViewResult { data: result })
        }
    }

    async fn execute_preview(&self, call: PreviewCall) -> SyncResult<ViewResult> {
        let runtime = self.runtime.read().await;
        let result = runtime
            .preview_async(
                &call.block_data,
                call.function_name,
                &call.input_data,
                call.height,
            )
            .await
            .map_err(|e| SyncError::ViewFunction(format!("Preview function failed: {}", e)))?;
        Ok(ViewResult { data: result })
    }

    async fn refresh_memory(&mut self) -> SyncResult<()> {
        log::info!("Memory refresh requested (typically during chain reorganization)");
        let mut runtime = self.runtime.write().await;
        runtime
            .refresh_memory()
            .map_err(|e| SyncError::Runtime(format!("Failed to refresh runtime memory: {}", e)))?;
        Ok(())
    }

    async fn is_ready(&self) -> bool {
        self.runtime.try_read().is_ok()
    }

    async fn get_stats(&self) -> SyncResult<RuntimeStats> {
        let runtime = self.runtime.write().await;
        let context = runtime
            .context
            .lock()
            .map_err(|e| SyncError::Runtime(format!("Failed to lock context: {}", e)))?;
        let blocks_processed = context.height;
        Ok(RuntimeStats {
            memory_usage_bytes: 0,
            blocks_processed,
            last_refresh_height: Some(blocks_processed),
        })
    }

    async fn get_prefix_root(&self, name: &str, _height: u32) -> SyncResult<Option<[u8; 32]>> {
        let runtime = self.runtime.read().await;
        let context = runtime
            .context
            .lock()
            .map_err(|e| SyncError::Runtime(format!("Failed to lock context: {}", e)))?;
        if let Some(smt) = context.prefix_smts.get(name) {
            Ok(Some(smt.root()))
        } else {
            Ok(None)
        }
    }

    async fn log_prefix_roots(&self) -> SyncResult<()> {
        let runtime = self.runtime.read().await;
        let context = runtime
            .context
            .lock()
            .map_err(|e| SyncError::Runtime(format!("Failed to lock context: {}", e)))?;
        for (name, smt) in context.prefix_smts.iter() {
            log::info!("prefixroot {}: {}", name, hex::encode(smt.root()));
        }
        Ok(())
    }
}
