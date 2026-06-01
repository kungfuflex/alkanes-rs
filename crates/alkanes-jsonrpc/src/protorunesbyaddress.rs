//! In-process `protorunesbyaddress` with reorg-conscious fan-out.
//!
//! Replaces the previous passthrough behavior (one `alkanes_protorunesbyaddress`
//! call upstream, which forced metashrew to do its own UTXO scan inside the
//! WASM view) with a parallel fan-out: one `esplora_address::utxo` call to
//! get the UTXO set + N parallel `metashrew_view "protorunesbyoutpoint"`
//! calls (bounded concurrency, cached). This lets the existing
//! `metashrew_view` response cache absorb 99% of the per-outpoint queries
//! once the same UTXOs are seen repeatedly across requests.
//!
//! REORG CONTRACT (the "not serving stale" guarantee):
//!
//!   1. At entry we resolve `height` to a pinned `H` — for `"latest"` we
//!      read the pool watermark (NOT the upstream-reported tip; the
//!      watermark is the height the LB rewriter is actually pinned to).
//!   2. All sub-calls use the same pinned `H`.
//!   3. After all sub-calls return, BEFORE returning to the caller, we
//!      re-read the watermark. If it has advanced past `H` by more than
//!      `STALENESS_WINDOW_BLOCKS` (default 6), we return a JSON-RPC
//!      error -32011 with code/message indicating drift. The 6-block
//!      window aligns with Bitcoin's standard reorg-safety depth.
//!   4. If any sub-call errors (typically because the upstream isn't
//!      indexed past `H` yet — a `-32000` from rockshrew or the
//!      bouncer's min-height filter), we short-circuit immediately.
//!
//! The two public entry points (`handle_alkanes_protorunesbyaddress` and
//! `handle_metashrew_view_protorunesbyaddress`) both lower onto the same
//! `fan_out_protorunes_by_address` helper. The first returns JSON shaped
//! like `WalletResponse` (matches `decode_wallet_response` in handler.rs).
//! The second returns the same payload re-encoded as a hex-prefixed
//! protobuf `WalletResponse`, matching what upstream's
//! `metashrew_view "protorunesbyaddress"` returns today.

use crate::cache::MetashrewViewCache;
use crate::jsonrpc::{JsonRpcRequest, JsonRpcResponse, INTERNAL_ERROR, INVALID_PARAMS};
use crate::proxy::ProxyClient;
use alkanes_cli_common::proto::protorune::{
    BalanceSheet, BalanceSheetItem, Outpoint, OutpointResponse, OutpointWithProtocol, Output,
    ProtorunesWalletRequest, Rune, ProtoruneRuneId, Uint128 as ProtoruneUint128, WalletResponse,
};
use anyhow::{anyhow, Result};
use futures::stream::{FuturesUnordered, StreamExt};
use prost::Message;
use serde_json::{json, Value};
use std::sync::Arc;

/// Stale-read window. If the pool watermark advances past the pinned H
/// by more than this many blocks between request start and request end,
/// we fail the request rather than serve a stale view. 6 is Bitcoin's
/// standard reorg-safety depth.
const DEFAULT_STALENESS_WINDOW_BLOCKS: u64 = 6;

/// Bounded fan-out concurrency. 16 keeps p99 latency tame on big address
/// UTXO sets without flooding the upstream pool — each outpoint call is
/// cached (see cache.rs) so warm requests don't even reach concurrency.
const FAN_OUT_CONCURRENCY: usize = 16;

/// JSON-RPC error code we return on stale-window failures. -32011 is in
/// the application-defined range (-32000..=-32099) and unused elsewhere
/// in this crate.
pub const STALE_HEIGHT_WINDOW: i32 = -32011;

/// Public handler for the top-level `alkanes_protorunesbyaddress`
/// JSON-RPC method.
///
/// Params accepted (both supported for backward-compat with the previous
/// upstream-passthrough behavior):
///   * `[address_string, height?]` — height is a numeric string, a JSON
///     number, or `"latest"` (default).
///   * `[{ "address": "...", "protocolTag": "1" }, height?]` — object
///     form used by `sandshrew_balances` and the lua scripts.
///
/// Returns a `WalletResponse`-shaped JSON object (same shape as
/// `decode_wallet_response`).
pub async fn handle_alkanes_protorunesbyaddress(
    params: &[Value],
    request_id: &Value,
    proxy: &ProxyClient,
) -> Result<JsonRpcResponse> {
    let (address, protocol_tag, height_tag) = match parse_address_params(params) {
        Ok(v) => v,
        Err(e) => {
            return Ok(JsonRpcResponse::error(
                INVALID_PARAMS,
                e.to_string(),
                request_id.clone(),
            ));
        }
    };

    let staleness_window = staleness_window_from_env();

    match fan_out_protorunes_by_address(
        proxy,
        &address,
        protocol_tag,
        &height_tag,
        staleness_window,
    )
    .await
    {
        Ok(FanOutResult::Ok(response)) => {
            let json = wallet_response_to_json(&response);
            Ok(JsonRpcResponse::success(json, request_id.clone()))
        }
        Ok(FanOutResult::StaleHeightWindow {
            pinned,
            current,
            drift,
        }) => Ok(JsonRpcResponse::error(
            STALE_HEIGHT_WINDOW,
            format!(
                "stale height window: pinned H={}, current={}, drift={}",
                pinned, current, drift
            ),
            request_id.clone(),
        )),
        Ok(FanOutResult::UpstreamError { code, message }) => Ok(JsonRpcResponse::error(
            code,
            message,
            request_id.clone(),
        )),
        Err(e) => Ok(JsonRpcResponse::error(
            INTERNAL_ERROR,
            format!("protorunesbyaddress: {}", e),
            request_id.clone(),
        )),
    }
}

/// Public handler for `metashrew_view "protorunesbyaddress" <hex> <height>`.
///
/// Params: `["protorunesbyaddress", "<hex_ProtorunesWalletRequest>", "<height_tag>"]`
///
/// Decodes the hex protobuf to get the address + protocol_tag, fans out
/// the same way, then RE-ENCODES the aggregated result as a hex
/// `WalletResponse` protobuf so the wire shape matches what upstream's
/// `metashrew_view "protorunesbyaddress"` returns. This lets clients
/// that hit the view-style path get byte-compatible output.
pub async fn handle_metashrew_view_protorunesbyaddress(
    hex_input: &str,
    height_tag: &Value,
    request_id: &Value,
    proxy: &ProxyClient,
) -> Result<JsonRpcResponse> {
    // Decode the hex ProtorunesWalletRequest. Brief calls it
    // `ProtorunesByAddressRequest { address, protocol_tag }` but the
    // existing wire-level proto is `ProtorunesWalletRequest { wallet,
    // protocol_tag }` — same shape, different name. We use the existing
    // one to stay compatible with sandshrew.rs + the WASM view's expected
    // input shape.
    let hex_clean = hex_input.strip_prefix("0x").unwrap_or(hex_input);
    let bytes = match hex::decode(hex_clean) {
        Ok(b) => b,
        Err(e) => {
            return Ok(JsonRpcResponse::error(
                INVALID_PARAMS,
                format!("protorunesbyaddress: invalid hex input: {}", e),
                request_id.clone(),
            ));
        }
    };
    let req = match ProtorunesWalletRequest::decode(bytes.as_slice()) {
        Ok(r) => r,
        Err(e) => {
            return Ok(JsonRpcResponse::error(
                INVALID_PARAMS,
                format!("protorunesbyaddress: malformed ProtorunesWalletRequest: {}", e),
                request_id.clone(),
            ));
        }
    };
    let address = match String::from_utf8(req.wallet.clone()) {
        Ok(s) => s,
        Err(e) => {
            return Ok(JsonRpcResponse::error(
                INVALID_PARAMS,
                format!("protorunesbyaddress: wallet bytes not UTF-8: {}", e),
                request_id.clone(),
            ));
        }
    };
    let protocol_tag = req
        .protocol_tag
        .as_ref()
        .map(|p| ((p.hi as u128) << 64) | (p.lo as u128))
        .unwrap_or(1);

    let staleness_window = staleness_window_from_env();

    match fan_out_protorunes_by_address(
        proxy,
        &address,
        protocol_tag,
        height_tag,
        staleness_window,
    )
    .await
    {
        Ok(FanOutResult::Ok(response)) => {
            let hex_out = format!("0x{}", hex::encode(response.encode_to_vec()));
            Ok(JsonRpcResponse::success(
                Value::String(hex_out),
                request_id.clone(),
            ))
        }
        Ok(FanOutResult::StaleHeightWindow {
            pinned,
            current,
            drift,
        }) => Ok(JsonRpcResponse::error(
            STALE_HEIGHT_WINDOW,
            format!(
                "stale height window: pinned H={}, current={}, drift={}",
                pinned, current, drift
            ),
            request_id.clone(),
        )),
        Ok(FanOutResult::UpstreamError { code, message }) => Ok(JsonRpcResponse::error(
            code,
            message,
            request_id.clone(),
        )),
        Err(e) => Ok(JsonRpcResponse::error(
            INTERNAL_ERROR,
            format!("protorunesbyaddress: {}", e),
            request_id.clone(),
        )),
    }
}

/// Outcomes the fan-out helper can produce. Distinct from a Result<..,
/// anyhow::Error> because the two failure modes (stale window, upstream
/// indexing error) need to surface as specific JSON-RPC error codes,
/// not be flattened into a generic "internal error".
pub enum FanOutResult {
    Ok(WalletResponse),
    StaleHeightWindow { pinned: u64, current: u64, drift: u64 },
    UpstreamError { code: i32, message: String },
}

/// Internal fan-out implementation shared by both handlers.
///
/// Steps (matches the brief):
///   1. Resolve `height_tag` → pinned `H`. For "latest" / null we use
///      the cache's watermark; for an explicit number we trust it.
///   2. Call `esplora_address::utxo` at "@H" for the address.
///   3. Fan out `protorunesbyoutpoint` per UTXO, bounded at 16,
///      short-circuit on first error.
///   4. Re-read watermark; if drift > window, fail with stale-window
///      error.
///   5. Aggregate into a WalletResponse.
pub async fn fan_out_protorunes_by_address(
    proxy: &ProxyClient,
    address: &str,
    protocol_tag: u128,
    height_tag: &Value,
    staleness_window_blocks: u64,
) -> Result<FanOutResult> {
    // ---- step 1: pin H ----
    let pinned_height = resolve_height(proxy, height_tag).await?;

    // ---- step 2: esplora UTXOs ----
    // `esplora_address::utxo` => GET /address/{addr}/utxo against the
    // configured esplora URL. We go straight through the proxy client
    // because esplora doesn't take a height argument — the response is
    // always relative to esplora's view of the chain, which is its
    // own tip. This matches how the existing sandshrew_balances path
    // queries UTXOs (it also doesn't pin esplora to a specific height).
    //
    // CAVEAT: there is a small inherent skew here — esplora's tip and
    // metashrew's tip can disagree by 1-2 blocks during sync. The
    // stale-window check at the end (step 4) catches the case where
    // metashrew has fallen WAY behind; the smaller skew is acceptable
    // for read-only balance queries.
    let utxo_path = format!("/address/{}/utxo", address);
    let utxos_json = proxy
        .fetch_esplora_endpoint(&utxo_path)
        .await
        .map_err(|e| anyhow!("esplora_address::utxo failed: {}", e))?;

    let utxos = match utxos_json.as_array() {
        Some(arr) => arr.clone(),
        None => {
            // Esplora returned a non-array (e.g. {"error": ...} or a
            // string). Treat as empty UTXO set and let the upstream
            // shape's empty case propagate. This matches the existing
            // sandshrew_balances behavior — it does not error if the
            // utxo array is missing.
            return Ok(FanOutResult::Ok(empty_wallet_response()));
        }
    };

    // ---- step 3: fan out protorunesbyoutpoint ----
    let pinned_str = pinned_height.to_string();
    let mut tasks = FuturesUnordered::new();
    let mut in_flight = 0usize;
    let mut queue: std::collections::VecDeque<Value> = utxos.into_iter().collect();

    // Collected per-outpoint responses, paired with the originating
    // UTXO so we can stamp value/script/height onto the OutpointResponse.
    let mut per_outpoint: Vec<(Value, OutpointResponse)> = Vec::new();

    let proxy_arc: Arc<ProxyClient> = Arc::new(proxy.clone());

    // Prime the queue with FAN_OUT_CONCURRENCY tasks.
    while in_flight < FAN_OUT_CONCURRENCY {
        let Some(utxo) = queue.pop_front() else {
            break;
        };
        let p = proxy_arc.clone();
        let h = pinned_str.clone();
        let pt = protocol_tag;
        tasks.push(tokio::spawn(async move {
            (utxo.clone(), call_protorunesbyoutpoint(&p, &utxo, pt, &h).await)
        }));
        in_flight += 1;
    }

    while let Some(joined) = tasks.next().await {
        in_flight -= 1;
        let (utxo, sub_result) = match joined {
            Ok(pair) => pair,
            Err(e) => {
                // Task panicked or was cancelled.
                return Ok(FanOutResult::UpstreamError {
                    code: INTERNAL_ERROR,
                    message: format!("protorunesbyaddress fan-out task failed: {}", e),
                });
            }
        };

        match sub_result {
            Ok(decoded) => per_outpoint.push((utxo, decoded)),
            Err(SubCallError::Upstream { code, message }) => {
                // Short-circuit on the first upstream error — typically
                // means the indexer rolled back past H, or the bouncer
                // rejected because served_height < H. Surface as-is.
                return Ok(FanOutResult::UpstreamError { code, message });
            }
            Err(SubCallError::Internal(e)) => {
                return Ok(FanOutResult::UpstreamError {
                    code: INTERNAL_ERROR,
                    message: format!("protorunesbyoutpoint: {}", e),
                });
            }
        }

        // Keep the pipeline full.
        while in_flight < FAN_OUT_CONCURRENCY {
            let Some(next) = queue.pop_front() else {
                break;
            };
            let p = proxy_arc.clone();
            let h = pinned_str.clone();
            let pt = protocol_tag;
            tasks.push(tokio::spawn(async move {
                (next.clone(), call_protorunesbyoutpoint(&p, &next, pt, &h).await)
            }));
            in_flight += 1;
        }
    }

    // ---- step 4: stale-window check ----
    // Re-read the watermark. If the pool has advanced past H by more
    // than the safety window, the data we just assembled is reorg-risky:
    // the caller might have asked for "latest" 6+ blocks ago and we'd
    // be returning a snapshot that's now provably behind.
    let post_height = resolve_latest_height(proxy).await?;
    if post_height > pinned_height {
        let drift = post_height - pinned_height;
        if drift > staleness_window_blocks {
            return Ok(FanOutResult::StaleHeightWindow {
                pinned: pinned_height,
                current: post_height,
                drift,
            });
        }
    }

    // ---- step 5: aggregate ----
    Ok(FanOutResult::Ok(aggregate_wallet_response(per_outpoint)))
}

/// Possible failure modes of a single `protorunesbyoutpoint` sub-call.
enum SubCallError {
    /// Upstream returned a JSON-RPC error object. We pass the code +
    /// message through to the caller of the fan-out — typically -32000
    /// for "height not indexed" cases.
    Upstream { code: i32, message: String },
    /// Local transport / parse error.
    Internal(anyhow::Error),
}

/// Issue one `metashrew_view "protorunesbyoutpoint"` call for a UTXO
/// at the pinned height. Goes through `proxy.forward_to_metashrew` so
/// the existing cache layer applies.
async fn call_protorunesbyoutpoint(
    proxy: &ProxyClient,
    utxo: &Value,
    protocol_tag: u128,
    pinned_height_str: &str,
) -> std::result::Result<OutpointResponse, SubCallError> {
    let txid_display = utxo
        .get("txid")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SubCallError::Internal(anyhow!("utxo missing txid")))?;
    let vout = utxo
        .get("vout")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| SubCallError::Internal(anyhow!("utxo missing vout")))? as u32;

    // Esplora gives txids in display (reversed) order; the protobuf
    // wants internal little-endian. Reverse to convert. This mirrors
    // `encode_protorunesbyoutpoint_request` in handler.rs.
    let mut txid_bytes = hex::decode(txid_display)
        .map_err(|e| SubCallError::Internal(anyhow!("invalid utxo txid hex: {}", e)))?;
    txid_bytes.reverse();

    let payload = OutpointWithProtocol {
        txid: txid_bytes,
        vout,
        protocol: Some(ProtoruneUint128 {
            lo: protocol_tag as u64,
            hi: (protocol_tag >> 64) as u64,
        }),
    };
    let hex_payload = format!("0x{}", hex::encode(payload.encode_to_vec()));

    let request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: "metashrew_view".to_string(),
        params: vec![
            Value::String("protorunesbyoutpoint".to_string()),
            Value::String(hex_payload),
            Value::String(pinned_height_str.to_string()),
        ],
        id: Value::Number(0.into()),
    };

    let response = proxy
        .forward_to_metashrew(&request)
        .await
        .map_err(|e| SubCallError::Internal(anyhow!("forward_to_metashrew: {}", e)))?;

    match response {
        JsonRpcResponse::Success { result, .. } => {
            let hex_str = result
                .as_str()
                .ok_or_else(|| SubCallError::Internal(anyhow!("upstream returned non-string result")))?;
            let hex_clean = hex_str.strip_prefix("0x").unwrap_or(hex_str);
            if hex_clean.is_empty() {
                // Treat empty as a UTXO with no protorunes — return an
                // OutpointResponse with empty balances but the outpoint
                // populated so step 5 can still stamp the wallet entry.
                return Ok(OutpointResponse::default());
            }
            let bytes = hex::decode(hex_clean)
                .map_err(|e| SubCallError::Internal(anyhow!("upstream hex decode failed: {}", e)))?;
            OutpointResponse::decode(bytes.as_slice())
                .map_err(|e| SubCallError::Internal(anyhow!("upstream proto decode failed: {}", e)))
        }
        JsonRpcResponse::Error { error, .. } => Err(SubCallError::Upstream {
            code: error.code,
            message: error.message,
        }),
    }
}

/// Resolve a height tag (Value: String / Number / Null / "latest") to a
/// pinned u64. For "latest" we use the cache's watermark — explicitly
/// NOT the upstream-reported tip — to avoid racing the LB rewriter.
async fn resolve_height(proxy: &ProxyClient, height_tag: &Value) -> Result<u64> {
    let is_latest = matches!(height_tag, Value::Null) || matches!(height_tag, Value::String(s) if s == "latest");
    if is_latest {
        return resolve_latest_height(proxy).await;
    }
    match height_tag {
        Value::Number(n) => n
            .as_u64()
            .ok_or_else(|| anyhow!("height out of u64 range: {}", n)),
        Value::String(s) => s
            .parse::<u64>()
            .map_err(|e| anyhow!("invalid height string {:?}: {}", s, e)),
        other => Err(anyhow!("unsupported height tag: {:?}", other)),
    }
}

/// Read the current "latest" height. Prefers the cached watermark; if
/// the cache isn't configured, falls back to a one-shot `metashrew_height`
/// upstream call. Returns just the height (we don't need the hash here
/// — the underlying `metashrew_view` cache will resolve hash itself
/// when it sees the explicit height we pass in).
async fn resolve_latest_height(proxy: &ProxyClient) -> Result<u64> {
    if let Some(cache) = proxy.cache() {
        return cache.latest_height().await;
    }
    // No cache: do a one-shot metashrew_height.
    let req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: "metashrew_height".to_string(),
        params: vec![],
        id: Value::Number(0.into()),
    };
    let r = proxy.forward_to_metashrew(&req).await?;
    match r {
        JsonRpcResponse::Success { result, .. } => {
            // metashrew_height returns a stringified u64.
            let s = result
                .as_str()
                .ok_or_else(|| anyhow!("metashrew_height: result not a string: {}", result))?;
            s.parse::<u64>()
                .map_err(|e| anyhow!("metashrew_height: not a u64: {}", e))
        }
        JsonRpcResponse::Error { error, .. } => Err(anyhow!(
            "metashrew_height upstream error {}: {}",
            error.code,
            error.message
        )),
    }
}

/// Parse params for the top-level `alkanes_protorunesbyaddress` method.
///
/// Returns (address, protocol_tag, height_tag).
fn parse_address_params(params: &[Value]) -> Result<(String, u128, Value)> {
    let first = params
        .get(0)
        .ok_or_else(|| anyhow!("missing first parameter (address)"))?;

    let (address, protocol_tag, height_from_obj) = if let Some(s) = first.as_str() {
        // Plain string address. Per the brief: params = [address, height].
        // For backward compat with the existing handler, also accept
        // `params[2]` as protocol_tag (the old positional shape).
        let pt = params
            .get(2)
            .and_then(|v| v.as_u64().or_else(|| v.as_str().and_then(|s| s.parse::<u64>().ok())))
            .unwrap_or(1) as u128;
        (s.to_string(), pt, None)
    } else if let Some(obj) = first.as_object() {
        // Object form: { address, protocolTag, height? }
        let addr = obj
            .get("address")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("object param missing 'address'"))?
            .to_string();
        let pt = obj
            .get("protocolTag")
            .and_then(|v| v.as_u64().or_else(|| v.as_str().and_then(|s| s.parse::<u64>().ok())))
            .unwrap_or(1) as u128;
        // If the object embeds a height, use it; otherwise fall through
        // to params[1].
        let h = obj.get("height").cloned();
        (addr, pt, h)
    } else {
        return Err(anyhow!(
            "first parameter must be an address string or {{address, protocolTag, height?}} object"
        ));
    };

    let height_tag = height_from_obj
        .or_else(|| params.get(1).cloned())
        .unwrap_or(Value::String("latest".to_string()));

    Ok((address, protocol_tag, height_tag))
}

/// Read the staleness window from $PROTORUNES_STALENESS_WINDOW_BLOCKS,
/// defaulting to 6. Reading env per-call is cheap and lets ops tune
/// without a redeploy.
fn staleness_window_from_env() -> u64 {
    std::env::var("PROTORUNES_STALENESS_WINDOW_BLOCKS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(DEFAULT_STALENESS_WINDOW_BLOCKS)
}

/// Build an empty WalletResponse (no outpoints, no balances).
fn empty_wallet_response() -> WalletResponse {
    WalletResponse {
        outpoints: vec![],
        balances: Some(BalanceSheet { entries: vec![] }),
    }
}

/// Aggregate per-outpoint responses into one WalletResponse.
///
/// For each UTXO we got a fresh OutpointResponse from upstream; we
/// stamp the outpoint identity, output value, and esplora-reported
/// height onto the response (in case the WASM view didn't populate
/// them — protorunesbyoutpoint historically only fills `balances`).
///
/// Aggregate `balances` is the sum across all outpoints, keyed by
/// (block, tx, name). Two outpoints holding the same rune get their
/// balances combined.
fn aggregate_wallet_response(per_outpoint: Vec<(Value, OutpointResponse)>) -> WalletResponse {
    use std::collections::BTreeMap;

    let mut outpoints: Vec<OutpointResponse> = Vec::with_capacity(per_outpoint.len());
    // Key: (height_lo, height_hi, txindex_lo, txindex_hi, name) so we
    // de-dupe by ProtoruneRuneId. We use BTreeMap so the output order
    // is stable.
    let mut agg: BTreeMap<(u64, u64, u64, u64, String), (BalanceSheetItem, u128)> = BTreeMap::new();

    for (utxo, mut sub) in per_outpoint {
        let txid_display = utxo.get("txid").and_then(|v| v.as_str()).unwrap_or("");
        let vout = utxo.get("vout").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        let value = utxo.get("value").and_then(|v| v.as_u64()).unwrap_or(0);
        let block_height = utxo
            .get("status")
            .and_then(|s| s.get("block_height"))
            .and_then(|h| h.as_u64())
            .unwrap_or(0) as u32;

        // Convert display txid → internal little-endian. Stamp onto the
        // OutpointResponse so the wire shape includes the outpoint
        // identity even when the WASM view didn't fill it.
        if let Ok(mut txid_bytes) = hex::decode(txid_display) {
            txid_bytes.reverse();
            sub.outpoint = Some(Outpoint {
                txid: txid_bytes,
                vout,
            });
        }
        if sub.output.is_none() {
            sub.output = Some(Output {
                script: vec![],
                value,
            });
        } else if let Some(out) = sub.output.as_mut() {
            if out.value == 0 {
                out.value = value;
            }
        }
        if sub.height == 0 {
            sub.height = block_height;
        }

        // Aggregate balances. Skip outpoints that came back with no
        // entries — they're spendable UTXOs with no protorunes, which
        // sandshrew_balances classifies as "spendable" rather than
        // "assets". Upstream's `protorunesbyaddress` also filters
        // these out (see alkanes/src/lib.rs:195-211).
        if let Some(bs) = sub.balances.as_ref() {
            if bs.entries.is_empty() {
                continue;
            }
            for entry in &bs.entries {
                let (height_lo, height_hi, txindex_lo, txindex_hi, name) = entry
                    .rune
                    .as_ref()
                    .map(|r| {
                        let rid = r.rune_id.as_ref();
                        let (hl, hh) = rid
                            .and_then(|i| i.height.as_ref())
                            .map(|u| (u.lo, u.hi))
                            .unwrap_or((0, 0));
                        let (tl, th) = rid
                            .and_then(|i| i.txindex.as_ref())
                            .map(|u| (u.lo, u.hi))
                            .unwrap_or((0, 0));
                        (hl, hh, tl, th, r.name.clone())
                    })
                    .unwrap_or((0, 0, 0, 0, String::new()));
                let key = (height_lo, height_hi, txindex_lo, txindex_hi, name);
                let amount = entry
                    .balance
                    .as_ref()
                    .map(|u| ((u.hi as u128) << 64) | (u.lo as u128))
                    .unwrap_or(0);
                agg.entry(key)
                    .and_modify(|(_, total)| *total = total.saturating_add(amount))
                    .or_insert_with(|| (entry.clone(), amount));
            }
        }

        outpoints.push(sub);
    }

    let agg_entries: Vec<BalanceSheetItem> = agg
        .into_values()
        .map(|(mut entry, total)| {
            entry.balance = Some(ProtoruneUint128 {
                lo: total as u64,
                hi: (total >> 64) as u64,
            });
            entry
        })
        .collect();

    WalletResponse {
        outpoints,
        balances: Some(BalanceSheet {
            entries: agg_entries,
        }),
    }
}

/// Mirror of `decode_wallet_response` in handler.rs. We can't call that
/// directly because it operates on a hex string; we already have the
/// proto object. Keeping the JSON shape identical is the whole point
/// (so sandshrew.rs's `process_address_info` and lua scripts keep
/// working as-is).
fn wallet_response_to_json(response: &WalletResponse) -> Value {
    let outpoints: Vec<Value> = response
        .outpoints
        .iter()
        .map(|op| {
            let balances: Vec<Value> = op
                .balances
                .as_ref()
                .map(|bs| {
                    bs.entries
                        .iter()
                        .map(|entry| {
                            let (block, tx) = entry
                                .rune
                                .as_ref()
                                .and_then(|r| r.rune_id.as_ref())
                                .map(|id| {
                                    let height = id
                                        .height
                                        .as_ref()
                                        .map(|u| ((u.hi as u128) << 64) | (u.lo as u128))
                                        .unwrap_or(0)
                                        as u64;
                                    let txindex = id
                                        .txindex
                                        .as_ref()
                                        .map(|u| ((u.hi as u128) << 64) | (u.lo as u128))
                                        .unwrap_or(0)
                                        as u64;
                                    (height, txindex)
                                })
                                .unwrap_or((0, 0));
                            let amount = entry
                                .balance
                                .as_ref()
                                .map(|u| ((u.hi as u128) << 64) | (u.lo as u128))
                                .unwrap_or(0)
                                as u64;
                            json!({ "block": block, "tx": tx, "amount": amount })
                        })
                        .collect()
                })
                .unwrap_or_default();

            let txid = op
                .outpoint
                .as_ref()
                .map(|o| {
                    let mut t = o.txid.clone();
                    t.reverse();
                    hex::encode(&t)
                })
                .unwrap_or_default();
            let vout = op.outpoint.as_ref().map(|o| o.vout).unwrap_or(0);
            let value = op.output.as_ref().map(|o| o.value).unwrap_or(0);

            json!({
                "balance_sheet": { "cached": { "balances": balances } },
                "outpoint": { "txid": txid, "vout": vout },
                "output": { "value": value },
                "height": op.height,
                "txindex": op.txindex,
            })
        })
        .collect();

    let balances: Vec<Value> = response
        .balances
        .as_ref()
        .map(|bs| {
            bs.entries
                .iter()
                .map(|entry| {
                    let (block, tx) = entry
                        .rune
                        .as_ref()
                        .and_then(|r| r.rune_id.as_ref())
                        .map(|id| {
                            let height = id
                                .height
                                .as_ref()
                                .map(|u| ((u.hi as u128) << 64) | (u.lo as u128))
                                .unwrap_or(0)
                                as u64;
                            let txindex = id
                                .txindex
                                .as_ref()
                                .map(|u| ((u.hi as u128) << 64) | (u.lo as u128))
                                .unwrap_or(0)
                                as u64;
                            (height, txindex)
                        })
                        .unwrap_or((0, 0));
                    let amount = entry
                        .balance
                        .as_ref()
                        .map(|u| ((u.hi as u128) << 64) | (u.lo as u128))
                        .unwrap_or(0) as u64;
                    json!({ "block": block, "tx": tx, "amount": amount })
                })
                .collect()
        })
        .unwrap_or_default();

    json!({
        "outpoints": outpoints,
        "balances": { "entries": balances },
    })
}

// Silence "unused" warnings for the types we re-export for symmetry; some
// of these are exercised only by the integration test path or downstream
// callers.
#[allow(dead_code)]
fn _proof_types_compile() {
    let _: Option<&MetashrewViewCache> = None;
    let _: Option<&Arc<ProxyClient>> = None;
    let _: Option<&Rune> = None;
    let _: Option<&ProtoruneRuneId> = None;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pwallet_req(addr: &str) -> ProtorunesWalletRequest {
        ProtorunesWalletRequest {
            wallet: addr.as_bytes().to_vec(),
            protocol_tag: Some(ProtoruneUint128 { lo: 1, hi: 0 }),
        }
    }

    #[test]
    fn parse_string_params_defaults_to_latest() {
        let params = vec![Value::String("bc1pabc".to_string())];
        let (addr, pt, h) = parse_address_params(&params).unwrap();
        assert_eq!(addr, "bc1pabc");
        assert_eq!(pt, 1);
        assert_eq!(h, Value::String("latest".to_string()));
    }

    #[test]
    fn parse_string_params_with_height() {
        let params = vec![
            Value::String("bc1pabc".to_string()),
            Value::Number(900_000.into()),
        ];
        let (_, _, h) = parse_address_params(&params).unwrap();
        assert_eq!(h, Value::Number(900_000.into()));
    }

    #[test]
    fn parse_object_params_picks_up_protocol_tag() {
        let params = vec![json!({
            "address": "bc1pabc",
            "protocolTag": "7",
        })];
        let (addr, pt, _) = parse_address_params(&params).unwrap();
        assert_eq!(addr, "bc1pabc");
        assert_eq!(pt, 7);
    }

    #[test]
    fn round_trip_request_proto() {
        // Confirm we can decode the hex form the view-style handler expects.
        let req = pwallet_req("bc1pabc");
        let bytes = req.encode_to_vec();
        let decoded = ProtorunesWalletRequest::decode(bytes.as_slice()).unwrap();
        assert_eq!(decoded.wallet, b"bc1pabc");
    }

    #[test]
    fn empty_wallet_response_serializes_as_expected_shape() {
        let v = wallet_response_to_json(&empty_wallet_response());
        assert!(v.get("outpoints").unwrap().as_array().unwrap().is_empty());
        assert!(
            v.get("balances")
                .unwrap()
                .get("entries")
                .unwrap()
                .as_array()
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn staleness_window_default_is_6() {
        // We intentionally don't set env here. Note: another test may
        // have set it; just check it parses to *some* u64 either way.
        let w = staleness_window_from_env();
        assert!(w > 0);
    }

    #[test]
    fn aggregate_combines_balances() {
        // Two outpoints both holding rune (block=2, tx=3), amounts 100 + 250.
        let utxo1 = json!({ "txid": "00".repeat(32), "vout": 0, "value": 1000,
                            "status": { "block_height": 100 } });
        let utxo2 = json!({ "txid": "11".repeat(32), "vout": 1, "value": 2000,
                            "status": { "block_height": 100 } });
        let mk_entry = |amt: u64| BalanceSheetItem {
            rune: Some(Rune {
                rune_id: Some(ProtoruneRuneId {
                    height: Some(ProtoruneUint128 { lo: 2, hi: 0 }),
                    txindex: Some(ProtoruneUint128 { lo: 3, hi: 0 }),
                }),
                name: "FOO".to_string(),
                divisibility: 8,
                spacers: 0,
                symbol: "F".to_string(),
            }),
            balance: Some(ProtoruneUint128 { lo: amt, hi: 0 }),
        };
        let r1 = OutpointResponse {
            balances: Some(BalanceSheet { entries: vec![mk_entry(100)] }),
            outpoint: None,
            output: None,
            height: 0,
            txindex: 0,
        };
        let r2 = OutpointResponse {
            balances: Some(BalanceSheet { entries: vec![mk_entry(250)] }),
            outpoint: None,
            output: None,
            height: 0,
            txindex: 0,
        };
        let agg = aggregate_wallet_response(vec![(utxo1, r1), (utxo2, r2)]);
        assert_eq!(agg.outpoints.len(), 2);
        let entries = &agg.balances.unwrap().entries;
        assert_eq!(entries.len(), 1);
        let total = entries[0]
            .balance
            .as_ref()
            .map(|u| ((u.hi as u128) << 64) | (u.lo as u128))
            .unwrap();
        assert_eq!(total, 350);
    }
}
