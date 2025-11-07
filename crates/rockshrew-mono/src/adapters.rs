use anyhow::Result;
use async_trait::async_trait;
use hex;
use metashrew_runtime::{KeyValueStoreLike, MetashrewRuntime};
use metashrew_sync::{
    AtomicBlockResult, BitcoinNodeAdapter, BlockInfo, ChainTip, PreviewCall, RuntimeAdapter,
    RuntimeStats, SyncError, SyncResult, ViewCall, ViewResult,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Number, Value};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

use crate::ssh_tunnel::{make_request_with_tunnel, SshTunnel, SshTunnelConfig};

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

    async fn request_with_retry<T: DeserializeOwned>(&self, method: &str, params: Vec<Value>) -> Result<T> {
        let max_retries = 5;
        let mut retry_delay = Duration::from_millis(500);
        let max_delay = Duration::from_secs(16);

        let mut active_tunnel_guard = if self.tunnel_config.is_some() {
            Some(self.active_tunnel.lock().await)
        } else {
            None
        };

        for attempt in 0..max_retries {
            let request_body = serde_json::to_string(&JsonRpcRequest {
                id: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_err(|e| anyhow::anyhow!("Time error: {}", e))?
                    .as_secs() as u32,
                jsonrpc: "2.0".to_string(),
                method: method.to_string(),
                params: params.clone(),
            })
            .map_err(|e| anyhow::anyhow!("JSON serialization error: {}", e))?;

            let existing_tunnel: Option<SshTunnel> = if let Some(guard) = &active_tunnel_guard {
                (**guard).clone()
            } else {
                None
            };

            match make_request_with_tunnel(
                &self.rpc_url,
                request_body.clone(),
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
                    
                    match tunneled_response.json::<T>().await {
                        Ok(response) => return Ok(response),
                        Err(e) => {
                            log::warn!("JSON parsing failed (attempt {}): {}. Retrying in {:?}...", attempt + 1, e, retry_delay);
                        }
                    }
                }
                Err(e) => {
                    log::warn!("Request failed (attempt {}): {}. Retrying in {:?}...", attempt + 1, e, retry_delay);
                    if let Some(guard) = &mut active_tunnel_guard {
                        **guard = None;
                    }
                }
            }

            let jitter = {
                use rand::Rng;
                rand::thread_rng().gen_range(0..=100) as u64
            };
            retry_delay = std::cmp::min(max_delay, retry_delay * 2 + Duration::from_millis(jitter));
            tokio::time::sleep(retry_delay).await;
        }

        Err(anyhow::anyhow!("Max retries exceeded for method {}", method))
    }
}
#[async_trait]
impl BitcoinNodeAdapter for BitcoinRpcAdapter {
    async fn get_tip_height(&self) -> SyncResult<u32> {
        let response: BlockCountResponse = self
            .request_with_retry("getblockcount", vec![])
            .await
            .map_err(|e| SyncError::BitcoinNode(e.to_string()))?;
        response
            .result
            .ok_or_else(|| SyncError::BitcoinNode("missing result".to_string()))
    }

    async fn get_block_hash(&self, height: u32) -> SyncResult<Vec<u8>> {
        let params = vec![Value::Number(Number::from(height))];
        let response: BlockHashResponse = self
            .request_with_retry("getblockhash", params)
            .await
            .map_err(|e| SyncError::BitcoinNode(e.to_string()))?;
        let blockhash = response
            .result
            .ok_or_else(|| SyncError::BitcoinNode("missing result".to_string()))?;
        hex::decode(blockhash)
            .map_err(|e| SyncError::BitcoinNode(format!("Hex decode error: {}", e)))
    }

    async fn get_block_data(&self, height: u32) -> SyncResult<Vec<u8>> {
        let blockhash = self.get_block_hash(height).await?;
        let params = vec![
            Value::String(hex::encode(&blockhash)),
            Value::Number(Number::from(0)),
        ];
        let response: BlockHashResponse = self
            .request_with_retry("getblock", params)
            .await
            .map_err(|e| SyncError::BitcoinNode(e.to_string()))?;
        let block_hex = response
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
/// 
/// No external locking is needed for the runtime because:
/// - View/preview calls create independent WASM runtime instances
/// - Block processing is sequential (one block at a time)
/// - Database is append-only with height-based reads (concurrent reads are safe)
/// - MetashrewRuntime handles internal synchronization for context/instance access
#[derive(Clone)]
pub struct MetashrewRuntimeAdapter<T: KeyValueStoreLike + Clone + Send + Sync + 'static> {
    runtime: Arc<MetashrewRuntime<T>>,
    snapshot_manager: Arc<RwLock<Option<Arc<RwLock<crate::snapshot::SnapshotManager>>>>>,
}

impl<T: KeyValueStoreLike + Clone + Send + Sync + 'static> MetashrewRuntimeAdapter<T> {
    pub fn new(runtime: Arc<MetashrewRuntime<T>>) -> Self {
        Self {
            runtime,
            snapshot_manager: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn set_snapshot_manager(&self, manager: Arc<RwLock<crate::snapshot::SnapshotManager>>) {
        let mut snapshot_manager = self.snapshot_manager.write().await;
        *snapshot_manager = Some(manager);
    }

    pub async fn get_snapshot_manager(&self) -> Option<Arc<RwLock<crate::snapshot::SnapshotManager>>> {
        self.snapshot_manager.read().await.as_ref().cloned()
    }
}

#[async_trait]
impl<T: KeyValueStoreLike + Clone + Send + Sync + 'static> RuntimeAdapter for MetashrewRuntimeAdapter<T>
where
    <T as KeyValueStoreLike>::Batch: Send,
{
    async fn process_block(&self, height: u32, block_data: &[u8]) -> SyncResult<()> {
        // No external lock needed - MetashrewRuntime handles internal synchronization
        self.runtime.process_block(height, block_data).await.map_err(|e| SyncError::Runtime(e.to_string()))
    }
    async fn process_block_atomic(
        &self,
        height: u32,
        block_data: &[u8],
        block_hash: &[u8],
    ) -> SyncResult<AtomicBlockResult> {
        // No external lock needed - MetashrewRuntime handles internal synchronization
        self.runtime.process_block_atomic(height, block_data, block_hash).await.map(|r| AtomicBlockResult {
            state_root: r.state_root,
            batch_data: r.batch_data,
            height: r.height,
            block_hash: r.block_hash,
        }).map_err(|e| SyncError::Runtime(e.to_string()))
    }

    async fn get_state_root(&self, height: u32) -> SyncResult<Vec<u8>> {
        // Briefly lock to get DB clone, then release for concurrent access
        let db = {
            let context = self.runtime.context.lock().await;
            context.db.clone()
        };
        let smt_helper = metashrew_runtime::smt::SMTHelper::new(db);
        match smt_helper.get_smt_root_at_height(height) {
            Ok(root) => Ok(root.to_vec()),
            Err(e) => Err(SyncError::Runtime(format!(
                "Failed to get state root for height {}: {}",
                height, e
            ))),
        }
    }

    async fn execute_view(&self, call: ViewCall) -> SyncResult<ViewResult> {
        // view() creates a completely independent WASM runtime instance
        // No external locking needed - the method handles internal coordination
        // Database reads are safe due to append-only structure and height-based queries
        let result = self.runtime
            .view(call.function_name, &call.input_data, call.height)
            .await
            .map_err(|e| SyncError::ViewFunction(format!("View function failed: {}", e)))?;
        Ok(ViewResult { data: result })
    }

    async fn execute_preview(&self, call: PreviewCall) -> SyncResult<ViewResult> {
        // preview() creates an isolated DB copy, processes the block, then runs a view function
        // No external locking needed - all isolation is handled internally
        let result = self.runtime
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

    async fn refresh_memory(&self) -> SyncResult<()> {
        Ok(())
    }

    async fn is_ready(&self) -> bool {
        // No try_read needed - runtime is always ready for concurrent access
        true
    }

    async fn get_stats(&self) -> SyncResult<RuntimeStats> {
        // Briefly lock to get height, then release
        let blocks_processed = {
            let context = self.runtime.context.lock().await;
            context.height
        };
        Ok(RuntimeStats {
            memory_usage_bytes: 0,
            blocks_processed,
            last_refresh_height: Some(blocks_processed),
        })
    }
}