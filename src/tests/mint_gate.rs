//! Mint gate on the DIESEL v3 genesis alkane (`alkanes-std-diesel-v3`, active
//! at `2:0` from `genesis::DIESEL_V3_BLOCK_HEIGHT`). Each test locks in one edge
//! of the design:
//!
//!   - default (no gate): the mint is NOT paid to the caller — it accrues to the
//!     contract's claimable-fees balance, which only the DSIGIL auth-token
//!     holder can withdraw via `collect_fees` (opcode 78, `only_owner`);
//!   - `set_mint_gate` (opcode 80) is `only_owner`: no auth token in the
//!     incoming alkanes, no switch;
//!   - a set gate takes effect IMMEDIATELY — there is no timelock and no expiry
//!     in v3 — and mints forward to it with the calldata that follows opcode 77;
//!   - a bare mint (`[77]` with no trailing calldata) does not forward even with
//!     a live gate; it falls back to the accrue-to-owner default;
//!   - a broken gate reverts the mint atomically (no fall-back to an ordinary
//!     mint, which would be the abusable path);
//!   - clearing (`0:0`) returns to the default;
//!   - below `DIESEL_V3_BLOCK_HEIGHT` the pre-v3 eoa binary still governs and
//!     pays the minter, which pins the fork boundary itself.
//!
//! The gate target used here is alkanes-std-test, whose opcode 7 (Donate)
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
/// Donate — the std-test opcode that swallows every incoming alkane.
const OP_DONATE: u128 = 7;
/// First height at which `2:0` executes the diesel-v3 binary.
const V3: u32 = genesis::DIESEL_V3_BLOCK_HEIGHT;

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
    clear();
    setup_pre_upgrade()?;
    upgrade()?;
    // Below the fork the eoa binary governs: a mint pays the caller, exactly as
    // it does on mainnet today. This pins the boundary — without it, a v3
    // regression that also happened to be active pre-fork would go unnoticed.
    let b = mint_with_tail(2, &[OP_DONATE])?;
    assert!(
        minter_diesel(&b)? > 0,
        "pre-v3 mints must still pay the minter"
    );
    Ok(())
}

#[wasm_bindgen_test]
fn test_no_gate_accrues_to_owner() -> Result<()> {
    clear();
    setup_pre_upgrade()?;
    let auth = upgrade()?;

    // v3 default: no gate is set, so the mint pays the caller NOTHING.
    let b = mint_with_tail(V3, &[OP_DONATE])?;
    assert_eq!(
        minter_diesel(&b)?,
        0,
        "v3 default must not pay the minter"
    );

    // The emission is not lost: it accrued to claimable fees, and the DSIGIL
    // auth-token holder can withdraw it.
    let (block, _) = collect_fees(V3 + 1, auth)?;
    let collected = get_sheet_for_outpoint(&block, 1, 0)?.get(&ProtoruneRuneId { block: 2, tx: 0 });
    assert!(
        collected > 0,
        "the owner must be able to collect the accrued mint"
    );
    Ok(())
}

#[wasm_bindgen_test]
fn test_collect_fees_requires_auth() -> Result<()> {
    clear();
    setup_pre_upgrade()?;
    upgrade()?;
    mint_with_tail(V3, &[OP_DONATE])?;

    // Accrued fees are only_owner: without the auth token the collect reverts,
    // so the balance cannot be swept by an arbitrary caller.
    let height = V3 + 1;
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
fn test_set_gate_requires_auth() -> Result<()> {
    clear();
    setup_pre_upgrade()?;
    upgrade()?;
    deploy_gate(2)?;

    // No auth token attached → only_owner must revert the switch.
    let height = V3;
    let (_, _, txid) = call_diesel(
        height,
        vec![80, GATE.block, GATE.tx],
        OutPoint::new(create_coinbase_transaction(height).compute_txid(), 0),
    )?;
    assert_revert_context(
        &OutPoint { txid, vout: 3 },
        "Auth token is not in incoming alkanes",
    )?;

    // And the gate must still be off: a mint accrues rather than forwarding.
    let b = mint_with_tail(V3 + 1, &[OP_DONATE])?;
    assert_eq!(minter_diesel(&b)?, 0, "gate must not have been set");
    Ok(())
}

#[wasm_bindgen_test]
fn test_gate_forwards_immediately() -> Result<()> {
    clear();
    setup_pre_upgrade()?;
    let auth = upgrade()?;
    deploy_gate(2)?;
    set_gate(V3, auth, GATE.block, GATE.tx)?;

    // No timelock in v3: the very next block already forwards to the gate,
    // where Donate swallows the mint. Sheet-empty alone cannot distinguish a
    // swallowed mint from a reverted one, so pin the return context too.
    let b = mint_with_tail(V3 + 1, &[OP_DONATE])?;
    assert_eq!(
        minter_diesel(&b)?,
        0,
        "an active gate must receive the whole mint"
    );
    alkane_helpers::assert_return_context(
        &OutPoint {
            txid: b.txdata[1].compute_txid(),
            vout: 3,
        },
        |_| Ok(()),
    )?;
    Ok(())
}

#[wasm_bindgen_test]
fn test_bare_mint_does_not_forward() -> Result<()> {
    clear();
    setup_pre_upgrade()?;
    let auth = upgrade()?;
    deploy_gate(2)?;
    set_gate(V3, auth, GATE.block, GATE.tx)?;

    // Calldata is zero-padded on the wire, so a bare mint ([77]) arrives as
    // [77, 0, ...]. A zero gate-opcode must NOT trigger a forward — that would
    // call the gate with opcode 0 (Initialize) and revert. It falls back to the
    // accrue-to-owner default instead, so this is a SUCCESS with no payout.
    let b = mint_with_tail(V3 + 1, &[])?;
    assert_eq!(minter_diesel(&b)?, 0, "bare mint must not pay the minter");
    alkane_helpers::assert_return_context(
        &OutPoint {
            txid: b.txdata[1].compute_txid(),
            vout: 3,
        },
        |_| Ok(()),
    )?;
    Ok(())
}

#[wasm_bindgen_test]
fn test_broken_gate_reverts_mint() -> Result<()> {
    clear();
    setup_pre_upgrade()?;
    let auth = upgrade()?;
    // Point at a gate that does not exist (nothing deployed at 2:99).
    set_gate(V3, auth, 2, 99)?;

    // The mint reverts ATOMICALLY — it must NOT silently fall back to an
    // ordinary mint or to the accrual default (either would be the abusable
    // path). A swallowed mint and a reverted mint both leave the sheet empty,
    // so assert the REVERT itself: the trace's last event must be a
    // RevertContext (the empty needle matches any message; the event type is
    // what distinguishes revert from a silent success).
    let b = mint_with_tail(V3 + 1, &[OP_DONATE])?;
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
fn test_clear_returns_to_default() -> Result<()> {
    clear();
    setup_pre_upgrade()?;
    let auth = upgrade()?;
    deploy_gate(2)?;
    let auth = set_gate(V3, auth, GATE.block, GATE.tx)?;

    // Gate is live…
    let b = mint_with_tail(V3 + 1, &[OP_DONATE])?;
    assert_eq!(minter_diesel(&b)?, 0);

    // …turn it OFF. Back to the default: the mint accrues to the owner rather
    // than forwarding, so it still does not pay the minter — but it is a
    // success, not a revert, and the owner can collect it.
    let auth = set_gate(V3 + 2, auth, 0, 0)?;
    let b = mint_with_tail(V3 + 3, &[OP_DONATE])?;
    assert_eq!(minter_diesel(&b)?, 0, "cleared gate returns to accrual");
    alkane_helpers::assert_return_context(
        &OutPoint {
            txid: b.txdata[1].compute_txid(),
            vout: 3,
        },
        |_| Ok(()),
    )?;
    let (block, _) = collect_fees(V3 + 4, auth)?;
    let collected = get_sheet_for_outpoint(&block, 1, 0)?.get(&ProtoruneRuneId { block: 2, tx: 0 });
    assert!(collected > 0, "post-clear mints must be collectable");
    Ok(())
}

#[wasm_bindgen_test]
fn test_view_reports_gate() -> Result<()> {
    clear();
    setup_pre_upgrade()?;
    let auth = upgrade()?;
    deploy_gate(2)?;

    // Before any set: the view reports no gate.
    let height = V3;
    let (_, _, txid) = call_diesel(
        height,
        vec![102],
        OutPoint::new(create_coinbase_transaction(height).compute_txid(), 0),
    )?;
    alkane_helpers::assert_return_context(&OutPoint { txid, vout: 3 }, |trace_response| {
        let d = &trace_response.inner.data;
        assert_eq!(d.len(), 41, "view layout is 41 bytes");
        assert_eq!(u128::from_le_bytes(d[0..16].try_into()?), 0);
        assert_eq!(u128::from_le_bytes(d[16..32].try_into()?), 0);
        assert_eq!(d[40], 0, "no gate is live");
        Ok(())
    })?;

    // After the set: the target and the live flag are public immediately.
    set_gate(V3 + 1, auth, GATE.block, GATE.tx)?;
    let height = V3 + 2;
    let (_, _, txid) = call_diesel(
        height,
        vec![102],
        OutPoint::new(create_coinbase_transaction(height).compute_txid(), 0),
    )?;
    alkane_helpers::assert_return_context(&OutPoint { txid, vout: 3 }, move |trace_response| {
        let d = &trace_response.inner.data;
        assert_eq!(d.len(), 41);
        assert_eq!(u128::from_le_bytes(d[0..16].try_into()?), GATE.block);
        assert_eq!(u128::from_le_bytes(d[16..32].try_into()?), GATE.tx);
        assert_eq!(u64::from_le_bytes(d[32..40].try_into()?), height as u64);
        assert_eq!(d[40], 1, "gate is live");
        Ok(())
    })?;
    Ok(())
}
