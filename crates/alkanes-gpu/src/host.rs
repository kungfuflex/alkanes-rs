//! Host-side GPU integration for metashrew runtime.
//!
//! This module provides the interface between the metashrew host runtime
//! (rockshrew-mono) and the GPU pipeline. It handles:
//!
//! 1. Block pre-analysis: scan for GPU-eligible alkanes messages
//! 2. Contract bytecode loading from the key-value store
//! 3. Storage context preloading for each shard
//! 4. GPU dispatch and result collection
//! 5. Writing GPU results back to the key-value store
//!
//! Architecture:
//! ```text
//!   rockshrew-mono
//!     ↓
//!   GpuBlockProcessor::process(block, height, db)
//!     ├── parse protostones → StorageTrackers
//!     ├── dependency analysis → parallel groups
//!     ├── for each group:
//!     │   ├── load contract bytecode from db
//!     │   ├── parse WASM → function table
//!     │   ├── preload storage context from db
//!     │   ├── build GPU input buffer
//!     │   ├── dispatch to GPU
//!     │   └── write results back to db
//!     └── return list of completed message indices
//!   rockshrew-mono
//!     ↓
//!   run WASM indexer, skipping GPU-completed messages
//! ```

use crate::pipeline;
use crate::tracking::{DependencyAnalyzer, StorageTracker};
use crate::wasm_parser::{self, WasmModuleInfo};
use crate::AlkanesGpu;
use anyhow::Result;
use std::collections::BTreeMap;

/// Trait for accessing the host key-value store.
/// Implemented by rockshrew-mono's storage adapter.
pub trait KeyValueStore {
    /// Get a value by key. Returns None if not found.
    fn get(&self, key: &[u8]) -> Option<Vec<u8>>;
    /// Set a key-value pair.
    fn set(&mut self, key: &[u8], value: &[u8]);
    /// Get all keys under a prefix.
    fn keys_with_prefix(&self, prefix: &[u8]) -> Vec<Vec<u8>>;
}

/// A message that was completed by GPU execution.
#[derive(Debug, Clone)]
pub struct GpuCompletedMessage {
    pub tx_index: usize,
    pub msg_index: usize,
    /// K/V writes produced by this message's GPU execution
    pub kv_writes: Vec<(Vec<u8>, Vec<u8>)>,
}

/// Result of GPU block processing.
#[derive(Debug)]
pub struct GpuBlockProcessResult {
    /// Messages successfully executed on GPU
    pub completed: Vec<GpuCompletedMessage>,
    /// Message indices that need sequential WASM execution
    pub remaining: Vec<(usize, usize)>, // (tx_index, msg_index)
    /// Whether GPU was used at all
    pub gpu_used: bool,
}

/// Process a block's alkanes messages with GPU acceleration.
///
/// This is the main entry point called by the host runtime.
/// It returns which messages were completed on GPU (with their K/V results)
/// and which still need sequential processing.
pub fn process_block_gpu<S: KeyValueStore>(
    gpu: &AlkanesGpu,
    analyzer: &DependencyAnalyzer,
    block_height: u32,
    fuel_per_message: u64,
    store: &S,
) -> Result<GpuBlockProcessResult> {
    let groups = analyzer.compute_parallel_groups();
    let trackers = analyzer.trackers();

    let mut completed: Vec<GpuCompletedMessage> = Vec::new();
    let mut remaining: Vec<(usize, usize)> = Vec::new();
    let mut gpu_used = false;

    for group in &groups {
        if group.len() < 4 {
            // Too small for GPU
            for &idx in group {
                let t = &trackers[idx];
                remaining.push((t.tx_index, t.msg_index));
            }
            continue;
        }

        // All messages in a conflict group target the same contract
        // (or at least share storage — load bytecode for the first one)
        let first = &trackers[group[0]];
        let contract_key = format!(
            "/alkanes/{:032x}{:032x}",
            first.target.0, first.target.1
        );

        // Load contract bytecode from store
        let bytecode = match store.get(contract_key.as_bytes()) {
            Some(bc) => bc,
            None => {
                // Contract not found — all messages eject to sequential
                for &idx in group {
                    let t = &trackers[idx];
                    remaining.push((t.tx_index, t.msg_index));
                }
                continue;
            }
        };

        // Parse WASM module
        let module_info = match wasm_parser::parse_wasm_module(&bytecode) {
            Ok(info) => info,
            Err(_) => {
                for &idx in group {
                    let t = &trackers[idx];
                    remaining.push((t.tx_index, t.msg_index));
                }
                continue;
            }
        };

        // Preload storage context: all keys under the contract's storage prefix
        let storage_prefix = format!("{}/storage/", contract_key);
        let storage_keys = store.keys_with_prefix(storage_prefix.as_bytes());
        let mut kv_pairs: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();
        for key in &storage_keys {
            if let Some(value) = store.get(key) {
                kv_pairs.push((key.clone(), value));
            }
        }

        // Build function table for GPU
        let func_table: Vec<(u32, u32)> = module_info
            .functions
            .iter()
            .map(|f| (f.code_offset, f.local_count))
            .collect();

        // Build GPU input buffer
        let kv_refs: Vec<(&[u8], &[u8])> = kv_pairs
            .iter()
            .map(|(k, v)| (k.as_slice(), v.as_slice()))
            .collect();

        let shard_input = pipeline::build_shard_input(
            group.len() as u32,
            block_height,
            fuel_per_message,
            &module_info.code_bytes,
            module_info.import_count,
            module_info.entry_pc,
            &func_table,
            &kv_refs,
        );

        // Dispatch to GPU
        match gpu.execute_shard_raw(&shard_input, group.len()) {
            Ok(results) => {
                gpu_used = true;
                for (i, r) in results.iter().enumerate() {
                    let t = &trackers[group[i]];
                    if r.ejected != 0 {
                        remaining.push((t.tx_index, t.msg_index));
                    } else {
                        completed.push(GpuCompletedMessage {
                            tx_index: t.tx_index,
                            msg_index: t.msg_index,
                            kv_writes: Vec::new(), // TODO: extract from GPU output
                        });
                    }
                }
            }
            Err(_) => {
                for &idx in group {
                    let t = &trackers[idx];
                    remaining.push((t.tx_index, t.msg_index));
                }
            }
        }
    }

    // Non-eligible messages always go to sequential
    for t in trackers {
        if !t.gpu_eligible {
            remaining.push((t.tx_index, t.msg_index));
        }
    }

    Ok(GpuBlockProcessResult {
        completed,
        remaining,
        gpu_used,
    })
}

/// Apply GPU-completed message results back to the key-value store.
pub fn apply_gpu_results<S: KeyValueStore>(
    store: &mut S,
    results: &[GpuCompletedMessage],
) {
    for msg in results {
        for (key, value) in &msg.kv_writes {
            store.set(key, value);
        }
    }
}
