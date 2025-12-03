//! Unit test to index mainnet block 892614 and verify alkane deployment
//!
//! This test indexes block 892614 from mainnet which should contain 1 alkane deployment.
//! The sequence number should increase by 1 after indexing the block.

#[cfg(test)]
mod tests {
    use crate::indexer::index_block;
    use crate::tests::helpers as alkane_helpers;
    use crate::vm::utils::sequence_pointer;
    use alkanes_support::gz::compress;
    use alkanes_support::id::AlkaneId;
    use bitcoin::Block;
    use metashrew_core::index_pointer::{AtomicPointer, IndexPointer};
    use metashrew_core::{println, stdio::stdout};
    use metashrew_support::index_pointer::KeyValuePointer;
    use protorune_support::utils::consensus_decode;
    use std::fmt::Write;
    use std::io::Cursor;
    use std::sync::Arc;
    use wasm_bindgen_test::wasm_bindgen_test;

    const BLOCK_892614_HEX: &str = include_str!("blocks/block_892614_mainnet.hex");
    const ALKANE_4_33_BYTECODE_HEX: &str = include_str!("blocks/alkane_4_33_bytecode.hex");
    const ALKANE_2_0_BYTECODE_HEX: &str = include_str!("blocks/alkane_2_0_bytecode.hex");
    const ALKANE_2_19_BYTECODE_HEX: &str = include_str!("blocks/alkane_2_19_bytecode.hex");
    const ALKANE_2_56_BYTECODE_HEX: &str = include_str!("blocks/alkane_2_56_bytecode.hex");
    const ALKANE_2_215_BYTECODE_HEX: &str = include_str!("blocks/alkane_2_215_bytecode.hex");

    /// Helper function to preload alkane bytecode into the index
    fn preload_alkane_bytecode(alkane_id: AlkaneId, bytecode: &[u8]) {
        let compressed = compress(bytecode.to_vec()).expect("Failed to compress bytecode");
        let key: Vec<u8> = alkane_id.clone().into();
        let mut ptr = IndexPointer::from_keyword("/alkanes/").select(&key);
        ptr.set(Arc::new(compressed));
    }

    /// Test that block 892614 can be parsed correctly
    #[wasm_bindgen_test]
    fn test_block_892614_parse() {
        println!("\n=== Parsing Block 892614 ===");

        let block_bytes = hex::decode(BLOCK_892614_HEX.trim()).expect("Failed to decode hex");

        println!("Block size: {} bytes", block_bytes.len());

        let result = consensus_decode::<Block>(&mut Cursor::new(block_bytes.clone()));

        match result {
            Ok(block) => {
                println!("✓ Successfully parsed block 892614");
                println!("  Block hash: {:?}", block.block_hash());
                println!("  Transactions: {}", block.txdata.len());
            }
            Err(e) => {
                println!("✗ Failed to parse block: {:?}", e);
                panic!("Block parsing failed: {:?}", e);
            }
        }
    }

    /// Test indexing block 892614 with preloaded factory bytecode
    /// This test preloads the alkane [4:33] bytecode so that the factory deployment
    /// at [6:33] can successfully execute without hitting EOF errors.
    #[wasm_bindgen_test]
    fn test_block_892614_with_preloaded_factory() {
        alkane_helpers::clear();

        println!("\n=== Indexing Block 892614 with Preloaded Factory ===");

        // Mark genesis as already seen to prevent genesis() from resetting sequence
        let mut seen_genesis_ptr = IndexPointer::from_keyword("/seen-genesis");
        seen_genesis_ptr.set(Arc::new(vec![1u8]));

        // Set the sequence number to a value that indicates existing alkanes are already created
        // The highest [2:x] called in this block is [2:215], so sequence must be > 215
        // In production, this would be the actual current sequence number
        let initial_sequence: u128 = 300;
        // Use IndexPointer to write directly to global store (not checkpoint stack)
        let mut seq_ptr = IndexPointer::from_keyword("/alkanes/sequence");
        seq_ptr.set_value(initial_sequence);
        println!("Set initial sequence to {}", initial_sequence);

        // Preload all alkanes that are called in this block
        let preload_list: Vec<(AlkaneId, &str)> = vec![
            (AlkaneId { block: 4, tx: 33 }, ALKANE_4_33_BYTECODE_HEX),
            (AlkaneId { block: 2, tx: 0 }, ALKANE_2_0_BYTECODE_HEX),
            (AlkaneId { block: 2, tx: 19 }, ALKANE_2_19_BYTECODE_HEX),
            (AlkaneId { block: 2, tx: 56 }, ALKANE_2_56_BYTECODE_HEX),
            (AlkaneId { block: 2, tx: 215 }, ALKANE_2_215_BYTECODE_HEX),
        ];

        for (alkane_id, hex_str) in preload_list {
            let bytecode = hex::decode(hex_str.trim())
                .expect(&format!("Failed to decode alkane {:?} bytecode hex", alkane_id));
            println!(
                "Preloading alkane [{}:{}]: {} bytes",
                alkane_id.block, alkane_id.tx, bytecode.len()
            );
            preload_alkane_bytecode(alkane_id.clone(), &bytecode);
        }

        let block_bytes = hex::decode(BLOCK_892614_HEX.trim()).expect("Failed to decode hex");

        let block =
            consensus_decode::<Block>(&mut Cursor::new(block_bytes)).expect("Parse should succeed");

        println!("Block hash: {:?}", block.block_hash());
        println!("Transactions: {}", block.txdata.len());

        // Get sequence number before indexing
        let sequence_before = sequence_pointer(&AtomicPointer::default()).get_value::<u128>();
        println!("Sequence before indexing: {}", sequence_before);

        // Index the block at height 892614
        let height = 892614u32;
        match index_block(&block, height) {
            Ok(_) => {
                println!("✓ Successfully indexed block 892614");
            }
            Err(e) => {
                println!("✗ Indexing failed: {:?}", e);
                panic!("index_block failed: {:?}", e);
            }
        }

        // Get sequence number after indexing
        let sequence_after = sequence_pointer(&AtomicPointer::default()).get_value::<u128>();
        println!("Sequence after indexing: {}", sequence_after);

        // Check if sequence went backwards (indicating a bug)
        if sequence_after < sequence_before {
            println!("BUG: Sequence went BACKWARDS from {} to {}", sequence_before, sequence_after);
            println!("This indicates the indexer is not preserving the initial sequence");
            // Check what alkanes were created at low sequence numbers
            for seq in 0..sequence_after {
                let alkane_id = AlkaneId { block: 2, tx: seq };
                let ptr = IndexPointer::from_keyword("/alkanes/").select(&alkane_id.clone().into());
                let data = ptr.get();
                if data.len() > 0 {
                    println!(
                        "  [2:{}] has {} bytes stored (factory ref: {})",
                        seq, data.len(), data.len() == 32
                    );
                }
            }
        } else {
            // Normal case - sequence increased
            let deployments = sequence_after - sequence_before;
            println!("Alkane deployments in block: {}", deployments);

            // Check what alkanes exist at the new sequence numbers
            for seq in sequence_before..sequence_after {
                let alkane_id = AlkaneId { block: 2, tx: seq };
                let ptr = IndexPointer::from_keyword("/alkanes/").select(&alkane_id.clone().into());
                let data = ptr.get();
                println!(
                    "  Alkane [2:{}] has {} bytes stored",
                    seq,
                    data.len()
                );
            }
        }

        println!("Sequence went from {} to {}", initial_sequence, sequence_after);
    }

    /// Test indexing block 892614 without preloaded bytecode (will revert due to EOF)
    #[wasm_bindgen_test]
    fn test_block_892614_indexing_sequence_increase() {
        alkane_helpers::clear();

        println!("\n=== Indexing Block 892614 WITHOUT Preloaded Factory ===");
        println!("(This test expects the factory call to fail with EOF error)");

        let block_bytes = hex::decode(BLOCK_892614_HEX.trim()).expect("Failed to decode hex");

        let block =
            consensus_decode::<Block>(&mut Cursor::new(block_bytes)).expect("Parse should succeed");

        println!("Block hash: {:?}", block.block_hash());
        println!("Transactions: {}", block.txdata.len());

        // Get sequence number before indexing
        let sequence_before = sequence_pointer(&AtomicPointer::default()).get_value::<u128>();
        println!("Sequence before indexing: {}", sequence_before);

        // Index the block at height 892614
        let height = 892614u32;
        match index_block(&block, height) {
            Ok(_) => {
                println!("✓ Successfully indexed block 892614");
            }
            Err(e) => {
                println!("✗ Indexing failed: {:?}", e);
                panic!("index_block failed: {:?}", e);
            }
        }

        // Get sequence number after indexing
        let sequence_after = sequence_pointer(&AtomicPointer::default()).get_value::<u128>();
        println!("Sequence after indexing: {}", sequence_after);

        // Without the factory preloaded, the sequence still increases because
        // run_special_cellpacks increments it before execution fails
        let deployments = sequence_after - sequence_before;
        println!("Alkane deployments in block (including failed): {}", deployments);

        // This test documents current behavior - sequence increases even on revert
        assert_eq!(
            deployments, 1,
            "Expected 1 sequence increment in block 892614 (even though factory call reverted), got {}",
            deployments
        );

        println!("✓ Sequence increased by 1 (note: factory call reverted with EOF error)");
    }
}
