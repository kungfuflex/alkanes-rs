use rocksdb::{DB, Options};

#[test]
fn test_height_encoding_discovery() {
    println!("\n=== Discovering Correct Height Encoding ===\n");

    let opts = Options::default();
    let db = DB::open_for_read_only(&opts, "/data/.metashrew", false).unwrap();

    // Get some sample keys
    let mut iter = db.raw_iterator();
    iter.seek(b"/txids/byheight");

    println!("First 5 /txids/byheight keys:\n");

    let mut count = 0;
    while iter.valid() && count < 5 {
        if let Some(key) = iter.key() {
            if !key.starts_with(b"/txids/byheight") {
                break;
            }

            println!("Key #{}:", count + 1);
            println!("  Hex: {}", hex::encode(key));
            println!("  UTF-8: {}", String::from_utf8_lossy(key));

            // Extract the 8 bytes after "/txids/byheight" (15 bytes)
            if key.len() > 23 {
                let height_bytes = &key[15..23];
                let height = u64::from_le_bytes([
                    height_bytes[0],
                    height_bytes[1],
                    height_bytes[2],
                    height_bytes[3],
                    height_bytes[4],
                    height_bytes[5],
                    height_bytes[6],
                    height_bytes[7],
                ]);
                println!("  Decoded height (u64 LE): {}", height);

                // Now try to build a key for this height and see if it matches
                let mut test_key = Vec::new();
                test_key.extend_from_slice(b"/txids/byheight");
                test_key.extend_from_slice(&height.to_le_bytes());
                test_key.extend_from_slice(b"/length");

                println!("  Test key for /length: {}", hex::encode(&test_key));

                // Check if this key exists
                if let Ok(Some(value)) = db.get(&test_key) {
                    let count_str = String::from_utf8_lossy(&value);
                    println!("  ✓ FOUND /length key! Value: {}", count_str);
                } else {
                    println!("  ✗ /length key not found");
                }
            }

            println!();
            count += 1;
        }
        iter.next();
    }

    // Now test heights in the 880,000+ range
    println!("\nTesting heights in 880,000-929,755 range:\n");

    for height in [880000u64, 900000, 920000, 929755] {
        let mut key = Vec::new();
        key.extend_from_slice(b"/txids/byheight");
        key.extend_from_slice(&height.to_le_bytes());
        key.extend_from_slice(b"/length");

        println!("Height {}:", height);
        println!("  Key hex: {}", hex::encode(&key));

        match db.get(&key) {
            Ok(Some(value)) => {
                let count_str = String::from_utf8_lossy(&value);
                println!("  ✓ Found! Process count: {}", count_str);
            }
            Ok(None) => {
                println!("  ✗ Not found");
            }
            Err(e) => {
                println!("  ✗ Error: {}", e);
            }
        }
        println!();
    }
}
