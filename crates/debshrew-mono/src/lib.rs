//! # Debshrew-Mono: CDC-Enabled Bitcoin Indexer
//!
//! ## PURPOSE
//! Debshrew-mono extends rockshrew-mono with Change Data Capture (CDC) functionality,
//! enabling real-time streaming of k/v updates to external systems via Kafka and Debezium.
//!
//! ## ARCHITECTURE
//! - Wraps existing rockshrew-mono components
//! - Intercepts k/v flushes during block processing
//! - Transforms k/v updates into relational CDC events
//! - Streams events to Kafka in Debezium-compatible format
//! - Maintains full compatibility with existing indexer functionality
//!
//! ## KEY FEATURES
//! - Configurable relationship mappings between k/v pairs and tables
//! - Real-time CDC event generation (CREATE/UPDATE/DELETE)
//! - Kafka integration with Debezium compatibility
//! - Schema evolution support
//! - Minimal performance impact on core indexing

pub mod cdc_config;
pub mod cdc_tracker;
pub mod kafka_producer;
pub mod relationship_matcher;
pub mod runtime_adapter;
pub mod schema_manager;

// Re-export key types
pub use cdc_config::{CDCConfig, CDCRelationship, OperationType};
pub use cdc_tracker::{CDCTracker, CDCEvent, CDCOperation};
pub use runtime_adapter::DebshrewRuntimeAdapter;

// Re-export rockshrew-mono functionality
pub use rockshrew_mono::*;

use anyhow::Result;
use std::path::Path;

/// Initialize debshrew-mono with CDC configuration
pub async fn initialize_with_cdc(
    rockshrew_args: rockshrew_mono::Args,
    cdc_config_path: Option<&Path>,
) -> Result<()> {
    let cdc_config = if let Some(config_path) = cdc_config_path {
        Some(CDCConfig::load_from_file(config_path)?)
    } else {
        None
    };

    // Set up the enhanced runtime adapter with CDC support
    let runtime_adapter = DebshrewRuntimeAdapter::new_with_cdc(cdc_config).await?;

    // Use the standard rockshrew-mono run function with our enhanced adapter
    // This maintains full compatibility while adding CDC functionality
    todo!("Complete integration with rockshrew-mono::run")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_cdc_config_loading() {
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("cdc_config.yaml");
        
        let config = CDCConfig::default();
        config.save_to_file(&config_path).unwrap();
        
        let loaded_config = CDCConfig::load_from_file(&config_path).unwrap();
        assert_eq!(config.kafka_config.topic_prefix, loaded_config.kafka_config.topic_prefix);
    }
}