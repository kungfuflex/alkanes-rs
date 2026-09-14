/// Tests for the auto-change protostone pattern used by alkanes-cli.
///
/// When a UTXO carries more alkane tokens than needed for a contract call,
/// alkanes-cli generates a two-protostone runestone:
///   p0 (auto-change): edicts route needed tokens to p1, excess to change output
///   p1 (user call): cellpack calling the contract
///
/// This test reproduces that exact pattern at the indexer level.

use crate::message::AlkaneMessageContext;
use crate::tests::helpers as alkane_helpers;
use crate::tests::std::alkanes_std_test_build;
use alkane_helpers::clear;
use alkanes::indexer::index_block;
use alkanes_support::cellpack::Cellpack;
use alkanes_support::id::AlkaneId;
use anyhow::Result;
use bitcoin::address::NetworkChecked;
use bitcoin::{transaction::Version, Address, Amount, Block, OutPoint, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Witness};
use metashrew_core::{
    println,
    stdio::{stdout, Write},
};
use metashrew_support::{index_pointer::KeyValuePointer, utils::consensus_encode};
use ordinals::Runestone;
use protorune::protostone::Protostones;
use protorune::test_helpers::{create_block_with_coinbase_tx, get_address, ADDRESS1};
use protorune::{
    balance_sheet::load_sheet, message::MessageContext, tables::RuneTable,
};
use protorune_support::balance_sheet::ProtoruneRuneId;
use protorune_support::protostone::{Protostone, ProtostoneEdict};
use wasm_bindgen_test::wasm_bindgen_test;

/// Two-protostone auto-change: p0 routes all tokens to p1 via edicts,
/// p1 calls the contract. No excess.
#[wasm_bindgen_test]
fn test_auto_change_protostone_all_to_call() -> Result<()> {
    clear();
    let block_height = 0;

    // Block 0: deploy contract and mint 100 tokens of alkane 2:1
    let arb_mint_cellpack = Cellpack {
        target: AlkaneId { block: 1, tx: 0 },
        inputs: vec![30, 2, 1, 100],
    };

    let mut test_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
        [alkanes_std_test_build::get_bytes()].into(),
        [arb_mint_cellpack].into(),
    );
    index_block(&test_block, block_height)?;

    // Verify tokens minted on vout:0 of last tx
    let mint_sheet = alkane_helpers::get_last_outpoint_sheet(&test_block)?;
    let alkane_id = ProtoruneRuneId { block: 2, tx: 1 };
    println!("Minted tokens on vout:0: {:?}", mint_sheet);
    assert_eq!(mint_sheet.get_cached(&alkane_id), 100);

    // Block 1: two-protostone auto-change pattern
    let mut test_block2 = create_block_with_coinbase_tx(block_height);

    let txin = TxIn {
        previous_output: OutPoint {
            txid: test_block.txdata[test_block.txdata.len() - 1].compute_txid(),
            vout: 0,
        },
        script_sig: ScriptBuf::new(),
        sequence: Sequence::MAX,
        witness: Witness::new(),
    };

    // Transaction will have 2 outputs: [txout, op_return]
    // tx.output.len() = 2
    // Shadow vouts: p0 = 2, p1 = 3
    let address: Address<NetworkChecked> = get_address(&ADDRESS1().as_str());
    let txout = TxOut {
        value: Amount::from_sat(100_000_000),
        script_pubkey: address.script_pubkey(),
    };

    let user_cellpack = Cellpack {
        target: AlkaneId { block: 2, tx: 1 },
        inputs: vec![5], // opcode 5: forward/return tokens
    };

    let protostones = vec![
        // p0: auto-change protostone — routes tokens to p1 via edict
        Protostone {
            message: vec![], // no cellpack
            protocol_tag: 1,
            burn: None,
            from: None,
            pointer: Some(3), // point to p1 (shadow vout = 2 + 1 = 3)
            refund: Some(0),  // refund to physical output 0
            edicts: vec![ProtostoneEdict {
                id: alkane_id.clone(),
                amount: 100,
                output: 3, // route to p1 (shadow vout = 3)
            }],
        },
        // p1: user call protostone
        Protostone {
            message: user_cellpack.encipher(),
            protocol_tag: 1,
            burn: None,
            from: None,
            pointer: Some(0), // output to physical vout 0
            refund: Some(0),
            edicts: vec![],
        },
    ];

    let runestone_script = (Runestone {
        edicts: vec![],
        etching: None,
        mint: None,
        pointer: Some(0),
        protocol: Some(protostones.encipher()?),
    })
    .encipher();

    let op_return = TxOut {
        value: Amount::from_sat(0),
        script_pubkey: runestone_script,
    };

    let tx = Transaction {
        version: Version::ONE,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: vec![txin],
        output: vec![txout, op_return],
    };
    test_block2.txdata.push(tx);

    index_block(&test_block2, block_height)?;

    // Verify: tokens should end up at vout:0 of the last tx
    let result_sheet = alkane_helpers::get_last_outpoint_sheet(&test_block2)?;
    println!("Result sheet after auto-change: {:?}", result_sheet);
    assert_eq!(
        result_sheet.get_cached(&alkane_id),
        100,
        "All 100 tokens should arrive at output 0 after auto-change routing"
    );

    Ok(())
}

/// Two-protostone auto-change with excess: p0 routes needed tokens to p1,
/// sends excess back to change output (vout:0).
#[wasm_bindgen_test]
fn test_auto_change_protostone_with_excess() -> Result<()> {
    clear();
    let block_height = 0;

    // Block 0: deploy contract and mint 1000 tokens of alkane 2:1
    let arb_mint_cellpack = Cellpack {
        target: AlkaneId { block: 1, tx: 0 },
        inputs: vec![30, 2, 1, 1000],
    };

    let mut test_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
        [alkanes_std_test_build::get_bytes()].into(),
        [arb_mint_cellpack].into(),
    );
    index_block(&test_block, block_height)?;

    let alkane_id = ProtoruneRuneId { block: 2, tx: 1 };
    let mint_sheet = alkane_helpers::get_last_outpoint_sheet(&test_block)?;
    assert_eq!(mint_sheet.get_cached(&alkane_id), 1000);

    // Block 1: auto-change splits 300 to contract call, 700 back to change
    let mut test_block2 = create_block_with_coinbase_tx(block_height);

    let txin = TxIn {
        previous_output: OutPoint {
            txid: test_block.txdata[test_block.txdata.len() - 1].compute_txid(),
            vout: 0,
        },
        script_sig: ScriptBuf::new(),
        sequence: Sequence::MAX,
        witness: Witness::new(),
    };

    // 2 outputs: [txout(change), op_return]
    // Shadow: p0 = 2, p1 = 3
    let address: Address<NetworkChecked> = get_address(&ADDRESS1().as_str());
    let txout = TxOut {
        value: Amount::from_sat(100_000_000),
        script_pubkey: address.script_pubkey(),
    };

    let user_cellpack = Cellpack {
        target: AlkaneId { block: 2, tx: 1 },
        inputs: vec![5],
    };

    let protostones = vec![
        // p0: auto-change — send 300 to p1, send 700 excess back to output 0
        Protostone {
            message: vec![],
            protocol_tag: 1,
            burn: None,
            from: None,
            pointer: Some(3), // p1
            refund: Some(0),
            edicts: vec![
                ProtostoneEdict {
                    id: alkane_id.clone(),
                    amount: 300,
                    output: 3, // to p1
                },
                ProtostoneEdict {
                    id: alkane_id.clone(),
                    amount: 700,
                    output: 0, // excess back to change (physical output 0)
                },
            ],
        },
        // p1: user call
        Protostone {
            message: user_cellpack.encipher(),
            protocol_tag: 1,
            burn: None,
            from: None,
            pointer: Some(0),
            refund: Some(0),
            edicts: vec![],
        },
    ];

    let runestone_script = (Runestone {
        edicts: vec![],
        etching: None,
        mint: None,
        pointer: Some(0),
        protocol: Some(protostones.encipher()?),
    })
    .encipher();

    let op_return = TxOut {
        value: Amount::from_sat(0),
        script_pubkey: runestone_script,
    };

    let tx = Transaction {
        version: Version::ONE,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: vec![txin],
        output: vec![txout, op_return],
    };
    test_block2.txdata.push(tx);

    index_block(&test_block2, block_height)?;

    // Verify: the contract call forwards 300 to output 0,
    // plus the 700 excess from the auto-change edict = 1000 total
    let result_sheet = alkane_helpers::get_last_outpoint_sheet(&test_block2)?;
    println!("Result sheet with excess: {:?}", result_sheet);

    // The contract (opcode 5) forwards incoming tokens to pointer (output 0).
    // So output 0 should get: 700 (excess from p0 edict) + 300 (forwarded by p1 contract) = 1000
    assert_eq!(
        result_sheet.get_cached(&alkane_id),
        1000,
        "Output 0 should receive 700 excess + 300 forwarded = 1000 total"
    );

    Ok(())
}

/// Three-protostone pattern: p0 auto-change routes to p1, p1 has edicts
/// routing to p2, p2 calls contract. Tests deeper nesting.
#[wasm_bindgen_test]
fn test_auto_change_protostone_three_deep() -> Result<()> {
    clear();
    let block_height = 0;

    // Block 0: deploy and mint
    let arb_mint_cellpack = Cellpack {
        target: AlkaneId { block: 1, tx: 0 },
        inputs: vec![30, 2, 1, 500],
    };

    let mut test_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
        [alkanes_std_test_build::get_bytes()].into(),
        [arb_mint_cellpack].into(),
    );
    index_block(&test_block, block_height)?;

    let alkane_id = ProtoruneRuneId { block: 2, tx: 1 };

    // Block 1: three protostones
    let mut test_block2 = create_block_with_coinbase_tx(block_height);

    let txin = TxIn {
        previous_output: OutPoint {
            txid: test_block.txdata[test_block.txdata.len() - 1].compute_txid(),
            vout: 0,
        },
        script_sig: ScriptBuf::new(),
        sequence: Sequence::MAX,
        witness: Witness::new(),
    };

    let address: Address<NetworkChecked> = get_address(&ADDRESS1().as_str());
    let txout = TxOut {
        value: Amount::from_sat(100_000_000),
        script_pubkey: address.script_pubkey(),
    };

    let user_cellpack = Cellpack {
        target: AlkaneId { block: 2, tx: 1 },
        inputs: vec![5],
    };

    // 2 outputs: [txout, op_return]
    // Shadow: p0=2, p1=3, p2=4
    let protostones = vec![
        // p0: auto-change — route all to p1
        Protostone {
            message: vec![],
            protocol_tag: 1,
            burn: None,
            from: None,
            pointer: Some(3), // p1
            refund: Some(0),
            edicts: vec![ProtostoneEdict {
                id: alkane_id.clone(),
                amount: 500,
                output: 3, // to p1
            }],
        },
        // p1: intermediate — no cellpack, routes to p2
        Protostone {
            message: vec![],
            protocol_tag: 1,
            burn: None,
            from: None,
            pointer: Some(4), // p2
            refund: Some(0),
            edicts: vec![ProtostoneEdict {
                id: alkane_id.clone(),
                amount: 500,
                output: 4, // to p2
            }],
        },
        // p2: user call
        Protostone {
            message: user_cellpack.encipher(),
            protocol_tag: 1,
            burn: None,
            from: None,
            pointer: Some(0),
            refund: Some(0),
            edicts: vec![],
        },
    ];

    let runestone_script = (Runestone {
        edicts: vec![],
        etching: None,
        mint: None,
        pointer: Some(0),
        protocol: Some(protostones.encipher()?),
    })
    .encipher();

    let op_return = TxOut {
        value: Amount::from_sat(0),
        script_pubkey: runestone_script,
    };

    let tx = Transaction {
        version: Version::ONE,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: vec![txin],
        output: vec![txout, op_return],
    };
    test_block2.txdata.push(tx);

    index_block(&test_block2, block_height)?;

    let result_sheet = alkane_helpers::get_last_outpoint_sheet(&test_block2)?;
    println!("Result sheet (three-deep): {:?}", result_sheet);
    assert_eq!(
        result_sheet.get_cached(&alkane_id),
        500,
        "All 500 tokens should arrive at output 0 after three-protostone routing"
    );

    Ok(())
}
