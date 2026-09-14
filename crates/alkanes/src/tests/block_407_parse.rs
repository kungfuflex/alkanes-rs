//! Test parsing of Zcash block 407 to identify where parsing breaks

#[cfg(test)]
mod tests {
    use bitcoin::Block;
    use std::io::Cursor;
    use crate::zcash::ZcashBlock;
    use crate::index_block;
    use wasm_bindgen_test::wasm_bindgen_test;
    #[allow(unused_imports)]
    use metashrew_core::{
        println,
        stdio::{stdout, Write},
    };

    const BLOCK_407_HEX: &str = include_str!("blocks/zec_407.hex");

    #[wasm_bindgen_test]
    fn test_parse_block_407_raw() {
        println!("\n=== Testing Block 407 Parsing ===");
        println!("Block hex length: {} chars", BLOCK_407_HEX.trim().len());
        
        // Decode hex
        let block_bytes = match hex::decode(BLOCK_407_HEX.trim()) {
            Ok(bytes) => {
                println!("✓ Successfully decoded hex to {} bytes", bytes.len());
                bytes
            }
            Err(e) => {
                println!("✗ Failed to decode hex: {:?}", e);
                panic!("Hex decode failed");
            }
        };

        println!("\n--- Attempting ZcashBlock::parse ---");
        let mut cursor = Cursor::new(block_bytes.clone());
        match ZcashBlock::parse(&mut cursor) {
            Ok(zblock) => {
                println!("✓ Successfully parsed ZcashBlock!");
                println!("  Version: {}", zblock.version);
                println!("  Previous block hash: {}", zblock.block.header.prev_blockhash);
                println!("  Merkle root: {}", zblock.block.header.merkle_root);
                println!("  Time: {}", zblock.block.header.time);
                println!("  Bits: {}", zblock.block.header.bits.to_consensus());
                println!("  Nonce: {}", zblock.block.header.nonce);
                println!("  Transaction count: {}", zblock.block.txdata.len());
                
                if !zblock.block.txdata.is_empty() {
                    println!("\n--- Coinbase Transaction ---");
                    let coinbase = &zblock.block.txdata[0];
                    println!("  Txid: {}", coinbase.compute_txid());
                    println!("  Inputs: {}", coinbase.input.len());
                    println!("  Outputs: {}", coinbase.output.len());
                    
                    if !coinbase.output.is_empty() {
                        println!("\n  First output:");
                        println!("    Value: {} satoshis", coinbase.output[0].value);
                        println!("    Script length: {} bytes", coinbase.output[0].script_pubkey.len());
                        println!("    Script (hex): {}", hex::encode(coinbase.output[0].script_pubkey.as_bytes()));
                    }
                }
                
                println!("\n--- Cursor Position After Parse ---");
                println!("  Position: {} / {} bytes", cursor.position(), block_bytes.len());
                println!("  Remaining: {} bytes", block_bytes.len() - cursor.position() as usize);
            }
            Err(e) => {
                println!("✗ Failed to parse ZcashBlock: {:?}", e);
                println!("\n--- Cursor Position at Failure ---");
                println!("  Position: {} / {} bytes", cursor.position(), block_bytes.len());
                
                // Show some bytes around the failure point
                let pos = cursor.position() as usize;
                let start = pos.saturating_sub(32);
                let end = (pos + 32).min(block_bytes.len());
                println!("\n--- Bytes around failure point ---");
                println!("  Context: bytes {} to {}", start, end);
                println!("  Hex: {}", hex::encode(&block_bytes[start..end]));
                
                panic!("ZcashBlock parsing failed: {:?}", e);
            }
        }
    }

    #[wasm_bindgen_test]
    fn test_parse_block_407_with_indexing() {
        use crate::tests::helpers as alkane_helpers;
        alkane_helpers::clear(); // Initialize network params
        
        println!("\n=== Testing Block 407 with Full Indexing ===");
        
        let block_bytes = hex::decode(BLOCK_407_HEX.trim())
            .expect("Failed to decode hex");
        
        println!("Block size: {} bytes", block_bytes.len());
        
        // Parse block
        let mut cursor = Cursor::new(block_bytes.clone());
        let zblock = ZcashBlock::parse(&mut cursor)
            .expect("Failed to parse ZcashBlock");
        
        let block: Block = zblock.into();
        
        println!("✓ Block parsed successfully");
        println!("  Transactions: {}", block.txdata.len());
        
        // Now try to index the block
        println!("\n--- Attempting to index block 407 ---");
        match index_block(&block, 407) {
            Ok(_) => {
                println!("✓ Successfully indexed block 407!");
            }
            Err(e) => {
                println!("✗ Failed to index block 407: {:?}", e);
                
                // Try to get more details about which transaction failed
                println!("\n--- Transaction Details ---");
                for (i, tx) in block.txdata.iter().enumerate() {
                    println!("  Tx {}: {} ({} inputs, {} outputs)", 
                        i, 
                        tx.compute_txid(),
                        tx.input.len(),
                        tx.output.len()
                    );
                }
                
                panic!("Indexing failed: {:?}", e);
            }
        }
    }

    #[wasm_bindgen_test]
    fn test_parse_block_407_transactions_detail() {
        println!("\n=== Detailed Transaction Analysis for Block 407 ===");
        
        let block_bytes = hex::decode(BLOCK_407_HEX.trim())
            .expect("Failed to decode hex");
        
        let mut cursor = Cursor::new(block_bytes);
        let zblock = ZcashBlock::parse(&mut cursor)
            .expect("Failed to parse ZcashBlock");
        
        let block: Block = zblock.into();
        
        println!("Total transactions: {}", block.txdata.len());
        
        for (i, tx) in block.txdata.iter().enumerate() {
            println!("\n--- Transaction {} ---", i);
            println!("  Txid: {}", tx.compute_txid());
            println!("  Version: {}", tx.version.0);
            println!("  Lock time: {}", tx.lock_time);
            println!("  Inputs: {}", tx.input.len());
            
            for (j, input) in tx.input.iter().enumerate() {
                println!("    Input {}:", j);
                println!("      Previous output: {}:{}", input.previous_output.txid, input.previous_output.vout);
                println!("      Script sig length: {}", input.script_sig.len());
                println!("      Sequence: {}", input.sequence);
            }
            
            println!("  Outputs: {}", tx.output.len());
            for (j, output) in tx.output.iter().enumerate() {
                println!("    Output {}:", j);
                println!("      Value: {} satoshis", output.value);
                println!("      Script pubkey length: {}", output.script_pubkey.len());
                println!("      Script type: {}", 
                    if output.script_pubkey.is_p2pkh() { "P2PKH" }
                    else if output.script_pubkey.is_p2sh() { "P2SH" }
                    else if output.script_pubkey.is_op_return() { "OP_RETURN" }
                    else { "OTHER" }
                );
                
                // Check if it's a t-address
                use crate::zcash::is_t_address;
                if is_t_address(&output.script_pubkey) {
                    println!("      ✓ Is t-address");
                } else {
                    println!("      ✗ Not a t-address");
                }
            }
        }
    }
}
