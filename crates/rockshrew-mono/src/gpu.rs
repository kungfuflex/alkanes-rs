//! GPU integration for rockshrew-mono block processing.
//!
//! Intercepts block data before WASM execution to identify and
//! pre-process GPU-eligible alkanes messages in parallel.

use alkanes_gpu::tracking::{DependencyAnalyzer, StorageTracker};
use alkanes_gpu::AlkanesGpu;
use anyhow::Result;
use bitcoin::consensus::Decodable;
use bitcoin::Block;
use log::{info, warn, debug};
use ordinals::{Artifact, Runestone};
use std::io::Cursor;
use std::sync::OnceLock;

/// The alkanes protocol tag (must match AlkaneMessageContext::protocol_tag())
/// This is the protobuf-encoded protocol identifier for alkanes messages.
const ALKANES_PROTOCOL_TAG: u128 = 1;

static GPU_INSTANCE: OnceLock<Option<AlkanesGpu>> = OnceLock::new();

/// Initialize the GPU. Returns true if GPU is available.
pub fn init() -> bool {
    let gpu = GPU_INSTANCE.get_or_init(|| {
        match AlkanesGpu::new() {
            Ok(gpu) => {
                info!("GPU initialized for block processing");
                Some(gpu)
            }
            Err(e) => {
                warn!("GPU not available: {}. Running CPU-only.", e);
                None
            }
        }
    });
    gpu.is_some()
}

/// Get the GPU instance.
fn gpu() -> Option<&'static AlkanesGpu> {
    GPU_INSTANCE.get().and_then(|o| o.as_ref())
}

/// Pre-process a block with GPU acceleration.
///
/// Parses the raw block bytes, identifies GPU-eligible alkanes messages,
/// runs dependency analysis, and dispatches parallel groups to the GPU.
///
/// Returns the number of messages successfully processed on GPU.
/// The WASM indexer still runs on the full block — in the future,
/// GPU-completed messages would be skipped in the WASM path.
pub fn pre_process_block(height: u32, block_bytes: &[u8]) -> Result<GpuBlockStats> {
    let gpu = match gpu() {
        Some(g) => g,
        None => return Ok(GpuBlockStats::default()),
    };

    // Parse the raw block
    let block: Block = match bitcoin::consensus::deserialize(block_bytes) {
        Ok(b) => b,
        Err(e) => {
            debug!("GPU: could not parse block {}: {}", height, e);
            return Ok(GpuBlockStats::default());
        }
    };

    // Scan for alkanes messages
    let analyzer = analyze_block(&block, height as u64);
    let stats = analyzer.stats();

    if stats.gpu_eligible == 0 {
        return Ok(GpuBlockStats {
            total_messages: stats.total_messages,
            ..Default::default()
        });
    }

    let groups = analyzer.compute_parallel_groups();
    let trackers = analyzer.trackers();

    let mut gpu_dispatched: usize = 0;
    let mut gpu_completed: usize = 0;
    let mut gpu_ejected: usize = 0;

    for group in &groups {
        if group.len() < 4 {
            continue; // too small for GPU
        }

        // Build a minimal shard with just `end` bytecode for now.
        // Full integration would load actual contract WASM per target.
        let bytecode = vec![0x0Bu8]; // end
        let input = alkanes_gpu::pipeline::build_shard_input(
            group.len() as u32,
            height,
            1_000_000,
            &bytecode,
            0, 0,
            &[], &[],
        );

        gpu_dispatched += group.len();

        match gpu.execute_shard_raw(&input, group.len()) {
            Ok(results) => {
                for r in &results {
                    if r.ejected != 0 {
                        gpu_ejected += 1;
                    } else {
                        gpu_completed += 1;
                    }
                }
            }
            Err(e) => {
                debug!("GPU shard failed for block {}: {}", height, e);
                gpu_ejected += group.len();
            }
        }
    }

    let block_stats = GpuBlockStats {
        total_messages: stats.total_messages,
        gpu_eligible: stats.gpu_eligible,
        gpu_dispatched,
        gpu_completed,
        gpu_ejected,
        parallel_groups: stats.parallel_groups,
    };

    if gpu_dispatched > 0 {
        info!(
            "GPU block {}: {}/{} eligible, {} dispatched, {} completed, {} ejected, {} groups",
            height,
            stats.gpu_eligible, stats.total_messages,
            gpu_dispatched, gpu_completed, gpu_ejected,
            stats.parallel_groups,
        );
    }

    Ok(block_stats)
}

/// Stats from GPU block processing.
#[derive(Debug, Default, Clone)]
pub struct GpuBlockStats {
    pub total_messages: usize,
    pub gpu_eligible: usize,
    pub gpu_dispatched: usize,
    pub gpu_completed: usize,
    pub gpu_ejected: usize,
    pub parallel_groups: usize,
}

/// Scan a block for alkanes messages and build dependency analysis.
fn analyze_block(block: &Block, height: u64) -> DependencyAnalyzer {
    let mut analyzer = DependencyAnalyzer::new();

    for (tx_index, tx) in block.txdata.iter().enumerate() {
        if let Some(Artifact::Runestone(ref runestone)) = Runestone::decipher(tx) {
            if let Ok(protostones) = protorune_support::protostone::Protostone::from_runestone(runestone) {
                for (msg_index, stone) in protostones.iter().enumerate() {
                    // Parse cellpack to get target
                    if let Ok(values) = protorune_support::utils::decode_varint_list(
                        &mut Cursor::new(stone.message.clone())
                    ) {
                        if values.len() >= 2 {
                            let mut tracker = StorageTracker::new(
                                tx_index, msg_index, (values[0], values[1]),
                            );
                            if values.len() >= 3 {
                                tracker.opcode = Some(values[2]);
                            }
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
