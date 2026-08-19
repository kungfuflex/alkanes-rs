//! The protostone-ordering invariant FIREVECTOR's qualifier rests on.
//!
//! The claim: **a DIESEL mint at protostone index `k` executes only if every
//! alkanes protostone before it in the same transaction succeeded.** That is what
//! lets a weightmap say "you earn weight only if a swap precedes your mint" and
//! trust the swap actually happened — same block, no committed traces, no
//! previous-block lag.
//!
//! The mechanism is fuel, not control flow. A reverting message unwinds through
//! `handle_message`'s `.or_else`, which calls `FuelTank::drain_fuel()`
//! (`message.rs:206` -> `fuel.rs:304`) and zeroes `transaction_fuel`. A later
//! protostone in the *same* transaction hits neither refuel branch — `is_top()`
//! is false and `should_advance(txindex)` is false because `current_txindex`
//! still equals this transaction (`message.rs:100-105`) — so it starts with 0
//! wasm fuel (`instance.rs:95`, under `config.consume_fuel(true)`) and traps on
//! its first instruction. Fuel is replenished only when the next *transaction*
//! advances the tank.
//!
//! These tests pin that invariant, and pin the places it does **not** hold. The
//! bypasses matter more than the happy path: a weightmap that trusts a preceding
//! protostone is only as sound as the weakest way to fake one.

use crate::index_block;
use crate::tests::helpers::{self as alkane_helpers, clear};
use crate::tests::std::alkanes_std_auth_token_build;
use crate::view;
use alkanes_support::cellpack::Cellpack;
use alkanes_support::constants::AUTH_TOKEN_FACTORY_ID;
use alkanes_support::id::AlkaneId;
use alkanes_support::trace::{Trace, TraceEvent};
use anyhow::Result;
use bitcoin::address::{Address, NetworkChecked};
use bitcoin::transaction::Version;
use bitcoin::{
    Amount, Block, OutPoint, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Witness,
};
use metashrew_core::index_pointer::IndexPointer;
use metashrew_support::index_pointer::KeyValuePointer;
use ordinals::Runestone;
use protorune::test_helpers::{create_block_with_coinbase_tx, get_address, ADDRESS1};
use protorune_support::protostone::{Protostone, Protostones};
use std::sync::{Mutex, MutexGuard};
use wasm_bindgen_test::wasm_bindgen_test;

use crate::firevector::{DIESEL_ID, DIESEL_MINT_OPCODE, FIREVECTOR_ID, WEIGHTMAP_KEY};
use alkanes_std_firevector::abi::{pack_weightmap, Weightmap};
use alkanes_std_firevector::vm::{
    Mode, OP_AND, OP_EQ, OP_GE, OP_MUL, OP_OPCODE, OP_PUSH, OP_SUB, OP_TARGET_BLOCK,
    OP_TARGET_TX, OP_TXINDEX,
};
use std::sync::Arc;

/// Serialises these tests against each other.
///
/// `clear()` wipes the global metashrew store and `index_block` mutates the
/// process-global `_FUEL_TANK`, so two of these running concurrently tear down
/// each other's DIESEL deployment — the symptom is a spurious
/// "already initialized" out of `setup_diesel`, on whichever test loses.
static SERIAL: Mutex<()> = Mutex::new(());

/// Take the serial lock, ignoring poisoning. A panicking test poisons the mutex,
/// and every subsequent test would then fail for a reason unrelated to what it
/// asserts, hiding the original failure behind a cascade.
fn serial() -> MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(|e| e.into_inner())
}

#[cfg(feature = "mainnet")]
const BASE: u32 = 925_000;
#[cfg(not(feature = "mainnet"))]
const BASE: u32 = 0;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

fn setup_diesel() -> Result<()> {
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
    index_block(&block, BASE)?;

    // Flip `/upgrade_initialized` directly rather than running the wasm upgrade
    // ceremony, which needs the premine spent into the upgrade tx. Same key the
    // wasm handler writes; see diesel_shadow::run_upgrade for the full rationale.
    let mut ptr = IndexPointer::default()
        .keyword("/alkanes/")
        .select(&DIESEL_ID.into())
        .keyword("/storage/")
        .select(&b"/upgrade_initialized".to_vec());
    ptr.set_value::<u8>(0x01);
    Ok(())
}

fn mint_cellpack() -> Cellpack {
    Cellpack {
        target: DIESEL_ID,
        inputs: vec![DIESEL_MINT_OPCODE],
    }
}

/// A call that always reverts: a target with no deployed binary.
///
/// `run_special_cellpacks` falls through every branch and hands back an empty
/// binary, wasmi then fails to instantiate, and the error unwinds through
/// `handle_message`'s `.or_else` — which is the path that calls `drain_fuel`.
/// Deliberately an ordinary failed call rather than a fuel-exhaustion loop,
/// which would make the test tautological.
fn reverting_cellpack() -> Cellpack {
    Cellpack {
        target: AlkaneId { block: 2, tx: 0xdead },
        inputs: vec![0],
    }
}

/// Build a transaction from explicit protostones so a test can malform one.
///
/// Two real outputs (vout 0 spendable, vout 1 the OP_RETURN), so protostone `i`
/// lives at shadow vout `i + 2 + 1 = 3 + i` — the inverse of
/// `shadow_vout = i + tx.output.len() + 1` (protorune/src/lib.rs:1177).
fn tx_with_protostones(protostones: Vec<Protostone>, prevout: OutPoint) -> Transaction {
    let runestone: ScriptBuf = (Runestone {
        etching: None,
        pointer: Some(0),
        edicts: Vec::new(),
        mint: None,
        protocol: protostones.encipher().ok(),
    })
    .encipher();
    let address: Address<NetworkChecked> = get_address(&ADDRESS1().as_str());
    Transaction {
        version: Version::ONE,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: vec![TxIn {
            previous_output: prevout,
            script_sig: ScriptBuf::new(),
            sequence: Sequence::MAX,
            witness: Witness::new(),
        }],
        output: vec![
            TxOut {
                value: Amount::from_sat(100_000_000),
                script_pubkey: address.script_pubkey(),
            },
            TxOut {
                value: Amount::from_sat(0),
                script_pubkey: runestone,
            },
        ],
    }
}

/// A well-formed message protostone.
fn message(cellpack: Cellpack) -> Protostone {
    Protostone {
        message: cellpack.encipher(),
        pointer: Some(0),
        refund: Some(0),
        edicts: vec![],
        from: None,
        burn: None,
        protocol_tag: 1u128,
    }
}

/// A protostone whose header is malformed: `pointer` is far past
/// `num_outputs + num_protostones`, which is the bound `process_message` checks
/// at `protostone.rs:102-107`.
fn malformed_header(cellpack: Cellpack) -> Protostone {
    Protostone {
        pointer: Some(100),
        ..message(cellpack)
    }
}

/// Shadow vout of protostone `i` for a `tx_with_protostones` transaction.
fn pstone_vout(i: u32) -> u32 {
    3 + i
}

fn outpoint(tx: &Transaction, pstone: u32) -> OutPoint {
    OutPoint {
        txid: tx.compute_txid(),
        vout: pstone_vout(pstone),
    }
}

/// This protostone's trace events, or empty if it left no trace at all.
///
/// The helpers in `tests::helpers` panic when the event they want is absent,
/// which makes them unusable for asserting a *negative*. A protostone that never
/// executed — the malformed-header case — writes no trace, and that has to read
/// as "no revert and no return", not as a test failure.
fn events(op: &OutPoint) -> Vec<TraceEvent> {
    let Ok(raw) = view::trace(op) else {
        return Vec::new();
    };
    let Ok(trace) = TryInto::<Trace>::try_into(raw) else {
        return Vec::new();
    };
    let guard = trace.0.lock().expect("trace mutex poisoned");
    guard.clone()
}

fn reverted(op: &OutPoint) -> bool {
    events(op)
        .iter()
        .any(|e| matches!(e, TraceEvent::RevertContext(_)))
}

/// Executed and returned normally. Stronger than `!reverted` — it also rules out
/// "never ran at all", which is what a protostone skipped before `T::handle`
/// looks like.
fn succeeded(op: &OutPoint) -> bool {
    let evs = events(op);
    evs.iter()
        .any(|e| matches!(e, TraceEvent::ReturnContext(_)))
        && !evs
            .iter()
            .any(|e| matches!(e, TraceEvent::RevertContext(_)))
}

// ---------------------------------------------------------------------------
// The invariant, where it holds
// ---------------------------------------------------------------------------

#[cfg_attr(not(target_arch = "wasm32"), test)]
#[wasm_bindgen_test]
fn reverting_protostone_kills_the_mint() -> Result<()> {
    // The case the whole qualifier design rests on, and which nothing in the
    // suite covered before: an ordinary contract revert ahead of a mint must
    // take the mint down with it, because the drained tank leaves the mint no
    // fuel to run on.
    let _g = serial();
    clear();
    setup_diesel()?;

    let height = BASE + 2;
    let mut block = create_block_with_coinbase_tx(height);
    let cb = block.txdata[0].compute_txid();
    let tx = tx_with_protostones(
        vec![message(reverting_cellpack()), message(mint_cellpack())],
        OutPoint::new(cb, 0),
    );
    block.txdata.push(tx.clone());
    index_block(&block, height)?;

    assert!(
        reverted(&outpoint(&tx, 0)),
        "fixture is wrong: protostone 0 was supposed to revert"
    );
    assert!(
        reverted(&outpoint(&tx, 1)),
        "a mint placed after a reverting protostone must not execute -- this is \
         the invariant a weightmap qualifier depends on"
    );
    Ok(())
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
#[wasm_bindgen_test]
fn a_mint_alone_in_its_transaction_still_succeeds() -> Result<()> {
    // The control for the test above. Without it, `reverting_protostone_kills
    // _the_mint` would also pass if mints never worked in this fixture at all.
    let _g = serial();
    clear();
    setup_diesel()?;

    let height = BASE + 2;
    let mut block = create_block_with_coinbase_tx(height);
    let cb = block.txdata[0].compute_txid();
    let tx = tx_with_protostones(vec![message(mint_cellpack())], OutPoint::new(cb, 0));
    block.txdata.push(tx.clone());
    index_block(&block, height)?;

    assert!(
        succeeded(&outpoint(&tx, 0)),
        "a lone DIESEL mint must succeed, or the ordering tests prove nothing"
    );
    Ok(())
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
#[wasm_bindgen_test]
fn the_drain_does_not_cross_transactions() -> Result<()> {
    // The invariant is strictly intra-transaction. `refuel_block` +
    // `fuel_transaction` (message.rs:100-105) hand the next transaction a fresh
    // budget, so one transaction's revert must not disenfranchise the rest of
    // the block. If this ever failed, a single reverting transaction near the
    // top of a block would zero out every mint below it.
    let _g = serial();
    clear();
    setup_diesel()?;

    let height = BASE + 2;
    let mut block = create_block_with_coinbase_tx(height);
    let cb = block.txdata[0].compute_txid();

    let poisoned = tx_with_protostones(
        vec![message(reverting_cellpack()), message(mint_cellpack())],
        OutPoint::new(cb, 0),
    );
    let clean = tx_with_protostones(vec![message(mint_cellpack())], OutPoint::new(cb, 1));
    block.txdata.push(poisoned.clone());
    block.txdata.push(clean.clone());
    index_block(&block, height)?;

    assert!(reverted(&outpoint(&poisoned, 1)), "same-tx mint must still die");
    assert!(
        succeeded(&outpoint(&clean, 0)),
        "a later transaction's mint must be unaffected by an earlier \
         transaction's revert"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// The invariant, where it does NOT hold
// ---------------------------------------------------------------------------

#[cfg_attr(not(target_arch = "wasm32"), test)]
#[wasm_bindgen_test]
fn malformed_header_protostone_does_not_drain_fuel() -> Result<()> {
    // H1, the critical bypass. A protostone whose header is malformed is
    // refunded and skipped with `return Ok(false)` at protostone.rs:107-125 --
    // *before* `T::handle` at :225. `handle_message` is never entered, so
    // `drain_fuel` never runs and the tank is untouched.
    //
    // That matters because `scan_transaction` decodes the cellpack without ever
    // inspecting `pointer`/`refund`, so this protostone is fully visible to a
    // weightmap as a qualifying action while costing nothing and executing
    // nothing. One out-of-range integer in the OP_RETURN forges the qualifier.
    //
    // This test asserts CURRENT behaviour so the fix has something to flip. Once
    // `scan_transaction` skips header-malformed protostones when matching the
    // qualifier, the mint still succeeds here -- it just stops earning weight.
    let _g = serial();
    clear();
    setup_diesel()?;

    let height = BASE + 2;
    let mut block = create_block_with_coinbase_tx(height);
    let cb = block.txdata[0].compute_txid();
    let tx = tx_with_protostones(
        vec![
            malformed_header(reverting_cellpack()),
            message(mint_cellpack()),
        ],
        OutPoint::new(cb, 0),
    );
    block.txdata.push(tx.clone());
    index_block(&block, height)?;

    assert!(
        events(&outpoint(&tx, 0)).is_empty(),
        "fixture is wrong: the malformed protostone was supposed to be skipped \
         before T::handle, which means it leaves no trace at all"
    );
    assert!(
        succeeded(&outpoint(&tx, 1)),
        "H1: a malformed-header protostone does not drain fuel, so the mint \
         after it runs -- the ordering guarantee cannot be relied on alone"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Supply: a block may never mint more than its reward
// ---------------------------------------------------------------------------

/// DIESEL's running total, read straight out of contract storage.
///
/// Read rather than modelled. The conservation tests in `tests::firevector`
/// divide `EMISSION / denominator` in the test itself, which checks the
/// denominators are sane but assumes the contract does what we think with them.
/// This reads what was actually minted.
fn total_supply() -> u128 {
    IndexPointer::default()
        .keyword("/alkanes/")
        .select(&DIESEL_ID.into())
        .keyword("/storage/")
        .select(&b"/totalsupply".to_vec())
        .get_value::<u128>()
}

/// `50e8 / 2^(height / 210000)` — the schedule `ChainConfiguration::block_reward`
/// implements, and the ceiling a block's total issuance must respect.
fn block_reward_at(height: u32) -> u128 {
    (50e8 as u128) / (1u128 << ((height as u128) / 210_000u128))
}

fn install_weightmap(wm: &Weightmap) {
    IndexPointer::default()
        .keyword("/alkanes/")
        .select(&<AlkaneId as Into<Vec<u8>>>::into(FIREVECTOR_ID))
        .keyword("/storage/")
        .select(&WEIGHTMAP_KEY.to_vec())
        .set(Arc::new(pack_weightmap(wm)));
}

/// A block of `n` independent DIESEL mint transactions.
fn mint_block(height: u32, n: usize) -> Block {
    let mut block = create_block_with_coinbase_tx(height);
    let cb = block.txdata[0].compute_txid();
    for i in 0..n {
        block.txdata.push(tx_with_protostones(
            vec![message(mint_cellpack())],
            OutPoint::new(cb, i as u32),
        ));
    }
    block
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
#[wasm_bindgen_test]
fn a_block_never_mints_more_than_the_block_reward() -> Result<()> {
    // The end-to-end form of the conservation claim: run real blocks through
    // `index_block` and assert what `/totalsupply` actually moved by.
    //
    // This is the assertion that would have caught the `floor(W/w_i)` bug, which
    // over-minted by 1.40x for weights [3,1,1]. Every mint calls
    // `increase_total_supply`, and `observe_upgraded_mint` adds `diesel_fee`
    // once, so a block's issuance is `sum(value_per_mint) + diesel_fee`. The
    // contract divides `block_reward - diesel_fee` by our denominator, so
    // holding `sum(1/d_i) <= 1` bounds the whole thing at `block_reward`.
    let _g = serial();
    clear();
    setup_diesel()?;

    let mut height = BASE + 2;
    for n in [1usize, 2, 3, 5] {
        let before = total_supply();
        let block = mint_block(height, n);
        index_block(&block, height)?;
        let minted = total_supply() - before;

        assert!(
            minted > 0,
            "n={n}: fixture minted nothing, so the bound proves nothing"
        );
        assert!(
            minted <= block_reward_at(height),
            "n={n}: block minted {minted} against a reward of {}",
            block_reward_at(height)
        );
        height += 1;
    }
    Ok(())
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
#[wasm_bindgen_test]
fn a_non_uniform_weightmap_never_mints_more_than_the_block_reward() -> Result<()> {
    // The teeth. A boolean weightmap would pass this bound even with the old
    // arithmetic, because an even split happened to conserve; only a genuinely
    // NON-uniform program breaks it.
    //
    // This program returns raw weights [3, 1, 1]. Under `floor(W/w_i)` that gave
    // W = 5 and denominators 5/3=1, 5/1=5, 5/1=5, so the first claimant took the
    // entire block and the other two took a fifth each -- 1.40x issuance, every
    // block, compounding into total supply. The SPLIT clamp flattens it to
    // [1, 1, 1] and the block pays exactly once over.
    let _g = serial();
    clear();
    setup_diesel()?;

    let height = BASE + 2;
    install_weightmap(&Weightmap {
        mode: Mode::Split,
        qualifier: None,
        rate_floor: 0,
        treasury_bps: 0,
        // 3 - 2 * (txindex >= 2)  ->  3 for the first mint, 1 for the rest
        program: vec![
            OP_TARGET_BLOCK, OP_PUSH, 2, OP_EQ,
            OP_TARGET_TX, OP_PUSH, 0, OP_EQ, OP_AND,
            OP_OPCODE, OP_PUSH, 77, OP_EQ, OP_AND,
            OP_PUSH, 3,
            OP_TXINDEX, OP_PUSH, 2, OP_GE,
            OP_PUSH, 2, OP_MUL,
            OP_SUB,
            OP_MUL,
        ],
    });

    let before = total_supply();
    let block = mint_block(height, 3);
    index_block(&block, height)?;
    let minted = total_supply() - before;
    let reward = block_reward_at(height);

    assert!(minted > 0, "nothing minted, so the bound proves nothing");
    assert!(
        minted <= reward,
        "non-uniform weightmap minted {minted} against a reward of {reward} \
         ({:.2}x) -- emission is no longer bounded by the halving schedule",
        minted as f64 / reward as f64
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Invariants that must not silently erode
// ---------------------------------------------------------------------------

#[cfg_attr(not(target_arch = "wasm32"), test)]
#[wasm_bindgen_test]
fn diesel_is_not_gasless() {
    // `GASLESS_ALKANES` (vm/fuel.rs:433) hands its members `u64::MAX / 2` fuel
    // regardless of a drained tank (message.rs:111-113). Adding DIESEL to that
    // list would let a mint execute after any revert and silently delete the
    // entire ordering guarantee -- with no test failing anywhere else.
    assert!(
        !crate::vm::fuel::is_gasless_alkane(&DIESEL_ID),
        "DIESEL must never be gasless: it would bypass drain_fuel and void the \
         protostone-ordering invariant FIREVECTOR's qualifier depends on"
    );
}

// ---------------------------------------------------------------------------
// Fuel parity
// ---------------------------------------------------------------------------
// The claim that makes this safe to activate on a live chain: FIREVECTOR does
// not change what a DIESEL mint costs in fuel.
//
// It rests on three things, and until now all three were argued rather than
// measured. The genesis alkane is not modified, so its metered instruction count
// cannot move. `handle_extcall` short-circuits `800000000:*` at :762, before
// `compute_extcall_fuel` at :815, so a precompile call pays no FUEL_EXTCALL and
// the callee reports `fuel_used = 0`. And the reply stays 16 bytes, so
// `returndatacopy` (FUEL_PER_LOAD_BYTE, 2/byte) charges what it charged before.
//
// The live risk is the third one plus a fourth nobody stated: that the CONTENT
// of a weightmap could reach the mint's fuel. A governance write is not a
// consensus upgrade, so if a large or expensive vector made mints cost more,
// governance could shift fuel accounting — and therefore which other protostones
// in a block succeed — without anyone treating it as a fork.

/// Index one block of `n` mints under `wm` and return the gas each DIESEL mint
/// actually burned, straight from `fuel_probe`.
///
/// Not from the trace: a message's `ReturnContext.fuel_used` is 0 for the
/// outermost call, so reading traces here compares zeroes and proves nothing.
/// `fuel_probe::record` is called from `run_after_special` with the real
/// `gas_used`, which is the number the FuelTank is actually charged.
fn mint_fuel_under(wm: &Weightmap, n: usize, height: u32) -> Result<Vec<u64>> {
    let mut block = create_block_with_coinbase_tx(height);
    let cb = block.txdata[0].compute_txid();
    let mut txs = Vec::new();
    for i in 0..n {
        let tx = tx_with_protostones(vec![message(mint_cellpack())], OutPoint::new(cb, i as u32));
        txs.push(tx.clone());
        block.txdata.push(tx);
    }
    if let Some(root) = block.compute_merkle_root() {
        block.header.merkle_root = root;
    }
    install_weightmap(wm);
    crate::fuel_probe::clear();
    index_block(&block, height)?;

    // Only the DIESEL mints. Setup and any other alkane traffic is noise here.
    let mints: Vec<u64> = crate::fuel_probe::snapshot()
        .into_iter()
        .filter(|r| r.target == AlkaneId { block: 2, tx: 0 } && r.opcode == 77)
        .map(|r| r.gas_used)
        .collect();

    // Every mint must have executed, or a comparison of empty vectors would pass
    // while proving nothing — which is exactly how the first version of this test
    // was vacuous.
    assert_eq!(
        mints.len(),
        txs.len(),
        "expected {} mints to execute, got {}",
        txs.len(),
        mints.len()
    );
    assert!(
        mints.iter().all(|g| *g > 0),
        "a mint reporting zero gas means the probe is not seeing real execution: {mints:?}"
    );
    Ok(mints)
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
fn weightmap_content_cannot_change_what_a_mint_costs() -> Result<()> {
    let _g = serial();

    // Same block, same mints, three very different vectors: the seeded identity,
    // one carrying a qualifier and a long convex program, and one diverting half
    // the reward to the treasury. If any of these moved the mint's fuel, a
    // governance write could change fuel accounting without a consensus upgrade.
    let identity = Weightmap {
        mode: alkanes_std_firevector::vm::Mode::Split,
        qualifier: None,
        rate_floor: 0,
        treasury_bps: 0,
        program: alkanes_std_firevector::programs::identity(),
    };
    let heavy = Weightmap {
        mode: alkanes_std_firevector::vm::Mode::Split,
        qualifier: Some(alkanes_std_firevector::vm::Qualifier {
            target_block: 4,
            target_tx: 1778,
            opcode: 1,
        }),
        rate_floor: 0,
        treasury_bps: 0,
        program: alkanes_std_firevector::programs::convex_on_prior(0),
    };
    let taxing = Weightmap {
        treasury_bps: 5000,
        ..identity.clone()
    };

    let mut seen: Vec<(&str, Vec<u64>)> = Vec::new();
    for (name, wm) in [("identity", &identity), ("heavy", &heavy), ("taxing", &taxing)] {
        clear();
        setup_diesel()?;
        seen.push((name, mint_fuel_under(wm, 3, BASE + 1)?));
    }

    let (_, baseline) = &seen[0];
    assert!(
        !baseline.is_empty(),
        "fixture produced no executed mints — the test proves nothing"
    );
    for (name, fuel) in &seen[1..] {
        assert_eq!(
            fuel, baseline,
            "weightmap '{name}' changed mint fuel: {fuel:?} vs {baseline:?}"
        );
    }
    Ok(())
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
fn the_precompiles_report_no_fuel_and_a_sixteen_byte_reply() -> Result<()> {
    let _g = serial();
    clear();
    setup_diesel()?;

    let identity = Weightmap {
        mode: alkanes_std_firevector::vm::Mode::Split,
        qualifier: None,
        rate_floor: 0,
        treasury_bps: 0,
        program: alkanes_std_firevector::programs::identity(),
    };
    install_weightmap(&identity);

    let height = BASE + 1;
    let mut block = create_block_with_coinbase_tx(height);
    let cb = block.txdata[0].compute_txid();
    let tx = tx_with_protostones(vec![message(mint_cellpack())], OutPoint::new(cb, 0));
    block.txdata.push(tx.clone());
    if let Some(root) = block.compute_merkle_root() {
        block.header.merkle_root = root;
    }
    index_block(&block, height)?;

    // Every nested call the mint made. The precompile answers are the ones whose
    // response is exactly 16 bytes and whose fuel_used is 0 — widening either
    // would silently reintroduce the divergence this design exists to avoid.
    let evs = events(&outpoint(&tx, 0));
    let free_16_byte_replies = evs
        .iter()
        .filter(|e| {
            matches!(e, TraceEvent::ReturnContext(r)
                if r.fuel_used == 0 && r.inner.data.len() == 16)
        })
        .count();
    assert!(
        free_16_byte_replies >= 2,
        "expected the mint-count and miner-fee precompiles to answer free and \
         16 bytes wide; found {free_16_byte_replies} such replies in {} events",
        evs.len()
    );
    Ok(())
}
