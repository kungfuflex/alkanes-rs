//! # Production Adapter Implementations
//!
//! This module provides concrete implementations of the adapter traits for production
//! use with the Metashrew runtime system. These adapters bridge the generic synchronization
//! framework with the actual Metashrew components, enabling real-world Bitcoin indexing
//! applications.
//!
//! ## Core Adapters
//!
//! ### [`MetashrewRuntimeAdapter`]
//! The primary runtime adapter that wraps the [`MetashrewRuntime`] for use with the
//! synchronization framework. This adapter provides:
//! - **WASM Execution**: Safe execution of indexer WASM modules
//! - **Atomic Processing**: Support for atomic block processing with rollback
//! - **View Functions**: Query execution against indexed state
//! - **Preview Functions**: Hypothetical block processing for testing
//! - **Memory Management**: Automatic memory refresh and optimization
//!
//! ## Integration Features
//!
//! ### Thread Safety
//! All adapters are designed for safe concurrent access:
//! - **Arc/Mutex Protection**: Shared ownership with exclusive access control
//! - **Async Operations**: Non-blocking operations throughout
//! - **Clone Support**: Efficient cloning for multi-threaded usage
//! - **Send/Sync Bounds**: Safe transfer between threads
//!
//! ### Error Handling
//! Comprehensive error handling with proper error type conversion:
//! - **Error Mapping**: Convert Metashrew errors to sync framework errors
//! - **Context Preservation**: Maintain error context for debugging
//! - **Graceful Degradation**: Fallback strategies for failed operations
//! - **Detailed Logging**: Comprehensive logging for troubleshooting
//!
//! ### Performance Optimization
//! Optimized for high-throughput indexing:
//! - **Atomic Operations**: Batch processing for improved performance
//! - **Memory Efficiency**: Automatic memory management and cleanup
//! - **Resource Monitoring**: Statistics collection for performance tuning
//! - **Lazy Initialization**: Efficient resource allocation patterns
//!
//! ## Usage Examples
//!
//! ### Basic Runtime Adapter Setup
//! ```rust,ignore
//! use metashrew_sync::adapters::*;
//! use metashrew_runtime::*;
//!
//! // Create Metashrew runtime
//! let runtime = MetashrewRuntime::new(storage, wasm_module)?;
//!
//! // Wrap in adapter
//! let adapter = MetashrewRuntimeAdapter::new(runtime);
//!
//! // Use with sync engine
//! let sync_engine = MetashrewSync::new(
//!     node_adapter,
//!     storage_adapter,
//!     adapter,  // Runtime adapter
//!     config
//! );
//! ```
//!
//! ### Shared Runtime Usage
//! ```rust,ignore
//! use std::sync::Arc;
//! use tokio::sync::Mutex;
//!
//! // Create shared runtime
//! let shared_runtime = Arc::new(Mutex::new(runtime));
//!
//! // Create adapter from shared runtime
//! let adapter = MetashrewRuntimeAdapter::from_arc(shared_runtime.clone());
//!
//! // Runtime can be shared across multiple components
//! ```
//!
//! ## Integration with Metashrew
//!
//! These adapters enable seamless integration between:
//! - **Synchronization Framework**: Generic blockchain sync capabilities
//! - **Metashrew Runtime**: WASM-based indexer execution environment
//! - **Storage Systems**: Various database backends and storage adapters
//! - **Bitcoin Nodes**: Different node implementations and protocols
//!
//! The adapters handle all the complexity of bridging these systems while
//! maintaining type safety, performance, and reliability.

use async_trait::async_trait;
use metashrew_runtime::{KeyValueStoreLike, MetashrewRuntime};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::{
    AtomicBlockResult, PreviewCall, RuntimeAdapter, RuntimeStats, SyncError, SyncResult, ViewCall,
    ViewResult,
};

/// Real runtime adapter that wraps MetashrewRuntime
pub struct MetashrewRuntimeAdapter<T: KeyValueStoreLike + Clone + Send + Sync + 'static> {
    runtime: Arc<Mutex<MetashrewRuntime<T>>>,
}

impl<T: KeyValueStoreLike + Clone + Send + Sync + 'static> MetashrewRuntimeAdapter<T> {
    pub fn new(runtime: MetashrewRuntime<T>) -> Self {
        Self {
            runtime: Arc::new(Mutex::new(runtime)),
        }
    }

    pub fn from_arc(runtime: Arc<Mutex<MetashrewRuntime<T>>>) -> Self {
        Self { runtime }
    }
}

impl<T: KeyValueStoreLike + Clone + Send + Sync + 'static> Clone for MetashrewRuntimeAdapter<T> {
    fn clone(&self) -> Self {
        Self {
            runtime: self.runtime.clone(),
        }
    }
}

#[async_trait]
impl<T: KeyValueStoreLike + Clone + Send + Sync + 'static> RuntimeAdapter
    for MetashrewRuntimeAdapter<T>
{
    async fn process_block(&mut self, height: u32, block_data: &[u8]) -> SyncResult<()> {
        let mut runtime = self.runtime.lock().await;
        {
            let mut context = runtime
                .context
                .lock()
                .map_err(|e| SyncError::Runtime(format!("Failed to lock context: {}", e)))?;
            context.block = block_data.to_vec();
            context.height = height;
            context.db.set_height(height);
        }
        runtime.run().map_err(|e| {
            SyncError::Runtime(format!("Runtime execution failed: {}", e))
        })?;
        Ok(())
    }

    async fn process_block_atomic(
        &mut self,
        height: u32,
        block_data: &[u8],
        block_hash: &[u8],
    ) -> SyncResult<AtomicBlockResult> {
        let mut runtime = self.runtime.lock().await;
        match runtime.process_block_atomic(height, block_data, block_hash).await {
            Ok(result) => {
                Ok(AtomicBlockResult {
                    state_root: result.state_root,
                    batch_data: result.batch_data,
                    height: result.height,
                    block_hash: result.block_hash,
                })
            }
            Err(e) => Err(SyncError::Runtime(format!(
                "Atomic block processing failed: {}",
                e
            ))),
        }
    }

    async fn execute_view(&self, call: ViewCall) -> SyncResult<ViewResult> {
        let runtime = self.runtime.lock().await;
        let result = runtime
            .view(call.function_name, &call.input_data, call.height)
            .await
            .map_err(|e| SyncError::ViewFunction(format!("View function failed: {}", e)))?;
        Ok(ViewResult { data: result })
    }

    async fn execute_preview(&self, call: PreviewCall) -> SyncResult<ViewResult> {
        let runtime = self.runtime.lock().await;
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

    async fn get_state_root(&self, height: u32) -> SyncResult<Vec<u8>> {
        let runtime = self.runtime.lock().await;
        runtime
            .get_state_root(height)
            .await
            .map_err(|e| SyncError::Runtime(format!("Failed to get state root for height {}: {}", height, e)))
    }

    async fn refresh_memory(&mut self) -> SyncResult<()> {
        log::info!("Manual memory refresh requested - note that memory is now refreshed automatically after each block");
        Ok(())
    }

    async fn is_ready(&self) -> bool {
        true
    }

    async fn get_stats(&self) -> SyncResult<RuntimeStats> {
        let runtime = self.runtime.lock().await;
        let memory_usage_bytes = 0;
        let blocks_processed = {
            let context = runtime
                .context
                .lock()
                .map_err(|e| SyncError::Runtime(format!("Failed to lock context: {}", e)))?;
            context.height
        };
        Ok(RuntimeStats {
            memory_usage_bytes,
            blocks_processed,
            last_refresh_height: Some(blocks_processed),
        })
    }
}