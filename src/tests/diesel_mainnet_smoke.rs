//! End-to-end smoke test for the mainnet precompile dispatch.
//!
//! Runs only when `mainnet` is enabled. The precompile is always
//! compiled in (no longer feature-gated), but its dispatch is height-
//! gated: it fires only at heights >= V220_FORK_HEIGHT on mainnet.
//! This test mints at SMOKE_HEIGHT > V220_FORK_HEIGHT so that
//! `index_block` routes the call through `run_diesel_eoa` natively
//! (no wasmi), then asserts the balance sheet at the mint output
//! contains the expected value_per_mint amount.
//!
//! This is the consensus-relevant E2E gate: if the precompile produces
//! an incorrect storage write or transfer amount at the fork height,
//! this test fails.

#![cfg(feature = "mainnet")]

use crate::index_block;
use crate::message::AlkaneMessageContext;
use crate::tests::helpers::{self as alkane_helpers, clear};
use crate::tests::std::alkanes_std_auth_token_build;
use alkanes_support::cellpack::Cellpack;
use alkanes_support::constants::AUTH_TOKEN_FACTORY_ID;
use alkanes_support::id::AlkaneId;
use anyhow::Result;
use bitcoin::{OutPoint, Witness};
use metashrew_core::index_pointer::IndexPointer;
use metashrew_support::index_pointer::KeyValuePointer;
use metashrew_support::utils::consensus_encode;
use protorune::balance_sheet::load_sheet;
use protorune::message::MessageContext;
use protorune::tables::RuneTable;
use protorune::test_helpers::create_block_with_coinbase_tx;
use protorune_support::balance_sheet::{BalanceSheetOperations, ProtoruneRuneId};
use wasm_bindgen_test::wasm_bindgen_test;

#[allow(unused_imports)]
use metashrew_core::{print, println, stdio::{stdout, Write}};

const DIESEL: AlkaneId = AlkaneId { block: 2, tx: 0 };
// Post-V220_FORK_HEIGHT (950_000): the precompile auto-activates here, so
// the wasmi-bypass path is exercised. SMOKE_HEIGHT - 1 is used for setup
// (it indexes a coinbase-only block to deploy DIESEL state) — that's
// still post-fork so we don't have to switch binaries during the test.
const SMOKE_HEIGHT: u32 = 950_001;

fn setup() -> Result<()> {
    let auth_cellpack = Cellpack {
        target: AlkaneId {
            block: 3,
            tx: AUTH_TOKEN_FACTORY_ID,
        },
        inputs: vec![100],
    };
    let block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
        [alkanes_std_auth_token_build::get_bytes()].into(),
        [auth_cellpack].into(),
    );
    index_block(&block, SMOKE_HEIGHT)?;
    Ok(())
}

#[wasm_bindgen_test]
fn diesel_mainnet_precompile_smoke() -> Result<()> {
    clear();
    setup()?;

    // Force /upgrade_initialized so mint takes the communist branch.
    let mut up_ptr = IndexPointer::default()
        .keyword("/alkanes/")
        .select(&DIESEL.into())
        .keyword("/storage/")
        .select(&b"/upgrade_initialized".to_vec());
    up_ptr.set_value::<u8>(0x01);

    // Build a 3-mint block at SMOKE_HEIGHT + 1. The precompile is gated on
    // matches_precompile_for_ctx, which requires exactly one mint protostone
    // per tx — satisfied by build_mint_block.
    let h = SMOKE_HEIGHT + 1;
    let mut block = create_block_with_coinbase_tx(h);
    let cb_txid = block.txdata[0].compute_txid();
    let mint = Cellpack {
        target: DIESEL,
        inputs: vec![77],
    };
    for i in 0..3usize {
        let tx = alkane_helpers::create_multiple_cellpack_with_witness_and_in(
            Witness::new(),
            vec![mint.clone()],
            OutPoint::new(cb_txid, i as u32),
            false,
        );
        block.txdata.push(tx);
    }

    index_block(&block, h)?;

    // Compute expected value_per_mint:
    //   total_mints = 3
    //   block_reward(h=925_001) = 50e8 / (1 << (h / 210000))
    //                           = 50e8 / (1 << 4) = 312_500_000
    //   coinbase output = 10 BTC = 1_000_000_000 sats > block_reward
    //   total_tx_fee = miner_fee - reward = 1_000_000_000 - 312_500_000
    //                = 687_500_000
    //   diesel_fee = min(reward/2, total_tx_fee) = 156_250_000
    //   value_per_mint = (reward - diesel_fee) / 3
    //                  = 156_250_000 / 3 = 52_083_333
    let reward: u128 = (50e8 as u128) / (1u128 << ((h as u128) / 210000u128));
    let miner_fee: u128 = 1_000_000_000; // 10 BTC coinbase from test harness
    let total_tx_fee = if miner_fee > reward { miner_fee - reward } else { 0 };
    let diesel_fee = std::cmp::min(reward / 2, total_tx_fee);
    let expected_value_per_mint = (reward - diesel_fee) / 3;
    println!(
        "expected value_per_mint at h={}: reward={}, value_per_mint={}",
        h, reward, expected_value_per_mint
    );

    // Diagnostic: how many mint protostones does count_mint_protostones_in_tx
    // and number_diesel_mints actually see in this block?
    use ordinals::{Artifact, Runestone};
    use protorune_support::protostone::Protostone;
    let mut total_protostones = 0u32;
    let mut mint_protostones = 0u32;
    for (idx, tx) in block.txdata.iter().enumerate() {
        let mut per_tx = 0;
        if let Some(Artifact::Runestone(ref runestone)) = Runestone::decipher(tx) {
            let ps = Protostone::from_runestone(runestone).unwrap_or_default();
            for p in &ps {
                total_protostones += 1;
                if p.protocol_tag == 1 && !p.message.is_empty() {
                    per_tx += 1;
                }
            }
            let n = ps.len();
            println!("  tx[{}] protostones={} mint-ish={}", idx, n, per_tx);
        } else {
            println!("  tx[{}] no runestone", idx);
        }
        mint_protostones += per_tx;
    }
    println!(
        "block has {} total protostones, {} mint-shaped (target=2:0/op=77)",
        total_protostones, mint_protostones
    );

    // Inspect the balance sheet at each mint's output (vout 3, the
    // protostone target output). Each mint tx should hold value_per_mint
    // of DIESEL after the index_block call.
    let diesel_id = ProtoruneRuneId { block: 2, tx: 0 };
    for (i, tx) in block.txdata.iter().enumerate().skip(1) {
        // Try vouts 0..=4: protostone pointer:0 directs to vout 0; OP_RETURN
        // sits at the last output; the helper places the runestone at the
        // end. Inspect all outputs to find where the mint landed.
        let mut found = 0u128;
        let mut found_vout = u32::MAX;
        for vout in 0..=4u32 {
            let outpoint = OutPoint {
                txid: tx.compute_txid(),
                vout,
            };
            let sheet = load_sheet(
                &RuneTable::for_protocol(AlkaneMessageContext::protocol_tag())
                    .OUTPOINT_TO_RUNES
                    .select(&consensus_encode(&outpoint)?),
            );
            let amount = sheet.get(&diesel_id);
            if amount > 0 {
                found = amount;
                found_vout = vout;
                break;
            }
        }
        println!(
            "tx {} ({}): DIESEL amount = {} @ vout {}",
            i,
            tx.compute_txid(),
            found,
            found_vout
        );
        assert_eq!(
            found, expected_value_per_mint,
            "tx {} mint amount mismatch: precompile produced {}, expected {}",
            i, found, expected_value_per_mint
        );
    }

    // Also verify total supply was updated correctly.
    let ts_ptr = IndexPointer::default()
        .keyword("/alkanes/")
        .select(&DIESEL.into())
        .keyword("/storage/")
        .select(&b"/totalsupply".to_vec());
    let ts_bytes = ts_ptr.get();
    let total_supply = u128::from_le_bytes(ts_bytes.as_ref().as_slice().try_into()?);
    let expected_ts_delta = 3u128 * expected_value_per_mint;
    println!(
        "totalsupply after block: {} (expected delta >= {})",
        total_supply, expected_ts_delta
    );
    assert!(
        total_supply >= expected_ts_delta,
        "totalsupply not incremented by expected mint amount"
    );
    Ok(())
}
