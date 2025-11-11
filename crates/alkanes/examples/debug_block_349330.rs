use std::io::Cursor;

#[cfg(feature = "zcash")]
use alkanes_support::zcash::ZcashBlock;
use alkanes_support::block_traits::{BlockLike, TransactionLike};

const BLOCK_349330_HEX: &str = include_str!("../src/tests/blocks/zec_349330.hex");

fn main() {
    #[cfg(feature = "zcash")]
    {
        println!("\n=== Analyzing Zcash Block 349330 (Memory Fault Block) ===\n");
        
        // Decode the block from hex
        let block_bytes = hex::decode(BLOCK_349330_HEX.trim())
            .expect("Failed to decode hex");
        
        println!("Block size: {} bytes", block_bytes.len());
        println!("Block hex (first 200 chars): {}", &BLOCK_349330_HEX[..200.min(BLOCK_349330_HEX.len())]);
        println!();
        
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
                
                std::process::exit(1);
            }
        };
        
        println!("Block hash: {:?}", zblock.block_hash());
        println!("Number of transactions: {}", zblock.transactions().len());
        println!("Block parsed bytes: {} / {}", cursor.position(), block_bytes.len());
        println!();
        
        // Detailed transaction analysis
        println!("=== Transaction Analysis ===");
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
                } else if output.script_pubkey.len() > 100 {
                    println!("    Script (first 100 bytes): {}", 
                        hex::encode(&output.script_pubkey.as_bytes()[..100.min(output.script_pubkey.len())]));
                }
            }
        }
        
        println!("\n=== Summary ===");
        println!("Block 349330 parsed successfully!");
        println!("Total transactions: {}", zblock.transactions().len());
        println!("Total inputs: {}", zblock.transactions().iter().map(|tx| tx.inputs().len()).sum::<usize>());
        println!("Total outputs: {}", zblock.transactions().iter().map(|tx| tx.outputs().len()).sum::<usize>());
        
        let max_script_size = zblock.transactions().iter()
            .flat_map(|tx| tx.outputs().iter())
            .map(|out| out.script_pubkey.len())
            .max()
            .unwrap_or(0);
        println!("Largest output script: {} bytes", max_script_size);
        
        if max_script_size > 10000 {
            println!("\n⚠ WARNING: Found very large script(s) that could cause memory issues!");
        }
        
        println!("\n=== Next Steps ===");
        println!("The block parses successfully, so the memory fault must occur during indexing.");
        println!("Run the full test suite to reproduce the error:");
        println!("  cargo test --package alkanes --features zcash zcash_block_349330 --lib");
    }
    
    #[cfg(not(feature = "zcash"))]
    {
        eprintln!("This example requires the 'zcash' feature to be enabled");
        eprintln!("Run with: cargo run --example debug_block_349330 --features zcash");
        std::process::exit(1);
    }
}
