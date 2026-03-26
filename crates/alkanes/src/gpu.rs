//! GPU-accelerated block processing for alkanes.
//!
//! Scans a block for alkanes messages, performs dependency analysis to
//! find parallelizable groups, dispatches eligible groups to the GPU,
//! and returns results. Messages that eject from GPU or aren't eligible
//! are returned for normal sequential processing.
//!
//! Feature-gated behind `gpu`.

use alkanes_gpu::tracking::{DependencyAnalyzer, StorageTracker};
use alkanes_gpu::AlkanesGpu;
use bitcoin::Block;
#[allow(unused_imports)]
use metashrew_core::{
    println,
    stdio::{stdout, Write},
};
use ordinals::{Artifact, Runestone};
use protorune::message::MessageContext;
use protorune_support::protostone::Protostone;
use protorune_support::utils::decode_varint_list;
use std::io::Cursor;
use std::sync::OnceLock;

use crate::message::AlkaneMessageContext;

/// Global GPU instance — initialized once, reused across blocks.
static GPU: OnceLock<Option<AlkanesGpu>> = OnceLock::new();

/// Initialize the GPU pipeline. Call once at startup.
/// Returns true if GPU is available, false if fallback to CPU-only.
pub fn init_gpu() -> bool {
    let gpu = GPU.get_or_init(|| match AlkanesGpu::new() {
        Ok(gpu) => {
            println!("GPU initialized successfully");
            Some(gpu)
        }
        Err(e) => {
            eprintln!("GPU not available, running CPU-only: {}", e);
            None
        }
    });
    gpu.is_some()
}

/// Get a reference to the GPU instance (if available).
fn get_gpu() -> Option<&'static AlkanesGpu> {
    GPU.get().and_then(|opt| opt.as_ref())
}

/// Result of GPU pre-processing a block.
pub struct GpuBlockResult {
    /// Tracker indices that completed on GPU (don't need sequential processing)
    pub completed: Vec<(usize, usize)>, // (tx_index, msg_index)
    /// Tracker indices that need sequential processing (ejected or ineligible)
    pub sequential: Vec<(usize, usize)>,
    /// Dependency analysis stats for logging
    pub stats: alkanes_gpu::tracking::DependencyStats,
}

/// Scan a block for alkanes messages and build a dependency analyzer.
///
/// This extracts protostones from each transaction's runestone, identifies
/// which ones target alkanes (protocol_tag match), and creates storage
/// trackers for GPU eligibility analysis.
pub fn analyze_block(block: &Block, height: u64) -> DependencyAnalyzer {
    let mut analyzer = DependencyAnalyzer::new();

    for (tx_index, tx) in block.txdata.iter().enumerate() {
        if let Some(Artifact::Runestone(ref runestone)) = Runestone::decipher(tx) {
            if let Ok(protostones) = Protostone::from_runestone(runestone) {
                for (msg_index, stone) in protostones.iter().enumerate() {
                    // Only process protostones targeting our protocol
                    if stone.protocol_tag != AlkaneMessageContext::protocol_tag() {
                        continue;
                    }

                    // Try to parse the cellpack to determine target
                    if let Ok(values) =
                        decode_varint_list(&mut Cursor::new(stone.message.clone()))
                    {
                        if values.len() >= 2 {
                            let target_block = values[0];
                            let target_tx = values[1];
                            let mut tracker = StorageTracker::new(
                                tx_index,
                                msg_index,
                                (target_block, target_tx),
                            );

                            // Set opcode if present
                            if values.len() >= 3 {
                                tracker.opcode = Some(values[2]);
                            }

                            // Store calldata for GPU execution
                            tracker.calldata = stone.message.clone();

                            analyzer.add_tracker(tracker);
                        }
                    }
                }
            }
        }
    }

    analyzer
}

/// Try to accelerate block processing with GPU.
///
/// Returns None if GPU is not available or no messages are GPU-eligible.
/// Returns Some(GpuBlockResult) with completed and remaining message indices.
pub fn try_gpu_accelerate(block: &Block, height: u32) -> Option<GpuBlockResult> {
    let gpu = get_gpu()?;
    let analyzer = analyze_block(block, height as u64);
    let stats = analyzer.stats();

    if stats.gpu_eligible == 0 {
        // debug:("block {}: no GPU-eligible messages", height);
        return None;
    }

    println!(
        "block {}: {} messages, {} GPU-eligible, {} parallel groups, {} conflicts",
        height,
        stats.total_messages,
        stats.gpu_eligible,
        stats.parallel_groups,
        stats.total_conflicts
    );

    let groups = analyzer.compute_parallel_groups();
    let trackers = analyzer.trackers();

    let mut completed: Vec<(usize, usize)> = Vec::new();
    let mut sequential: Vec<(usize, usize)> = Vec::new();

    for group in &groups {
        if group.len() < 4 {
            // Too small for GPU — route to sequential
            for &idx in group {
                let t = &trackers[idx];
                sequential.push((t.tx_index, t.msg_index));
            }
            continue;
        }

        // For now, we dispatch with empty bytecode as a proof-of-concept.
        // Full integration would load the actual contract bytecode per target,
        // preload storage context, and process real WASM execution.
        //
        // The GPU dispatch returns which messages completed and which ejected.
        // For this phase, we just mark all GPU-eligible large groups as
        // "attempted" and log the result. Real K/V state application comes
        // in a future iteration.

        let bytecode = vec![0x0Bu8]; // just `end` — placeholder
        let input = alkanes_gpu::pipeline::build_shard_input(
            group.len() as u32,
            height,
            1_000_000, // fuel
            &bytecode,
            0, // import_count
            0, // entry_pc
            &[],
            &[],
        );

        match gpu.execute_shard_raw(&input, group.len()) {
            Ok(results) => {
                for (i, r) in results.iter().enumerate() {
                    let t = &trackers[group[i]];
                    if r.ejected != 0 {
                        sequential.push((t.tx_index, t.msg_index));
                    } else {
                        completed.push((t.tx_index, t.msg_index));
                    }
                }
            }
            Err(e) => {
                eprintln!("GPU shard failed: {}, falling back to sequential", e);
                for &idx in group {
                    let t = &trackers[idx];
                    sequential.push((t.tx_index, t.msg_index));
                }
            }
        }
    }

    // Also route non-eligible messages to sequential
    for (i, t) in trackers.iter().enumerate() {
        if !t.gpu_eligible {
            sequential.push((t.tx_index, t.msg_index));
        }
    }

    println!(
        "block {}: GPU completed {} messages, {} need sequential",
        height,
        completed.len(),
        sequential.len()
    );

    Some(GpuBlockResult {
        completed,
        sequential,
        stats,
    })
}
