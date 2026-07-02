//! Confirm-or-kill PoC for the "reconcile burns the refund" protorune bug.
//!
//! This is the fund-loss SIBLING of the "two stores, one rollback" inflation bug
//! (see `inflation_poc.rs`). The `pipe()` all-or-nothing fix closed the *inflation*
//! path (no more partial pointer credit surviving a rollback), but it did NOT fix
//! the *refund* path, because the ordering inside `reconcile()` is still:
//!
//! ```ignore
//! balances_by_output.remove(&vout);                                  // (1) drop incoming
//! increase_balances_using_sheet(balances_by_output, &outgoing, ptr)?;// (2) FALLIBLE (overflow)
//! ```
//!
//! When (2) overflows (the `pointer` output already holds ~u128::MAX of a rune the
//! message also forwards), `reconcile` returns `Err` — but the incoming balance was
//! already `remove`d at (1). `process_message` then runs its reconcile-Err branch:
//!
//! ```ignore
//! refund_to_refund_pointer(balances_by_output, protomessage_vout, refund_pointer)?;
//! atomic.rollback();
//! ```
//!
//! `refund_to_refund_pointer` reads the (now-missing) `protomessage_vout`, finds
//! nothing, and refunds ZERO. `atomic.rollback()` only unwinds the KV side. Net
//! result: a reverted message SILENTLY BURNS the caller's incoming balance instead
//! of refunding it to `refund_pointer` — violating the documented refund invariant.
//!
//! This test drives the *real* production functions (`OutgoingRunes::reconcile` and
//! `refund_to_refund_pointer`) in the exact order `process_message` calls them, and
//! asserts the secure invariant: **a failed message refunds the full incoming
//! balance**. It FAILS on the buggy ordering (refund == 0) and PASSES once
//! `reconcile` is made all-or-nothing.

use metashrew_core::index_pointer::AtomicPointer;
use protorune::balance_sheet::OutgoingRunes;
use protorune_support::balance_sheet::{BalanceSheet, BalanceSheetOperations, ProtoruneRuneId};
use protorune_support::rune_transfer::{refund_to_refund_pointer, RuneTransfer};
use std::collections::BTreeMap;
use wasm_bindgen_test::wasm_bindgen_test;

#[wasm_bindgen_test]
fn test_reconcile_refund_is_not_burned_on_pointer_overflow() {
    // A rune the protomessage both HOLDS (incoming) and FORWARDS (outgoing).
    let rune_a = ProtoruneRuneId::new(2, 0);

    // Virtual/real vout layout mirrored from `index_protostones`:
    //   shadow_vout : this protostone's incoming balance sheet
    //   pointer     : where the message forwards its outgoing balance
    //   refund      : where a FAILED message must refund the incoming balance
    let shadow_vout: u32 = 3;
    let pointer: u32 = 0;
    let refund_pointer: u32 = 1;

    // The caller's incoming balance to the protomessage.
    let incoming: u128 = 1_000;

    let mut balances_by_output: BTreeMap<u32, BalanceSheet<AtomicPointer>> = BTreeMap::new();

    // Incoming balance sitting on the protomessage's shadow vout.
    let mut incoming_sheet = BalanceSheet::<AtomicPointer>::default();
    incoming_sheet.set(&rune_a, incoming);
    balances_by_output.insert(shadow_vout, incoming_sheet);

    // The pointer output is ALREADY holding a near-MAX balance of the SAME rune —
    // e.g. primed by an earlier protostone's self-mint in the same tx. Forwarding
    // even 1 more unit here overflows u128.
    let mut pointer_sheet = BalanceSheet::<AtomicPointer>::default();
    pointer_sheet.set(&rune_a, u128::MAX);
    balances_by_output.insert(pointer, pointer_sheet);

    // The message succeeds and forwards its full incoming balance to `pointer`.
    let outgoing: Vec<RuneTransfer> = vec![RuneTransfer {
        id: rune_a,
        value: incoming,
    }];
    let outgoing_runtime = BalanceSheet::<AtomicPointer>::default();
    let values = (outgoing, outgoing_runtime);

    let mut atomic = AtomicPointer::default();

    // --- exactly what process_message does on the T::handle(Ok) path ---
    let reconcile_result =
        values.reconcile(&mut atomic, &mut balances_by_output, shadow_vout, pointer);
    assert!(
        reconcile_result.is_err(),
        "reconcile must fail when forwarding overflows the pointer output"
    );

    // reconcile-Err branch: refund to refund_pointer, then roll back the KV side.
    refund_to_refund_pointer(&mut balances_by_output, shadow_vout, refund_pointer)
        .expect("refund itself must not error");
    atomic.rollback();

    // INVARIANT: a reverted message refunds the caller's full incoming balance to
    // refund_pointer. Nothing may be silently destroyed.
    let refunded = balances_by_output
        .get(&refund_pointer)
        .map(|s| s.get(&rune_a))
        .unwrap_or(0);
    assert_eq!(
        refunded, incoming,
        "reverted message BURNED {} of incoming balance instead of refunding it \
         (refund_pointer received {})",
        incoming, refunded
    );
}
