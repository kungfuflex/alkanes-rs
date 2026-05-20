//! Generic abstraction for precompiled alkanes.
//!
//! A "precompiled alkane" is a native Rust reimplementation of an
//! alkane's wasm logic that we dispatch in place of wasmi when the
//! call shape is safe. The DIESEL EOA implementation in
//! [`crate::precompile_diesel`] is the canonical first example; this
//! trait lets us add more (frBTC wrap/unwrap, OrbitalInstance batch
//! reads, etc.) without re-deriving the consensus-invariant safety
//! envelope each time.
//!
//! # Division of responsibility
//!
//! The PRECOMPILE handles the per-alkane semantics:
//!  * `TARGET_ID` — the alkane id it shadows
//!  * `handles_opcode` — which opcodes it claims
//!  * `execute()` — actual storage reads + writes + response assembly
//!    for one call
//!  * gas-table lookup per execution path
//!
//! The GENERIC DISPATCHER ([`can_dispatch`] + [`try_dispatch_precompile`])
//! handles the consensus-invariant safety envelope:
//!  * EOA-only gating (for opcodes that require caller == 0:0)
//!  * single-mint-per-tx gating (for opcodes subject to
//!    `enforce_one_mint_per_tx`)
//!  * single-cellpack-per-protostone gating (for opcodes where
//!    chained cellpacks could expose StorageMap-iteration-order
//!    nondeterminism — see the audit doc, divergence class #4)
//!  * storage-byte fuel accounting
//!  * piping the pending `StorageMap` into the call response
//!
//! Calls that don't satisfy the safety envelope fall through to the
//! wasmi path, which charges the correct gas + handles the edge cases
//! the precompile isn't equipped for. The fall-through is the load-
//! bearing safety guarantee — if the precompile says "I don't handle
//! this", wasmi takes over and consensus stays canonical.
//!
//! # Adding a new precompile
//!
//! 1. Define `Path: Copy + Eq` and `GasTable` types.
//! 2. Implement `PrecompiledAlkane` for the alkane.
//! 3. In `vm/utils.rs::run_after_special`, add a
//!    `try_dispatch_precompile::<MyPrecompile>(ctx, &MY_GAS)?` arm
//!    above the wasmi fallback. First match wins.
//! 4. Calibrate `GasTable` by running the shadow-test harness
//!    against the precompile's reference wasm binary at enumerated
//!    paths and asserting byte-equality with this impl's `execute`
//!    output.
//!
//! See `precompile-abstraction-plan.md` in the subkube fastpath
//! investigation directory for the full design + proof-cases list.

use crate::vm::runtime::AlkanesRuntimeContext;
use alkanes_support::id::AlkaneId;
use alkanes_support::response::ExtendedCallResponse;
use alkanes_support::storage::StorageMap;
use anyhow::{anyhow, Result};
use std::sync::{Arc, Mutex};

/// A native Rust reimplementation of an alkane. Implementors describe
/// their target id, the opcodes they claim, and the per-opcode
/// execution semantics. Universal consensus invariants are handled by
/// the generic dispatcher in this module.
pub trait PrecompiledAlkane {
    /// The alkane id this precompile shadows. Dispatch only fires when
    /// `ctx.myself == TARGET_ID`.
    const TARGET_ID: AlkaneId;

    /// Tag for path-classification + gas-table indexing. The DIESEL
    /// impl uses a `DieselPath` enum (CommunistFirstOfBlock,
    /// CommunistSubsequent, LegacyFirstOfBlock, …). Implementors are
    /// expected to enumerate ALL the wasm-side execution paths their
    /// `execute()` can take so the shadow-test harness can
    /// exhaustively calibrate the gas table.
    type Path: Copy + Eq + std::fmt::Debug + Send + Sync + 'static;

    /// The per-path gas cost table. Typically a struct with one
    /// `u64` field per `Path` variant (the DIESEL impl uses
    /// `DieselPathGas`). Calibrated once against the reference wasm
    /// binary; held in a `const` so dispatch doesn't pay per-call
    /// table-construction cost.
    type GasTable: Send + Sync;

    /// True iff this precompile claims the given opcode. Other
    /// opcodes fall through to wasm even if the alkane id matches.
    fn handles_opcode(opcode: u128) -> bool;

    /// True iff this opcode requires `caller == 0:0` (EOA). When
    /// true, cross-alkane callers fall through to wasm — which
    /// charges the correct fuel for the EOA-check trap.
    ///
    /// This is the abstraction fix for audit divergence class #3
    /// (cross-contract EOA-check gas). The precompile no longer
    /// needs to handle the non-EOA path itself; the dispatcher
    /// routes those calls to wasm where the gas accounting is
    /// canonical.
    fn requires_eoa(opcode: u128) -> bool {
        let _ = opcode;
        false
    }

    /// True iff this opcode is subject to `enforce_one_mint_per_tx`
    /// (the canonical anti-double-spend gate for mint operations).
    /// When true, the dispatcher checks
    /// [`count_target_mint_protostones_in_tx`] and falls through to
    /// wasm if the count isn't exactly 1.
    fn is_mint_opcode(opcode: u128) -> bool {
        let _ = opcode;
        false
    }

    /// True iff this opcode requires the OWNING PROTOSTONE to
    /// contain exactly one cellpack (the one calling this opcode).
    /// When true, the dispatcher falls through to wasm for
    /// multi-cellpack protostones — eliminating the StorageMap-
    /// iteration-order risk from audit divergence class #4.
    ///
    /// Default false. Phase 3 of the abstraction rollout will wire
    /// an enforcement helper and flip DIESEL's mint opcode to true.
    fn requires_solo_cellpack(opcode: u128) -> bool {
        let _ = opcode;
        false
    }

    /// Execute one precompile call. The caller provides a fresh
    /// `pending: StorageMap`; the impl populates it with the writes
    /// the wasmi `_CACHE` would have made. The dispatcher pipes
    /// `pending` into `response.storage` and adds storage-byte fuel
    /// before returning.
    ///
    /// The returned `Path` MUST match the path actually taken — the
    /// shadow-test harness validates this on every dispatched call.
    fn execute(
        ctx: &AlkanesRuntimeContext,
        pending: &mut StorageMap,
    ) -> Result<(ExtendedCallResponse, Self::Path)>;

    /// Look up the gas cost for a given path. Implementors typically
    /// `match` on the path enum and return the matching `GasTable`
    /// field.
    fn gas_for_path(path: Self::Path, gas: &Self::GasTable) -> u64;
}

/// Decides whether a precompile is willing + safe to dispatch the
/// current call. Returns false for any of:
///   * `ctx.myself != P::TARGET_ID`
///   * `P::handles_opcode(opcode) == false`
///   * `P::requires_eoa(opcode) && ctx.caller != 0:0`
///   * `P::is_mint_opcode(opcode)` and the tx has != 1 mint protostone
///     targeting this alkane
///   * `P::requires_solo_cellpack(opcode)` and the owning protostone
///     has != 1 cellpack (NOTE: Phase 3 — currently a no-op stub)
///
/// All of these classes are SAFE FALLTHROUGHS — when false, the
/// wasmi path takes over and produces the canonical result.
pub fn can_dispatch<P: PrecompiledAlkane>(ctx: &AlkanesRuntimeContext) -> bool {
    if ctx.myself != P::TARGET_ID {
        return false;
    }
    let opcode = ctx.inputs.first().copied().unwrap_or(0);
    if !P::handles_opcode(opcode) {
        return false;
    }
    if P::requires_eoa(opcode) && ctx.caller != AlkaneId::new(0, 0) {
        return false;
    }
    if P::is_mint_opcode(opcode) {
        match count_target_mint_protostones_in_tx(&ctx.message.transaction, &P::TARGET_ID, opcode) {
            Ok(n) if n == 1 => {}
            _ => return false,
        }
    }
    if P::requires_solo_cellpack(opcode) {
        // Phase 3 — wire the actual cellpack-counting helper here.
        // For now, default-false means we never gate on this, which
        // matches the pre-abstraction behavior. The plan doc explains
        // the iteration-order risk; the wasm path is still canonical
        // for chained-cellpack protostones, so this isn't actively
        // dangerous, just an open hole until phase 3 lands.
    }
    true
}

/// Run a precompile end-to-end if it claims the current call. The
/// returned `Option` is:
///
///   * `None` — the precompile declined to dispatch (call falls
///     through to wasm)
///   * `Some(Ok((response, total_gas)))` — precompile ran successfully
///   * `Some(Err(_))` — precompile claimed the call but execution
///     errored (revert path)
///
/// `total_gas = gas_for_path(path) + storage_byte_fuel(pending)`,
/// matching the semantics of `run_after_special`'s `fuel_used`.
pub fn try_dispatch_precompile<P: PrecompiledAlkane>(
    ctx_arc: Arc<Mutex<AlkanesRuntimeContext>>,
    gas: &P::GasTable,
) -> Option<Result<(ExtendedCallResponse, u64)>> {
    // Gate check + height capture in one lock acquire.
    let height = {
        let ctx = ctx_arc.lock().unwrap();
        if !can_dispatch::<P>(&ctx) {
            return None;
        }
        ctx.message.height as u32
    };

    let mut pending = StorageMap::default();
    let (mut response, path) = {
        let ctx = ctx_arc.lock().unwrap();
        match P::execute(&ctx, &mut pending) {
            Ok(v) => v,
            Err(e) => return Some(Err(e)),
        }
    };
    response.storage = pending;

    let storage_len = response.storage.serialize().len() as u64;
    let storage_fuel = match crate::vm::fuel::fuel_per_store_byte(height).checked_mul(storage_len) {
        Some(v) => v,
        None => return Some(Err(anyhow!("storage fuel overflow"))),
    };
    let total = match P::gas_for_path(path, gas).checked_add(storage_fuel) {
        Some(v) => v,
        None => return Some(Err(anyhow!("gas overflow"))),
    };
    Some(Ok((response, total)))
}

/// Count mint protostones in a tx that target the given alkane id +
/// opcode. Used by [`can_dispatch`] to enforce
/// `is_mint_opcode → exactly-one-mint-protostone-in-tx`.
///
/// Returns `Err` if the runestone has malformed varint payload —
/// matching the wasm path's `?` propagation in
/// `vm/host_functions.rs:_get_number_diesel_mints`. The caller treats
/// `Err` as "don't dispatch precompile, fall through to wasm" — the
/// wasm path will see the same error and revert correctly.
pub fn count_target_mint_protostones_in_tx(
    tx: &bitcoin::Transaction,
    target: &AlkaneId,
    mint_opcode: u128,
) -> Result<u32> {
    use ordinals::{Artifact, Runestone};
    use protorune_support::protostone::Protostone;
    use protorune_support::utils::decode_varint_list;
    use alkanes_support::cellpack::Cellpack;
    use std::io::Cursor;

    let mut count = 0u32;
    if let Some(Artifact::Runestone(ref runestone)) = Runestone::decipher(tx) {
        let protostones = Protostone::from_runestone(runestone)?;
        for p in protostones {
            if p.protocol_tag != 1 {
                continue;
            }
            let calldata: Vec<u8> = p.message.iter().flat_map(|v| v.to_be_bytes()).collect();
            if calldata.is_empty() {
                continue;
            }
            // Propagate the decode error rather than `continue` — this
            // is the bug class the precompile_diesel fix in commit
            // b6ca458d addressed. The wasm host_functions.rs uses `?`;
            // matching that behavior here is what keeps consensus
            // aligned on malformed-varint-adjacent-to-mint inputs.
            let list = decode_varint_list(&mut Cursor::new(calldata))?;
            if list.len() < 2 {
                continue;
            }
            if let Ok(cp) = TryInto::<Cellpack>::try_into(list) {
                if cp.target == *target
                    && cp.inputs.first().copied() == Some(mint_opcode)
                {
                    count += 1;
                }
            }
        }
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    //! Trait-shape compile tests. The actual end-to-end behavior tests
    //! live in `tests::diesel_*` once the DieselEoa impl lands in
    //! Phase 2 of the abstraction rollout.

    use super::*;
    use alkanes_support::id::AlkaneId;
    use wasm_bindgen_test::wasm_bindgen_test;

    /// Stub impl just to exercise the trait surface compiles.
    struct DummyPrecompile;

    #[derive(Copy, Clone, Debug, PartialEq, Eq)]
    enum DummyPath {
        Success,
    }

    struct DummyGas {
        success_gas: u64,
    }

    impl PrecompiledAlkane for DummyPrecompile {
        const TARGET_ID: AlkaneId = AlkaneId { block: 99, tx: 99 };
        type Path = DummyPath;
        type GasTable = DummyGas;

        fn handles_opcode(opcode: u128) -> bool {
            opcode == 1
        }

        fn execute(
            _ctx: &AlkanesRuntimeContext,
            _pending: &mut StorageMap,
        ) -> Result<(ExtendedCallResponse, Self::Path)> {
            Ok((ExtendedCallResponse::default(), DummyPath::Success))
        }

        fn gas_for_path(path: Self::Path, gas: &Self::GasTable) -> u64 {
            match path {
                DummyPath::Success => gas.success_gas,
            }
        }
    }

    #[wasm_bindgen_test]
    fn trait_methods_have_sensible_defaults() {
        // requires_eoa default false
        assert!(!DummyPrecompile::requires_eoa(0));
        assert!(!DummyPrecompile::requires_eoa(1));
        // is_mint_opcode default false
        assert!(!DummyPrecompile::is_mint_opcode(0));
        assert!(!DummyPrecompile::is_mint_opcode(1));
        // requires_solo_cellpack default false
        assert!(!DummyPrecompile::requires_solo_cellpack(0));
        assert!(!DummyPrecompile::requires_solo_cellpack(1));
    }

    #[wasm_bindgen_test]
    fn handles_opcode_gates_correctly() {
        assert!(DummyPrecompile::handles_opcode(1));
        assert!(!DummyPrecompile::handles_opcode(2));
        assert!(!DummyPrecompile::handles_opcode(99));
    }

    #[wasm_bindgen_test]
    fn gas_table_lookup_works() {
        let gas = DummyGas { success_gas: 12345 };
        assert_eq!(
            DummyPrecompile::gas_for_path(DummyPath::Success, &gas),
            12345
        );
    }
}
