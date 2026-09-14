//! Full-coverage shadow comparison: runs real blocks through
//! `Protorune::index_block` with the precompile-DIESEL shadow harness
//! enabled, exercises every enumerated DIESEL path, and asserts that
//! no shadow records report divergence.
//!
//! This is the consensus-safety test for swapping wasm DIESEL for the
//! native precompile.

use crate::index_block;
use crate::network::genesis;
use crate::precompile_diesel::{
    self, shadow_clear, shadow_disable, shadow_enable, shadow_snapshot, DieselPath, DIESEL_ID,
};
use crate::tests::helpers::{self as alkane_helpers, clear};
use crate::tests::std::alkanes_std_auth_token_build;
use alkanes_support::cellpack::Cellpack;
use alkanes_support::constants::AUTH_TOKEN_FACTORY_ID;
use alkanes_support::id::AlkaneId;
use anyhow::Result;
use bitcoin::hashes::Hash;
use bitcoin::{Block, OutPoint, Txid, Witness};
use metashrew_core::index_pointer::IndexPointer;
use metashrew_support::index_pointer::KeyValuePointer;
use protorune::test_helpers::create_block_with_coinbase_tx;
use std::collections::BTreeMap;
use wasm_bindgen_test::wasm_bindgen_test;

#[allow(unused_imports)]
use metashrew_core::{print, println, stdio::{stdout, Write}};

/// Per-chain test starting height. We need to be past `GENESIS_BLOCK` for
/// `is_active` to fire setup_diesel; for mainnet we also need to be past
/// `GENESIS_UPGRADE_EOA_BLOCK_HEIGHT` so the EOA binary is the active
/// DIESEL. We deliberately stay BELOW `V220_FORK_HEIGHT` so this matches
/// the path the test set was designed against.
#[cfg(feature = "mainnet")]
const TEST_BASE_HEIGHT: u32 = 925_000; // > GENESIS_UPGRADE_EOA_BLOCK_HEIGHT (917_888), < V220_FORK_HEIGHT (950_000)
#[cfg(not(feature = "mainnet"))]
const TEST_BASE_HEIGHT: u32 = 0;

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
    index_block(&block, TEST_BASE_HEIGHT)?;
    Ok(())
}

fn run_upgrade(height: u32) -> Result<()> {
    // Direct-write `/upgrade_initialized = 0x01` under DIESEL's storage
    // subtree. Same key path the wasm `upgrade()` handler writes to (see
    // `alkanes-std-genesis-alkane-upgraded-eoa::upgrade`); this just
    // bypasses the wasm ceremony.
    //
    // Why direct-write on BOTH chains (was only mainnet pre-fix):
    //   * mainnet: legacy binary's 50M default premine doesn't match
    //     the EOA binary's 44T expectation → wasm opcode 1 rejects
    //     with "Premine is not spent into the upgrade".
    //   * regtest: the upgrade tx referenced `GENESIS_OUTPOINT` as its
    //     prevout, but the test setup never ran DIESEL.initialize first
    //     to MINT the premine into that outpoint — so the upgrade tx's
    //     incoming_alkanes is empty, wasm sees no premine, same reject.
    //     (The reject is silent — index_block doesn't fail the test,
    //     it just doesn't write /upgrade_initialized, and the
    //     subsequent mint runs the legacy path. That's the bug class
    //     that kept these proof-tests red since task #14 landed.)
    //
    // For shadow-mode gas calibration we don't care about the upgrade
    // *mechanism* — we just need the storage flag flipped. Same applies
    // to diesel_sidebyside's setup_with_upgrade, which now uses the same
    // direct-write approach.
    let _ = height;
    let mut ptr = IndexPointer::default()
        .keyword("/alkanes/")
        .select(&DIESEL_ID.into())
        .keyword("/storage/")
        .select(&b"/upgrade_initialized".to_vec());
    ptr.set_value::<u8>(0x01);
    Ok(())
}

fn build_mint_block(height: u32, n: usize) -> Block {
    let mint = Cellpack {
        target: DIESEL_ID,
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

fn assert_no_divergence(label: &str) -> Vec<precompile_diesel::ShadowRecord> {
    let records = shadow_snapshot();
    let diverged: Vec<_> = records.iter().filter(|r| r.diverged).collect();
    if !diverged.is_empty() {
        println!("\n[{}] shadow divergences ({}):", label, diverged.len());
        for r in &diverged {
            println!(
                "  height={} opcode={} reason={:?}",
                r.height,
                r.opcode,
                r.divergence_reason.as_deref().unwrap_or("?")
            );
            println!(
                "    wasm_gas={} precomp_gas={} wasm_err={:?} precomp_err={:?}",
                r.wasm_gas, r.precomp_gas, r.wasm_error, r.precomp_error
            );
        }
        panic!(
            "[{}] shadow comparison found {} divergences out of {} records",
            label,
            diverged.len(),
            records.len()
        );
    }
    records
}

#[wasm_bindgen_test]
fn diesel_shadow_full_coverage() -> Result<()> {
    clear();
    shadow_disable();
    shadow_clear();
    setup_diesel_only()?;
    run_upgrade(TEST_BASE_HEIGHT + 1)?;

    // Diagnostic: is /upgrade_initialized set?
    let upgrade_init = IndexPointer::default()
        .keyword("/alkanes/")
        .select(&DIESEL_ID.into())
        .keyword("/storage/")
        .select(&b"/upgrade_initialized".to_vec())
        .get();
    println!(
        "DIAG: /upgrade_initialized len after run_upgrade = {} (expect 1 for communist path)",
        upgrade_init.len()
    );

    // === Communist 5-mint block: exercises C1 once, C2 four times ===========
    shadow_clear();
    shadow_enable();
    let h_c = TEST_BASE_HEIGHT + 2;
    let block_c = build_mint_block(h_c, 5);
    index_block(&block_c, h_c)?;
    shadow_disable();
    let r_c = assert_no_divergence("communist 5-mint block");
    let mut c1: Vec<u64> = Vec::new();
    let mut c2: Vec<u64> = Vec::new();
    for r in &r_c {
        match r.path {
            Some(DieselPath::CommunistFirstOfBlock) => c1.push(r.wasm_gas),
            Some(DieselPath::CommunistSubsequent) => c2.push(r.wasm_gas),
            _ => {}
        }
    }
    println!("C1 observations: {:?}", c1);
    println!("C2 observations: {:?}", c2);

    // === E1: single tx with two opcode 77 protostones ========================
    // Construct it manually so the SAME txid is used for both mints. The
    // 1st protostone takes C1 (1st-of-block in the EOA binary) — but with
    // extra fuel from the larger tx — and the 2nd protostone reverts
    // because /tx-hashes/<txid> was piped between them.
    clear();
    shadow_disable();
    shadow_clear();
    setup_diesel_only()?;
    run_upgrade(TEST_BASE_HEIGHT + 1)?;
    shadow_clear();
    shadow_enable();
    let h_e1 = TEST_BASE_HEIGHT + 4;
    let mut block_e1 = create_block_with_coinbase_tx(h_e1);
    let cb_txid = block_e1.txdata[0].compute_txid();
    let mint_cell = Cellpack {
        target: DIESEL_ID,
        inputs: vec![77],
    };
    let e1_tx = alkane_helpers::create_multiple_cellpack_with_witness_and_in(
        Witness::new(),
        vec![mint_cell.clone(), mint_cell.clone()],
        OutPoint::new(cb_txid, 0),
        false,
    );
    block_e1.txdata.push(e1_tx);
    index_block(&block_e1, h_e1)?;
    shadow_disable();
    let r_e1 = assert_no_divergence("E1 same-tx double mint");
    println!(
        "E1 records: {:?}",
        r_e1.iter()
            .map(|r| (r.opcode, r.path.map(|p| p.tag()), r.wasm_gas, r.wasm_error.as_deref(), r.precomp_error.as_deref()))
            .collect::<Vec<_>>()
    );

    // === E2: legacy mint then upgrade then upgraded mint in same block =====
    // On mainnet the natural upgrade ceremony is unreachable from the test
    // harness (legacy binary's default 50M premine vs EOA binary's 44T
    // expectation), so we synthesize the post-condition: /seen/<h> AND
    // /upgrade_initialized both set → any communist mint at <h> reverts
    // via E2c "upgraded mint in the same block as legacy mint".
    clear();
    shadow_disable();
    shadow_clear();
    setup_diesel_only()?;
    let h_e2 = TEST_BASE_HEIGHT + 1;
    #[cfg(feature = "mainnet")]
    {
        // Mark a legacy mint as already-seen at h_e2.
        let mut seen_key: Vec<u8> = b"/seen/".to_vec();
        seen_key.extend_from_slice(&(h_e2 as u64).to_le_bytes());
        let mut seen_ptr = IndexPointer::default()
            .keyword("/alkanes/")
            .select(&DIESEL_ID.into())
            .keyword("/storage/")
            .select(&seen_key);
        seen_ptr.set_value::<u32>(1);
        // Mark the upgrade as completed.
        let mut up_ptr = IndexPointer::default()
            .keyword("/alkanes/")
            .select(&DIESEL_ID.into())
            .keyword("/storage/")
            .select(&b"/upgrade_initialized".to_vec());
        up_ptr.set_value::<u8>(0x01);
    }
    shadow_clear();
    shadow_enable();
    #[cfg(not(feature = "mainnet"))]
    {
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
            target: DIESEL_ID,
            inputs: vec![1],
        };
        let mut block_e2 = create_block_with_coinbase_tx(h_e2);
        let cb_txid_e2 = block_e2.txdata[0].compute_txid();
        block_e2.txdata.push(
            alkane_helpers::create_multiple_cellpack_with_witness_and_in(
                Witness::new(),
                vec![mint_cell.clone()],
                OutPoint::new(cb_txid_e2, 0),
                false,
            ),
        );
        block_e2.txdata.push(
            alkane_helpers::create_multiple_cellpack_with_witness_and_in(
                Witness::new(),
                vec![upgrade_cell],
                premine_outpoint,
                false,
            ),
        );
        block_e2.txdata.push(
            alkane_helpers::create_multiple_cellpack_with_witness_and_in(
                Witness::new(),
                vec![mint_cell.clone()],
                OutPoint::new(cb_txid_e2, 1),
                false,
            ),
        );
        index_block(&block_e2, h_e2)?;
    }
    #[cfg(feature = "mainnet")]
    {
        // Pre-populated state: /seen/<h> + /upgrade_initialized both set.
        // A single upgraded mint at h_e2 should revert E2c.
        let mut block_e2 = create_block_with_coinbase_tx(h_e2);
        let cb_txid_e2 = block_e2.txdata[0].compute_txid();
        block_e2.txdata.push(
            alkane_helpers::create_multiple_cellpack_with_witness_and_in(
                Witness::new(),
                vec![mint_cell.clone()],
                OutPoint::new(cb_txid_e2, 0),
                false,
            ),
        );
        index_block(&block_e2, h_e2)?;
    }
    shadow_disable();
    let r_e2 = assert_no_divergence("E2 legacy+upgrade+upgraded");
    println!(
        "E2 records: {:?}",
        r_e2.iter()
            .map(|r| (r.opcode, r.path.map(|p| p.tag()), r.wasm_gas, r.wasm_error.as_deref(), r.precomp_error.as_deref()))
            .collect::<Vec<_>>()
    );

    // === P2: legacy duplicate revert ========================================
    // 3 single-protostone mint txs in one block; pre-upgrade. The 1st mint
    // succeeds (P1), the 2nd and 3rd revert with "already minted".
    clear();
    shadow_disable();
    shadow_clear();
    setup_diesel_only()?;
    shadow_clear();
    shadow_enable();
    let h_p = TEST_BASE_HEIGHT + 6;
    let block_p = build_mint_block(h_p, 3);
    index_block(&block_p, h_p)?;
    shadow_disable();
    let r_p = assert_no_divergence("legacy 3-mint block (P1 + 2×P2)");
    println!(
        "P records: {:?}",
        r_p.iter()
            .map(|r| (
                r.path.map(|p| p.tag()),
                r.wasm_gas,
                r.wasm_error.as_deref().unwrap_or(""),
                r.precomp_error.as_deref().unwrap_or("")
            ))
            .collect::<Vec<_>>()
    );

    // === Cross-height C1+C2 sweep ===========================================
    // Run separate 3-mint communist blocks at heights {2,10,50,250} and
    // assert C1/C2 are constant across them.
    let mut c1_observations: Vec<u64> = Vec::new();
    let mut c2_observations: Vec<u64> = Vec::new();
    for delta in [2u32, 10, 50, 250] {
        let h = TEST_BASE_HEIGHT + delta;
        clear();
        shadow_disable();
        shadow_clear();
        setup_diesel_only()?;
        run_upgrade(TEST_BASE_HEIGHT + 1)?;
        shadow_clear();
        shadow_enable();
        let block = build_mint_block(h, 3);
        index_block(&block, h)?;
        shadow_disable();
        let recs = assert_no_divergence(&format!("communist 3-mint @h={}", h));
        for r in &recs {
            match r.path {
                Some(DieselPath::CommunistFirstOfBlock) => c1_observations.push(r.wasm_gas),
                Some(DieselPath::CommunistSubsequent) => c2_observations.push(r.wasm_gas),
                _ => {}
            }
        }
    }
    let c1_set: std::collections::BTreeSet<u64> = c1_observations.iter().copied().collect();
    let c2_set: std::collections::BTreeSet<u64> = c2_observations.iter().copied().collect();
    println!(
        "Cross-height C1: {:?} -> unique={:?}",
        c1_observations, c1_set
    );
    println!(
        "Cross-height C2: {:?} -> unique={:?}",
        c2_observations, c2_set
    );
    assert_eq!(
        c1_set.len(),
        1,
        "C1 gas must be height-invariant: observed {:?}",
        c1_set
    );
    assert_eq!(
        c2_set.len(),
        1,
        "C2 gas must be height-invariant: observed {:?}",
        c2_set
    );

    // === View opcodes 99/100/101 ============================================
    clear();
    shadow_disable();
    shadow_clear();
    setup_diesel_only()?;
    run_upgrade(TEST_BASE_HEIGHT + 1)?;
    // bump total supply to make V101 non-zero
    let mut ts_ptr = IndexPointer::default()
        .keyword("/alkanes/")
        .select(&DIESEL_ID.into())
        .keyword("/storage/")
        .select(&b"/totalsupply".to_vec());
    ts_ptr.set(std::sync::Arc::new(987654321u128.to_le_bytes().to_vec()));
    shadow_clear();
    shadow_enable();
    let h_v = TEST_BASE_HEIGHT + 3;
    let mut block_v = create_block_with_coinbase_tx(h_v);
    let cb_v = block_v.txdata[0].compute_txid();
    for (i, op) in [99u128, 100, 101, 99, 100, 101].iter().enumerate() {
        let cell = Cellpack {
            target: DIESEL_ID,
            inputs: vec![*op],
        };
        let tx = alkane_helpers::create_multiple_cellpack_with_witness_and_in(
            Witness::new(),
            vec![cell],
            OutPoint::new(cb_v, i as u32),
            false,
        );
        block_v.txdata.push(tx);
    }
    index_block(&block_v, h_v)?;
    shadow_disable();
    let r_v = assert_no_divergence("views 99/100/101");
    let mut by_op: BTreeMap<u128, Vec<u64>> = BTreeMap::new();
    for r in &r_v {
        by_op.entry(r.opcode).or_default().push(r.wasm_gas);
    }
    for (op, gases) in &by_op {
        println!("V{} observations: {:?}", op, gases);
    }

    // Capture the cross-height sweep records too.
    let mut r_sweep: Vec<precompile_diesel::ShadowRecord> = Vec::new();
    for delta in [2u32, 10, 50, 250] {
        let h = TEST_BASE_HEIGHT + delta;
        clear();
        shadow_disable();
        shadow_clear();
        setup_diesel_only()?;
        run_upgrade(TEST_BASE_HEIGHT + 1)?;
        shadow_clear();
        shadow_enable();
        let block = build_mint_block(h, 3);
        index_block(&block, h)?;
        shadow_disable();
        r_sweep.extend(assert_no_divergence(&format!(
            "communist 3-mint @h={} (recapture for calibration)",
            h
        )));
    }

    // === Calibration table dump ==============================================
    // After full coverage, the shadow records carry the per-path gas values
    // that a production precompile would need. Collect them.
    let all = [r_c, r_e1, r_e2, r_p, r_sweep, r_v].concat();
    let mut table: BTreeMap<&str, Vec<u64>> = BTreeMap::new();
    for r in &all {
        if let Some(p) = r.path {
            table.entry(p.tag()).or_default().push(r.wasm_gas);
        }
    }
    let chain_label = if cfg!(feature = "mainnet") {
        "mainnet"
    } else {
        "regtest"
    };
    println!(
        "\n=== DIESEL gas calibration table ({} EOA) — total gas values ===",
        chain_label
    );
    for (tag, gases) in &table {
        let unique: std::collections::BTreeSet<u64> = gases.iter().copied().collect();
        let canonical = *gases.first().unwrap_or(&0);
        let invariant = unique.len() == 1;
        println!(
            "  {} -> total={} (samples={}, invariant={})",
            tag,
            canonical,
            gases.len(),
            invariant
        );
    }

    println!(
        "\n=== Internal-gas (subtract storage_fuel) for CHAIN_GAS ({}) ===",
        chain_label
    );
    // Re-walk records to extract internal gas per path. Each record carries
    // both wasm_gas (total) and wasm_response.storage so we can compute
    // storage_fuel and subtract.
    use alkanes::vm::fuel::fuel_per_store_byte;
    let mut internal_table: BTreeMap<&str, Vec<u64>> = BTreeMap::new();
    for r in &all {
        if let (Some(p), Some(resp)) = (r.path, r.wasm_response.as_ref()) {
            let sl = resp.storage.serialize().len() as u64;
            let sf = fuel_per_store_byte(r.height).saturating_mul(sl);
            let internal = r.wasm_gas.saturating_sub(sf);
            internal_table.entry(p.tag()).or_default().push(internal);
        }
    }
    for (tag, vals) in &internal_table {
        let unique: std::collections::BTreeSet<u64> = vals.iter().copied().collect();
        let canonical = *vals.first().unwrap_or(&0);
        println!(
            "  {} -> internal={} (samples={}, invariant={})",
            tag,
            canonical,
            vals.len(),
            unique.len() == 1
        );
    }

    Ok(())
}
