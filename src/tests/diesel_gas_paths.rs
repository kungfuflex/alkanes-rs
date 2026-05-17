//! Exhaustive gas-determinism probe for DIESEL.
//!
//! Runs every distinct DIESEL execution path against the regtest binary
//! (which on regtest is the EOA-upgraded variant), snapshots the
//! `gas_used` recorded by `fuel_probe`, buckets observations per path, and
//! asserts each bucket contains exactly one unique gas value.

use crate::fuel_probe::{self, Record};
use crate::index_block;
use crate::tests::helpers::{
    self as alkane_helpers, assert_return_context, assert_revert_context, clear,
};
use crate::tests::std::alkanes_std_auth_token_build;
use alkanes_support::cellpack::Cellpack;
use alkanes_support::constants::AUTH_TOKEN_FACTORY_ID;
use alkanes_support::id::AlkaneId;
use anyhow::Result;
use bitcoin::hashes::Hash;
use bitcoin::{OutPoint, Txid, Witness};
use protorune::test_helpers::{create_block_with_coinbase_tx, create_coinbase_transaction};
use std::collections::BTreeMap;
use wasm_bindgen_test::wasm_bindgen_test;

use crate::network::genesis;
#[allow(unused_imports)]
use metashrew_core::{println, print, stdio::{stdout, Write}};

const DIESEL: AlkaneId = AlkaneId { block: 2, tx: 0 };

fn diesel_records() -> Vec<Record> {
    fuel_probe::snapshot()
        .into_iter()
        .filter(|r| r.target == DIESEL)
        .collect()
}

fn print_records(label: &str, records: &[Record]) {
    println!("--- {} ({} records) ---", label, records.len());
    for r in records {
        println!(
            "  opcode={} height={} gas_used={}",
            r.opcode, r.height, r.gas_used
        );
    }
}

/// Indexes a coinbase-only block at height 0 to trigger setup_diesel and the
/// precompile upgrade chain (regtest immediately deploys the EOA-upgraded
/// binary because GENESIS_UPGRADE_*_HEIGHT = 0 on regtest).
fn setup_diesel_only() -> Result<()> {
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
    index_block(&block, 0)?;
    Ok(())
}

/// Burns through opcode 1 (Upgrade): spends the genesis premine into the
/// upgrade tx and observes_upgrade_initialization, which sets
/// `/upgrade_initialized` and switches every subsequent mint to the
/// communist path.
fn run_upgrade(height: u32) -> Result<()> {
    let premine_outpoint = OutPoint {
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
    let upgrade = Cellpack {
        target: DIESEL.clone(),
        inputs: vec![1],
    };
    let mut block = create_block_with_coinbase_tx(height);
    let upgrade_tx = alkane_helpers::create_multiple_cellpack_with_witness_and_in(
        Witness::new(),
        vec![upgrade],
        premine_outpoint,
        false,
    );
    block.txdata.push(upgrade_tx);
    index_block(&block, height)?;
    Ok(())
}

/// Builds a block at `height` that contains `n` distinct mint txs, each
/// spending a unique outpoint from the coinbase tx so each gets a unique
/// txid. Returns the constructed block.
fn build_mint_block(height: u32, n: usize) -> bitcoin::Block {
    let mint = Cellpack {
        target: DIESEL.clone(),
        inputs: vec![77],
    };
    let mut block = create_block_with_coinbase_tx(height);
    let cb_txid = block.txdata[0].compute_txid();
    for i in 0..n {
        let mint_tx = alkane_helpers::create_multiple_cellpack_with_witness_and_in(
            Witness::new(),
            vec![mint.clone()],
            OutPoint::new(cb_txid, i as u32),
            false,
        );
        block.txdata.push(mint_tx);
    }
    block
}

#[wasm_bindgen_test]
fn diesel_gas_paths_exhaustive() -> Result<()> {
    clear();

    // === Phase 1: setup the diesel precompile ====================================
    setup_diesel_only()?;
    let setup_records = fuel_probe::snapshot();
    println!(
        "setup_diesel produced {} probe records (any target):",
        setup_records.len()
    );
    for r in &setup_records {
        println!("  target={:?} opcode={} h={} gas={}", r.target, r.opcode, r.height, r.gas_used);
    }
    fuel_probe::clear();

    // === Phase 2: legacy path (pre-upgrade) ======================================
    // Block contains 3 mint txs. On regtest the EOA-binary's mint() branches
    // on /upgrade_initialized; since the upgrade hasn't happened yet, all 3
    // take the legacy `create_mint_transfer` path: tx0 succeeds (writes
    // /seen/<h>); tx1 and tx2 revert in observe_mint (already minted).
    let legacy_height = 1u32;
    let legacy_block = build_mint_block(legacy_height, 3);
    let legacy_txids: Vec<Txid> = legacy_block
        .txdata
        .iter()
        .skip(1)
        .map(|t| t.compute_txid())
        .collect();
    index_block(&legacy_block, legacy_height)?;

    let all_after_legacy = fuel_probe::snapshot();
    println!("ALL records after legacy block: {} entries", all_after_legacy.len());
    for r in &all_after_legacy {
        println!("  target={:?} opcode={} h={} gas={}", r.target, r.opcode, r.height, r.gas_used);
    }
    let legacy_records = diesel_records();
    print_records("legacy 3-mint block", &legacy_records);
    fuel_probe::clear();

    // Sanity: tx0 succeeds, tx1+tx2 revert with "already minted"
    assert_revert_context(
        &OutPoint {
            txid: legacy_txids[1],
            vout: 3,
        },
        "already minted",
    )?;
    assert_revert_context(
        &OutPoint {
            txid: legacy_txids[2],
            vout: 3,
        },
        "already minted",
    )?;

    assert_eq!(
        legacy_records.len(),
        3,
        "expected 3 DIESEL invocations from the legacy block"
    );
    let legacy_success_gas = legacy_records[0].gas_used;
    let legacy_revert_gas_a = legacy_records[1].gas_used;
    let legacy_revert_gas_b = legacy_records[2].gas_used;
    assert_eq!(
        legacy_revert_gas_a, legacy_revert_gas_b,
        "P2 (legacy duplicate revert) must be deterministic within a block"
    );

    // Repeat legacy phase at a different height to test height invariance.
    // We need fresh state — clear() drops the indexer KV store.
    clear();
    setup_diesel_only()?;
    fuel_probe::clear();
    let alt_height = 5u32;
    let alt_block = build_mint_block(alt_height, 2);
    index_block(&alt_block, alt_height)?;
    let alt_records = diesel_records();
    print_records("legacy 2-mint block at h=5", &alt_records);
    fuel_probe::clear();
    assert_eq!(alt_records[0].gas_used, legacy_success_gas,
        "P1 (legacy success) must be height-invariant: got {} at h=1, {} at h=5",
        legacy_success_gas, alt_records[0].gas_used);
    assert_eq!(alt_records[1].gas_used, legacy_revert_gas_a,
        "P2 (legacy revert) must be height-invariant");

    // === Phase 3: upgrade then communist path ====================================
    clear();
    setup_diesel_only()?;
    // Pre-upgrade, mint once at height 1 so the legacy /seen pointer ISN'T set
    // for the post-upgrade heights we'll test.
    run_upgrade(1)?;
    fuel_probe::clear();

    // Block at h=2 with N=5 distinct mint txs.
    let n = 5usize;
    let communist_height = 2u32;
    let communist_block = build_mint_block(communist_height, n);
    index_block(&communist_block, communist_height)?;
    let communist_records = diesel_records();
    print_records("communist 5-mint block", &communist_records);
    fuel_probe::clear();

    assert_eq!(communist_records.len(), n);
    let c1_gas = communist_records[0].gas_used; // first-of-block write path
    let c2_gas = communist_records[1].gas_used; // subsequent early-return path
    for (i, r) in communist_records.iter().enumerate().skip(2) {
        assert_eq!(
            r.gas_used, c2_gas,
            "C2 (communist subsequent mint #{}) drifted: {} vs {}",
            i, r.gas_used, c2_gas
        );
    }
    assert_ne!(
        c1_gas, c2_gas,
        "Sanity: 1st-of-block (writes /upgraded_seen, /fees, /totalsupply) \
         should cost more than subsequent (early-returns) — fix the test \
         classifier if this fails"
    );

    // Repeat at a different post-upgrade height to test height invariance.
    clear();
    setup_diesel_only()?;
    run_upgrade(1)?;
    fuel_probe::clear();
    let later_height = 17u32;
    let later_block = build_mint_block(later_height, 3);
    index_block(&later_block, later_height)?;
    let later_records = diesel_records();
    print_records("communist 3-mint block at h=17", &later_records);
    fuel_probe::clear();
    assert_eq!(later_records[0].gas_used, c1_gas,
        "C1 (communist 1st-of-block) must be height-invariant: {} at h=2, {} at h=17",
        c1_gas, later_records[0].gas_used);
    assert_eq!(later_records[1].gas_used, c2_gas,
        "C2 (communist subsequent) must be height-invariant");

    // === Phase 4: view opcodes ===================================================
    clear();
    setup_diesel_only()?;
    run_upgrade(1)?;
    fuel_probe::clear();
    let view_height = 3u32;
    let mut view_block = create_block_with_coinbase_tx(view_height);
    let cb = view_block.txdata[0].compute_txid();
    for (i, op) in [99u128, 100, 101, 99, 100, 101].iter().enumerate() {
        let cellpack = Cellpack {
            target: DIESEL.clone(),
            inputs: vec![*op],
        };
        let tx = alkane_helpers::create_multiple_cellpack_with_witness_and_in(
            Witness::new(),
            vec![cellpack],
            OutPoint::new(cb, i as u32),
            false,
        );
        view_block.txdata.push(tx);
    }
    index_block(&view_block, view_height)?;
    let view_records = diesel_records();
    print_records("view opcodes", &view_records);
    fuel_probe::clear();

    let mut buckets: BTreeMap<u128, Vec<u64>> = BTreeMap::new();
    for r in &view_records {
        buckets.entry(r.opcode).or_default().push(r.gas_used);
    }
    for (op, gases) in &buckets {
        let first = gases[0];
        for g in gases {
            assert_eq!(
                *g, first,
                "view opcode {} gas drift: saw {} after {}",
                op, g, first
            );
        }
        println!("  view opcode {} -> {} gas (×{})", op, first, gases.len());
    }

    // === Phase 4b: sweep N to see if total_mints in the block affects gas ========
    let mut sweep_c1: BTreeMap<usize, u64> = BTreeMap::new();
    let mut sweep_c2: BTreeMap<usize, u64> = BTreeMap::new();
    for n_mints in [1usize, 2, 3, 5, 8, 13] {
        clear();
        setup_diesel_only()?;
        run_upgrade(1)?;
        fuel_probe::clear();
        let h = 50u32 + n_mints as u32;
        let block = build_mint_block(h, n_mints);
        index_block(&block, h)?;
        let recs = diesel_records();
        sweep_c1.insert(n_mints, recs[0].gas_used);
        if recs.len() >= 2 {
            sweep_c2.insert(n_mints, recs[1].gas_used);
        }
        fuel_probe::clear();
    }
    println!("\n=== Communist gas sweep by N (mint count in block) ===");
    for (n, g) in &sweep_c1 {
        println!("  N={:>2}  C1 (1st-of-block) gas = {}", n, g);
    }
    for (n, g) in &sweep_c2 {
        println!("  N={:>2}  C2 (subsequent)    gas = {}", n, g);
    }
    let c1_values: std::collections::BTreeSet<u64> = sweep_c1.values().copied().collect();
    let c2_values: std::collections::BTreeSet<u64> = sweep_c2.values().copied().collect();
    println!(
        "C1 distinct gas values across N: {:?}",
        c1_values.iter().collect::<Vec<_>>()
    );
    println!(
        "C2 distinct gas values across N: {:?}",
        c2_values.iter().collect::<Vec<_>>()
    );

    // === Phase 5: error paths ====================================================
    //
    // E1 (post-upgrade): same txid used to mint twice. We construct one tx
    // containing TWO mint protostones — the helper says "multiple mints in
    // one protostone is ignored" but a single tx with TWO protostones each
    // calling 77 should hit `enforce_one_mint_per_tx` on the second.
    clear();
    setup_diesel_only()?;
    run_upgrade(1)?;
    fuel_probe::clear();
    let e1_height = 4u32;
    let mut e1_block = create_block_with_coinbase_tx(e1_height);
    let mint_cell = Cellpack {
        target: DIESEL.clone(),
        inputs: vec![77],
    };
    let e1_tx = alkane_helpers::create_multiple_cellpack_with_witness_and_in(
        Witness::new(),
        vec![mint_cell.clone(), mint_cell.clone()],
        OutPoint::new(e1_block.txdata[0].compute_txid(), 0),
        false,
    );
    e1_block.txdata.push(e1_tx);
    index_block(&e1_block, e1_height)?;
    let e1_records = diesel_records();
    print_records("E1 (same-tx double mint)", &e1_records);
    fuel_probe::clear();

    // E2 (post-upgrade): legacy mint and upgraded mint in same block.
    // Achieved by minting on a fresh non-upgraded chain at height H, then
    // upgrading at H, then having a 2nd mint in the same block H. We model
    // this with the existing `run_upgrade` style test_block. Re-uses the
    // pattern from the existing genesis_upgrade::upgrade() helper.
    clear();
    setup_diesel_only()?;
    fuel_probe::clear();
    let e2_height = 1u32;
    let premine_outpoint = OutPoint {
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
    let upgrade_cell = Cellpack {
        target: DIESEL.clone(),
        inputs: vec![1],
    };
    let mut e2_block = create_block_with_coinbase_tx(e2_height);
    let cb = e2_block.txdata[0].compute_txid();
    // mint #1 (legacy, succeeds, sets /seen/1)
    e2_block.txdata.push(
        alkane_helpers::create_multiple_cellpack_with_witness_and_in(
            Witness::new(),
            vec![mint_cell.clone()],
            OutPoint::new(cb, 0),
            false,
        ),
    );
    // upgrade (sets /upgrade_initialized)
    e2_block.txdata.push(
        alkane_helpers::create_multiple_cellpack_with_witness_and_in(
            Witness::new(),
            vec![upgrade_cell],
            premine_outpoint,
            false,
        ),
    );
    // mint #2 (post-upgrade attempt, should revert via E2)
    e2_block.txdata.push(
        alkane_helpers::create_multiple_cellpack_with_witness_and_in(
            Witness::new(),
            vec![mint_cell],
            OutPoint::new(cb, 1),
            false,
        ),
    );
    index_block(&e2_block, e2_height)?;
    let e2_records = diesel_records();
    print_records("E2 (legacy + upgrade + upgraded mint)", &e2_records);
    fuel_probe::clear();

    // === Summary ================================================================
    println!("\n=== DIESEL gas-path table (regtest, EOA binary) ===");
    println!("  P1  legacy success           : {}", legacy_success_gas);
    println!("  P2  legacy duplicate revert  : {}", legacy_revert_gas_a);
    println!("  C1  communist 1st-of-block   : {}", c1_gas);
    println!("  C2  communist subsequent     : {}", c2_gas);
    for (op, gases) in &buckets {
        println!("  V{}  view opcode {}            : {}", op, op, gases[0]);
    }
    if let Some(r) = e1_records.first() {
        println!("  E1  same-tx double mint      : {}", r.gas_used);
    }
    if e2_records.len() >= 3 {
        println!(
            "  E2a legacy mint pre-upgrade   : {}",
            e2_records[0].gas_used
        );
        println!("  E2b upgrade opcode 1          : {}", e2_records[1].gas_used);
        println!(
            "  E2c upgraded mint w/ legacy seen: {}",
            e2_records[2].gas_used
        );
    }

    Ok(())
}
