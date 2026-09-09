//! Deliverable 1 — direct unit test of the height-versioned DIESEL genesis-alkane
//! (`2:0`) code map restored by the consensus fix.
//!
//! `crate::network::genesis_alkane_wasm_for_height(height)` MUST progress
//! base -> upgraded -> upgraded-EOA at the documented mainnet fork heights,
//! mirroring the one-shot swaps in `check_and_upgrade_precompiled`:
//!
//!   * base          (`genesis_alkane_bytes`,             174_225 B) for
//!     h < GENESIS_UPGRADE_BLOCK_HEIGHT (908_888)
//!   * upgraded      (`genesis_alkane_upgrade_bytes`,     260_605 B) for
//!     GENESIS_UPGRADE_BLOCK_HEIGHT <= h < GENESIS_UPGRADE_EOA_BLOCK_HEIGHT (917_888)
//!   * upgraded-EOA  (`genesis_alkane_upgrade_bytes_eoa`, 262_445 B) for
//!     h >= GENESIS_UPGRADE_EOA_BLOCK_HEIGHT
//!
//! The pre-fix develop code had no such map: `get_alkane_binary_from_context`
//! special-cased only frBTC (`32:0`) and `2:0` fell through to indexed state,
//! which was the heavier upgraded-EOA build from genesis. This test pins the
//! restored progression and would FAIL on that pre-fix code (which had no
//! `genesis_alkane_wasm_for_height` at all / resolved EOA for every height).
//!
//! The three byte lengths asserted here are the exact on-chain `getbytecode`
//! sizes for `2:0` across its lifetime; the `assert_eq!` on the source-fn bytes
//! guarantees the map returns *the same* blob (not merely the same length).

#![cfg(feature = "mainnet")]

use crate::network::{
    genesis, genesis_alkane_bytes, genesis_alkane_upgrade_bytes, genesis_alkane_upgrade_bytes_eoa,
    genesis_alkane_wasm_for_height,
};
use wasm_bindgen_test::wasm_bindgen_test;

#[allow(unused_imports)]
use metashrew_core::{
    println,
    stdio::{stdout, Write},
};

// Exact on-chain getbytecode sizes for genesis alkane 2:0 across its lifetime.
const BASE_LEN: usize = 174_225;
const UPGRADED_LEN: usize = 260_605;
const EOA_LEN: usize = 262_445;

#[wasm_bindgen_test]
fn genesis_alkane_source_bytes_have_expected_lengths() {
    // First confirm the underlying build bytes are exactly the documented
    // on-chain sizes, so the map assertions below are anchored to real code.
    let base = genesis_alkane_bytes();
    let upgraded = genesis_alkane_upgrade_bytes();
    let eoa = genesis_alkane_upgrade_bytes_eoa();
    println!(
        "genesis 2:0 build sizes -> base={} upgraded={} eoa={}",
        base.len(),
        upgraded.len(),
        eoa.len()
    );
    assert_eq!(base.len(), BASE_LEN, "base (genesis_alkane_bytes) length");
    assert_eq!(
        upgraded.len(),
        UPGRADED_LEN,
        "upgraded (genesis_alkane_upgrade_bytes) length"
    );
    assert_eq!(
        eoa.len(),
        EOA_LEN,
        "upgraded-EOA (genesis_alkane_upgrade_bytes_eoa) length"
    );

    // The three builds must be distinct (heavier at each step).
    assert!(base.len() < upgraded.len() && upgraded.len() < eoa.len());
}

#[wasm_bindgen_test]
fn genesis_alkane_wasm_for_height_progresses_base_upgraded_eoa() {
    // Sanity-check the fork constants this test is written against.
    assert_eq!(genesis::GENESIS_UPGRADE_BLOCK_HEIGHT, 908_888);
    assert_eq!(genesis::GENESIS_UPGRADE_EOA_BLOCK_HEIGHT, 917_888);

    let base = genesis_alkane_bytes();
    let upgraded = genesis_alkane_upgrade_bytes();
    let eoa = genesis_alkane_upgrade_bytes_eoa();

    // --- base range: h < 908_888 -------------------------------------------
    // 893_514 is the first mainnet divergence block (alkane 2:465). Pre-fix,
    // `2:0` ran EOA here and fuel-reverted the free-mint; the map must return
    // the light base build.
    for h in [893_514u32, 908_887u32] {
        let got = genesis_alkane_wasm_for_height(h);
        assert_eq!(
            got.len(),
            BASE_LEN,
            "h={} must resolve base ({} B), got {} B",
            h,
            BASE_LEN,
            got.len()
        );
        assert_eq!(got, base, "h={} must be byte-identical to base build", h);
    }

    // --- upgraded range: 908_888 <= h < 917_888 ----------------------------
    for h in [908_888u32, 917_887u32] {
        let got = genesis_alkane_wasm_for_height(h);
        assert_eq!(
            got.len(),
            UPGRADED_LEN,
            "h={} must resolve upgraded ({} B), got {} B",
            h,
            UPGRADED_LEN,
            got.len()
        );
        assert_eq!(
            got, upgraded,
            "h={} must be byte-identical to upgraded build",
            h
        );
    }

    // --- upgraded-EOA range: h >= 917_888 ----------------------------------
    {
        let h = 917_888u32;
        let got = genesis_alkane_wasm_for_height(h);
        assert_eq!(
            got.len(),
            EOA_LEN,
            "h={} must resolve upgraded-EOA ({} B), got {} B",
            h,
            EOA_LEN,
            got.len()
        );
        assert_eq!(
            got, eoa,
            "h={} must be byte-identical to upgraded-EOA build",
            h
        );
    }

    println!("✓ genesis_alkane_wasm_for_height base->upgraded->eoa progression verified");
}
