// Simulate exactly what the rockshrew-debug scanner will see
use rockshrew_runtime::adapter::RocksDBRuntimeAdapter;
use rockshrew_runtime::KeyValueStoreLike;

#[path = "../src/key_builder.rs"]
mod key_builder;

#[test]
fn simulate_reorg_scan() {
    println!("\n=== Simulating Reorg Scan (What rockshrew-debug Will See) ===\n");

    let mut adapter = RocksDBRuntimeAdapter::open_optimized("/data/.metashrew".to_string()).unwrap();

    // Get current height
    let height_bytes = adapter.get(b"__INTERNAL/height").unwrap().expect("No indexed height");
    let current_height = u32::from_le_bytes([
        height_bytes[0],
        height_bytes[1],
        height_bytes[2],
        height_bytes[3],
    ]) as u64;

    println!("Database indexed height: {}\n", current_height);
    println!("Scanning heights {} down to {} (sample)\n", current_height, current_height.saturating_sub(20));

    let mut blocks_with_data = 0;
    let mut blocks_scanned = 0;

    // Scan backwards from current height (same as the tool does)
    for height in (current_height.saturating_sub(20)..=current_height).rev() {
        blocks_scanned += 1;

        // THIS IS THE EXACT CODE FROM THE TOOL
        let length_key = key_builder::build_txid_length_key(height);

        match adapter.get(&length_key).unwrap() {
            Some(bytes) => {
                let count_str = String::from_utf8_lossy(&bytes);
                if let Ok(process_count) = count_str.parse::<u32>() {
                    blocks_with_data += 1;
                    let status = if process_count > 1 { "❌ REORG!" } else { "✓ OK" };
                    println!("  Height {}: count={} {}", height, process_count, status);

                    if process_count > 1 {
                        println!("    ^^^ MISSED REORG DETECTED! ^^^");
                    }
                }
            }
            None => {
                // No data for this height - this is expected for recent blocks
                if blocks_scanned <= 5 {
                    println!("  Height {}: (no /txids/byheight entry)", height);
                }
            }
        }
    }

    println!("\n=== Scan Summary ===");
    println!("Blocks scanned: {}", blocks_scanned);
    println!("Blocks with txid data: {}", blocks_with_data);
    println!("\nIf 'Blocks with data' is 0, it means:");
    println!("  - Recent blocks don't have /txids/byheight entries yet");
    println!("  - The indexer may not be configured to store this data");
    println!("  - Try scanning a lower height range");
}
