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
use alkanes_support::trace::{Trace, TraceEvent};
use anyhow::Result;
use bitcoin::block::Header;
use bitcoin::Transaction;
use bitcoin::{OutPoint, Witness};

#[allow(unused_imports)]
use metashrew_core::{
    println,
    stdio::{stdout, Write},
};
use protorune::test_helpers::create_block_with_coinbase_tx;
use protorune::test_helpers::create_coinbase_transaction;
use protorune_support::balance_sheet::BalanceSheetOperations;
use protorune_support::utils::consensus_decode;
use wasm_bindgen_test::wasm_bindgen_test;

#[wasm_bindgen_test]
fn test_new_genesis_contract() -> Result<()> {
    clear();

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

    let num_mints = 5;

    for i in 1..=num_mints {
        test_block.txdata.push(mint_tx.clone());
    }

    index_block(&test_block, block_height)?;
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
