//! testnet4 (`--features testnet`): every consensus rule is live from block 0.
//!
//! Locks in the testnet genesis module: no height-gated activation is left
//! pending, `2:0` resolves to the testnet DIESEL v3 build (V3_ACTIVATION_HEIGHT
//! = 0) at every height, and the genesis block initializes v3 directly (its
//! premine lands on the genesis outpoint, exactly as on regtest).
#![cfg(feature = "testnet")]

use crate::index_block;
use crate::network::{
    diesel_v3_bytes, genesis, genesis_alkane_wasm_for_height, precompiled_alkane_wasm_for_height,
};
use crate::precompiled::{alkanes_std_diesel_v3_regtest_build, alkanes_std_diesel_v3_testnet_build};
use crate::tests::helpers::clear;
use alkanes::message::AlkaneMessageContext;
use alkanes_support::id::AlkaneId;
use anyhow::Result;
use bitcoin::hashes::Hash;
use bitcoin::{OutPoint, Txid};
use metashrew_support::index_pointer::KeyValuePointer;
use protorune::balance_sheet::load_sheet;
use protorune::message::MessageContext;
use protorune::tables::RuneTable;
use protorune::test_helpers::create_block_with_coinbase_tx;
use protorune_support::balance_sheet::{BalanceSheetOperations, ProtoruneRuneId};
use protorune_support::utils::consensus_encode;
use wasm_bindgen_test::wasm_bindgen_test;

#[wasm_bindgen_test]
fn testnet_has_no_pending_activation() {
    assert_eq!(genesis::GENESIS_BLOCK, 0);
    assert_eq!(genesis::GENESIS_OUTPOINT_BLOCK_HEIGHT, 0);
    assert_eq!(genesis::GENESIS_UPGRADE_BLOCK_HEIGHT, 0);
    assert_eq!(genesis::GENESIS_UPGRADE_EOA_BLOCK_HEIGHT, 0);
    assert_eq!(genesis::DIESEL_V3_BLOCK_HEIGHT, 0);
    assert_eq!(genesis::V220_FORK_HEIGHT, 0);
    assert_eq!(genesis::FRBTC_V130_FORK_HEIGHT, 0);
    assert_eq!(genesis::FRBTC_V131_FORK_HEIGHT, 0);
    assert_eq!(protorune::POST_AUDIT_FORK_HEIGHT, 0);
    assert_eq!(protorune::V217_FIX_HEIGHT, 0);
    assert!(protorune::post_audit_fork_active(0));
}

#[wasm_bindgen_test]
fn testnet_2_0_is_diesel_v3_testnet_build_from_genesis() {
    let v3 = alkanes_std_diesel_v3_testnet_build::get_bytes();
    assert_ne!(v3, alkanes_std_diesel_v3_regtest_build::get_bytes());
    assert_eq!(diesel_v3_bytes(), v3);
    for h in [0u32, 1, 50_000, 156_000, 2_000_000] {
        assert_eq!(genesis_alkane_wasm_for_height(h), v3, "height {}", h);
        assert_eq!(
            precompiled_alkane_wasm_for_height(&AlkaneId { block: 2, tx: 0 }, h),
            Some(v3.clone())
        );
    }
}

#[wasm_bindgen_test]
fn testnet_genesis_block_initializes_v3() -> Result<()> {
    clear();
    let block = create_block_with_coinbase_tx(0);
    index_block(&block, 0)?;
    let outpoint = OutPoint {
        txid: Txid::from_byte_array(
            hex::decode(genesis::GENESIS_OUTPOINT)?
                .into_iter()
                .rev()
                .collect::<Vec<u8>>()
                .try_into()
                .unwrap(),
        ),
        vout: 0,
    };
    let sheet = load_sheet(
        &RuneTable::for_protocol(AlkaneMessageContext::protocol_tag())
            .OUTPOINT_TO_RUNES
            .select(&consensus_encode(&outpoint)?),
    );
    assert_eq!(sheet.get(&ProtoruneRuneId { block: 2, tx: 0 }), 50_000_000u128);
    // a second block indexes cleanly on top
    index_block(&create_block_with_coinbase_tx(1), 1)?;
    Ok(())
}
