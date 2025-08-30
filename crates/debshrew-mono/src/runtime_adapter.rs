//! # Debshrew Runtime Adapter
//!
//! ## PURPOSE
//! Wraps the existing MetashrewRuntimeAdapter to add CDC functionality.
//! Intercepts k/v flushes during block processing and generates CDC events.

use crate::cdc_config::CDCConfig;
use crate::cdc_tracker::CDCTracker;
use anyhow::Result;
use async_trait::async_trait;
use metashrew_runtime::MetashrewRuntime;
use metashrew_sync::{
    AtomicBlockResult, PreviewCall, RuntimeAdapter, RuntimeStats, SyncResult, ViewCall, ViewResult,
};
use rockshrew_mono::MetashrewRuntimeAdapter;
use rockshrew_runtime::RocksDBRuntimeAdapter;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Enhanced runtime adapter with CDC functionality
pub struct DebshrewRuntimeAdapter {
    /// Underlying rockshrew-mono runtime adapter
    inner: MetashrewRuntimeAdapter,
    /// Optional CDC tracker for streaming events
    cdc_tracker: Option<CDCTracker>,
}

impl DebshrewRuntimeAdapter {
    /// Create a new runtime adapter with optional CDC support
    pub async fn new_with_cdc(cdc_config: Option<CDCConfig>) -> Result<Self> {
        // This is a placeholder - in the real implementation, you'd need to:
        // 1. Create the MetashrewRuntime with RocksDB
        // 2. Wrap it in MetashrewRuntimeAdapter
        // 3. Set up CDC tracking if configured
        
        todo!("Complete integration with actual runtime creation")
    }

    /// Create a new runtime adapter wrapping an existing MetashrewRuntimeAdapter
    pub async fn new(
        inner: MetashrewRuntimeAdapter,
        cdc_config: Option<CDCConfig>,
    ) -> Result<Self> {
        let cdc_tracker = if let Some(config) = cdc_config {
            Some(CDCTracker::new(config).await?)
        } else {
            None
        };

        Ok(Self {
            inner,
            cdc_tracker,
        })
    }

    /// Create a runtime adapter from a MetashrewRuntime instance
    pub async fn from_runtime(
        runtime: Arc<RwLock<MetashrewRuntime<RocksDBRuntimeAdapter>>>,
        cdc_config: Option<CDCConfig>,
    ) -> Result<Self> {
        let inner = MetashrewRuntimeAdapter::new(runtime);
        Self::new(inner, cdc_config).await
    }

    /// Set up CDC tracking for the current block
    async fn setup_cdc_tracking(&self, height: u32, block_hash: Option<&[u8]>) -> Result<()> {
        if let Some(cdc_tracker) = &self.cdc_tracker {
            cdc_tracker.set_current_height(height);
            
            if let Some(hash) = block_hash {
                cdc_tracker.set_current_block_hash(hex::encode(hash));
            }

            // TODO: Install the KV tracker in the runtime
            // This would require modifications to the MetashrewRuntime to accept
            // a KV tracker function that gets called during __flush operations
            
            log::debug!("CDC tracking set up for block {}", height);
        }
        Ok(())
    }

    /// Flush CDC events after successful block processing
    async fn flush_cdc_events(&self, height: u32) -> Result<()> {
        if let Some(cdc_tracker) = &self.cdc_tracker {
            match cdc_tracker.flush_events().await {
                Ok(()) => {
                    log::debug!("Successfully flushed CDC events for block {}", height);
                }
                Err(e) => {
                    log::error!("Failed to flush CDC events for block {}: {}", height, e);
                    // Don't fail the block processing, but log the error
                    // In production, you might want to implement dead letter queues
                    // or other error handling strategies
                }
            }
        }
        Ok(())
    }

    /// Get CDC tracker statistics
    pub fn get_cdc_stats(&self) -> Option<CDCStats> {
        self.cdc_tracker.as_ref().map(|tracker| CDCStats {
            buffer_size: tracker.get_buffer_size(),
            is_enabled: true,
        })
    }

    /// Check if CDC is enabled
    pub fn is_cdc_enabled(&self) -> bool {
        self.cdc_tracker.is_some()
    }
}

/// CDC statistics for monitoring
#[derive(Debug, Clone)]
pub struct CDCStats {
    pub buffer_size: usize,
    pub is_enabled: bool,
}

#[async_trait]
impl RuntimeAdapter for DebshrewRuntimeAdapter {
    async fn process_block(&mut self, height: u32, block_data: &[u8]) -> SyncResult<()> {
        // Set up CDC tracking for this block
        self.setup_cdc_tracking(height, None).await
            .map_err(|e| metashrew_sync::SyncError::Runtime(format!("CDC setup failed: {}", e)))?;

        // Process block with the underlying adapter
        let result = self.inner.process_block(height, block_data).await;

        // Flush CDC events if block processing succeeded
        if result.is_ok() {
            self.flush_cdc_events(height).await
                .map_err(|e| metashrew_sync::SyncError::Runtime(format!("CDC flush failed: {}", e)))?;
        }

        result
    }

    async fn process_block_atomic(
        &mut self,
        height: u32,
        block_data: &[u8],
        block_hash: &[u8],
    ) -> SyncResult<AtomicBlockResult> {
        // Set up CDC tracking for this block
        self.setup_cdc_tracking(height, Some(block_hash)).await
            .map_err(|e| metashrew_sync::SyncError::Runtime(format!("CDC setup failed: {}", e)))?;

        // Process block atomically with the underlying adapter
        let result = self.inner.process_block_atomic(height, block_data, block_hash).await;

        // Flush CDC events if block processing succeeded
        if result.is_ok() {
            self.flush_cdc_events(height).await
                .map_err(|e| metashrew_sync::SyncError::Runtime(format!("CDC flush failed: {}", e)))?;
        }

        result
    }

    async fn execute_view(&self, call: ViewCall) -> SyncResult<ViewResult> {
        // View operations don't generate CDC events, just delegate
        self.inner.execute_view(call).await
    }

    async fn execute_preview(&self, call: PreviewCall) -> SyncResult<ViewResult> {
        // Preview operations don't generate CDC events, just delegate
        self.inner.execute_preview(call).await
    }

    async fn get_state_root(&self, height: u32) -> SyncResult<Vec<u8>> {
        self.inner.get_state_root(height).await
    }

    async fn refresh_memory(&mut self) -> SyncResult<()> {
        self.inner.refresh_memory().await
    }

    async fn is_ready(&self) -> bool {
        self.inner.is_ready().await
    }

    async fn get_stats(&self) -> SyncResult<RuntimeStats> {
        // Get base stats from the inner adapter
        let mut stats = self.inner.get_stats().await?;

        // Add CDC information to memory usage if CDC is enabled
        if let Some(cdc_stats) = self.get_cdc_stats() {
            // Rough estimate of CDC memory usage
            let cdc_memory = cdc_stats.buffer_size * 1024; // Assume ~1KB per event
            stats.memory_usage_bytes += cdc_memory;
        }

        Ok(stats)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cdc_config::{CDCConfig, GlobalCDCSettings, KafkaConfig};

    #[tokio::test]
    async fn test_cdc_disabled() {
        // Create adapter without CDC
        let mock_inner = create_mock_runtime_adapter();
        let adapter = DebshrewRuntimeAdapter::new(mock_inner, None).await.unwrap();

        assert!(!adapter.is_cdc_enabled());
        assert!(adapter.get_cdc_stats().is_none());
    }

    #[tokio::test]
    async fn test_cdc_enabled() {
        // Create adapter with CDC
        let mock_inner = create_mock_runtime_adapter();
        let cdc_config = CDCConfig {
            relationships: vec![],
            kafka_config: KafkaConfig::default(),
            schema_registry: None,
            global_settings: GlobalCDCSettings::default(),
        };

        let adapter = DebshrewRuntimeAdapter::new(mock_inner, Some(cdc_config)).await.unwrap();

        assert!(adapter.is_cdc_enabled());
        assert!(adapter.get_cdc_stats().is_some());
    }

    fn create_mock_runtime_adapter() -> MetashrewRuntimeAdapter {
        // This is a placeholder - in real tests you'd create a proper mock
        todo!("Create mock MetashrewRuntimeAdapter for testing")
    }
}