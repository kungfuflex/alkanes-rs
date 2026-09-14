use rocksdb::{DB, Options};

#[test]
fn search_for_length_patterns() {
    println!("\n=== Searching for Different /length Key Patterns ===\n");

    let opts = Options::default();
    let db = DB::open_for_read_only(&opts, "/data/.metashrew", false).unwrap();

    // Search for keys containing "length"
    let mut iter = db.raw_iterator();
    iter.seek(b"/txids/byheight");

    let mut patterns = std::collections::HashMap::new();

    let mut count = 0;
    while iter.valid() && count < 1000 {
        if let Some(key) = iter.key() {
            let key_str = String::from_utf8_lossy(key);
            if !key_str.starts_with("/txids/byheight") {
                break;
            }

            // Categorize the pattern
            if key_str.contains("/length/length") {
                *patterns.entry("has /length/length").or_insert(0) += 1;
                if patterns.get("has /length/length").unwrap() <= &3 {
                    println!("Found /length/length: {}", hex::encode(key));
                }
            } else if key_str.ends_with("/length") {
                *patterns.entry("ends with /length").or_insert(0) += 1;
                if patterns.get("ends with /length").unwrap() <= &3 {
                    println!("Found ends with /length: {}", hex::encode(key));
                }
            } else {
                *patterns.entry("other").or_insert(0) += 1;
            }

            count += 1;
        }
        iter.next();
    }

    println!("\n=== Pattern Summary ===");
    for (pattern, count) in patterns.iter() {
        println!("{}: {}", pattern, count);
    }

    // Now let me check if there's a way to get the append count
    // by checking what `.select_value::<u64>(917504)` gives us
    println!("\n=== Testing .select_value Behavior ===");

    let height = 917504u64;
    let mut key = Vec::new();
    key.extend_from_slice(b"/txids/byheight");
    key.extend_from_slice(&height.to_le_bytes());

    println!("Base key for height {}: {}", height, hex::encode(&key));

    // Try just the base key
    match db.get(&key) {
        Ok(Some(value)) => {
            println!("✓ Base key has value: {}", String::from_utf8_lossy(&value));
        }
        Ok(None) => {
            println!("✗ Base key not found (expected for KeyValuePointer)");
        }
        Err(e) => {
            println!("✗ Error: {}", e);
        }
    }

    // The KeyValuePointer .append() method adds to a list
    // Let's see if there's a key with just the height that tells us the append count
    let mut length_key = key.clone();
    length_key.extend_from_slice(b"/length");
    println!("\nTrying /length key: {}", hex::encode(&length_key));
    match db.get(&length_key) {
        Ok(Some(value)) => {
            println!("✓ /length value: {}", String::from_utf8_lossy(&value));
        }
        Ok(None) => {
            println!("✗ /length not found");
        }
        Err(e) => {
            println!("✗ Error: {}", e);
        }
    }

    // Try /length/length
    let mut length_length_key = key.clone();
    length_length_key.extend_from_slice(b"/length/length");
    println!("\nTrying /length/length key: {}", hex::encode(&length_length_key));
    match db.get(&length_length_key) {
        Ok(Some(value)) => {
            println!("✓ /length/length value: {}", String::from_utf8_lossy(&value));
        }
        Ok(None) => {
            println!("✗ /length/length not found");
        }
        Err(e) => {
            println!("✗ Error: {}", e);
        }
    }
}
