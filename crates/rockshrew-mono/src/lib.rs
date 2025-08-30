//! # Rockshrew-Mono: Monolithic Bitcoin Indexer
//!
//! ## OVERVIEW
//!
//! `rockshrew-mono` is a monolithic binary that combines a Bitcoin indexer and a view layer
//! into a single, efficient application. It leverages a generic, adapter-based synchronization
//! framework to provide flexibility and testability.
//!
//! ## ARCHITECTURE
//!
//! The application is built on the `metashrew-sync` crate, which provides a generic
//! `SnapshotMetashrewSync` engine. This engine is configured with adapters for:
//!
//! - **Bitcoin Node**: `BitcoinRpcAdapter` for connecting to a Bitcoin Core node.
//! - **Storage**: `RocksDBStorageAdapter` for persistent storage using RocksDB.
//! - **Runtime**: `MetashrewRuntimeAdapter` for executing WASM-based indexers.
//!
//! This modular design allows for easy replacement of components, such as using an
//! in-memory storage adapter for testing.
//!
//! ## CORE FUNCTIONALITY
//!
//! - **Indexing**: Synchronizes with the Bitcoin blockchain, processing blocks through a
//!   WASM indexer and storing the resulting state in RocksDB.
//! - **View Layer**: Exposes a JSON-RPC API for querying the indexed state.
//! - **Snapshotting**: Supports creating and consuming snapshots for fast synchronization.
//! - **Reorg Handling**: Automatically detects and handles blockchain reorganizations.
//!
//! ## USAGE
//!
//! `rockshrew-mono` is configured and run via command-line arguments. See `Args` for a
//! full list of options.

// In-code documentation for `rockshrew-mono` crate.
//
// PURPOSE:
// This crate serves as the main entry point for the `rockshrew-mono` binary. It is
// responsible for parsing command-line arguments, setting up the synchronization
// engine, and running the JSON-RPC server.
//
// PROMPT CONSIDERATIONS:
// - The primary goal is to refactor this crate to be a lightweight, generic
//   implementation of the indexer stack.
// - All duplicated logic should be moved to lower-level crates like `metashrew-sync`.
// - The `run` function should be generic over the adapter traits to support both
//   production (RocksDB) and testing (in-memory) environments.

pub mod adapters;
pub mod smt_helper;
pub mod snapshot;
pub mod snapshot_adapters;
pub mod ssh_tunnel;

#[cfg(test)]
mod tests;

use actix_cors::Cors;
use actix_web::{web, App, HttpResponse, HttpServer, Responder, Result as ActixResult};
use anyhow::Result;
use clap::Parser;
use log::{error, info, warn};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::signal;
use tracing::{debug, instrument};

use crate::adapters::BitcoinRpcAdapter;
use crate::adapters::MetashrewRuntimeAdapter;
use crate::ssh_tunnel::parse_daemon_rpc_url;
use metashrew_runtime::{set_label, MetashrewRuntime, ViewPoolConfig};
use metashrew_sync::{
    BitcoinNodeAdapter, JsonRpcProvider, RuntimeAdapter, SnapshotMetashrewSync, SnapshotProvider,
    SnapshotSyncEngine, StorageAdapter, SyncConfig, SyncMode,
};
use rockshrew_runtime::{
    adapter::{query_height_legacy, RocksDBRuntimeAdapter},
    fork_adapter::{ForkAdapter, LegacyRocksDBRuntimeAdapter},
    query_height, RocksDBStorageAdapter,
};

/// Command-line arguments for `rockshrew-mono`.
#[derive(Parser, Debug, Clone)]
#[command(version, about, long_about = None)]
pub struct Args {
    #[arg(long)]
    pub daemon_rpc_url: String,
    #[arg(long)]
    pub indexer: PathBuf,
    #[arg(long)]
    pub db_path: PathBuf,
    #[arg(long)]
    pub fork: Option<PathBuf>,
    #[arg(long)]
    pub legacy_fork: bool,
    #[arg(long)]
    pub start_block: Option<u32>,
    #[arg(long)]
    pub auth: Option<String>,
    #[arg(long, env = "HOST", default_value = "127.0.0.1")]
    pub host: String,
    #[arg(long, env = "PORT", default_value_t = 8080)]
    pub port: u16,
    #[arg(long)]
    pub label: Option<String>,
    #[arg(long)]
    pub exit_at: Option<u32>,
    #[arg(long)]
    pub pipeline_size: Option<usize>,
    #[arg(long)]
    pub cors: Option<String>,
    #[arg(long)]
    pub snapshot_directory: Option<PathBuf>,
    #[arg(long, default_value_t = 1000)]
    pub snapshot_interval: u32,
    #[arg(long)]
    pub repo: Option<String>,
    #[arg(long, default_value_t = 100)]
    pub max_reorg_depth: u32,
    #[arg(long, default_value_t = 6)]
    pub reorg_check_threshold: u32,
    #[arg(long)]
    pub prefixroot: Vec<String>,
    /// Enable view pool for parallel view execution
    #[arg(long)]
    pub enable_view_pool: bool,
    /// Number of view runtimes in the pool (default: number of CPU cores)
    #[arg(long)]
    pub view_pool_size: Option<usize>,
    /// Maximum concurrent view requests (default: pool_size * 2)
    #[arg(long)]
    pub view_pool_max_concurrent: Option<usize>,
    /// Enable view pool logging for debugging
    #[arg(long)]
    pub view_pool_logging: bool,
    /// Disable LRU cache and refresh memory for each WASM invocation
    #[arg(long)]
    pub disable_lru_cache: bool,
    /// Disable WASM __log host function (silently ignore WASM log calls)
    #[arg(long)]
    pub disable_wasmtime_log: bool,
}

/// Shared application state for the JSON-RPC server.
#[derive(Clone)]
pub struct AppState<N, S, R>
where
    N: BitcoinNodeAdapter + 'static,
    S: StorageAdapter + 'static,
    R: RuntimeAdapter + 'static,
{
    pub sync_engine: Arc<tokio::sync::RwLock<SnapshotMetashrewSync<N, S, R>>>,
}

/// Handles JSON-RPC requests.
#[instrument(skip(body, state))]
async fn handle_jsonrpc<N, S, R>(
    body: web::Json<serde_json::Value>,
    state: web::Data<AppState<N, S, R>>,
) -> ActixResult<impl Responder>
where
    N: BitcoinNodeAdapter + 'static,
    S: StorageAdapter + 'static,
    R: RuntimeAdapter + 'static,
{
    let request: serde_json::Value = body.into_inner();
    let method = request["method"].as_str().unwrap_or_default();
    info!("Received RPC method: '{}'", method);
    let empty_params = vec![];
    let params = request["params"].as_array().unwrap_or(&empty_params);
    let id = request["id"].clone();

    let start_time = Instant::now();
    let result = match method {
        "metashrew_view" => {
            let function_name = params[0].as_str().unwrap_or_default().to_string();
            let input_hex = params[1].as_str().unwrap_or_default().to_string();
            let height = params[2].as_str().unwrap_or("latest").to_string();
            state
                .sync_engine
                .read()
                .await
                .metashrew_view(function_name, input_hex, height)
                .await
        }
        "metashrew_preview" => {
            let block_hex = params[0].as_str().unwrap_or_default().to_string();
            let function_name = params[1].as_str().unwrap_or_default().to_string();
            let input_hex = params[2].as_str().unwrap_or_default().to_string();
            let height = params[3].as_str().unwrap_or("latest").to_string();
            state
                .sync_engine
                .read()
                .await
                .metashrew_preview(block_hex, function_name, input_hex, height)
                .await
        }
        "metashrew_height" => state
            .sync_engine
            .read()
            .await
            .metashrew_height()
            .await
            .map(|h| h.to_string()),
        "metashrew_getblockhash" => {
            let height = params[0].as_u64().unwrap_or_default() as u32;
            state
                .sync_engine
                .read()
                .await
                .metashrew_getblockhash(height)
                .await
        }
        "metashrew_stateroot" => {
            let height = params[0].as_str().unwrap_or("latest").to_string();
            state
                .sync_engine
                .read()
                .await
                .metashrew_stateroot(height)
                .await
        }
        "metashrew_snapshot" => state
            .sync_engine
            .read()
            .await
            .metashrew_snapshot()
            .await
            .map(|v| v.to_string()),
        "metashrew_prefixroot" => {
            let name = params[0].as_str().unwrap_or_default().to_string();
            let height = params[1].as_str().unwrap_or("latest").to_string();
            state
                .sync_engine
                .read()
                .await
                .metashrew_prefixroot(name, height)
                .await
        }
        _ => Err(anyhow::anyhow!("Method not found").into()),
    };

    let duration = start_time.elapsed();

    // Log slow RPC calls
    if duration > std::time::Duration::from_millis(100) {
        warn!("Slow RPC call: {} took {:?}", method, duration);
    } else {
        debug!("RPC call: {} completed in {:?}", method, duration);
    }

    let response = match result {
        Ok(res) => serde_json::json!({
            "jsonrpc": "2.0",
            "result": res,
            "id": id
        }),
        Err(e) => {
            error!("RPC error for method {}: {}", method, e);
            serde_json::json!({
                "jsonrpc": "2.0",
                "error": {
                    "code": -32000,
                    "message": e.to_string()
                },
                "id": id
            })
        }
    };

    Ok(HttpResponse::Ok().json(response))
}

/// Sets up a robust signal handler for graceful and forceful shutdown.
///
/// First Ctrl-C: Initiates graceful shutdown
/// Second Ctrl-C: Forces immediate process termination
async fn setup_signal_handler() -> Arc<AtomicBool> {
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let shutdown_clone = shutdown_requested.clone();
    
    tokio::spawn(async move {
        let mut ctrl_c_count = 0;
        
        loop {
            match signal::ctrl_c().await {
                Ok(()) => {
                    ctrl_c_count += 1;
                    
                    if ctrl_c_count == 1 {
                        info!("First Ctrl-C received, initiating graceful shutdown...");
                        info!("Press Ctrl-C again to force immediate termination");
                        shutdown_clone.store(true, Ordering::SeqCst);
                        
                        // Start a timeout for graceful shutdown
                        let shutdown_timeout = shutdown_clone.clone();
                        tokio::spawn(async move {
                            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                            if shutdown_timeout.load(Ordering::SeqCst) {
                                warn!("Graceful shutdown timeout reached, forcing termination...");
                                std::process::exit(1);
                            }
                        });
                    } else {
                        warn!("Second Ctrl-C received, forcing immediate termination!");
                        warn!("Terminating process forcefully...");
                        std::process::exit(1);
                    }
                }
                Err(e) => {
                    error!("Failed to listen for Ctrl-C signal: {}", e);
                    break;
                }
            }
        }
    });
    
    shutdown_requested
}

/// Main run function, generic over the adapter traits.
pub async fn run<N, S, R>(
    args: Args,
    node_adapter: N,
    storage_adapter: S,
    runtime_adapter: R,
    snapshot_provider: Option<Box<dyn SnapshotProvider>>,
) -> Result<()>
where
    N: BitcoinNodeAdapter + 'static,
    S: StorageAdapter + 'static,
    R: RuntimeAdapter + 'static,
{
    if let Some(ref label) = args.label {
        set_label(label.clone());
    }

    let start_block = if let Some(fork_path) = &args.fork {
        let fork_db_path = fork_path.to_string_lossy().to_string();
        let opts = RocksDBRuntimeAdapter::get_optimized_options();
        let fork_db = rocksdb::DB::open_for_read_only(&opts, fork_db_path, false)?;
        let tip_height = if args.legacy_fork {
            query_height_legacy(Arc::new(fork_db), 0).await?
        } else {
            query_height(Arc::new(fork_db), 0).await?
        };
        info!("Forking from height: {}", tip_height);
        args.start_block.unwrap_or(tip_height)
    } else {
        args.start_block.unwrap_or(0)
    };

    let sync_config = SyncConfig {
        start_block,
        exit_at: args.exit_at,
        pipeline_size: args.pipeline_size,
        max_reorg_depth: args.max_reorg_depth,
        reorg_check_threshold: args.reorg_check_threshold,
    };

    let sync_mode = if args.snapshot_directory.is_some() {
        SyncMode::Snapshot(Default::default())
    } else if args.repo.is_some() {
        SyncMode::Repo(Default::default())
    } else {
        SyncMode::Normal
    };

    let sync_engine = SnapshotMetashrewSync::new(
        node_adapter,
        storage_adapter,
        runtime_adapter,
        sync_config,
        sync_mode,
    );

    if let Some(provider) = snapshot_provider {
        sync_engine.set_snapshot_provider(provider).await;
    }

    let sync_engine_arc = Arc::new(tokio::sync::RwLock::new(sync_engine));
    let app_state = web::Data::new(AppState {
        sync_engine: sync_engine_arc.clone(),
    });

    let (indexer_handle, indexer_abort_handle) = {
        let task = tokio::spawn({
            let sync_engine_clone = sync_engine_arc.clone();
            async move {
                info!("Starting block indexing process...");
                let mut block_count = 0u64;
                let mut total_processing_time = std::time::Duration::ZERO;
                let start_time = Instant::now();

                loop {
                    let block_start = Instant::now();
                    let mut engine = sync_engine_clone.write().await;

                    match engine.process_next_block().await {
                        Ok(Some(height)) => {
                            let block_duration = block_start.elapsed();
                            block_count += 1;
                            total_processing_time += block_duration;

                            // Log performance metrics
                            if block_duration > std::time::Duration::from_millis(500) {
                                warn!(
                                    "Slow block processing at height {}: {:?}",
                                    height, block_duration
                                );
                            }

                            // Log periodic performance summary
                            if block_count % 100 == 0 {
                                let avg_time = total_processing_time / block_count as u32;
                                let elapsed = start_time.elapsed();
                                let blocks_per_sec = block_count as f64 / elapsed.as_secs_f64();
                                info!(
                                    "Performance: {} blocks processed, avg: {:?}/block, rate: {:.2} blocks/sec",
                                    block_count, avg_time, blocks_per_sec
                                );
                            }

                            debug!("Processed block {} in {:?}", height, block_duration);

                            // Log prefix roots
                            let runtime_adapter = engine.runtime.read().await;
                            if let Err(e) = runtime_adapter.log_prefix_roots().await {
                                error!("Failed to log prefix roots: {}", e);
                            }

                            if Some(height) == engine.config.exit_at {
                                break;
                            }
                            // Successfully processed a block, continue immediately
                            continue;
                        }
                        Ok(None) => {
                            // No more blocks to process, wait for new blocks
                            debug!("No new blocks available, waiting...");
                            drop(engine); // Release the lock before sleeping
                            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                        }
                        Err(e) => {
                            // CRITICAL FIX: Don't advance to next block on error!
                            // The process_next_block() method internally manages height,
                            // but on error we need to retry the SAME block, not skip it.
                            let error_duration = block_start.elapsed();
                            error!("Indexer error after {:?}: {}", error_duration, e);
                            drop(engine); // Release the lock before sleeping
                            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                            // Continue the loop to retry the same block
                        }
                    }
                }
            }
        });
        let abort_handle = task.abort_handle();
        (task, abort_handle)
    };

    let (server_handle, server_abort_handle) = {
        let task = tokio::spawn({
            let args_clone = Arc::new(args.clone());
            HttpServer::new(move || {
                let cors = match &args_clone.cors {
                    Some(cors_value) if cors_value == "*" => Cors::default()
                        .allow_any_origin()
                        .allow_any_method()
                        .allow_any_header(),
                    Some(cors_value) => {
                        let mut cors_builder = Cors::default();
                        for origin in cors_value.split(',') {
                            cors_builder = cors_builder.allowed_origin(origin.trim());
                        }
                        cors_builder
                    }
                    None => Cors::default().allowed_origin("http://localhost:8080"),
                };
                App::new()
                    .wrap(cors)
                    .app_data(app_state.clone())
                    .service(web::resource("/").route(web::post().to(handle_jsonrpc::<N, S, R>)))
            })
            .bind((args.host.as_str(), args.port))?
            .run()
        });
        let abort_handle = task.abort_handle();
        (task, abort_handle)
    };

    info!(
        "JSON-RPC server running at http://{}:{}",
        args.host, args.port
    );
    info!("Indexer is ready and processing blocks.");

    let shutdown_signal = setup_signal_handler().await;
    
    // Enhanced shutdown handling with proper task cleanup
    tokio::select! {
        result = indexer_handle => {
            if let Err(e) = result {
                error!("Indexer task failed: {}", e);
            } else {
                info!("Indexer task completed successfully");
            }
        }
        result = server_handle => {
            if let Err(e) = result {
                error!("Server task failed: {}", e);
            } else {
                info!("Server task completed successfully");
            }
        }
        _ = async {
            loop {
                if shutdown_signal.load(Ordering::SeqCst) {
                    info!("Shutdown signal detected, beginning graceful shutdown...");
                    break;
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
        } => {
            info!("Initiating graceful shutdown of all services...");
            
            // Give tasks a moment to finish current operations
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            
            // Abort all tasks
            info!("Stopping indexer task...");
            indexer_abort_handle.abort();
            
            info!("Stopping server task...");
            server_abort_handle.abort();
            
            // Wait a bit for cleanup
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            info!("Graceful shutdown complete.");
        }
    }

    Ok(())
}

// RocksDB configuration has been moved to rockshrew-runtime/src/optimized_config.rs
// for better organization and reusability across the codebase.

use hex::FromHex;

/// Production-specific run function.
async fn run_generic<R: RuntimeAdapter + 'static>(
    args: Args,
    runtime_adapter: R,
    storage_adapter: RocksDBStorageAdapter,
) -> Result<()> {
    let (rpc_url, bypass_ssl, tunnel_config) =
        parse_daemon_rpc_url(&args.daemon_rpc_url).await?;
    let node_adapter = BitcoinRpcAdapter::new(rpc_url, args.auth.clone(), bypass_ssl, tunnel_config);
    run(args, node_adapter, storage_adapter, runtime_adapter, None).await
}

pub async fn run_prod(args: Args) -> Result<()> {
    info!("Initializing RocksDB with performance-optimized configuration");
    info!("Database path: {}", args.db_path.display());
    info!("Optimizations: bloom filter tuning, cache optimization, reduced I/O overhead");

    let prefix_configs = args
        .prefixroot
        .iter()
        .flat_map(|s| s.split(','))
        .map(|s| {
            let parts: Vec<&str> = s.split(':').collect();
            if parts.len() != 2 {
                return Err(anyhow::anyhow!("Invalid prefixroot format: {}", s));
            }
            let name = parts[0].to_string();
            let prefix_hex = parts[1].strip_prefix("0x").unwrap_or(parts[1]);
            let prefix = Vec::from_hex(prefix_hex)?;
            Ok((name, prefix))
        })
        .collect::<Result<Vec<(String, Vec<u8>)>>>()?;
    if let Some(fork_path) = args.fork.clone() {
        info!("Fork mode enabled, forking from: {}", fork_path.display());
        let db_path = args.db_path.to_string_lossy().to_string();
        let fork_path_str = fork_path.to_string_lossy().to_string();
        let opts = RocksDBRuntimeAdapter::get_optimized_options();
        let adapter = if args.legacy_fork {
            info!("Using legacy fork adapter.");
            let primary_db = rocksdb::DB::open(&opts, db_path)?;
            let fork_db = rocksdb::DB::open_for_read_only(&opts, fork_path_str, false)?;
            let legacy_adapter = LegacyRocksDBRuntimeAdapter {
                db: Arc::new(primary_db),
                fork_db: Some(Arc::new(fork_db)),
                height: 0,
                kv_tracker: Arc::new(std::sync::Mutex::new(None)),
            };
            ForkAdapter::Legacy(legacy_adapter)
        } else {
            let modern_adapter = RocksDBRuntimeAdapter::open_fork(db_path, fork_path_str, opts)?;
            ForkAdapter::Modern(modern_adapter)
        };
        let runtime = MetashrewRuntime::load(args.indexer.clone(), adapter, prefix_configs)?;
        let storage_adapter = match runtime.context.lock().unwrap().db {
            ForkAdapter::Modern(ref modern_adapter) => {
                RocksDBStorageAdapter::new(modern_adapter.db.clone())
            }
            ForkAdapter::Legacy(ref legacy_adapter) => {
                RocksDBStorageAdapter::new(legacy_adapter.db.clone())
            }
        };
        let runtime_adapter =
            MetashrewRuntimeAdapter::new(Arc::new(tokio::sync::RwLock::new(runtime)));
        run_generic(args, runtime_adapter, storage_adapter).await
    } else {
        let adapter =
            RocksDBRuntimeAdapter::open_optimized(args.db_path.to_string_lossy().to_string())?;
        let mut runtime = MetashrewRuntime::load(args.indexer.clone(), adapter.clone(), prefix_configs)?;
        if args.disable_wasmtime_log {
            runtime.set_disable_wasmtime_log(true);
        }
        let storage_adapter = RocksDBStorageAdapter::new(adapter.db.clone());
        let runtime_adapter =
            MetashrewRuntimeAdapter::new(Arc::new(tokio::sync::RwLock::new(runtime)));
        if args.disable_lru_cache {
            runtime_adapter.set_disable_lru_cache(true);
        }
        if args.enable_view_pool && !args.disable_lru_cache {
            let pool_size = args.view_pool_size.unwrap_or_else(num_cpus::get);
            let max_concurrent = args.view_pool_max_concurrent.unwrap_or(pool_size * 2);
            let view_pool_config = ViewPoolConfig {
                pool_size,
                max_concurrent_requests: Some(max_concurrent),
                enable_logging: args.view_pool_logging,
            };
            info!(
                "Initializing view pool with {} runtimes, max {} concurrent requests",
                pool_size, max_concurrent
            );
            if let Err(e) = runtime_adapter.initialize_view_pool(view_pool_config).await {
                error!("Failed to initialize view pool: {}", e);
                return Err(e);
            }
            info!("View pool initialized successfully - using stateful view runtimes for parallel execution");
        } else {
            runtime_adapter.disable_stateful_views().await;
            if args.disable_lru_cache {
                info!("LRU cache disabled - view pool disabled, will refresh memory for each WASM invocation");
            } else {
                info!("View pool disabled - using non-stateful async wasmtime for view execution");
            }
        }
        run_generic(args, runtime_adapter, storage_adapter).await
    }
}
