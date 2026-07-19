use crate::tables::{TRACES, TRACES_BY_HEIGHT};
use alkanes_support::proto;
use alkanes_support::trace::Trace;
use anyhow::Result;
use bitcoin::OutPoint;
use metashrew_support::index_pointer::KeyValuePointer;
use metashrew_support::utils::consensus_encode;
use prost::Message;
use std::cell::RefCell;
use std::sync::Arc;
#[allow(unused_imports)]
use {
    metashrew_core::{println, stdio::stdout},
    std::fmt::Write,
};

// View-mode trace collector. When set (by `simulatetransaction` and similar
// non-persisting view paths), `save_trace` pushes the trace into this buffer
// instead of writing to the on-disk TRACES + TRACES_BY_HEIGHT tables. The
// indexer's hot path is unchanged: by default the collector is None, and
// `save_trace` behaves exactly as before.
//
// Thread-local so the trace collector for one view request can't bleed into
// another. wasm32 is single-threaded so this is effectively a static; the
// `thread_local!` macro works identically.
thread_local! {
    static VIEW_TRACE_COLLECTOR: RefCell<Option<Vec<(OutPoint, Trace)>>> =
        const { RefCell::new(None) };
}

/// Switch trace persistence into "collect" mode. Used by `simulatetransaction`
/// before driving `index_protostones`. Resets any prior collected entries.
pub fn enable_view_trace_collector() {
    VIEW_TRACE_COLLECTOR.with(|c| *c.borrow_mut() = Some(Vec::new()));
}

/// Take the collected (outpoint, trace) pairs and disable the collector.
/// Returns the pairs in the order they were saved during execution.
pub fn drain_view_traces() -> Vec<(OutPoint, Trace)> {
    VIEW_TRACE_COLLECTOR.with(|c| c.borrow_mut().take().unwrap_or_default())
}

/// Disable the collector without taking what's there (used as a safety
/// drop-guard).
pub fn disable_view_trace_collector() {
    VIEW_TRACE_COLLECTOR.with(|c| *c.borrow_mut() = None);
}

pub fn save_trace(outpoint: &OutPoint, height: u64, trace: Trace) -> Result<()> {
    // If a view-mode collector is active, push the trace into it and skip the
    // on-disk write. This keeps `simulatetransaction` non-persisting at the
    // per-message granularity, matching the no-write semantics the rest of
    // the view path achieves via the sandbox AtomicPointer.
    let collected = VIEW_TRACE_COLLECTOR.with(|c| {
        if let Some(buf) = c.borrow_mut().as_mut() {
            buf.push((outpoint.clone(), trace.clone()));
            true
        } else {
            false
        }
    });
    if collected {
        return Ok(());
    }

    // Normal persistent path — what the live indexer always does.
    let buffer: Vec<u8> = consensus_encode::<OutPoint>(outpoint)?;
    TRACES.select(&buffer).set(Arc::<Vec<u8>>::new(
        <Trace as Into<proto::alkanes::AlkanesTrace>>::into(trace).encode_to_vec(),
    ));
    TRACES_BY_HEIGHT
        .select_value(height)
        .append(Arc::new(buffer));
    Ok(())
}
