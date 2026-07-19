use rocksdb::{DB, Options};

#[test]
fn find_keys_with_high_heights() {
    println!("\n=== Searching for Keys with Heights 880000+ ===\n");

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

    // Search for keys containing "88" or "89" or "90" or "91" or "92" (880000-929999 range)
    let search_patterns = vec!["880", "890", "900", "910", "920", "929"];

    for pattern in search_patterns {
        println!("Searching for keys containing '{}':", pattern);

        let mut iter = db.raw_iterator();
        iter.seek_to_first();

        let mut found = 0;
        while iter.valid() && found < 5 {
            if let Some(key) = iter.key() {
                let key_str = String::from_utf8_lossy(key);

                // Look for /txids/byheight keys containing this pattern
                if key_str.contains("/txids/byheight") && key_str.contains(pattern) {
                    println!("  Found: {}", key_str);
                    println!("    Hex: {}", hex::encode(key));
                    found += 1;
                }
            }
            iter.next();
        }

        if found == 0 {
            println!("  (none found)");
        }
        println!();
    }

    // Also check /__INTERNAL/height-to-hash/ to see what those keys look like
    println!("\nSample /__INTERNAL/height-to-hash/ keys:");
    let mut iter = db.raw_iterator();
    iter.seek(b"/__INTERNAL/height-to-hash/88");

    let mut count = 0;
    while iter.valid() && count < 5 {
        if let Some(key) = iter.key() {
            let key_str = String::from_utf8_lossy(key);
            if key_str.starts_with("/__INTERNAL/height-to-hash/") {
                println!("  {}", key_str);
                println!("    Hex: {}", hex::encode(key));
                count += 1;
            } else {
                break;
            }
        }
        iter.next();
    }
}

#[test]
fn examine_binary_separator_meaning() {
    println!("\n=== Examining Binary Separator ===\n");

    let opts = Options::default();
    let db = DB::open_for_read_only(&opts, "/data/.metashrew", false).unwrap();

    // Get first few /txids/byheight keys
    let mut iter = db.raw_iterator();
    iter.seek(b"/txids/byheight");

    println!("First 10 /txids/byheight keys with binary separator analysis:\n");

    let mut count = 0;
    while iter.valid() && count < 10 {
        if let Some(key) = iter.key() {
            if !key.starts_with(b"/txids/byheight") {
                break;
            }

            println!("Key #{}:", count + 1);
            println!("  Full hex: {}", hex::encode(key));

            // Extract the binary separator (bytes 15-23 if key is long enough)
            if key.len() > 23 {
                let separator = &key[15..23];
                println!("  Separator bytes: {:?}", separator);
                println!("  Separator hex: {}", hex::encode(separator));

                // Try to decode as u64 little-endian
                let mut buf = [0u8; 8];
                buf.copy_from_slice(separator);
                let as_u64_le = u64::from_le_bytes(buf);
                let as_u64_be = u64::from_be_bytes(buf);
                println!("  As u64 LE: {} (0x{:x})", as_u64_le, as_u64_le);
                println!("  As u64 BE: {} (0x{:x})", as_u64_be, as_u64_be);

                // Try to decode as u32 little-endian from different positions
                let as_u32_le_0 = u32::from_le_bytes([separator[0], separator[1], separator[2], separator[3]]);
                let as_u32_le_4 = u32::from_le_bytes([separator[4], separator[5], separator[6], separator[7]]);
                println!("  As u32 LE [0-3]: {}", as_u32_le_0);
                println!("  As u32 LE [4-7]: {}", as_u32_le_4);
            }

            println!("  UTF-8: {}", String::from_utf8_lossy(key));
            println!();

            count += 1;
        }
        iter.next();
    }
}
