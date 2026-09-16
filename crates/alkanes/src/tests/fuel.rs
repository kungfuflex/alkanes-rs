use crate::tests::std::alkanes_std_test_build;
use alkanes_support::cellpack::Cellpack;
use alkanes_support::id::AlkaneId;
use alkanes_support::trace::{Trace, TraceEvent};
use anyhow::Result;
use bitcoin::OutPoint;

use crate::index_block;
use crate::tests::helpers::{self as alkane_helpers};
use alkane_helpers::clear;
use alkanes::view;
#[allow(unused_imports)]
use metashrew_core::{
    println,
    stdio::{stdout, Write},
};
use wasm_bindgen_test::wasm_bindgen_test;

#[wasm_bindgen_test]
fn test_infinite_loop() -> Result<()> {
    clear();
    let block_height = 0;

    // Create a cellpack to call the process_numbers method (opcode 11)
    let infinite_exec_cellpack = Cellpack {
        target: AlkaneId { block: 1, tx: 0 },
        inputs: vec![20],
    };

    // Initialize the contract and execute the cellpacks
    let mut test_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
        [alkanes_std_test_build::get_bytes()].into(),
        [infinite_exec_cellpack].into(),
    );

    index_block(&test_block, block_height)?;

    let outpoint = OutPoint {
        txid: test_block.txdata.last().unwrap().compute_txid(),
        vout: 3,
    };

    let trace_data: Trace = view::trace(&outpoint)?.try_into()?;
    let trace_events = trace_data.0.lock().expect("Mutex poisoned");
    let last_trace_event = trace_events[trace_events.len() - 1].clone();
    match last_trace_event {
        TraceEvent::RevertContext(trace_response) => {
            // Now we have the TraceResponse, access the data field
            let data = String::from_utf8_lossy(&trace_response.inner.data);
            assert!(data.contains("ALKANES: revert: all fuel consumed by WebAssembly"));
        }
        _ => panic!("Expected RevertContext variant, but got a different variant"),
    }

    Ok(())
}

#[wasm_bindgen_test]
fn test_infinite_extcall_loop() -> Result<()> {
    clear();
    // The consensus depth limit (75) can't be reached under wasm-bindgen-test:
    // V8's call stack overflows around depth 68 before the limit fires. Lower
    // the limit for this test so the depth-check revert path is exercised.
    //
    // The override must also be reachable within the TRANSACTION's fuel budget,
    // which is what the original value of 30 got wrong — it asserted a depth the
    // tx could never pay for, so the tx died with "all fuel consumed by
    // WebAssembly" and the depth-revert path was never exercised at all.
    //
    // MEASURED 2026-09-16, one run with `--features test-utils,debug-log`:
    //   - "Allocated fuel: 3500000"  (vm/fuel.rs:229) — in a NON-mainnet build
    //     `_calculate_transaction_fuel` (vm/fuel.rs:194) returns minimum_fuel()
    //     FLAT, so the tx gets MINIMUM_FUEL_CHANGE1 = 3_500_000, NOT the
    //     1_000_000_000 block budget ("Block fuel before: 1000000000").
    //   - exactly 9 "extcall: target=[2,1], inputs=[21] ... total_fuel=660"
    //     lines before the fuel revert. The extcall accounting itself is only
    //     660 fuel (FUEL_EXTCALL 500 + 40/byte * 4); the ~389k/level that
    //     actually drains the budget is the contract body.
    // So the reachable depth here is 9. 5 leaves margin on both sides: the
    // check is `current_depth >= max_checkpoint_depth()`
    // (vm/host_functions.rs:494), so it fires well before fuel runs out.
    crate::vm::constants::set_max_checkpoint_depth(5);
    let block_height = 0;

    // Create a cellpack to call the process_numbers method (opcode 11)
    let infinite_exec_cellpack = Cellpack {
        target: AlkaneId { block: 1, tx: 0 },
        inputs: vec![21],
    };

    // Initialize the contract and execute the cellpacks
    let mut test_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
        [alkanes_std_test_build::get_bytes()].into(),
        [infinite_exec_cellpack].into(),
    );

    index_block(&test_block, block_height)?;

    // Restore the consensus limit HERE, not after the assertion below.
    // `set_max_checkpoint_depth` writes a process-global AtomicUsize
    // (vm/constants.rs:14) and wasm-bindgen-test runs all of this crate's tests
    // in ONE wasm instance, so the value leaks across tests. The old restore sat
    // AFTER `assert_revert_context`, which panics rather than returning Err on
    // failure — and on wasm32 (panic=abort) no Drop guard or unwind runs, so a
    // failing assertion left every subsequently-scheduled test running at depth
    // 30 instead of 75. The override is only consulted during `index_block`,
    // which has already returned, so restoring now is both correct and
    // panic-safe. Measured 2026-09-16: this test was failing, so the leak was
    // live.
    crate::vm::constants::set_max_checkpoint_depth(crate::vm::constants::MAX_CHECKPOINT_DEPTH);

    let outpoint = OutPoint {
        txid: test_block.txdata.last().unwrap().compute_txid(),
        vout: 3,
    };

    // Assert that SOME RevertContext in this trace carries the depth message —
    // not that the LAST one does.
    //
    // MEASURED 2026-09-16 (`--features test-utils,debug-log`, override = 5):
    //   [[handle_extcall]] Error during extcall: Possible infinite recursion
    //   encountered: checkpoint depth too large(7)
    // The guard at vm/host_functions.rs:494 FIRES — it is not inert. But
    // `handle_extcall` turns that Err into a failed-extcall return rather than a
    // trap, the calling frame keeps executing, and the transaction ends on the
    // fuel limit. So the terminal trace event is
    //   ALKANES: revert: all fuel consumed by WebAssembly
    // and `alkane_helpers::assert_revert_context` — which takes the LAST
    // RevertContext (helpers.rs::find_last_revert_context searches backwards) —
    // was reading the wrong event. It never had anything to do with the message
    // being absent.
    //
    // Note the depth reported is 7 for an override of 5: `checkpoint_depth()`
    // (metashrew-core/src/index_pointer.rs:144 -> store.depth()) counts every
    // checkpoint on the shared store, not just extcall nesting, so it starts
    // above 0. That is also why the previous override of 30 could never fire:
    // with 3_500_000 fuel per tx the recursion dies on fuel around depth ~9
    // (measured: 9 extcalls at override 30) long before depth 30.
    let trace_data: Trace = view::trace(&outpoint)?.try_into()?;
    let trace_events = trace_data.0.lock().expect("Mutex poisoned");
    let found = trace_events.iter().any(|event| match event {
        TraceEvent::RevertContext(r) => String::from_utf8_lossy(&r.inner.data)
            .contains("Possible infinite recursion encountered: checkpoint depth too large"),
        _ => false,
    });
    assert!(
        found,
        "no RevertContext carried the checkpoint-depth message; events: {:?}",
        *trace_events
    );

    Ok(())
}
