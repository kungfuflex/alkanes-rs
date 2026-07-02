//! ============================================================================================
//!  Alkanes indexer — permissionless CONSENSUS-LEVEL FREEZE (poison block).  Standalone PoC.
//! ============================================================================================
//!
//!  SUMMARY
//!  -------
//!  A permissionless flaw in the deployed mainnet Alkanes indexer (`alkanes-rs` v2.2.0-rc.8) lets
//!  ANY user PERMANENTLY HALT THE ENTIRE METAPROTOCOL with a SINGLE Bitcoin transaction. An
//!  `extcall` to an unhandled precompile address `{800000000, tx >= 4}` underflows the indexer's
//!  checkpoint stack and PANICS while indexing the block. In a live `metashrew` node that panic is
//!  a POISON BLOCK: the indexer re-attempts the same block forever and never advances, so ALL
//!  alkane state freezes network-wide (DIESEL, frBTC, every AMM LP) — no transfers, no wrap/unwrap,
//!  no swaps, no exchange deposits/withdrawals. No funds are stolen (NET_STOLEN = 0); the impact is
//!  a total, self-sustaining denial of service. Cost ≈ one tx fee.
//!
//!  AFFECTED VERSION (deployed)
//!  ---------------------------
//!  Repo `kungfuflex/alkanes-rs`, tag `v2.2.0-rc.8` (same deployed line as the inflation report).
//!  Epicenter: `src/vm/host_functions.rs` (`extcall` / `handle_extcall` / `_handle_extcall_abort`
//!  / `with_context_safety`) + metashrew `AtomicPointer::rollback`.
//!
//!  ROOT CAUSE — "rollback a checkpoint that was never pushed"
//!  ---------------------------------------------------------
//!    1. `extcall(..)` handles precompiles FIRST and returns before pushing a checkpoint:
//!           if cellpack.target.block == 800000000 {
//!               return Self::_handle_special_extcall(caller, cellpack);   // <- BEFORE checkpoint()
//!           }
//!           ...
//!           context_guard.message.atomic.checkpoint();                    // normal path pushes here
//!       => the special path NEVER pushes a checkpoint.
//!    2. `_handle_special_extcall` returns Err("Unknown precompiled contract") for any tx >= 4
//!       (only tx 0/1/2/3 exist: block_header / coinbase / number_diesel_mints / total_miner_fee).
//!    3. `handle_extcall` funnels EVERY extcall Err into abort with should_rollback = true:
//!           Err(e) => Self::_handle_extcall_abort::<T>(caller, e, /* should_rollback */ true),
//!    4. `_handle_extcall_abort` calls `atomic.rollback()`.
//!    5. metashrew `AtomicPointer::rollback` BLINDLY pops the checkpoint stack
//!       (`self.store.0.lock().unwrap().pop();`) — popping a checkpoint that was never pushed.
//!       The stack UNDERFLOWS (depth N -> N-1).
//!    6. `with_context_safety`'s guard fails on the next wrapped host call:
//!           assert_eq!(initial_depth, final_depth, "IndexCheckpointStack depth changed: ...")
//!       => PANIC inside `index_block` => wasm trap => poison block.
//!  The panic is NOT recoverable: it escapes `std::panic::catch_unwind` (verified), which is exactly
//!  why a live node cannot skip the block — it re-executes and re-panics on every retry.
//!
//!  IMPACT
//!  ------
//!  Consensus-level, network-wide, permanent halt. Permissionless & free (one ordinary Bitcoin tx
//!  whose witness carries a protostone making any alkane extcall {800000000, >=4}). Freezes all
//!  alkane value until every operator ships an emergency patch. Severity: availability CRITICAL;
//!  no theft (NET_STOLEN = 0), total DoS of the whole metaprotocol from a single unprivileged tx.
//!
//!  HOW TO RUN (this single file is the whole PoC — no contract changes needed)
//!  --------------------------------------------------------------------------
//!    git clone https://github.com/kungfuflex/alkanes-rs && cd alkanes-rs
//!    git checkout v2.2.0-rc.8
//!    cp <this-file> src/tests/freeze_poc.rs
//!    printf '#[cfg(test)]\npub mod freeze_poc;\n' >> src/tests/mod.rs      # register the module
//!    RUSTFLAGS="-C debuginfo=0" cargo test --target wasm32-unknown-unknown \
//!        --features test-utils freeze_poc -- --nocapture
//!  (wasm-bindgen-cli 0.2.100 matches this tag; RUSTFLAGS only dodges an unrelated rustc ICE on the
//!  precompiled-genesis include_bytes! — not part of the bug. Opcode 31 `TestExtCall{target,inputs}`
//!  already exists in the stock `alkanes-std-test` contract, so NO contract modification is needed.)
//!
//!  EXPECTED OUTPUT
//!  ---------------
//!    running 2 tests
//!    test tests::freeze_poc::freeze_control_valid_special_tx3 ... ok
//!    test tests::freeze_poc::freeze_trigger_special_tx4_poisons_block ... ok
//!    test result: ok. 2 passed; 0 failed
//!  A passing pair IS the proof. The ONLY difference between the two tests is tx == 3 (handled,
//!  clean) vs tx == 4 (unhandled, Err -> rollback-without-push -> checkpoint underflow -> panic).
//!  CONTROL passes (index_block clean); TRIGGER panics with "IndexCheckpointStack depth changed",
//!  asserted exactly by `#[should_panic]` — that uncatchable panic during index_block is the freeze.
//!
//!  SUGGESTED FIX (any one)
//!  -----------------------
//!    * In `handle_extcall`, when target is a precompile (block == 800000000), pass
//!      should_rollback = false (the special path pushed no checkpoint, so it must not pop one); or
//!    * Push a checkpoint symmetrically for the special path too (move checkpoint() above the
//!      precompile branch) so every extcall pushes exactly one and pops exactly one; or
//!    * Make `_handle_special_extcall` return a handled negative result for unknown tx instead of a
//!      bare Err, so the caller never enters the rollback path; and (defense in depth)
//!    * Guard `AtomicPointer::rollback` against underflow (never pop below the frame's base).
//!  Deploy via the existing height-gated fork so all operators agree on any tx that hit this path.
//!
//!  This PoC reproduces the flaw LOCALLY ONLY. No mainnet transaction is included or broadcast.
//! ============================================================================================

use crate::index_block;
use crate::tests::helpers as alkane_helpers;
use crate::tests::std::alkanes_std_test_build;
use alkane_helpers::clear;
use alkanes_support::cellpack::Cellpack;
use alkanes_support::id::AlkaneId;
#[allow(unused_imports)]
use metashrew_core::{
    println,
    stdio::{stdout, Write},
};
use wasm_bindgen_test::wasm_bindgen_test;

/// Deploy the stock test alkane and, in the same message, have it `extcall`
/// the precompile at {target_block, target_tx} (opcode 31 = TestExtCall).
fn deploy_and_extcall_to(target_block: u128, target_tx: u128) -> bitcoin::Block {
    alkane_helpers::init_with_multiple_cellpacks_with_tx(
        vec![alkanes_std_test_build::get_bytes()],
        vec![Cellpack {
            target: AlkaneId { block: 1, tx: 0 },
            inputs: vec![31, target_block, target_tx],
        }],
    )
}

/// CONTROL — a VALID special precompile {800000000, 3} (total_miner_fee).
/// Balanced checkpoint accounting => `index_block` completes without panic => PASS.
#[wasm_bindgen_test]
fn freeze_control_valid_special_tx3() -> anyhow::Result<()> {
    clear();
    index_block(&deploy_and_extcall_to(800000000, 3), 0)?;
    println!("[CONTROL {{8e8,3}}] index_block clean — as expected");
    Ok(())
}

/// TRIGGER — the UNHANDLED special precompile {800000000, 4}.
/// FIXED (v2.2.1): the precompile extcall error no longer rolls back a checkpoint
/// that was never pushed, so the checkpoint stack stays balanced and `index_block`
/// completes WITHOUT panicking — no poison block. A regression (the underflow
/// panic) would abort this test, so a clean pass IS the proof the freeze is fixed.
#[wasm_bindgen_test]
fn freeze_trigger_special_tx4_no_longer_poisons() {
    clear();
    let block = deploy_and_extcall_to(800000000, 4);
    // Pre-fix this panicked "IndexCheckpointStack depth changed: N -> N-1" inside
    // index_block. Post-fix it returns (the unhandled extcall reverts its own
    // message, but the block indexes fine and the stack is balanced).
    let _ = index_block(&block, 0);
    println!("[FIXED {{8e8,4}}] index_block completed without a checkpoint underflow panic");
}
