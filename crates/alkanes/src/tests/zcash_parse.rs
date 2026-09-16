#[cfg(test)]
mod zcash_parse_tests {
    use bitcoin::Block;
    use crate::zcash::ZcashBlock;
    use std::io::Cursor;

    #[test]
    fn test_parse_zcash_block_1500000() {
        let block_hex = include_str!("blocks/zec_1500000.hex").trim();
        let block_bytes = hex::decode(block_hex).expect("Failed to decode hex");
        
        println!("Testing Zcash block 1500000");
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
            }
            Err(e) => {
                println!("✗ Failed to parse with ZcashBlock::parse: {:?}", e);
                panic!("ZcashBlock::parse failed for Zcash block: {:?}", e);
            }
        }
    }
    
    #[test]
    fn test_zcash_block_structure() {
        // This test documents the Zcash block structure
        // Zcash blocks have a different format than Bitcoin/Dogecoin
        
        let block_hex = include_str!("blocks/zec_1500000.hex").trim();
        let block_bytes = hex::decode(block_hex).expect("Failed to decode hex");
        
        println!("\nZcash Block Structure Analysis:");
        println!("Total size: {} bytes", block_bytes.len());
        println!("First 32 bytes (hex): {}", hex::encode(&block_bytes[0..32]));
        
        // Zcash block header format:
        // - Version (4 bytes)
        // - Previous block hash (32 bytes)
        // - Merkle root (32 bytes)
        // - Reserved field (32 bytes) - Zcash-specific
        // - Time (4 bytes)
        // - Bits (4 bytes)
        // - Nonce (32 bytes) - Zcash uses 32 bytes instead of Bitcoin's 4 bytes
        // - Solution size (variable int)
        // - Solution (variable)
        
        let version = u32::from_le_bytes([block_bytes[0], block_bytes[1], block_bytes[2], block_bytes[3]]);
        println!("Version: 0x{:08x} ({})", version, version);
        
        // Check if this is a Zcash v4 block (Sapling)
        if version == 4 {
            println!("This is a Zcash v4 (Sapling) block");
        }
    }
}
