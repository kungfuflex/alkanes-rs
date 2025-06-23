//! Simplified rockshrew-mono using generic framework components
//!
//! This is the new lightweight main.rs that demonstrates how the generic
//! framework reduces code complexity from 992 lines to ~200 lines.

use anyhow::{anyhow, Result};
use clap::Parser;
use env_logger;
use log::{error, info};
use metashrew_runtime::{set_label, MetashrewRuntime};
use num_cpus;
use rocksdb::Options;
use rockshrew_runtime::RocksDBRuntimeAdapter;
use rockshrew_sync::{
    SyncConfig, SnapshotMetashrewSync, SyncMode, SyncEngine,
    RepoConfig, SnapshotConfig as GenericSnapshotConfig,
    MetashrewJsonRpcServer,
    jsonrpc::server::ServerConfig as JsonRpcServerConfig,
};
use std::path::PathBuf;
use std::sync::atomic::AtomicU32;
use std::sync::Arc;
use tokio::sync::RwLock;

// Import our modules (still needed for RocksDB-specific implementations)
mod smt_helper;
mod adapters;
use adapters::{BitcoinRpcAdapter, MetashrewRuntimeAdapter, RocksDBStorageAdapter};

// Import our SSH tunneling module (still needed for Bitcoin RPC)
mod ssh_tunnel;
use ssh_tunnel::parse_daemon_rpc_url;

// Import our snapshot module (still needed for legacy snapshot support)
mod snapshot;
use snapshot::{SnapshotConfig, SnapshotManager};

// Import our snapshot adapters for generic framework integration
mod snapshot_adapters;
use snapshot_adapters::{RockshrewSnapshotProvider, RockshrewSnapshotConsumer};

// Use the original Args structure for now
#[derive(Parser, Debug, Clone)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(long)]
    daemon_rpc_url: String,
    #[arg(long, required_unless_present = "repo")]
    indexer: Option<PathBuf>,
    #[arg(long)]
    db_path: PathBuf,
    #[arg(long)]
    start_block: Option<u32>,
    #[arg(long)]
    auth: Option<String>,
    #[arg(long, env = "HOST", default_value = "127.0.0.1")]
    host: String,
    #[arg(long, env = "PORT", default_value_t = 8080)]
    port: u16,
    #[arg(long)]
    label: Option<String>,
    #[arg(long)]
    exit_at: Option<u32>,
    #[arg(long, help = "Size of the processing pipeline")]
    pipeline_size: Option<usize>,
    #[arg(long, help = "CORS allowed origins")]
    cors: Option<String>,
    #[arg(long, help = "Directory to store snapshots")]
    snapshot_directory: Option<PathBuf>,
    #[arg(long, help = "Interval in blocks to create snapshots", default_value_t = 1000)]
    snapshot_interval: u32,
    #[arg(long, help = "URL to a remote snapshot repository")]
    repo: Option<String>,
    #[arg(long, help = "Maximum reorg depth", default_value_t = 100)]
    max_reorg_depth: u32,
    #[arg(long, help = "Reorg check threshold", default_value_t = 6)]
    reorg_check_threshold: u32,
}

// JSON-RPC response structures needed by adapters.rs
#[derive(serde::Serialize, serde::Deserialize)]
#[allow(dead_code)]
pub struct JsonRpcRequest {
    pub id: u32,
    pub jsonrpc: String,
    pub method: String,
    pub params: Vec<serde_json::Value>,
}

#[derive(serde::Deserialize)]
#[allow(dead_code)]
pub struct BlockCountResponse {
    pub id: u32,
    pub result: Option<u32>,
    pub error: Option<serde_json::Value>,
}

#[derive(serde::Deserialize)]
#[allow(dead_code)]
pub struct BlockHashResponse {
    pub id: u32,
    pub result: Option<String>,
    pub error: Option<serde_json::Value>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logger
    env_logger::builder().format_timestamp_secs().init();
    info!("Starting Metashrew Indexer (rockshrew-mono) with simplified architecture");
    info!("System has {} CPU cores available", num_cpus::get());

    // Parse command line arguments
    let args = Args::parse();

    // Set the label if provided
    if let Some(ref label) = args.label {
        set_label(label.clone());
    }

    // Parse the daemon RPC URL to determine if SSH tunneling is needed
    let (rpc_url, bypass_ssl, tunnel_config) = parse_daemon_rpc_url(&args.daemon_rpc_url).await?;
    info!("Parsed RPC URL: {}", rpc_url);

    // Configure RocksDB with optimal settings
    let opts = configure_rocksdb_options();
    let mut start_block = args.start_block.unwrap_or(0);

    // Handle repository sync if needed
    let indexer_path = handle_repository_sync(&args, &mut start_block).await?;

    // Create runtime with RocksDB adapter
    let runtime = Arc::new(RwLock::new(MetashrewRuntime::load(
        indexer_path.clone(),
        RocksDBRuntimeAdapter::open(args.db_path.to_string_lossy().to_string(), opts)?,
    )?));

    info!("Successfully loaded WASM module from {}", indexer_path.display());

    // Get database handle for adapters
    let db = extract_database_handle(&runtime).await?;

    // Create sync configuration
    let sync_config = SyncConfig {
        start_block,
        exit_at: args.exit_at,
        pipeline_size: args.pipeline_size,
        max_reorg_depth: args.max_reorg_depth,
        reorg_check_threshold: args.reorg_check_threshold,
    };

    // Determine sync mode
    let sync_mode = determine_sync_mode(&args);
    info!("Using sync mode: {:?}", sync_mode);

    // Create adapters for the sync framework
    let bitcoin_adapter = BitcoinRpcAdapter::new(rpc_url, args.auth.clone(), bypass_ssl, tunnel_config);
    let storage_adapter = RocksDBStorageAdapter::new(db.clone());
    let runtime_adapter = MetashrewRuntimeAdapter::new(runtime.clone(), db.clone());

    // Create shared references for JSON-RPC server
    let storage_adapter_ref = Arc::new(RwLock::new(RocksDBStorageAdapter::new(db.clone())));
    let runtime_adapter_ref = Arc::new(RwLock::new(MetashrewRuntimeAdapter::new(runtime.clone(), db.clone())));
    let current_height = Arc::new(AtomicU32::new(start_block));

    // Create sync engine
    let sync_engine = SnapshotMetashrewSync::new(
        bitcoin_adapter,
        storage_adapter,
        runtime_adapter,
        sync_config,
        sync_mode.clone(),
    );

    // Set up snapshot components
    setup_snapshot_components(&sync_engine, &sync_mode, &args, &storage_adapter_ref, &indexer_path).await?;

    let sync_engine = Arc::new(RwLock::new(sync_engine));

    // Create and start the JSON-RPC server using the generic framework
    let server_config = JsonRpcServerConfig {
        host: args.host.clone(),
        port: args.port,
        cors: args.cors.clone(),
    };

    let json_rpc_server = MetashrewJsonRpcServer::new(
        storage_adapter_ref,
        runtime_adapter_ref,
        current_height.clone(),
        server_config,
    );

    // Start the indexer in a separate task
    let indexer_handle = {
        let sync_engine_clone = sync_engine.clone();
        tokio::spawn(async move {
            info!("Starting block indexing process from height {}", start_block);
            if let Err(e) = sync_engine_clone.write().await.start().await {
                error!("Indexer error: {}", e);
            }
        })
    };

    // Start the JSON-RPC server
    let server_handle = tokio::spawn(async move {
        if let Err(e) = json_rpc_server.start().await {
            error!("Server error: {}", e);
        }
    });

    info!("JSON-RPC server running at http://{}:{}", args.host, args.port);
    info!("Indexer is ready and processing blocks");
    info!("Available RPC methods: metashrew_view, metashrew_preview, metashrew_height, metashrew_getblockhash, metashrew_stateroot, metashrew_snapshot");

    // Wait for either component to finish
    tokio::select! {
        result = indexer_handle => {
            if let Err(e) = result {
                error!("Indexer task failed: {}", e);
            }
        }
        result = server_handle => {
            if let Err(e) = result {
                error!("Server task failed: {}", e);
            }
        }
    }

    Ok(())
}

/// Configure RocksDB options for optimal performance
fn configure_rocksdb_options() -> Options {
    let mut opts = Options::default();
    let available_cpus = num_cpus::get();

    // Calculate optimal settings based on CPU cores
    let background_jobs: i32 = std::cmp::min(std::cmp::max(4, available_cpus / 4), 16).try_into().unwrap();
    let write_buffer_number: i32 = std::cmp::min(std::cmp::max(6, available_cpus / 6), 12).try_into().unwrap();

    info!("Configuring RocksDB with {} background jobs and {} write buffers", background_jobs, write_buffer_number);

    opts.create_if_missing(true);
    opts.set_max_open_files(10000);
    opts.set_use_fsync(false);
    opts.set_bytes_per_sync(8388608);
    opts.optimize_for_point_lookup(1024);
    opts.set_table_cache_num_shard_bits(6);
    opts.set_max_write_buffer_number(write_buffer_number);
    opts.set_write_buffer_size(256 * 1024 * 1024);
    opts.set_target_file_size_base(256 * 1024 * 1024);
    opts.set_min_write_buffer_number_to_merge(2);
    opts.set_level_zero_file_num_compaction_trigger(4);
    opts.set_level_zero_slowdown_writes_trigger(20);
    opts.set_level_zero_stop_writes_trigger(30);
    opts.set_max_background_jobs(background_jobs);
    opts.set_disable_auto_compactions(false);

    opts
}

/// Handle repository synchronization if needed
async fn handle_repository_sync(args: &Args, start_block: &mut u32) -> Result<PathBuf> {
    if let Some(ref repo_url) = args.repo {
        info!("Repository URL provided: {}", repo_url);

        let config = SnapshotConfig {
            interval: args.snapshot_interval,
            directory: PathBuf::from("temp_sync"),
            enabled: true,
        };

        let mut sync_manager = SnapshotManager::new(config);

        match sync_manager.sync_from_repo(repo_url, &args.db_path, args.indexer.as_ref()).await {
            Ok((height, wasm_path)) => {
                info!("Successfully synced from repository to height {}", height);
                if *start_block < height {
                    info!("Adjusting start block from {} to {}", *start_block, height);
                    *start_block = height;
                }

                if args.indexer.is_none() {
                    if let Some(path) = wasm_path {
                        info!("Using WASM file from repository: {:?}", path);
                        return Ok(path);
                    } else {
                        return Err(anyhow!("No WASM file provided or found in repository"));
                    }
                }
            }
            Err(e) => {
                error!("Failed to sync from repository: {}", e);
                return Err(anyhow!("Failed to sync from repository: {}", e));
            }
        }
    }

    // Use provided indexer or error if none
    args.indexer.clone().ok_or_else(|| anyhow!("No indexer WASM file provided"))
}

/// Extract database handle from runtime
async fn extract_database_handle(runtime: &Arc<RwLock<MetashrewRuntime<RocksDBRuntimeAdapter>>>) -> Result<Arc<rocksdb::DB>> {
    let runtime_guard = runtime.read().await;
    let context = runtime_guard.context.lock().map_err(|_| anyhow!("Failed to lock context"))?;
    Ok(context.db.db.clone())
}

/// Determine sync mode based on command-line arguments
fn determine_sync_mode(args: &Args) -> SyncMode {
    if args.snapshot_directory.is_some() {
        let snapshot_config = GenericSnapshotConfig {
            snapshot_interval: args.snapshot_interval,
            max_snapshots: 10,
            compression_level: 6,
            reorg_buffer_size: 100,
        };
        SyncMode::Snapshot(snapshot_config)
    } else if args.repo.is_some() {
        let repo_config = RepoConfig {
            repo_url: args.repo.clone().unwrap(),
            check_interval: 300,
            max_snapshot_age: 86400,
            continue_sync: true,
            min_blocks_behind: 100,
        };
        SyncMode::Repo(repo_config)
    } else {
        SyncMode::Normal
    }
}

/// Set up snapshot components based on sync mode
async fn setup_snapshot_components(
    sync_engine: &SnapshotMetashrewSync<BitcoinRpcAdapter, RocksDBStorageAdapter, MetashrewRuntimeAdapter>,
    sync_mode: &SyncMode,
    args: &Args,
    storage_adapter_ref: &Arc<RwLock<RocksDBStorageAdapter>>,
    indexer_path: &PathBuf,
) -> Result<()> {
    match sync_mode {
        SyncMode::Snapshot(config) => {
            info!("Setting up snapshot provider for snapshot creation mode");
            
            let snapshot_config = SnapshotConfig {
                interval: config.snapshot_interval,
                directory: args.snapshot_directory.clone().unwrap_or_else(|| PathBuf::from("snapshots")),
                enabled: true,
            };
            
            let provider = RockshrewSnapshotProvider::new(snapshot_config, storage_adapter_ref.clone());
            
            let current_height = {
                let storage = storage_adapter_ref.read().await;
                storage.get_current_height().await.unwrap_or(args.start_block.unwrap_or(0))
            };
            
            provider.initialize_with_height(current_height).await?;
            provider.set_current_wasm(indexer_path.clone()).await?;
            
            info!("Successfully initialized snapshot provider");
            sync_engine.set_snapshot_provider(Box::new(provider)).await;
        }
        SyncMode::Repo(_config) => {
            info!("Setting up snapshot consumer for repository mode");
            
            let snapshot_config = SnapshotConfig {
                interval: 1000,
                directory: PathBuf::from("temp_snapshots"),
                enabled: true,
            };
            
            let consumer = RockshrewSnapshotConsumer::new(snapshot_config, storage_adapter_ref.clone());
            sync_engine.set_snapshot_consumer(Box::new(consumer)).await;
        }
        SyncMode::Normal => {
            info!("Using normal sync mode - no snapshot components needed");
        }
        SyncMode::SnapshotServer(_config) => {
            info!("Snapshot server mode not yet implemented");
        }
    }
    Ok(())
}