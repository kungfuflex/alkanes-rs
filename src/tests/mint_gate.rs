//! Mint-gate (diesel-gate branch) — governance-set forward target on the EOA
//! genesis mint, with a one-week timelock, immediate off, and a dead-man's
//! switch. Each test locks in one edge of the design:
//!
//!   - default OFF: no gate → the mint pays the caller, byte-for-byte today's
//!     behaviour;
//!   - set_mint_gate is only_owner: no auth token in the incoming alkanes, no
//!     switch;
//!   - a set gate is PENDING for GATE_DELAY (1008) blocks: mints inside the
//!     window still pay the caller; after activation they forward to the gate
//!     with the calldata that follows opcode 77;
//!   - a bare mint ([77] with no trailing calldata) stays an ordinary mint even
//!     with a live gate;
//!   - re-setting within the pending window cannot promote a never-activated
//!     target past its own delay (no timelock bypass by double-set), and a
//!     broken gate reverts the mint atomically once it DOES activate;
//!   - clearing (0:0) is immediate — the safe default is never delayed;
//!   - the gate auto-expires to OFF after GATE_TTL (52_560) blocks without a
//!     re-affirm (dead-man's switch).
//!
//! The gate target used here is alkanes-std-test whose opcode 7 (Donate)
//! swallows every incoming alkane — so "the gate got the mint" is observable as
//! the minter's sheet holding zero DIESEL.

use crate::index_block;
use crate::tests::helpers::{self as alkane_helpers, assert_revert_context, get_sheet_for_outpoint};
use crate::tests::std::{alkanes_std_auth_token_build, alkanes_std_test_build};
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
/// Where the std-test gate lands: the auth token took sequence slot 2:1, the
/// next created alkane is 2:2.
const GATE: AlkaneId = AlkaneId { block: 2, tx: 2 };
const GATE_DELAY: u64 = 1_008;
const GATE_TTL: u64 = 52_560;
/// Donate — the std-test opcode that swallows every incoming alkane.
const OP_DONATE: u128 = 7;

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

/// Deploy alkanes-std-test as the gate contract (lands at GATE = 2:2).
fn deploy_gate(height: u32) -> Result<()> {
    let test_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
        [alkanes_std_test_build::get_bytes()].into(),
        [Cellpack {
            target: AlkaneId { block: 1, tx: 0 },
            inputs: vec![0],
        }]
        .into(),
    );
    index_block(&test_block, height)?;
    Ok(())
}

/// Call set_mint_gate(gate) with the auth token attached; returns the outpoint
/// the (forwarded) auth token now sits on. Reverted calls still consume the
/// outpoint per protorune accounting, so callers chain the returned value.
fn set_gate(height: u32, auth: OutPoint, gate_block: u128, gate_tx: u128) -> Result<OutPoint> {
    let cellpack = Cellpack {
        target: DIESEL,
        inputs: vec![80, gate_block, gate_tx],
    };
    let mut test_block = create_block_with_coinbase_tx(height);
    let tx = alkane_helpers::create_multiple_cellpack_with_witness_and_in(
        Witness::new(),
        vec![cellpack],
        auth,
        false,
    );
    test_block.txdata.push(tx.clone());
    index_block(&test_block, height)?;
    Ok(OutPoint {
        txid: tx.compute_txid(),
        vout: 0,
    })
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
fn test_gate_default_off() -> Result<()> {
    clear();
    setup_pre_upgrade()?;
    upgrade()?;
    // No gate was ever set: a mint (even with trailing calldata) pays the
    // caller — today's behaviour is the default.
    let b = mint_with_tail(2, &[OP_DONATE])?;
    assert!(minter_diesel(&b)? > 0, "default: minter must be paid");
    Ok(())
}

#[wasm_bindgen_test]
fn test_set_gate_requires_auth() -> Result<()> {
    clear();
    setup_pre_upgrade()?;
    upgrade()?;
    // No auth token attached → only_owner must revert the switch.
    let cellpack = Cellpack {
        target: DIESEL,
        inputs: vec![80, GATE.block, GATE.tx],
    };
    let height = 2;
    let mut test_block = create_block_with_coinbase_tx(height);
    let tx = alkane_helpers::create_multiple_cellpack_with_witness_and_in(
        Witness::new(),
        vec![cellpack],
        OutPoint::new(create_coinbase_transaction(height).compute_txid(), 0),
        false,
    );
    test_block.txdata.push(tx.clone());
    index_block(&test_block, height)?;
    assert_revert_context(
        &OutPoint {
            txid: tx.compute_txid(),
            vout: 3,
        },
        "Auth token is not in incoming alkanes",
    )?;
    // And the gate must still be off.
    let b = mint_with_tail(3, &[OP_DONATE])?;
    assert!(minter_diesel(&b)? > 0, "gate must not have been set");
    Ok(())
}

#[wasm_bindgen_test]
fn test_gate_timelock_pending_then_active() -> Result<()> {
    clear();
    setup_pre_upgrade()?;
    let auth = upgrade()?;
    deploy_gate(2)?;
    let set_h: u32 = 3;
    set_gate(set_h, auth, GATE.block, GATE.tx)?;

    // Inside the pending window: the mint still pays the caller.
    let b = mint_with_tail(set_h + 1, &[OP_DONATE])?;
    assert!(
        minter_diesel(&b)? > 0,
        "pending gate must not intercept mints"
    );

    // After GATE_DELAY: the mint forwards to the gate (Donate swallows it) —
    // and it is a SUCCESS, not a revert (sheet-empty alone cannot tell those
    // apart, so pin the return context too).
    let active_h = set_h + (GATE_DELAY as u32) + 1;
    let b = mint_with_tail(active_h, &[OP_DONATE])?;
    assert_eq!(
        minter_diesel(&b)?,
        0,
        "active gate must receive the whole mint"
    );
    alkane_helpers::assert_return_context(
        &OutPoint {
            txid: b.txdata[1].compute_txid(),
            vout: 3,
        },
        |_| Ok(()),
    )?;

    // A bare mint (no trailing calldata) stays an ordinary mint even with a
    // live gate: calldata is zero-padded on the wire, so [77] arrives as
    // [77, 0, ...]; a zero gate-opcode must NOT trigger a forward (that would
    // freeze ordinary minting once a gate is set).
    let b = mint_with_tail(active_h + 1, &[])?;
    assert!(minter_diesel(&b)? > 0, "bare mint must stay ordinary");
    Ok(())
}

#[wasm_bindgen_test]
fn test_double_set_cannot_bypass_timelock() -> Result<()> {
    clear();
    setup_pre_upgrade()?;
    let auth = upgrade()?;
    deploy_gate(2)?;
    // Activate the good gate A.
    let auth = set_gate(3, auth, GATE.block, GATE.tx)?;
    let a_active: u32 = 3 + (GATE_DELAY as u32) + 1;

    // Point at a BROKEN gate B (nothing deployed at 2:99), twice in a row.
    // If the second set promoted the first (never-activated) B into the
    // fallback slot, mints would start reverting immediately.
    let auth = set_gate(a_active, auth, 2, 99)?;
    let _auth = set_gate(a_active + 1, auth, 2, 99)?;

    // Still inside B's window: mints must run through A (swallowed, not
    // reverted, not paid to the miner).
    let b = mint_with_tail(a_active + 2, &[OP_DONATE])?;
    assert_eq!(minter_diesel(&b)?, 0, "gate A must still govern");

    // Once B activates, the broken gate reverts the mint ATOMICALLY — no
    // fall-back to an ordinary mint (that would be the abusable path). A
    // swallowed mint and a reverted mint both leave the minter's sheet empty,
    // so assert the REVERT itself: the trace's last event must be a
    // RevertContext (the empty needle matches any message; the event type is
    // what distinguishes revert from Donate's silent success).
    let b_active = a_active + 1 + (GATE_DELAY as u32) + 1;
    let b = mint_with_tail(b_active, &[OP_DONATE])?;
    assert_revert_context(
        &OutPoint {
            txid: b.txdata[1].compute_txid(),
            vout: 3,
        },
        "",
    )?;
    assert_eq!(minter_diesel(&b)?, 0);
    Ok(())
}

#[wasm_bindgen_test]
fn test_clear_is_immediate() -> Result<()> {
    clear();
    setup_pre_upgrade()?;
    let auth = upgrade()?;
    deploy_gate(2)?;
    let auth = set_gate(3, auth, GATE.block, GATE.tx)?;
    let active_h: u32 = 3 + (GATE_DELAY as u32) + 1;
    // Gate is live…
    let b = mint_with_tail(active_h, &[OP_DONATE])?;
    assert_eq!(minter_diesel(&b)?, 0);
    // …turn it OFF: takes effect at once, no pending window.
    let _auth = set_gate(active_h + 1, auth, 0, 0)?;
    let b = mint_with_tail(active_h + 2, &[OP_DONATE])?;
    assert!(minter_diesel(&b)? > 0, "0:0 must switch off immediately");
    Ok(())
}

#[wasm_bindgen_test]
fn test_ttl_dead_mans_switch() -> Result<()> {
    clear();
    setup_pre_upgrade()?;
    let auth = upgrade()?;
    deploy_gate(2)?;
    let set_h: u32 = 3;
    set_gate(set_h, auth, GATE.block, GATE.tx)?;
    // Live after the delay…
    let b = mint_with_tail(set_h + (GATE_DELAY as u32) + 1, &[OP_DONATE])?;
    assert_eq!(minter_diesel(&b)?, 0);
    // …but with no re-affirm for GATE_TTL blocks the gate lapses to OFF.
    let expired_h = set_h + (GATE_TTL as u32) + 1;
    let b = mint_with_tail(expired_h, &[OP_DONATE])?;
    assert!(
        minter_diesel(&b)? > 0,
        "an unaffirmed gate must expire to the default"
    );
    Ok(())
}

#[wasm_bindgen_test]
fn test_view_shows_pending_before_active() -> Result<()> {
    clear();
    setup_pre_upgrade()?;
    let auth = upgrade()?;
    deploy_gate(2)?;
    let set_h: u32 = 3;
    set_gate(set_h, auth, GATE.block, GATE.tx)?;

    // Inside the window get_mint_gate must expose the pending switch: nothing
    // governs yet (live = 0), while the pending target and its activation
    // height are public — the whole point of the timelock.
    let height = set_h + 1;
    let view_cellpack = Cellpack {
        target: DIESEL,
        inputs: vec![102],
    };
    let mut test_block = create_block_with_coinbase_tx(height);
    let tx = alkane_helpers::create_multiple_cellpack_with_witness_and_in(
        Witness::new(),
        vec![view_cellpack],
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
        |trace_response| {
            let d = &trace_response.inner.data;
            assert_eq!(d.len(), 97, "view layout is 97 bytes");
            let gov_block = u128::from_le_bytes(d[0..16].try_into()?);
            let gov_tx = u128::from_le_bytes(d[16..32].try_into()?);
            let pend_block = u128::from_le_bytes(d[48..64].try_into()?);
            let pend_tx = u128::from_le_bytes(d[64..80].try_into()?);
            let pend_act = u64::from_le_bytes(d[80..88].try_into()?);
            let live = d[96];
            assert_eq!((gov_block, gov_tx), (0, 0), "nothing governs yet");
            assert_eq!((pend_block, pend_tx), (GATE.block, GATE.tx));
            assert_eq!(pend_act, set_h as u64 + GATE_DELAY);
            assert_eq!(live, 0);
            Ok(())
        },
    )
}
