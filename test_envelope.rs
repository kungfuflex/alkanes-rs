// Test to verify envelope extraction from witness
use alkanes_support::envelope::AlkanesEnvelope;
use bitcoin::consensus::Decodable;
use bitcoin::Transaction;
use std::io::Cursor;

fn main() {
    let txid = "d0b8447f0e1efe17fd9a85485287728bb35b80e5e55e3d0eabe25246293bbe5c";
    
    // Get raw transaction hex
    let output = std::process::Command::new("./target/release/alkanes-cli")
        .args(&["-p", "regtest", "bitcoind", "getrawtransaction", txid])
        .output()
        .expect("Failed to execute alkanes-cli");
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    
    // Parse JSON response to get hex
    let hex_line = stdout.lines()
        .filter(|line| line.contains("\"hex\""))
        .next()
        .expect("Could not find hex field");
    
    // Extract hex value between quotes
    let hex_start = hex_line.find("\":\"").unwrap() + 3;
    let hex_end = hex_line[hex_start..].find("\"").unwrap();
    let raw_hex = &hex_line[hex_start..hex_start + hex_end];
    
    println!("Transaction ID: {}", txid);
    println!("Raw hex length: {} chars", raw_hex.len());
    
    // Decode transaction
    let tx_bytes = hex::decode(raw_hex).expect("Failed to decode hex");
    let mut cursor = Cursor::new(&tx_bytes);
    let tx = Transaction::consensus_decode(&mut cursor).expect("Failed to decode transaction");
    
    println!("\nTransaction details:");
    println!("  Inputs: {}", tx.input.len());
    println!("  Outputs: {}", tx.output.len());
    
    if tx.input.is_empty() {
        println!("❌ Transaction has no inputs!");
        return;
    }
    
    let witness = &tx.input[0].witness;
    println!("  Witness items in first input: {}", witness.len());
    
    if witness.len() < 2 {
        println!("❌ Not enough witness items (need at least 2)");
        return;
    }
    
    // Witness item 1 should contain the envelope
    let envelope_bytes = &witness[1];
    println!("\nWitness item 1 (envelope):");
    println!("  Length: {} bytes", envelope_bytes.len());
    println!("  First 40 bytes (hex): {}", hex::encode(&envelope_bytes[..40.min(envelope_bytes.len())]));
    
    // Try to parse envelope
    match AlkanesEnvelope::parse_from_witness(envelope_bytes) {
        Ok(envelope) => {
            println!("\n✅ Envelope parsed successfully!");
            println!("  Protocol tag: {:?}", envelope.protocol_tag);
            println!("  Compression: {:?}", envelope.compression);
            println!("  Payload length: {} bytes", envelope.payload.len());
            
            // Check WASM magic bytes
            if envelope.payload.len() >= 4 {
                let magic = &envelope.payload[..4];
                if magic == b"\x00asm" {
                    println!("  ✅ Payload contains WASM module!");
                    println!("  WASM version: {:?}", &envelope.payload[4..8]);
                } else {
                    println!("  ⚠️  Payload doesn't start with WASM magic: {:02x?}", magic);
                }
            }
        }
        Err(e) => {
            println!("\n❌ Failed to parse envelope: {:?}", e);
        }
    }
}
