use bitcoin;
use bitcoin_hashes::Hash;
use metashrew_core::{get, index_pointer::IndexPointer};
use metashrew_support::index_pointer::KeyValuePointer;
use std::io::Cursor;
use std::sync::Arc;

pub mod benchmark;

#[metashrew_core::main]
pub fn main(height: u32, block: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    // Store the block data
    let mut block_pointer = IndexPointer::from_keyword(format!("/blocks/{}", height).as_str());
    block_pointer.set(Arc::new(block.to_vec()));

    // Parse the block
    let parsed_block = metashrew_core::utils::consensus_decode::<bitcoin::Block>(
        &mut Cursor::new(block.to_vec()),
    )?;

    // Update block tracker
    let mut tracker = IndexPointer::from_keyword("/blocktracker");
    let mut new_tracker = tracker.get().as_ref().clone();
    new_tracker.extend_from_slice(parsed_block.header.block_hash().as_byte_array());
    tracker.set(Arc::new(new_tracker));

    // Benchmark: Create 1000 storage entries for benchmarking view function performance
    let storage_pointer = IndexPointer::from_keyword("/storage");
    for _i in 0u32..1000 {
        storage_pointer.append(Arc::new(vec![0x01]));
    }

    Ok(())
}

#[metashrew_core::view]
pub fn getblock(input: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut height_bytes = Cursor::new(input.to_vec());
    let height = metashrew_core::utils::consume_sized_int::<u32>(&mut height_bytes)?;
    let key = format!("/blocks/{}", height).into_bytes();
    let block_bytes_arc = get(Arc::new(key));
    let block_bytes: &Vec<u8> = &*block_bytes_arc;

    Ok(block_bytes.clone())
}

#[metashrew_core::view]
pub fn blocktracker(input: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let tracker_data = IndexPointer::from_keyword("/blocktracker")
        .get()
        .as_ref()
        .clone();

    Ok(tracker_data)
}

#[metashrew_core::view]
pub fn benchmark_view(input: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    // Read all 1000 storage values and concatenate them into a single bytearray
    let mut result = Vec::new();

    let storage_pointer = IndexPointer::from_keyword("/storage");
    for i in 0u32..1000 {
        let value = storage_pointer.select_index(i).get();
        result.extend_from_slice(value.as_ref());
    }

    Ok(result)
}


#[metashrew_core::view]
pub fn benchmark_view_with_set(input: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    // Read all 1000 storage values and concatenate them into a single bytearray
    let mut result = Vec::new();

    let storage_pointer = IndexPointer::from_keyword("/storage");
    for i in 0u32..1000 {
        let mut ptr = storage_pointer.select_index(i);
        ptr.set(Arc::new(vec![0x02]));
        let value = ptr.get();
        result.extend_from_slice(value.as_ref());
    }

    Ok(result)
}
