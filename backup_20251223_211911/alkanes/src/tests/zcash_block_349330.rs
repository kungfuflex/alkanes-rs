//! Test for Zcash block 349330 that causes memory fault
//! 
//! This block triggers the error:
//! ```
//! [ERROR] WASM _start function failed: error while executing at wasm backtrace:
//!         0: 0x1b319f - alkanes.wasm!metashrew_core::input::h11aff8e94162f345
//!         1: 0x4487f - alkanes.wasm!_start
//!     Caused by:
//!         0: memory fault at wasm address 0xff897608 in linear memory of size 0xe10000
//!         1: wasm trap: out of bounds memory access
//! ```
//!
//! The error occurs in metashrew_core::input which suggests the block data itself
//! may be causing an out-of-bounds read when being parsed or processed.

#[cfg(test)]
mod tests {
    use crate::indexer::index_block;
    use crate::tests::helpers as alkane_helpers;
    use alkanes_support::zcash::ZcashBlock;
    use alkanes_support::block_traits::{BlockLike, TransactionLike};
    use metashrew_core::{println, stdio::{stdout, Write}};
    use metashrew_support::index_pointer::KeyValuePointer;
    use protorune::tables::{OUTPOINT_SPENDABLE_BY, RUNES};
    use std::io::Cursor;
    use wasm_bindgen_test::wasm_bindgen_test;

    const BLOCK_349330_HEX: &str = include_str!("blocks/zec_349330.hex");

    #[wasm_bindgen_test]
    fn test_zcash_block_349330_parsing() {
        println!("\n=== Testing Block 349330 Parsing (Memory Fault Block) ===");
        
        // Decode the block from hex
        let block_bytes = hex::decode(BLOCK_349330_HEX.trim())
            .expect("Failed to decode hex");
        
        println!("Block size: {} bytes", block_bytes.len());
        println!("Block hex (first 200 chars): {}", &BLOCK_349330_HEX[..200.min(BLOCK_349330_HEX.len())]);
        
        // Parse with ZcashBlock
        let mut cursor = Cursor::new(block_bytes.clone());
        let zblock = match ZcashBlock::parse(&mut cursor) {
            Ok(zb) => {
                println!("✓ Successfully parsed ZcashBlock");
                zb
            }
            Err(e) => {
                println!("✗ Failed to parse ZcashBlock: {:?}", e);
                println!("Cursor position: {} / {}", cursor.position(), block_bytes.len());
                
                // Print context around failure point
                let pos = cursor.position() as usize;
                if pos > 0 {
                    let start = pos.saturating_sub(50);
                    let end = (pos + 50).min(block_bytes.len());
                    println!("Context (hex): {}", hex::encode(&block_bytes[start..end]));
                }
                
                panic!("ZcashBlock parsing failed: {:?}", e);
            }
        };
        
        println!("Block hash: {:?}", zblock.block_hash());
        println!("Number of transactions: {}", zblock.transactions().len());
        println!("Block parsed bytes: {} / {}", cursor.position(), block_bytes.len());
        
        // Detailed transaction analysis
        println!("\n=== Transaction Analysis ===");
        for (i, tx) in zblock.transactions().iter().enumerate().take(10) {
            println!("TX {}: {} inputs, {} outputs", i, tx.inputs().len(), tx.outputs().len());
            
            // Check for any unusual transaction properties
            if tx.inputs().is_empty() && !tx.is_coinbase() {
                println!("  ⚠ WARNING: Non-coinbase transaction with no inputs!");
            }
            
            // Check output scripts
            for (vout, output) in tx.outputs().iter().enumerate().take(3) {
                let script_len = output.script_pubkey.len();
                println!("    Output {}: {} sats, script len: {}", vout, output.value, script_len);
                
                // Check for unusually large scripts
                if script_len > 10000 {
                    println!("      ⚠ WARNING: Unusually large script ({} bytes)", script_len);
                    println!("      Script (first 100 bytes): {}", 
                        hex::encode(&output.script_pubkey.as_bytes()[..100.min(script_len)]));
                }
            }
        }
        
        if zblock.transactions().len() > 10 {
            println!("... ({} more transactions)", zblock.transactions().len() - 10);
        }
    }

    #[wasm_bindgen_test]
    fn test_zcash_block_349330_incremental_parsing() {
        println!("\n=== Testing Block 349330 Incremental Parsing ===");
        
        // This test will parse the block piece by piece to identify exactly where the issue occurs
        let block_bytes = hex::decode(BLOCK_349330_HEX.trim())
            .expect("Failed to decode hex");
        
        let mut cursor = Cursor::new(block_bytes.clone());
        
        println!("Step 1: Parsing block header...");
        // The ZcashBlock parser will handle this internally, but we can test manually
        // For now, let's just verify we can read the header size
        use std::io::Read;
        let mut header_bytes = vec![0u8; 80]; // Standard Bitcoin/Zcash header is 80 bytes
        cursor.read_exact(&mut header_bytes).expect("Failed to read header");
        println!("  ✓ Header read successfully (80 bytes)");
        println!("  Header hex: {}", hex::encode(&header_bytes));
        
        // Reset cursor for full parse
        cursor.set_position(0);
        
        println!("\nStep 2: Full block parse...");
        match ZcashBlock::parse(&mut cursor) {
            Ok(zblock) => {
                println!("  ✓ Block parsed successfully");
                println!("  Block hash: {:?}", zblock.block_hash());
                println!("  Transactions: {}", zblock.transactions().len());
                
                // Calculate expected vs actual block size
                let parsed_size = cursor.position();
                let total_size = block_bytes.len() as u64;
                println!("  Parsed: {} bytes", parsed_size);
                println!("  Total: {} bytes", total_size);
                
                if parsed_size < total_size {
                    println!("  ⚠ WARNING: {} bytes left unparsed", total_size - parsed_size);
                }
            }
            Err(e) => {
                println!("  ✗ Parsing failed: {:?}", e);
                println!("  Failed at position: {} / {}", cursor.position(), block_bytes.len());
            }
        }
    }

    #[wasm_bindgen_test]
    #[should_panic(expected = "memory fault")]
    fn test_zcash_block_349330_indexing_reproduces_error() {
        alkane_helpers::clear();
        
        println!("\n=== Testing Block 349330 Indexing (Should Reproduce Memory Fault) ===");
        
        // Decode the block from hex
        let block_bytes = hex::decode(BLOCK_349330_HEX.trim())
            .expect("Failed to decode hex");
        let mut cursor = Cursor::new(block_bytes.clone());
        let zblock = ZcashBlock::parse(&mut cursor)
            .expect("Failed to parse ZcashBlock");

        println!("Block hash: {:?}", zblock.block_hash());
        println!("Number of transactions: {}", zblock.transactions().len());
        println!("Block size: {} bytes", block_bytes.len());

        println!("\n⚠ Attempting to index block - this should trigger the memory fault...");
        
        // Index the block - this is where the memory fault should occur
        match index_block(&zblock, 349330) {
            Ok(_) => {
                println!("✓ Block indexed successfully (unexpected - should have failed!)");
            }
            Err(e) => {
                println!("✗ Indexing failed: {:?}", e);
                // Re-panic to satisfy should_panic
                panic!("memory fault during indexing: {:?}", e);
            }
        }
    }

    #[wasm_bindgen_test]
    fn test_zcash_block_349330_transaction_details() {
        println!("\n=== Testing Block 349330 Transaction Details ===");
        
        let block_bytes = hex::decode(BLOCK_349330_HEX.trim())
            .expect("Failed to decode hex");
        let mut cursor = Cursor::new(block_bytes.clone());
        let zblock = ZcashBlock::parse(&mut cursor)
            .expect("Failed to parse ZcashBlock");

        println!("=== Detailed Transaction Analysis ===");
        println!("Total transactions: {}", zblock.transactions().len());
        
        for (i, tx) in zblock.transactions().iter().enumerate() {
            println!("\n--- Transaction {} ---", i);
            println!("Txid: {}", tx.txid());
            println!("Is coinbase: {}", tx.is_coinbase());
            println!("Inputs: {}", tx.inputs().len());
            println!("Outputs: {}", tx.outputs().len());
            
            // Analyze inputs
            if !tx.is_coinbase() {
                for (vin, input) in tx.inputs().iter().enumerate().take(3) {
                    println!("  Input {}:", vin);
                    println!("    Previous output: {}:{}", input.previous_output.txid, input.previous_output.vout);
                    println!("    ScriptSig len: {}", input.script_sig.len());
                    println!("    Sequence: {}", input.sequence);
                }
                if tx.inputs().len() > 3 {
                    println!("  ... ({} more inputs)", tx.inputs().len() - 3);
                }
            }
            
            // Analyze outputs in detail
            for (vout, output) in tx.outputs().iter().enumerate() {
                println!("  Output {}:", vout);
                println!("    Value: {} sats", output.value);
                println!("    Script len: {}", output.script_pubkey.len());
                
                let is_op_return = output.script_pubkey.is_op_return();
                let is_p2pkh = output.script_pubkey.is_p2pkh();
                let is_p2sh = output.script_pubkey.is_p2sh();
                
                println!("    OP_RETURN: {}, P2PKH: {}, P2SH: {}", is_op_return, is_p2pkh, is_p2sh);
                
                // Check for very large scripts that might cause memory issues
                if output.script_pubkey.len() > 10000 {
                    println!("    ⚠ LARGE SCRIPT DETECTED: {} bytes", output.script_pubkey.len());
                    println!("    This could be causing memory issues!");
                    println!("    Script (first 200 bytes): {}", 
                        hex::encode(&output.script_pubkey.as_bytes()[..200.min(output.script_pubkey.len())]));
                }
            }
            
            // Limit output for readability
            if i >= 20 {
                println!("\n... (showing first 20 transactions, {} more exist)", zblock.transactions().len() - 20);
                break;
            }
        }
    }
}
