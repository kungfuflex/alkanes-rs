use alkanes_cli_common::proto::alkanes as alkanes_pb;
use alkanes_cli_common::runestone_enhanced::format_runestone_with_decoded_messages;
use alkanes_cli_common::traits::{
    AlkanesProvider, BitcoinRpcProvider, DeezelProvider, EsploraProvider, JsonRpcProvider,
};
use anyhow::{Context, Result, bail};
use bitcoin::Transaction;
use bitcoin::consensus::encode::deserialize;
use futures::stream::{self, StreamExt};
use serde_json::{Value as JsonValue, json};
use std::collections::HashMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{Duration, sleep, timeout};
use tracing::{debug, error, info, warn};

use crate::helpers::block::tx_has_op_return;

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

/// Helper to convert Uint128 to u128
fn uint128_to_u128(u: &alkanes_pb::Uint128) -> u128 {
    ((u.hi as u128) << 64) | (u.lo as u128)
}

/// Helper to convert AlkaneId to (block_str, tx_str) without normalizing malformed protobufs.
fn alkane_id_to_strings(id: &alkanes_pb::AlkaneId, label: &str) -> Result<(String, String)> {
    let block = id
        .block
        .as_ref()
        .with_context(|| format!("{label}.block is missing"))?;
    let tx = id
        .tx
        .as_ref()
        .with_context(|| format!("{label}.tx is missing"))?;
    Ok((
        uint128_to_u128(block).to_string(),
        uint128_to_u128(tx).to_string(),
    ))
}

fn alkane_transfer_to_json(
    transfer: &alkanes_pb::AlkaneTransfer,
    label: &str,
) -> Result<JsonValue> {
    let id = transfer
        .id
        .as_ref()
        .with_context(|| format!("{label}.id is missing"))?;
    let value = transfer
        .value
        .as_ref()
        .with_context(|| format!("{label}.value is missing"))?;
    let (block, tx) = alkane_id_to_strings(id, &format!("{label}.id"))?;
    Ok(json!({
        "id": {
            "block": block,
            "tx": tx,
        },
        "value": uint128_to_u128(value).to_string(),
    }))
}

/// Convert AlkanesTrace events to JSON format compatible with existing indexers
fn convert_trace_to_events(trace: &alkanes_pb::AlkanesTrace, vout: i32) -> Result<Vec<JsonValue>> {
    let mut events = Vec::new();

    for (event_index, event) in trace.events.iter().enumerate() {
        let ev = event
            .event
            .as_ref()
            .with_context(|| format!("trace event {event_index} has no event payload"))?;
        match ev {
            alkanes_pb::alkanes_trace_event::Event::EnterContext(enter) => {
                let call_type = match enter.call_type {
                    0 => "none",
                    1 => "call",
                    2 => "delegatecall",
                    3 => "staticcall",
                    value => bail!("trace event {event_index} has unknown call type {value}"),
                };
                let ctx = enter.context.as_ref().with_context(|| {
                    format!("trace event {event_index} enter context is missing")
                })?;
                let inner = ctx.inner.as_ref().with_context(|| {
                    format!("trace event {event_index} inner context is missing")
                })?;
                let myself = inner.myself.as_ref().with_context(|| {
                    format!("trace event {event_index} context.myself is missing")
                })?;
                let (myself_block, myself_tx) = alkane_id_to_strings(
                    myself,
                    &format!("trace event {event_index} context.myself"),
                )?;

                let inputs: Vec<String> = inner
                    .inputs
                    .iter()
                    .map(|i| format!("0x{:x}", uint128_to_u128(i)))
                    .collect();
                let incoming_alkanes: Vec<JsonValue> = inner
                    .incoming_alkanes
                    .iter()
                    .enumerate()
                    .map(|(transfer_index, transfer)| {
                        alkane_transfer_to_json(
                            transfer,
                            &format!("trace event {event_index} incoming alkane {transfer_index}"),
                        )
                    })
                    .collect::<Result<_>>()?;

                let data = json!({
                    "type": call_type,
                    "context": {
                        "myself": {
                            "block": myself_block,
                            "tx": myself_tx,
                        },
                        "inputs": inputs,
                        "incomingAlkanes": incoming_alkanes,
                        "fuel": ctx.fuel,
                    },
                });

                events.push(json!({
                    "event": "invoke",
                    "vout": vout,
                    "data": data,
                }));
            }
            alkanes_pb::alkanes_trace_event::Event::ExitContext(exit) => {
                let status = match exit.status {
                    0 => "success",
                    1 => "failure",
                    value => bail!("trace event {event_index} has unknown exit status {value}"),
                };

                let mut response_data = json!({});
                if let Some(ref resp) = exit.response {
                    // Convert response alkanes
                    let alkanes: Vec<JsonValue> = resp
                        .alkanes
                        .iter()
                        .enumerate()
                        .map(|(transfer_index, transfer)| {
                            alkane_transfer_to_json(
                                transfer,
                                &format!(
                                    "trace event {event_index} response alkane {transfer_index}"
                                ),
                            )
                        })
                        .collect::<Result<_>>()?;
                    response_data["alkanes"] = json!(alkanes);
                }

                events.push(json!({
                    "event": "return",
                    "vout": vout,
                    "data": {
                        "status": status,
                        "response": response_data,
                    },
                }));
            }
            alkanes_pb::alkanes_trace_event::Event::CreateAlkane(create) => {
                let new_alkane = create
                    .new_alkane
                    .as_ref()
                    .with_context(|| format!("trace event {event_index} new alkane is missing"))?;
                let (block, tx) = alkane_id_to_strings(
                    new_alkane,
                    &format!("trace event {event_index} new alkane"),
                )?;
                events.push(json!({
                    "event": "create",
                    "vout": vout,
                    "data": {
                        "newAlkane": {
                            "block": block,
                            "tx": tx,
                        },
                    },
                }));
            }
            alkanes_pb::alkanes_trace_event::Event::ReceiveIntent(receive_intent) => {
                let incoming_alkanes: Vec<JsonValue> = receive_intent
                    .incoming_alkanes
                    .iter()
                    .enumerate()
                    .map(|(transfer_index, transfer)| {
                        alkane_transfer_to_json(
                            transfer,
                            &format!("trace event {event_index} receive alkane {transfer_index}"),
                        )
                    })
                    .collect::<Result<_>>()?;
                events.push(json!({
                    "event": "receive_intent",
                    "vout": vout,
                    "data": {
                        "incomingAlkanes": incoming_alkanes,
                    },
                }));
            }
            alkanes_pb::alkanes_trace_event::Event::ValueTransfer(value_transfer) => {
                let transfers: Vec<JsonValue> = value_transfer
                    .transfers
                    .iter()
                    .enumerate()
                    .map(|(transfer_index, transfer)| {
                        alkane_transfer_to_json(
                            transfer,
                            &format!("trace event {event_index} value transfer {transfer_index}"),
                        )
                    })
                    .collect::<Result<_>>()?;

                events.push(json!({
                    "event": "value_transfer",
                    "vout": vout,
                    "data": {
                        "transfers": transfers,
                        "redirect_to": value_transfer.redirect_to,
                    },
                }));
            }
        }
    }

    Ok(events)
}

async fn trace_call<P: AlkanesProvider + DeezelProvider + JsonRpcProvider + Send + Sync>(
    provider: &P,
    _url: &str,
    job: TraceJob,
) -> Result<Vec<JsonValue>> {
    // Convert little-endian txid back to big-endian for outpoint string
    let txid_be = {
        let mut bytes = hex::decode(&job.txid_le_hex)?;
        bytes.reverse();
        hex::encode(bytes)
    };
    let outpoint_str = format!("{}:{}", txid_be, job.vout);

    // Use AlkanesProvider::trace() which properly decodes the protobuf response
    let trace_pb = provider
        .trace(&outpoint_str)
        .await
        .context("AlkanesProvider::trace call failed")?;

    // Convert to JSON events
    let alkanes_trace = trace_pb
        .trace
        .as_ref()
        .context("AlkanesProvider::trace response is missing trace payload")?;
    let events = convert_trace_to_events(alkanes_trace, job.vout as i32)?;

    Ok(events)
}

async fn tx_from_json_or_fetch_hex<
    P: DeezelProvider + JsonRpcProvider + BitcoinRpcProvider + EsploraProvider + Send + Sync,
>(
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
            if attempt >= 2 {
                break String::new();
            }
            sleep(Duration::from_millis(200 * attempt as u64)).await;
        }
    };
    let hex_str = if !hex_str.is_empty() {
        hex_str
    } else {
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
                return Err(
                    _last_err.unwrap_or_else(|| anyhow::anyhow!("get_transaction_hex failed"))
                )
                .context("get_transaction_hex call failed");
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
    P: AlkanesProvider
        + DeezelProvider
        + JsonRpcProvider
        + BitcoinRpcProvider
        + EsploraProvider
        + Send
        + Sync,
{
    let url = resolve_sandshrew_url(provider);
    info!(
        txs = txs.len(),
        "decode_and_trace_for_block: start (batched parallel)"
    );
    // Only OP_RETURN txs
    let op_return_txs: Vec<JsonValue> = txs
        .iter()
        .filter(|t| tx_has_op_return(t))
        .cloned()
        .collect();
    let total = op_return_txs.len();
    info!(op_return_txs = total, "filtered OP_RETURN transactions");
    if total == 0 {
        return Ok(Vec::new());
    }

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
                let decode_attempt = catch_unwind(AssertUnwindSafe(|| format_runestone_with_decoded_messages(&tx, bitcoin::Network::Bitcoin)));
                match decode_attempt {
                    Ok(Ok(formatted)) => {
                        let txid_be = formatted.get("transaction_id").and_then(|v| v.as_str()).map(|s| s.to_string()).unwrap_or_else(|| tx.compute_txid().to_string());
                        let txid_le = to_little_endian_hex(&txid_be);
                        let start = (tx.output.len() as u32) + 1;
                        let Some(protos) = formatted
                            .get("protostones")
                            .and_then(|value| value.as_array())
                            .cloned()
                        else {
                            *fatal_err.lock().await = Some(format!(
                                "protostone decode result is missing its array for {txid_be}"
                            ));
                            return;
                        };
                        info!(batch = batch_idx, %txid_be, protostones = protos.len(), start_vout = start, "decoded runestone");
                        let mut decoded_items: Vec<DecodedProtostoneItem> = Vec::with_capacity(protos.len());
                        let mut trace_events: Vec<TraceEventItem> = Vec::new();
                        let mut has_trace = false;
                        let mut trace_succeed = false;

                        for (i, p) in protos.iter().enumerate() {
                            let vout = start + i as u32;
                            info!(batch = batch_idx, %txid_be, protostone_idx = i, vout, "calling trace");
                            if p.get("message_decoded").is_none_or(JsonValue::is_null) {
                                *fatal_err.lock().await = Some(format!(
                                    "protostone message decode is incomplete for {txid_be} vout {vout}"
                                ));
                                return;
                            }
                            let job = TraceJob { txid_le_hex: txid_le.clone(), vout, protostone_idx: i };
                            debug!(batch = batch_idx, %txid_be, protostone_idx = i, "dispatching trace job");
                            decoded_items.push(DecodedProtostoneItem { vout: vout as i32, protostone_index: i as i32, decoded: p.clone() });
                            match trace_call(provider, &url, job).await {
                                Ok(events) => {
                                    info!(batch = batch_idx, %txid_be, protostone_idx = i, vout, events_count = events.len(), "trace ok");
                                    if !events.is_empty() {
                                        has_trace = true;
                                    }
                                    
                                    // Process events - they are already in the correct format from convert_trace_to_events
                                    for ev in &events {
                                        let Some(event_type) = ev.get("event").and_then(|v| v.as_str()) else {
                                            *fatal_err.lock().await = Some(format!(
                                                "trace event is missing event type for {} vout {}",
                                                txid_be, vout
                                            ));
                                            return;
                                        };
                                        let Some(data) = ev.get("data").cloned() else {
                                            *fatal_err.lock().await = Some(format!(
                                                "trace event is missing data for {} vout {}",
                                                txid_be, vout
                                            ));
                                            return;
                                        };
                                        let event_type = event_type.to_string();
                                        
                                        // Check for success status in return events
                                        if event_type == "return" {
                                            let st = data.get("status").and_then(|s| s.as_str()).unwrap_or("").to_ascii_lowercase();
                                            if st.contains("success") { trace_succeed = true; }
                                        }
                                        
                                        // Extract alkane address from invoke context
                                        let (blk_str, tx_str) = if event_type == "invoke" {
                                            let myself = data.get("context")
                                                .and_then(|c| c.get("myself"));
                                            let Some(blk) = myself.and_then(|m| m.get("block")).and_then(|v| v.as_str()) else {
                                                *fatal_err.lock().await = Some(format!(
                                                    "invoke trace is missing context.myself.block for {} vout {}",
                                                    txid_be, vout
                                                ));
                                                return;
                                            };
                                            let Some(tx) = myself.and_then(|m| m.get("tx")).and_then(|v| v.as_str()) else {
                                                *fatal_err.lock().await = Some(format!(
                                                    "invoke trace is missing context.myself.tx for {} vout {}",
                                                    txid_be, vout
                                                ));
                                                return;
                                            };
                                            (blk.to_string(), tx.to_string())
                                        } else { (String::new(), String::new()) };
                                        
                                        trace_events.push(TraceEventItem {
                                            vout: vout as i32,
                                            event_type,
                                            data,
                                            alkane_address_block: blk_str,
                                            alkane_address_tx: tx_str,
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
                    Ok(Err(e)) => {
                        error!(batch = batch_idx, %txid_str, error = %e, "protostone decode failed; aborting block batch");
                        *fatal_err.lock().await = Some(format!("protostone decode failed for {}: {}", txid_str, e));
                        return;
                    }
                    Err(panic) => {
                        let panic_msg: &str = if let Some(s) = panic.downcast_ref::<&str>() {
                            s
                        } else if let Some(s) = panic.downcast_ref::<String>() {
                            s.as_str()
                        } else {
                            "panic"
                        };
                        error!(batch = batch_idx, %txid_str, message = %panic_msg, "protostone decode panicked; aborting block batch");
                        *fatal_err.lock().await = Some(format!("protostone decode panicked for {}: {}", txid_str, panic_msg));
                        return;
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

    let mut by_txid = HashMap::with_capacity(total);
    for result in results.lock().await.clone() {
        let result_txid = result.transaction_id.clone();
        if by_txid.insert(result_txid.clone(), result).is_some() {
            return Err(anyhow::anyhow!("duplicate trace result for {result_txid}"));
        }
    }
    let mut ordered = Vec::with_capacity(total);
    for tx in &op_return_txs {
        let txid = tx
            .get("txid")
            .and_then(|value| value.as_str())
            .context("OP_RETURN transaction is missing txid")?;
        ordered.push(by_txid.remove(txid).with_context(|| {
            format!("trace pipeline produced no result for OP_RETURN transaction {txid}")
        })?);
    }
    if !by_txid.is_empty() {
        return Err(anyhow::anyhow!(
            "trace pipeline returned transactions outside the source block"
        ));
    }
    Ok(ordered)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alkanes_pb::alkanes_trace_event::Event;

    fn uint128(value: u64) -> alkanes_pb::Uint128 {
        alkanes_pb::Uint128 { lo: value, hi: 0 }
    }

    #[test]
    fn trace_event_without_payload_fails_closed() {
        let trace = alkanes_pb::AlkanesTrace {
            events: vec![alkanes_pb::AlkanesTraceEvent { event: None }],
        };

        assert!(convert_trace_to_events(&trace, 1).is_err());
    }

    #[test]
    fn transfer_without_id_or_value_fails_closed() {
        let missing_id = alkanes_pb::AlkaneTransfer {
            id: None,
            value: Some(uint128(1)),
        };
        let missing_value = alkanes_pb::AlkaneTransfer {
            id: Some(alkanes_pb::AlkaneId {
                block: Some(uint128(4)),
                tx: Some(uint128(10)),
            }),
            value: None,
        };

        for transfer in [missing_id, missing_value] {
            let trace = alkanes_pb::AlkanesTrace {
                events: vec![alkanes_pb::AlkanesTraceEvent {
                    event: Some(Event::ValueTransfer(alkanes_pb::AlkanesValueTransfer {
                        transfers: vec![transfer],
                        redirect_to: 0,
                    })),
                }],
            };
            assert!(convert_trace_to_events(&trace, 1).is_err());
        }
    }

    #[test]
    fn transfer_conversion_preserves_full_u128_value() {
        let trace = alkanes_pb::AlkanesTrace {
            events: vec![alkanes_pb::AlkanesTraceEvent {
                event: Some(Event::ValueTransfer(alkanes_pb::AlkanesValueTransfer {
                    transfers: vec![alkanes_pb::AlkaneTransfer {
                        id: Some(alkanes_pb::AlkaneId {
                            block: Some(uint128(4)),
                            tx: Some(uint128(10)),
                        }),
                        value: Some(alkanes_pb::Uint128 { lo: 7, hi: 1 }),
                    }],
                    redirect_to: 0,
                })),
            }],
        };

        let events = convert_trace_to_events(&trace, 1).unwrap();
        assert_eq!(
            events[0]["data"]["transfers"][0]["value"],
            JsonValue::String(((1_u128 << 64) + 7).to_string())
        );
    }
}
