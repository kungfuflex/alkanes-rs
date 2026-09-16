#[cfg(test)]
mod zcash_block_0_tests {
    use bitcoin::Block;
    use crate::zcash::ZcashBlock;
    use crate::index_block;
    use crate::tests::helpers as alkane_helpers;
    use std::io::Cursor;
    use anyhow::Result;
    use wasm_bindgen_test::wasm_bindgen_test;

    #[wasm_bindgen_test]
    fn test_parse_zcash_block_0() -> Result<()> {
        let block_hex = include_str!("blocks/zec_0.hex").trim();
        let block_bytes = hex::decode(block_hex).expect("Failed to decode hex");
        
        println!("Testing Zcash block 0 (genesis)");
        println!("Block size: {} bytes", block_bytes.len());
        
        // Test: Parse with ZcashBlock
        let result = ZcashBlock::parse(&mut Cursor::new(block_bytes.clone()));
        
        match result {
            Ok(zcash_block) => {
                println!("✓ Successfully parsed with ZcashBlock::parse");
                println!("  Zcash version: {}", zcash_block.version);
                
                // Try converting to bitcoin::Block
                let consensus_block: Block = zcash_block.into();
                
                println!("✓ Successfully converted to bitcoin::Block");
                println!("  Block header version: {:?}", consensus_block.header.version);
                println!("  Previous block hash: {}", consensus_block.header.prev_blockhash);
                println!("  Merkle root: {}", consensus_block.header.merkle_root);
                println!("  Time: {}", consensus_block.header.time);
                println!("  Bits: {}", consensus_block.header.bits.to_consensus());
                println!("  Nonce: {}", consensus_block.header.nonce);
                println!("  Transaction count: {}", consensus_block.txdata.len());
                
                // Verify we have transactions
                assert!(consensus_block.txdata.len() > 0, "Block should have transactions");
                
                // Verify coinbase transaction
                assert!(consensus_block.txdata[0].is_coinbase(), "First transaction should be coinbase");
                
                println!("  Coinbase txid: {}", consensus_block.txdata[0].compute_txid());
                println!("  Coinbase inputs: {}", consensus_block.txdata[0].input.len());
                println!("  Coinbase outputs: {}", consensus_block.txdata[0].output.len());
                
                Ok(())
            }
            Err(e) => {
                println!("✗ Failed to parse with ZcashBlock::parse: {:?}", e);
                Err(anyhow::anyhow!("ZcashBlock::parse failed for Zcash block 0: {:?}", e))
            }
        }
    }
    
    #[wasm_bindgen_test]
    fn test_index_zcash_block_0() -> Result<()> {
        // Setup
        alkane_helpers::clear();
        
        let block_hex = include_str!("blocks/zec_0.hex").trim();
        let block_bytes = hex::decode(block_hex).expect("Failed to decode hex");
        
        println!("Testing indexing of Zcash block 0");
        
        // Parse block
        let zcash_block = ZcashBlock::parse(&mut Cursor::new(block_bytes))
            .map_err(|e| anyhow::anyhow!("Failed to parse block 0: {:?}", e))?;
        
        let consensus_block: Block = zcash_block.into();
        
        println!("  Block parsed successfully");
        println!("  Transaction count: {}", consensus_block.txdata.len());
        
        // Try to index the block
        match index_block(&consensus_block, 0) {
            Ok(_) => {
                println!("✓ Successfully indexed Zcash block 0");
                Ok(())
            }
            Err(e) => {
                println!("✗ Failed to index Zcash block 0: {:?}", e);
                Err(anyhow::anyhow!("index_block failed for block 0: {:?}", e))
            }
        }
    }
}
