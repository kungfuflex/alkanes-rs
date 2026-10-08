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

#[wasm_bindgen_test]
fn test_gate_revert_reverts_mint_in_current_runtime() -> Result<()> {
    setup(true)?;
    // Put the default gate into revert mode (its own opcode 2, called directly).
    view_of(V3, DEFAULT_GATE, vec![2, 1])?;
    let supply_before = total_supply();
    let fees_before = claimable_fees();

    // DIESEL's fallback-to-fees branch is NOT reachable today: on a child
    // revert the host (`extcall`, post-V220 containment) rolls the child back
    // but sets the PARENT's remaining fuel to 0, so DIESEL traps with
    // out-of-fuel on its next instruction and the whole mint reverts. Pin that
    // so a runtime change that makes the fallback live is a visible diff.
    let b = mint_with_tail(V3 + 1, &[])?;
    assert_mint_reverted(&b, "all fuel consumed")?;
    assert_eq!(minter_diesel(&b)?, 0);
    // Atomic: supply / fee accounting untouched, gate frame rolled back.
    assert_eq!(total_supply(), supply_before);
    assert_eq!(claimable_fees(), fees_before);
    assert_eq!(gate_calls(V3 + 2, DEFAULT_GATE)?, 0);
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
