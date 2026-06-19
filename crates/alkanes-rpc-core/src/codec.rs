use anyhow::Result;
use prost::Message;
use serde_json::{Value, json};

use alkanes_cli_common::proto::alkanes::{
    MessageContextParcel, AlkaneTransfer, AlkaneId, Uint128, SimulateResponse, KeyValuePair,
    AlkaneIdToOutpointRequest, AlkaneIdToOutpointResponse,
    AlkanesTrace, AlkanesTraceEvent,
    alkanes_trace_event::Event as TraceEventEnum,
    Outpoint,
};
use alkanes_cli_common::proto::protorune::{
    OutpointResponse, OutpointWithProtocol, ProtorunesWalletRequest, WalletResponse,
    Uint128 as ProtoruneUint128,
};
use alkanes_cli_common::alkanes::utils::encode_varint_list;

// ---------------------------------------------------------------------------
// Uint128 helpers
// ---------------------------------------------------------------------------

/// Parse a u128 from JSON value (handles both string and number).
pub fn parse_u128(value: &Value) -> Option<u128> {
    match value {
        Value::Number(n) => n.as_u64().map(|v| v as u128),
        Value::String(s) => s.parse::<u128>().ok(),
        _ => None,
    }
}

/// Convert u128 to protobuf Uint128 (lo/hi split).
pub fn to_uint128(value: u128) -> Uint128 {
    Uint128 {
        lo: value as u64,
        hi: (value >> 64) as u64,
    }
}

/// Convert protobuf Uint128 to u128.
pub fn from_uint128(value: &Option<Uint128>) -> u128 {
    match value {
        Some(v) => (v.hi as u128) << 64 | (v.lo as u128),
        None => 0,
    }
}

/// Convert protorune's Uint128 to u128.
pub fn from_protorune_uint128(value: &Option<ProtoruneUint128>) -> u128 {
    match value {
        Some(v) => (v.hi as u128) << 64 | (v.lo as u128),
        None => 0,
    }
}

// ---------------------------------------------------------------------------
// JSON formatters
// ---------------------------------------------------------------------------

/// Recursively convert string numbers to actual numbers in JSON values.
pub fn convert_string_numbers(value: Value) -> Value {
    match value {
        Value::String(s) => {
            if let Ok(n) = s.parse::<u64>() {
                Value::Number(n.into())
            } else if let Ok(n) = s.parse::<i64>() {
                Value::Number(n.into())
            } else if let Ok(n) = s.parse::<f64>() {
                serde_json::Number::from_f64(n)
                    .map(Value::Number)
                    .unwrap_or(Value::String(s))
            } else {
                Value::String(s)
            }
        }
        Value::Array(arr) => {
            Value::Array(arr.into_iter().map(convert_string_numbers).collect())
        }
        Value::Object(obj) => {
            Value::Object(
                obj.into_iter()
                    .map(|(k, v)| (k, convert_string_numbers(v)))
                    .collect(),
            )
        }
        other => other,
    }
}

/// Format a storage key for display (matches TypeScript formatKey).
pub fn format_key(key: &[u8]) -> String {
    key.split(|&c| c == b'/')
        .map(|segment| {
            if segment.is_empty() {
                return String::new();
            }
            match String::from_utf8(segment.to_vec()) {
                Ok(s) if s.chars().all(|c| c.is_alphanumeric() || c == '_') => s,
                _ => hex::encode(segment),
            }
        })
        .collect::<Vec<_>>()
        .join("/")
}

/// Convert AlkaneTransfer to JSON.
pub fn alkane_transfer_to_json(transfer: &AlkaneTransfer) -> Value {
    let id = transfer.id.as_ref();
    json!({
        "id": {
            "block": from_uint128(&id.and_then(|i| i.block.clone())).to_string(),
            "tx": from_uint128(&id.and_then(|i| i.tx.clone())).to_string()
        },
        "value": from_uint128(&transfer.value).to_string()
    })
}

/// Convert KeyValuePair (storage slot) to JSON.
pub fn storage_slot_to_json(slot: &KeyValuePair) -> Value {
    json!({
        "key": format_key(&slot.key),
        "value": format!("0x{}", hex::encode(&slot.value))
    })
}

/// Convert call type enum to string (matches TypeScript fromCallType).
pub fn call_type_to_string(call_type: i32) -> &'static str {
    match call_type {
        1 => "call",
        2 => "delegatecall",
        3 => "staticcall",
        _ => "unknowncall",
    }
}

/// Convert AlkanesTraceEvent to JSON (matches TypeScript toEvent).
pub fn trace_event_to_json(event: &AlkanesTraceEvent) -> Value {
    match &event.event {
        Some(TraceEventEnum::EnterContext(enter)) => {
            let context = enter.context.as_ref().and_then(|tc| tc.inner.as_ref());
            match context {
                Some(ctx) => json!({
                    "event": "invoke",
                    "data": {
                        "type": call_type_to_string(enter.call_type),
                        "context": {
                            "myself": {
                                "block": from_uint128(&ctx.myself.as_ref()
                                    .and_then(|m| m.block.clone())).to_string(),
                                "tx": from_uint128(&ctx.myself.as_ref()
                                    .and_then(|m| m.tx.clone())).to_string()
                            },
                            "caller": {
                                "block": from_uint128(&ctx.caller.as_ref()
                                    .and_then(|c| c.block.clone())).to_string(),
                                "tx": from_uint128(&ctx.caller.as_ref()
                                    .and_then(|c| c.tx.clone())).to_string()
                            },
                            "inputs": ctx.inputs.iter()
                                .map(|i| from_uint128(&Some(i.clone())).to_string())
                                .collect::<Vec<_>>(),
                            "incomingAlkanes": ctx.incoming_alkanes.iter()
                                .map(alkane_transfer_to_json)
                                .collect::<Vec<_>>(),
                            "vout": ctx.vout
                        },
                        "fuel": enter.context.as_ref().map(|tc| tc.fuel).unwrap_or(0)
                    }
                }),
                None => json!({"event": "invoke", "data": {"type": "unknowncall", "context": {}, "fuel": 0}})
            }
        }
        Some(TraceEventEnum::ExitContext(exit)) => {
            let response = exit.response.as_ref();
            json!({
                "event": "return",
                "data": {
                    "status": if exit.status == 0 { "success" } else { "revert" },
                    "response": match response {
                        Some(resp) => json!({
                            "alkanes": resp.alkanes.iter().map(alkane_transfer_to_json).collect::<Vec<_>>(),
                            "storage": resp.storage.iter().map(storage_slot_to_json).collect::<Vec<_>>(),
                            "data": format!("0x{}", hex::encode(&resp.data))
                        }),
                        None => json!({"alkanes": [], "storage": [], "data": "0x"})
                    }
                }
            })
        }
        Some(TraceEventEnum::CreateAlkane(create)) => {
            let new_alkane = create.new_alkane.as_ref();
            match new_alkane {
                Some(id) => json!({
                    "event": "create",
                    "data": {
                        "block": from_uint128(&id.block).to_string(),
                        "tx": from_uint128(&id.tx).to_string()
                    }
                }),
                None => json!({"event": "create", "data": {"block": "0", "tx": "0"}})
            }
        }
        _ => json!({"event": "unknown", "data": {}})
    }
}

// ---------------------------------------------------------------------------
// Response decoders (protobuf hex → JSON)
// ---------------------------------------------------------------------------

/// Decode SimulateResponse protobuf to JSON matching TypeScript format.
pub fn decode_simulate_response(hex_response: &str) -> Result<Value> {
    let hex_data = hex_response.strip_prefix("0x").unwrap_or(hex_response);
    let bytes = hex::decode(hex_data)?;
    let response = SimulateResponse::decode(bytes.as_slice())?;

    if !response.error.is_empty() {
        return Ok(json!({
            "status": 1,
            "gasUsed": 0,
            "execution": {
                "alkanes": [],
                "storage": [],
                "data": "0x",
                "error": response.error
            }
        }));
    }

    let execution = match &response.execution {
        Some(exec) => exec,
        None => {
            return Ok(json!({
                "status": 1,
                "gasUsed": 0,
                "execution": {
                    "alkanes": [],
                    "storage": [],
                    "data": "0x",
                    "error": "No execution result"
                }
            }));
        }
    };

    Ok(json!({
        "status": 0,
        "gasUsed": response.gas_used,
        "execution": {
            "alkanes": execution.alkanes.iter().map(alkane_transfer_to_json).collect::<Vec<_>>(),
            "storage": execution.storage.iter().map(storage_slot_to_json).collect::<Vec<_>>(),
            "data": format!("0x{}", hex::encode(&execution.data)),
            "error": null
        }
    }))
}

/// Decode OutpointResponse protobuf to JSON for protorunesbyoutpoint.
pub fn decode_outpoint_response(hex_response: &str) -> Result<Value> {
    let hex_data = hex_response.strip_prefix("0x").unwrap_or(hex_response);
    let bytes = hex::decode(hex_data)?;
    let response = OutpointResponse::decode(bytes.as_slice())?;

    let balances: Vec<Value> = response.balances.as_ref()
        .map(|bs| {
            bs.entries.iter().map(|entry| {
                let (block, tx) = entry.rune.as_ref()
                    .and_then(|r| r.rune_id.as_ref())
                    .map(|id| {
                        let height = from_protorune_uint128(&id.height);
                        let txindex = from_protorune_uint128(&id.txindex);
                        (height as u64, txindex as u64)
                    })
                    .unwrap_or((0, 0));
                let amount = from_protorune_uint128(&entry.balance);
                json!({
                    "block": block,
                    "tx": tx,
                    "amount": amount as u64
                })
            }).collect()
        })
        .unwrap_or_default();

    let txid = response.outpoint.as_ref()
        .map(|op| {
            let mut txid_bytes = op.txid.clone();
            txid_bytes.reverse();
            hex::encode(&txid_bytes)
        })
        .unwrap_or_default();

    let vout = response.outpoint.as_ref()
        .map(|op| op.vout)
        .unwrap_or(0);

    let value = response.output.as_ref()
        .map(|o| o.value)
        .unwrap_or(0);

    Ok(json!({
        "balance_sheet": {
            "cached": {
                "balances": balances
            }
        },
        "outpoint": {
            "txid": txid,
            "vout": vout
        },
        "output": {
            "value": value
        }
    }))
}

/// Decode meta response (raw UTF-8 JSON string, not protobuf).
pub fn decode_meta_response(hex_response: &str) -> Result<Value> {
    let hex_data = hex_response.strip_prefix("0x").unwrap_or(hex_response);
    if hex_data.is_empty() {
        return Ok(Value::Null);
    }
    let bytes = hex::decode(hex_data)?;
    let utf8_str = String::from_utf8(bytes)?;
    if utf8_str.is_empty() {
        return Ok(Value::Null);
    }
    serde_json::from_str(&utf8_str)
        .map_err(|e| anyhow::anyhow!("Invalid JSON in meta response: {}", e))
}

/// Decode AlkaneIdToOutpointResponse protobuf to JSON.
pub fn decode_alkanes_id_to_outpoint_response(hex_response: &str) -> Result<Value> {
    let hex_data = hex_response.strip_prefix("0x").unwrap_or(hex_response);

    if hex_data.is_empty() {
        return Ok(json!({"outpoint": {}}));
    }

    let bytes = hex::decode(hex_data)?;
    let response = AlkaneIdToOutpointResponse::decode(bytes.as_slice())?;

    if response.txid.is_empty() {
        return Ok(json!({"outpoint": {}}));
    }

    let txid_hex = hex::encode(&response.txid);

    Ok(json!({
        "outpoint": {
            "txid": txid_hex,
            "vout": response.vout
        }
    }))
}

/// Decode AlkanesTrace protobuf to JSON array.
pub fn decode_trace_response(hex_response: &str) -> Result<Value> {
    let hex_data = hex_response.strip_prefix("0x").unwrap_or(hex_response);

    if hex_data.is_empty() {
        return Ok(json!([]));
    }

    let bytes = hex::decode(hex_data)?;
    let trace = AlkanesTrace::decode(bytes.as_slice())?;

    let events: Vec<Value> = trace.events.iter()
        .map(trace_event_to_json)
        .collect();

    Ok(json!(events))
}

/// Decode WalletResponse protobuf into JSON.
pub fn decode_wallet_response(hex_response: &str) -> Result<Value> {
    let hex_data = hex_response.strip_prefix("0x").unwrap_or(hex_response);
    if hex_data.is_empty() {
        return Ok(json!({ "outpoints": [], "balances": { "entries": [] } }));
    }
    let bytes = hex::decode(hex_data)?;
    if bytes.is_empty() {
        return Ok(json!({ "outpoints": [], "balances": { "entries": [] } }));
    }
    let response = WalletResponse::decode(bytes.as_slice())?;

    let outpoints: Vec<Value> = response.outpoints.iter().map(|op| {
        let balances: Vec<Value> = op.balances.as_ref()
            .map(|bs| {
                bs.entries.iter().map(|entry| {
                    let (block, tx) = entry.rune.as_ref()
                        .and_then(|r| r.rune_id.as_ref())
                        .map(|id| {
                            let height = from_protorune_uint128(&id.height);
                            let txindex = from_protorune_uint128(&id.txindex);
                            (height as u64, txindex as u64)
                        })
                        .unwrap_or((0, 0));
                    let amount = from_protorune_uint128(&entry.balance);
                    json!({ "block": block, "tx": tx, "amount": amount as u64 })
                }).collect()
            })
            .unwrap_or_default();

        let txid = op.outpoint.as_ref()
            .map(|o| {
                let mut txid_bytes = o.txid.clone();
                txid_bytes.reverse();
                hex::encode(&txid_bytes)
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
    }).collect();

    let balances: Vec<Value> = response.balances.as_ref()
        .map(|bs| {
            bs.entries.iter().map(|entry| {
                let (block, tx) = entry.rune.as_ref()
                    .and_then(|r| r.rune_id.as_ref())
                    .map(|id| {
                        let height = from_protorune_uint128(&id.height);
                        let txindex = from_protorune_uint128(&id.txindex);
                        (height as u64, txindex as u64)
                    })
                    .unwrap_or((0, 0));
                let amount = from_protorune_uint128(&entry.balance);
                json!({ "block": block, "tx": tx, "amount": amount as u64 })
            }).collect()
        })
        .unwrap_or_default();

    Ok(json!({
        "outpoints": outpoints,
        "balances": { "entries": balances },
    }))
}

// ---------------------------------------------------------------------------
// Request encoders (JSON → protobuf hex)
// ---------------------------------------------------------------------------

/// Encode meta request: {target: {block, tx}} → protobuf hex.
pub fn encode_meta_request(params: &Value) -> Result<String> {
    let obj = params.as_object()
        .ok_or_else(|| anyhow::anyhow!("meta params must be an object"))?;

    let target_obj = obj.get("target")
        .and_then(|v| v.as_object())
        .ok_or_else(|| anyhow::anyhow!("Missing or invalid 'target' parameter"))?;

    let block = parse_u128(target_obj.get("block")
        .ok_or_else(|| anyhow::anyhow!("Missing 'target.block' parameter"))?)
        .ok_or_else(|| anyhow::anyhow!("Invalid 'target.block' parameter"))?;

    let tx = parse_u128(target_obj.get("tx")
        .ok_or_else(|| anyhow::anyhow!("Missing 'target.tx' parameter"))?)
        .ok_or_else(|| anyhow::anyhow!("Invalid 'target.tx' parameter"))?;

    let alkane_id = AlkaneId {
        block: Some(to_uint128(block)),
        tx: Some(to_uint128(tx)),
    };

    let mut buf = Vec::new();
    alkane_id.encode(&mut buf)?;
    Ok(format!("0x{}", hex::encode(buf)))
}

/// Encode alkanes_id_to_outpoint request: {block, tx} → protobuf hex.
pub fn encode_alkanes_id_to_outpoint_request(params: &Value) -> Result<String> {
    let obj = params.as_object()
        .ok_or_else(|| anyhow::anyhow!("alkanesidtooutpoint params must be an object"))?;

    let block = parse_u128(obj.get("block")
        .ok_or_else(|| anyhow::anyhow!("Missing 'block' parameter"))?)
        .ok_or_else(|| anyhow::anyhow!("Invalid 'block' parameter"))?;

    let tx = parse_u128(obj.get("tx")
        .ok_or_else(|| anyhow::anyhow!("Missing 'tx' parameter"))?)
        .ok_or_else(|| anyhow::anyhow!("Invalid 'tx' parameter"))?;

    let alkane_id = AlkaneId {
        block: Some(to_uint128(block)),
        tx: Some(to_uint128(tx)),
    };

    let request = AlkaneIdToOutpointRequest {
        id: Some(alkane_id),
    };

    let mut buf = Vec::new();
    request.encode(&mut buf)?;
    Ok(format!("0x{}", hex::encode(buf)))
}

/// Encode trace request: {txid, vout} → protobuf hex.
pub fn encode_trace_request(params: &Value) -> Result<String> {
    let obj = params.as_object()
        .ok_or_else(|| anyhow::anyhow!("trace params must be an object"))?;

    let txid_str = obj.get("txid")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing or invalid 'txid' parameter"))?;

    let txid_hex = txid_str.strip_prefix("0x").unwrap_or(txid_str);
    let txid_bytes = hex::decode(txid_hex)
        .map_err(|e| anyhow::anyhow!("Invalid txid hex: {}", e))?;

    let vout = obj.get("vout")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| anyhow::anyhow!("Missing or invalid 'vout' parameter"))? as u32;

    let outpoint = Outpoint {
        txid: txid_bytes,
        vout,
    };

    let mut buf = Vec::new();
    outpoint.encode(&mut buf)?;
    Ok(format!("0x{}", hex::encode(buf)))
}

/// Encode protorunesbyoutpoint request from params to protobuf hex.
///
/// Accepts either:
///   - Positional params: [txid_hex, vout, block_tag?, protocol_tag?]
///   - Object param: [{ txid, vout, protocolTag? }, block_tag?]
pub fn encode_protorunesbyoutpoint_request(params: &[Value]) -> Result<String> {
    let input = params.get(0)
        .ok_or_else(|| anyhow::anyhow!("protorunesbyoutpoint requires at least one parameter"))?;

    let (txid_hex, vout, protocol_tag) = if let Some(txid_str) = input.as_str() {
        let vout = params.get(1)
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;
        let pt = params.get(3)
            .and_then(|v| v.as_u64().or_else(|| v.as_str().and_then(|s| s.parse::<u64>().ok())))
            .unwrap_or(1) as u128;
        (txid_str.to_string(), vout, pt)
    } else if let Some(obj) = input.as_object() {
        let txid = obj.get("txid")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("protorunesbyoutpoint object must have 'txid' field"))?;
        let vout = obj.get("vout")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;
        let pt = obj.get("protocolTag")
            .and_then(|v| v.as_u64().or_else(|| v.as_str().and_then(|s| s.parse::<u64>().ok())))
            .unwrap_or(1) as u128;
        (txid.to_string(), vout, pt)
    } else {
        return Err(anyhow::anyhow!("protorunesbyoutpoint: first param must be a txid string or {{txid, vout, protocolTag?}} object"));
    };

    let mut txid_bytes = hex::decode(&txid_hex)
        .map_err(|e| anyhow::anyhow!("Invalid txid hex: {}", e))?;
    txid_bytes.reverse();

    let request = OutpointWithProtocol {
        txid: txid_bytes,
        vout,
        protocol: Some(ProtoruneUint128 {
            lo: protocol_tag as u64,
            hi: 0,
        }),
    };

    Ok(format!("0x{}", hex::encode(request.encode_to_vec())))
}

/// Encode protorunesbyaddress request from params to protobuf hex.
///
/// Accepts either:
///   - A plain string address: params[0] = "bcrt1p..."
///   - A JSON object: params[0] = {"address": "bcrt1p...", "protocolTag": "1"}
pub fn encode_protorunesbyaddress_request(params: &[Value]) -> Result<String> {
    let input = params.get(0)
        .ok_or_else(|| anyhow::anyhow!("protorunesbyaddress requires an address parameter"))?;

    let (address, protocol_tag) = if let Some(addr_str) = input.as_str() {
        let pt = params.get(2)
            .and_then(|v| v.as_u64().or_else(|| v.as_str().and_then(|s| s.parse::<u64>().ok())))
            .unwrap_or(1) as u128;
        (addr_str.to_string(), pt)
    } else if let Some(obj) = input.as_object() {
        let addr = obj.get("address")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("protorunesbyaddress object must have 'address' field"))?;
        let pt = obj.get("protocolTag")
            .and_then(|v| v.as_u64().or_else(|| v.as_str().and_then(|s| s.parse::<u64>().ok())))
            .unwrap_or(1) as u128;
        (addr.to_string(), pt)
    } else {
        return Err(anyhow::anyhow!("protorunesbyaddress: first param must be an address string or {{address, protocolTag}} object"));
    };

    let request = ProtorunesWalletRequest {
        wallet: address.into_bytes(),
        protocol_tag: Some(ProtoruneUint128 {
            lo: protocol_tag as u64,
            hi: 0,
        }),
    };

    Ok(format!("0x{}", hex::encode(request.encode_to_vec())))
}

/// Encode simulate request JSON to protobuf hex string.
pub fn encode_simulate_request(params: &Value) -> Result<String> {
    let obj = params.as_object()
        .ok_or_else(|| anyhow::anyhow!("simulate params must be an object"))?;

    let alkanes: Vec<AlkaneTransfer> = obj.get("alkanes")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter().filter_map(|item| {
                let id_obj = item.get("id")?;
                let block = parse_u128(id_obj.get("block")?)?;
                let tx = parse_u128(id_obj.get("tx")?)?;
                let value = parse_u128(item.get("value")?)?;
                Some(AlkaneTransfer {
                    id: Some(AlkaneId {
                        block: Some(to_uint128(block)),
                        tx: Some(to_uint128(tx)),
                    }),
                    value: Some(to_uint128(value)),
                })
            }).collect()
        })
        .unwrap_or_default();

    let transaction: Vec<u8> = obj.get("transaction")
        .and_then(|v| v.as_str())
        .map(|s| {
            let s = s.strip_prefix("0x").unwrap_or(s);
            hex::decode(s).unwrap_or_default()
        })
        .unwrap_or_default();

    let block: Vec<u8> = obj.get("block")
        .and_then(|v| v.as_str())
        .map(|s| {
            let s = s.strip_prefix("0x").unwrap_or(s);
            hex::decode(s).unwrap_or_default()
        })
        .unwrap_or_default();

    let height = obj.get("height")
        .and_then(|v| match v {
            Value::Number(n) => n.as_u64(),
            Value::String(s) => s.parse::<u64>().ok(),
            _ => None,
        })
        .unwrap_or(0);

    let txindex = obj.get("txindex")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;

    let mut calldata_values: Vec<u128> = Vec::new();

    if let Some(target) = obj.get("target") {
        if let Some(target_obj) = target.as_object() {
            if let Some(block) = parse_u128(target_obj.get("block").unwrap_or(&Value::Null)) {
                calldata_values.push(block);
            }
            if let Some(tx) = parse_u128(target_obj.get("tx").unwrap_or(&Value::Null)) {
                calldata_values.push(tx);
            }
        } else if let Some(target_str) = target.as_str() {
            let parts: Vec<&str> = target_str.split(':').collect();
            if parts.len() == 2 {
                if let Ok(block) = parts[0].parse::<u128>() {
                    calldata_values.push(block);
                }
                if let Ok(tx) = parts[1].parse::<u128>() {
                    calldata_values.push(tx);
                }
            }
        }
    }

    if let Some(inputs) = obj.get("inputs").and_then(|v| v.as_array()) {
        for input in inputs {
            if let Some(val) = parse_u128(input) {
                calldata_values.push(val);
            }
        }
    }

    let calldata = encode_varint_list(&calldata_values);

    let vout = obj.get("vout")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;

    let pointer = obj.get("pointer")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;

    let refund_pointer = obj.get("refundPointer")
        .or_else(|| obj.get("refund_pointer"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;

    let parcel = MessageContextParcel {
        alkanes,
        transaction,
        block,
        height,
        txindex,
        calldata,
        vout,
        pointer,
        refund_pointer,
    };

    let mut buf = Vec::new();
    parcel.encode(&mut buf)?;
    Ok(format!("0x{}", hex::encode(buf)))
}
