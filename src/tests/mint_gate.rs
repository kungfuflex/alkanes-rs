//! Mint gate on the DIESEL v3 genesis alkane (`alkanes-std-diesel-v3`, active
//! at `2:0` from `genesis::DIESEL_V3_BLOCK_HEIGHT`). Each test locks in one edge
//! of the design:
//!
//!   - the DEFAULT gate (`DEFAULT_MINT_GATE` = 4:1001, SUBMINER-GATE on
//!     mainnet) is live at activation with no governance tx;
//!   - EVERY mint, bare `[77]` included, routes through the gate; DIESEL calls
//!     the gate with the FIXED opcode 1 — the minter's trailing calldata cannot
//!     select a different gate entry point;
//!   - `set_mint_gate` (opcode 80) is gated on the cFIRE-SIGIL (4:866), not on
//!     DIESEL's own DSIGIL auth token; clearing to `0:0` persists as "no gate"
//!     and mints then accrue to claimable fees for the DSIGIL owner;
//!   - a gate that reverts or runs out of fuel does NOT revert the mint: DIESEL
//!     caps the gate's fuel (post-audit runtime) and routes the mint to
//!     claimable fees, so the block still emits exactly its reward; before
//!     POST_AUDIT_FORK_HEIGHT the old all-or-nothing behaviour is kept;
//!   - the per-block division remainder goes to claimable fees, so a block's
//!     emission lands exactly on the reward;
//!   - the real mainnet SUBMINER-GATE + treasury bytecode fits its allotment;
//!   - a gate that returns more DIESEL than was minted reverts the mint;
//!   - the supply cap is enforced on the FULL post-mint supply;
//!   - below `DIESEL_V3_BLOCK_HEIGHT` the pre-v3 eoa binary still governs.
//!
//! Gate and sigil are deployed at their mainnet reserved slots: the test gate
//! (alkanes-std-test-mint-gate) at 4:1001, an auth token as the sigil at 4:866.

use crate::index_block;
use crate::tests::helpers::{self as alkane_helpers, assert_revert_context, get_sheet_for_outpoint};
use crate::tests::std::{alkanes_std_auth_token_build, alkanes_std_test_mint_gate_build};
use alkanes_support::envelope::RawEnvelope;
use alkanes_support::utils::string_to_u128_list;
use metashrew_core::index_pointer::IndexPointer;
use metashrew_support::index_pointer::KeyValuePointer;
use std::sync::Arc;
use alkane_helpers::clear;
use alkanes_support::cellpack::Cellpack;
use alkanes_support::constants::AUTH_TOKEN_FACTORY_ID;
use alkanes_support::id::AlkaneId;
use anyhow::Result;
use bitcoin::{Block, OutPoint, Witness};

use crate::network::genesis;
use bitcoin::hashes::Hash;
use bitcoin::Txid;
#[allow(unused_imports)]
use metashrew_core::{
    println,
    stdio::{stdout, Write},
};
use protorune::test_helpers::{create_block_with_coinbase_tx, create_coinbase_transaction};
use protorune_support::balance_sheet::{BalanceSheetOperations, ProtoruneRuneId};
use wasm_bindgen_test::wasm_bindgen_test;

const DIESEL: AlkaneId = AlkaneId { block: 2, tx: 0 };
/// Default gate slot (SUBMINER-GATE on mainnet).
const DEFAULT_GATE: AlkaneId = AlkaneId { block: 4, tx: 1001 };
/// cFIRE-SIGIL slot.
const SIGIL: AlkaneId = AlkaneId { block: 4, tx: 866 };
/// A second test gate, deployed by plain CREATE after the upgrade's 2:1.
const OTHER_GATE: AlkaneId = AlkaneId { block: 2, tx: 2 };
/// First height at which `2:0` executes the diesel-v3 binary.
const V3: u32 = genesis::DIESEL_V3_BLOCK_HEIGHT;
/// Block reward at V3 on regtest (5e9 >> (2_000_000 / 210_000)).
const REWARD: u128 = 5_000_000_000u128 >> ((V3 as u128) / 210_000);
/// What one mint is worth in these tests: the test coinbase carries enough
/// miner fee that `diesel_fee` hits its 50%-of-reward cap, so a lone minter's
/// `value_per_mint` is REWARD - REWARD/2.
const MINT: u128 = REWARD - REWARD / 2;
/// The test gate (mode 0) hands the minter half of what it receives.
const SHARE: u128 = MINT / 2;
/// Mirrors `alkanes_std_diesel_v3::GATE_FALLBACK_RESERVE` (the contract crate
/// is not a dependency of the indexer). Keep in step.
const GATE_FALLBACK_RESERVE: u64 = 500_000;

// ---------------------------------------------------------------- setup ----

/// Block 0: seed DIESEL + the auth-token factory template.
fn setup_pre_upgrade() -> Result<()> {
    let auth_cellpack = Cellpack {
        target: AlkaneId {
            block: 3,
            tx: AUTH_TOKEN_FACTORY_ID,
        },
        inputs: vec![100],
    };
    let test_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
        [alkanes_std_auth_token_build::get_bytes()].into(),
        [auth_cellpack].into(),
    );
    index_block(&test_block, 0)?;
    Ok(())
}

/// Block 1: spend the premine into the upgrade; the owner's 5 auth-token units
/// land on the returned outpoint. (Same flow as tests::genesis_upgrade.)
///
/// This runs below `DIESEL_V3_BLOCK_HEIGHT`, i.e. against the pre-v3 eoa
/// binary. That is deliberate and is exactly how mainnet will cross the fork:
/// `/upgrade_initialized` and the auth token are already-established state that
/// v3 inherits, since the swap changes the code at `2:0` and not its storage.
fn upgrade() -> Result<OutPoint> {
    let block_height = 1;
    let outpoint = OutPoint {
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
    let upgrade_cellpack = Cellpack {
        target: DIESEL,
        inputs: vec![1],
    };
    let mut test_block = create_block_with_coinbase_tx(block_height);
    let upgrade_tx = alkane_helpers::create_multiple_cellpack_with_witness_and_in(
        Witness::new(),
        vec![upgrade_cellpack],
        outpoint,
        false,
    );
    test_block.txdata.push(upgrade_tx.clone());
    index_block(&test_block, block_height)?;
    Ok(OutPoint {
        txid: upgrade_tx.compute_txid(),
        vout: 0,
    })
}

/// Deploy `binary` with `cellpack` in its own block at `height`, spending the
/// coinbase. Returns the outpoint the deploy response landed on.
fn deploy(height: u32, binary: Vec<u8>, cellpack: Cellpack) -> Result<OutPoint> {
    let mut test_block = create_block_with_coinbase_tx(height);
    let tx = alkane_helpers::create_multiple_cellpack_with_witness_and_in(
        RawEnvelope::from(binary).to_witness(true),
        vec![cellpack],
        OutPoint::new(create_coinbase_transaction(height).compute_txid(), 0),
        false,
    );
    test_block.txdata.push(tx.clone());
    index_block(&test_block, height)?;
    Ok(OutPoint {
        txid: tx.compute_txid(),
        vout: 0,
    })
}

/// Deploy the test gate at reserved slot 4:1001 (the default gate).
fn deploy_default_gate(height: u32) -> Result<()> {
    deploy(
        height,
        alkanes_std_test_mint_gate_build::get_bytes(),
        Cellpack {
            target: AlkaneId { block: 3, tx: DEFAULT_GATE.tx },
            inputs: vec![0],
        },
    )?;
    Ok(())
}

/// Deploy a second test gate by plain CREATE (lands at OTHER_GATE = 2:2).
fn deploy_other_gate(height: u32) -> Result<()> {
    deploy(
        height,
        alkanes_std_test_mint_gate_build::get_bytes(),
        Cellpack {
            target: AlkaneId { block: 1, tx: 0 },
            inputs: vec![0],
        },
    )?;
    Ok(())
}

/// Deploy an auth token at reserved slot 4:866 as the cFIRE-SIGIL; returns the
/// outpoint holding its 5 units.
fn deploy_sigil(height: u32) -> Result<OutPoint> {
    let mut inputs = vec![0u128];
    inputs.extend(string_to_u128_list("cFIRE-SIGIL".into()));
    inputs.extend(string_to_u128_list("CFS".into()));
    inputs.push(5);
    deploy(
        height,
        alkanes_std_auth_token_build::get_bytes(),
        Cellpack {
            target: AlkaneId { block: 3, tx: SIGIL.tx },
            inputs,
        },
    )
}

/// Full pre-v3 setup: DIESEL upgraded (DSIGIL on the returned `.0`), the
/// cFIRE-SIGIL deployed (units on `.1`) and, optionally, the default gate.
fn setup(with_default_gate: bool) -> Result<(OutPoint, OutPoint)> {
    clear();
    setup_pre_upgrade()?;
    let auth = upgrade()?;
    let sigil = deploy_sigil(2)?;
    if with_default_gate {
        deploy_default_gate(3)?;
    }
    Ok((auth, sigil))
}

/// Call `target` with `inputs` from a fresh coinbase input; returns the
/// returned data of a successful call.
fn view_of(height: u32, target: AlkaneId, inputs: Vec<u128>) -> Result<Vec<u8>> {
    let mut test_block = create_block_with_coinbase_tx(height);
    let tx = alkane_helpers::create_multiple_cellpack_with_witness_and_in(
        Witness::new(),
        vec![Cellpack { target, inputs }],
        OutPoint::new(create_coinbase_transaction(height).compute_txid(), 0),
        false,
    );
    test_block.txdata.push(tx.clone());
    index_block(&test_block, height)?;
    alkane_helpers::assert_return_context(
        &OutPoint {
            txid: tx.compute_txid(),
            vout: 3,
        },
        |r| Ok(r.inner.data.clone()),
    )
}

fn gate_calls(height: u32, gate: AlkaneId) -> Result<u128> {
    let d = view_of(height, gate, vec![3])?;
    Ok(u128::from_le_bytes(d[0..16].try_into()?))
}

fn diesel_storage(key: &[u8]) -> IndexPointer {
    IndexPointer::default()
        .keyword("/alkanes/")
        .select(&DIESEL.into())
        .keyword("/storage/")
        .select(&key.to_vec())
}

fn total_supply() -> u128 {
    diesel_storage(b"/totalsupply").get_value::<u128>()
}

fn set_total_supply(v: u128) {
    diesel_storage(b"/totalsupply").set(Arc::new(v.to_le_bytes().to_vec()));
}

fn claimable_fees() -> u128 {
    diesel_storage(b"/fees").get_value::<u128>()
}

fn assert_mint_ok(b: &Block) -> Result<()> {
    alkane_helpers::assert_return_context(
        &OutPoint {
            txid: b.txdata[1].compute_txid(),
            vout: 3,
        },
        |_| Ok(()),
    )
}

fn assert_mint_reverted(b: &Block, needle: &str) -> Result<()> {
    assert_revert_context(
        &OutPoint {
            txid: b.txdata[1].compute_txid(),
            vout: 3,
        },
        needle,
    )
}

/// Run one cellpack against DIESEL in its own block, spending `input`.
/// Returns the block and the outpoint the response landed on (vout 0).
fn call_diesel(
    height: u32,
    inputs: Vec<u128>,
    input: OutPoint,
) -> Result<(Block, OutPoint, Txid)> {
    let cellpack = Cellpack {
        target: DIESEL,
        inputs,
    };
    let mut test_block = create_block_with_coinbase_tx(height);
    let tx = alkane_helpers::create_multiple_cellpack_with_witness_and_in(
        Witness::new(),
        vec![cellpack],
        input,
        false,
    );
    test_block.txdata.push(tx.clone());
    index_block(&test_block, height)?;
    let txid = tx.compute_txid();
    Ok((test_block, OutPoint { txid, vout: 0 }, txid))
}

/// Call set_mint_gate(gate) with the auth token attached; returns the outpoint
/// the (forwarded) auth token now sits on. Reverted calls still consume the
/// outpoint per protorune accounting, so callers chain the returned value.
fn set_gate(height: u32, auth: OutPoint, gate_block: u128, gate_tx: u128) -> Result<OutPoint> {
    let (_, out, _) = call_diesel(height, vec![80, gate_block, gate_tx], auth)?;
    Ok(out)
}

/// Call collect_fees (opcode 78) with the auth token attached. Returns the
/// outpoint carrying the payout + the forwarded auth token.
fn collect_fees(height: u32, auth: OutPoint) -> Result<(Block, OutPoint)> {
    let (block, out, _) = call_diesel(height, vec![78], auth)?;
    Ok((block, out))
}

/// One mint tx in its own block: calldata `[77, tail...]`.
fn mint_with_tail(height: u32, tail: &[u128]) -> Result<Block> {
    let mut inputs = vec![77u128];
    inputs.extend_from_slice(tail);
    let cellpack = Cellpack {
        target: DIESEL,
        inputs,
    };
    let mut test_block = create_block_with_coinbase_tx(height);
    let tx = alkane_helpers::create_multiple_cellpack_with_witness_and_in(
        Witness::new(),
        vec![cellpack],
        OutPoint::new(create_coinbase_transaction(height).compute_txid(), 0),
        false,
    );
    test_block.txdata.push(tx);
    index_block(&test_block, height)?;
    Ok(test_block)
}

/// DIESEL on the minter's output (tx #1, vout 0) of a one-mint block.
fn minter_diesel(block: &Block) -> Result<u128> {
    let sheet = get_sheet_for_outpoint(block, 1, 0)?;
    Ok(sheet.get(&ProtoruneRuneId { block: 2, tx: 0 }))
}

// ---------------------------------------------------------------- tests ----

#[wasm_bindgen_test]
fn test_pre_v3_still_pays_minter() -> Result<()> {
    setup(true)?;
    // Below the fork the eoa binary governs: a mint pays the caller in full,
    // exactly as it does on mainnet today, and the gate is never touched.
    let b = mint_with_tail(4, &[])?;
    assert!(minter_diesel(&b)? > 0, "pre-v3 mints must still pay the minter");
    assert_eq!(gate_calls(5, DEFAULT_GATE)?, 0);
    Ok(())
}

#[wasm_bindgen_test]
fn test_default_gate_active_without_governance() -> Result<()> {
    setup(true)?;
    let fees_before = claimable_fees();
    // No set_mint_gate ever ran: the compile-time default (4:1001) is live at
    // activation. A bare mint routes through it and the minter receives the
    // gate's share (the test gate returns half).
    let b = mint_with_tail(V3, &[])?;
    assert_mint_ok(&b)?;
    assert_eq!(minter_diesel(&b)?, SHARE);
    assert_eq!(gate_calls(V3 + 1, DEFAULT_GATE)?, 1);
    assert_eq!(
        claimable_fees() - fees_before,
        REWARD - MINT,
        "only the communist diesel_fee accrues; the gated mint itself does not"
    );

    let d = view_of(V3 + 2, DIESEL, vec![102])?;
    assert_eq!(d.len(), 42, "view layout is 42 bytes");
    assert_eq!(u128::from_le_bytes(d[0..16].try_into()?), DEFAULT_GATE.block);
    assert_eq!(u128::from_le_bytes(d[16..32].try_into()?), DEFAULT_GATE.tx);
    assert_eq!(u64::from_le_bytes(d[32..40].try_into()?), (V3 + 2) as u64);
    assert_eq!(d[40], 1, "gate is live");
    assert_eq!(d[41], 0, "source = compile-time default");
    Ok(())
}

#[wasm_bindgen_test]
fn test_missing_default_gate_reverts_mint() -> Result<()> {
    // Nothing deployed at 4:1001: the runtime traps on the missing binary
    // before the gate frame exists, so the mint reverts atomically. This pins
    // the operational requirement that SUBMINER-GATE is deployed BEFORE the
    // activation height.
    setup(false)?;
    let b = mint_with_tail(V3, &[])?;
    assert_mint_reverted(&b, "")?;
    assert_eq!(minter_diesel(&b)?, 0);
    Ok(())
}

#[wasm_bindgen_test]
fn test_bare_mint_routes_through_set_gate() -> Result<()> {
    let (_, sigil) = setup(true)?;
    deploy_other_gate(4)?;
    set_gate(V3, sigil, OTHER_GATE.block, OTHER_GATE.tx)?;

    let b = mint_with_tail(V3 + 1, &[])?;
    assert_mint_ok(&b)?;
    assert_eq!(minter_diesel(&b)?, SHARE, "bare mint split by the set gate");
    assert_eq!(gate_calls(V3 + 2, OTHER_GATE)?, 1);
    assert_eq!(gate_calls(V3 + 3, DEFAULT_GATE)?, 0);

    let d = view_of(V3 + 4, DIESEL, vec![102])?;
    assert_eq!(u128::from_le_bytes(d[0..16].try_into()?), OTHER_GATE.block);
    assert_eq!(u128::from_le_bytes(d[16..32].try_into()?), OTHER_GATE.tx);
    assert_eq!((d[40], d[41]), (1, 1), "live, set by governance");
    Ok(())
}

#[wasm_bindgen_test]
fn test_minter_cannot_choose_gate_opcode() -> Result<()> {
    setup(true)?;
    // [77, 102] would select the gate's pass-through view (the whole mint back
    // unsplit); [77, 2, 1] would even reconfigure the gate. DIESEL ignores the
    // tail and always calls opcode 1 with no further calldata.
    let mut h = V3;
    for tail in [vec![102u128], vec![2, 1], vec![7], vec![1, 5, 5]] {
        let b = mint_with_tail(h, &tail)?;
        assert_mint_ok(&b)?;
        assert_eq!(minter_diesel(&b)?, SHARE, "tail {:?} must not bypass the split", tail);
        let last = view_of(h + 1, DEFAULT_GATE, vec![4])?;
        assert_eq!(last, 1u128.to_le_bytes().to_vec(), "gate saw exactly [1]");
        h += 2;
    }
    assert_eq!(gate_calls(h, DEFAULT_GATE)?, 4);
    Ok(())
}

#[wasm_bindgen_test]
fn test_set_gate_requires_cfire_sigil() -> Result<()> {
    let (auth, _sigil) = setup(true)?;
    deploy_other_gate(4)?;

    // DIESEL's own owner token (DSIGIL) no longer governs the gate.
    let (_, _, txid) = call_diesel(V3, vec![80, OTHER_GATE.block, OTHER_GATE.tx], auth)?;
    assert_revert_context(
        &OutPoint { txid, vout: 3 },
        "cFIRE-SIGIL not present in incoming alkanes",
    )?;
    // Nothing attached at all.
    let h = V3 + 1;
    let (_, _, txid) = call_diesel(
        h,
        vec![80, 0, 0],
        OutPoint::new(create_coinbase_transaction(h).compute_txid(), 0),
    )?;
    assert_revert_context(
        &OutPoint { txid, vout: 3 },
        "cFIRE-SIGIL not present in incoming alkanes",
    )?;

    // Still the default gate.
    let b = mint_with_tail(V3 + 2, &[])?;
    assert_eq!(minter_diesel(&b)?, SHARE);
    assert_eq!(gate_calls(V3 + 3, DEFAULT_GATE)?, 1);
    assert_eq!(gate_calls(V3 + 4, OTHER_GATE)?, 0);
    Ok(())
}

#[wasm_bindgen_test]
fn test_sigil_clear_and_reset() -> Result<()> {
    let (auth, sigil) = setup(true)?;

    // Clear to 0:0 with the sigil: persists as "no gate" (distinct from the
    // unset default), so mints accrue to claimable fees.
    let sigil = set_gate(V3, sigil, 0, 0)?;
    let fees_before = claimable_fees();
    let d = view_of(V3 + 1, DIESEL, vec![102])?;
    assert_eq!(&d[0..32], &[0u8; 32]);
    assert_eq!((d[40], d[41]), (0, 2), "not live, cleared by governance");

    let b = mint_with_tail(V3 + 2, &[])?;
    assert_mint_ok(&b)?;
    assert_eq!(minter_diesel(&b)?, 0, "cleared gate: mint accrues to fees");
    // diesel_fee + the whole (ungated) mint
    assert_eq!(claimable_fees(), fees_before + REWARD);
    assert_eq!(gate_calls(V3 + 3, DEFAULT_GATE)?, 0);

    // collect_fees stays only_owner (DSIGIL), not sigil.
    let (block, _) = collect_fees(V3 + 4, auth)?;
    let collected = get_sheet_for_outpoint(&block, 1, 0)?.get(&ProtoruneRuneId { block: 2, tx: 0 });
    // The regtest premine (50_000_000) spent into the upgrade rides along on
    // the DSIGIL outpoint and is forwarded back with it.
    assert_eq!(collected, fees_before + REWARD + 50_000_000);
    assert_eq!(claimable_fees(), 0);

    // Point it back at 4:1001 explicitly: source becomes "set".
    set_gate(V3 + 5, sigil, DEFAULT_GATE.block, DEFAULT_GATE.tx)?;
    let b = mint_with_tail(V3 + 6, &[])?;
    assert_eq!(minter_diesel(&b)?, SHARE);
    let d = view_of(V3 + 7, DIESEL, vec![102])?;
    assert_eq!((d[40], d[41]), (1, 1));
    Ok(())
}

#[wasm_bindgen_test]
fn test_collect_fees_requires_auth() -> Result<()> {
    let (_, sigil) = setup(true)?;
    set_gate(V3, sigil, 0, 0)?;
    mint_with_tail(V3 + 1, &[])?;
    let height = V3 + 2;
    let (_, _, txid) = call_diesel(
        height,
        vec![78],
        OutPoint::new(create_coinbase_transaction(height).compute_txid(), 0),
    )?;
    assert_revert_context(
        &OutPoint { txid, vout: 3 },
        "Auth token is not in incoming alkanes",
    )?;
    Ok(())
}

/// Wasm fuel each frame of a mint consumed, from `crate::fuel_probe` (records
/// every frame, reverts included; excludes the per-byte storage fee charged on
/// success). A child's whole cost, storage fee included, is charged back into
/// its parent's store, so DIESEL's own record covers the gate (and treasury).
fn probe_gas(records: &[crate::fuel_probe::Record], target: AlkaneId, opcode: u128) -> u64 {
    records
        .iter()
        .filter(|r| r.target == target && r.opcode == opcode)
        .map(|r| r.gas_used)
        .sum()
}

/// From the trace: the fuel the top-level DIESEL call started with, and the
/// fuel DIESEL handed `gate` (its EnterCall).
fn mint_fuel_allotments(b: &Block, gate: AlkaneId) -> Result<(u64, u64)> {
    use alkanes_support::trace::{Trace, TraceEvent};
    let outpoint = OutPoint {
        txid: b.txdata[1].compute_txid(),
        vout: 3,
    };
    let trace: Trace = crate::view::trace(&outpoint)?.try_into()?;
    let events = trace.0.lock().expect("Mutex poisoned").clone();
    let mut diesel_start = None;
    for e in events.iter() {
        if let TraceEvent::EnterCall(c) = e {
            if diesel_start.is_none() {
                diesel_start = Some(c.fuel);
            } else if c.target == gate {
                return Ok((diesel_start.unwrap(), c.fuel));
            }
        }
    }
    Err(anyhow::anyhow!("gate was not called"))
}

/// Shared body of the two "broken gate" tests: the gate is put into `mode`
/// (1 = revert, 3 = spin until out of fuel); the mint must SUCCEED, pay the
/// minter nothing, and route its share to claimable fees, so the block still
/// emits exactly REWARD (MINT to fees via the fallback + REWARD/2 fee accrual).
fn assert_broken_gate_routes_to_fees(mode: u128) -> Result<()> {
    setup(true)?;
    view_of(V3, DEFAULT_GATE, vec![2, mode])?;
    let supply_before = total_supply();
    let fees_before = claimable_fees();

    crate::fuel_probe::clear();
    let b = mint_with_tail(V3 + 1, &[])?;
    let records = crate::fuel_probe::snapshot();
    assert_mint_ok(&b)?;
    assert_eq!(minter_diesel(&b)?, 0, "a reverted gate pays the minter nothing");
    assert_eq!(total_supply() - supply_before, REWARD, "block emits exactly the reward");
    assert_eq!(
        claimable_fees() - fees_before,
        REWARD,
        "the mint's share and the fee accrual both land in claimable fees"
    );
    // The gate frame was rolled back.
    assert_eq!(gate_calls(V3 + 2, DEFAULT_GATE)?, 0);

    // DIESEL called the gate holding exactly allotted + RESERVE (it passes
    // fuel() - RESERVE), and the gate burned (mode 3) or forfeited (mode 1)
    // its whole allotment. So what DIESEL spent AFTER the gate failed (the
    // extcall charge, the revert read-back, the claimable-fees write and the
    // response) is consumed - (start - RESERVE). It must fit the reserve with
    // a wide margin.
    let (diesel_start, allotted) = mint_fuel_allotments(&b, DEFAULT_GATE)?;
    let consumed = probe_gas(&records, DIESEL, 77);
    let fallback = consumed - (diesel_start - GATE_FALLBACK_RESERVE);
    wasm_bindgen_test::console_log!(
        "gate mode {}: DIESEL start {}; gate allotted {}; DIESEL consumed {}; fallback after the gate failed {} of reserve {}",
        mode, diesel_start, allotted, consumed, fallback, GATE_FALLBACK_RESERVE
    );
    assert!(
        fallback * 4 <= GATE_FALLBACK_RESERVE,
        "fallback used {}; keep a 4x margin under reserve {}",
        fallback,
        GATE_FALLBACK_RESERVE
    );
    Ok(())
}

#[wasm_bindgen_test]
fn test_gate_revert_routes_mint_to_claimable_fees() -> Result<()> {
    assert_broken_gate_routes_to_fees(1)
}

#[wasm_bindgen_test]
fn test_gate_out_of_fuel_routes_mint_to_claimable_fees() -> Result<()> {
    // The worst case: the gate burns its whole allotment. DIESEL's reserve is
    // never handed to it, so the fallback still runs.
    assert_broken_gate_routes_to_fees(3)
}

#[wasm_bindgen_test]
fn test_gate_revert_reverts_mint_before_post_audit_fork() -> Result<()> {
    // Below POST_AUDIT_FORK_HEIGHT the runtime ignores the extcall fuel cap
    // and zeroes the caller's fuel on a child revert (the pre-fork consensus
    // rule), so the fallback is unreachable and the whole mint reverts.
    protorune::set_post_audit_fork_height_override(Some(V3 as u64 + 1_000));
    let result = (|| -> Result<()> {
        setup(true)?;
        view_of(V3, DEFAULT_GATE, vec![2, 1])?;
        let supply_before = total_supply();
        let fees_before = claimable_fees();
        let b = mint_with_tail(V3 + 1, &[])?;
        assert_mint_reverted(&b, "all fuel consumed")?;
        assert_eq!(minter_diesel(&b)?, 0);
        assert_eq!(total_supply(), supply_before);
        assert_eq!(claimable_fees(), fees_before);
        Ok(())
    })();
    protorune::set_post_audit_fork_height_override(None);
    result
}

#[wasm_bindgen_test]
fn test_block_emission_lands_exactly_on_reward() -> Result<()> {
    // Three mints in one block: (REWARD - fee) is not divisible by 3, and the
    // remainder must go to claimable fees so supply grows by exactly REWARD.
    setup(true)?;
    let supply_before = total_supply();
    let fees_before = claimable_fees();
    let height = V3;
    let mut test_block = create_block_with_coinbase_tx(height);
    let coinbase = create_coinbase_transaction(height).compute_txid();
    for i in 0..3u32 {
        test_block.txdata.push(alkane_helpers::create_multiple_cellpack_with_witness_and_in(
            Witness::new(),
            vec![Cellpack {
                target: DIESEL,
                inputs: vec![77],
            }],
            OutPoint::new(coinbase, i),
            false,
        ));
    }
    index_block(&test_block, height)?;
    let fee = REWARD / 2;
    let per_mint = (REWARD - fee) / 3;
    let remainder = (REWARD - fee) % 3;
    assert!(remainder > 0, "test needs a non-zero remainder");
    let mut to_minters = 0u128;
    for i in 1..=3usize {
        to_minters += get_sheet_for_outpoint(&test_block, i, 0)?
            .get(&ProtoruneRuneId { block: 2, tx: 0 });
    }
    // mode-0 test gate returns half of each mint and keeps the rest
    assert_eq!(to_minters, 3 * (per_mint / 2));
    assert_eq!(total_supply() - supply_before, REWARD);
    assert_eq!(claimable_fees() - fees_before, fee + remainder);
    Ok(())
}

/// The mainnet SUBMINER-GATE (4:1001, keccak 1925e997…) and SUBMINER treasury
/// (2:96782, keccak a11e447d…) bytecode, fetched with `getbytecode`.
const MAINNET_SUBMINER_GATE: &[u8] = include_bytes!("static/mainnet_4_1001_subminer_gate.wasm");
const MAINNET_SUBMINER_TREASURY: &[u8] =
    include_bytes!("static/mainnet_2_96782_subminer_treasury.wasm");

#[wasm_bindgen_test]
fn test_mainnet_subminer_gate_fuel() -> Result<()> {
    // Deploy the exact mainnet bytecode: treasury by plain CREATE (2:2), the
    // gate at its reserved slot with the mainnet init args (caller_bp 192).
    setup(false)?;
    let treasury = AlkaneId { block: 2, tx: 2 };
    deploy(
        3,
        MAINNET_SUBMINER_TREASURY.to_vec(),
        Cellpack {
            target: AlkaneId { block: 1, tx: 0 },
            inputs: vec![0, 2, 0, SIGIL.block, SIGIL.tx],
        },
    )?;
    deploy(
        4,
        MAINNET_SUBMINER_GATE.to_vec(),
        Cellpack {
            target: AlkaneId { block: 3, tx: DEFAULT_GATE.tx },
            inputs: vec![0, 2, 0, treasury.block, treasury.tx, SIGIL.block, SIGIL.tx, 192],
        },
    )?;

    crate::fuel_probe::clear();
    let b = mint_with_tail(V3, &[])?;
    let records = crate::fuel_probe::snapshot();
    assert_mint_ok(&b)?;
    let caller_share = MINT * 192 / 10_000;
    assert_eq!(minter_diesel(&b)?, caller_share);
    let deposited = u128::from_le_bytes(view_of(V3 + 1, treasury, vec![102])?[0..16].try_into()?);
    assert_eq!(deposited, MINT - caller_share);

    // DIESEL's record includes the gate + treasury frames charged back into it,
    // so it is an upper bound on what the gate needs.
    let gate_own = probe_gas(&records, DEFAULT_GATE, 1);
    let treasury_own = probe_gas(&records, treasury, 1);
    let diesel_total = probe_gas(&records, DIESEL, 77);
    let (diesel_start, allotted) = mint_fuel_allotments(&b, DEFAULT_GATE)?;
    // DIESEL's work before the call is start - (allotted + RESERVE); the rest of
    // its consumption is the gate (incl. treasury + extcall charge) and a small
    // tail, so this is an upper bound on what the gate needs.
    let pre_gate = diesel_start - allotted - GATE_FALLBACK_RESERVE;
    let gate_upper = diesel_total - pre_gate;
    wasm_bindgen_test::console_log!(
        "SUBMINER-GATE 4:1001 mint-split: gate wasm {} + treasury deposit wasm {}; gate incl. charges (upper bound) {}; allotted {} on a {}-fuel tx",
        gate_own, treasury_own, gate_upper, allotted, diesel_start
    );
    assert!(gate_own > 0 && treasury_own > 0, "both frames must have run");
    // The test tx carries the minimum per-tx fuel, so `allotted` is the least
    // the gate can get on mainnet. Keep a 4x margin.
    assert_eq!(diesel_start, crate::vm::fuel::minimum_fuel(V3));
    assert!(
        gate_upper * 4 <= allotted,
        "gate used up to {} fuel; allotted {}",
        gate_upper,
        allotted
    );
    Ok(())
}

#[wasm_bindgen_test]
fn test_gate_returning_more_than_minted_reverts() -> Result<()> {
    setup(true)?;
    // First mint: the gate keeps half, so it now holds REWARD - REWARD/2.
    let b = mint_with_tail(V3, &[])?;
    assert_eq!(minter_diesel(&b)?, SHARE);
    let supply_before = total_supply();
    // Mode 2: return the incoming mint PLUS everything previously retained.
    view_of(V3 + 1, DEFAULT_GATE, vec![2, 2])?;
    let b = mint_with_tail(V3 + 2, &[])?;
    assert_mint_reverted(&b, "mint gate returned")?;
    assert_eq!(minter_diesel(&b)?, 0);
    assert_eq!(total_supply(), supply_before, "reverted mint leaves supply untouched");
    Ok(())
}

#[wasm_bindgen_test]
fn test_cap_final_mint_clamped_to_headroom() -> Result<()> {
    // Regtest max_supply is u128::MAX, so the cap edge here is also the
    // overflow edge: the old `total_supply + value` would wrap.
    let (_, sigil) = setup(true)?;
    set_gate(V3, sigil, 0, 0)?; // accrue to fees: the full minted value is observable
    set_total_supply(u128::MAX - 1000);
    let fees_before = claimable_fees();
    let b = mint_with_tail(V3 + 1, &[])?;
    assert_mint_ok(&b)?;
    assert_eq!(total_supply(), u128::MAX, "final mint lands exactly on the cap");
    // The minter's share is committed first (clamped to 1000); the diesel_fee
    // finds no headroom left and accrues 0.
    assert_eq!(claimable_fees() - fees_before, 1000, "final mint clamped to the headroom");

    // At the cap: rejected, nothing changes.
    let b = mint_with_tail(V3 + 2, &[])?;
    assert_mint_reverted(&b, "total supply has been reached")?;
    assert_eq!(total_supply(), u128::MAX);
    assert_eq!(claimable_fees() - fees_before, 1000);
    Ok(())
}

#[wasm_bindgen_test]
fn test_cap_exact_boundary_and_overflow_edge() -> Result<()> {
    setup(true)?;
    // Supply exactly one full mint below the cap: the whole mint goes through
    // the gate (half back to the minter); the diesel_fee finds no headroom.
    set_total_supply(u128::MAX - MINT);
    let b = mint_with_tail(V3, &[])?;
    assert_mint_ok(&b)?;
    assert_eq!(minter_diesel(&b)?, SHARE);
    assert_eq!(total_supply(), u128::MAX);

    // One unit below the cap (old code: u128 overflow): mint of exactly 1,
    // forwarded to the gate (which returns floor(1/2) = 0).
    set_total_supply(u128::MAX - 1);
    let b = mint_with_tail(V3 + 1, &[])?;
    assert_mint_ok(&b)?;
    assert_eq!(total_supply(), u128::MAX);
    assert_eq!(gate_calls(V3 + 2, DEFAULT_GATE)?, 2);
    Ok(())
}
