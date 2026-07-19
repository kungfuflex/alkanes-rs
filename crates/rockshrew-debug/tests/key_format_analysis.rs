use anyhow::Result;
use rocksdb::{DB, Options};

#[test]
fn test_analyze_txid_key_format() -> Result<()> {
    println!("\n=== Analyzing Exact Key Format ===\n");

    let opts = Options::default();
    let db = DB::open_for_read_only(&opts, "/data/.metashrew", false)?;

    // Get current height
    let height_bytes = db.get(b"__INTERNAL/height")?.expect("No indexed height");
    let current_height = u32::from_le_bytes([
        height_bytes[0],
        height_bytes[1],
        height_bytes[2],
        height_bytes[3],
    ]);
    println!("Current indexed height: {}\n", current_height);

    // Find some /txids/byheight keys and analyze their exact byte structure
    let mut iter = db.raw_iterator();
    iter.seek(b"/txids/byheight");

    let mut analyzed = 0;
    while iter.valid() && analyzed < 5 {
        if let Some(key) = iter.key() {
            let key_str = String::from_utf8_lossy(key);
            if key_str.starts_with("/txids/byheight") {
                println!("Key #{}: {}", analyzed + 1, key_str);
                println!("  Length: {} bytes", key.len());
                println!("  Hex: {}", hex::encode(key));
                println!("  Bytes: {:?}", key);

                // Try to parse the structure
                if let Some(value) = iter.value() {
                    println!("  Value length: {} bytes", value.len());
                    if key_str.contains("/length") {
                        let length_str = String::from_utf8_lossy(value);
                        println!("  Length value: '{}'", length_str);
                        if let Ok(count) = length_str.parse::<u32>() {
                            println!("  Parsed count: {}", count);
                        }
                    }
                }
                println!();
                analyzed += 1;
            } else {
                break;
            }
        }
        iter.next();
    }

    // Now test specific heights using what we learned
    println!("\n--- Testing Specific Height Formats ---\n");

    for test_height in [0, 1, 10, 100, 1000, current_height - 1, current_height] {
        println!("Height {}:", test_height);

        // Try to construct the key based on observed pattern
        // The pattern seems to be "/txids/byheight" + some binary + "/{height}/length"

        // Let's try seeking to the prefix and seeing what matches
        let prefix = format!("/txids/byheight");
        let mut iter = db.raw_iterator();
        iter.seek(prefix.as_bytes());

        let mut found_for_height = false;
        while iter.valid() {
            if let Some(key) = iter.key() {
                let key_str = String::from_utf8_lossy(key);
                if key_str.contains(&format!("/{}/", test_height)) || key_str.contains(&format!("/{}/length", test_height)) {
                    println!("  Found: {}", key_str);
                    if let Some(value) = iter.value() {
                        if key_str.contains("/length") {
                            let length_str = String::from_utf8_lossy(value);
                            println!("    Count: {}", length_str);
                        }
                    }
                    found_for_height = true;
                } else if found_for_height {
                    // We've moved past this height
                    break;
                }
            }
            iter.next();
        }

        if !found_for_height {
            println!("  No keys found");
        }
        println!();
    }

    Ok(())
}

#[test]
fn test_find_working_key_pattern() -> Result<()> {
    println!("\n=== Finding Working Key Pattern for Recent Heights ===\n");

    let opts = Options::default();
    let db = DB::open_for_read_only(&opts, "/data/.metashrew", false)?;

    // Get current height
    let height_bytes = db.get(b"__INTERNAL/height")?.expect("No indexed height");
    let current_height = u32::from_le_bytes([
        height_bytes[0],
        height_bytes[1],
        height_bytes[2],
        height_bytes[3],
    ]);

    // Scan all keys that might be related to the current height
    println!("Scanning for keys related to heights {} to {}\n", current_height - 5, current_height);

    let mut iter = db.raw_iterator();
    iter.seek_to_first();

    let height_str = current_height.to_string();
    let mut found_keys = Vec::new();

    while iter.valid() {
        if let Some(key) = iter.key() {
            let key_str = String::from_utf8_lossy(key);

            // Check if this key contains our current height as a number
            if key_str.contains(&height_str) {
                found_keys.push((key.to_vec(), key_str.to_string()));

                if found_keys.len() >= 20 {
                    break;
                }
            }
        }
        iter.next();
    }

    println!("Found {} keys containing height {}:\n", found_keys.len(), current_height);
    for (key_bytes, key_str) in found_keys.iter().take(10) {
        println!("  {}", key_str);
        println!("    Hex: {}", hex::encode(key_bytes));
    }

    Ok(())
}
