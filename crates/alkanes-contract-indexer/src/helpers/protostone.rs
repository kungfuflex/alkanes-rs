use anyhow::{Context, Result};
use bitcoin::consensus::encode::deserialize;
use bitcoin::Transaction;
use deezel_common::runestone_enhanced::format_runestone_with_decoded_messages;
use deezel_common::traits::{DeezelProvider, JsonRpcProvider, BitcoinRpcProvider, EsploraProvider};
use serde_json::{json, Value as JsonValue};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{sleep, timeout, Duration};
use futures::stream::{self, StreamExt};
use tracing::{debug, error, info, warn};
use crate::helpers::rpc::{resilient_call, resilient_call_with_last_error};
use std::panic::{catch_unwind, AssertUnwindSafe};

use crate::helpers::block::tx_has_op_return;

// Hardcoded list of txids to skip during trace/decode. These are big-endian txid strings.
// If a txid appears here, it will be excluded from processing to avoid blocking a whole block.
static IGNORED_TRACE_TXIDS: &[&str] = &[
    "a807e8d4e91a6fa957c3f9929d267f6795971e41e6da61c44886deaa45797830",
    "33c5a9f2d415b2b826a2ea1230d1849be0a74dc73857460c9c7674fe76147830",
    "79c202e94c425320c91d6176108b93c033fb5a627fc2750453e97c6c434e7830",
    "12a6c6f41a722e75d48caf57ed7a22feb56686c2ba51e226e6b6033ef3357830",
    "6ee9eda5df0814af442f75db1d553f951f04699b854b9e0cff6d1395c2bdf075",
    "c4c00e467ec76aa228a737156488b74dc27a998cf3056655612dbd3eeb5e6fb0",
    "9d258d9e805ca9252101d5839aee46d63fbda8786e3f80988f5b10ce35aa060e",
    "83d0deb1d223c932e0ff4306c0f408d17dbd520bd1dfb9e8d5823b711be77830",
    "b19dd4c02942b0c2f19c5f11e9e4b1211051a779ff4bb8e84b02f37a2f415f6d"
];

#[derive(Debug, Clone)]
struct TraceJob {
    txid_le_hex: String,
    vout: u32,
    #[allow(dead_code)]
    protostone_idx: usize,
}

fn to_little_endian_hex(txid_be_hex: &str) -> String {
    match hex::decode(txid_be_hex) {
        Ok(mut b) => {
            b.reverse();
            hex::encode(b)
        }
        Err(_) => txid_be_hex.to_string(),
    }
}

async fn trace_call<P: DeezelProvider + JsonRpcProvider + Send + Sync>(
    provider: &P,
    url: &str,
    job: TraceJob,
) -> Result<JsonValue> {
    let req = json!([{ "txid": job.txid_le_hex, "vout": job.vout }]);
    // Use variant that preserves the last error string, so callers can match specific upstream failures
    let res = resilient_call_with_last_error(provider, url, "alkanes_trace", req, 1)
        .await
        .context("alkanes_trace call failed")?;
    Ok(res)
}

async fn tx_from_json_or_fetch_hex<P: DeezelProvider + JsonRpcProvider + BitcoinRpcProvider + EsploraProvider + Send + Sync>(
    provider: &P,
    tx_json: &JsonValue,
) -> Result<Transaction> {
    // Prefer embedded hex if present; fallback to JSON-RPC "esplora_tx::hex"
    if let Some(hex_str) = tx_json.get("hex").and_then(|v| v.as_str()) {
        let raw = hex::decode(hex_str).context("failed to decode tx hex")?;
        let tx: Transaction = deserialize(&raw).context("failed to deserialize tx")?;
        return Ok(tx);
    }

    let txid = tx_json
        .get("txid")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("txid missing in tx json"))?;
    // First try EsploraProvider::get_tx_hex (works with native-deps or JSON-RPC proxy), then fall back to bitcoind getrawtransaction
    info!(%txid, "fetching tx hex via EsploraProvider::get_tx_hex");
    let mut _last_err: Option<anyhow::Error> = None;
    let hex_str = {
        let mut attempt = 0;
        loop {
            attempt += 1;
            let fut = provider.get_tx_hex(txid);
            match timeout(Duration::from_secs(20), fut).await {
                Ok(Ok(h)) => break h,
                Ok(Err(e)) => {
                    _last_err = Some(anyhow::anyhow!(e));
                    warn!(%txid, attempt, "get_tx_hex error; will retry or fall back");
                }
                Err(_elapsed) => {
                    _last_err = Some(anyhow::anyhow!("timeout"));
                    warn!(%txid, attempt, "get_tx_hex timed out; will retry or fall back");
                }
            }
            if attempt >= 2 { break String::new(); }
            sleep(Duration::from_millis(200 * attempt as u64)).await;
        }
    };
    let hex_str = if !hex_str.is_empty() { hex_str } else {
        info!(%txid, "falling back to BitcoinRpcProvider::get_transaction_hex");
        let mut attempt = 0;
        loop {
            attempt += 1;
            let fut = provider.get_transaction_hex(txid);
            match timeout(Duration::from_secs(20), fut).await {
                Ok(Ok(h)) => break h,
                Ok(Err(e)) => {
                    _last_err = Some(anyhow::anyhow!(e));
                    warn!(%txid, attempt, "get_transaction_hex error; will retry");
                }
                Err(_elapsed) => {
                    _last_err = Some(anyhow::anyhow!("timeout"));
                    warn!(%txid, attempt, "get_transaction_hex timed out; will retry");
                }
            }
            if attempt >= 3 {
                return Err(_last_err.unwrap_or_else(|| anyhow::anyhow!("get_transaction_hex failed"))).context("get_transaction_hex call failed");
            }
            sleep(Duration::from_millis(200 * attempt as u64)).await;
        }
    };
    let raw = hex::decode(hex_str).context("failed to decode tx hex")?;
    let tx: Transaction = deserialize(&raw).context("failed to deserialize tx")?;
    debug!(%txid, size = raw.len(), "decoded tx hex");
    Ok(tx)
}

fn resolve_sandshrew_url<P: JsonRpcProvider + DeezelProvider>(provider: &P) -> String {
    std::env::var("SANDSHREW_RPC_URL")
        .ok()
        .or_else(|| provider.get_bitcoin_rpc_url())
        .unwrap_or_else(|| "http://localhost:18888".to_string())
}

#[derive(Debug, Clone)]
pub struct DecodedProtostoneItem {
    pub vout: i32,
    pub protostone_index: i32,
    pub decoded: JsonValue,
}

#[derive(Debug, Clone)]
pub struct TraceEventItem {
    pub vout: i32,
    pub event_type: String,
    pub data: JsonValue,
    pub alkane_address_block: String,
    pub alkane_address_tx: String,
}

#[derive(Debug, Clone)]
pub struct TxDecodeTraceResult {
    pub transaction_id: String,
    pub transaction_json: JsonValue,
    pub decoded_protostones: Vec<DecodedProtostoneItem>,
    pub trace_events: Vec<TraceEventItem>,
    pub has_trace: bool,
    pub trace_succeed: bool,
}

/// Decode runestones for OP_RETURN txs, call trace RPC, and return structured results.
pub async fn decode_and_trace_for_block<P>(
    provider: &P,
    txs: &[JsonValue],
    _max_decode_in_flight: usize,
    _max_trace_concurrency: usize,
) -> Result<Vec<TxDecodeTraceResult>>
where
    P: DeezelProvider + JsonRpcProvider + BitcoinRpcProvider + EsploraProvider + Send + Sync,
{
    let url = resolve_sandshrew_url(provider);
    info!(txs = txs.len(), "decode_and_trace_for_block: start (batched parallel)");
    // Only OP_RETURN txs
    let op_return_txs: Vec<JsonValue> = txs.iter().filter(|t| tx_has_op_return(t)).cloned().collect();
    // Skip txids explicitly ignored
    let op_return_txs: Vec<JsonValue> = op_return_txs
        .into_iter()
        .filter(|t| {
            let id = t.get("txid").and_then(|v| v.as_str()).unwrap_or("");
            let skip = IGNORED_TRACE_TXIDS.contains(&id);
            if skip { info!(%id, "skipping txid from ignore list"); }
            !skip
        })
        .collect();
    let total = op_return_txs.len();
    info!(op_return_txs = total, "filtered OP_RETURN transactions");
    if total == 0 { return Ok(Vec::new()); }

    // Split into up to 10 batches and process each batch concurrently.
    let num_batches = usize::min(10, total);
    let batch_size = (total + num_batches - 1) / num_batches; // ceildiv
    let batches: Vec<Vec<JsonValue>> = op_return_txs
        .chunks(batch_size)
        .map(|c| c.to_vec())
        .collect();

    let results: Arc<Mutex<Vec<TxDecodeTraceResult>>> = Arc::new(Mutex::new(Vec::new()));
    let fatal_err: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

    stream::iter(batches.into_iter().enumerate())
        .for_each_concurrent(num_batches, |(batch_idx, batch)| {
            let url = url.clone();
            let results = results.clone();
            let fatal_err = fatal_err.clone();
            async move {
            info!(batch = batch_idx, size = batch.len(), "batch start");
            for (local_idx, tx_json) in batch.into_iter().enumerate() {
                // If a fatal error has been recorded, stop further work in this task
                if fatal_err.lock().await.is_some() { return; }
                let txid_str = tx_json.get("txid").and_then(|v| v.as_str()).unwrap_or("<no-txid>");
                info!(batch = batch_idx, index = local_idx, %txid_str, "fetching tx hex");
                let tx = match tx_from_json_or_fetch_hex(provider, &tx_json).await {
                    Ok(t) => t,
                    Err(e) => {
                        error!(batch = batch_idx, %txid_str, error = %e, "failed to materialize tx; aborting block batch");
                        // Record fatal error to fail the block rather than silently skipping this tx
                        *fatal_err.lock().await = Some(format!("materialize_tx failed for {}: {}", txid_str, e));
                        return;
                    }
                };
                info!(batch = batch_idx, index = local_idx, %txid_str, outputs = tx.output.len(), "tx ready; decoding runestone");
                let decode_attempt = catch_unwind(AssertUnwindSafe(|| format_runestone_with_decoded_messages(&tx)));
                match decode_attempt {
                    Ok(Ok(formatted)) => {
                        let txid_be = formatted.get("transaction_id").and_then(|v| v.as_str()).map(|s| s.to_string()).unwrap_or_else(|| tx.compute_txid().to_string());
                        let txid_le = to_little_endian_hex(&txid_be);
                        let start = (tx.output.len() as u32) + 1;
                        let protos = formatted.get("protostones").and_then(|v| v.as_array()).cloned().unwrap_or_default();
                        info!(batch = batch_idx, %txid_be, protostones = protos.len(), start_vout = start, "decoded runestone");
                        let mut decoded_items: Vec<DecodedProtostoneItem> = Vec::with_capacity(protos.len());
                        let mut trace_events: Vec<TraceEventItem> = Vec::new();
                        let mut has_trace = false;
                        let mut trace_succeed = false;

                        for (i, p) in protos.iter().enumerate() {
                            let vout = start + i as u32;
                            info!(batch = batch_idx, %txid_be, protostone_idx = i, vout, "calling trace");
                            let job = TraceJob { txid_le_hex: txid_le.clone(), vout, protostone_idx: i };
                            debug!(batch = batch_idx, %txid_be, protostone_idx = i, "dispatching trace job");
                            decoded_items.push(DecodedProtostoneItem { vout: vout as i32, protostone_index: i as i32, decoded: p.clone() });
                            match trace_call(provider, &url, job).await {
                                Ok(res) => {
                                    info!(batch = batch_idx, %txid_be, protostone_idx = i, vout, "trace ok");
                                    debug!(result = %res);
                                    has_trace = true;
                                    // Determine success from either structured trace or raw events
                                    let mut ok_status = false;
                                    if let Ok(trace_parsed) = serde_json::from_value::<deezel_common::alkanes::trace::Trace>(res.clone()) {
                                        if let Some(first_call) = trace_parsed.calls.first() {
                                            for ev in &first_call.events {
                                                if let deezel_common::alkanes::trace::Event::Exit(exit) = ev {
                                                    let s = exit.status.to_ascii_lowercase();
                                                    if s.contains("ok") || s.contains("success") { ok_status = true; }
                                                }
                                            }
                                        }
                                    } else if let Some(arr) = res.as_array() {
                                        for ev in arr {
                                            let typ = ev.get("event").and_then(|v| v.as_str()).unwrap_or("");
                                            if typ == "return" {
                                                let st = ev.get("data").and_then(|d| d.get("status")).and_then(|s| s.as_str()).unwrap_or("").to_ascii_lowercase();
                                                if st.contains("ok") || st.contains("success") { ok_status = true; }
                                            }
                                        }
                                    }
                                    if ok_status { trace_succeed = true; }

                                    // Flatten trace result into individual events for storage/indexing
                                    if let Some(arr) = res.as_array() {
                                        for ev in arr {
                                            let event_type = ev.get("event").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
                                            let data = ev.get("data").cloned().unwrap_or_else(|| serde_json::json!({}));
                                            // Extract alkane address from invoke context; keep empty for others
                                            let (blk_str, tx_str) = if event_type == "invoke" {
                                                let blk_hex = data.get("context").and_then(|c| c.get("myself")).and_then(|m| m.get("block")).and_then(|v| v.as_str()).unwrap_or("");
                                                let tx_hex  = data.get("context").and_then(|c| c.get("myself")).and_then(|m| m.get("tx")).and_then(|v| v.as_str()).unwrap_or("");
                                                let blk = if blk_hex.is_empty() { String::new() } else { crate::helpers::poolswap::hex_to_dec_u128_str(blk_hex).unwrap_or_else(|_| blk_hex.to_string()) };
                                                let tx  = if tx_hex.is_empty()  { String::new() } else { crate::helpers::poolswap::hex_to_dec_u128_str(tx_hex).unwrap_or_else(|_| tx_hex.to_string()) };
                                                (blk, tx)
                                            } else { (String::new(), String::new()) };
                                            trace_events.push(TraceEventItem {
                                                vout: vout as i32,
                                                event_type,
                                                data,
                                                alkane_address_block: blk_str,
                                                alkane_address_tx: tx_str,
                                            });
                                        }
                                    } else {
                                        // Fallback: store whole trace if shape is unexpected
                                        trace_events.push(TraceEventItem {
                                            vout: vout as i32,
                                            event_type: "trace".to_string(),
                                            data: res,
                                            alkane_address_block: String::new(),
                                            alkane_address_tx: String::new(),
                                        });
                                    }
                                }
                                Err(e) => {
                                    // Build a combined error string including all causes in the chain
                                    let mut combined = String::new();
                                    combined.push_str(&e.to_string());
                                    for cause in e.chain().skip(1) { // skip the top-level to avoid duplication
                                        combined.push_str(" | ");
                                        combined.push_str(&cause.to_string());
                                    }
                                    let lc = combined.to_ascii_lowercase();
                                    // Known upstream non-deterministic client error from alkanes base-rpc addHexPrefix
                                    // Example contains: "non-standard error object received" and "cannot read properties of undefined (reading 'substr')"
                                    let is_known_upstream_typeerror = lc.contains("non-standard error object received") && lc.contains("cannot read properties of undefined");
                                    if is_known_upstream_typeerror {
                                        warn!(batch = batch_idx, %txid_be, protostone_idx = i, vout, error = %combined, "trace returned upstream TypeError; skipping this protostone");
                                        // Do not set has_trace; continue with next protostone/tx
                                        continue;
                                    }
                                    error!(batch = batch_idx, %txid_be, protostone_idx = i, vout, error = ?e, "trace failed; aborting block batch");
                                    // Record fatal error to fail the block rather than proceeding with partial results
                                    *fatal_err.lock().await = Some(format!("trace failed for {} vout {}: {}", txid_be, vout, combined));
                                    return;
                                }
                            }
                        }
                        let result = TxDecodeTraceResult {
                            transaction_id: txid_be,
                            transaction_json: tx_json.clone(),
                            decoded_protostones: decoded_items,
                            trace_events,
                            has_trace,
                            trace_succeed,
                        };
                        results.lock().await.push(result);
                    }
                    Ok(Err(e)) => { warn!(batch = batch_idx, %txid_str, error = %e, "protostone decode failed; skipping tx"); }
                    Err(panic) => {
                        let panic_msg: &str = if let Some(s) = panic.downcast_ref::<&str>() {
                            s
                        } else if let Some(s) = panic.downcast_ref::<String>() {
                            s.as_str()
                        } else {
                            "panic"
                        };
                        error!(batch = batch_idx, %txid_str, message = %panic_msg, "protostone decode panicked; skipping tx");
                    }
                }
            }
            info!(batch = batch_idx, "batch complete");
            }
        })
        .await;

    info!("decode_and_trace_for_block: complete (batched parallel)");

    if let Some(err) = fatal_err.lock().await.clone() {
        return Err(anyhow::anyhow!(err));
    }

    let out = results.lock().await.clone();
    Ok(out)
}


