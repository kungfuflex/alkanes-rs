//! Compare qubitcoin's block/tx serialization with the `bitcoin` crate.
//!
//! The alkanes WASM indexer uses the `bitcoin` crate to deserialize blocks.
//! If qubitcoin serializes differently, the WASM will parse corrupted data.

use bitcoin::consensus::{Decodable, Encodable};
use qubitcoin_consensus::transaction::Transaction as QTx;
use qubitcoin_serialize::{deserialize as q_deser, serialize as q_ser};

/// Take a known segwit tx hex, parse with both libs, re-serialize, compare.
#[test]
fn test_segwit_tx_roundtrip_compat() {
    // Real-ish segwit tx with 3-item witness (taproot script-path spend)
    // Structure: 1 input (spending a taproot output), 2 outputs, 3 witness items
    let tx_hex = concat!(
        "02000000",                                 // version
        "0001",                                     // segwit marker + flag
        "01",                                       // 1 input
        "0000000000000000000000000000000000000000000000000000000000000000", // prev_hash
        "00000000",                                 // prev_index
        "00",                                       // script_sig length (empty)
        "ffffffff",                                 // sequence
        "02",                                       // 2 outputs
        "2202000000000000",                         // output 0: 546 sats
        "2251200000000000000000000000000000000000000000000000000000000000000000", // P2TR script
        "0000000000000000",                         // output 1: 0 sats
        "096a5d0542494e000102",                     // OP_RETURN with BIN envelope
        // witness for input 0: 3 items
        "03",                                       // 3 witness items
        "40",                                       // item 0: 64 bytes (signature)
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0a",                                       // item 1: 10 bytes (script)
        "00630342494e00010268",                     // OP_FALSE OP_IF PUSH(BIN) PUSH(0) PUSH(1,2) OP_ENDIF
        "21",                                       // item 2: 33 bytes (control block)
        "c0",                                       // leaf version 0xc0
        "0000000000000000000000000000000000000000000000000000000000000000",
        "00000000",                                 // locktime
    );

    let tx_bytes = hex::decode(tx_hex).expect("valid hex");

    // Parse with qubitcoin
    let qtx: QTx = q_deser(&tx_bytes).expect("qubitcoin should parse segwit tx");
    println!("qubitcoin: {} inputs, {} outputs, witness[0] items: {}",
        qtx.vin.len(), qtx.vout.len(), qtx.vin[0].witness.stack.len());

    // Re-serialize with qubitcoin
    let q_reserialized = q_ser(&qtx).expect("qubitcoin should serialize");
    let q_hex = hex::encode(&q_reserialized);

    // Parse with bitcoin crate
    let btx: bitcoin::Transaction = bitcoin::consensus::deserialize(&tx_bytes)
        .expect("bitcoin crate should parse segwit tx");
    println!("bitcoin: {} inputs, {} outputs, witness[0] items: {}",
        btx.input.len(), btx.output.len(), btx.input[0].witness.len());

    // Re-serialize with bitcoin crate
    let mut b_reserialized = Vec::new();
    btx.consensus_encode(&mut b_reserialized).expect("bitcoin should serialize");
    let b_hex = hex::encode(&b_reserialized);

    // Both should match the original
    println!("Original  hex len: {}", tx_hex.len());
    println!("Qubitcoin hex len: {}", q_hex.len());
    println!("Bitcoin   hex len: {}", b_hex.len());

    // Compare qubitcoin vs bitcoin crate output
    if q_hex != b_hex {
        // Find first difference
        let q_bytes_vec = hex::decode(&q_hex).unwrap();
        let b_bytes_vec = hex::decode(&b_hex).unwrap();
        for (i, (a, b)) in q_bytes_vec.iter().zip(b_bytes_vec.iter()).enumerate() {
            if a != b {
                println!("MISMATCH at byte {}: qubitcoin=0x{:02x} bitcoin=0x{:02x}", i, a, b);
                println!("  qubitcoin context: ...{}...",
                    &q_hex[i.saturating_sub(10)*2..(i+5).min(q_hex.len()/2)*2]);
                println!("  bitcoin   context: ...{}...",
                    &b_hex[i.saturating_sub(10)*2..(i+5).min(b_hex.len()/2)*2]);
                break;
            }
        }
        if q_bytes_vec.len() != b_bytes_vec.len() {
            println!("LENGTH MISMATCH: qubitcoin={} bitcoin={}", q_bytes_vec.len(), b_bytes_vec.len());
        }
    }

    assert_eq!(q_hex, b_hex, "qubitcoin and bitcoin crate should produce identical serialization");

    // Also verify witness items are preserved
    assert_eq!(qtx.vin[0].witness.stack.len(), 3, "should have 3 witness items");
    assert_eq!(qtx.vin[0].witness.stack[0].len(), 64, "signature should be 64 bytes");
    assert_eq!(qtx.vin[0].witness.stack[1].len(), 10, "script should be 10 bytes");
    assert_eq!(qtx.vin[0].witness.stack[2].len(), 33, "control block should be 33 bytes");
}

/// Test block serialization compatibility
#[test]
fn test_block_with_witness_tx_compat() {
    use qubitcoin_consensus::block::Block as QBlock;

    // Minimal block with a coinbase and a segwit tx
    // For simplicity, use a block hex that we can construct
    // First, let's just test tx-level compatibility and ensure the block wrapping works

    // Create a qubitcoin block with TestChain-like structure
    // This is harder without TestChain. Let's test at the tx level first.

    // Parse the tx with qubitcoin, include in a block, serialize the block,
    // then parse the block bytes with the bitcoin crate
    let tx_hex = concat!(
        "02000000",
        "0001",
        "01",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "00000000", "00", "ffffffff",
        "02",
        "2202000000000000",
        "2251200000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000",
        "096a5d0542494e000102",
        "03",
        "40",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0a", "00630342494e00010268",
        "21", "c0",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "00000000",
    );
    let tx_bytes = hex::decode(tx_hex).unwrap();

    // Parse with qubitcoin
    let qtx: QTx = q_deser(&tx_bytes).unwrap();

    // Serialize JUST the tx with qubitcoin
    let q_tx_bytes = q_ser(&qtx).unwrap();

    // Now parse THOSE bytes with the bitcoin crate
    let btx_from_q: bitcoin::Transaction = bitcoin::consensus::deserialize(&q_tx_bytes)
        .expect("bitcoin crate should parse qubitcoin's serialized tx");

    // Check witness is preserved
    assert_eq!(btx_from_q.input[0].witness.len(), 3,
        "bitcoin crate should see 3 witness items from qubitcoin's serialization");
    assert_eq!(btx_from_q.input[0].witness.nth(0).unwrap().len(), 64,
        "signature should be 64 bytes");
    assert_eq!(btx_from_q.input[0].witness.nth(1).unwrap().len(), 10,
        "script should be 10 bytes");
    assert_eq!(btx_from_q.input[0].witness.nth(2).unwrap().len(), 33,
        "control block should be 33 bytes");

    println!("Cross-library witness round-trip: OK");
}

/// Test that a qubitcoin Block containing a witness tx can be parsed by the bitcoin crate
#[test]
fn test_block_bytes_cross_compat() {
    use qubitcoin_consensus::block::Block as QBlock;

    // Create a minimal valid block with a coinbase tx and a witness tx
    // Coinbase: version 2, 1 input (coinbase), 1 output, no witness
    let coinbase_hex = concat!(
        "02000000",                                 // version
        "01",                                       // 1 input
        "0000000000000000000000000000000000000000000000000000000000000000", // null prevhash
        "ffffffff",                                 // prev_index (coinbase)
        "04", "01000101",                           // coinbase script: height 1
        "ffffffff",                                 // sequence
        "01",                                       // 1 output
        "00f2052a01000000",                         // 50 BTC
        "160014d0c4a3ef09e997b6e99e397e518fe3e41a118ca1", // P2WPKH
        "00000000",                                 // locktime
    );

    // Witness tx with BIN envelope
    let witness_tx_hex = concat!(
        "02000000",
        "0001",                                     // segwit
        "01",
        "0000000000000000000000000000000000000000000000000000000000000001",
        "00000000", "00", "ffffffff",
        "02",
        "2202000000000000",
        "2251200000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000",
        "096a5d0542494e000102",
        "03",
        "40",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0a", "00630342494e00010268",
        "21", "c0",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "00000000",
    );

    // Parse both txs with qubitcoin
    let coinbase: QTx = q_deser(&hex::decode(coinbase_hex).unwrap()).unwrap();
    let witness_tx: QTx = q_deser(&hex::decode(witness_tx_hex).unwrap()).unwrap();

    // Build a minimal block header (80 bytes)
    let mut block_bytes = Vec::new();
    // version
    block_bytes.extend_from_slice(&1u32.to_le_bytes());
    // prev_block_hash
    block_bytes.extend_from_slice(&[0u8; 32]);
    // merkle_root (fake)
    block_bytes.extend_from_slice(&[0u8; 32]);
    // time
    block_bytes.extend_from_slice(&1234567890u32.to_le_bytes());
    // bits
    block_bytes.extend_from_slice(&0x207fffff_u32.to_le_bytes());
    // nonce
    block_bytes.extend_from_slice(&0u32.to_le_bytes());
    // tx_count = 2
    block_bytes.push(2);
    // coinbase tx
    block_bytes.extend_from_slice(&q_ser(&coinbase).unwrap());
    // witness tx
    block_bytes.extend_from_slice(&q_ser(&witness_tx).unwrap());

    println!("Block bytes: {} bytes", block_bytes.len());

    // Now parse the block bytes with the bitcoin crate
    let btc_block: bitcoin::Block = bitcoin::consensus::deserialize(&block_bytes)
        .expect("bitcoin crate should parse qubitcoin-serialized block");

    assert_eq!(btc_block.txdata.len(), 2, "should have 2 txs");

    // Check the witness tx (index 1)
    let btx = &btc_block.txdata[1];
    assert_eq!(btx.input[0].witness.len(), 3, "witness should have 3 items");
    assert_eq!(btx.input[0].witness.nth(0).unwrap().len(), 64, "sig = 64 bytes");
    assert_eq!(btx.input[0].witness.nth(1).unwrap().len(), 10, "script = 10 bytes");
    assert_eq!(btx.input[0].witness.nth(2).unwrap().len(), 33, "ctrl = 33 bytes");

    // Check OP_RETURN
    assert!(btx.output[1].script_pubkey.is_op_return(), "output 1 should be OP_RETURN");

    // Check tapscript extraction
    let tapscript = btx.input[0].witness.tapscript();
    assert!(tapscript.is_some(), "tapscript should be extractable");
    let script = tapscript.unwrap();
    assert_eq!(script.len(), 10, "tapscript should be 10 bytes");

    println!("Block cross-compat: OK — bitcoin crate correctly parses qubitcoin block with witness tx");
}
