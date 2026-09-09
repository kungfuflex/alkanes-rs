//! Side-by-side comparison of DIESEL wasm execution and the native
//! precompile implementation.
//!
//! For each enumerated code path:
//!   1. Build a parcel + context that exercises the path.
//!   2. Run the wasm via `run_after_special` — capture `(response_w,
//!      gas_w)`. Note: `run_after_special` does NOT mutate atomic for
//!      DIESEL mints because the StorageMap is returned unapplied; it is
//!      `pipe_storagemap_to` (called from `message.rs::handle_message`)
//!      that applies it. So both sides leave atomic alone.
//!   3. Run the precompile on a freshly-built context with the same
//!      atomic state — capture `(response_p, gas_p)`.
//!   4. Assert `response_w == response_p` field-by-field, and
//!      `gas_w == gas_p`.
//!
//! Any divergence is a consensus bug — the test prints the field-level
//! diff and aborts.

use crate::fuel_probe;
use crate::index_block;
use crate::network::genesis;
use crate::precompile_diesel::{
    self, DieselPath, DieselPathGas, DIESEL_ID,
};
use crate::tests::helpers::{self as alkane_helpers, clear};
use crate::tests::std::alkanes_std_auth_token_build;
use crate::vm::fuel::FuelTank;
use crate::vm::runtime::AlkanesRuntimeContext;
use crate::vm::utils::{prepare_context, run_after_special, run_special_cellpacks};
use alkanes_support::cellpack::Cellpack;
use alkanes_support::constants::AUTH_TOKEN_FACTORY_ID;
use alkanes_support::id::AlkaneId;
use alkanes_support::parcel::AlkaneTransferParcel;
use alkanes_support::response::ExtendedCallResponse;
use alkanes_support::storage::StorageMap;
use anyhow::Result;
use bitcoin::hashes::Hash;
use bitcoin::{Block, OutPoint, Transaction, Txid, Witness};
use metashrew_core::index_pointer::{AtomicPointer, IndexPointer};
use metashrew_support::index_pointer::KeyValuePointer;
use protorune::message::MessageContextParcel;
use protorune_support::balance_sheet::BalanceSheet;
use protorune::test_helpers::{create_block_with_coinbase_tx, create_coinbase_transaction};
use std::sync::{Arc, Mutex};
use wasm_bindgen_test::wasm_bindgen_test;

#[allow(unused_imports)]
use metashrew_core::{println, print, stdio::{stdout, Write}};

/// Builds the indexer state required to exercise communist DIESEL paths:
///   * height 0: auth-token factory deployed (triggers setup_diesel +
///     EOA binary at 2:0).
///   * `/upgrade_initialized` set DIRECTLY via IndexPointer (no opcode-1
///     wasm tx). This bypasses the upgrade's premine check, which would
///     otherwise reject because no DIESEL.initialize was run beforehand
///     to mint premine into the `GENESIS_OUTPOINT` UTXO the upgrade tx
///     references as its prevout. Without this direct-write, the upgrade
///     reverts silently and subsequent mints take the legacy path —
///     which kept these proof-tests red since task #14 landed.
///
///     See the longer-form rationale on
///     `diesel_shadow::run_upgrade`; same key path
///     (`/alkanes/<2:0>/storage//upgrade_initialized`), same single-byte
///     `0x01` payload the wasm `upgrade()` handler writes.
fn setup_with_upgrade() -> Result<()> {
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

    // Direct-write the upgrade flag — see fn doc-comment for the
    // wasm-upgrade-reverts-silently rationale.
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

/// Build a parcel that mirrors what `Protorune::index_block` would build
/// for the i-th non-coinbase tx in `block`.
fn parcel_for_tx(
    block: &Block,
    height: u32,
    txindex: u32,
    tx: &Transaction,
    calldata: Vec<u8>,
    vout: u32,
) -> MessageContextParcel {
    MessageContextParcel {
        atomic: AtomicPointer::default(),
        runes: vec![],
        transaction: tx.clone(),
        block: block.clone(),
        height: height as u64,
        pointer: 0,
        refund_pointer: 0,
        calldata,
        sheets: Box::<BalanceSheet<AtomicPointer>>::new(BalanceSheet::default()),
        txindex,
        vout,
        runtime_balances: Box::<BalanceSheet<AtomicPointer>>::new(BalanceSheet::default()),
    }
}

/// Run the wasm DIESEL implementation against a synthetic parcel and
/// return (response, wasmi_internal_gas, total_gas_incl_storage).
fn run_wasm_path(
    parcel: &MessageContextParcel,
    cellpack: &Cellpack,
) -> Result<(ExtendedCallResponse, u64, u64)> {
    let context = Arc::new(Mutex::new(
        AlkanesRuntimeContext::from_parcel_and_cellpack(parcel, cellpack),
    ));
    let (caller, myself, binary) =
        run_special_cellpacks(context.clone(), cellpack)?;
    prepare_context(context.clone(), &caller, &myself, false);

    // The fuel allocation matches what handle_message would compute at the
    // top of a top-level call. For these tests we use the per-tx minimum.
    let start_fuel = crate::vm::fuel::minimum_fuel(parcel.height as u32);
    fuel_probe::clear();
    // run_after_special would normally dispatch to the precompile on
    // regtest (V220_FORK_HEIGHT=0). Enable shadow mode so the dispatcher
    // skips the precompile and the wasm path runs, which is what this
    // test wants to observe.
    crate::precompile_diesel::shadow_enable();
    let result = run_after_special(context.clone(), binary.clone(), start_fuel);
    crate::precompile_diesel::shadow_disable();
    let (response, total_gas) = result?;
    let wasmi_internal_gas = fuel_probe::snapshot()
        .iter()
        .last()
        .map(|r| r.gas_used)
        .ok_or_else(|| anyhow::anyhow!("fuel_probe did not record a wasm call"))?;
    Ok((response, wasmi_internal_gas, total_gas))
}

/// Run the precompile DIESEL implementation against a synthetic parcel.
/// `expected_path` is checked against the path actually taken.
fn run_precompile_path(
    parcel: &MessageContextParcel,
    cellpack: &Cellpack,
    table: &DieselPathGas,
    expected_path: DieselPath,
) -> Result<(ExtendedCallResponse, u64, DieselPath)> {
    let context = Arc::new(Mutex::new(
        AlkanesRuntimeContext::from_parcel_and_cellpack(parcel, cellpack),
    ));
    let (caller, myself, _binary) =
        run_special_cellpacks(context.clone(), cellpack)?;
    prepare_context(context.clone(), &caller, &myself, false);
    let (response, gas, path) = precompile_diesel::run_diesel_eoa(context, table)?;
    assert_eq!(path, expected_path, "precompile took unexpected path");
    Ok((response, gas, path))
}

/// Compares two ExtendedCallResponses field-by-field and prints a diff if
/// they don't match.
fn assert_responses_equal(
    wasm: &ExtendedCallResponse,
    precomp: &ExtendedCallResponse,
    label: &str,
) {
    if wasm.alkanes != precomp.alkanes {
        println!(
            "[{}] alkanes mismatch:\n  wasm: {:?}\n  precomp: {:?}",
            label, wasm.alkanes, precomp.alkanes
        );
    }
    if wasm.data != precomp.data {
        println!(
            "[{}] data mismatch:\n  wasm: {}\n  precomp: {}",
            label,
            hex::encode(&wasm.data),
            hex::encode(&precomp.data)
        );
    }
    if wasm.storage != precomp.storage {
        println!("[{}] storage mismatch:", label);
        println!("  wasm storage map ({} entries):", wasm.storage.0.len());
        for (k, v) in &wasm.storage.0 {
            println!(
                "    {} -> {}",
                hex::encode(k),
                hex::encode(v)
            );
        }
        println!("  precomp storage map ({} entries):", precomp.storage.0.len());
        for (k, v) in &precomp.storage.0 {
            println!(
                "    {} -> {}",
                hex::encode(k),
                hex::encode(v)
            );
        }
    }
    assert_eq!(wasm.alkanes, precomp.alkanes, "[{}] alkanes diverged", label);
    assert_eq!(wasm.data, precomp.data, "[{}] data diverged", label);
    assert_eq!(wasm.storage, precomp.storage, "[{}] storage diverged", label);
}

#[wasm_bindgen_test]
fn diesel_sidebyside_communist_first_of_block() -> Result<()> {
    clear();
    setup_with_upgrade()?;

    // Build a 1-mint block at height 2 (only 1 DIESEL invocation; this is
    // the C1 path).
    let height: u32 = 2;
    let block = build_mint_block(height, 1);
    let mint_tx = &block.txdata[1];
    let cellpack = Cellpack {
        target: DIESEL_ID,
        inputs: vec![77],
    };
    let parcel = parcel_for_tx(&block, height, 1, mint_tx, cellpack.encipher(), 3);
    FuelTank::initialize(&block, height);
    FuelTank::fuel_transaction(parcel.transaction.weight().to_wu() / 4, 1, height);

    let (wasm_resp, wasm_internal_gas, wasm_total) = run_wasm_path(&parcel, &cellpack)?;

    // Calibrate the table from the wasm INTERNAL gas. The precompile
    // adds storage_fuel on top to produce a total matching wasm_total.
    let mut table = DieselPathGas::default();
    table.communist_first_of_block = wasm_internal_gas;

    let (precomp_resp, precomp_gas, _path) = run_precompile_path(
        &parcel,
        &cellpack,
        &table,
        DieselPath::CommunistFirstOfBlock,
    )?;

    println!(
        "[C1] wasm internal={} total={}, precomp total={}, storage_len={}",
        wasm_internal_gas,
        wasm_total,
        precomp_gas,
        wasm_resp.storage.0.len()
    );
    assert_responses_equal(&wasm_resp, &precomp_resp, "C1");
    assert_eq!(precomp_gas, wasm_total, "C1 total gas drift");

    Ok(())
}

#[wasm_bindgen_test]
fn diesel_sidebyside_communist_subsequent() -> Result<()> {
    clear();
    setup_with_upgrade()?;

    // Apply a fake "1st mint already happened" state. The wasm path's
    // observe_upgraded_mint checks /upgraded_seen/<height> via the host's
    // `__load_storage` which reads from
    // `ctx.message.atomic.keyword("/alkanes/").select(&myself.into())
    //  .keyword("/storage/").select(&key)`. We have to set the same path.
    let height: u32 = 2;
    let mut upgraded_seen_key = b"/upgraded_seen/".to_vec();
    upgraded_seen_key.extend_from_slice(&(height as u64).to_le_bytes());
    let mut ptr = IndexPointer::default()
        .keyword("/alkanes/")
        .select(&DIESEL_ID.into())
        .keyword("/storage/")
        .select(&upgraded_seen_key);
    ptr.set_value::<u32>(1);
    // also pre-populate /fees and /totalsupply as the 1st mint would have
    let mut fees_ptr = IndexPointer::default()
        .keyword("/alkanes/")
        .select(&DIESEL_ID.into())
        .keyword("/storage/")
        .select(&b"/fees".to_vec());
    fees_ptr.set(Arc::new(0u128.to_le_bytes().to_vec()));
    let mut ts_ptr = IndexPointer::default()
        .keyword("/alkanes/")
        .select(&DIESEL_ID.into())
        .keyword("/storage/")
        .select(&b"/totalsupply".to_vec());
    ts_ptr.set(Arc::new(50_000_000u128.to_le_bytes().to_vec()));

    // Now build a 2-mint block so number_diesel_mints returns 2 (matches
    // what a "real" mint in this hypothetical block would see).
    let block = build_mint_block(height, 2);
    // Use the SECOND tx (index 2 in block, the "Nth" mint).
    let mint_tx = &block.txdata[2];
    let cellpack = Cellpack {
        target: DIESEL_ID,
        inputs: vec![77],
    };
    let parcel = parcel_for_tx(&block, height, 2, mint_tx, cellpack.encipher(), 3);
    FuelTank::initialize(&block, height);
    FuelTank::fuel_transaction(parcel.transaction.weight().to_wu() / 4, 2, height);

    let (wasm_resp, wasm_internal_gas, wasm_total) = run_wasm_path(&parcel, &cellpack)?;
    let mut table = DieselPathGas::default();
    table.communist_subsequent = wasm_internal_gas;

    let (precomp_resp, precomp_gas, _) = run_precompile_path(
        &parcel,
        &cellpack,
        &table,
        DieselPath::CommunistSubsequent,
    )?;

    println!(
        "[C2] wasm internal={} total={}, precomp total={}, storage_len={}",
        wasm_internal_gas,
        wasm_total,
        precomp_gas,
        wasm_resp.storage.0.len()
    );
    assert_responses_equal(&wasm_resp, &precomp_resp, "C2");
    assert_eq!(precomp_gas, wasm_total, "C2 total gas drift");

    Ok(())
}

#[wasm_bindgen_test]
fn diesel_sidebyside_view_get_total_supply() -> Result<()> {
    clear();
    setup_with_upgrade()?;
    // Bump total supply so we see a non-zero return.
    let mut ts_ptr = IndexPointer::default()
        .keyword("/alkanes/")
        .select(&DIESEL_ID.into())
        .keyword("/storage/")
        .select(&b"/totalsupply".to_vec());
    ts_ptr.set(Arc::new(123456789u128.to_le_bytes().to_vec()));

    let height: u32 = 2;
    let mut block = create_block_with_coinbase_tx(height);
    let cellpack = Cellpack {
        target: DIESEL_ID,
        inputs: vec![101],
    };
    let cb_txid = block.txdata[0].compute_txid();
    let view_tx = alkane_helpers::create_multiple_cellpack_with_witness_and_in(
        Witness::new(),
        vec![cellpack.clone()],
        OutPoint::new(cb_txid, 0),
        false,
    );
    block.txdata.push(view_tx);
    let parcel = parcel_for_tx(&block, height, 1, &block.txdata[1], cellpack.encipher(), 3);
    FuelTank::initialize(&block, height);
    FuelTank::fuel_transaction(parcel.transaction.weight().to_wu() / 4, 1, height);

    let (wasm_resp, wasm_internal_gas, wasm_total) = run_wasm_path(&parcel, &cellpack)?;
    let mut table = DieselPathGas::default();
    table.view_get_total_supply = wasm_internal_gas;

    let (precomp_resp, precomp_gas, _) = run_precompile_path(
        &parcel,
        &cellpack,
        &table,
        DieselPath::ViewGetTotalSupply,
    )?;

    println!(
        "[V101] wasm internal={} total={}, precomp total={}, data={}",
        wasm_internal_gas,
        wasm_total,
        precomp_gas,
        hex::encode(&wasm_resp.data)
    );
    assert_responses_equal(&wasm_resp, &precomp_resp, "V101");
    assert_eq!(precomp_gas, wasm_total, "V101 total gas drift");
    Ok(())
}
