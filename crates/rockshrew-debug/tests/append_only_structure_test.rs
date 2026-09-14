use anyhow::Result;
use rocksdb::{DB, Options};

#[test]
fn test_list_txid_keys_from_real_db() -> Result<()> {
    println!("\n=== Testing Append-Only Structure from /data/.metashrew ===\n");

    // Open database in read-only mode so we don't conflict with running rockshrew-mono
    let mut opts = Options::default();
    let db = DB::open_for_read_only(&opts, "/data/.metashrew", false)?;

    // Get indexed height
    let height_bytes = db.get(b"__INTERNAL/height")?.expect("No indexed height");
    let current_height = u32::from_le_bytes([
        height_bytes[0],
        height_bytes[1],
        height_bytes[2],
        height_bytes[3],
    ]);
    println!("Current indexed height: {}", current_height);

    // Try to find any keys matching /txids/byheight pattern
    println!("\n--- Scanning for /txids/byheight keys ---");
    let mut iter = db.raw_iterator();
    iter.seek(b"/txids/byheight");

    let mut found_keys = 0;
    while iter.valid() && found_keys < 20 {
        if let Some(key) = iter.key() {
            let key_str = String::from_utf8_lossy(key);
            if key_str.starts_with("/txids/byheight") {
                let value_len = iter.value().map(|v| v.len()).unwrap_or(0);
                println!("  Found: {} (value: {} bytes)", key_str, value_len);
                found_keys += 1;
            } else {
                // We've passed the /txids/byheight prefix
                break;
            }
        }
        iter.next();
    }

    if found_keys == 0 {
        println!("  ❌ No /txids/byheight keys found!");
        println!("\n--- Let's check what keys DO exist ---");

        // Sample first 50 keys in the database
        let mut iter = db.raw_iterator();
        iter.seek_to_first();

        let mut count = 0;
        while iter.valid() && count < 50 {
            if let Some(key) = iter.key() {
                let key_str = String::from_utf8_lossy(key);
                let value_len = iter.value().map(|v| v.len()).unwrap_or(0);
                println!("{:4}. {} (value: {} bytes)", count + 1, key_str, value_len);
                count += 1;
            }
            iter.next();
        }
    } else {
        println!("  ✓ Found {} /txids/byheight keys", found_keys);
    }

    Ok(())
}

#[test]
fn test_read_specific_height_append_only() -> Result<()> {
    println!("\n=== Testing Specific Height Append-Only Read ===\n");

    let mut opts = Options::default();
    let db = DB::open_for_read_only(&opts, "/data/.metashrew", false)?;

    // Get current height
    let height_bytes = db.get(b"__INTERNAL/height")?.expect("No indexed height");
    let current_height = u32::from_le_bytes([
        height_bytes[0],
        height_bytes[1],
        height_bytes[2],
        height_bytes[3],
    ]);

    // Test a few recent heights
    for offset in 0..5 {
        let test_height = current_height.saturating_sub(offset);
        println!("Testing height {}:", test_height);

        // Try different key formats
        let formats = vec![
            format!("/txids/byheight/{}/length", test_height),
            format!("/txids/byheight/{}/0", test_height),
            format!("/runes/proto/alkanes/txids/byheight/{}/length", test_height),
            format!("/runes/proto/alkanes/txids/byheight/{}/0", test_height),
        ];

        for key_format in formats {
            match db.get(key_format.as_bytes())? {
                Some(value) => {
                    println!("  ✓ Key exists: {} ({} bytes)", key_format, value.len());
                    if key_format.contains("/length") {
                        let length_str = String::from_utf8_lossy(&value);
                        println!("    Length value: '{}'", length_str);
                        if let Ok(num) = length_str.parse::<u32>() {
                            println!("    Parsed as: {}", num);
                        }
                    } else {
                        // Show first 100 bytes of value
                        let preview = if value.len() > 100 {
                            format!("{}...", String::from_utf8_lossy(&value[..100]))
                        } else {
                            String::from_utf8_lossy(&value).to_string()
                        };
                        println!("    Value: {}", preview);
                    }
                }
                None => {
                    println!("  ✗ Key not found: {}", key_format);
                }
            }
        }
        println!();
    }

    Ok(())
}

#[test]
fn test_scan_all_height_patterns() -> Result<()> {
    println!("\n=== Scanning All Possible Height-Related Patterns ===\n");

    let mut opts = Options::default();
    let db = DB::open_for_read_only(&opts, "/data/.metashrew", false)?;

    let patterns = vec![
        "/txids/",
        "/runes/",
        "/alkanes/",
        "/height/",
        "/blockhash/",
    ];

    for pattern in patterns {
        println!("Pattern: {}", pattern);
        let mut iter = db.raw_iterator();
        iter.seek(pattern.as_bytes());

        let mut count = 0;
        while iter.valid() && count < 10 {
            if let Some(key) = iter.key() {
                let key_str = String::from_utf8_lossy(key);
                if key_str.starts_with(pattern) {
                    let value_len = iter.value().map(|v| v.len()).unwrap_or(0);
                    println!("  {}: {} bytes", key_str, value_len);
                    count += 1;
                } else {
                    break;
                }
            }
            iter.next();
        }
        if count == 0 {
            println!("  (no keys found)");
        }
        println!();
    }

    Ok(())
}
