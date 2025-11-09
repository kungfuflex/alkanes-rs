use alkanes::indexer::index_block;
use alkanes_support::zcash::ZcashBlock;
use bitcoin::Block;
use metashrew_support::index_pointer::KeyValuePointer;
use protorune::tables::OUTPOINT_SPENDABLE_BY;
use std::io::Cursor;

const BLOCK_286639_HEX: &str = include_str!("../src/tests/blocks/zec_286639.hex");

fn main() {
    println!("\n=== Testing Zcash Block 286639 ===\n");
    
    // Decode hex
    let block_bytes = match hex::decode(BLOCK_286639_HEX.trim()) {
        Ok(bytes) => {
            println!("✓ Decoded hex: {} bytes", bytes.len());
            bytes
        }
        Err(e) => {
            eprintln!("✗ Failed to decode hex: {:?}", e);
            std::process::exit(1);
        }
    };
    
    // Parse block
    let mut cursor = Cursor::new(block_bytes.clone());
    let zblock = match ZcashBlock::parse(&mut cursor) {
        Ok(zb) => {
            println!("✓ Parsed ZcashBlock");
            zb
        }
        Err(e) => {
            eprintln!("✗ Failed to parse ZcashBlock: {:?}", e);
            std::process::exit(1);
        }
    };
    
    use alkanes_support::block_traits::BlockLike;
    
    println!("  Block hash: {}", zblock.block_hash());
    println!("  Transactions: {}", zblock.transactions().len());
    
    // Analyze outputs before indexing
    let mut total_outputs = 0;
    let mut p2pkh_outputs = 0;
    let mut p2sh_outputs = 0;
    let mut op_return_outputs = 0;
    let mut other_outputs = 0;
    
    use alkanes_support::block_traits::TransactionLike;
    
    for tx in zblock.transactions() {
        for output in tx.outputs() {
            total_outputs += 1;
            if output.script_pubkey.is_op_return() {
                op_return_outputs += 1;
            } else if output.script_pubkey.is_p2pkh() {
                p2pkh_outputs += 1;
            } else if output.script_pubkey.is_p2sh() {
                p2sh_outputs += 1;
            } else {
                other_outputs += 1;
            }
        }
    }
    
    println!("\n=== Output Analysis ===");
    println!("  Total outputs: {}", total_outputs);
    println!("  P2PKH (t1): {}", p2pkh_outputs);
    println!("  P2SH (t3): {}", p2sh_outputs);
    println!("  OP_RETURN: {}", op_return_outputs);
    println!("  Other: {}", other_outputs);
    
    // Index the block
    println!("\n=== Indexing Block ===");
    match index_block(&zblock, 286639) {
        Ok(_) => println!("✓ Successfully indexed block 286639"),
        Err(e) => {
            eprintln!("✗ Failed to index block: {:?}", e);
            std::process::exit(1);
        }
    }
    
    // Check OUTPOINT_SPENDABLE_BY entries
    println!("\n=== Checking OUTPOINT_SPENDABLE_BY ===");
    let mut spendable_count = 0;
    let mut first_five = Vec::new();
    
    for (tx_idx, tx) in zblock.transactions().iter().enumerate() {
        let txid = tx.txid();
        for (vout, _output) in tx.outputs().iter().enumerate() {
            let outpoint = bitcoin::OutPoint {
                txid: txid.clone(),
                vout: vout as u32,
            };
            let outpoint_bytes = metashrew_support::utils::consensus_encode(&outpoint)
                .expect("Failed to encode outpoint");
            let spendable_by = OUTPOINT_SPENDABLE_BY.select(&outpoint_bytes).get();
            
            if !spendable_by.is_empty() {
                spendable_count += 1;
                if first_five.len() < 5 {
                    let addr = String::from_utf8_lossy(&spendable_by);
                    first_five.push(format!("  TX {} output {} -> {}", tx_idx, vout, addr));
                }
            }
        }
    }
    
    println!("  Total OUTPOINT_SPENDABLE_BY entries: {}", spendable_count);
    if !first_five.is_empty() {
        println!("\n  First few entries:");
        for entry in first_five {
            println!("{}", entry);
        }
    }
    
    // Final verdict
    println!("\n=== Results ===");
    let expected_spendable = p2pkh_outputs + p2sh_outputs;
    println!("  Expected spendable outputs: {}", expected_spendable);
    println!("  Actual OUTPOINT_SPENDABLE_BY: {}", spendable_count);
    
    if spendable_count == 0 {
        eprintln!("\n✗ ERROR: No OUTPOINT_SPENDABLE_BY entries were created!");
        eprintln!("This indicates the indexing is not properly tracking spendable outputs.");
        std::process::exit(1);
    } else if spendable_count < expected_spendable {
        eprintln!("\n⚠ WARNING: Only {} out of {} expected outputs were indexed", 
            spendable_count, expected_spendable);
    } else {
        println!("\n✓ SUCCESS: All expected outputs were indexed");
    }
}
