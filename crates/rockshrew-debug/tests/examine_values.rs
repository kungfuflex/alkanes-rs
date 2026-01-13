use rocksdb::{DB, Options};

#[test]
fn examine_actual_values() {
    println!("\n=== Examining Actual Values ===\n");

    let opts = Options::default();
    let db = DB::open_for_read_only(&opts, "/data/.metashrew", false).unwrap();

    let height = 917504u64;
    let mut base_key = Vec::new();
    base_key.extend_from_slice(b"/txids/byheight");
    base_key.extend_from_slice(&height.to_le_bytes());

    // Look at the first few entries
    for idx in 0..10 {
        println!("Index {}:", idx);

        // Check /idx/0 (the data at position 0)
        let mut data_key = base_key.clone();
        data_key.extend_from_slice(format!("/{}/0", idx).as_bytes());
        match db.get(&data_key) {
            Ok(Some(value)) => {
                println!("  /{}/0 = {} bytes: {}", idx, value.len(), hex::encode(&value));
                // If it's a txid, it should be 32 bytes + maybe height prefix
                if value.len() == 71 || value.len() == 32 {
                    println!("    ^ Looks like a txid!");
                }
            }
            Ok(None) => {
                println!("  /{}/0 not found", idx);
            }
            Err(e) => {
                println!("  Error: {}", e);
            }
        }

        // Check /idx/length
        let mut length_key = base_key.clone();
        length_key.extend_from_slice(format!("/{}/length", idx).as_bytes());
        match db.get(&length_key) {
            Ok(Some(value)) => {
                let count_str = String::from_utf8_lossy(&value);
                println!("  /{}/length = {}", idx, count_str);
            }
            Ok(None) => {
                if idx < 3 {
                    println!("  /{}/length not found (unexpected!)", idx);
                }
            }
            Err(e) => {
                println!("  Error: {}", e);
            }
        }
        println!();
    }

    println!("\n=== Understanding the Pattern ===");
    println!("It looks like:");
    println!("  /{{idx}}/0 contains the actual txid data");
    println!("  /{{idx}}/length = 1 means there's 1 item in that sub-list");
    println!("\nSo the structure is:");
    println!("  /txids/byheight + [height] + /{{tx_index}}/0 = txid");
    println!("  /txids/byheight + [height] + /{{tx_index}}/length = 1");
    println!("\nThe outer list has 3905 transactions, NOT 3905 processings!");
    println!("This means there's NO reorg detection here - just a list of txids.");
}
