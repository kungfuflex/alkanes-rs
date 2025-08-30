//! # Debshrew-Mono Main Binary
//!
//! ## PURPOSE
//! Main entry point for the debshrew-mono CDC-enabled Bitcoin indexer.
//! Extends rockshrew-mono with Change Data Capture functionality.

use anyhow::{anyhow, Result};
use clap::Parser;
use debshrew_mono::{CDCConfig, DebshrewRuntimeAdapter};
use log::info;
use metashrew_runtime::MetashrewRuntime;
use metashrew_sync::SnapshotMetashrewSync;
use rockshrew_mono::{
    adapters::{BitcoinRpcAdapter, MetashrewRuntimeAdapter},
    ssh_tunnel::parse_daemon_rpc_url,
};
use rockshrew_runtime::{RocksDBRuntimeAdapter, RocksDBStorageAdapter};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Command-line arguments for debshrew-mono
#[derive(Parser, Debug, Clone)]
#[command(version, about = "CDC-enabled Bitcoin indexer with Kafka streaming")]
pub struct Args {
    /// Inherit all rockshrew-mono arguments
    #[command(flatten)]
    pub rockshrew_args: rockshrew_mono::Args,
    
    /// Enable CDC functionality
    #[arg(long, help = "Enable Change Data Capture streaming")]
    pub enable_cdc: bool,
    
    /// Path to CDC configuration file
    #[arg(long, help = "Path to CDC configuration YAML file")]
    pub cdc_config: Option<PathBuf>,
    
    /// Kafka bootstrap servers (overrides config file)
    #[arg(long, help = "Kafka bootstrap servers (comma-separated)")]
    pub kafka_servers: Option<String>,
    
    /// Kafka topic prefix (overrides config file)
    #[arg(long, help = "Kafka topic prefix for CDC events")]
    pub kafka_topic_prefix: Option<String>,
    
    /// Dry run mode - don't actually send to Kafka
    #[arg(long, help = "Dry run mode - log events but don't send to Kafka")]
    pub dry_run: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    init_logging();
    
    let args = Args::parse();
    
    info!("Starting debshrew-mono v{}", env!("CARGO_PKG_VERSION"));
    info!("CDC enabled: {}", args.enable_cdc);
    
    // Load and validate CDC configuration if enabled
    let cdc_config = if args.enable_cdc {
        Some(load_cdc_config(&args).await?)
    } else {
        None
    };
    
    // Set up the enhanced runtime with CDC support
    run_with_cdc(args, cdc_config).await
}

/// Initialize logging based on environment variables
fn init_logging() {
    // Check if console debugging is requested via environment variable
    let _use_console = std::env::var("DEBSHREW_CONSOLE").is_ok();
    
    #[cfg(feature = "console")]
    if _use_console {
        console_subscriber::init();
        return;
    }
    
    // Check if JSON tracing is requested
    if std::env::var("DEBSHREW_JSON_TRACING").is_ok() {
        use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
        
        tracing_subscriber::registry()
            .with(tracing_subscriber::fmt::layer().json())
            .with(tracing_subscriber::EnvFilter::from_default_env())
            .init();
        return;
    }
    
    // Enhanced tracing for debugging
    if std::env::var("DEBSHREW_DEBUG").is_ok() {
        use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
        
        tracing_subscriber::registry()
            .with(
                tracing_subscriber::fmt::layer()
                    .with_target(true)
                    .with_thread_ids(true)
                    .with_line_number(true)
                    .with_file(true)
            )
            .with(tracing_subscriber::EnvFilter::from_default_env())
            .init();
        return;
    }
    
    // Default: use env_logger for backward compatibility
    env_logger::builder()
        .format_timestamp_secs()
        .filter_level(log::LevelFilter::Info)
        .init();
}

/// Load and validate CDC configuration
async fn load_cdc_config(args: &Args) -> Result<CDCConfig> {
    let mut config = if let Some(config_path) = &args.cdc_config {
        info!("Loading CDC configuration from: {}", config_path.display());
        CDCConfig::load_from_file(config_path)?
    } else {
        info!("No CDC config file provided, using defaults");
        CDCConfig::default()
    };
    
    // Apply command-line overrides
    if let Some(kafka_servers) = &args.kafka_servers {
        info!("Overriding Kafka servers: {}", kafka_servers);
        config.kafka_config.bootstrap_servers = kafka_servers.clone();
    }
    
    if let Some(topic_prefix) = &args.kafka_topic_prefix {
        info!("Overriding Kafka topic prefix: {}", topic_prefix);
        config.kafka_config.topic_prefix = topic_prefix.clone();
    }
    
    // Validate configuration
    config.validate()?;
    
    info!("CDC configuration loaded successfully:");
    info!("  - Relationships: {}", config.relationships.len());
    info!("  - Kafka servers: {}", config.kafka_config.bootstrap_servers);
    info!("  - Topic prefix: {}", config.kafka_config.topic_prefix);
    info!("  - Dry run mode: {}", args.dry_run);
    
    Ok(config)
}

/// Run debshrew-mono with CDC support
async fn run_with_cdc(args: Args, cdc_config: Option<CDCConfig>) -> Result<()> {
    // Parse daemon RPC URL (same as rockshrew-mono)
    let (rpc_url, bypass_ssl, tunnel_config) =
        parse_daemon_rpc_url(&args.rockshrew_args.daemon_rpc_url).await?;

    info!("Initializing RocksDB with performance-optimized configuration");
    info!("Database path: {}", args.rockshrew_args.db_path.display());

    // Create the MetashrewRuntime with RocksDB backend
    let runtime = MetashrewRuntime::load(
        args.rockshrew_args.indexer.clone(),
        RocksDBRuntimeAdapter::open_optimized(
            args.rockshrew_args.db_path.to_string_lossy().to_string()
        )?,
    )?;

    let db = {
        let context = runtime.context.lock()
            .map_err(|_| anyhow!("Failed to lock runtime context"))?;
        context.db.db.clone()
    };

    // Create adapters
    let node_adapter = BitcoinRpcAdapter::new(
        rpc_url,
        args.rockshrew_args.auth.clone(),
        bypass_ssl,
        tunnel_config,
    );
    
    let storage_adapter = RocksDBStorageAdapter::new(db.clone());
    
    // Create the enhanced runtime adapter with CDC support
    let runtime_adapter = if cdc_config.is_some() {
        info!("Creating CDC-enabled runtime adapter");
        DebshrewRuntimeAdapter::from_runtime(
            Arc::new(RwLock::new(runtime)),
            cdc_config,
        ).await?
    } else {
        info!("Creating standard runtime adapter (CDC disabled)");
        // For non-CDC mode, we could still use DebshrewRuntimeAdapter with None config
        // or fall back to the standard MetashrewRuntimeAdapter
        DebshrewRuntimeAdapter::from_runtime(
            Arc::new(RwLock::new(runtime)),
            None,
        ).await?
    };

    // Create sync configuration
    let sync_config = metashrew_sync::SyncConfig {
        start_block: args.rockshrew_args.start_block.unwrap_or(0),
        exit_at: args.rockshrew_args.exit_at,
        pipeline_size: args.rockshrew_args.pipeline_size,
        max_reorg_depth: args.rockshrew_args.max_reorg_depth,
        reorg_check_threshold: args.rockshrew_args.reorg_check_threshold,
    };

    // Determine sync mode
    let sync_mode = if args.rockshrew_args.snapshot_directory.is_some() {
        metashrew_sync::SyncMode::Snapshot(Default::default())
    } else if args.rockshrew_args.repo.is_some() {
        metashrew_sync::SyncMode::Repo(Default::default())
    } else {
        metashrew_sync::SyncMode::Normal
    };

    // Create the sync engine
    let sync_engine = SnapshotMetashrewSync::new(
        node_adapter,
        storage_adapter,
        runtime_adapter,
        sync_config,
        sync_mode,
    );

    let sync_engine_arc = Arc::new(tokio::sync::RwLock::new(sync_engine));

    // Set up signal handling for graceful shutdown
    let shutdown_signal = setup_signal_handler().await;

    // Start the indexer
    info!("Starting debshrew-mono indexer...");
    let indexer_handle = tokio::spawn({
        let sync_engine_clone = sync_engine_arc.clone();
        async move {
            let mut engine = sync_engine_clone.write().await;
            if let Err(e) = engine.start().await {
                log::error!("Indexer failed: {}", e);
            }
        }
    });

    // Start the JSON-RPC server (reuse rockshrew-mono's server logic)
    let server_handle = tokio::spawn({
        let sync_engine_clone = sync_engine_arc.clone();
        let args_clone = args.rockshrew_args.clone();
        async move {
            if let Err(e) = start_json_rpc_server(sync_engine_clone, args_clone).await {
                log::error!("JSON-RPC server failed: {}", e);
            }
        }
    });

    info!("Debshrew-mono is running. Press Ctrl+C to stop.");

    // Wait for shutdown signal or task completion
    tokio::select! {
        result = indexer_handle => {
            if let Err(e) = result {
                log::error!("Indexer task failed: {}", e);
            }
        }
        result = server_handle => {
            if let Err(e) = result {
                log::error!("Server task failed: {}", e);
            }
        }
        _ = shutdown_signal => {
            info!("Shutdown signal received, stopping debshrew-mono...");
        }
    }

    info!("Debshrew-mono shutdown complete.");
    Ok(())
}

/// Set up signal handler for graceful shutdown
async fn setup_signal_handler() -> tokio::signal::unix::Signal {
    use tokio::signal::unix::{signal, SignalKind};
    
    let mut sigterm = signal(SignalKind::terminate())
        .expect("Failed to install SIGTERM signal handler");
    let mut sigint = signal(SignalKind::interrupt())
        .expect("Failed to install SIGINT signal handler");

    tokio::select! {
        _ = sigterm.recv() => {},
        _ = sigint.recv() => {},
    }

    sigterm
}

/// Start the JSON-RPC server (placeholder - would integrate with rockshrew-mono's server)
async fn start_json_rpc_server(
    _sync_engine: Arc<tokio::sync::RwLock<SnapshotMetashrewSync<BitcoinRpcAdapter, RocksDBStorageAdapter, DebshrewRuntimeAdapter>>>,
    _args: rockshrew_mono::Args,
) -> Result<()> {
    // TODO: Integrate with rockshrew-mono's JSON-RPC server
    // This would involve creating the Actix web server with the same endpoints
    // but using our enhanced sync engine
    
    info!("JSON-RPC server would start here");
    
    // For now, just wait indefinitely
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_args_parsing() {
        let args = Args::try_parse_from(&[
            "debshrew-mono",
            "--daemon-rpc-url", "http://localhost:8332",
            "--indexer", "test.wasm",
            "--db-path", "/tmp/test",
            "--enable-cdc",
            "--kafka-servers", "localhost:9092",
            "--kafka-topic-prefix", "test.cdc",
        ]).unwrap();

        assert!(args.enable_cdc);
        assert_eq!(args.kafka_servers, Some("localhost:9092".to_string()));
        assert_eq!(args.kafka_topic_prefix, Some("test.cdc".to_string()));
    }

    #[tokio::test]
    async fn test_cdc_config_loading() {
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("cdc_config.yaml");
        
        // Create a test config file
        let config = CDCConfig::default();
        config.save_to_file(&config_path).unwrap();
        
        let args = Args {
            rockshrew_args: rockshrew_mono::Args {
                daemon_rpc_url: "http://localhost:8332".to_string(),
                indexer: PathBuf::from("test.wasm"),
                db_path: PathBuf::from("/tmp/test"),
                start_block: None,
                auth: None,
                host: "127.0.0.1".to_string(),
                port: 8080,
                label: None,
                exit_at: None,
                pipeline_size: None,
                cors: None,
                snapshot_directory: None,
                snapshot_interval: 1000,
                repo: None,
                max_reorg_depth: 100,
                reorg_check_threshold: 6,
            },
            enable_cdc: true,
            cdc_config: Some(config_path),
            kafka_servers: None,
            kafka_topic_prefix: None,
            dry_run: false,
        };
        
        let loaded_config = load_cdc_config(&args).await.unwrap();
        assert_eq!(loaded_config.kafka_config.bootstrap_servers, "localhost:9092");
    }
}