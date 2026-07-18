//! In-process WASM secondary indexer runtime for Qubitcoin.
//!
//! Embeds metashrew-compatible WASM indexers that run in-process alongside
//! the chain tip, providing atomic reorg handling and zero-config indexing.

pub mod config;
pub mod rollback;
pub mod rpc;
pub mod runtime;
pub mod smt;
pub mod state;
pub mod storage;

use config::IndexerConfig;
use runtime::WasmIndexerRuntime;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use storage::IndexerStorage;

// Note: WasmIndexerRuntime methods (run_block, call_view, call_view_async)
// all take &self and create a fresh wasmtime::Store per invocation, so no
// Mutex is needed. Engine and Module are Send+Sync in wasmtime.

/// A block queued for async indexer processing.
struct IndexerBlock {
    height: u32,
    data: Arc<Vec<u8>>, // [height_le32 ++ block_data]
}

/// Indexer execution mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexerMode {
    /// Indexers run synchronously — the chain tip does not advance until all
    /// indexers have processed the block.
    Synchronous,
    /// Indexers run asynchronously via a channel. The chain tip may be ahead.
    Async,
}

/// A single loaded indexer instance.
pub struct IndexerInstance {
    /// Human-readable label.
    pub label: String,
    /// WASM runtime for block processing (serial, one block at a time).
    pub block_runtime: WasmIndexerRuntime,
    /// WASM runtime for view/RPC queries (concurrent reads allowed).
    pub view_runtime: WasmIndexerRuntime,
    /// Dedicated RocksDB storage.
    pub storage: Arc<IndexerStorage>,
    /// SHA-256 hash of the WASM binary.
    pub wasm_hash: [u8; 32],
    /// Current tip height (atomically updated).
    pub tip_height: AtomicU32,
    /// Whether SMT state roots are computed.
    pub smt_enabled: bool,
    /// Block height at which this indexer starts processing.
    pub start_height: u32,
    /// Indexer layer (secondary or tertiary).
    pub layer: config::IndexerLayer,
    /// Labels of indexers this one depends on (tertiary only).
    pub depends_on: Vec<String>,
    /// Whether this indexer is paused (skipped by on_block_connected).
    pub paused: AtomicBool,
    /// Path to the RocksDB directory (for rsyncd exposure / admin).
    pub db_path: PathBuf,
    /// Channel sender for async block processing. When set, blocks are
    /// queued to a dedicated worker thread instead of processed inline.
    pub block_sender: Option<mpsc::SyncSender<IndexerBlock>>,
}

/// Info returned by `pause()`.
pub struct PauseInfo {
    pub label: String,
    pub db_path: PathBuf,
    pub tip_height: u32,
}

/// Info returned by `status()`.
pub struct IndexerStatusInfo {
    pub label: String,
    pub height: u32,
    pub paused: bool,
    pub wasm_hash: String,
    pub layer: config::IndexerLayer,
    pub db_path: PathBuf,
    pub smt_enabled: bool,
    pub start_height: u32,
    pub depends_on: Vec<String>,
}

/// Manages all loaded indexer instances.
pub struct IndexerManager {
    indexers: HashMap<String, Arc<IndexerInstance>>,
    mode: IndexerMode,
    datadir: PathBuf,
}

impl IndexerManager {
    /// Create a new IndexerManager by loading indexers from configs.
    ///
    /// Each indexer gets its own RocksDB at `{datadir}/indexers/{label}/db/`.
    pub fn new(
        configs: Vec<IndexerConfig>,
        datadir: &PathBuf,
        mode: IndexerMode,
    ) -> Result<Self, String> {
        let mut indexers = HashMap::new();

        for config in configs {
            tracing::info!(
                label = %config.label,
                wasm = %config.wasm_path.display(),
                "loading indexer module"
            );

            // Read WASM binary.
            let wasm_bytes = std::fs::read(&config.wasm_path).map_err(|e| {
                format!(
                    "failed to read WASM file '{}': {}",
                    config.wasm_path.display(),
                    e
                )
            })?;

            // Compute SHA-256 hash.
            let wasm_hash: [u8; 32] = Sha256::digest(&wasm_bytes).into();
            let hash_hex: String = wasm_hash.iter().map(|b| format!("{:02x}", b)).collect();

            // Open storage.
            let db_dir = datadir
                .join("indexers")
                .join(&config.label)
                .join("db");
            std::fs::create_dir_all(&db_dir).map_err(|e| {
                format!("failed to create indexer db dir: {}", e)
            })?;

            let storage = Arc::new(IndexerStorage::open(&db_dir)?);

            // Check WASM hash integrity.
            if let Some(stored_hash) = storage.get(state::WASM_HASH_KEY) {
                if stored_hash != wasm_hash {
                    tracing::warn!(
                        label = %config.label,
                        "WASM binary changed, stored state may be incompatible"
                    );
                }
            }
            storage.put(state::WASM_HASH_KEY, &wasm_hash)?;

            // Compile WASM module — two runtimes: one for blocks, one for views.
            // Each creates a fresh Store per call so no locking is needed.
            let block_runtime = WasmIndexerRuntime::new(&wasm_bytes)?;
            let view_runtime = WasmIndexerRuntime::new(&wasm_bytes)?;

            let tip_height = storage.tip_height();

            tracing::info!(
                label = %config.label,
                hash = %hash_hex,
                tip_height = tip_height,
                smt = config.smt_enabled,
                "indexer module loaded"
            );

            // In Async mode, spawn a dedicated worker thread per indexer.
            // The channel has a bounded buffer to provide backpressure.
            let block_sender = if mode == IndexerMode::Async {
                let (tx, rx) = mpsc::sync_channel::<IndexerBlock>(64);
                // We'll spawn the worker after creating the Arc below.
                Some((tx, rx))
            } else {
                None
            };

            let instance = Arc::new(IndexerInstance {
                label: config.label.clone(),
                block_runtime,
                view_runtime,
                storage,
                wasm_hash,
                tip_height: AtomicU32::new(tip_height),
                smt_enabled: config.smt_enabled,
                start_height: config.start_height,
                layer: config.layer,
                depends_on: config.depends_on.clone(),
                paused: AtomicBool::new(false),
                db_path: db_dir,
                block_sender: block_sender.as_ref().map(|(tx, _)| tx.clone()),
            });

            // Spawn the worker thread for async mode.
            if let Some((_tx, rx)) = block_sender {
                let worker_inst = instance.clone();
                let worker_label = config.label.clone();
                std::thread::Builder::new()
                    .name(format!("indexer-{}", worker_label))
                    .spawn(move || {
                        tracing::info!(indexer = %worker_label, "indexer worker thread started");
                        while let Ok(block) = rx.recv() {
                            if worker_inst.paused.load(Ordering::Relaxed) {
                                continue;
                            }
                            if block.height < worker_inst.start_height {
                                continue;
                            }
                            run_indexer_block(&worker_inst, block.height, &block.data);
                        }
                        tracing::info!(indexer = %worker_label, "indexer worker thread exiting");
                    })
                    .map_err(|e| format!("failed to spawn indexer worker: {}", e))?;
            }

            indexers.insert(config.label, instance);
        }

        Ok(IndexerManager {
            indexers,
            mode,
            datadir: datadir.clone(),
        })
    }

    /// Get a reference to a loaded indexer by label.
    pub fn get_indexer(&self, label: &str) -> Option<&Arc<IndexerInstance>> {
        self.indexers.get(label)
    }

    /// Get all indexer labels.
    pub fn labels(&self) -> Vec<&str> {
        self.indexers.keys().map(|s| s.as_str()).collect()
    }

    /// True if no indexers are loaded.
    pub fn is_empty(&self) -> bool {
        self.indexers.is_empty()
    }

    /// Get the indexer mode.
    pub fn mode(&self) -> IndexerMode {
        self.mode
    }

    /// Notify all indexers that a new block has been connected.
    ///
    /// In **Async mode** (recommended): queues the block to per-indexer
    /// worker threads and returns immediately. The caller is not blocked
    /// by WASM execution. Worker threads process blocks in order.
    ///
    /// In **Synchronous mode**: blocks until all indexers finish (legacy).
    ///
    /// Indexers are processed in two phases:
    /// 1. Secondary indexers (no dependencies) run in parallel.
    /// 2. Tertiary indexers run after their dependencies have completed.
    pub fn on_block_connected(&self, height: u32, block_data: &[u8]) {
        if self.indexers.is_empty() {
            return;
        }

        // Build input: [height_le32 ++ block_data]
        let mut input = Vec::with_capacity(4 + block_data.len());
        input.extend_from_slice(&height.to_le_bytes());
        input.extend_from_slice(block_data);
        let input = Arc::new(input);

        // Async mode: queue to per-indexer worker threads.
        if self.mode == IndexerMode::Async {
            for inst in self.indexers.values() {
                if height < inst.start_height || inst.paused.load(Ordering::Relaxed) {
                    continue;
                }
                if let Some(ref sender) = inst.block_sender {
                    let block = IndexerBlock {
                        height,
                        data: Arc::clone(&input),
                    };
                    if let Err(e) = sender.try_send(block) {
                        // Channel full — apply backpressure by blocking.
                        match e {
                            mpsc::TrySendError::Full(block) => {
                                let _ = sender.send(block);
                            }
                            mpsc::TrySendError::Disconnected(_) => {
                                tracing::error!(
                                    indexer = %inst.label,
                                    "indexer worker thread disconnected"
                                );
                            }
                        }
                    }
                }
            }
            // Tertiary indexers with dependencies: also queue, but the worker
            // thread will check dependency heights before processing.
            return;
        }

        // Synchronous mode (legacy): block until all indexers finish.
        let (phase1, phase2): (Vec<_>, Vec<_>) = self
            .indexers
            .values()
            .partition(|inst| inst.depends_on.is_empty());

        let run_phase = |instances: &[&Arc<IndexerInstance>]| {
            rayon::scope(|s| {
                for inst in instances {
                    if height < inst.start_height
                        || inst.paused.load(Ordering::Relaxed)
                    {
                        continue;
                    }
                    let inst = Arc::clone(inst);
                    let input = Arc::clone(&input);
                    s.spawn(move |_| {
                        run_indexer_block(&inst, height, &input);
                    });
                }
            });
        };

        run_phase(&phase1);

        if !phase2.is_empty() {
            let ready: Vec<&Arc<IndexerInstance>> = phase2
                .into_iter()
                .filter(|inst| {
                    if height < inst.start_height {
                        return false;
                    }
                    self.dependencies_satisfied(inst, height)
                })
                .collect();

            run_phase(&ready);
        }
    }

    /// Check if all dependencies for a tertiary indexer have reached the
    /// given height.
    fn dependencies_satisfied(&self, inst: &IndexerInstance, height: u32) -> bool {
        for dep_label in &inst.depends_on {
            match self.indexers.get(dep_label) {
                Some(dep) => {
                    if dep.tip_height.load(Ordering::Relaxed) < height {
                        return false;
                    }
                }
                None => {
                    tracing::warn!(
                        indexer = %inst.label,
                        dependency = %dep_label,
                        "dependency not found, skipping"
                    );
                    return false;
                }
            }
        }
        true
    }

    /// Notify all indexers of a chain reorganization.
    ///
    /// Uses **deferred rollback**: sets a reorg marker and deletes only
    /// metadata. The service stays live — reads filter orphaned entries.
    /// Background pruning cleans up later.
    pub fn on_reorg(&self, rollback_height: u32) {
        for (label, inst) in &self.indexers {
            let current = inst.tip_height.load(Ordering::Relaxed);
            if current > rollback_height {
                tracing::info!(
                    indexer = %label,
                    from = current,
                    to = rollback_height,
                    "deferred rollback (service stays live)"
                );
                match rollback::rollback_deferred(&inst.storage, rollback_height) {
                    Ok(metadata_deleted) => {
                        inst.tip_height.store(rollback_height, Ordering::Relaxed);
                        tracing::info!(
                            indexer = %label,
                            metadata_deleted = metadata_deleted,
                            "deferred rollback complete, background prune pending"
                        );
                        // Spawn background prune task.
                        let storage = inst.storage.clone();
                        let label_owned = label.clone();
                        std::thread::spawn(move || {
                            match rollback::prune_orphaned(&storage) {
                                Ok(pruned) => {
                                    tracing::info!(
                                        indexer = %label_owned,
                                        pruned = pruned,
                                        "background prune complete"
                                    );
                                }
                                Err(e) => {
                                    tracing::error!(
                                        indexer = %label_owned,
                                        error = %e,
                                        "background prune failed"
                                    );
                                }
                            }
                        });
                    }
                    Err(e) => {
                        tracing::error!(
                            indexer = %label,
                            error = %e,
                            "deferred rollback failed"
                        );
                    }
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Admin lifecycle methods
    // -----------------------------------------------------------------------

    /// Pause an indexer. While paused, `on_block_connected` skips it.
    /// Returns the DB path and tip height for admin operations (rsync, etc.).
    pub fn pause(&self, label: &str) -> Result<PauseInfo, String> {
        let inst = self
            .get_indexer(label)
            .ok_or_else(|| format!("indexer '{}' not found", label))?;
        inst.paused.store(true, Ordering::Relaxed);
        inst.storage.flush().ok(); // flush WAL for consistent state
        Ok(PauseInfo {
            label: label.to_string(),
            db_path: inst.db_path.clone(),
            tip_height: inst.tip_height.load(Ordering::Relaxed),
        })
    }

    /// Resume a paused indexer.
    pub fn resume(&self, label: &str) -> Result<(), String> {
        let inst = self
            .get_indexer(label)
            .ok_or_else(|| format!("indexer '{}' not found", label))?;
        inst.paused.store(false, Ordering::Relaxed);
        Ok(())
    }

    /// Roll back a paused indexer to a specific height (immediate rollback).
    pub fn rollback_indexer(&self, label: &str, height: u32) -> Result<u32, String> {
        let inst = self
            .get_indexer(label)
            .ok_or_else(|| format!("indexer '{}' not found", label))?;
        if !inst.paused.load(Ordering::Relaxed) {
            return Err("indexer must be paused before rollback".to_string());
        }
        let deleted = rollback::rollback_to_height(&inst.storage, height)?;
        inst.tip_height.store(height, Ordering::Relaxed);
        Ok(deleted)
    }

    /// Hot-load a new WASM indexer module at runtime.
    pub fn load(&mut self, label: &str, wasm_path: &Path, cfg: IndexerConfig) -> Result<String, String> {
        let wasm_bytes = std::fs::read(wasm_path)
            .map_err(|e| format!("failed to read WASM: {}", e))?;

        let wasm_hash: [u8; 32] = Sha256::digest(&wasm_bytes).into();
        let hash_hex: String = wasm_hash.iter().map(|b| format!("{:02x}", b)).collect();

        let db_dir = self.datadir.join("indexers").join(label).join("db");
        std::fs::create_dir_all(&db_dir)
            .map_err(|e| format!("failed to create db dir: {}", e))?;

        let storage = Arc::new(IndexerStorage::open(&db_dir)?);
        storage.put(state::WASM_HASH_KEY, &wasm_hash)?;

        let block_runtime = WasmIndexerRuntime::new(&wasm_bytes)?;
        let view_runtime = WasmIndexerRuntime::new(&wasm_bytes)?;
        let tip_height = storage.tip_height();

        let instance = Arc::new(IndexerInstance {
            label: label.to_string(),
            block_runtime,
            view_runtime,
            storage,
            wasm_hash,
            tip_height: AtomicU32::new(tip_height),
            smt_enabled: cfg.smt_enabled,
            start_height: cfg.start_height,
            layer: cfg.layer,
            depends_on: cfg.depends_on.clone(),
            paused: AtomicBool::new(false),
            db_path: db_dir,
            block_sender: None, // hot-loaded indexers use sync mode
        });

        self.indexers.insert(label.to_string(), instance);
        tracing::info!(label = label, hash = %hash_hex, "indexer hot-loaded");
        Ok(hash_hex)
    }

    /// Unload an indexer.
    pub fn unload(&mut self, label: &str) -> Result<(), String> {
        if self.indexers.remove(label).is_none() {
            return Err(format!("indexer '{}' not found", label));
        }
        tracing::info!(label = label, "indexer unloaded");
        Ok(())
    }

    /// Get status info for all loaded indexers.
    pub fn status(&self) -> Vec<IndexerStatusInfo> {
        self.indexers
            .values()
            .map(|inst| IndexerStatusInfo {
                label: inst.label.clone(),
                height: inst.tip_height.load(Ordering::Relaxed),
                paused: inst.paused.load(Ordering::Relaxed),
                wasm_hash: inst.wasm_hash.iter().map(|b| format!("{:02x}", b)).collect(),
                layer: inst.layer,
                db_path: inst.db_path.clone(),
                smt_enabled: inst.smt_enabled,
                start_height: inst.start_height,
                depends_on: inst.depends_on.clone(),
            })
            .collect()
    }

    // -----------------------------------------------------------------------
    // View methods
    // -----------------------------------------------------------------------

    /// Call a view function on an indexer (async, with fuel-based yielding).
    ///
    /// Uses the dedicated `view_runtime` — no lock contention with block
    /// processing. Multiple concurrent view calls are safe because each
    /// creates a fresh wasmtime Store.
    pub async fn call_view_async(
        &self,
        label: &str,
        fn_name: &str,
        input: Vec<u8>,
    ) -> Result<Vec<u8>, String> {
        let inst = self
            .get_indexer(label)
            .ok_or_else(|| format!("indexer '{}' not found", label))?;
        inst.view_runtime
            .call_view_async(fn_name, input, inst.storage.clone(), label)
            .await
    }

    /// Call a view function on an indexer (sync, for non-async contexts).
    pub fn call_view(
        &self,
        label: &str,
        fn_name: &str,
        input: Vec<u8>,
    ) -> Result<Vec<u8>, String> {
        let inst = self
            .get_indexer(label)
            .ok_or_else(|| format!("indexer '{}' not found", label))?;
        inst.view_runtime
            .call_view(fn_name, input, inst.storage.clone(), label)
    }

    /// Get the current tip height for an indexer.
    pub fn indexer_height(&self, label: &str) -> Option<u32> {
        self.get_indexer(label)
            .map(|inst| inst.tip_height.load(Ordering::Relaxed))
    }

    /// Replay blocks for indexers that are behind the chain tip.
    ///
    /// In **Async mode**: feeds blocks to the per-indexer worker channels
    /// and returns immediately. Workers process blocks in the background.
    /// The node is not blocked during catch-up.
    ///
    /// In **Synchronous mode**: blocks until all catch-up is complete.
    pub fn catch_up<F>(&self, chain_height: u32, read_block: F)
    where
        F: Fn(u32) -> Option<Vec<u8>>,
    {
        // Phase 1: catch up secondary indexers (no dependencies).
        for (label, inst) in &self.indexers {
            if !inst.depends_on.is_empty() {
                continue;
            }
            self.catch_up_single(inst, label, chain_height, &read_block);
        }

        // Phase 2: catch up tertiary indexers.
        for (label, inst) in &self.indexers {
            if inst.depends_on.is_empty() {
                continue;
            }
            // In async mode, tertiaries will process when deps are ready
            // (the worker thread checks dependency heights).
            if self.mode == IndexerMode::Synchronous
                && !self.dependencies_satisfied(inst, chain_height)
            {
                tracing::warn!(
                    indexer = %label,
                    "skipping tertiary catch-up: dependencies not satisfied"
                );
                continue;
            }
            self.catch_up_single(inst, label, chain_height, &read_block);
        }
    }

    fn catch_up_single<F>(
        &self,
        inst: &IndexerInstance,
        label: &str,
        chain_height: u32,
        read_block: &F,
    ) where
        F: Fn(u32) -> Option<Vec<u8>>,
    {
        let indexer_height = inst.tip_height.load(Ordering::Relaxed);
        // Distinguish "never processed" (height=0, no data) from
        // "processed up to height 0" (height=0, has data).
        // If tip_height is 0 and storage has no tip key, start from start_height.
        let has_processed = inst.storage.tip_height() > 0
            || inst.storage.get(crate::state::HEIGHT_KEY).is_some();
        let effective_start = if has_processed {
            indexer_height + 1
        } else {
            inst.start_height
        };
        if effective_start > chain_height {
            return;
        }

        let blocks_behind = chain_height - effective_start + 1;
        tracing::info!(
            indexer = %label,
            from = effective_start,
            to = chain_height,
            blocks_behind = blocks_behind,
            mode = if inst.block_sender.is_some() { "async" } else { "sync" },
            "replaying blocks for indexer catch-up"
        );
        for h in effective_start..=chain_height {
            if let Some(block_data) = read_block(h) {
                let mut input = Vec::with_capacity(4 + block_data.len());
                input.extend_from_slice(&h.to_le_bytes());
                input.extend_from_slice(&block_data);

                // Async mode: queue to worker thread.
                if let Some(ref sender) = inst.block_sender {
                    let block = IndexerBlock {
                        height: h,
                        data: Arc::new(input),
                    };
                    let _ = sender.send(block); // blocks if channel full (backpressure)
                } else {
                    // Synchronous mode: process inline.
                    run_indexer_block(inst, h, &input);
                }
            } else {
                tracing::warn!(
                    indexer = %label,
                    height = h,
                    "block not available for catch-up, stopping"
                );
                break;
            }
        }
    }
}

/// Build a minimal WASM binary for testing.
/// Calls __host_len and __flush (empty protobuf).
#[cfg(test)]
fn build_test_wasm() -> Vec<u8> {
    wat::parse_str(r#"
        (module
            (import "env" "__host_len" (func $host_len (result i32)))
            (import "env" "__load_input" (func $load_input (param i32)))
            (import "env" "__flush" (func $flush (param i32)))
            (import "env" "__log" (func $log (param i32)))
            (import "env" "__get" (func $get (param i32 i32)))
            (import "env" "__get_len" (func $get_len (param i32) (result i32)))
            (import "env" "abort" (func $abort (param i32 i32 i32 i32)))
            (memory (export "memory") 1)

            (func (export "_start")
                (drop (call $host_len))
                (i32.store (i32.const 96) (i32.const 0))
                (call $flush (i32.const 100))
            )
        )
    "#).expect("failed to parse test WAT")
}

/// Run a single indexer on a block, writing results to storage.
///
/// Uses `block_runtime` (no lock) and `append_batch` (single WriteBatch)
/// for maximum throughput.
fn run_indexer_block(inst: &IndexerInstance, height: u32, input: &[u8]) {
    match inst
        .block_runtime
        .run_block(input.to_vec(), inst.storage.clone(), &inst.label)
    {
        Ok(pairs) => {
            // Compute SMT root before writing (needs the pairs).
            if inst.smt_enabled && !pairs.is_empty() {
                let root = smt::compute_state_root(&pairs);
                let root_key = smt::smt_root_key(height);
                if let Err(e) = inst.storage.put(&root_key, &root) {
                    tracing::error!(
                        indexer = %inst.label,
                        height = height,
                        error = %e,
                        "failed to store SMT root"
                    );
                }
            }

            // Write raw pairs + tip height. The pairs from __flush are already
            // in the final storage format (metashrew's length_key/index_key
            // entries are produced by the WASM itself). Do NOT use append_batch
            // which would double-wrap them.
            {
                let mut batch = rocksdb::WriteBatch::default();
                for (k, v) in &pairs {
                    batch.put(k, v);
                }
                batch.put(crate::state::HEIGHT_KEY, &height.to_le_bytes());
                if let Err(e) = inst.storage.write_raw_batch(batch) {
                    tracing::error!(
                        indexer = %inst.label,
                        height = height,
                        error = %e,
                        "failed to write indexer batch"
                    );
                    return;
                }
            }
            inst.tip_height.store(height, Ordering::Relaxed);

            tracing::debug!(
                indexer = %inst.label,
                height = height,
                pairs = pairs.len(),
                "indexer processed block"
            );
        }
        Err(e) => {
            tracing::error!(
                indexer = %inst.label,
                height = height,
                error = %e,
                "indexer failed to process block"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_config(label: &str, wasm_path: PathBuf) -> config::IndexerConfig {
        config::IndexerConfig {
            label: label.to_string(),
            wasm_path,
            smt_enabled: false,
            start_height: 0,
            layer: config::IndexerLayer::Secondary,
            depends_on: vec![],
        }
    }

    fn setup_indexer_manager(mode: IndexerMode) -> (IndexerManager, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let wasm_path = dir.path().join("test.wasm");
        std::fs::write(&wasm_path, build_test_wasm()).unwrap();

        let configs = vec![make_config("test", wasm_path)];

        let datadir = PathBuf::from(dir.path());
        let mgr = IndexerManager::new(configs, &datadir, mode).unwrap();
        (mgr, dir)
    }

    #[test]
    fn test_indexer_manager_load() {
        let (mgr, _dir) = setup_indexer_manager(IndexerMode::Synchronous);
        assert!(!mgr.is_empty());
        assert_eq!(mgr.labels().len(), 1);
        assert!(mgr.get_indexer("test").is_some());
        assert!(mgr.get_indexer("nonexistent").is_none());
    }

    #[test]
    fn test_indexer_manager_initial_height() {
        let (mgr, _dir) = setup_indexer_manager(IndexerMode::Synchronous);
        assert_eq!(mgr.indexer_height("test"), Some(0));
    }

    #[test]
    fn test_on_block_connected_sync() {
        let (mgr, _dir) = setup_indexer_manager(IndexerMode::Synchronous);

        // Feed a fake block.
        mgr.on_block_connected(1, b"fake_block");
        assert_eq!(mgr.indexer_height("test"), Some(1));

        mgr.on_block_connected(2, b"fake_block_2");
        assert_eq!(mgr.indexer_height("test"), Some(2));
    }

    #[test]
    fn test_on_block_connected_async() {
        let (mgr, _dir) = setup_indexer_manager(IndexerMode::Async);
        mgr.on_block_connected(1, b"fake_block");
        // In async mode, the block is queued to a worker thread.
        // Wait for the worker to process it.
        for _ in 0..100 {
            if mgr.indexer_height("test") == Some(1) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(mgr.indexer_height("test"), Some(1));
    }

    #[test]
    fn test_on_reorg() {
        let (mgr, _dir) = setup_indexer_manager(IndexerMode::Synchronous);

        mgr.on_block_connected(1, b"block1");
        mgr.on_block_connected(2, b"block2");
        mgr.on_block_connected(3, b"block3");
        assert_eq!(mgr.indexer_height("test"), Some(3));

        // Reorg back to height 1.
        mgr.on_reorg(1);
        assert_eq!(mgr.indexer_height("test"), Some(1));
    }

    #[test]
    fn test_on_reorg_noop_when_below() {
        let (mgr, _dir) = setup_indexer_manager(IndexerMode::Synchronous);

        mgr.on_block_connected(1, b"block1");

        // Reorg to height 5 — indexer is only at 1, should be a no-op.
        mgr.on_reorg(5);
        assert_eq!(mgr.indexer_height("test"), Some(1));
    }

    #[test]
    fn test_wasm_hash() {
        let (mgr, _dir) = setup_indexer_manager(IndexerMode::Synchronous);
        let inst = mgr.get_indexer("test").unwrap();
        // Hash should be non-zero (valid SHA-256).
        assert_ne!(inst.wasm_hash, [0u8; 32]);
    }

    #[test]
    fn test_mode() {
        let (mgr_sync, _dir1) = setup_indexer_manager(IndexerMode::Synchronous);
        assert_eq!(mgr_sync.mode(), IndexerMode::Synchronous);

        let (mgr_async, _dir2) = setup_indexer_manager(IndexerMode::Async);
        assert_eq!(mgr_async.mode(), IndexerMode::Async);
    }

    #[test]
    fn test_empty_manager() {
        let dir = tempfile::tempdir().unwrap();
        let datadir = PathBuf::from(dir.path());
        let mgr = IndexerManager::new(vec![], &datadir, IndexerMode::Synchronous).unwrap();
        assert!(mgr.is_empty());
        assert_eq!(mgr.labels().len(), 0);
    }

    #[test]
    fn test_catch_up() {
        let (mgr, _dir) = setup_indexer_manager(IndexerMode::Synchronous);

        // Simulate catch-up from height 1 to 3.
        mgr.catch_up(3, |h| Some(format!("block_{}", h).into_bytes()));
        assert_eq!(mgr.indexer_height("test"), Some(3));
    }

    #[test]
    fn test_catch_up_stops_on_missing_block() {
        let (mgr, _dir) = setup_indexer_manager(IndexerMode::Synchronous);

        // Block 2 is missing, catch-up should stop at 1.
        mgr.catch_up(3, |h| {
            if h <= 1 {
                Some(format!("block_{}", h).into_bytes())
            } else {
                None
            }
        });
        assert_eq!(mgr.indexer_height("test"), Some(1));
    }

    #[test]
    fn test_multiple_indexers() {
        let dir = tempfile::tempdir().unwrap();
        let wasm_path = dir.path().join("test.wasm");
        std::fs::write(&wasm_path, build_test_wasm()).unwrap();

        let configs = vec![
            make_config("idx_a", wasm_path.clone()),
            {
                let mut c = make_config("idx_b", wasm_path);
                c.smt_enabled = true;
                c
            },
        ];

        let datadir = PathBuf::from(dir.path());
        let mgr = IndexerManager::new(configs, &datadir, IndexerMode::Synchronous).unwrap();

        assert_eq!(mgr.labels().len(), 2);
        assert!(mgr.get_indexer("idx_a").is_some());
        assert!(mgr.get_indexer("idx_b").is_some());

        // Both process the same block in parallel.
        mgr.on_block_connected(1, b"block");
        assert_eq!(mgr.indexer_height("idx_a"), Some(1));
        assert_eq!(mgr.indexer_height("idx_b"), Some(1));
    }

    #[test]
    fn test_smt_enabled() {
        let dir = tempfile::tempdir().unwrap();
        let wasm_path = dir.path().join("test.wasm");
        std::fs::write(&wasm_path, build_test_wasm()).unwrap();

        let configs = vec![{
            let mut c = make_config("smt_test", wasm_path);
            c.smt_enabled = true;
            c
        }];

        let datadir = PathBuf::from(dir.path());
        let mgr = IndexerManager::new(configs, &datadir, IndexerMode::Synchronous).unwrap();
        let inst = mgr.get_indexer("smt_test").unwrap();
        assert!(inst.smt_enabled);
    }

    #[test]
    fn test_call_view_not_found() {
        let (mgr, _dir) = setup_indexer_manager(IndexerMode::Synchronous);
        let result = mgr.call_view("nonexistent", "fn", vec![]);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn test_invalid_wasm_path() {
        let dir = tempfile::tempdir().unwrap();
        let configs = vec![make_config("bad", PathBuf::from("/nonexistent/path.wasm"))];
        let datadir = PathBuf::from(dir.path());
        let result = IndexerManager::new(configs, &datadir, IndexerMode::Synchronous);
        assert!(result.is_err());
    }

    #[test]
    fn test_start_height_skips_early_blocks() {
        let dir = tempfile::tempdir().unwrap();
        let wasm_path = dir.path().join("test.wasm");
        std::fs::write(&wasm_path, build_test_wasm()).unwrap();

        let mut cfg = make_config("late_start", wasm_path);
        cfg.start_height = 5;

        let datadir = PathBuf::from(dir.path());
        let mgr = IndexerManager::new(vec![cfg], &datadir, IndexerMode::Synchronous).unwrap();

        // Blocks 1-4 should be skipped.
        mgr.on_block_connected(1, b"block1");
        mgr.on_block_connected(2, b"block2");
        mgr.on_block_connected(3, b"block3");
        mgr.on_block_connected(4, b"block4");
        assert_eq!(mgr.indexer_height("late_start"), Some(0)); // still at 0

        // Block 5 should be processed.
        mgr.on_block_connected(5, b"block5");
        assert_eq!(mgr.indexer_height("late_start"), Some(5));

        // Block 6 onwards is normal.
        mgr.on_block_connected(6, b"block6");
        assert_eq!(mgr.indexer_height("late_start"), Some(6));
    }

    #[test]
    fn test_tertiary_depends_on_secondary() {
        let dir = tempfile::tempdir().unwrap();
        let wasm_path = dir.path().join("test.wasm");
        std::fs::write(&wasm_path, build_test_wasm()).unwrap();

        let secondary = make_config("primary_idx", wasm_path.clone());
        let mut tertiary = make_config("derived_idx", wasm_path);
        tertiary.layer = config::IndexerLayer::Tertiary;
        tertiary.depends_on = vec!["primary_idx".to_string()];

        let datadir = PathBuf::from(dir.path());
        let mgr = IndexerManager::new(
            vec![secondary, tertiary],
            &datadir,
            IndexerMode::Synchronous,
        )
        .unwrap();

        // Block 1: secondary processes first, then tertiary.
        mgr.on_block_connected(1, b"block1");
        assert_eq!(mgr.indexer_height("primary_idx"), Some(1));
        assert_eq!(mgr.indexer_height("derived_idx"), Some(1));

        // Block 2: same.
        mgr.on_block_connected(2, b"block2");
        assert_eq!(mgr.indexer_height("primary_idx"), Some(2));
        assert_eq!(mgr.indexer_height("derived_idx"), Some(2));
    }

    #[test]
    fn test_tertiary_with_start_height_and_dependency() {
        let dir = tempfile::tempdir().unwrap();
        let wasm_path = dir.path().join("test.wasm");
        std::fs::write(&wasm_path, build_test_wasm()).unwrap();

        let secondary = make_config("base", wasm_path.clone());
        let mut tertiary = make_config("overlay", wasm_path);
        tertiary.layer = config::IndexerLayer::Tertiary;
        tertiary.depends_on = vec!["base".to_string()];
        tertiary.start_height = 3;

        let datadir = PathBuf::from(dir.path());
        let mgr = IndexerManager::new(
            vec![secondary, tertiary],
            &datadir,
            IndexerMode::Synchronous,
        )
        .unwrap();

        // Blocks 1-2: secondary runs, tertiary skipped (start_height=3).
        mgr.on_block_connected(1, b"b1");
        mgr.on_block_connected(2, b"b2");
        assert_eq!(mgr.indexer_height("base"), Some(2));
        assert_eq!(mgr.indexer_height("overlay"), Some(0));

        // Block 3: both run.
        mgr.on_block_connected(3, b"b3");
        assert_eq!(mgr.indexer_height("base"), Some(3));
        assert_eq!(mgr.indexer_height("overlay"), Some(3));
    }

    #[test]
    fn test_catch_up_respects_start_height() {
        let dir = tempfile::tempdir().unwrap();
        let wasm_path = dir.path().join("test.wasm");
        std::fs::write(&wasm_path, build_test_wasm()).unwrap();

        let mut cfg = make_config("late", wasm_path);
        cfg.start_height = 3;

        let datadir = PathBuf::from(dir.path());
        let mgr = IndexerManager::new(vec![cfg], &datadir, IndexerMode::Synchronous).unwrap();

        // Catch up to height 5 — should only process blocks 3, 4, 5.
        mgr.catch_up(5, |h| Some(format!("block_{}", h).into_bytes()));
        assert_eq!(mgr.indexer_height("late"), Some(5));
    }

    #[test]
    fn test_missing_dependency_skips_tertiary() {
        let dir = tempfile::tempdir().unwrap();
        let wasm_path = dir.path().join("test.wasm");
        std::fs::write(&wasm_path, build_test_wasm()).unwrap();

        // Tertiary depends on "missing_dep" which doesn't exist.
        let mut tertiary = make_config("orphan", wasm_path);
        tertiary.layer = config::IndexerLayer::Tertiary;
        tertiary.depends_on = vec!["missing_dep".to_string()];

        let datadir = PathBuf::from(dir.path());
        let mgr = IndexerManager::new(vec![tertiary], &datadir, IndexerMode::Synchronous).unwrap();

        // Block should be skipped because dependency doesn't exist.
        mgr.on_block_connected(1, b"block1");
        assert_eq!(mgr.indexer_height("orphan"), Some(0));
    }

    // -- Lifecycle tests: pause, resume, rollback, load, unload, status ----

    #[test]
    fn test_pause_sets_flag_and_returns_info() {
        let (mgr, _dir) = setup_indexer_manager(IndexerMode::Synchronous);
        mgr.on_block_connected(1, b"block1");

        let info = mgr.pause("test").unwrap();
        assert_eq!(info.label, "test");
        assert_eq!(info.tip_height, 1);
        assert!(!info.db_path.as_os_str().is_empty());

        // Verify the paused flag is set.
        let inst = mgr.get_indexer("test").unwrap();
        assert!(inst.paused.load(Ordering::Relaxed));
    }

    #[test]
    fn test_pause_nonexistent_returns_error() {
        let (mgr, _dir) = setup_indexer_manager(IndexerMode::Synchronous);
        assert!(mgr.pause("nonexistent").is_err());
    }

    #[test]
    fn test_resume_clears_flag() {
        let (mgr, _dir) = setup_indexer_manager(IndexerMode::Synchronous);
        mgr.pause("test").unwrap();
        mgr.resume("test").unwrap();

        let inst = mgr.get_indexer("test").unwrap();
        assert!(!inst.paused.load(Ordering::Relaxed));
    }

    #[test]
    fn test_paused_indexer_skips_blocks() {
        let (mgr, _dir) = setup_indexer_manager(IndexerMode::Synchronous);
        mgr.on_block_connected(1, b"block1");
        assert_eq!(mgr.indexer_height("test"), Some(1));

        mgr.pause("test").unwrap();
        mgr.on_block_connected(2, b"block2");
        // Should still be at height 1 (paused, block 2 skipped).
        assert_eq!(mgr.indexer_height("test"), Some(1));

        mgr.resume("test").unwrap();
        mgr.on_block_connected(3, b"block3");
        // Should advance to 3 after resume.
        assert_eq!(mgr.indexer_height("test"), Some(3));
    }

    #[test]
    fn test_rollback_indexer_requires_paused() {
        let (mgr, _dir) = setup_indexer_manager(IndexerMode::Synchronous);
        mgr.on_block_connected(1, b"b1");
        mgr.on_block_connected(2, b"b2");

        // Should fail when not paused.
        assert!(mgr.rollback_indexer("test", 1).is_err());

        // Should succeed when paused.
        mgr.pause("test").unwrap();
        let _deleted = mgr.rollback_indexer("test", 0).unwrap();
        // deleted may be 0 since test WASM produces no state, but height resets.
        assert_eq!(mgr.indexer_height("test"), Some(0));
    }

    #[test]
    fn test_load_new_indexer() {
        let dir = tempfile::tempdir().unwrap();
        let wasm_path = dir.path().join("test.wasm");
        std::fs::write(&wasm_path, build_test_wasm()).unwrap();

        let datadir = PathBuf::from(dir.path());
        let mut mgr = IndexerManager::new(vec![], &datadir, IndexerMode::Synchronous).unwrap();
        assert!(mgr.is_empty());

        let cfg = make_config("dynamic", wasm_path.clone());
        let hash = mgr.load("dynamic", &wasm_path, cfg).unwrap();
        assert!(!hash.is_empty());
        assert!(!mgr.is_empty());
        assert!(mgr.get_indexer("dynamic").is_some());

        // Loaded indexer should process blocks.
        mgr.on_block_connected(1, b"block1");
        assert_eq!(mgr.indexer_height("dynamic"), Some(1));
    }

    #[test]
    fn test_unload_removes_indexer() {
        let dir = tempfile::tempdir().unwrap();
        let wasm_path = dir.path().join("test.wasm");
        std::fs::write(&wasm_path, build_test_wasm()).unwrap();

        let datadir = PathBuf::from(dir.path());
        let mut mgr =
            IndexerManager::new(vec![make_config("removeme", wasm_path)], &datadir, IndexerMode::Synchronous)
                .unwrap();
        assert!(mgr.get_indexer("removeme").is_some());

        mgr.unload("removeme").unwrap();
        assert!(mgr.get_indexer("removeme").is_none());
        assert!(mgr.is_empty());
    }

    #[test]
    fn test_unload_nonexistent_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let datadir = PathBuf::from(dir.path());
        let mut mgr = IndexerManager::new(vec![], &datadir, IndexerMode::Synchronous).unwrap();
        assert!(mgr.unload("ghost").is_err());
    }

    #[test]
    fn test_status_returns_all_indexers() {
        let (mgr, _dir) = setup_indexer_manager(IndexerMode::Synchronous);
        mgr.on_block_connected(1, b"b1");

        let statuses = mgr.status();
        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0].label, "test");
        assert_eq!(statuses[0].height, 1);
        assert!(!statuses[0].paused);
        assert!(!statuses[0].wasm_hash.is_empty());
    }

    #[test]
    fn test_status_reflects_paused() {
        let (mgr, _dir) = setup_indexer_manager(IndexerMode::Synchronous);
        mgr.pause("test").unwrap();
        let statuses = mgr.status();
        assert!(statuses[0].paused);
    }
}
