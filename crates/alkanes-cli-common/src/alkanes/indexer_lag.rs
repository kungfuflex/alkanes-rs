//! Indexer-lag helpers for spend paths.
//!
//! Ported from `subfrost-mobile/crates/subfrost-mobile-ffi/src/rpc.rs`'s
//! `fetch_max_indexed_height_or_none` pattern.
//!
//! The idea: before building a PSBT we ask the indexer where it's caught up
//! to. If we can find out, we skip UTXOs whose creating block is past that
//! point — those might hold alkanes the indexer hasn't seen yet, and
//! spending them blind would silently burn the unseen alkanes. If we
//! *can't* find out (RPC failure, network blip), we DEGRADE GRACEFULLY:
//! return `None`, which `select_utxos` interprets as "no filter, spend
//! anything". That's the same trade-off subfrost-mobile makes — locking
//! out the user when the indexer is unreachable is worse than the rare
//! case of an unseen-alkane spend.
//!
//! The actual filtering happens in [`check_utxo_eligibility`], which the
//! main `select_utxos` path already calls; this module is just the
//! convenience for the fetch.

use crate::traits::MetashrewRpcProvider;

/// Best-effort fetch of the indexer tip height.
///
/// Returns `None` on any RPC failure — the caller MUST treat this as
/// "spend without the filter" (degraded mode) rather than as a fatal
/// error. See module docs for rationale.
///
/// Cost: one extra `metashrew_height` JSON-RPC round-trip (~50ms against
/// subfrost.io). Worth paying on every PSBT build path because the
/// alternative is silent alkane burn on a single bad pick.
pub async fn fetch_max_indexed_height_or_none<P: MetashrewRpcProvider + ?Sized>(
    provider: &P,
) -> Option<u64> {
    match provider.get_metashrew_height().await {
        Ok(h) => Some(h),
        Err(e) => {
            log::warn!(
                "fetch_max_indexed_height_or_none: metashrew_height failed ({e}); \
                 spend path will run without indexer-lag filter"
            );
            None
        }
    }
}

// The function is a 3-line `Result::ok` wrapper; we don't try to mock the
// 20-method MetashrewRpcProvider trait here. Real coverage comes from the
// integration suite that exercises ConcreteProvider against a live or
// mocked metashrew endpoint.
