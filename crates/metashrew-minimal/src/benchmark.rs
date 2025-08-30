//! Benchmark-specific functions for testing stateful view performance
//!
//! This module contains the benchmark implementation that creates 1000 storage
//! entries and provides a view function to read them all.

use crate::*;
use metashrew_support::index_pointer::KeyValuePointer;

/// Benchmark main function that creates 1000 storage entries
/// This is a helper function, not the actual WASM entry point
pub fn benchmark_main_impl(height: u32, block: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    // Store the block data
    let mut block_pointer = IndexPointer::from_keyword(format!("/blocks/{}", height).as_str());
    block_pointer.set(Arc::new(block.to_vec()));

    // Parse the block
    let parsed_block = metashrew_support::utils::consensus_decode::<bitcoin::Block>(
        &mut Cursor::new(block.to_vec()),
    )?;

    // Update block tracker
    let mut tracker = IndexPointer::from_keyword("/blocktracker");
    let mut new_tracker = tracker.get().as_ref().clone();
    new_tracker.extend((&[parsed_block.header.block_hash()[0]]).to_vec());
    tracker.set(Arc::new(new_tracker));

    // Benchmark: Create 1000 storage entries for benchmarking view function performance
    let storage_pointer = IndexPointer::from_keyword("/storage");
    for _i in 0u32..1000 {
        storage_pointer.append(Arc::new(vec![0x01]));
    }

    Ok(())
}
