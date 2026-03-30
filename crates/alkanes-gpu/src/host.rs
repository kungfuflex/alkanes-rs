//! Host-side GPU integration for metashrew runtime.
//!
//! Implements iterative preloading: when the GPU shader encounters a
//! HOST_CALL (extcall) to an unknown contract, it ejects with the
//! target AlkaneId in return_data. The host retries with that contract's
//! bytecode preloaded. This repeats until all call targets are satisfied
//! or a depth/count limit is reached.

use crate::pipeline;
use crate::tracking::{DependencyAnalyzer, StorageTracker};
use crate::wasm_parser;
use crate::types::*;
use crate::AlkanesGpu;
use anyhow::Result;
use flate2::read::GzDecoder;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;

/// Maximum number of preload retries per message group.
const MAX_PRELOAD_RETRIES: usize = 8;
/// Maximum number of unique contracts to preload for a single group.
const MAX_PRELOADED_CONTRACTS: usize = 16;

/// Trait for accessing the host key-value store.
pub trait KeyValueStore {
    fn get(&self, key: &[u8]) -> Option<Vec<u8>>;
    fn set(&mut self, key: &[u8], value: &[u8]);
    fn keys_with_prefix(&self, prefix: &[u8]) -> Vec<Vec<u8>>;
}

#[derive(Debug, Clone)]
pub struct GpuCompletedMessage {
    pub tx_index: usize,
    pub msg_index: usize,
    pub kv_writes: Vec<(Vec<u8>, Vec<u8>)>,
}

#[derive(Debug)]
pub struct GpuBlockProcessResult {
    pub completed: Vec<GpuCompletedMessage>,
    pub remaining: Vec<(usize, usize)>,
    pub gpu_used: bool,
}


/// Load cached SPIR-V from the index, or compile from WASM and cache it.
/// The SPIR-V is stored at /alkanes/{id}/spirv in the same DB.
fn load_or_compile_spirv<S: KeyValueStore>(
    store: &S,
    block: u128,
    tx: u128,
    wasm_bytes: &[u8],
) -> Option<Vec<u8>> {
    // Check for cached SPIR-V
    let mut spirv_key = Vec::with_capacity(9 + 32 + 6);
    spirv_key.extend_from_slice(b"/alkanes/");
    spirv_key.extend_from_slice(&block.to_le_bytes());
    spirv_key.extend_from_slice(&tx.to_le_bytes());
    spirv_key.extend_from_slice(b"/spirv");

    if let Some(cached) = store.get(&spirv_key) {
        if !cached.is_empty() {
            log::debug!("GPU: loaded cached SPIR-V for ({},{}) — {} bytes", block, tx, cached.len());
            return Some(cached);
        }
    }

    // Not cached — compile from WASM
    #[cfg(feature = "llvm")]
    {
        log::info!("GPU: compiling SPIR-V for contract ({},{}) — {} bytes WASM", block, tx, wasm_bytes.len());
        match alkanes_llvm::WasmToSpirv::new().compile_and_emit_spirv(wasm_bytes, (block, tx)) {
            Ok(spirv) => {
                log::info!("GPU: compiled {} bytes of SPIR-V for ({},{})", spirv.len(), block, tx);
                // Store in the index for future use
                // Note: store is immutable here, so we can't write back directly.
                // The caller should handle caching.
                return Some(spirv);
            }
            Err(e) => {
                log::warn!("GPU: SPIR-V compilation failed for ({},{}): {}", block, tx, e);
            }
        }
    }

    None
}

/// Build the DB key for an AlkaneId: /alkanes/{block LE u128}{tx LE u128}
fn alkane_id_key(block: u128, tx: u128) -> Vec<u8> {
    let mut key = Vec::with_capacity(9 + 32);
    key.extend_from_slice(b"/alkanes/");
    key.extend_from_slice(&block.to_le_bytes());
    key.extend_from_slice(&tx.to_le_bytes());
    key
}

/// Load and decompress a contract's WASM bytecode from the store.
fn load_contract_wasm<S: KeyValueStore>(store: &S, block: u128, tx: u128) -> Option<Vec<u8>> {
    let key = alkane_id_key(block, tx);
    let compressed = store.get(&key)?;
    // Decompress gzip if needed
    if compressed.len() >= 2 && compressed[0] == 0x1f && compressed[1] == 0x8b {
        let mut decoder = GzDecoder::new(&compressed[..]);
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed).ok()?;
        Some(decompressed)
    } else {
        Some(compressed)
    }
}

/// Extract a target AlkaneId from a GPU result's return_data.
/// Returns (block, tx) if return_data contains a valid 32-byte AlkaneId.
fn extract_extcall_target(result: &GpuMessageResult) -> Option<(u128, u128)> {
    if result.ejection_reason != EJECTION_EXTCALL || result.return_data_len < 32 {
        return None;
    }
    let block = u128::from_le_bytes(result.return_data[0..16].try_into().ok()?);
    let tx = u128::from_le_bytes(result.return_data[16..32].try_into().ok()?);
    Some((block, tx))
}

/// Process a block's alkanes messages with GPU acceleration and iterative preloading.
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
        if group.len() < 1 {
            for &idx in group {
                let t = &trackers[idx];
                remaining.push((t.tx_index, t.msg_index));
            }
            continue;
        }

        let first = &trackers[group[0]];
        let mut contract_key = Vec::with_capacity(9 + 32);
        contract_key.extend_from_slice(b"/alkanes/");
        contract_key.extend_from_slice(&first.target.0.to_le_bytes());
        contract_key.extend_from_slice(&first.target.1.to_le_bytes());

        // Load primary contract bytecode
        let primary_wasm = match load_contract_wasm(store, first.target.0, first.target.1) {
            Some(wasm) => wasm,
            None => {
                log::debug!("GPU: contract ({},{}) not in DB", first.target.0, first.target.1);
                for &idx in group {
                    remaining.push((trackers[idx].tx_index, trackers[idx].msg_index));
                }
                continue;
            }
        };

        let module_info = match wasm_parser::parse_wasm_module(&primary_wasm) {
            Ok(info) => info,
            Err(e) => {
                log::debug!("GPU: WASM parse failed for ({},{}): {}", first.target.0, first.target.1, e);
                for &idx in group {
                    remaining.push((trackers[idx].tx_index, trackers[idx].msg_index));
                }
                continue;
            }
        };

        // Preload storage K/V pairs
        let mut storage_prefix = contract_key.clone();
        storage_prefix.extend_from_slice(b"/storage/");
        let storage_keys = store.keys_with_prefix(&storage_prefix);
        let mut kv_pairs: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();
        for key in &storage_keys {
            if let Some(value) = store.get(key) {
                kv_pairs.push((key.clone(), value));
            }
        }

        let func_table: Vec<(u32, u32)> = module_info.functions.iter()
            .map(|f| (f.code_offset, f.local_count)).collect();
        let import_map = crate::wasm_parser::build_import_map(&module_info.import_names);
        let globals_packed = wasm_parser::pack_globals(&module_info.globals);
        let globals_count = module_info.globals.len() as u32;
        let data_segments_packed = wasm_parser::pack_data_segments(&module_info.data_segments);
        let data_segments_count = module_info.data_segments.len() as u32;
        log::info!("GPU: contract ({},{}) imports={:?} map={:?} globals={} data_segments={}",
            first.target.0, first.target.1, module_info.import_names, import_map,
            globals_count, data_segments_count);

        // === Iterative preload retry loop with multi-bytecode dispatch ===
        let mut preloaded_ids: BTreeSet<(u128, u128)> = BTreeSet::new();
        preloaded_ids.insert((first.target.0, first.target.1));
        let mut additional_contracts: Vec<pipeline::ContractInfo> = Vec::new();
        let mut group_completed = false;

        for retry in 0..MAX_PRELOAD_RETRIES {
            let kv_refs: Vec<(&[u8], &[u8])> = kv_pairs.iter()
                .map(|(k, v)| (k.as_slice(), v.as_slice())).collect();

            let shard_input = pipeline::build_shard_input_multi(
                group.len() as u32,
                block_height,
                fuel_per_message,
                &module_info.code_bytes,
                module_info.import_count,
                module_info.entry_pc,
                &func_table,
                &import_map,
                &globals_packed,
                globals_count,
                &data_segments_packed,
                data_segments_count,
                &additional_contracts,
                &kv_refs,
            );

            match gpu.execute_shard_raw(&shard_input, group.len()) {
                Ok(results) => {
                    gpu_used = true;

                    // Log first ejected result details for debugging
                    if let Some(r) = results.iter().find(|r| r.ejected != 0) {
                        log::info!("GPU: first ejection detail: reason={} return_data_len={} return_data[0..8]={:?}",
                            r.ejection_reason, r.return_data_len, &r.return_data[..8]);
                    }
                    // Check for EXTCALL ejections that tell us what to preload
                    let mut new_targets: Vec<(u128, u128)> = Vec::new();
                    let mut all_done = true;
                    let mut has_extcall_eject = false;

                    for (_i, r) in results.iter().enumerate() {
                        if r.ejected != 0 {
                            all_done = false;
                            if let Some(target) = extract_extcall_target(r) {
                                if !preloaded_ids.contains(&target) {
                                    new_targets.push(target);
                                    has_extcall_eject = true;
                                }
                            }
                        }
                    }

                    if all_done || !has_extcall_eject || preloaded_ids.len() >= MAX_PRELOADED_CONTRACTS {
                        // Final result: record completed and remaining
                        for (_i, r) in results.iter().enumerate() {
                            let idx = group[_i];
                            let t = &trackers[idx];
                            if r.ejected != 0 {
                                remaining.push((t.tx_index, t.msg_index));
                            } else {
                                completed.push(GpuCompletedMessage {
                                    tx_index: t.tx_index,
                                    msg_index: t.msg_index,
                                    kv_writes: Vec::new(),
                                });
                            }
                        }
                        group_completed = true;

                        if !all_done && !has_extcall_eject {
                            // Log ejection reasons for debugging
                            let mut reason_counts: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
                            for r in results.iter() {
                                if r.ejected != 0 {
                                    let reason_name = match r.ejection_reason {
                                        0 => "NONE".to_string(), 1 => "STORAGE_MISS".to_string(),
                                        2 => "MEMORY".to_string(), 3 => "KV_OVERFLOW".to_string(),
                                        4 => "CALLDATA".to_string(), 5 => "EXTCALL".to_string(),
                                        6 => "FUEL".to_string(), 7 => "TRAP".to_string(),
                                        8 => {
                                            // Decode which host function caused UNSUPPORTED
                                            let func_id = if r.return_data_len >= 4 {
                                                u32::from_le_bytes(r.return_data[0..4].try_into().unwrap_or([0;4]))
                                            } else { 
                                                log::info!("GPU: UNSUPPORTED with return_data_len={}, first bytes: {:?}", 
                                                    r.return_data_len, &r.return_data[..8]);
                                                99 
                                            };
                                            {
                                                let name = match func_id {
                                                    0 => "abort".to_string(),
                                                    1 => "load_storage".to_string(),
                                                    2 => "request_storage".to_string(),
                                                    3 => "log".to_string(),
                                                    4 => "balance".to_string(),
                                                    5 => "request_context".to_string(),
                                                    6 => "load_context".to_string(),
                                                    7 => "sequence".to_string(),
                                                    8 => "fuel".to_string(),
                                                    9 => "height".to_string(),
                                                    10 => "returndatacopy".to_string(),
                                                    11 => "request_tx".to_string(),
                                                    12 => "load_tx".to_string(),
                                                    13 => "request_block".to_string(),
                                                    14 => "load_block".to_string(),
                                                    15 => "call".to_string(),
                                                    16 => "delegatecall".to_string(),
                                                    17 => "staticcall".to_string(),
                                                    _ => {
                                                        if func_id >= 0x10000 {
                                                            format!("opcode_0x{:02x}", func_id - 0x10000)
                                                        } else {
                                                            format!("fn#{}", func_id)
                                                        }
                                                    }
                                                };
                                                format!("UNSUPPORTED({}, raw={}, debug=0x{:08x})", name, func_id, {
                                                let debug_val = if r.return_data_len >= 8 {
                                                    u32::from_le_bytes(r.return_data[4..8].try_into().unwrap_or([0;4]))
                                                } else { 0 };
                                                debug_val
                                            })
                                            }
                                        }
                                        _ => format!("UNKNOWN({})", r.ejection_reason),
                                    };
                                    *reason_counts.entry(reason_name).or_insert(0) += 1;
                                }
                            }
                            let reasons: Vec<String> = reason_counts.iter()
                                .map(|(k, v)| format!("{}={}", k, v)).collect();
                            log::info!("GPU: group ejected — reasons: {}", reasons.join(", "));
                        }
                        if preloaded_ids.len() >= MAX_PRELOADED_CONTRACTS {
                            log::warn!("GPU: hit preload limit ({} contracts) for group targeting ({},{})",
                                preloaded_ids.len(), first.target.0, first.target.1);
                        }
                        break;
                    }

                    // Preload newly discovered targets: both storage AND bytecode
                    for (block, tx) in &new_targets {
                        if preloaded_ids.insert((*block, *tx)) {
                            log::debug!("GPU: preloading contract ({},{}) for retry {}", block, tx, retry + 1);
                            // Load the contract's storage into kv_pairs
                            let target_key = alkane_id_key(*block, *tx);
                            let mut target_prefix = target_key.clone();
                            target_prefix.extend_from_slice(b"/storage/");
                            for key in store.keys_with_prefix(&target_prefix) {
                                if let Some(value) = store.get(&key) {
                                    kv_pairs.push((key, value));
                                }
                            }

                            // Load and parse the contract bytecode for multi-bytecode dispatch
                            if let Some(wasm) = load_contract_wasm(store, *block, *tx) {
                                if let Ok(info) = wasm_parser::parse_wasm_module(&wasm) {
                                    let ft: Vec<(u32, u32)> = info.functions.iter()
                                        .map(|f| (f.code_offset, f.local_count)).collect();
                                    let im = wasm_parser::build_import_map(&info.import_names);
                                    let gp = wasm_parser::pack_globals(&info.globals);
                                    let gc = info.globals.len() as u32;
                                    let dp = wasm_parser::pack_data_segments(&info.data_segments);
                                    let dc = info.data_segments.len() as u32;
                                    additional_contracts.push(pipeline::ContractInfo {
                                        alkane_id: (*block, *tx),
                                        bytecode: info.code_bytes,
                                        import_count: info.import_count,
                                        entry_pc: info.entry_pc,
                                        func_table: ft,
                                        import_map: im,
                                        globals_packed: gp,
                                        globals_count: gc,
                                        data_segments_packed: dp,
                                        data_segments_count: dc,
                                    });
                                    log::debug!("GPU: loaded bytecode for contract ({},{}) — {} contracts in shard",
                                        block, tx, additional_contracts.len() + 1);
                                } else {
                                    log::debug!("GPU: WASM parse failed for preloaded contract ({},{})", block, tx);
                                }
                            } else {
                                log::debug!("GPU: contract ({},{}) not in DB for preloading", block, tx);
                            }
                        }
                    }
                }
                Err(e) => {
                    log::debug!("GPU: shard dispatch failed: {}", e);
                    for &idx in group {
                        remaining.push((trackers[idx].tx_index, trackers[idx].msg_index));
                    }
                    group_completed = true;
                    break;
                }
            }
        }

        if !group_completed {
            // Exhausted retries
            log::warn!("GPU: exhausted {} retries for group targeting ({},{})",
                MAX_PRELOAD_RETRIES, first.target.0, first.target.1);
            for &idx in group {
                remaining.push((trackers[idx].tx_index, trackers[idx].msg_index));
            }
        }
    }

    // Non-eligible messages go to sequential
    for t in trackers {
        if !t.gpu_eligible {
            remaining.push((t.tx_index, t.msg_index));
        }
    }

    Ok(GpuBlockProcessResult { completed, remaining, gpu_used })
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
