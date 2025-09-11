use crate::message::AlkaneMessageContext;
use crate::tests::helpers as alkane_helpers;
use crate::tests::std::alkanes_std_test_build;
use alkane_helpers::clear;
use alkanes::indexer::index_block;
use alkanes_support::cellpack::Cellpack;
use alkanes_support::envelope::RawEnvelope;
use alkanes_support::id::AlkaneId;
use anyhow::Result;
use bitcoin::address::NetworkChecked;
use bitcoin::{transaction::Version, ScriptBuf, Sequence};
use bitcoin::{Address, Amount, Block, OutPoint, Transaction, TxIn, TxOut, Witness};
use metashrew_core::{
    println,
    stdio::{stdout, Write},
};
use metashrew_support::{index_pointer::KeyValuePointer, utils::consensus_encode};
use ordinals::Runestone;
use protorune::protostone::Protostones;
use protorune::test_helpers::{
    create_block_with_coinbase_tx, get_address, get_btc_network, ADDRESS1,
};
use protorune::view;
use protorune::{
    balance_sheet::load_sheet, message::MessageContext, tables::RuneTable, test_helpers as helpers,
};
use protorune_support::balance_sheet::ProtoruneRuneId;
use protorune_support::protostone::{Protostone, ProtostoneEdict};
use std::str::FromStr;
use wasm_bindgen_test::wasm_bindgen_test;

#[wasm_bindgen_test]
fn test_edict_to_protomessage() -> Result<()> {
    clear();
    let block_height = 0;
    let mut test_block: Block = helpers::create_block_with_coinbase_tx(block_height);
    let tx = Transaction {
        version: Version::ONE,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: vec![TxIn {
            previous_output: OutPoint::null(),
            script_sig: ScriptBuf::new(),
            sequence: Sequence::MAX,
            witness: RawEnvelope::from(alkanes_std_test_build::get_bytes()).to_witness(true),
        }],
        output: vec![
            TxOut {
                script_pubkey: Address::from_str(ADDRESS1().as_str())
                    .unwrap()
                    .require_network(get_btc_network())
                    .unwrap()
                    .script_pubkey(),
                value: Amount::from_sat(100),
            },
            TxOut {
                script_pubkey: Address::from_str(ADDRESS1().as_str())
                    .unwrap()
                    .require_network(get_btc_network())
                    .unwrap()
                    .script_pubkey(),
                value: Amount::from_sat(100),
            },
            TxOut {
                script_pubkey: (Runestone {
                    edicts: vec![],
                    etching: None,
                    mint: None,
                    pointer: None,
                    protocol: Some(
                        vec![
                            Protostone {
                                message: vec![1, 0, 4],
                                protocol_tag: 1,
                                from: None,
                                burn: None,
                                pointer: Some(6),
                                refund: Some(6),
                                edicts: vec![],
                            },
                            Protostone {
                                message: vec![1, 0, 4],
                                protocol_tag: 1,
                                from: None,
                                burn: None,
                                refund: Some(6),
                                pointer: Some(6),
                                edicts: vec![],
                            },
                            Protostone {
                                message: vec![],
                                protocol_tag: 1,
                                burn: None,
                                from: None,
                                refund: Some(7),
                                pointer: Some(7),
                                edicts: vec![ProtostoneEdict {
                                    id: ProtoruneRuneId { block: 2, tx: 1 },
                                    amount: 100,
                                    output: 0,
                                }],
                            },
                            Protostone {
                                message: vec![2, 1, 3],
                                protocol_tag: 1,
                                from: None,
                                pointer: Some(1),
                                burn: None,
                                refund: Some(1),
                                edicts: vec![],
                            },
                        ]
                        .encipher()?,
                    ),
                })
                .encipher(),
                value: Amount::from_sat(0),
            },
        ],
    };
    test_block.txdata.push(tx);
    index_block(&test_block, block_height)?;
    let edict_outpoint = OutPoint {
        txid: test_block.txdata[test_block.txdata.len() - 1].compute_txid(),
        vout: 0,
    };
    let result_outpoint = OutPoint {
        txid: test_block.txdata[test_block.txdata.len() - 1].compute_txid(),
        vout: 1,
    };
    let edict_sheet = load_sheet(
        &RuneTable::for_protocol(AlkaneMessageContext::protocol_tag())
            .OUTPOINT_TO_RUNES
            .select(&consensus_encode(&edict_outpoint)?),
    );
    let sheet = load_sheet(
        &RuneTable::for_protocol(AlkaneMessageContext::protocol_tag())
            .OUTPOINT_TO_RUNES
            .select(&consensus_encode(&result_outpoint)?),
    );
    println!("edict sheet: {:?}", edict_sheet);
    println!("output sheet: {:?}", sheet);
    Ok(())
}

#[wasm_bindgen_test]
fn test_edict_message_same_protostone() -> Result<()> {
    clear();
    let block_height = 0;

    // Create a cellpack to call the process_numbers method (opcode 11)
    let arb_mint_cellpack = Cellpack {
        target: AlkaneId { block: 1, tx: 0 },
        inputs: vec![30, 2, 1, 1],
    };

    // Initialize the contract and execute the cellpacks
    let mut test_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
        [alkanes_std_test_build::get_bytes()].into(),
        [arb_mint_cellpack].into(),
    );

    index_block(&test_block, block_height)?;

    let mut test_block2 = create_block_with_coinbase_tx(block_height);

    let input_script = ScriptBuf::new();
    let txin1 = TxIn {
        previous_output: OutPoint {
            txid: test_block.txdata[test_block.txdata.len() - 1].compute_txid(),
            vout: 0,
        },
        script_sig: input_script.clone(),
        sequence: Sequence::MAX,
        witness: Witness::new(),
    };

    test_block2.txdata.push(
        alkane_helpers::create_multiple_cellpack_with_witness_and_txins_edicts(
            [Cellpack {
                target: AlkaneId { block: 2, tx: 1 },
                inputs: vec![5],
            }]
            .into(),
            vec![txin1],
            false,
            vec![ProtostoneEdict {
                id: ProtoruneRuneId { block: 2, tx: 1 },
                amount: 1,
                output: 0,
            }],
        ),
    );

    index_block(&test_block2, block_height)?;

    let sheet = alkane_helpers::get_last_outpoint_sheet(&test_block2)?;

    println!("Last sheet: {:?}", sheet);

    assert_eq!(sheet.get_cached(&ProtoruneRuneId { block: 2, tx: 1 }), 1);

    Ok(())
}

#[wasm_bindgen_test]
fn test_edict_message_same_protostone_2() -> Result<()> {
    clear();
    let block_height = 880000;

    // Create a cellpack to call the process_numbers method (opcode 11)
    let arb_mint_cellpack = Cellpack {
        target: AlkaneId { block: 1, tx: 0 },
        inputs: vec![22, 10000000000],
    };

    let gen_1_init = Cellpack {
        target: AlkaneId { block: 3, tx: 0 },
        inputs: vec![0],
    };

    let alkamon_init = Cellpack {
        target: AlkaneId { block: 3, tx: 1 },
        inputs: vec![
            0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 4, 0, 1, 1, 3, 3, 2, 1, 2, 2, 1, 2, 100, 1, 1, 1, 1, 1, 1,
        ],
    };

    let gen_1_build = include_bytes!("/Users/kevinyao/Documents/Code/alkamon/target/alkanes/wasm32-unknown-unknown/release/alkamon_gen_1.wasm").to_vec();

    let alkamon_build = include_bytes!("/Users/kevinyao/Documents/Code/alkamon/target/alkanes/wasm32-unknown-unknown/release/alkane_alkamon_child.wasm").to_vec();

    // Initialize the contract and execute the cellpacks
    let mut test_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
        [
            gen_1_build,
            alkamon_build,
            alkanes_std_test_build::get_bytes(),
            alkanes_std_test_build::get_bytes(),
        ]
        .into(),
        [
            gen_1_init,
            alkamon_init,
            arb_mint_cellpack.clone(),
            arb_mint_cellpack.clone(),
        ]
        .into(),
    );

    index_block(&test_block, block_height)?;

    let mut test_block2 = create_block_with_coinbase_tx(block_height);

    let txin1 = TxIn {
        previous_output: OutPoint {
            txid: test_block.txdata[test_block.txdata.len() - 1].compute_txid(),
            vout: 0,
        },
        script_sig: ScriptBuf::new(),
        sequence: Sequence::MAX,
        witness: Witness::new(),
    };

    let protocol_id = 1;
    let protostone: Vec<Protostone> = vec![
        Protostone {
            message: Cellpack {
                target: AlkaneId { block: 4, tx: 1 },
                inputs: vec![21, 8],
            }
            .encipher(),
            pointer: Some(0),
            refund: Some(0),
            edicts: vec![
                ProtostoneEdict {
                    id: ProtoruneRuneId { block: 4, tx: 1 },
                    amount: 0,
                    output: 0,
                },
                ProtostoneEdict {
                    id: ProtoruneRuneId { block: 2, tx: 2 },
                    amount: 0,
                    output: 1,
                },
                ProtostoneEdict {
                    id: ProtoruneRuneId { block: 2, tx: 1 },
                    amount: 0,
                    output: 0,
                },
            ],
            from: None,
            burn: None,
            protocol_tag: protocol_id as u128,
        },
        // Protostone {
        //     message: Cellpack {
        //         target: AlkaneId { block: 2, tx: 1 },
        //         inputs: vec![3],
        //     }
        //     .encipher(),
        //     pointer: Some(0),
        //     refund: Some(0),
        //     edicts: vec![
        //         ProtostoneEdict {
        //             id: ProtoruneRuneId { block: 2, tx: 1 },
        //             amount: 0,
        //             output: 2,
        //         },
        //         ProtostoneEdict {
        //             id: ProtoruneRuneId { block: 2, tx: 3 },
        //             amount: 0,
        //             output: 0,
        //         },
        //     ],
        //     from: None,
        //     burn: None,
        //     protocol_tag: protocol_id as u128,
        // },
    ];
    let runestone: ScriptBuf = (Runestone {
        etching: None,
        pointer: Some(0),
        edicts: Vec::new(),
        mint: None,
        protocol: protostone.encipher().ok(),
    })
    .encipher();

    //     // op return is at output 1
    let op_return = TxOut {
        value: Amount::from_sat(0),
        script_pubkey: runestone,
    };
    let address: Address<NetworkChecked> = get_address(&ADDRESS1().as_str());

    let script_pubkey = address.script_pubkey();
    let my_txout = TxOut {
        value: Amount::from_sat(100_000_000),
        script_pubkey,
    };
    test_block2.txdata.push(Transaction {
        version: Version::ONE,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: vec![txin1],
        output: vec![
            my_txout.clone(),
            my_txout.clone(),
            my_txout.clone(),
            my_txout.clone(),
            op_return,
        ],
    });

    index_block(&test_block2, block_height)?;

    let view0 = view::protorune_outpoint_to_outpoint_response(
        &OutPoint {
            txid: test_block2.txdata.last().unwrap().compute_txid(),
            vout: 0,
        },
        1,
    );
    println!("view0 {:?}", view0);

    let view1 = view::protorune_outpoint_to_outpoint_response(
        &OutPoint {
            txid: test_block2.txdata.last().unwrap().compute_txid(),
            vout: 1,
        },
        1,
    );
    println!("view1 {:?}", view1);

    let sheet = alkane_helpers::get_last_outpoint_sheet(&test_block2)?;
    let txnum = test_block2.txdata.len() - 1;

    let sheet1 = alkane_helpers::get_sheet_for_outpoint(&test_block2, txnum, 1)?;
    let sheet2 = alkane_helpers::get_sheet_for_outpoint(&test_block2, txnum, 2)?;

    assert_eq!(
        sheet.get_cached(&ProtoruneRuneId { block: 2, tx: 1 }),
        10000000000
    );

    assert_eq!(
        sheet1.get_cached(&ProtoruneRuneId { block: 2, tx: 2 }),
        9999999999
    );

    Ok(())
}

#[wasm_bindgen_test]
fn test_edict_message_same_protostone_revert() -> Result<()> {
    clear();
    let block_height = 0;

    // Create a cellpack to call the process_numbers method (opcode 11)
    let arb_mint_cellpack = Cellpack {
        target: AlkaneId { block: 1, tx: 0 },
        inputs: vec![30, 2, 1, 1],
    };

    // Initialize the contract and execute the cellpacks
    let mut test_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
        [alkanes_std_test_build::get_bytes()].into(),
        [arb_mint_cellpack].into(),
    );

    index_block(&test_block, block_height)?;

    let mut test_block2 = create_block_with_coinbase_tx(0);

    let input_script = ScriptBuf::new();
    let txin1 = TxIn {
        previous_output: OutPoint {
            txid: test_block.txdata[test_block.txdata.len() - 1].compute_txid(),
            vout: 0,
        },
        script_sig: input_script.clone(),
        sequence: Sequence::MAX,
        witness: Witness::new(),
    };

    test_block2.txdata.push(
        alkane_helpers::create_multiple_cellpack_with_witness_and_txins_edicts(
            [Cellpack {
                target: AlkaneId { block: 2, tx: 1 },
                inputs: vec![100], //revert
            }]
            .into(),
            vec![txin1],
            false,
            vec![ProtostoneEdict {
                id: ProtoruneRuneId { block: 2, tx: 1 },
                amount: 1,
                output: 0,
            }],
        ),
    );

    index_block(&test_block2, block_height)?;

    let sheet = alkane_helpers::get_last_outpoint_sheet(&test_block2)?;

    println!("Last sheet: {:?}", sheet);

    assert_eq!(sheet.get_cached(&ProtoruneRuneId { block: 2, tx: 1 }), 1);

    Ok(())
}
