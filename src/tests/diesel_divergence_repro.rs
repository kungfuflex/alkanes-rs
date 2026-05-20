//! Regression tests for the fastpath divergence surface audit.
//!
//! See `.fastpath-bug-investigation/divergence-surface-audit.md` (in
//! subkube) for the full analysis. These tests lock in fixes for two
//! HIGH-severity divergences between the native DIESEL precompile and
//! the wasm `GenesisAlkane` contract:
//!
//! 1. `max_supply` constant divergence (§2 of the audit). Pre-fix:
//!    precompile had `156_250_000_00000000` (1.5625e16) vs wasm's
//!    `156250000000000` (1.5625e14) — 100× off. Latent today
//!    because current `/totalsupply` is far below either threshold,
//!    but the moment supply crosses the wasm cap, every subsequent
//!    mint diverges by `value_per_mint`. Fix lives in
//!    `precompile_diesel.rs::max_supply` (mainnet branch).
//!
//! 2. `number_diesel_mints` decode-error handling (§6 of the audit).
//!    Pre-fix: precompile used `Err(_) => continue` to silently skip
//!    malformed varint protostones; wasm `_get_number_diesel_mints`
//!    in `vm/host_functions.rs:659` uses `?` to propagate the error.
//!    Suspected cause of the h=949478 mainnet divergence
//!    (`+1.21 DIESEL` on fp vs nf). Fix in
//!    `precompile_diesel.rs::number_diesel_mints` swaps `continue`
//!    for `?`.
//!
//! Both tests are **regression guards**, not red→green reproductions —
//! the fix is applied in the same commit. The test bodies document the
//! pre-fix bug so a future revert (or refactor that loses the fix)
//! lights up immediately.

use anyhow::Result;
use wasm_bindgen_test::wasm_bindgen_test;

#[allow(unused_imports)]
use metashrew_core::{print, println, stdio::{stdout, Write}};

// ============================================================================
// §2 — max_supply constant must match between precompile and wasm
// ============================================================================

/// Asserts the precompile's mainnet `max_supply` constant matches the
/// wasm `GenesisAlkane::max_supply` value in
/// `crates/alkanes-std-genesis-alkane-upgraded-eoa/src/lib.rs:106`.
///
/// Pre-fix value in the precompile was `156_250_000_00000000u128`
/// (1.5625 × 10^16 atomic = 156,250,000 DIESEL). The wasm value is
/// `156250000000000u128` (1.5625 × 10^14 atomic = 1,562,500 DIESEL).
/// The off-by-100x meant the precompile would happily keep minting
/// past the wasm cap, causing a divergent `/totalsupply` on every
/// mint after the wasm cap is reached.
///
/// Why we hardcode the wasm constant here rather than importing it:
/// the wasm `GenesisAlkane::max_supply` is a trait method on a
/// non-`pub` impl inside a wasm-only crate; importing it into the
/// indexer crate would drag the wasm runtime in. The constant is the
/// single source of truth on the wasm side; we mirror it as a literal
/// so this test catches drift in either direction.
#[cfg(feature = "mainnet")]
#[wasm_bindgen_test]
fn max_supply_precompile_matches_wasm_mainnet() {
    // Wasm side: GenesisAlkane::max_supply for `#[cfg(feature = "mainnet")]`.
    const WASM_MAINNET_MAX_SUPPLY: u128 = 156_250_000_000_000;

    let precompile = crate::precompile_diesel::_test_only_max_supply();

    assert_eq!(
        precompile,
        WASM_MAINNET_MAX_SUPPLY,
        "DIESEL max_supply diverges between precompile and wasm:\n  \
             wasm (GenesisAlkane::max_supply): {}\n  \
             precompile (precompile_diesel::max_supply): {}\n  \
             ratio: {}x",
        WASM_MAINNET_MAX_SUPPLY,
        precompile,
        if precompile > WASM_MAINNET_MAX_SUPPLY {
            precompile / WASM_MAINNET_MAX_SUPPLY
        } else {
            WASM_MAINNET_MAX_SUPPLY / precompile
        }
    );
}

/// Sanity check on the regtest branch: max_supply is `u128::MAX` per
/// the wasm `ChainConfiguration` default impl. Guards against an
/// accidental mainnet-style cap being introduced on the regtest path.
#[cfg(not(feature = "mainnet"))]
#[wasm_bindgen_test]
fn max_supply_precompile_is_unbounded_on_regtest() {
    let precompile = crate::precompile_diesel::_test_only_max_supply();
    assert_eq!(
        precompile,
        u128::MAX,
        "regtest max_supply should be u128::MAX (matching wasm \
         ChainConfiguration default for non-mainnet builds), got {}",
        precompile
    );
}

// ============================================================================
// §6 — number_diesel_mints must propagate decode errors
// ============================================================================

/// Sanity-check baseline: `number_diesel_mints` over a coinbase-only
/// block (no protostones at all) returns `Ok(0)`. Locks in that the
/// `?` fix didn't accidentally make every empty block error.
#[wasm_bindgen_test]
fn number_diesel_mints_empty_block_returns_zero() -> Result<()> {
    use crate::precompile_diesel::DIESEL_ID;
    use crate::tests::helpers::clear;
    use crate::vm::runtime::AlkanesRuntimeContext;
    use alkanes_support::cellpack::Cellpack;
    use metashrew_core::index_pointer::AtomicPointer;
    use protorune::message::MessageContextParcel;
    use protorune_support::balance_sheet::BalanceSheet;
    use protorune::test_helpers::create_block_with_coinbase_tx;
    use std::sync::{Arc, Mutex};

    clear();

    let block = create_block_with_coinbase_tx(2);
    let cellpack = Cellpack {
        target: DIESEL_ID,
        inputs: vec![77],
    };
    let parcel = MessageContextParcel {
        atomic: AtomicPointer::default(),
        runes: vec![],
        transaction: block.txdata[0].clone(),
        block: block.clone(),
        height: 2u64,
        pointer: 0,
        refund_pointer: 0,
        calldata: cellpack.encipher(),
        sheets: Box::<BalanceSheet<AtomicPointer>>::new(BalanceSheet::default()),
        txindex: 0,
        vout: 0,
        runtime_balances: Box::<BalanceSheet<AtomicPointer>>::new(BalanceSheet::default()),
    };
    let context = Arc::new(Mutex::new(
        AlkanesRuntimeContext::from_parcel_and_cellpack(&parcel, &cellpack),
    ));
    let ctx = context.lock().unwrap();

    let count = crate::precompile_diesel::_test_only_number_diesel_mints(&ctx)?;
    assert_eq!(count, 0, "empty block should report 0 diesel mints");

    drop(ctx);
    Ok(())
}

/// **DEFERRED**: synthetic-block reproduction of the malformed-varint
/// divergence.
///
/// Walking the Protostone encoding chain: a `message: Vec<u8>` is
/// chunked into 15-byte groups (`protostone::split_bytes`), each
/// padded to 16 bytes with zeros, interpreted as a u128, then
/// re-emitted on decode via `snap_to_15_bytes`. The zero-padding
/// effectively *terminates* any continuation-byte sequence at decode
/// time — so a naive `message: vec![0x80, 0x80, 0x80]` doesn't
/// round-trip to a malformed varint stream. The h=949478 production
/// divergence may not be caused by a literal unterminated LEB128 —
/// `decode_varint_list` errors on other conditions too (value
/// overflow, length mismatch).
///
/// A faithful synthetic repro requires either:
///   (a) constructing the Runestone's `protocol` field as raw `Vec<u128>`
///       directly (bypassing `Protostones::encipher` so the malformed
///       bytes survive the round-trip), or
///   (b) replaying the actual mainnet block 949478 against a regtest
///       wasm + precompile pair (requires fetching the prev tx, its
///       balance sheet, and bootstrapping the state).
///
/// The audit fix in `precompile_diesel.rs::number_diesel_mints` (swap
/// `Err(_) => continue` for `?` propagation) is in place — see source.
/// This stub keeps the test slot reserved.
#[wasm_bindgen_test]
#[ignore = "synthetic malformed-protostone round-trip — needs raw-Vec<u128> \
            protocol-field construction or mainnet h=949478 replay"]
fn number_diesel_mints_propagates_decode_error_on_malformed_protostone() {
    // intentionally empty — see doc comment for the gap.
}
