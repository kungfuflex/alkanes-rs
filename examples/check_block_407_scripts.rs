use std::io::Cursor;
use bitcoin::Block;

#[cfg(feature = "zcash")]
use alkanes::zcash::ZcashBlock;

const BLOCK_407_HEX: &str = include_str!("../src/tests/blocks/zec_407.hex");

fn main() {
    let block_bytes = hex::decode(BLOCK_407_HEX.trim()).expect("Failed to decode hex");
    
    #[cfg(feature = "zcash")]
    {
        let mut cursor = Cursor::new(block_bytes);
        let zblock = ZcashBlock::parse(&mut cursor).expect("Failed to parse");
        let block: Block = zblock.into();
        
        println!("=== Block 407 Output Scripts ===\n");
        
        for (i, tx) in block.txdata.iter().enumerate() {
            println!("Transaction {}:", i);
            println!("  Txid: {}", tx.compute_txid());
            
            for (j, output) in tx.output.iter().enumerate() {
                println!("\n  Output {}:", j);
                println!("    Value: {} satoshis", output.value);
                println!("    Script hex: {}", hex::encode(output.script_pubkey.as_bytes()));
                println!("    Script len: {} bytes", output.script_pubkey.len());
                println!("    is_p2pkh: {}", output.script_pubkey.is_p2pkh());
                println!("    is_p2sh: {}", output.script_pubkey.is_p2sh());
                println!("    is_op_return: {}", output.script_pubkey.is_op_return());
                println!("    is_p2wpkh: {}", output.script_pubkey.is_p2wpkh());
                println!("    is_p2wsh: {}", output.script_pubkey.is_p2wsh());
                
                // Try to identify the script type manually
                let bytes = output.script_pubkey.as_bytes();
                if bytes.len() == 35 && bytes[0] == 0xa9 && bytes[1] == 0x14 && bytes[34] == 0x87 {
                    println!("    → Looks like P2SH (OP_HASH160 <20 bytes> OP_EQUAL)");
                } else if bytes.len() == 25 && bytes[0] == 0x76 && bytes[1] == 0xa9 && bytes[2] == 0x14 {
                    println!("    → Looks like P2PKH (OP_DUP OP_HASH160 <20 bytes> OP_EQUALVERIFY OP_CHECKSIG)");
                } else {
                    println!("    → Unknown script type");
                    if bytes.len() <= 100 {
                        println!("    → Raw bytes: {:?}", bytes);
                    }
                }
            }
            println!();
        }
    }
}
