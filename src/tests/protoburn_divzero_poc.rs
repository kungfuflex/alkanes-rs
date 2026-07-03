//! PoC for the modulo-by-zero panic in `BurnCycle::next`
//! (crates/protorune/src/protoburn.rs:204).
//!
//! `Protoburns::process` builds a `BurnCycle` whose `max` is the number of
//! protoburns (`construct_burncycle` -> `BurnCycle::new(self.len())`). When the
//! runestone's output index equals the default output, it cycles every leftover
//! rune through the burn set via `burn_cycles.next(rune)`, which computes
//! `(cycle + 1) % (self.max as i32)` (protoburn.rs:204). With ZERO protoburns
//! `max == 0`, so any leftover rune triggers `% 0` — an unconditional panic.
//!
//! A tx can carry leftover runes at the runestone OP_RETURN output with no
//! protoburn protostone at all, so this is an attacker-reachable panic. Today
//! `process_burns` is `#[cfg(test)]`-gated in `index_protostones`, so it is
//! latent in production but LIVE in the test harness and the moment protoburn
//! is enabled — and since a panic aborts `index_block` and metashrew retries
//! forever, it is a block-wedge primitive.
//!
//! Fix: `process` no-ops when there are no protoburns (nothing to burn), and
//! `BurnCycle::next` returns a graceful `Err` instead of dividing by zero.
//!
//! Pre-fix `test_protoburn_empty_burns_no_panic` panics with "attempt to
//! calculate the remainder with a divisor of zero"; post-fix it returns Ok.

use crate::tests::helpers::clear;
use anyhow::Result;
use bitcoin::hashes::Hash;
use bitcoin::Txid;
use metashrew_core::index_pointer::AtomicPointer;
use protorune::protoburn::{BurnCycle, Protoburn, Protoburns};
use protorune_support::balance_sheet::{BalanceSheet, BalanceSheetOperations, ProtoruneRuneId};
use std::collections::BTreeMap;
use wasm_bindgen_test::wasm_bindgen_test;

#[wasm_bindgen_test]
fn test_protoburn_empty_burns_no_panic() -> Result<()> {
    clear();

    // No protoburns => BurnCycle max == 0.
    let mut burns: Vec<Protoburn> = vec![];

    // A leftover rune sitting at the runestone output index (== default output),
    // which drives the `runestone_output_index == default_output` cycle branch.
    let output_index: u32 = 0;
    let sheet: BalanceSheet<AtomicPointer> =
        BalanceSheet::from_pairs(vec![ProtoruneRuneId { block: 1, tx: 1 }], vec![100u128]);
    let mut balances_by_output: BTreeMap<u32, BalanceSheet<AtomicPointer>> = BTreeMap::new();
    balances_by_output.insert(output_index, sheet);

    let mut atomic = AtomicPointer::default();
    let mut proto_balances_by_output: BTreeMap<u32, BalanceSheet<AtomicPointer>> = BTreeMap::new();
    let txid = Txid::from_byte_array([7u8; 32]);

    // Pre-fix: panics at protoburn.rs:204 with "% 0". Post-fix: graceful no-op.
    let result = burns.process(
        &mut atomic,
        vec![],       // runestone_edicts
        output_index, // runestone_output_index
        &balances_by_output,
        &mut proto_balances_by_output,
        output_index, // default_output == runestone_output_index
        txid,
    );

    // INVARIANT: an empty burn set must never divide by zero — it has nothing to
    // burn, so processing is a clean no-op.
    assert!(
        result.is_ok(),
        "process with zero protoburns must be a graceful no-op, got {:?}",
        result.err()
    );
    Ok(())
}

#[wasm_bindgen_test]
fn test_burncycle_zero_max_is_graceful() -> Result<()> {
    // Defense-in-depth: the primitive itself must not panic on max == 0.
    let mut cycle = BurnCycle::new(0);
    let r = cycle.next(&ProtoruneRuneId { block: 2, tx: 3 });
    assert!(
        r.is_err(),
        "BurnCycle::next(max=0) must return Err, not divide by zero"
    );
    Ok(())
}
