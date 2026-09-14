//! Unit test to reproduce the block 349330 parsing error in _start()
//!
//! This test mimics exactly what happens in the WASM _start() function
//! to reproduce the parsing error that occurs at runtime.

#[cfg(test)]
mod tests {
    use crate::indexer::index_block;
    use crate::tests::helpers as alkane_helpers;
    use alkanes_support::zcash::ZcashBlock;
    use alkanes_support::block_traits::BlockLike;
    use std::io::Cursor;
    use wasm_bindgen_test::wasm_bindgen_test;

    const BLOCK_349330_HEX: &str = include_str!("blocks/zec_349330.hex");

    /// Test that mimics the exact flow in _start() function
    #[wasm_bindgen_test]
    fn test_block_349330_as_start_function() {
        println!("\n=== Reproducing _start() Flow for Block 349330 ===");
        
        // Decode the block from hex (simulating what we get from RPC)
        let block_bytes = hex::decode(BLOCK_349330_HEX.trim())
            .expect("Failed to decode hex");
        
        println!("Block size from hex: {} bytes", block_bytes.len());
        
        // Simulate what _start() receives:
        // input() returns: [4 bytes height] + [block data]
        let height = 349330u32;
        let mut data = Vec::new();
        data.extend_from_slice(&height.to_le_bytes());
        data.extend_from_slice(&block_bytes);
        
        println!("Total data size (as _start receives): {} bytes", data.len());
        println!("  Height bytes: 4");
        println!("  Block bytes: {}", block_bytes.len());
        
        // Extract height (as _start does)
        let extracted_height = u32::from_le_bytes((&data[0..4]).try_into().unwrap());
        println!("Extracted height: {}", extracted_height);
        assert_eq!(extracted_height, 349330);
        
        // Get the reader slice (as _start does)
        let reader = &data[4..];
        println!("Reader slice length: {}", reader.len());
        println!("First 100 bytes of reader: {}", hex::encode(&reader[..100.min(reader.len())]));
        
        // Parse exactly as _start() does
        println!("\n=== Parsing with ZcashBlock (as _start does) ===");
        let block_data = reader.to_vec();  // This is what _start does
        println!("block_data.len() = {}", block_data.len());
        
        let parse_result = ZcashBlock::parse(&mut Cursor::<Vec<u8>>::new(block_data.clone()));
        
        match parse_result {
            Ok(zblock) => {
                println!("✓ Successfully parsed ZcashBlock");
                println!("  Block hash: {:?}", zblock.block_hash());
                println!("  Transactions: {}", zblock.transactions().len());
                
                // Now try indexing (this is what _start does)
                println!("\n=== Attempting to index block ===");
                alkane_helpers::clear();
                
                // index_block is generic over BlockLike, so we can pass ZcashBlock directly
                match index_block(&zblock, extracted_height) {
                    Ok(_) => {
                        println!("✓ Successfully indexed block 349330");
                    }
                    Err(e) => {
                        println!("✗ Indexing failed: {:?}", e);
                        panic!("index_block failed: {:?}", e);
                    }
                }
            }
            Err(e) => {
                println!("✗ ZcashBlock parsing failed: {:?}", e);
                println!("\n=== Debugging Info ===");
                println!("Block data length: {}", block_data.len());
                println!("Block data (full hex): {}", hex::encode(&block_data));
                
                // Compare with known working parse
                println!("\n=== Trying alternative parse method ===");
                let alt_result = ZcashBlock::parse(&mut Cursor::new(block_bytes.clone()));
                match alt_result {
                    Ok(alt_zblock) => {
                        println!("✓ Alternative parse succeeded!");
                        println!("  This means the issue is with reader.to_vec()");
                        
                        // Check if data is identical
                        if block_data == block_bytes {
                            println!("  Data is IDENTICAL - parsing should work!");
                        } else {
                            println!("  Data is DIFFERENT!");
                            println!("    Original length: {}", block_bytes.len());
                            println!("    reader.to_vec() length: {}", block_data.len());
                            
                            // Find the difference
                            for (i, (a, b)) in block_bytes.iter().zip(block_data.iter()).enumerate() {
                                if a != b {
                                    println!("    First diff at byte {}: {} vs {}", i, a, b);
                                    break;
                                }
                            }
                        }
                    }
                    Err(alt_e) => {
                        println!("✗ Alternative parse also failed: {:?}", alt_e);
                        println!("  This means the block data itself has an issue");
                    }
                }
                
                panic!("ZcashBlock parsing failed (reproducing _start error): {:?}", e);
            }
        }
    }

    /// Test parsing with direct bytes (no height prefix)
    #[wasm_bindgen_test]
    fn test_block_349330_direct_parse() {
        println!("\n=== Direct Parse Test (No Height Prefix) ===");
        
        let block_bytes = hex::decode(BLOCK_349330_HEX.trim())
            .expect("Failed to decode hex");
        
        println!("Block size: {} bytes", block_bytes.len());
        
        let result = ZcashBlock::parse(&mut Cursor::new(block_bytes.clone()));
        
        match result {
            Ok(zblock) => {
                println!("✓ Direct parse successful");
                println!("  Block hash: {:?}", zblock.block_hash());
            }
            Err(e) => {
                println!("✗ Direct parse failed: {:?}", e);
                panic!("Direct parse should work but failed: {:?}", e);
            }
        }
    }

    /// Test with to_vec() conversion (isolating the potential issue)
    #[wasm_bindgen_test]
    fn test_block_349330_to_vec_conversion() {
        println!("\n=== Testing to_vec() Conversion ===");
        
        let block_bytes = hex::decode(BLOCK_349330_HEX.trim())
            .expect("Failed to decode hex");
        
        // Create a Vec, then create a slice, then to_vec() it (as _start does)
        let mut full_data = Vec::new();
        full_data.extend_from_slice(&349330u32.to_le_bytes());
        full_data.extend_from_slice(&block_bytes);
        
        let reader = &full_data[4..];
        let block_data = reader.to_vec();  // This is the conversion in _start
        
        println!("Original block_bytes length: {}", block_bytes.len());
        println!("After to_vec() length: {}", block_data.len());
        
        assert_eq!(block_bytes.len(), block_data.len(), "Length should be the same");
        assert_eq!(block_bytes, block_data, "Data should be identical");
        
        println!("✓ Data is identical after to_vec()");
        
        // Now parse it
        let result = ZcashBlock::parse(&mut Cursor::new(block_data));
        
        match result {
            Ok(zblock) => {
                println!("✓ Parse after to_vec() successful");
                println!("  Block hash: {:?}", zblock.block_hash());
            }
            Err(e) => {
                println!("✗ Parse after to_vec() failed: {:?}", e);
                panic!("Parse failed after to_vec(): {:?}", e);
            }
        }
    }

    /// Test indexing with the block
    #[wasm_bindgen_test]
    fn test_block_349330_full_indexing_flow() {
        alkane_helpers::clear();
        
        println!("\n=== Full Indexing Flow Test ===");
        
        let block_bytes = hex::decode(BLOCK_349330_HEX.trim())
            .expect("Failed to decode hex");
        
        let zblock = ZcashBlock::parse(&mut Cursor::new(block_bytes))
            .expect("Parse should succeed");
        
        println!("Attempting to index block 349330...");
        
        // index_block is generic over BlockLike trait
        match index_block(&zblock, 349330) {
            Ok(_) => {
                println!("✓ Indexing successful");
            }
            Err(e) => {
                println!("✗ Indexing failed: {:?}", e);
                
                // Try to understand what went wrong
                println!("\n=== Block Details ===");
                println!("Transactions: {}", zblock.transactions().len());
                for (i, tx) in zblock.transactions().iter().enumerate() {
                    println!("  TX {}: {} inputs, {} outputs", i, tx.inputs.len(), tx.outputs.len());
                }
                
                panic!("index_block failed: {:?}", e);
            }
        }
    }

    /// Test to check if there's a newline or extra character
    #[wasm_bindgen_test]
    fn test_block_349330_hex_file_format() {
        println!("\n=== Hex File Format Test ===");
        
        let raw_hex = BLOCK_349330_HEX;
        println!("Raw hex string length: {}", raw_hex.len());
        println!("Trimmed hex string length: {}", raw_hex.trim().len());
        
        let has_newline = raw_hex.contains('\n');
        let has_carriage_return = raw_hex.contains('\r');
        
        println!("Has newline: {}", has_newline);
        println!("Has carriage return: {}", has_carriage_return);
        
        if has_newline || has_carriage_return {
            println!("⚠ Hex string contains whitespace characters");
        }
        
        let decoded = hex::decode(raw_hex.trim())
            .expect("Should decode");
        
        println!("Decoded block size: {} bytes", decoded.len());
        println!("Expected: 1624 bytes");
        
        assert_eq!(decoded.len(), 1624, "Block should be exactly 1624 bytes");
    }
}
