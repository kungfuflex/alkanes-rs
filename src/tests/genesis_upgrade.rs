use crate::index_block;
use crate::tests::helpers::{
    self as alkane_helpers, get_last_outpoint_sheet, get_sheet_for_outpoint,
};
use crate::tests::std::{alkanes_std_auth_token_build, alkanes_std_genesis_alkane_upgrade_build};
use alkane_helpers::clear;
use alkanes::view;
use alkanes_support::cellpack::Cellpack;
use alkanes_support::constants::AUTH_TOKEN_FACTORY_ID;
use alkanes_support::id::AlkaneId;
use anyhow::Result;
use bitcoin::{OutPoint, Witness};

use crate::network::genesis;
use alkanes::message::AlkaneMessageContext;
use bitcoin::hashes::Hash;
use bitcoin::Block;
#[allow(unused_imports)]
use metashrew_core::{get_cache, index_pointer::IndexPointer};
#[allow(unused_imports)]
use metashrew_core::{
    println,
    stdio::{stdout, Write},
};
use metashrew_support::index_pointer::KeyValuePointer;
use protorune::test_helpers::{create_block_with_coinbase_tx, create_protostone_encoded_tx};
use protorune::view::protorune_outpoint_to_outpoint_response;
use protorune::{balance_sheet::load_sheet, message::MessageContext, tables::RuneTable};
use protorune_support::balance_sheet::{BalanceSheet, BalanceSheetOperations, ProtoruneRuneId};
use protorune_support::protostone::Protostone;
use protorune_support::utils::consensus_encode;
use wasm_bindgen_test::wasm_bindgen_test;

fn setup_pre_upgrade() -> Result<()> {
    let auth_cellpack = Cellpack {
        target: AlkaneId {
            block: 3,
            tx: AUTH_TOKEN_FACTORY_ID,
        },
        inputs: vec![100],
    };

    // Initialize the contract and execute the cellpacks
    let test_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
        [alkanes_std_auth_token_build::get_bytes()].into(),
        [auth_cellpack].into(),
    );
    index_block(&test_block, 0)?; // just to init the diesel
    Ok(())
}

fn mint_after_upgrade(num_mints: usize) -> Result<Block> {
    let block_height = 890_000;
    let diesel = AlkaneId { block: 2, tx: 0 };

    let mint = Cellpack {
        target: diesel.clone(),
        inputs: vec![77],
    };

    // Initialize the contract and execute the cellpacks
    let mut test_block = create_block_with_coinbase_tx(block_height);
    let mint_tx = alkane_helpers::create_multiple_cellpack_with_witness_and_in(
        Witness::new(),
        vec![mint.clone()],
        OutPoint::default(),
        false,
    );

    for i in 1..=num_mints {
        test_block.txdata.push(mint_tx.clone());
    }

    index_block(&test_block, block_height)?;
    Ok(test_block)
}

#[wasm_bindgen_test]
fn test_new_genesis_contract() -> Result<()> {
    clear();
    setup_pre_upgrade()?;
    let num_mints = 5;
    let test_block = mint_after_upgrade(num_mints)?;
    let diesel = AlkaneId { block: 2, tx: 0 };

    for i in 1..=num_mints {
        let sheet = get_sheet_for_outpoint(&test_block, i, 0)?;
        assert_eq!(
            sheet.get(&diesel.clone().into()),
            ((312500000 - (350000000 - 312500000)) / num_mints)
                .try_into()
                .unwrap(),
        )
    }
    Ok(())
}

#[wasm_bindgen_test]
fn test_new_genesis_collect_fees() -> Result<()> {
    use bitcoin::Txid;
    clear();
    setup_pre_upgrade()?;
    mint_after_upgrade(5)?;
    let outpoint = OutPoint {
        txid: Txid::from_byte_array(
            <Vec<u8> as AsRef<[u8]>>::as_ref(
                &hex::decode(genesis::GENESIS_OUTPOINT)?
                    .iter()
                    .cloned()
                    .rev()
                    .collect::<Vec<u8>>(),
            )
            .try_into()?,
        ),
        vout: 0,
    };
    // Check final balances
    let ptr = RuneTable::for_protocol(AlkaneMessageContext::protocol_tag())
        .OUTPOINT_TO_RUNES
        .select(&consensus_encode(&outpoint)?);
    let sheet = load_sheet(&ptr);

    let genesis_id = ProtoruneRuneId { block: 2, tx: 0 };
    let auth_token = ProtoruneRuneId { block: 2, tx: 1 };
    assert_eq!(sheet.get(&auth_token), 5);
    let out = protorune_outpoint_to_outpoint_response(&outpoint, 1)?;
    let out_sheet: BalanceSheet<IndexPointer> = out.into();
    assert_eq!(sheet, out_sheet);

    // make sure premine is spendable
    let block_height = 890_001;
    let mut spend_block = create_block_with_coinbase_tx(block_height);
    let collect_tx = alkane_helpers::create_multiple_cellpack_with_witness_and_in(
        Witness::new(),
        vec![Cellpack {
            target: genesis_id.clone().into(),
            inputs: vec![78],
        }],
        outpoint.clone(),
        false,
    );
    spend_block.txdata.push(collect_tx.clone());
    index_block(&spend_block, block_height)?;
    let new_outpoint = OutPoint {
        txid: collect_tx.compute_txid(),
        vout: 0,
    };
    let new_ptr = RuneTable::for_protocol(AlkaneMessageContext::protocol_tag())
        .OUTPOINT_TO_RUNES
        .select(&consensus_encode(&new_outpoint)?);
    let new_sheet = load_sheet(&new_ptr);

    let genesis_id = ProtoruneRuneId { block: 2, tx: 0 };
    assert_eq!(new_sheet.get(&genesis_id), 350000000 - 312500000);
    Ok(())
}
