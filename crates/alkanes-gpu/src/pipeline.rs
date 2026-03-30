//! GPU execution pipeline: shard construction + dispatch + result collection.
//!
//! Takes parallel groups from the DependencyAnalyzer, builds GPU input
//! buffers with preloaded K/V context, dispatches to the GPU, and
//! collects results. Ejected messages are returned for CPU fallback.

use crate::device::GpuDevice;
use crate::tracking::{DependencyAnalyzer, StorageTracker};
use crate::types::*;
use crate::AlkanesGpu;
use anyhow::{Context, Result};
use std::collections::BTreeMap;

/// Minimum messages in a parallel group to justify GPU dispatch overhead.
const MIN_GPU_BATCH: usize = 4;

/// Maximum messages per GPU shard (matches shader workgroup size).
const MAX_SHARD_SIZE: usize = 64;

/// Result of processing a shard on GPU.
#[derive(Debug)]
pub struct ShardResult {
    /// Per-message results from GPU execution
    pub results: Vec<GpuMessageResult>,
    /// Indices of messages that were ejected (need CPU fallback)
    pub ejected_indices: Vec<usize>,
    /// Indices of messages that completed successfully on GPU
    pub completed_indices: Vec<usize>,
}

/// Builds a GPU input buffer for a shard of messages.
///
/// Input layout (all u32 words):
///   [0]:  message_count
///   [1]:  kv_count
///   [2]:  block_height
///   [3]:  base_fuel_lo
///   [4]:  base_fuel_hi
///   [5]:  bytecode_len (bytes)
///   [6]:  import_count
///   [7]:  entry_pc
///   [8]:  func_count
///   [9]:  func_table_offset
///   [10]: contract_count
///   [11]: contract_table_offset
///   [12]: import_map_offset
///   [13]: globals_count
///   [14]: globals_offset
///   [15]: data_segments_count
///   [16]: data_segments_offset
///   [17..]: bytecode words
///   Then: function table entries (code_offset, local_count per function)
///   Then: import mapping table (one u32 host_function_id per import)
///   Then: packed globals (2 u32s per global: value_lo, value_hi)
///   Then: packed data segments ([offset, length, data_words...] per segment)
///   Then: K/V pairs (key_len, value_len, pad, pad, key[64], value[256] per pair)
pub fn build_shard_input(
    message_count: u32,
    block_height: u32,
    fuel: u64,
    bytecode: &[u8],
    import_count: u32,
    entry_pc: u32,
    func_table: &[(u32, u32)], // (code_offset, local_count) per function
    import_map: &[u32],        // host function ID per import index
    kv_pairs: &[(&[u8], &[u8])], // (key, value) pairs to preload
    globals_packed: &[u32],       // packed globals from pack_globals()
    globals_count: u32,
    data_segments_packed: &[u32], // packed data segments from pack_data_segments()
    data_segments_count: u32,
) -> Vec<u32> {
    // Pack bytecode into u32 words
    let bytecode_len = bytecode.len() as u32;
    let word_count = (bytecode.len() + 3) / 4;
    let mut bytecode_words: Vec<u32> = vec![0u32; word_count];
    for (i, &b) in bytecode.iter().enumerate() {
        bytecode_words[i / 4] |= (b as u32) << ((i % 4) * 8);
    }

    let func_table_offset = 17 + bytecode_words.len() as u32;
    let func_count = func_table.len() as u32;

    // Header (17 words)
    let mut input: Vec<u32> = vec![
        message_count,
        kv_pairs.len() as u32,
        block_height,
        (fuel & 0xFFFFFFFF) as u32,
        (fuel >> 32) as u32,
        bytecode_len,
        import_count,
        entry_pc,
        func_count,
        func_table_offset,
        1, // contract_count (1 = primary only)
        0, // contract_table_offset (unused when count=1)
        0, // import_map_offset — placeholder, filled below
        globals_count,
        0, // globals_offset — placeholder, filled below
        data_segments_count,
        0, // data_segments_offset — placeholder, filled below
    ];

    // Bytecode
    input.extend_from_slice(&bytecode_words);

    // Function table
    for &(offset, locals) in func_table {
        input.push(offset);
        input.push(locals);
    }

    // Import mapping table
    let import_map_offset = input.len() as u32;
    input[12] = import_map_offset;
    input.extend_from_slice(import_map);

    // Globals
    let globals_offset = input.len() as u32;
    input[14] = globals_offset;
    input.extend_from_slice(globals_packed);

    // Data segments
    let data_segments_offset = input.len() as u32;
    input[16] = data_segments_offset;
    input.extend_from_slice(data_segments_packed);

    // K/V pairs: each is 4 + 64 + 256 = 324 u32s
    for (key, value) in kv_pairs {
        let key_len = key.len() as u32;
        let value_len = value.len() as u32;
        input.push(key_len);
        input.push(value_len);
        input.push(0); // pad
        input.push(0); // pad

        // Key (64 u32 words = 256 bytes max)
        let mut key_words = vec![0u32; 64];
        for (i, &b) in key.iter().enumerate() {
            key_words[i / 4] |= (b as u32) << ((i % 4) * 8);
        }
        input.extend_from_slice(&key_words);

        // Value (256 u32 words = 1024 bytes max)
        let mut value_words = vec![0u32; 256];
        for (i, &b) in value.iter().enumerate() {
            value_words[i / 4] |= (b as u32) << ((i % 4) * 8);
        }
        input.extend_from_slice(&value_words);
    }

    input
}


/// Information about an additional contract to include in a multi-bytecode shard.
#[derive(Debug, Clone)]
pub struct ContractInfo {
    pub alkane_id: (u128, u128), // (block, tx)
    pub bytecode: Vec<u8>,
    pub import_count: u32,
    pub entry_pc: u32,
    pub func_table: Vec<(u32, u32)>, // (code_offset, local_count)
    pub import_map: Vec<u32>,        // host function ID per import index
    pub globals_packed: Vec<u32>,    // packed globals
    pub globals_count: u32,
    pub data_segments_packed: Vec<u32>, // packed data segments
    pub data_segments_count: u32,
}

/// Builds a GPU input buffer for a multi-bytecode shard.
///
/// Extended input layout (all u32 words):
///   [0]:  message_count
///   [1]:  kv_count
///   [2]:  block_height
///   [3]:  base_fuel_lo
///   [4]:  base_fuel_hi
///   [5]:  bytecode_len (bytes) — primary contract
///   [6]:  import_count — primary contract
///   [7]:  entry_pc — primary contract
///   [8]:  func_count — primary contract
///   [9]:  func_table_offset — primary contract
///   [10]: contract_count (total contracts, 1 = primary only)
///   [11]: contract_table_offset (offset in u32 words to the contract index table)
///   [12]: import_map_offset — primary contract
///   [13]: globals_count — primary contract
///   [14]: globals_offset — primary contract
///   [15]: data_segments_count — primary contract
///   [16]: data_segments_offset — primary contract
///   [17..]: primary bytecode words
///   Then: primary function table
///   Then: primary import map
///   Then: primary globals + data segments
///   Then: additional contract bytecodes + function tables + import maps + globals + data (interleaved)
///   Then: contract index table (19 u32s per additional contract)
///   Then: K/V pairs
pub fn build_shard_input_multi(
    message_count: u32,
    block_height: u32,
    fuel: u64,
    // Primary contract
    primary_bytecode: &[u8],
    primary_import_count: u32,
    primary_entry_pc: u32,
    primary_func_table: &[(u32, u32)],
    primary_import_map: &[u32],
    primary_globals_packed: &[u32],
    primary_globals_count: u32,
    primary_data_segments_packed: &[u32],
    primary_data_segments_count: u32,
    // Additional contracts
    additional_contracts: &[ContractInfo],
    // K/V pairs
    kv_pairs: &[(&[u8], &[u8])],
) -> Vec<u32> {
    // Pack primary bytecode into u32 words
    let bytecode_len = primary_bytecode.len() as u32;
    let word_count = (primary_bytecode.len() + 3) / 4;
    let mut bytecode_words: Vec<u32> = vec![0u32; word_count];
    for (i, &b) in primary_bytecode.iter().enumerate() {
        bytecode_words[i / 4] |= (b as u32) << ((i % 4) * 8);
    }

    let total_contracts = 1 + additional_contracts.len() as u32;

    let func_table_offset = 17 + bytecode_words.len() as u32;
    let func_count = primary_func_table.len() as u32;

    // Header (17 words now)
    let mut input: Vec<u32> = vec![
        message_count,
        kv_pairs.len() as u32,
        block_height,
        (fuel & 0xFFFFFFFF) as u32,
        (fuel >> 32) as u32,
        bytecode_len,
        primary_import_count,
        primary_entry_pc,
        func_count,
        func_table_offset,
        total_contracts,
        0, // contract_table_offset — placeholder, filled below
        0, // import_map_offset — placeholder, filled below
        primary_globals_count,
        0, // globals_offset — placeholder
        primary_data_segments_count,
        0, // data_segments_offset — placeholder
    ];

    // Primary bytecode
    input.extend_from_slice(&bytecode_words);

    // Primary function table
    for &(offset, locals) in primary_func_table {
        input.push(offset);
        input.push(locals);
    }

    // Primary import map
    let primary_import_map_offset = input.len() as u32;
    input[12] = primary_import_map_offset;
    input.extend_from_slice(primary_import_map);

    // Primary globals
    let primary_globals_offset = input.len() as u32;
    input[14] = primary_globals_offset;
    input.extend_from_slice(primary_globals_packed);

    // Primary data segments
    let primary_data_segments_offset = input.len() as u32;
    input[16] = primary_data_segments_offset;
    input.extend_from_slice(primary_data_segments_packed);

    // Additional contracts: bytecode + function table + import map + globals + data for each
    struct ContractLayout {
        bytecode_offset: u32,
        bytecode_len: u32,
        import_count: u32,
        entry_pc: u32,
        func_count: u32,
        func_table_offset: u32,
        import_map_offset: u32,
        globals_count: u32,
        globals_offset: u32,
        data_segments_count: u32,
        data_segments_offset: u32,
        alkane_id: (u128, u128),
    }
    let mut layouts: Vec<ContractLayout> = Vec::new();

    for contract in additional_contracts {
        let bc_offset = input.len() as u32;
        let bc_len = contract.bytecode.len() as u32;
        let bc_word_count = (contract.bytecode.len() + 3) / 4;
        let mut bc_words: Vec<u32> = vec![0u32; bc_word_count];
        for (i, &b) in contract.bytecode.iter().enumerate() {
            bc_words[i / 4] |= (b as u32) << ((i % 4) * 8);
        }
        input.extend_from_slice(&bc_words);

        let ft_offset = input.len() as u32;
        let fc = contract.func_table.len() as u32;
        for &(offset, locals) in &contract.func_table {
            input.push(offset);
            input.push(locals);
        }

        let im_offset = input.len() as u32;
        input.extend_from_slice(&contract.import_map);

        let g_offset = input.len() as u32;
        input.extend_from_slice(&contract.globals_packed);

        let ds_offset = input.len() as u32;
        input.extend_from_slice(&contract.data_segments_packed);

        layouts.push(ContractLayout {
            bytecode_offset: bc_offset,
            bytecode_len: bc_len,
            import_count: contract.import_count,
            entry_pc: contract.entry_pc,
            func_count: fc,
            func_table_offset: ft_offset,
            import_map_offset: im_offset,
            globals_count: contract.globals_count,
            globals_offset: g_offset,
            data_segments_count: contract.data_segments_count,
            data_segments_offset: ds_offset,
            alkane_id: contract.alkane_id,
        });
    }

    // Contract index table (now 19 u32s per additional contract)
    let contract_table_offset = input.len() as u32;
    input[11] = contract_table_offset;

    for layout in &layouts {
        let (block, tx) = layout.alkane_id;
        // alkane_id_block as 4 u32 LE
        input.push(block as u32);
        input.push((block >> 32) as u32);
        input.push((block >> 64) as u32);
        input.push((block >> 96) as u32);
        // alkane_id_tx as 4 u32 LE
        input.push(tx as u32);
        input.push((tx >> 32) as u32);
        input.push((tx >> 64) as u32);
        input.push((tx >> 96) as u32);
        // bytecode_offset, bytecode_len, import_count, entry_pc, func_count, func_table_offset, import_map_offset
        input.push(layout.bytecode_offset);
        input.push(layout.bytecode_len);
        input.push(layout.import_count);
        input.push(layout.entry_pc);
        input.push(layout.func_count);
        input.push(layout.func_table_offset);
        input.push(layout.import_map_offset);
        // globals and data segments info
        input.push(layout.globals_count);
        input.push(layout.globals_offset);
        input.push(layout.data_segments_count);
        input.push(layout.data_segments_offset);
    }

    // K/V pairs: each is 4 + 64 + 256 = 324 u32s
    for (key, value) in kv_pairs {
        let key_len = key.len() as u32;
        let value_len = value.len() as u32;
        input.push(key_len);
        input.push(value_len);
        input.push(0); // pad
        input.push(0); // pad

        // Key (64 u32 words = 256 bytes max)
        let mut key_words = vec![0u32; 64];
        for (i, &b) in key.iter().enumerate() {
            key_words[i / 4] |= (b as u32) << ((i % 4) * 8);
        }
        input.extend_from_slice(&key_words);

        // Value (256 u32 words = 1024 bytes max)
        let mut value_words = vec![0u32; 256];
        for (i, &b) in value.iter().enumerate() {
            value_words[i / 4] |= (b as u32) << ((i % 4) * 8);
        }
        input.extend_from_slice(&value_words);
    }

    input
}

/// Process a block's messages through the GPU pipeline.
///
/// 1. Takes the dependency analyzer results
/// 2. Identifies parallel groups >= MIN_GPU_BATCH
/// 3. Builds GPU shards with preloaded K/V context
/// 4. Dispatches to GPU
/// 5. Returns completed + ejected results
pub fn process_block(
    gpu: &AlkanesGpu,
    analyzer: &DependencyAnalyzer,
    block_height: u32,
    fuel_per_message: u64,
    // In a real implementation, these would come from the contract registry:
    bytecode: &[u8],
    import_count: u32,
    entry_pc: u32,
    func_table: &[(u32, u32)],
    kv_context: &BTreeMap<Vec<u8>, Vec<u8>>,
) -> Result<Vec<ShardResult>> {
    let groups = analyzer.compute_parallel_groups();
    let mut shard_results = Vec::new();

    for group in &groups {
        if group.len() < MIN_GPU_BATCH {
            // Too small for GPU — mark all as ejected for CPU fallback
            let results: Vec<GpuMessageResult> = group
                .iter()
                .map(|_| {
                    let mut r = GpuMessageResult::default();
                    r.ejected = 1;
                    r.ejection_reason = EJECTION_NONE; // not a real ejection, just too small
                    r
                })
                .collect();
            shard_results.push(ShardResult {
                ejected_indices: group.clone(),
                completed_indices: Vec::new(),
                results,
            });
            continue;
        }

        // Build K/V pairs to preload for this shard
        let kv_pairs: Vec<(&[u8], &[u8])> = kv_context
            .iter()
            .map(|(k, v)| (k.as_slice(), v.as_slice()))
            .collect();

        // Chunk the group into shards of MAX_SHARD_SIZE
        for chunk in group.chunks(MAX_SHARD_SIZE) {
            let shard_input = build_shard_input(
                chunk.len() as u32,
                block_height,
                fuel_per_message,
                bytecode,
                import_count,
                entry_pc,
                func_table,
                &(0..import_count).collect::<Vec<u32>>(), // identity map
                &kv_pairs,
                &[], // no globals
                0,
                &[], // no data segments
                0,
            );

            match gpu.execute_shard_raw(&shard_input, chunk.len()) {
                Ok(results) => {
                    let mut ejected = Vec::new();
                    let mut completed = Vec::new();

                    for (i, r) in results.iter().enumerate() {
                        if r.ejected != 0 {
                            ejected.push(chunk[i]);
                        } else {
                            completed.push(chunk[i]);
                        }
                    }

                    shard_results.push(ShardResult {
                        results,
                        ejected_indices: ejected,
                        completed_indices: completed,
                    });
                }
                Err(e) => {
                    log::warn!("GPU shard dispatch failed, ejecting all: {}", e);
                    let results: Vec<GpuMessageResult> = chunk
                        .iter()
                        .map(|_| {
                            let mut r = GpuMessageResult::default();
                            r.ejected = 1;
                            r.ejection_reason = EJECTION_KV_OVERFLOW;
                            r
                        })
                        .collect();
                    shard_results.push(ShardResult {
                        ejected_indices: chunk.to_vec(),
                        completed_indices: Vec::new(),
                        results,
                    });
                }
            }
        }
    }

    Ok(shard_results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tracking::StorageTracker;

    #[test]
    fn test_build_shard_input_layout() {
        let bytecode = vec![0x41, 0x2A, 0x0B]; // i32.const 42, end
        let kv_pairs: Vec<(&[u8], &[u8])> = vec![
            (b"key1", b"value1"),
        ];

        let input = build_shard_input(
            2, 100, 1_000_000, &bytecode, 0, 0, &[], &[], &kv_pairs,
            &[], 0, &[], 0,
        );

        // Check header
        assert_eq!(input[0], 2);     // message_count
        assert_eq!(input[1], 1);     // kv_count
        assert_eq!(input[2], 100);   // block_height
        assert_eq!(input[3], 1_000_000); // fuel_lo
        assert_eq!(input[5], 3);     // bytecode_len
        assert_eq!(input[6], 0);     // import_count
        assert_eq!(input[8], 0);     // func_count

        // Check bytecode is after header at offset 17
        let bc_word = input[17];
        assert_eq!(bc_word & 0xFF, 0x41); // first byte = i32.const opcode
    }

    #[test]
    fn test_build_shard_input_with_globals_and_data() {
        let bytecode = vec![0x0B]; // just end
        let globals = vec![1048576u32, 0, 42, 0]; // 2 globals: [1048576, 42]
        let data_seg = vec![256u32, 4, 0x64636261]; // offset=256, len=4, "abcd"

        let input = build_shard_input(
            1, 100, 1_000_000, &bytecode, 0, 0, &[], &[], &[],
            &globals, 2, &data_seg, 1,
        );

        // Check header globals/data fields
        assert_eq!(input[13], 2); // globals_count
        let g_off = input[14] as usize;
        assert_eq!(input[g_off], 1048576);   // global[0] lo
        assert_eq!(input[g_off + 1], 0);     // global[0] hi
        assert_eq!(input[g_off + 2], 42);    // global[1] lo

        assert_eq!(input[15], 1); // data_segments_count
        let ds_off = input[16] as usize;
        assert_eq!(input[ds_off], 256);       // segment offset
        assert_eq!(input[ds_off + 1], 4);     // segment length
        assert_eq!(input[ds_off + 2], 0x64636261); // data word
    }

    #[test]
    fn test_process_block_small_group_ejects() {
        let gpu = match AlkanesGpu::new() {
            Ok(gpu) => gpu,
            Err(e) => {
                eprintln!("Skipping GPU test: {}", e);
                return;
            }
        };

        let mut analyzer = DependencyAnalyzer::new();
        // Only 2 messages — below MIN_GPU_BATCH
        let a = StorageTracker::new(0, 0, (2, 1));
        let b = StorageTracker::new(1, 0, (2, 2));
        analyzer.add_tracker(a);
        analyzer.add_tracker(b);

        let results = process_block(
            &gpu, &analyzer, 100, 1_000_000,
            &[], 0, 0, &[], &BTreeMap::new(),
        ).unwrap();

        // Both should be ejected (group too small)
        assert!(!results.is_empty());
        for shard in &results {
            assert!(shard.completed_indices.is_empty());
        }
    }

    #[test]
    fn test_process_block_large_group_dispatches() {
        let gpu = match AlkanesGpu::new() {
            Ok(gpu) => gpu,
            Err(e) => {
                eprintln!("Skipping GPU test: {}", e);
                return;
            }
        };

        let mut analyzer = DependencyAnalyzer::new();
        // 6 messages that all write the SAME key → they conflict and form ONE group
        // Group of 6 > MIN_GPU_BATCH(4) → dispatched to GPU
        for i in 0..6 {
            let mut t = StorageTracker::new(i, 0, (2, 1)); // same target
            t.record_write(b"shared_key".to_vec());
            analyzer.add_tracker(t);
        }

        let groups = analyzer.compute_parallel_groups();
        assert_eq!(groups.len(), 1, "all should be in one conflict group");
        assert_eq!(groups[0].len(), 6);

        let bytecode = vec![0x0B]; // just end
        let results = process_block(
            &gpu, &analyzer, 100, 1_000_000,
            &bytecode, 0, 0, &[], &BTreeMap::new(),
        ).unwrap();

        // With bytecode = just `end`, all messages should succeed on GPU
        let total_completed: usize = results.iter().map(|s| s.completed_indices.len()).sum();
        assert!(total_completed > 0, "messages should complete on GPU");
        assert_eq!(total_completed, 6);
    }

    #[test]
    fn test_build_shard_input_multi_layout() {
        let bytecode_a = vec![0x41, 0x2A, 0x0B]; // i32.const 42, end
        let bytecode_b = vec![0x41, 0x07, 0x0B]; // i32.const 7, end

        let additional = vec![ContractInfo {
            alkane_id: (2, 1),
            bytecode: bytecode_b.clone(),
            import_count: 0,
            entry_pc: 0,
            func_table: vec![],
            import_map: vec![],
            globals_packed: vec![],
            globals_count: 0,
            data_segments_packed: vec![],
            data_segments_count: 0,
        }];

        let input = build_shard_input_multi(
            2, 100, 1_000_000,
            &bytecode_a, 0, 0, &[], &[],
            &[], 0, &[], 0,
            &additional,
            &[],
        );

        // Check header
        assert_eq!(input[0], 2);     // message_count
        assert_eq!(input[1], 0);     // kv_count
        assert_eq!(input[2], 100);   // block_height
        assert_eq!(input[5], 3);     // primary bytecode_len
        assert_eq!(input[10], 2);    // contract_count (1 primary + 1 additional)

        // Check contract table
        let ct_offset = input[11] as usize;
        // alkane_id_block[0]
        assert_eq!(input[ct_offset], 2);    // block lo
        assert_eq!(input[ct_offset + 4], 1); // tx lo
        // bytecode_len
        assert_eq!(input[ct_offset + 9], 3); // bytecode_b len = 3
    }

}
