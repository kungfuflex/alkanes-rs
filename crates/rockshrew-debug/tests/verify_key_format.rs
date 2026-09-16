use rocksdb::{DB, Options};
use rockshrew_runtime::adapter::RocksDBRuntimeAdapter;
use rockshrew_runtime::KeyValueStoreLike;

#[test]
fn test_our_key_format_works() {
    println!("\n=== Testing Our Key Format ===\n");

    let opts = Options::default();
    let db = DB::open_for_read_only(&opts, "/data/.metashrew", false).unwrap();
    let mut adapter = RocksDBRuntimeAdapter::open_optimized("/data/.metashrew".to_string()).unwrap();

    // Test heights 0, 1, 10, 100
    for height in [0u64, 1, 10, 100] {
        println!("Testing height {}:", height);

        // Build key using our format
        let mut length_key = Vec::new();
        length_key.extend_from_slice(b"/txids/byheight");
        length_key.extend_from_slice(&[0x00, 0x00, 0x0e, 0x00, 0x00, 0x00, 0x00, 0x00]);
        length_key.extend_from_slice(format!("/{}/length", height).as_bytes());

        println!("  Key hex: {}", hex::encode(&length_key));

        // Try to read it
        match db.get(&length_key).unwrap() {
            Some(value) => {
                let count_str = String::from_utf8_lossy(&value);
                println!("  ✓ Found! Value: '{}' ({} bytes)", count_str, value.len());
                if let Ok(count) = count_str.parse::<u32>() {
                    println!("    Parsed as: {}", count);

                    // Also try to read /0 entry
                    let mut data_key = Vec::new();
                    data_key.extend_from_slice(b"/txids/byheight");
                    data_key.extend_from_slice(&[0x00, 0x00, 0x0e, 0x00, 0x00, 0x00, 0x00, 0x00]);
                    data_key.extend_from_slice(format!("/{}/0", height).as_bytes());

                    if let Some(data) = db.get(&data_key).unwrap() {
                        println!("    Data entry /0 exists ({} bytes)", data.len());
                        println!("    Data: {}", String::from_utf8_lossy(&data));
                    }
                }
            }
            None => {
                println!("  ✗ Not found");
            }
        }
        println!();
    }
}
