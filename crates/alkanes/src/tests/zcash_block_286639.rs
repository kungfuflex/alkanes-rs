//! Test for indexing Zcash block 286639
//! This block should index more than 4 k/v pairs and properly index OUTPOINT_SPENDABLE_BY

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

    const BLOCK_286639_HEX: &str = include_str!("blocks/zec_286639.hex");

    #[wasm_bindgen_test]
    fn test_zcash_block_286639_parsing() {
        println!("\n=== Testing Block 286639 Parsing ===");
        
        // Decode the block from hex
        let block_bytes = hex::decode(BLOCK_286639_HEX.trim())
            .expect("Failed to decode hex");
        
        println!("Block size: {} bytes", block_bytes.len());
        
        // Parse with ZcashBlock
        let mut cursor = Cursor::new(block_bytes.clone());
        let zblock = match ZcashBlock::parse(&mut cursor) {
            Ok(zb) => {
                println!("✓ Successfully parsed ZcashBlock");
                zb
            }
            Err(e) => {
                println!("✗ Failed to parse ZcashBlock: {:?}", e);
                panic!("ZcashBlock parsing failed: {:?}", e);
            }
        };
        
        println!("Block hash: {:?}", zblock.block_hash());
        println!("Number of transactions: {}", zblock.transactions().len());
        
        // Print first few transactions
        for (i, tx) in zblock.transactions().iter().enumerate().take(5) {
            println!("TX {}: {} inputs, {} outputs", i, tx.inputs().len(), tx.outputs().len());
        }
    }

    #[wasm_bindgen_test]
    fn test_zcash_block_286639_indexing() {
        alkane_helpers::clear();
        
        println!("\n=== Testing Block 286639 Indexing ===");
        
        // Decode the block from hex
        let block_bytes = hex::decode(BLOCK_286639_HEX.trim())
            .expect("Failed to decode hex");
        let mut cursor = Cursor::new(block_bytes.clone());
        let zblock = ZcashBlock::parse(&mut cursor)
            .expect("Failed to parse ZcashBlock");

        println!("=== Before indexing ===");
        println!("Block hash: {:?}", zblock.block_hash());
        println!("Number of transactions: {}", zblock.transactions().len());

        // Index the block using the generic function - works with Zcash!
        index_block(&zblock, 286639)
            .expect("Failed to index block");

        // Check specific tables
        println!("\n=== Checking OUTPOINT_SPENDABLE_BY entries ===");
        let mut spendable_count = 0;
        let mut valid_outputs = 0;
        
        for (i, tx) in zblock.transactions().iter().enumerate() {
            let txid = tx.txid();
            for (vout, output) in tx.outputs().iter().enumerate() {
                // Count valid outputs (not OP_RETURN)
                if !output.script_pubkey.is_op_return() {
                    let is_p2pkh = output.script_pubkey.is_p2pkh();
                    let is_p2sh = output.script_pubkey.is_p2sh();
                    
                    if is_p2pkh || is_p2sh {
                        valid_outputs += 1;
                    }
                }
                
                let outpoint = bitcoin::OutPoint {
                    txid: txid.clone(),
                    vout: vout as u32,
                };
                let outpoint_bytes = metashrew_support::utils::consensus_encode(&outpoint)
                    .expect("Failed to encode outpoint");
                let spendable_by = OUTPOINT_SPENDABLE_BY.select(&outpoint_bytes).get();
                
                if !spendable_by.is_empty() {
                    spendable_count += 1;
                    if spendable_count <= 5 {
                        let addr = String::from_utf8_lossy(&spendable_by);
                        println!("  TX {} output {} is spendable by: {}", i, vout, addr);
                    }
                }
            }
        }
        println!("Total valid outputs (P2PKH/P2SH): {}", valid_outputs);
        println!("Total OUTPOINT_SPENDABLE_BY entries: {}", spendable_count);

        // Check HEIGHT_TO_BLOCKHASH
        let blockhash = RUNES.HEIGHT_TO_BLOCKHASH.select_value::<u64>(286639).get();
        println!("\n=== HEIGHT_TO_BLOCKHASH ===");
        println!("Stored: {}", !blockhash.is_empty());

        // We expect OUTPOINT_SPENDABLE_BY entries for valid outputs
        println!("\n=== ASSERTION CHECK ===");
        println!("Valid outputs: {}, OUTPOINT_SPENDABLE_BY entries: {}", valid_outputs, spendable_count);
        
        if spendable_count == 0 {
            println!("WARNING: No OUTPOINT_SPENDABLE_BY entries created!");
            println!("This indicates the indexing is not properly tracking spendable outputs.");
        }
        
        assert!(
            spendable_count > 0,
            "Expected OUTPOINT_SPENDABLE_BY entries for {} valid outputs, got {}",
            valid_outputs,
            spendable_count
        );
    }

    #[wasm_bindgen_test]
    fn test_zcash_block_286639_transaction_details() {
        println!("\n=== Testing Block 286639 Transaction Details ===");
        
        // Decode the block from hex
        let block_bytes = hex::decode(BLOCK_286639_HEX.trim())
            .expect("Failed to decode hex");
        let mut cursor = Cursor::new(block_bytes.clone());
        let zblock = ZcashBlock::parse(&mut cursor)
            .expect("Failed to parse ZcashBlock");

        println!("=== Transaction Details ===");
        for (i, tx) in zblock.transactions().iter().enumerate() {
            println!("\nTransaction {}: {}", i, tx.txid());
            println!("  Inputs: {}", tx.inputs().len());
            println!("  Outputs: {}", tx.outputs().len());
            
            // Check each output for valid scriptPubKey
            for (vout, output) in tx.outputs().iter().enumerate() {
                let is_op_return = output.script_pubkey.is_op_return();
                let is_p2pkh = output.script_pubkey.is_p2pkh();
                let is_p2sh = output.script_pubkey.is_p2sh();
                
                println!("    Output {}: {} sats, OP_RETURN={}, P2PKH={}, P2SH={}", 
                    vout, output.value, is_op_return, is_p2pkh, is_p2sh);
            }
            
            if i >= 10 {
                println!("... (showing first 10 transactions)");
                break;
            }
        }
    }
}
