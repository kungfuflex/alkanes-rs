use crate::balance_sheet::load_sheet;
use crate::message::{MessageContext, MessageContextParcel};
use crate::test_helpers::{self as helpers};
use crate::{tables, Protorune};
use anyhow::Result;
use bitcoin::{OutPoint, Transaction};
use metashrew::index_pointer::{AtomicPointer, IndexPointer};
use protorune_support::balance_sheet::{BalanceSheet, ProtoruneRuneId};
use protorune_support::rune_transfer::RuneTransfer;
use protorune_support::utils::consensus_encode;

#[allow(unused_imports)]
use metashrew::{
    println,
    stdio::{stdout, Write},
};

use helpers::clear;
use metashrew_support::index_pointer::KeyValuePointer;
use std::str::FromStr;
use wasm_bindgen_test::*;

struct ForwardAll(());

impl MessageContext for ForwardAll {
    fn protocol_tag() -> u128 {
        122
    }
    // takes half of the first runes balance
    fn handle(
        parcel: &MessageContextParcel,
    ) -> Result<(Vec<RuneTransfer>, BalanceSheet<AtomicPointer>)> {
        let runes: Vec<RuneTransfer> = parcel.runes.clone();
        // transfer protorunes to the pointer
        Ok((runes, BalanceSheet::default()))
    }
}

fn protomessages_from_protocol_ids(protocol_ids: Vec<u128>) -> bitcoin::Block {
    let mock_output = OutPoint {
        txid: bitcoin::Txid::from_str(
            "0000000000000000000000000000000000000000000000000000000000000000",
        )
        .unwrap(),
        vout: 0,
    };
    let transaction = helpers::create_multi_protoburn_transaction(mock_output, &protocol_ids);

    let inputs = protocol_ids
        .clone()
        .into_iter()
        .enumerate()
        .map(|(i, _)| OutPoint {
            txid: transaction.compute_txid(),
            vout: i as u32,
        })
        .collect::<Vec<OutPoint>>();
    // output 0 holds all the protorunes
    let mut protoburn_txs: Vec<Transaction> = vec![transaction];
    let protomessage_tx =
        helpers::create_multiple_protomessage_from_edict_tx(inputs, protocol_ids, vec![]);
    protoburn_txs.push(protomessage_tx);
    helpers::create_block_with_txs(protoburn_txs)
}

fn multi_protomessage_protocol_test_template<T: MessageContext>(
    expected_pointer_amount: u128,
    expected_refunded_amount: u128,
    expected_runtime_amount: u128,
) -> (
    BalanceSheet<IndexPointer>,
    BalanceSheet<IndexPointer>,
    BalanceSheet<IndexPointer>,
) {
    clear();
    let block_height = 840000;
    let protocol_ids = vec![122, 123];

    let test_block = protomessages_from_protocol_ids(protocol_ids);
    let protorune_id = ProtoruneRuneId {
        block: block_height as u128,
        tx: 1,
    };

    assert!(Protorune::index_block::<T>(test_block.clone(), block_height as u64).is_ok());
    // print_cache();
    // tx 0 is protoburn, tx 2 is protomessage
    let outpoint_address0: OutPoint = OutPoint {
        txid: test_block.txdata[0].compute_txid(),
        vout: 0,
    };
    let outpoint_address1: OutPoint = OutPoint {
        txid: test_block.txdata[1].compute_txid(),
        vout: 1,
    };
    let outpoint_address2: OutPoint = OutPoint {
        txid: test_block.txdata[1].compute_txid(),
        vout: 2,
    };
    // check runes balance
    let sheet = load_sheet(
        &tables::RUNES
            .OUTPOINT_TO_RUNES
            .select(&consensus_encode(&outpoint_address0).unwrap()),
    );
    let protorunes_sheet0 = load_sheet(
        &tables::RuneTable::for_protocol(122)
            .OUTPOINT_TO_RUNES
            .select(&consensus_encode(&outpoint_address0).unwrap()),
    );
    let protorunes_sheet1 = load_sheet(
        &tables::RuneTable::for_protocol(122)
            .OUTPOINT_TO_RUNES
            .select(&consensus_encode(&outpoint_address1).unwrap()),
    );
    let protorunes_sheet2 = load_sheet(
        &tables::RuneTable::for_protocol(123)
            .OUTPOINT_TO_RUNES
            .select(&consensus_encode(&outpoint_address1).unwrap()),
    );
    println!("{:?}", protorunes_sheet2);
    let protorunes_sheet_runtime =
        load_sheet(&tables::RuneTable::for_protocol(122).RUNTIME_BALANCE);

    let stored_runes_balance = sheet.get_cached(&protorune_id);
    assert_eq!(stored_runes_balance, 0);

    let stored_protorune_balance0 = protorunes_sheet0.get_cached(&protorune_id);
    assert_eq!(stored_protorune_balance0, expected_pointer_amount);
    let stored_protorune_balance1 = protorunes_sheet1.get_cached(&protorune_id);
    assert_eq!(stored_protorune_balance1, expected_refunded_amount);
    let stored_protorune_balance_runtime = protorunes_sheet_runtime.get_cached(&protorune_id);
    assert_eq!(stored_protorune_balance_runtime, expected_runtime_amount);

    return (
        protorunes_sheet0,
        protorunes_sheet1,
        protorunes_sheet_runtime,
    );
}

/// protomessage from edict
/// The first transaction is a protoburn. The next transaction is a protostone that
/// has an edict that targets the protomessage
#[wasm_bindgen_test]
fn multi_protomessage_protocol_test() {
    multi_protomessage_protocol_test_template::<ForwardAll>(1000, 0, 0);
}
