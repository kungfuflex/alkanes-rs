//! PoC for the unchecked `+` in `transfer_from` (src/utils.rs:138).
//!
//! `credit_balances` credits a KV inventory with `checked_add` (graceful Err on
//! overflow). But `transfer_from` — the credit path used by every extcall
//! (`host_functions::extcall` incoming, and `Saveable::save` return) — credits
//! with a RAW `+`:
//!
//!     to_pointer.set_value::<u128>(to_pointer.get_value::<u128>() + transfer.value);
//!
//! In a debug/test build (overflow-checks on) this PANICS, which — because a
//! panic aborts `index_block` and metashrew retries the same block forever —
//! wedges the whole indexer on a crafted transaction. In a release build
//! (overflow-checks off, per [profile.release]) it silently WRAPS, corrupting
//! the balance (a conservation/consensus break). Either way it must instead be
//! a graceful per-tx revert.
//!
//! ATTACK: a contract C forwards `C:MAX` twice, in a single tx, into a second
//! contract D that hoards its incoming (opcode 7 = donate). The first forward
//! credits D with C:MAX; the second computes `MAX + MAX` on D's balance.
//! The two forwards are funded by C self-minting itself (allowed), so no real
//! supply is needed.

use crate::index_block;
use crate::message::AlkaneMessageContext;
use crate::tests::helpers as alkane_helpers;
use crate::tests::std::alkanes_std_test_build;
use alkane_helpers::clear;
use alkanes_support::cellpack::Cellpack;
use alkanes_support::id::AlkaneId;
use anyhow::Result;
use bitcoin::{OutPoint, Witness};
use metashrew_core::index_pointer::IndexPointer;
#[allow(unused_imports)]
use metashrew_core::{
    println,
    stdio::{stdout, Write},
};
use metashrew_support::index_pointer::KeyValuePointer;
use metashrew_support::utils::consensus_encode;
use protorune::balance_sheet::load_sheet;
use protorune::message::MessageContext;
use protorune::tables::RuneTable;
use protorune::test_helpers::create_block_with_coinbase_tx;
use protorune_support::balance_sheet::{BalanceSheet, BalanceSheetOperations, ProtoruneRuneId};
use wasm_bindgen_test::wasm_bindgen_test;

const MAX: u128 = u128::MAX;

fn outpoint_sheet(outpoint: &OutPoint) -> BalanceSheet<IndexPointer> {
    let ptr = RuneTable::for_protocol(AlkaneMessageContext::protocol_tag())
        .OUTPOINT_TO_RUNES
        .select(&consensus_encode(outpoint).unwrap());
    load_sheet(&ptr)
}

fn rune_of(id: &AlkaneId) -> ProtoruneRuneId {
    ProtoruneRuneId {
        block: id.block,
        tx: id.tx,
    }
}

fn find_minted_id(outpoint: &OutPoint, amount: u128) -> AlkaneId {
    let sheet = outpoint_sheet(outpoint);
    let id = sheet
        .balances()
        .iter()
        .find(|(_, b)| **b == amount)
        .map(|(id, _)| *id)
        .unwrap_or_else(|| panic!("no token with balance {} at outpoint", amount));
    AlkaneId {
        block: id.block,
        tx: id.tx,
    }
}

fn alkane_inventory(holder: AlkaneId, token: AlkaneId) -> u128 {
    let token_bytes: Vec<u8> = token.into();
    let holder_bytes: Vec<u8> = holder.into();
    IndexPointer::from_keyword("/alkanes/")
        .select(&token_bytes)
        .keyword("/balances/")
        .select(&holder_bytes)
        .get_value::<u128>()
}

/// Deploy C (self-mints C:MAX) and D (self-mints D:7 so it can be discovered).
/// Returns (C, D, outpoint holding C:MAX).
fn setup() -> Result<(AlkaneId, AlkaneId, OutPoint)> {
    // Block 0: deploy C and self-mint C:MAX (lands unspent at (tx, 0)).
    let block0 = alkane_helpers::init_with_multiple_cellpacks_with_tx(
        vec![alkanes_std_test_build::get_bytes()],
        vec![Cellpack {
            target: AlkaneId { block: 1, tx: 0 },
            inputs: vec![22, MAX],
        }],
    );
    index_block(&block0, 0)?;
    let c_outpoint = OutPoint {
        txid: block0.txdata[block0.txdata.len() - 1].compute_txid(),
        vout: 0,
    };
    let c = find_minted_id(&c_outpoint, MAX);

    // Block 1: deploy D and self-mint D:7 (distinct instance, discoverable).
    let block1 = alkane_helpers::init_with_multiple_cellpacks_with_tx(
        vec![alkanes_std_test_build::get_bytes()],
        vec![Cellpack {
            target: AlkaneId { block: 1, tx: 0 },
            inputs: vec![22, 7],
        }],
    );
    index_block(&block1, 1)?;
    let d_outpoint = OutPoint {
        txid: block1.txdata[block1.txdata.len() - 1].compute_txid(),
        vout: 0,
    };
    let d = find_minted_id(&d_outpoint, 7);

    println!("[setup] C = {:?}, D = {:?}", c, d);
    assert_ne!(c, d, "need two distinct instances");
    Ok((c, d, c_outpoint))
}

#[wasm_bindgen_test]
fn test_transfer_from_overflow_wedge() -> Result<()> {
    clear();
    println!("=== transfer_from unchecked-add overflow PoC ===");
    let (c, d, c_max_outpoint) = setup()?;

    // Message target = C, opcode 34 (TestMultipleExtCall): two sub-calls to D
    // with opcode 7 (donate), each forwarding the incoming C:MAX. Encoding:
    //   [34, D.block, D.tx, len(inputs)=1, 7,  D.block, D.tx, len(inputs2)=1, 7]
    let attack = alkane_helpers::create_multiple_cellpack_with_witness_and_in(
        Witness::new(),
        vec![Cellpack {
            target: c.clone(),
            inputs: vec![34, d.block, d.tx, 1, 7, d.block, d.tx, 1, 7],
        }],
        c_max_outpoint,
        false,
    );
    let attack_txid = attack.compute_txid();
    let mut block2 = create_block_with_coinbase_tx(2);
    block2.txdata.push(attack);

    // On buggy code: the second forward credits D with MAX + MAX and PANICS
    // here (debug) — aborting index_block. The invariant we WANT: index_block
    // returns cleanly and D's balance never exceeds MAX.
    index_block(&block2, 2)?;

    // Reaching this line at all proves index_block did NOT panic/wedge on the
    // MAX+MAX credit (the pre-fix behavior aborted here at src/utils.rs:138).
    let d_holds_c = alkane_inventory(d.clone(), c.clone());
    // C:MAX was refunded to the attack tx's refund pointer (vout 0) after the
    // whole message reverted.
    let refunded_c = outpoint_sheet(&OutPoint {
        txid: attack_txid,
        vout: 0,
    })
    .get_cached(&rune_of(&c));
    println!(
        "[result] D inventory of C = {}, C refunded to vout0 = {}",
        d_holds_c, refunded_c
    );

    // FIXED: the overflowing credit is a graceful Err (checked_add), which
    // aborts the sub-call, which reverts the whole message and refunds the
    // incoming. No panic, no partial state, and total C is conserved at MAX.
    assert_eq!(d_holds_c, 0, "attack fully reverted: D holds no C");
    assert_eq!(
        refunded_c, MAX,
        "CONSERVATION: C:MAX refunded intact (no inflation, no loss)"
    );
    Ok(())
}
