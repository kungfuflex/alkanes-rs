//! Broadcast-response interpretation for gateway-stripped bitcoind errors.
//!
//! Background — the runtime gap this closes:
//!
//!   The subfrost RPC gateway (tlsd-edge alkanes-jsonrpc WASM) STRIPS
//!   JSON-RPC error envelopes from bitcoind responses: bitcoind's 1.0
//!   error shape carries BOTH `result: null` AND `error: {...}`, and the
//!   gateway's untagged `JsonRpcResponse` enum matches that as a Success,
//!   dropping the `error` member. Every bitcoind rejection therefore
//!   reaches the client as HTTP 200 `{"result": null}` — verified live
//!   2026-07-13. Success responses pass through intact.
//!
//!   Result: `sendrawtransaction` failures surfaced as the useless
//!   "Invalid txid response" instead of bitcoind's real reject reason
//!   (bad-txns-*, scriptpubkey, min relay fee, mempool conflict, ...).
//!   PR #370 fixed this for the frontend's own broadcast path
//!   (subfrost-app `lib/alkanes/rpc.ts`), but the keystore/autoConfirm
//!   flows broadcast INSIDE the SDK WASM (`alkanes-web-sys`
//!   `send_raw_transaction`) — only an SDK-level fix reaches those.
//!
//! The recovery trick (same as PR #370):
//!
//!   `testmempoolaccept` returns its verdict INSIDE `result` (verified
//!   live: `{"result":[{"allowed":false,"reject-reason":"scriptpubkey",
//!   "txid":"..."}]}`), so it SURVIVES the stripping. When
//!   `sendrawtransaction` comes back with the stripped-error signature
//!   (non-string result), providers re-probe the same tx hex via
//!   `testmempoolaccept([[tx_hex]])` and this module turns the pair of
//!   raw responses into either a txid or an actionable error message.
//!
//! Decision table (mirrors PR #370's frontend logic):
//!
//!   - sendraw result is a 64-hex txid string  -> Ok(txid), no probe needed.
//!   - probe verdict `allowed:false` with reject-reason
//!     `txn-already-in-mempool` / `txn-already-known` -> Ok(verdict txid).
//!     This is SUCCESS IN DISGUISE: `sendrawtransaction` returns error
//!     -27 for a tx that is ALREADY in the mempool (stripped to null),
//!     but the tx IS there — throwing would fail a broadcast that
//!     effectively succeeded (observed with retry loops / RBF races).
//!   - probe verdict `allowed:false` otherwise -> Err with bitcoind's
//!     real reject-reason.
//!   - probe verdict `allowed:true` -> Err naming the gateway as the
//!     fault: the node WOULD accept the tx but the gateway returned no
//!     txid, so the tx may not have propagated — caller should retry.
//!   - probe null / failed -> Err with a generic gateway-stripped
//!     message. This happens when the tx hex is UNDECODABLE: bitcoind
//!     errors (-22) before producing a verdict, so `testmempoolaccept`
//!     ALSO comes back as `result: null`.
//!
//! Why this lives in `alkanes-cli-common` and not `alkanes-web-sys`:
//! `alkanes-web-sys` only compiles for wasm32 (`cargo test -p
//! alkanes-web-sys` cannot run on host), so the interpretation is
//! factored into pure functions here — no provider/network/async deps —
//! where `cargo test -p alkanes-cli-common` exercises them natively.
//! Both the web (`alkanes-web-sys/src/provider.rs`) and native
//! (`provider.rs` `ConcreteProvider`) `send_raw_transaction` impls call
//! into this module; the gateway strips identically for both since the
//! mangling happens server-side.

// alloc-only imports: this module must compile for every crate config
// (std, web-compat, wasip2) — `extern crate alloc` is unconditional in
// lib.rs, so `alloc::` paths work everywhere.
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use serde_json::Value as JsonValue;

/// Reject reasons that mean the tx is ALREADY in the mempool — success
/// in disguise (see module docs). `txn-already-known` additionally
/// covers the wtxid-known variant some Core versions report.
const ALREADY_IN_MEMPOOL_REASONS: [&str; 2] = ["txn-already-in-mempool", "txn-already-known"];

/// Generic error for the fully-blind case: sendraw stripped AND the
/// probe produced no verdict (undecodable hex errors at -22 before a
/// verdict exists, so it is stripped to null too).
pub const GATEWAY_STRIPPED_MSG: &str = "Broadcast failed: node rejected the transaction but the RPC gateway stripped the reason (known gateway bug); tx hex may be malformed";

/// Returns the txid iff `result` is the canonical `sendrawtransaction`
/// success shape: a 64-char hex string. Anything else (null from the
/// stripped error envelope, objects, short strings) is treated as the
/// stripped-error signature by callers, who then probe
/// `testmempoolaccept`. Same txid-shape validation as PR #370's
/// frontend `broadcastTransaction`.
pub fn sendraw_txid(result: &JsonValue) -> Option<String> {
    let s = result.as_str()?;
    if s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()) {
        Some(s.to_string())
    } else {
        None
    }
}

/// Interpret a batch `sendrawtransactions` result. Returns `Some(txids)`
/// ONLY when the result is the canonical success shape — a JSON array in
/// which EVERY entry is a string. Returns `None` for the gateway-stripped
/// signature (`Null`), a non-array, or an array with a non-string entry —
/// the caller then falls back to the sequential per-tx broadcast path,
/// which probes each tx via the hardened single-tx broadcast (FIX #3).
///
/// Note: unlike `submitpackage`, bitcoind's `sendrawtransactions` success
/// result is a bare array of txid strings, so there is no wtxid→txid
/// indirection to resolve here — the strings ARE the txids.
pub fn batch_result_txids(batch_result: &JsonValue) -> Option<Vec<String>> {
    let arr = batch_result.as_array()?;
    arr.iter()
        .map(|v| v.as_str().map(|s| s.to_string()))
        .collect()
}

/// Interpret a `sendrawtransaction` result together with an optional
/// `testmempoolaccept([[tx_hex]])` probe result (None if the probe call
/// itself failed). Pure — takes raw `serde_json::Value`s, returns
/// `Ok(txid)` or `Err(actionable message)` — so it is unit-testable
/// without wasm or a network. Callers map `Err` into
/// `AlkanesError::RpcError`.
pub fn interpret_broadcast_response(
    sendraw_result: &JsonValue,
    probe_result: Option<&JsonValue>,
) -> Result<String, String> {
    // Fast path: gateway passes success envelopes through intact, so a
    // txid-shaped string is trustworthy as-is.
    if let Some(txid) = sendraw_txid(sendraw_result) {
        return Ok(txid);
    }

    // Stripped-error signature. Extract the first verdict from the probe:
    // testmempoolaccept returns one verdict object per submitted hex and
    // we always probe exactly the one tx that failed to broadcast.
    let verdict = probe_result
        .and_then(|p| p.as_array())
        .and_then(|arr| arr.first());

    let Some(verdict) = verdict else {
        // Probe failed, returned null (undecodable hex -> -22 before a
        // verdict, stripped too), or returned a shape we don't recognize:
        // nothing more to learn — surface the gateway bug explicitly so
        // the failure is at least attributable.
        return Err(GATEWAY_STRIPPED_MSG.to_string());
    };

    match verdict.get("allowed").and_then(|a| a.as_bool()) {
        Some(false) => {
            let reason = verdict
                .get("reject-reason")
                .and_then(|r| r.as_str())
                .unwrap_or("unknown reject reason");
            if ALREADY_IN_MEMPOOL_REASONS.contains(&reason) {
                // Success in disguise: sendrawtransaction's -27 (stripped
                // to null) + this verdict means the tx IS in the mempool.
                // Return its txid instead of failing the broadcast.
                if let Some(txid) = verdict.get("txid").and_then(|t| t.as_str()) {
                    return Ok(txid.to_string());
                }
                // Defensive: Core always includes txid alongside a
                // verdict, but if it is somehow absent we cannot claim
                // success without one to hand back.
                return Err(format!(
                    "Transaction is already in the mempool ({reason}) but the probe verdict carried no txid"
                ));
            }
            // The real reject reason bitcoind produced and the gateway
            // swallowed — the entire point of this fallback.
            Err(format!("Broadcast rejected by node: {reason}"))
        }
        Some(true) => {
            // Anomaly: node says it WOULD accept the tx, yet sendraw
            // returned no txid. That points at the gateway (or a race
            // where the tx was evicted between calls) — the tx may NOT
            // have propagated, so this must stay an error the caller can
            // retry, not a silent success.
            let txid = verdict.get("txid").and_then(|t| t.as_str()).unwrap_or("unknown");
            Err(format!(
                "Broadcast anomaly: node reports it would accept the transaction (txid {txid}) but the RPC gateway returned no txid from sendrawtransaction; the transaction may not have propagated — retry the broadcast"
            ))
        }
        // Verdict object without a boolean `allowed` — treat as no verdict.
        None => Err(GATEWAY_STRIPPED_MSG.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // Shape validity only — a syntactically valid txid, NOT a real one
    // (no hardcoded live cryptographic values per repo test policy).
    const TXID: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    #[test]
    fn string_txid_passes_through() {
        let result = json!(TXID);
        assert_eq!(
            interpret_broadcast_response(&result, None),
            Ok(TXID.to_string())
        );
        // And the probe (even a hostile one) is irrelevant when sendraw succeeded.
        let probe = json!([{ "allowed": false, "reject-reason": "scriptpubkey", "txid": TXID }]);
        assert_eq!(
            interpret_broadcast_response(&result, Some(&probe)),
            Ok(TXID.to_string())
        );
    }

    #[test]
    fn non_txid_string_is_not_success() {
        // Defense against a gateway returning a non-txid string: must not
        // be blindly propagated as a txid.
        assert_eq!(sendraw_txid(&json!("OK")), None);
        assert_eq!(sendraw_txid(&json!(null)), None);
        assert_eq!(
            interpret_broadcast_response(&json!("OK"), None),
            Err(GATEWAY_STRIPPED_MSG.to_string())
        );
    }

    #[test]
    fn refused_verdict_surfaces_real_reason() {
        // The live-verified shape: {"result":[{"allowed":false,
        // "reject-reason":"scriptpubkey","txid":"..."}]} survives stripping.
        let probe = json!([{ "txid": TXID, "allowed": false, "reject-reason": "scriptpubkey" }]);
        let err = interpret_broadcast_response(&json!(null), Some(&probe)).unwrap_err();
        assert_eq!(err, "Broadcast rejected by node: scriptpubkey");
    }

    #[test]
    fn refused_verdict_without_reason_still_errors() {
        let probe = json!([{ "txid": TXID, "allowed": false }]);
        let err = interpret_broadcast_response(&json!(null), Some(&probe)).unwrap_err();
        assert_eq!(err, "Broadcast rejected by node: unknown reject reason");
    }

    #[test]
    fn already_in_mempool_is_success_in_disguise() {
        // sendrawtransaction -27 (stripped to null) + this verdict = the
        // tx IS in the mempool; return the verdict's txid.
        for reason in ["txn-already-in-mempool", "txn-already-known"] {
            let probe = json!([{ "txid": TXID, "allowed": false, "reject-reason": reason }]);
            assert_eq!(
                interpret_broadcast_response(&json!(null), Some(&probe)),
                Ok(TXID.to_string()),
                "reason {reason} must be treated as success"
            );
        }
    }

    #[test]
    fn null_probe_yields_gateway_message() {
        // Undecodable hex: bitcoind -22s before a verdict, so the probe
        // is ALSO stripped to null — or the probe call failed outright.
        assert_eq!(
            interpret_broadcast_response(&json!(null), None),
            Err(GATEWAY_STRIPPED_MSG.to_string())
        );
        assert_eq!(
            interpret_broadcast_response(&json!(null), Some(&json!(null))),
            Err(GATEWAY_STRIPPED_MSG.to_string())
        );
        assert_eq!(
            interpret_broadcast_response(&json!(null), Some(&json!([]))),
            Err(GATEWAY_STRIPPED_MSG.to_string())
        );
    }

    #[test]
    fn allowed_true_anomaly_is_error_not_success() {
        // Node would accept it but the gateway returned no txid: the tx
        // may not have propagated, so claiming success would be a lie.
        let probe = json!([{ "txid": TXID, "allowed": true }]);
        let err = interpret_broadcast_response(&json!(null), Some(&probe)).unwrap_err();
        assert!(err.contains("may not have propagated"), "got: {err}");
        assert!(err.contains(TXID), "anomaly message should carry the txid: {err}");
    }

    // --- batch sendrawtransactions decision (FIX #3) ----------------------
    // These pin the Null-batch → sequential-probe fallback decision: the
    // provider treats `None` from `batch_result_txids` as "not a usable
    // batch result, fall through to the hardened per-tx sequential path".

    const TXID_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    #[test]
    fn batch_array_of_strings_is_usable() {
        let result = json!([TXID, TXID_B]);
        assert_eq!(
            batch_result_txids(&result),
            Some(vec![TXID.to_string(), TXID_B.to_string()])
        );
    }

    #[test]
    fn batch_stripped_null_is_not_usable_triggers_sequential() {
        // THE FIX #3 CASE: a rejected batch is stripped to Ok(Null), NOT an
        // Err. `None` here is what makes the provider fall through to the
        // sequential per-tx probe instead of throwing an opaque error.
        assert_eq!(batch_result_txids(&json!(null)), None);
    }

    #[test]
    fn batch_non_array_shapes_are_not_usable() {
        // An object (e.g. a stray submitpackage-style envelope) or a bare
        // string is not the sendrawtransactions success shape → sequential.
        assert_eq!(batch_result_txids(&json!({ "package_msg": "success" })), None);
        assert_eq!(batch_result_txids(&json!("not-an-array")), None);
    }

    #[test]
    fn batch_array_with_non_string_entry_is_not_usable() {
        // A malformed array (one entry isn't a txid string) must not be
        // half-trusted — fall back to the per-tx path so each is probed.
        assert_eq!(batch_result_txids(&json!([TXID, 42])), None);
        assert_eq!(batch_result_txids(&json!([TXID, null])), None);
    }

    #[test]
    fn batch_empty_array_is_usable_but_empty() {
        // An empty array is technically well-shaped (zero txids); the caller
        // only reaches the batch path with >1 hex, so this is a defensive
        // edge — it is NOT the stripped-null signature, so returns Some([]).
        assert_eq!(batch_result_txids(&json!([])), Some(Vec::<String>::new()));
    }
}
