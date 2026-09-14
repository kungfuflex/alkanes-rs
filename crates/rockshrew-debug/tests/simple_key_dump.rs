use rocksdb::{DB, Options};

#[test]
fn dump_txid_key_bytes() {
    let opts = Options::default();
    let db = DB::open_for_read_only(&opts, "/data/.metashrew", false).unwrap();

    let mut iter = db.raw_iterator();
    iter.seek(b"/txids/byheight");

    println!("\n=== First 3 /txids/byheight keys ===\n");

    let mut count = 0;
    while iter.valid() && count < 3 {
        if let Some(key) = iter.key() {
            // Check if still in txids range
            if !key.starts_with(b"/txids/byheight") {
                break;
            }

            println!("Key #{}:", count + 1);
            println!("  Raw bytes ({} bytes): {:?}", key.len(), key);
            println!("  Hex: {}", hex::encode(key));
            println!("  As UTF-8: {}", String::from_utf8_lossy(key));

            if let Some(value) = iter.value() {
                println!("  Value ({} bytes): {}", value.len(), String::from_utf8_lossy(value));
            }
            println!();

            count += 1;
        }
        iter.next();
    }
}
