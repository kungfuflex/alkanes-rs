//! Test for alkanes backtest functionality
//!
//! This test verifies that the preview function works correctly for
//! backtesting alkanes transactions.

use crate::in_memory_adapters::{InMemoryBitcoinNode, InMemoryRuntime};
use crate::test_utils::WASM;
use metashrew_sync::{PreviewCall, ViewCall, ViewResult};
use async_trait::async_trait;
use metashrew_sync::traits::RuntimeAdapter;
use bitcoin::{Transaction, TxIn, TxOut, OutPoint, ScriptBuf, Witness, Sequence};
use bitcoin::blockdata::transaction::Version;
use bitcoin::locktime::absolute::LockTime;
use bitcoin::hashes::Hash;

#[tokio::test]
async fn test_alkanes_backtest_with_trace() {
    // 1. Setup
    let wasm_bytes = WASM;
    let mut runtime = InMemoryRuntime::new(&wasm_bytes).await;
    let mut adapter = runtime.new_runtime_adapter();
    
    // 2. Create test blocks
    let genesis_block = crate::test_utils::TestUtils::create_test_block(
        0,
        bitcoin::BlockHash::from_byte_array([0u8; 32])
    );
    let block1 = crate::test_utils::TestUtils::create_test_block(1, genesis_block.block_hash());
    
    // 3. Index genesis and block 1
    adapter.process_block(
        0,
        &metashrew_support::utils::consensus_encode(&genesis_block).unwrap()
    ).await.unwrap();
    adapter.process_block(
        1,
        &metashrew_support::utils::consensus_encode(&block1).unwrap()
    ).await.unwrap();
    
    // 4. Create a test transaction (simulating an alkanes transaction)
    let test_tx = Transaction {
        version: Version::TWO,
        lock_time: LockTime::ZERO,
        input: vec![TxIn {
            previous_output: OutPoint {
                txid: bitcoin::Txid::from_byte_array([1u8; 32]),
                vout: 0,
            },
            script_sig: ScriptBuf::new(),
            sequence: Sequence::MAX,
            witness: Witness::new(),
        }],
        output: vec![
            TxOut {
                value: bitcoin::Amount::from_sat(546),
                script_pubkey: ScriptBuf::new(),
            },
            TxOut {
                value: bitcoin::Amount::from_sat(0),
                script_pubkey: ScriptBuf::new(), // Would be OP_RETURN with runestone
            },
        ],
    };
    
    // 5. Create a coinbase transaction
    let coinbase_tx = Transaction {
        version: Version::TWO,
        lock_time: LockTime::ZERO,
        input: vec![TxIn {
            previous_output: OutPoint::null(),
            script_sig: ScriptBuf::new(),
            sequence: Sequence::MAX,
            witness: Witness::from_slice(&[vec![0u8; 32]]),
        }],
        output: vec![TxOut {
            value: bitcoin::Amount::from_sat(50_00000000),
            script_pubkey: ScriptBuf::new(),
        }],
    };
    
    // 6. Create simulated block with coinbase + test transaction
    let simulated_block = bitcoin::Block {
        header: bitcoin::block::Header {
            version: bitcoin::block::Version::TWO,
            prev_blockhash: block1.block_hash(),
            merkle_root: bitcoin::TxMerkleNode::from_byte_array([0u8; 32]),
            time: 2,
            bits: bitcoin::CompactTarget::from_consensus(0),
            nonce: 0,
        },
        txdata: vec![coinbase_tx, test_tx.clone()],
    };
    
    // 7. Test preview with blocktracker (since that's what the test WASM has)
    // Note: The actual alkanes indexer would have a "trace" view function
    let call = PreviewCall {
        block_data: metashrew_support::utils::consensus_encode(&simulated_block).unwrap(),
        function_name: "blocktracker".to_string(),
        input_data: vec![],
        height: 2,
    };
    
    // 8. Execute the preview
    let preview_result = adapter.execute_preview(call).await;
    
    match preview_result {
        Ok(result) => {
            // The blocktracker should now show 3 bytes (0, 1, 2 for 3 blocks)
            assert_eq!(
                result.data.len(),
                3,
                "Preview should show 3 blocks processed"
            );
        }
        Err(e) => {
            panic!("Preview failed: {:?}", e);
        }
    }
    
    // 9. Verify original state unchanged
    let final_state = adapter.execute_view(metashrew_sync::ViewCall {
        function_name: "blocktracker".to_string(),
        input_data: vec![],
        height: 1,
    }).await.unwrap();
    
    assert_eq!(
        final_state.data.len(),
        2,
        "Original state should still show 2 blocks"
    );
}

#[tokio::test]
async fn test_trace_input_encoding() {
    // This test demonstrates how to properly encode the trace input
    // for the alkanes indexer's "trace" view function
    
    use protorune_support::proto::protorune::Outpoint;
    use prost::Message;
    
    let txid = "2d95f568908349fd00f88c2f5801e5bf7bac084bc561c7c0a6acc1940fc0de57";
    let vout = 0u32;
    let height = 1305u32;
    
    // Convert txid hex to bytes
    let txid_bytes = hex::decode(txid).expect("Invalid txid hex");
    assert_eq!(txid_bytes.len(), 32, "TXID should be 32 bytes");
    
    // Create protobuf Outpoint message
    let outpoint = Outpoint {
        txid: txid_bytes.clone(),
        vout,
    };
    
    let outpoint_bytes = outpoint.encode_to_vec();
    
    // The trace view function expects:
    // 1. Height (u32) - 4 bytes little-endian
    // 2. Protobuf-encoded Outpoint
    let mut input_data = Vec::new();
    input_data.extend_from_slice(&height.to_le_bytes());
    input_data.extend_from_slice(&outpoint_bytes);
    
    println!("Height: {}", height);
    println!("Outpoint protobuf length: {}", outpoint_bytes.len());
    println!("Outpoint protobuf hex: {}", hex::encode(&outpoint_bytes));
    println!("Total input_data length: {}", input_data.len());
    println!("Total input_data hex: {}", hex::encode(&input_data));
    
    // Protobuf encoding of Outpoint { txid: [32 bytes], vout: 0 }
    // Field 1 (txid): 0a (field 1, wire type 2) + 20 (length 32) + 32 bytes
    // Field 2 (vout): 10 (field 2, wire type 0) + 00 (varint 0)
    // Total: 1 + 1 + 32 + 1 + 1 = 36 bytes for outpoint
    // Plus 4 bytes for height = 40 bytes total
    
    assert_eq!(input_data.len(), 40, "Input should be height (4) + protobuf outpoint (~36)");
}

#[tokio::test]
async fn test_preview_with_actual_bitcoin_block() {
    // This test demonstrates creating a realistic Bitcoin block structure
    // that would be used for backtesting
    
    use bitcoin::hashes::Hash;
    
    let wasm_bytes = WASM;
    let mut runtime = InMemoryRuntime::new(&wasm_bytes).await;
    let mut adapter = runtime.new_runtime_adapter();
    
    // Index some initial blocks
    let genesis_block = crate::test_utils::TestUtils::create_test_block(
        0,
        bitcoin::BlockHash::from_byte_array([0u8; 32])
    );
    adapter.process_block(
        0,
        &metashrew_support::utils::consensus_encode(&genesis_block).unwrap()
    ).await.unwrap();
    
    // Create a block with actual structure
    let prev_hash = genesis_block.block_hash();
    
    // Coinbase transaction
    let coinbase = Transaction {
        version: Version::TWO,
        lock_time: LockTime::ZERO,
        input: vec![TxIn {
            previous_output: OutPoint::null(),
            script_sig: ScriptBuf::new(),
            sequence: Sequence::MAX,
            witness: Witness::from_slice(&[vec![0u8; 32]]),
        }],
        output: vec![TxOut {
            value: bitcoin::Amount::from_sat(50_00000000),
            script_pubkey: ScriptBuf::new(),
        }],
    };
    
    // Regular transaction
    let tx = Transaction {
        version: Version::TWO,
        lock_time: LockTime::ZERO,
        input: vec![TxIn {
            previous_output: OutPoint {
                txid: bitcoin::Txid::from_byte_array([1u8; 32]),
                vout: 0,
            },
            script_sig: ScriptBuf::new(),
            sequence: Sequence::MAX,
            witness: Witness::new(),
        }],
        output: vec![TxOut {
            value: bitcoin::Amount::from_sat(1000),
            script_pubkey: ScriptBuf::new(),
        }],
    };
    
    let block = bitcoin::Block {
        header: bitcoin::block::Header {
            version: bitcoin::block::Version::TWO,
            prev_blockhash: prev_hash,
            merkle_root: bitcoin::TxMerkleNode::from_byte_array([0u8; 32]),
            time: 1,
            bits: bitcoin::CompactTarget::from_consensus(0x1d00ffff),
            nonce: 0,
        },
        txdata: vec![coinbase, tx],
    };
    
    let block_data = metashrew_support::utils::consensus_encode(&block).unwrap();
    
    println!("Block header size: 80 bytes");
    println!("Total block size: {} bytes", block_data.len());
    println!("Block has {} transactions", block.txdata.len());
    
    // Preview the block
    let call = PreviewCall {
        block_data: block_data.clone(),
        function_name: "blocktracker".to_string(),
        input_data: vec![],
        height: 1,
    };
    
    let result = adapter.execute_preview(call).await;
    assert!(result.is_ok(), "Preview should succeed with valid block structure");
}
