use std::io::{Cursor, Read};

const BLOCK_407_HEX: &str = include_str!("../src/tests/blocks/zec_407.hex");

fn consume_varint(cursor: &mut Cursor<Vec<u8>>) -> std::io::Result<u64> {
    let mut buf = [0u8; 1];
    cursor.read_exact(&mut buf)?;
    
    match buf[0] {
        0xFF => {
            let mut buf = [0u8; 8];
            cursor.read_exact(&mut buf)?;
            Ok(u64::from_le_bytes(buf))
        }
        0xFE => {
            let mut buf = [0u8; 4];
            cursor.read_exact(&mut buf)?;
            Ok(u32::from_le_bytes(buf) as u64)
        }
        0xFD => {
            let mut buf = [0u8; 2];
            cursor.read_exact(&mut buf)?;
            Ok(u16::from_le_bytes(buf) as u64)
        }
        n => Ok(n as u64),
    }
}

fn main() {
    println!("\n=== Analyzing Block 407 Structure ===");
    
    let block_bytes = hex::decode(BLOCK_407_HEX.trim()).expect("Failed to decode hex");
    println!("Total block size: {} bytes", block_bytes.len());
    
    let mut cursor = Cursor::new(block_bytes.clone());
    
    // Read block header fields manually
    println!("\n--- Block Header ---");
    
    // Version (4 bytes)
    let mut version_bytes = [0u8; 4];
    cursor.read_exact(&mut version_bytes).unwrap();
    let version = i32::from_le_bytes(version_bytes);
    println!("Version: {} (pos: 0-4)", version);
    
    // Previous block hash (32 bytes)
    let mut prev_hash = [0u8; 32];
    cursor.read_exact(&mut prev_hash).unwrap();
    println!("Prev block hash: {} (pos: 4-36)", hex::encode(&prev_hash));
    
    // Merkle root (32 bytes)
    let mut merkle_root = [0u8; 32];
    cursor.read_exact(&mut merkle_root).unwrap();
    println!("Merkle root: {} (pos: 36-68)", hex::encode(&merkle_root));
    
    // Reserved/final sapling root (32 bytes)
    let mut reserved = [0u8; 32];
    cursor.read_exact(&mut reserved).unwrap();
    println!("Reserved: {} (pos: 68-100)", hex::encode(&reserved));
    
    // Time (4 bytes)
    let mut time_bytes = [0u8; 4];
    cursor.read_exact(&mut time_bytes).unwrap();
    let time = u32::from_le_bytes(time_bytes);
    println!("Time: {} (pos: 100-104)", time);
    
    // nBits (4 bytes)
    let mut bits_bytes = [0u8; 4];
    cursor.read_exact(&mut bits_bytes).unwrap();
    let bits = u32::from_le_bytes(bits_bytes);
    println!("nBits: {} (pos: 104-108)", bits);
    
    // Nonce (32 bytes)
    let mut nonce = [0u8; 32];
    cursor.read_exact(&mut nonce).unwrap();
    println!("Nonce: {} (pos: 108-140)", hex::encode(&nonce));
    
    let pos_before_solution = cursor.position();
    
    // Solution size (varint)
    let solution_size = consume_varint(&mut cursor).unwrap();
    let pos_after_solution_size = cursor.position();
    println!("\nSolution size: {} bytes (pos: {}-{})", 
        solution_size, pos_before_solution, pos_after_solution_size);
    
    // Solution
    let mut solution = vec![0u8; solution_size as usize];
    cursor.read_exact(&mut solution).unwrap();
    let pos_after_solution = cursor.position();
    println!("Solution data read (pos: {}-{})", pos_after_solution_size, pos_after_solution);
    
    println!("\n--- Transactions ---");
    
    // Transaction count
    let tx_count = consume_varint(&mut cursor).unwrap();
    let pos_after_tx_count = cursor.position();
    println!("Transaction count: {} (pos: {})", tx_count, pos_after_tx_count);
    
    // Now try to parse each transaction manually to see where it fails
    for i in 0..tx_count {
        println!("\n--- Transaction {} ---", i);
        let tx_start_pos = cursor.position();
        println!("Start position: {}", tx_start_pos);
        
        // Version (4 bytes)
        let mut tx_version = [0u8; 4];
        match cursor.read_exact(&mut tx_version) {
            Ok(_) => {
                let version = i32::from_le_bytes(tx_version);
                println!("  Version: {}", version);
                
                // Zcash transaction version 2+ has extra fields
                // Check for overwinter/sapling version group
                if version >= 3 || version < 0 {  // Negative version indicates overwinter
                    println!("  ⚠ Zcash transaction version {} detected!", version);
                    println!("  This is likely a Zcash-specific transaction format");
                    println!("  Position: {}", cursor.position());
                    
                    // Show next bytes
                    let pos = cursor.position() as usize;
                    let preview_len = 64.min(block_bytes.len() - pos);
                    println!("  Next {} bytes: {}", preview_len, hex::encode(&block_bytes[pos..pos+preview_len]));
                }
            }
            Err(e) => {
                println!("  ✗ Failed to read version: {:?}", e);
                println!("  Position: {} / {}", cursor.position(), block_bytes.len());
                break;
            }
        }
        
        // Input count
        match consume_varint(&mut cursor) {
            Ok(input_count) => {
                println!("  Inputs: {}", input_count);
                
                // Try to parse inputs
                for j in 0..input_count {
                    let input_start = cursor.position();
                    println!("    Input {}: pos {}", j, input_start);
                    
                    // Previous output (36 bytes: 32 byte hash + 4 byte index)
                    let mut prev_out = [0u8; 36];
                    match cursor.read_exact(&mut prev_out) {
                        Ok(_) => {
                            println!("      Previous output: OK");
                        }
                        Err(e) => {
                            println!("      ✗ Failed to read previous output: {:?}", e);
                            println!("      Position: {} / {}", cursor.position(), block_bytes.len());
                            return;
                        }
                    }
                    
                    // Script length
                    match consume_varint(&mut cursor) {
                        Ok(script_len) => {
                            println!("      Script length: {}", script_len);
                            
                            // Script
                            let mut script = vec![0u8; script_len as usize];
                            match cursor.read_exact(&mut script) {
                                Ok(_) => println!("      Script: OK"),
                                Err(e) => {
                                    println!("      ✗ Failed to read script: {:?}", e);
                                    println!("      Position: {} / {}", cursor.position(), block_bytes.len());
                                    return;
                                }
                            }
                        }
                        Err(e) => {
                            println!("      ✗ Failed to read script length: {:?}", e);
                            println!("      Position: {} / {}", cursor.position(), block_bytes.len());
                            return;
                        }
                    }
                    
                    // Sequence (4 bytes)
                    let mut sequence = [0u8; 4];
                    match cursor.read_exact(&mut sequence) {
                        Ok(_) => println!("      Sequence: OK"),
                        Err(e) => {
                            println!("      ✗ Failed to read sequence: {:?}", e);
                            println!("      Position: {} / {}", cursor.position(), block_bytes.len());
                            return;
                        }
                    }
                }
            }
            Err(e) => {
                println!("  ✗ Failed to read input count: {:?}", e);
                println!("  Position: {} / {}", cursor.position(), block_bytes.len());
                break;
            }
        }
        
        // Output count
        match consume_varint(&mut cursor) {
            Ok(output_count) => {
                println!("  Outputs: {}", output_count);
                
                // Try to parse outputs
                for j in 0..output_count {
                    let output_start = cursor.position();
                    println!("    Output {}: pos {}", j, output_start);
                    
                    // Value (8 bytes)
                    let mut value = [0u8; 8];
                    match cursor.read_exact(&mut value) {
                        Ok(_) => {
                            let val = i64::from_le_bytes(value);
                            println!("      Value: {} satoshis", val);
                        }
                        Err(e) => {
                            println!("      ✗ Failed to read value: {:?}", e);
                            println!("      Position: {} / {}", cursor.position(), block_bytes.len());
                            return;
                        }
                    }
                    
                    // Script length
                    match consume_varint(&mut cursor) {
                        Ok(script_len) => {
                            println!("      Script length: {}", script_len);
                            
                            // Script
                            let mut script = vec![0u8; script_len as usize];
                            match cursor.read_exact(&mut script) {
                                Ok(_) => println!("      Script: OK"),
                                Err(e) => {
                                    println!("      ✗ Failed to read script: {:?}", e);
                                    println!("      Position: {} / {}", cursor.position(), block_bytes.len());
                                    return;
                                }
                            }
                        }
                        Err(e) => {
                            println!("      ✗ Failed to read script length: {:?}", e);
                            println!("      Position: {} / {}", cursor.position(), block_bytes.len());
                            return;
                        }
                    }
                }
            }
            Err(e) => {
                println!("  ✗ Failed to read output count: {:?}", e);
                println!("  Position: {} / {}", cursor.position(), block_bytes.len());
                break;
            }
        }
        
        // Lock time (4 bytes)
        let mut lock_time = [0u8; 4];
        match cursor.read_exact(&mut lock_time) {
            Ok(_) => {
                let lt = u32::from_le_bytes(lock_time);
                println!("  Lock time: {}", lt);
                println!("  Transaction end position: {}", cursor.position());
            }
            Err(e) => {
                println!("  ✗ Failed to read lock time: {:?}", e);
                println!("  Position: {} / {}", cursor.position(), block_bytes.len());
                
                // This might be where Zcash-specific fields start
                println!("\n  ⚠ IMPORTANT: Zcash transactions v2+ have additional fields:");
                println!("    - nExpiryHeight (4 bytes) for Overwinter+");
                println!("    - valueBalance (8 bytes) for Sapling+");
                println!("    - nShieldedSpend count + data");
                println!("    - nShieldedOutput count + data");
                println!("    - joinSplitData (for Sprout)");
                println!("    - bindingSig (for Sapling+)");
                break;
            }
        }
    }
    
    println!("\n--- Final Position ---");
    println!("Cursor: {} / {} bytes", cursor.position(), block_bytes.len());
    println!("Remaining: {} bytes", block_bytes.len() - cursor.position() as usize);
}
