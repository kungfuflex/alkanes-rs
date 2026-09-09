//! Test for indexing Zcash block 250
//! This block has only 1 transaction (coinbase), so we expect minimal k/v pairs:
//! 1. HEIGHT_TO_BLOCKHASH
//! 2. HEIGHT_TO_TRANSACTION_IDS  
//! 3. TRANSACTION_ID_TO_HEIGHT
//! 4. Potentially one more for block metadata
//!
//! Total: ~4 k/v pairs (no OUTPOINT_SPENDABLE_BY since coinbase outputs can't be spent yet)

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

    const BLOCK_250_HEX: &str = include_str!("blocks/zec_250.hex");

    #[wasm_bindgen_test]
    fn test_zcash_block_250_parsing() {
        println!("\n=== Testing Block 250 Parsing ===");
        
        // Decode the block from hex
        let block_bytes = hex::decode(BLOCK_250_HEX.trim())
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
        
        // Verify it's a single-transaction block (coinbase only)
        assert_eq!(zblock.transactions().len(), 1, "Block 250 should have exactly 1 transaction");
        
        // Verify it's a coinbase transaction
        let tx = &zblock.transactions()[0];
        assert!(tx.is_coinbase(), "First transaction should be coinbase");
        
        println!("Coinbase transaction:");
        println!("  Inputs: {}", tx.inputs().len());
        println!("  Outputs: {}", tx.outputs().len());
        
        // Check outputs
        for (vout, output) in tx.outputs().iter().enumerate() {
            let is_op_return = output.script_pubkey.is_op_return();
            let is_p2pkh = output.script_pubkey.is_p2pkh();
            let is_p2sh = output.script_pubkey.is_p2sh();
            
            println!("    Output {}: {} sats, OP_RETURN={}, P2PKH={}, P2SH={}", 
                vout, output.value, is_op_return, is_p2pkh, is_p2sh);
        }
    }

    #[wasm_bindgen_test]
    fn test_zcash_block_250_indexing() {
        alkane_helpers::clear();
        
        println!("\n=== Testing Block 250 Indexing ===");
        
        // Decode the block from hex
        let block_bytes = hex::decode(BLOCK_250_HEX.trim())
            .expect("Failed to decode hex");
        let mut cursor = Cursor::new(block_bytes.clone());
        let zblock = ZcashBlock::parse(&mut cursor)
            .expect("Failed to parse ZcashBlock");

        println!("=== Before indexing ===");
        println!("Block hash: {:?}", zblock.block_hash());
        println!("Number of transactions: {}", zblock.transactions().len());

        // Index the block using the generic function
        index_block(&zblock, 250)
            .expect("Failed to index block");

        println!("\n=== After indexing - Verifying k/v pairs ===");
        
        // Check HEIGHT_TO_BLOCKHASH
        let blockhash = RUNES.HEIGHT_TO_BLOCKHASH.select_value::<u64>(250).get();
        println!("HEIGHT_TO_BLOCKHASH stored: {}", !blockhash.is_empty());
        assert!(!blockhash.is_empty(), "HEIGHT_TO_BLOCKHASH should be stored");

        // Check OUTPOINT_SPENDABLE_BY - should be empty or only have coinbase outputs
        let mut spendable_count = 0;
        
        for tx in zblock.transactions().iter() {
            let txid = tx.txid();
            for (vout, output) in tx.outputs().iter().enumerate() {
                let outpoint = bitcoin::OutPoint {
                    txid: txid.clone(),
                    vout: vout as u32,
                };
                let outpoint_bytes = metashrew_support::utils::consensus_encode(&outpoint)
                    .expect("Failed to encode outpoint");
                let spendable_by = OUTPOINT_SPENDABLE_BY.select(&outpoint_bytes).get();
                
                if !spendable_by.is_empty() {
                    spendable_count += 1;
                    let addr = String::from_utf8_lossy(&spendable_by);
                    println!("  Output {} is spendable by: {}", vout, addr);
                }
            }
        }
        
        println!("Total OUTPOINT_SPENDABLE_BY entries: {}", spendable_count);
        
        // For block 250 with only a coinbase transaction:
        // - Coinbase outputs are indexed if they have valid t-addresses
        // - Expected: 2 outputs (one P2PKH-like, one P2SH)
        // So we should have 2 OUTPOINT_SPENDABLE_BY entries
        println!("\n=== Verification ===");
        println!("Expected k/v pairs:");
        println!("  1. HEIGHT_TO_BLOCKHASH: 1 entry");
        println!("  2. HEIGHT_TO_TRANSACTION_IDS: 1 entry (for the block)");
        println!("  3. TRANSACTION_ID_TO_HEIGHT: 1 entry (for the coinbase tx)");
        println!("  4. OUTPOINT_SPENDABLE_BY: {} entries (for valid outputs)", spendable_count);
        println!("  Total: ~{} k/v pairs", 3 + spendable_count);
        
        // The "4 k/v pairs" mentioned in the PR is likely:
        // - 1 HEIGHT_TO_BLOCKHASH
        // - 1 HEIGHT_TO_TRANSACTION_IDS
        // - 1 TRANSACTION_ID_TO_HEIGHT
        // - 1 or more OUTPOINT_SPENDABLE_BY
        // If spendable_count is 0-1, that explains the ~4 k/v pairs
        assert!(
            spendable_count <= 2,
            "Block 250 with 1 coinbase transaction should have at most 2 spendable outputs"
        );
    }

    #[wasm_bindgen_test]
    fn test_zcash_block_250_transaction_details() {
        println!("\n=== Testing Block 250 Transaction Details ===");
        
        // Decode the block from hex
        let block_bytes = hex::decode(BLOCK_250_HEX.trim())
            .expect("Failed to decode hex");
        let mut cursor = Cursor::new(block_bytes.clone());
        let zblock = ZcashBlock::parse(&mut cursor)
            .expect("Failed to parse ZcashBlock");

        println!("=== Transaction Details ===");
        for (i, tx) in zblock.transactions().iter().enumerate() {
            println!("\nTransaction {}: {}", i, tx.txid());
            println!("  Is Coinbase: {}", tx.is_coinbase());
            println!("  Inputs: {}", tx.inputs().len());
            println!("  Outputs: {}", tx.outputs().len());
            
            // Check each output
            for (vout, output) in tx.outputs().iter().enumerate() {
                let is_op_return = output.script_pubkey.is_op_return();
                let is_p2pkh = output.script_pubkey.is_p2pkh();
                let is_p2sh = output.script_pubkey.is_p2sh();
                let script_len = output.script_pubkey.len();
                
                println!("    Output {}: {} sats", vout, output.value);
                println!("      Script length: {} bytes", script_len);
                println!("      OP_RETURN: {}, P2PKH: {}, P2SH: {}", is_op_return, is_p2pkh, is_p2sh);
                
                if !is_op_return && !is_p2pkh && !is_p2sh {
                    println!("      Script type: UNKNOWN/OTHER");
                    println!("      Script hex: {}", hex::encode(output.script_pubkey.as_bytes()));
                }
            }
        }
    }
}
