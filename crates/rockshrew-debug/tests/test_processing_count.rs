use rocksdb::{DB, Options};

#[test]
fn test_processing_count_vs_txid_count() {
    println!("\n=== Understanding /length vs /length/length ===\n");

    let opts = Options::default();
    let db = DB::open_for_read_only(&opts, "/data/.metashrew", false).unwrap();

    let height = 917504u64;

    let mut base_key = Vec::new();
    base_key.extend_from_slice(b"/txids/byheight");
    base_key.extend_from_slice(&height.to_le_bytes());

    // Check /length (should be processing count - how many times block was indexed)
    let mut length_key = base_key.clone();
    length_key.extend_from_slice(b"/length");

    println!("Checking /length (processing count):");
    println!("  Key: {}", hex::encode(&length_key));
    match db.get(&length_key) {
        Ok(Some(value)) => {
            let count_str = String::from_utf8_lossy(&value);
            if let Ok(count) = count_str.parse::<u32>() {
                println!("  ✓ Processing count: {} {}", count, if count > 1 { "❌ REORG!" } else { "✓ OK" });
            } else {
                println!("  Value (raw): {:?}", value);
            }
        }
        Ok(None) => {
            println!("  ✗ Not found");
        }
        Err(e) => {
            println!("  ✗ Error: {}", e);
        }
    }

    // Check /length/length (should be txid count for current version)
    let mut length_length_key = base_key.clone();
    length_length_key.extend_from_slice(b"/length/length");

    println!("\nChecking /length/length (txid count):");
    println!("  Key: {}", hex::encode(&length_length_key));
    match db.get(&length_length_key) {
        Ok(Some(value)) => {
            let count_str = String::from_utf8_lossy(&value);
            println!("  ✓ Transaction count: {}", count_str);
        }
        Ok(None) => {
            println!("  ✗ Not found");
        }
        Err(e) => {
            println!("  ✗ Error: {}", e);
        }
    }

    // Check specific processing entries
    for processing_idx in 0..5 {
        let mut proc_key = base_key.clone();
        proc_key.extend_from_slice(format!("/{}/length", processing_idx).as_bytes());

        match db.get(&proc_key) {
            Ok(Some(value)) => {
                let count_str = String::from_utf8_lossy(&value);
                println!("  Processing {} has {} txids", processing_idx, count_str);
            }
            Ok(None) => {
                if processing_idx == 0 {
                    println!("  Processing {} not found (unexpected!)", processing_idx);
                }
                break;
            }
            Err(e) => {
                println!("  Error: {}", e);
                break;
            }
        }
    }

    // Now test the algorithm: iterate until we stop finding processing entries
    println!("\n=== Scanning for all processings ===");
    let mut processing_count = 0;
    loop {
        let mut proc_key = base_key.clone();
        proc_key.extend_from_slice(format!("/{}/length", processing_count).as_bytes());

        match db.get(&proc_key) {
            Ok(Some(_)) => {
                processing_count += 1;
            }
            _ => break,
        }
    }
    println!("Found {} processings by iteration", processing_count);
    if processing_count > 1 {
        println!("❌ REORG DETECTED! Block was processed {} times", processing_count);
    } else {
        println!("✓ No reorg, block processed once");
    }
}
