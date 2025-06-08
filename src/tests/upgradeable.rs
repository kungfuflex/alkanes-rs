use crate::index_block;
use crate::tests::helpers::{self as alkane_helpers};
use crate::tests::std::{
    alkanes_std_auth_token_build, alkanes_std_test_build, alkanes_std_upgradeable_build,
};
use alkane_helpers::clear;
use alkanes::view;
use alkanes_support::id::AlkaneId;
use alkanes_support::trace::{Trace, TraceEvent};
use alkanes_support::{cellpack::Cellpack, constants::AUTH_TOKEN_FACTORY_ID};
use anyhow::Result;
use bitcoin::block::Header;
use bitcoin::OutPoint;
use bitcoin::Transaction;
#[allow(unused_imports)]
use metashrew_core::{
    println,
    stdio::{stdout, Write},
};
use protorune::test_helpers::create_coinbase_transaction;
use protorune_support::balance_sheet::ProtoruneRuneId;
use protorune_support::utils::consensus_decode;
use wasm_bindgen_test::wasm_bindgen_test;

#[wasm_bindgen_test]
fn test_upgradeability() -> Result<()> {
    clear();
    let block_height = 840_000;
    let auth_cellpack = Cellpack {
        target: AlkaneId {
            block: 3,
            tx: AUTH_TOKEN_FACTORY_ID,
        },
        inputs: vec![100],
    };
    let test = Cellpack {
        target: AlkaneId { block: 1, tx: 0 },
        inputs: vec![50],
    };
    let upgrade = Cellpack {
        target: AlkaneId { block: 1, tx: 0 },
        inputs: vec![0x7fff, 2, 1, 1],
    };
    let mint = Cellpack {
        target: AlkaneId { block: 2, tx: 2 },
        inputs: vec![0x7ffd, 2, 22, 1_000_000],
    };

    // Initialize the contract and execute the cellpacks
    let mut test_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
        [
            alkanes_std_auth_token_build::get_bytes(),
            alkanes_std_test_build::get_bytes(),
            alkanes_std_upgradeable_build::get_bytes(),
            [].into(),
        ]
        .into(),
        [auth_cellpack, test, upgrade, mint].into(),
    );

    index_block(&test_block, block_height)?;

    let sheet = alkane_helpers::get_last_outpoint_sheet(&test_block)?;
    assert_eq!(
        sheet.get_cached(&ProtoruneRuneId { block: 2, tx: 2 }),
        1_000_000
    );
    assert_eq!(sheet.get_cached(&ProtoruneRuneId { block: 2, tx: 1 }), 0);

    Ok(())
}
