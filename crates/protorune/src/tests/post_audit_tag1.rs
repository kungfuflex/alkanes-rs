//! Post-audit (Halborn) protocol-tag settlement fixes, gated on
//! `POST_AUDIT_FORK_HEIGHT`. Every case is indexed once just BELOW the gate
//! (legacy behaviour must be preserved byte-for-byte) and once AT the gate.
//! The gate is moved to `GATE` with the test-only override so both sides are
//! exercised on non-mainnet builds, where the constant is 0.
//!
//! Findings covered:
//!  * Pre-Message Validation Errors Roll Back Whole Protocol Transactions and
//!    Strand Spent-Input Assets (missing / out-of-range pointer, 100-cap)
//!  * Input Balance Aggregation Overflow Can Strand Co-Spent Alkane Assets
//!  * Protocol Assets Are Stranded or Erased When Spent Without a Matching
//!    Protostone (no runestone, cenotaph, no protocol field, foreign tag only)
//!  * Zero Runtime Balance Updates Leave Previous Stored Balances Active

use crate::balance_sheet::{load_sheet, PersistentRecord};
use crate::message::{MessageContext, MessageContextParcel};
use crate::protostone::Protostones;
use crate::test_helpers::{self as helpers, clear};
use crate::{set_post_audit_fork_height_override, tables, Protorune};
use anyhow::Result;
use bitcoin::{
    script::Builder, transaction::Version, Amount, Block, OutPoint, ScriptBuf, Sequence,
    Transaction, TxIn, TxOut, Witness,
};
use metashrew_core::index_pointer::{AtomicPointer, IndexPointer};
use metashrew_support::index_pointer::KeyValuePointer;
use ordinals::{Edict, RuneId, Runestone};
use protorune_support::balance_sheet::{BalanceSheet, BalanceSheetOperations, ProtoruneRuneId};
use protorune_support::protostone::Protostone;
use protorune_support::rune_transfer::RuneTransfer;
use protorune_support::utils::consensus_encode;
use std::str::FromStr;
#[allow(unused_imports)]
use wasm_bindgen_test::*;

const TAG: u128 = 122;
const FOREIGN_TAG: u128 = 123;
const GATE: u64 = 1000;
const BELOW: u64 = GATE - 1;

struct GateGuard;
impl GateGuard {
    fn new() -> Self {
        set_post_audit_fork_height_override(Some(GATE));
        GateGuard
    }
}
impl Drop for GateGuard {
    fn drop(&mut self) {
        set_post_audit_fork_height_override(None);
    }
}

/// calldata[0] == 1: deposit all incoming into the runtime balance.
/// calldata[0] == 2: withdraw all of every incoming-or-runtime asset in
///                   `ASSET_A` to the pointer, leaving an EXPLICIT zero.
/// otherwise: forward incoming to the pointer.
struct Ctx(());
impl MessageContext for Ctx {
    fn protocol_tag() -> u128 {
        TAG
    }
    fn handle(
        parcel: &MessageContextParcel,
    ) -> Result<(Vec<RuneTransfer>, BalanceSheet<AtomicPointer>)> {
        match parcel.calldata.first().copied() {
            Some(1) => {
                let sheet: BalanceSheet<AtomicPointer> = parcel.runes.clone().try_into()?;
                let mut runtime = parcel.runtime_balances.as_ref().clone();
                for (id, v) in sheet.balances() {
                    let cur = runtime.get(id);
                    runtime.set(id, cur + v);
                }
                Ok((vec![], runtime))
            }
            Some(2) => {
                let mut runtime = parcel.runtime_balances.as_ref().clone();
                let held = runtime.get(&ASSET_A);
                runtime.set(&ASSET_A, 0);
                Ok((
                    vec![RuneTransfer {
                        id: ASSET_A,
                        value: held,
                    }],
                    runtime,
                ))
            }
            _ => Ok((parcel.runes.clone(), BalanceSheet::default())),
        }
    }
}

const ASSET_A: ProtoruneRuneId = ProtoruneRuneId { block: 2, tx: 1 };
const ASSET_B: ProtoruneRuneId = ProtoruneRuneId { block: 2, tx: 2 };

fn mock_outpoint(n: u32) -> OutPoint {
    OutPoint {
        txid: bitcoin::Txid::from_str(&format!("{:064x}", 0xabc000 + n as u64)).unwrap(),
        vout: 0,
    }
}

fn table_ptr(outpoint: &OutPoint) -> IndexPointer {
    tables::RuneTable::for_protocol(TAG)
        .OUTPOINT_TO_RUNES
        .select(&consensus_encode(outpoint).unwrap())
}

fn seed(outpoint: &OutPoint, pairs: &[(ProtoruneRuneId, u128)]) {
    let mut sheet = BalanceSheet::<IndexPointer>::default();
    for (id, v) in pairs {
        sheet.set(id, *v);
    }
    sheet.save(&table_ptr(outpoint), false);
}

fn sheet_at(outpoint: &OutPoint) -> BalanceSheet<IndexPointer> {
    load_sheet(&table_ptr(outpoint))
}

fn spendable_out() -> TxOut {
    TxOut {
        value: Amount::from_sat(10_000),
        script_pubkey: helpers::get_address(&helpers::ADDRESS1()).script_pubkey(),
    }
}

fn plain_op_return() -> TxOut {
    TxOut {
        value: Amount::from_sat(0),
        script_pubkey: Builder::new()
            .push_opcode(bitcoin::opcodes::all::OP_RETURN)
            .push_slice(b"x")
            .into_script(),
    }
}

fn runestone_out(runestone: Runestone) -> TxOut {
    TxOut {
        value: Amount::from_sat(0),
        script_pubkey: runestone.encipher(),
    }
}

fn protocol_runestone(stones: Vec<Protostone>) -> Runestone {
    Runestone {
        etching: None,
        pointer: None,
        edicts: vec![],
        mint: None,
        protocol: Some(stones.encipher().unwrap()),
    }
}

fn tx(inputs: &[OutPoint], outputs: Vec<TxOut>) -> Transaction {
    Transaction {
        version: Version::ONE,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: inputs
            .iter()
            .map(|o| TxIn {
                previous_output: *o,
                script_sig: ScriptBuf::new(),
                sequence: Sequence::MAX,
                witness: Witness::new(),
            })
            .collect(),
        output: outputs,
    }
}

fn index(t: &Transaction, height: u64) -> Block {
    let block = helpers::create_block_with_txs(vec![
        helpers::create_coinbase_transaction(height as u32),
        t.clone(),
    ]);
    Protorune::index_block::<Ctx>(block.clone(), height).unwrap();
    block
}

fn out(t: &Transaction, vout: u32) -> OutPoint {
    OutPoint {
        txid: t.compute_txid(),
        vout,
    }
}

fn stone(pointer: Option<u32>, refund: Option<u32>, message: Vec<u8>) -> Protostone {
    Protostone {
        burn: None,
        message,
        edicts: vec![],
        refund,
        pointer,
        from: None,
        protocol_tag: TAG,
    }
}

/// Runs `build` at BELOW and at GATE on fresh state; each run seeds `input`
/// with 1000 of ASSET_A. Returns (input, output-n) sheets for each run.
fn both_sides(
    build: impl Fn(&OutPoint) -> Transaction,
    n: u32,
) -> [(BalanceSheet<IndexPointer>, BalanceSheet<IndexPointer>); 2] {
    let _g = GateGuard::new();
    [BELOW, GATE].map(|h| {
        clear();
        let input = mock_outpoint(1);
        seed(&input, &[(ASSET_A, 1000)]);
        let t = build(&input);
        index(&t, h);
        (sheet_at(&input), sheet_at(&out(&t, n)))
    })
}

// ---------------------------------------------------------------------------
// Pre-message validation errors
// ---------------------------------------------------------------------------

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn malformed_header_strands_below_gate_and_refunds_at_gate() {
    let cases: Vec<(&str, Protostone)> = vec![
        ("missing pointer", stone(None, Some(0), vec![9])),
        ("missing refund", stone(Some(0), None, vec![9])),
        ("pointer out of range", stone(Some(50), Some(0), vec![9])),
        ("refund out of range", stone(Some(0), Some(50), vec![9])),
    ];
    for (name, s) in cases {
        let [below, at] = both_sides(
            |i| tx(&[*i], vec![spendable_out(), runestone_out(protocol_runestone(vec![s.clone()]))]),
            0,
        );
        // below: legacy Err voids the tx, balance stays on the spent input
        assert_eq!(below.0.get_cached(&ASSET_A), 1000, "{name}: below input");
        assert_eq!(below.1.get_cached(&ASSET_A), 0, "{name}: below output");
        // at gate: failed message, refunded to a real output, input cleared
        assert_eq!(at.0.get_cached(&ASSET_A), 0, "{name}: at input");
        assert_eq!(at.1.get_cached(&ASSET_A), 1000, "{name}: at output");
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn hundredth_protostone_cap_removed_at_gate() {
    // 2 outputs -> shadow vouts start at 3; legacy cap = 2 + 100 = 102, i.e. the
    // 100th protostone is rejected.
    let build = |i: &OutPoint| {
        let stones = (0..100).map(|_| stone(Some(0), Some(0), vec![9])).collect();
        tx(&[*i], vec![spendable_out(), runestone_out(protocol_runestone(stones))])
    };
    let [below, at] = both_sides(build, 0);
    assert_eq!(below.0.get_cached(&ASSET_A), 1000);
    assert_eq!(below.1.get_cached(&ASSET_A), 0);
    assert_eq!(at.0.get_cached(&ASSET_A), 0);
    assert_eq!(at.1.get_cached(&ASSET_A), 1000);

    // 99 protostones settle on both sides (control)
    let build99 = |i: &OutPoint| {
        let stones = (0..99).map(|_| stone(Some(0), Some(0), vec![9])).collect();
        tx(&[*i], vec![spendable_out(), runestone_out(protocol_runestone(stones))])
    };
    for (input, output) in both_sides(build99, 0) {
        assert_eq!(input.get_cached(&ASSET_A), 0);
        assert_eq!(output.get_cached(&ASSET_A), 1000);
    }
}

// ---------------------------------------------------------------------------
// Input aggregation overflow
// ---------------------------------------------------------------------------

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn input_aggregation_overflow_settles_saturating_at_gate() {
    let _g = GateGuard::new();
    for h in [BELOW, GATE] {
        clear();
        let (p1, p2, victim) = (mock_outpoint(1), mock_outpoint(2), mock_outpoint(3));
        seed(&p1, &[(ASSET_A, u128::MAX)]);
        seed(&p2, &[(ASSET_A, 1)]);
        seed(&victim, &[(ASSET_B, 7)]);
        let t = tx(
            &[p1, p2, victim],
            vec![
                spendable_out(),
                runestone_out(protocol_runestone(vec![stone(Some(0), Some(0), vec![9])])),
            ],
        );
        index(&t, h);
        let o = sheet_at(&out(&t, 0));
        if h < GATE {
            // legacy: whole tx rolled back, everything stranded on spent inputs
            assert_eq!(sheet_at(&p1).get_cached(&ASSET_A), u128::MAX);
            assert_eq!(sheet_at(&p2).get_cached(&ASSET_A), 1);
            assert_eq!(sheet_at(&victim).get_cached(&ASSET_B), 7);
            assert_eq!(o.get_cached(&ASSET_A), 0);
            assert_eq!(o.get_cached(&ASSET_B), 0);
        } else {
            // fixed: settled to default output; poisoned asset saturates, the
            // co-spent asset is conserved; every input cleared.
            assert_eq!(o.get_cached(&ASSET_A), u128::MAX);
            assert_eq!(o.get_cached(&ASSET_B), 7);
            for p in [p1, p2] {
                assert_eq!(sheet_at(&p).get_cached(&ASSET_A), 0);
            }
            assert_eq!(sheet_at(&victim).get_cached(&ASSET_B), 0);
        }
    }

    // exact-boundary control: u128::MAX - 1 + 1 settles normally on both sides
    for h in [BELOW, GATE] {
        clear();
        let (p1, p2) = (mock_outpoint(1), mock_outpoint(2));
        seed(&p1, &[(ASSET_A, u128::MAX - 1)]);
        seed(&p2, &[(ASSET_A, 1)]);
        let t = tx(
            &[p1, p2],
            vec![
                spendable_out(),
                runestone_out(protocol_runestone(vec![stone(Some(0), Some(0), vec![9])])),
            ],
        );
        index(&t, h);
        assert_eq!(sheet_at(&out(&t, 0)).get_cached(&ASSET_A), u128::MAX);
        assert_eq!(sheet_at(&p1).get_cached(&ASSET_A), 0);
    }
}

// ---------------------------------------------------------------------------
// Spent without a matching protostone
// ---------------------------------------------------------------------------

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn spend_without_matching_protostone_settles_to_first_non_op_return_at_gate() {
    // Output 0 is a plain OP_RETURN (not a runestone) so the target must be
    // output 1 -- the first non-OP_RETURN output.
    let no_runestone = |i: &OutPoint| tx(&[*i], vec![plain_op_return(), spendable_out()]);
    let cenotaph = |i: &OutPoint| {
        let rs = Runestone {
            edicts: vec![Edict {
                id: RuneId { block: 1, tx: 0 },
                amount: 1,
                output: 99, // > number of outputs -> cenotaph
            }],
            ..Default::default()
        };
        tx(&[*i], vec![runestone_out(rs), spendable_out()])
    };
    let no_protocol =
        |i: &OutPoint| tx(&[*i], vec![runestone_out(Runestone::default()), spendable_out()]);
    let foreign_only = |i: &OutPoint| {
        let mut s = stone(Some(1), Some(1), vec![9]);
        s.protocol_tag = FOREIGN_TAG;
        tx(&[*i], vec![runestone_out(protocol_runestone(vec![s])), spendable_out()])
    };

    // (name, builder, legacy-input-kept?) -- foreign-only legacy ERASES (input
    // cleared, nothing credited), the others legacy STRAND (input kept).
    let cases: Vec<(&str, Box<dyn Fn(&OutPoint) -> Transaction>, bool)> = vec![
        ("no runestone", Box::new(no_runestone), true),
        ("cenotaph", Box::new(cenotaph), true),
        ("no protocol field", Box::new(no_protocol), true),
        ("foreign tag only", Box::new(foreign_only), false),
    ];
    for (name, build, legacy_kept) in cases {
        let [below, at] = both_sides(|i| build(i), 1);
        assert_eq!(
            below.0.get_cached(&ASSET_A),
            if legacy_kept { 1000 } else { 0 },
            "{name}: below input"
        );
        assert_eq!(below.1.get_cached(&ASSET_A), 0, "{name}: below output");
        assert_eq!(at.0.get_cached(&ASSET_A), 0, "{name}: at input");
        assert_eq!(at.1.get_cached(&ASSET_A), 1000, "{name}: at output");
    }

    // all-OP_RETURN tx: nothing spendable, balance burns (input cleared)
    let all_op_return = |i: &OutPoint| tx(&[*i], vec![plain_op_return()]);
    let [_, at] = both_sides(all_op_return, 0);
    assert_eq!(at.0.get_cached(&ASSET_A), 0);
    assert_eq!(at.1.get_cached(&ASSET_A), 0);

    // control: a matching protostone is unaffected on both sides
    let matching = |i: &OutPoint| {
        tx(
            &[*i],
            vec![
                runestone_out(protocol_runestone(vec![stone(Some(1), Some(1), vec![9])])),
                spendable_out(),
            ],
        )
    };
    for (input, output) in both_sides(matching, 1) {
        assert_eq!(input.get_cached(&ASSET_A), 0);
        assert_eq!(output.get_cached(&ASSET_A), 1000);
    }
}

// ---------------------------------------------------------------------------
// Runtime zero persistence
// ---------------------------------------------------------------------------

fn runtime_ptr() -> IndexPointer {
    tables::RuneTable::for_protocol(TAG).RUNTIME_BALANCE
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn runtime_balance_debited_to_zero_is_persisted_at_gate() {
    let _g = GateGuard::new();
    for h in [BELOW, GATE] {
        clear();
        let input = mock_outpoint(1);
        seed(&input, &[(ASSET_A, 125), (ASSET_B, 5)]);
        // tx1: deposit everything into the runtime
        let t1 = tx(
            &[input],
            vec![
                spendable_out(),
                runestone_out(protocol_runestone(vec![stone(Some(0), Some(0), vec![1])])),
            ],
        );
        index(&t1, h);
        assert_eq!(BalanceSheet::new_ptr_backed(runtime_ptr()).get(&ASSET_A), 125);
        // tx2: withdraw ASSET_A, leaving an explicit cached zero
        let t2 = tx(
            &[out(&t1, 0)],
            vec![
                spendable_out(),
                runestone_out(protocol_runestone(vec![stone(Some(0), Some(0), vec![2])])),
            ],
        );
        index(&t2, h);
        assert_eq!(sheet_at(&out(&t2, 0)).get_cached(&ASSET_A), 125);

        let lazy = BalanceSheet::new_ptr_backed(runtime_ptr()).get(&ASSET_A);
        let listed = load_sheet(&runtime_ptr()).get_cached(&ASSET_A);
        // the unrelated asset is untouched on both sides
        assert_eq!(BalanceSheet::new_ptr_backed(runtime_ptr()).get(&ASSET_B), 5);
        assert_eq!(load_sheet(&runtime_ptr()).get_cached(&ASSET_B), 5);
        if h < GATE {
            // legacy: the stale 125 is resurrected by a fresh sheet
            assert_eq!(lazy, 125);
            assert_eq!(listed, 125);
        } else {
            assert_eq!(lazy, 0);
            assert_eq!(listed, 0);
        }
    }
}
