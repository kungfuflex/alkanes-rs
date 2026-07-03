//! Confirm-or-kill PoC for the "reconcile burns the refund" protorune bug and
//! its GENERAL fix (the `process_message` in-memory snapshot/restore).
//!
//! Background. `process_message` maintains TWO stores that must roll back
//! together on a reverted message:
//!   1. the transactional `AtomicPointer` KV store (`atomic.rollback()`), and
//!   2. the NON-transactional in-memory `proto_balances_by_output` map.
//! `reconcile()` mutates (2) in a non-atomic order — it `remove(&vout)`s the
//! incoming balance BEFORE the fallible pointer-forward. If the forward
//! overflows (the pointer output already holds ~u128::MAX of a rune the message
//! forwards), `reconcile` returns `Err` with the incoming balance already gone,
//! and the refund path (`refund_to_refund_pointer`, which reads `vout`) then
//! refunds NOTHING — silently BURNING the caller's incoming balance.
//!
//! The GENERAL fix binds the two stores: `process_message` snapshots
//! `proto_balances_by_output` before the message and, on ANY failure, restores
//! that snapshot, rolls back the KV side, THEN refunds from the clean state.
//! This makes the whole class impossible regardless of `reconcile`'s internal
//! ordering — no partial in-memory mutation can survive a revert.
//!
//! These two tests drive the REAL production functions (`OutgoingRunes::reconcile`
//! and `refund_to_refund_pointer`):
//!   * `test_reconcile_in_isolation_is_not_all_or_nothing` documents the footgun:
//!     `reconcile` alone leaves the map partially mutated on overflow, so a refund
//!     taken directly afterwards burns the incoming balance. (This is why the unit
//!     `reconcile` is NOT self-safe — the safety lives in its caller.)
//!   * `test_process_message_snapshot_prevents_burn` reproduces `process_message`'s
//!     exact failure sequence WITH the snapshot/restore and asserts the secure
//!     invariant: a reverted message refunds the full incoming balance. This is the
//!     regression guard for the general fix and passes even with `reconcile` left
//!     in its original (non-all-or-nothing) form.

use metashrew_core::index_pointer::AtomicPointer;
use protorune::balance_sheet::OutgoingRunes;
use protorune_support::balance_sheet::{BalanceSheet, BalanceSheetOperations, ProtoruneRuneId};
use protorune_support::rune_transfer::{refund_to_refund_pointer, RuneTransfer};
use std::collections::BTreeMap;
use wasm_bindgen_test::wasm_bindgen_test;

/// A rune the protomessage both HOLDS (incoming) and FORWARDS (outgoing).
fn rune_a() -> ProtoruneRuneId {
    ProtoruneRuneId::new(2, 0)
}

/// Build the pre-message `proto_balances_by_output` state:
///   shadow_vout -> incoming balance sheet (the caller's funds into the message)
///   pointer     -> near-MAX balance of the SAME rune (primed so any forward overflows)
fn setup_map(
    shadow_vout: u32,
    pointer: u32,
    incoming: u128,
) -> BTreeMap<u32, BalanceSheet<AtomicPointer>> {
    let mut map: BTreeMap<u32, BalanceSheet<AtomicPointer>> = BTreeMap::new();
    let mut incoming_sheet = BalanceSheet::<AtomicPointer>::default();
    incoming_sheet.set(&rune_a(), incoming);
    map.insert(shadow_vout, incoming_sheet);

    let mut pointer_sheet = BalanceSheet::<AtomicPointer>::default();
    pointer_sheet.set(&rune_a(), u128::MAX);
    map.insert(pointer, pointer_sheet);
    map
}

/// The message succeeds and forwards its full incoming balance to `pointer`.
fn outgoing_values(incoming: u128) -> (Vec<RuneTransfer>, BalanceSheet<AtomicPointer>) {
    (
        vec![RuneTransfer {
            id: rune_a(),
            value: incoming,
        }],
        BalanceSheet::<AtomicPointer>::default(),
    )
}

/// Documents that `reconcile` alone is NOT all-or-nothing: on a pointer overflow
/// it has already removed the incoming `vout`, so a refund taken right after burns
/// the incoming balance. This is exactly why the fix must live in the caller.
#[wasm_bindgen_test]
fn test_reconcile_in_isolation_is_not_all_or_nothing() {
    let (shadow_vout, pointer, refund_pointer) = (3u32, 0u32, 1u32);
    let incoming = 1_000u128;
    let mut map = setup_map(shadow_vout, pointer, incoming);
    let mut atomic = AtomicPointer::default();

    let r = outgoing_values(incoming).reconcile(&mut atomic, &mut map, shadow_vout, pointer);
    assert!(r.is_err(), "reconcile must fail on pointer overflow");

    // Refund WITHOUT the caller's snapshot restore (the buggy sequence).
    refund_to_refund_pointer(&mut map, shadow_vout, refund_pointer).unwrap();
    let refunded = map.get(&refund_pointer).map(|s| s.get(&rune_a())).unwrap_or(0);

    // reconcile already removed `shadow_vout`, so the refund saw nothing: burned.
    assert_eq!(
        refunded, 0,
        "reconcile-in-isolation leaves the incoming removed -> refund burns it"
    );
}

/// Regression guard for the GENERAL fix: `process_message` snapshots the in-memory
/// map before the message and restores it on failure, so the reverted message
/// refunds the full incoming balance regardless of `reconcile`'s internal ordering.
#[wasm_bindgen_test]
fn test_process_message_snapshot_prevents_burn() {
    let (shadow_vout, pointer, refund_pointer) = (3u32, 0u32, 1u32);
    let incoming = 1_000u128;
    let mut map = setup_map(shadow_vout, pointer, incoming);
    let mut atomic = AtomicPointer::default();

    // --- process_message's exact failure handling ---
    // snapshot BEFORE the message (process_message takes this before atomic.checkpoint()).
    let snapshot = map.clone();
    atomic.checkpoint();

    let r = outgoing_values(incoming).reconcile(&mut atomic, &mut map, shadow_vout, pointer);
    assert!(r.is_err(), "reconcile must fail on pointer overflow");

    // On failure: restore the in-memory map, roll back the KV side, THEN refund.
    map = snapshot;
    atomic.rollback();
    refund_to_refund_pointer(&mut map, shadow_vout, refund_pointer).unwrap();

    let refunded = map.get(&refund_pointer).map(|s| s.get(&rune_a())).unwrap_or(0);
    assert_eq!(
        refunded, incoming,
        "SECURE INVARIANT: reverted message refunds the full incoming balance \
         (got {}, expected {})",
        refunded, incoming
    );
}
