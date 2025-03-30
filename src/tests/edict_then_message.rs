use crate::message::AlkaneMessageContext;
use crate::tests::helpers as alkane_helpers;
use crate::tests::std::alkanes_std_test_build;
use alkane_helpers::clear;
use alkanes::indexer::index_block;
use alkanes_support::envelope::RawEnvelope;
use alkanes_support::id::AlkaneId;
use anyhow::Result;
use bitcoin::address::NetworkChecked;
use bitcoin::{transaction::Version, ScriptBuf, Sequence};
use bitcoin::{Address, Amount, Block, OutPoint, Transaction, TxIn, TxOut};
use metashrew_core::{
    println,
    stdio::{stdout, Write},
};
use metashrew_support::{index_pointer::KeyValuePointer, utils::consensus_encode};
use ordinals::Runestone;
use protorune::protostone::Protostones;
use protorune::test_helpers::{get_btc_network, ADDRESS1};
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
    let block_height = 840_000;
    let mut test_block: Block = helpers::create_block_with_coinbase_tx(block_height);
    let tx = Transaction {
        version: Version::ONE,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: vec![TxIn {
            previous_output: OutPoint::null(),
            script_sig: ScriptBuf::new(),
            sequence: Sequence::MAX,
            witness: RawEnvelope::from(alkanes_std_test_build::get_bytes()).to_gzipped_witness(),
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
