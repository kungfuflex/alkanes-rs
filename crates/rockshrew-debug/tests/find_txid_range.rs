use rocksdb::{DB, Options};

#[test]
fn find_txid_height_range() {
    println!("\n=== Finding Range of Heights with /txids/byheight Data ===\n");

    let opts = Options::default();
    let db = DB::open_for_read_only(&opts, "/data/.metashrew", false).unwrap();

    // Get current indexed height
    let height_bytes = db.get(b"__INTERNAL/height").unwrap().expect("No indexed height");
    let current_height = u32::from_le_bytes([
        height_bytes[0],
        height_bytes[1],
        height_bytes[2],
        height_bytes[3],
    ]);
    println!("Current indexed height: {}\n", current_height);

    // Scan backwards from current height to find the last height with txid data
    let mut iter = db.raw_iterator();
    iter.seek(b"/txids/byheight");

    let mut min_height = u64::MAX;
    let mut max_height = 0u64;
    let mut count = 0;

    while iter.valid() {
        if let Some(key) = iter.key() {
            let key_str = String::from_utf8_lossy(key);
            if !key_str.starts_with("/txids/byheight") {
                break;
            }

            // Try to extract height from key
            // Format: /txids/byheight [binary] /{height}/...
            if let Some(height_str) = extract_height_from_key(&key_str) {
                if let Ok(height) = height_str.parse::<u64>() {
                    min_height = min_height.min(height);
                    max_height = max_height.max(height);
                    count += 1;

                    if count <= 10 || count % 10000 == 0 {
                        println!("  Found height: {} (key: {})", height, key_str.chars().take(50).collect::<String>());
                    }
                }
            }
        }
        iter.next();
    }

    println!("\n=== Summary ===");
    println!("Total keys scanned: {}", count);
    println!("Height range: {} to {}", min_height, max_height);
    println!("Current indexed height: {}", current_height);
    println!("\nGap from max txid height to current: {}", current_height as u64 - max_height);
}

fn extract_height_from_key(key: &str) -> Option<String> {
    // Key format: /txids/byheight [binary] /{height}/...
    // After the binary separator, we have /{height}/

    // Find the last occurrence of a pattern like "/12345/"
    let parts: Vec<&str> = key.split('/').collect();
    for part in parts {
        if !part.is_empty() && part.chars().all(|c| c.is_numeric()) {
            return Some(part.to_string());
        }
    }
    None
}
