use rocksdb::{DB, Options};
use std::collections::HashSet;

#[test]
fn scan_all_heights_in_db() {
    println!("\n=== Scanning All Heights in Database ===\n");

    let opts = Options::default();
    let db = DB::open_for_read_only(&opts, "/data/.metashrew", false).unwrap();

    let mut iter = db.raw_iterator();
    iter.seek(b"/txids/byheight");

    let mut heights = HashSet::new();
    let mut count = 0;

    // Scan through keys to find all unique heights
    while iter.valid() && count < 100000 {
        if let Some(key) = iter.key() {
            if !key.starts_with(b"/txids/byheight") {
                break;
            }

            // Extract height from bytes 15-23
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
                heights.insert(height);
            }

            count += 1;
        }
        iter.next();
    }

    println!("Found {} unique heights", heights.len());
    let mut heights_vec: Vec<u64> = heights.into_iter().collect();
    heights_vec.sort();

    println!("\nFirst 20 heights:");
    for h in heights_vec.iter().take(20) {
        println!("  {}", h);
    }

    if heights_vec.len() > 20 {
        println!("\nLast 20 heights:");
        for h in heights_vec.iter().rev().take(20).rev() {
            println!("  {}", h);
        }
    }

    println!("\nHeight range: {} to {}", heights_vec.first().unwrap(), heights_vec.last().unwrap());
}
