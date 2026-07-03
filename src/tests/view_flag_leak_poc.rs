//! PoC for the view-mode flag leak in `simulate_protostones` (src/view.rs).
//!
//! `simulate_protostones` enables four process-global view collectors —
//! `enable_skip_protostone_persistence()` chief among them (src/view.rs:1018) —
//! and disables them inline near the end of the function (src/view.rs:1064-1067)
//! with NO `Drop`/RAII guard. Between the enable and the disable sits an
//! early-return: `seed_input_balances(...)?` (src/view.rs:1043). If it returns
//! `Err`, the disables never run and `SKIP_PROTOSTONE_PERSISTENCE` stays `true`
//! for the rest of the process. The next REAL `index_block` then observes
//! `should_skip_protostone_persistence() == true` and SKIPS `save_balances` +
//! `clear_balances` (crates/protorune/src/lib.rs:1208) — balances are never
//! written to OUTPOINT_TO_RUNES and spent inputs are never cleared. That is
//! silent index corruption / consensus divergence, triggered by a single
//! crafted view RPC.
//!
//! Trigger: `seed_input_balances` builds a `BalanceSheet::try_from` over the
//! caller-supplied `alkane_inputs`; two inputs with the same id summing past
//! `u128::MAX` make `increase()`'s `checked_add` return `Err`.
//!
//! Fix: an RAII guard that disables all four collectors on drop, so an
//! early-return (or a panic during `index_protostones`) can never leak a flag.
//!
//! Pre-fix: the `should_skip_protostone_persistence()` assertion below fails
//! (leaked `true`) and the follow-up mint is not persisted. Post-fix both pass.

use crate::index_block;
use crate::message::AlkaneMessageContext;
use crate::tests::helpers as alkane_helpers;
use crate::tests::std::alkanes_std_test_build;
use crate::view::{simulate_protostones, SimulateProtostonesInput};
use alkane_helpers::clear;
use alkanes_support::cellpack::Cellpack;
use alkanes_support::id::AlkaneId;
use alkanes_support::parcel::AlkaneTransfer;
use anyhow::Result;
use bitcoin::OutPoint;
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
use protorune::protostone::Protostones;
use protorune::tables::RuneTable;
use protorune_support::balance_sheet::{BalanceSheet, BalanceSheetOperations, ProtoruneRuneId};
use protorune_support::protostone::Protostone;
use protorune_support::utils::encode_varint_list;
use wasm_bindgen_test::wasm_bindgen_test;

fn outpoint_sheet(outpoint: &OutPoint) -> BalanceSheet<IndexPointer> {
    let ptr = RuneTable::for_protocol(AlkaneMessageContext::protocol_tag())
        .OUTPOINT_TO_RUNES
        .select(&consensus_encode(outpoint).unwrap());
    load_sheet(&ptr)
}

/// True iff `outpoint` has any nonzero rune balance persisted.
fn outpoint_has_balance(outpoint: &OutPoint, amount: u128) -> bool {
    outpoint_sheet(outpoint)
        .balances()
        .values()
        .any(|v| *v == amount)
}

/// Enciphered-protostones bytes for a single message calling the genesis
/// alkane (2,0) opcode 99 (a pure read) — enough to get past the empty-protocol
/// short-circuit so the flag enables actually run.
fn one_readonly_protostone_bytes() -> Result<Vec<u8>> {
    let cellpack = Cellpack {
        target: AlkaneId { block: 2, tx: 0 },
        inputs: vec![99u128],
    };
    let protostone = Protostone {
        message: cellpack.encipher(),
        protocol_tag: 1,
        from: None,
        burn: None,
        pointer: Some(0),
        refund: Some(0),
        edicts: vec![],
    };
    Ok(encode_varint_list(&vec![protostone].encipher()?))
}

#[wasm_bindgen_test]
fn test_view_flag_leak_corrupts_indexer() -> Result<()> {
    clear();
    println!("=== simulate_protostones flag-leak PoC ===");

    // Block 0: a REAL indexed block (deploy test alkane + self-mint 1000). This
    // also sets /seen-genesis, so the never-unset view-mode flag can't perturb
    // later blocks — isolating SKIP_PROTOSTONE_PERSISTENCE as the only variable.
    let block0 = alkane_helpers::init_with_multiple_cellpacks_with_tx(
        vec![alkanes_std_test_build::get_bytes()],
        vec![Cellpack {
            target: AlkaneId { block: 1, tx: 0 },
            inputs: vec![22, 1000],
        }],
    );
    index_block(&block0, 0)?;
    let c_outpoint = OutPoint {
        txid: block0.txdata[block0.txdata.len() - 1].compute_txid(),
        vout: 0,
    };
    assert!(
        outpoint_has_balance(&c_outpoint, 1000),
        "sanity: normal indexing persists balances before the leak"
    );

    // Trigger the leak: a view call whose synth-tx path seeds input balances,
    // with two same-id inputs that overflow u128 in BalanceSheet::try_from ->
    // seed_input_balances returns Err -> simulate_protostones returns Err
    // BEFORE the disable calls run.
    let bad = AlkaneId { block: 2, tx: 0 };
    let sim = simulate_protostones(SimulateProtostonesInput {
        height: 1,
        alkane_inputs: vec![
            AlkaneTransfer {
                id: bad.clone(),
                value: u128::MAX,
            },
            AlkaneTransfer {
                id: bad.clone(),
                value: 1,
            },
        ],
        protostones_bytes: one_readonly_protostone_bytes()?,
        transaction_bytes: None, // force the synth-tx path (seeds input balances)
        block_bytes: None,
        storage_overrides: vec![],
    });
    println!("[leak] simulate_protostones returned err = {}", sim.is_err());
    assert!(
        sim.is_err(),
        "expected seed_input_balances overflow to surface as Err (the leak trigger)"
    );

    // CORE INVARIANT: a view call must NEVER leave a persistence-skip flag set
    // for the indexer. Pre-fix this is `true` (leaked); post-fix `false`.
    assert!(
        !protorune::should_skip_protostone_persistence(),
        "SKIP_PROTOSTONE_PERSISTENCE leaked `true` into the indexer after a failed simulate"
    );

    // Functional proof: a subsequent real block must still persist balances.
    let block1 = alkane_helpers::init_with_multiple_cellpacks_with_tx(
        vec![alkanes_std_test_build::get_bytes()],
        vec![Cellpack {
            target: AlkaneId { block: 1, tx: 0 },
            inputs: vec![22, 777],
        }],
    );
    index_block(&block1, 1)?;
    let mint_outpoint = OutPoint {
        txid: block1.txdata[block1.txdata.len() - 1].compute_txid(),
        vout: 0,
    };
    assert!(
        outpoint_has_balance(&mint_outpoint, 777),
        "index corruption: post-leak block did not persist balances to OUTPOINT_TO_RUNES"
    );

    let _ = ProtoruneRuneId::new(bad.block, bad.tx);
    Ok(())
}
