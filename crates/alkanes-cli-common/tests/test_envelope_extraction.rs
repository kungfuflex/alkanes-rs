use alkanes_support::witness::find_witness_payload;
use bitcoin::consensus::Decodable;
use bitcoin::Transaction;
use std::io::Cursor;
use std::process::Command;

#[test]
fn test_envelope_extraction_from_transaction() {
    let txid = "d0b8447f0e1efe17fd9a85485287728bb35b80e5e55e3d0eabe25246293bbe5c";
    
    println!("Testing envelope extraction from transaction: {}", txid);
    
    // Get raw transaction hex using alkanes-cli
    let output = Command::new("../../target/release/alkanes-cli")
        .args(&["-p", "regtest", "bitcoind", "getrawtransaction", txid])
        .output()
        .expect("Failed to execute alkanes-cli");
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    
    // Parse JSON to extract hex field
    let json_line = stdout.lines()
        .find(|line| line.trim().starts_with('{'))
        .expect("Could not find JSON output");
    
    let parsed: serde_json::Value = serde_json::from_str(json_line)
        .expect("Failed to parse JSON");
    
    let raw_hex = parsed.get("hex")
        .and_then(|v| v.as_str())
        .expect("Could not find hex field in JSON");
    
    println!("Raw hex length: {} chars", raw_hex.len());
    
    // Decode transaction
    let tx_bytes = hex::decode(raw_hex).expect("Failed to decode hex");
    let mut cursor = Cursor::new(&tx_bytes);
    let tx = Transaction::consensus_decode(&mut cursor).expect("Failed to decode transaction");
    
    println!("Transaction details:");
    println!("  Inputs: {}", tx.input.len());
    println!("  Outputs: {}", tx.output.len());
    
    assert!(!tx.input.is_empty(), "Transaction has no inputs");
    
    let witness = &tx.input[0].witness;
    println!("  Witness items: {}", witness.len());
    
    assert!(witness.len() >= 2, "Not enough witness items (expected at least 2, got {})", witness.len());
    
    println!("\nWitness item 1 (script):");
    println!("  Length: {} bytes", witness[1].len());
    println!("  First 40 bytes (hex): {}", hex::encode(&witness[1][..40.min(witness[1].len())]));
    
    // Extract payload using find_witness_payload - it takes the transaction and input index
    match find_witness_payload(&tx, 0) {
        Some(payload) => {
            println!("\n✅ Envelope found in witness!");
            println!("  Compressed payload length: {} bytes", payload.len());
            
            // Check if it's gzip compressed
            let is_gzipped = payload.len() >= 2 && payload[0] == 0x1f && payload[1] == 0x8b;
            println!("  Is gzip compressed: {}", is_gzipped);
            
            let decompressed = if is_gzipped {
                println!("  Decompressing gzip payload...");
                use flate2::read::GzDecoder;
                use std::io::Read;
                
                let mut decoder = GzDecoder::new(&payload[..]);
                let mut decompressed_data = Vec::new();
                decoder.read_to_end(&mut decompressed_data)
                    .expect("Failed to decompress");
                println!("  Decompressed to {} bytes", decompressed_data.len());
                decompressed_data
            } else {
                payload.clone()
            };
            
            // Check WASM magic bytes
            if decompressed.len() >= 4 {
                let magic = &decompressed[..4];
                if magic == b"\x00asm" {
                    println!("  ✅ Payload contains valid WASM module!");
                    if decompressed.len() >= 8 {
                        let version = &decompressed[4..8];
                        println!("  WASM version: {:02x?}", version);
                    }
                    
                    // Success!
                    assert_eq!(magic, b"\x00asm", "WASM magic bytes mismatch");
                } else {
                    panic!("Decompressed payload doesn't start with WASM magic bytes: {:02x?}", magic);
                }
            } else {
                panic!("Decompressed payload too small: {} bytes", decompressed.len());
            }
        }
        None => {
            panic!("Failed to find witness payload in transaction");
        }
    }
    
    println!("\n✅ Test passed! Envelope extraction working correctly.");
}
