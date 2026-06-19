// Test that uses the SAME key builder as the main code
use rocksdb::{DB, Options};
use rockshrew_runtime::adapter::RocksDBRuntimeAdapter;
use rockshrew_runtime::KeyValueStoreLike;

// Import the key builder from the main crate
// Since tests are part of the same crate, we can access the module
#[path = "../src/key_builder.rs"]
mod key_builder;

#[test]
fn test_key_builder_with_real_db() {
    println!("\n=== Testing Key Builder Against Real Database ===\n");

    let mut adapter = RocksDBRuntimeAdapter::open_optimized("/data/.metashrew".to_string()).unwrap();

    // Get current height
    let height_bytes = adapter.get(b"__INTERNAL/height").unwrap().expect("No indexed height");
    let current_height = u32::from_le_bytes([
        height_bytes[0],
        height_bytes[1],
        height_bytes[2],
        height_bytes[3],
    ]) as u64;
    println!("Current indexed height: {}\n", current_height);

    // Test several heights using the SAME key builder as main code
    for test_height in [0u64, 1, 10, 100, 1000, current_height - 10, current_height - 1] {
        print!("Height {}: ", test_height);

        // Use the EXACT same function as main code
        let length_key = key_builder::build_txid_length_key(test_height);

        match adapter.get(&length_key).unwrap() {
            Some(bytes) => {
                let count_str = String::from_utf8_lossy(&bytes);
                if let Ok(count) = count_str.parse::<u32>() {
                    print!("length={} ", count);

                    // Also test data key
                    let data_key = key_builder::build_txid_data_key(test_height, 0);
                    if let Some(data) = adapter.get(&data_key).unwrap() {
                        println!("✓ (data: {} bytes)", data.len());
                    } else {
                        println!("✗ (no data entry)");
                    }
                } else {
                    println!("✗ (invalid length value: '{}')", count_str);
                }
            }
            None => {
                println!("✗ (key not found)");
            }
        }
    }

    println!("\n=== Summary ===");
    println!("If you see '✓' above, the key builder is working correctly!");
    println!("If you see '✗ (key not found)', there might be an issue with:");
    println!("  1. The binary separator format");
    println!("  2. The adapter's get() method");
    println!("  3. The database structure");
}

#[test]
fn test_adapter_vs_direct_db() {
    println!("\n=== Comparing Adapter vs Direct DB Access ===\n");

    let opts = Options::default();
    let db = DB::open_for_read_only(&opts, "/data/.metashrew", false).unwrap();
    let mut adapter = RocksDBRuntimeAdapter::open_optimized("/data/.metashrew".to_string()).unwrap();

    for height in [0u64, 1, 10] {
        let key = key_builder::build_txid_length_key(height);

        let direct_result = db.get(&key).unwrap();
        let adapter_result = adapter.get(&key).unwrap();

        println!("Height {}:", height);
        println!("  Direct DB:  {:?}", direct_result.as_ref().map(|v| String::from_utf8_lossy(v).to_string()));
        println!("  Adapter:    {:?}", adapter_result.as_ref().map(|v| String::from_utf8_lossy(v).to_string()));
        println!("  Match: {}", direct_result == adapter_result);
        println!();
    }
}
