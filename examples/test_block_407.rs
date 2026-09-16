use std::io::Cursor;
use bitcoin::Block;

#[cfg(feature = "zcash")]
use alkanes::zcash::ZcashBlock;

const BLOCK_407_HEX: &str = include_str!("../src/tests/blocks/zec_407.hex");

fn main() {
    println!("\n=== Testing Block 407 Parsing ===");
    println!("Block hex length: {} chars", BLOCK_407_HEX.trim().len());
    
    // Decode hex
    let block_bytes = match hex::decode(BLOCK_407_HEX.trim()) {
        Ok(bytes) => {
            println!("✓ Successfully decoded hex to {} bytes", bytes.len());
            bytes
        }
        Err(e) => {
            eprintln!("✗ Failed to decode hex: {:?}", e);
            std::process::exit(1);
        }
    };

    #[cfg(feature = "zcash")]
    {
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
                    }
                }
                
                println!("\n--- Cursor Position After Parse ---");
                println!("  Position: {} / {} bytes", cursor.position(), block_bytes.len());
                println!("  Remaining: {} bytes", block_bytes.len() - cursor.position() as usize);
                
                // Show transaction details
                println!("\n--- Transaction Details ---");
                let block: Block = zblock.into();
                for (i, tx) in block.txdata.iter().enumerate() {
                    println!("  Tx {}: {} ({} inputs, {} outputs)", 
                        i, 
                        tx.compute_txid(),
                        tx.input.len(),
                        tx.output.len()
                    );
                }
                
                println!("\n✓ Block 407 parsed successfully!")
            }
            Err(e) => {
                eprintln!("✗ Failed to parse ZcashBlock: {:?}", e);
                eprintln!("\n--- Cursor Position at Failure ---");
                eprintln!("  Position: {} / {} bytes", cursor.position(), block_bytes.len());
                
                // Show some bytes around the failure point
                let pos = cursor.position() as usize;
                let start = pos.saturating_sub(32);
                let end = (pos + 32).min(block_bytes.len());
                eprintln!("\n--- Bytes around failure point ---");
                eprintln!("  Context: bytes {} to {}", start, end);
                eprintln!("  Hex: {}", hex::encode(&block_bytes[start..end]));
                
                std::process::exit(1);
            }
        }
    }

    #[cfg(not(feature = "zcash"))]
    {
        eprintln!("This example requires the 'zcash' feature to be enabled");
        std::process::exit(1);
    }
}
