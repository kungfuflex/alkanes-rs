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
///   [10..]: bytecode words
///   Then: function table entries (code_offset, local_count per function)
///   Then: K/V pairs (key_len, value_len, pad, pad, key[64], value[256] per pair)
pub fn build_shard_input(
    message_count: u32,
    block_height: u32,
    fuel: u64,
    bytecode: &[u8],
    import_count: u32,
    entry_pc: u32,
    func_table: &[(u32, u32)], // (code_offset, local_count) per function
    kv_pairs: &[(&[u8], &[u8])], // (key, value) pairs to preload
) -> Vec<u32> {
    // Pack bytecode into u32 words
    let bytecode_len = bytecode.len() as u32;
    let word_count = (bytecode.len() + 3) / 4;
    let mut bytecode_words: Vec<u32> = vec![0u32; word_count];
    for (i, &b) in bytecode.iter().enumerate() {
        bytecode_words[i / 4] |= (b as u32) << ((i % 4) * 8);
    }

    let func_table_offset = 10 + bytecode_words.len() as u32;
    let func_count = func_table.len() as u32;

    // Header
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
    ];

    // Bytecode
    input.extend_from_slice(&bytecode_words);

    // Function table
    for &(offset, locals) in func_table {
        input.push(offset);
        input.push(locals);
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
                &kv_pairs,
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
            2, 100, 1_000_000, &bytecode, 0, 0, &[], &kv_pairs,
        );

        // Check header
        assert_eq!(input[0], 2);     // message_count
        assert_eq!(input[1], 1);     // kv_count
        assert_eq!(input[2], 100);   // block_height
        assert_eq!(input[3], 1_000_000); // fuel_lo
        assert_eq!(input[5], 3);     // bytecode_len
        assert_eq!(input[6], 0);     // import_count
        assert_eq!(input[8], 0);     // func_count

        // Check bytecode is after header at offset 10
        let bc_word = input[10];
        assert_eq!(bc_word & 0xFF, 0x41); // first byte = i32.const opcode
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
}
