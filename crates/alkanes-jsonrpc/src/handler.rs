use crate::jsonrpc::{JsonRpcRequest, JsonRpcResponse, INTERNAL_ERROR, INVALID_PARAMS, METHOD_NOT_FOUND};
use crate::proxy::ProxyClient;
use crate::sandshrew;
use anyhow::Result;
use serde_json::{Value, json};
use prost::Message;
use alkanes_cli_common::proto::alkanes::{
    MessageContextParcel, AlkaneTransfer, AlkaneId, Uint128, SimulateResponse, KeyValuePair,
    AlkaneIdToOutpointRequest, AlkaneIdToOutpointResponse,
    AlkanesTrace, AlkanesTraceEvent,
    alkanes_trace_event::Event as TraceEventEnum,
    Outpoint,
};
use alkanes_cli_common::proto::protorune::{OutpointResponse, OutpointWithProtocol, ProtorunesWalletRequest, WalletResponse, Uint128 as ProtoruneUint128};
use alkanes_cli_common::alkanes::utils::encode_varint_list;

pub async fn handle_request(
    request: &JsonRpcRequest,
    proxy: &ProxyClient,
) -> Result<JsonRpcResponse> {
    handle_request_with_storage(request, proxy, None).await
}

pub async fn handle_request_with_storage(
    request: &JsonRpcRequest,
    proxy: &ProxyClient,
    script_storage: Option<&crate::lua_executor::ScriptStorage>,
) -> Result<JsonRpcResponse> {
    let method_parts: Vec<&str> = request.method.split('_').collect();
    
    if method_parts.is_empty() {
        return Ok(JsonRpcResponse::error(
            METHOD_NOT_FOUND,
            "Invalid method format".to_string(),
            request.id.clone(),
        ));
    }

    let namespace = method_parts[0];
    let method_name = if method_parts.len() > 1 {
        method_parts[1..].join("_")
    } else {
        String::new()
    };

    // Handle special non-namespaced methods
    if request.method == "spendablesbyaddress" {
        return handle_spendables_by_address(&request.params, &request.id, proxy).await;
    }

    match namespace {
        "ord" => handle_ord_method(&method_name, &request.params, &request.id, proxy).await,
        "esplora" => handle_esplora_method(&method_name, &request.params, &request.id, proxy).await,
        "alkanes" => handle_alkanes_method(&method_name, &request.params, &request.id, proxy).await,
        "metashrew" => handle_metashrew_method(request, proxy).await,
        "memshrew" => handle_memshrew_method(request, proxy).await,
        "lua" => sandshrew::handle_sandshrew_method(&method_name, &request.params, &request.id, proxy, script_storage).await,
        "sandshrew" => sandshrew::handle_sandshrew_method(&method_name, &request.params, &request.id, proxy, script_storage).await,
        "subfrost" => handle_subfrost_method(request, proxy).await,
        "btc" => handle_bitcoind_method(request, proxy).await,
        _ => handle_bitcoind_method(request, proxy).await,
    }
}

/// Handle spendablesbyaddress - returns UTXOs for an address via esplora
/// This is used by the WASM SDK to get spendable UTXOs for building transactions
async fn handle_spendables_by_address(
    params: &[Value],
    request_id: &Value,
    proxy: &ProxyClient,
) -> Result<JsonRpcResponse> {
    if params.is_empty() {
        return Ok(JsonRpcResponse::error(
            INVALID_PARAMS,
            "spendablesbyaddress requires address parameter".to_string(),
            request_id.clone(),
        ));
    }

    let address = params[0].as_str().ok_or_else(|| {
        anyhow::anyhow!("address must be a string")
    })?;

    // Fetch UTXOs from esplora
    let path = format!("/address/{}/utxo", address);
    let utxos = proxy.fetch_esplora_endpoint(&path).await?;

    // Transform esplora UTXOs to spendables format expected by the SDK
    // esplora format: [{ txid, vout, value, status: { block_height, ... } }]
    // spendables format: { outpoints: [{ outpoint: { txid, vout }, value, height }] }
    let empty_vec = vec![];
    let utxo_array = utxos.as_array().unwrap_or(&empty_vec);
    let outpoints: Vec<Value> = utxo_array.iter().map(|utxo| {
        serde_json::json!({
            "outpoint": {
                "txid": utxo.get("txid").and_then(|v| v.as_str()).unwrap_or(""),
                "vout": utxo.get("vout").and_then(|v| v.as_u64()).unwrap_or(0)
            },
            "value": utxo.get("value").and_then(|v| v.as_u64()).unwrap_or(0),
            "height": utxo.get("status").and_then(|s| s.get("block_height")).and_then(|v| v.as_u64()).unwrap_or(0)
        })
    }).collect();

    let result = serde_json::json!({
        "outpoints": outpoints
    });

    Ok(JsonRpcResponse::success(result, request_id.clone()))
}

async fn handle_ord_method(
    method: &str,
    params: &[Value],
    request_id: &Value,
    proxy: &ProxyClient,
) -> Result<JsonRpcResponse> {
    if method == "content" {
        if params.is_empty() {
            return Ok(JsonRpcResponse::error(
                INTERNAL_ERROR,
                "ord_content requires inscription_id parameter".to_string(),
                request_id.clone(),
            ));
        }

        let inscription_id = params[0].as_str().ok_or_else(|| {
            anyhow::anyhow!("inscription_id must be a string")
        })?;

        let content = proxy.fetch_ord_content(inscription_id).await?;
        use base64::Engine;
        let base64_content = base64::engine::general_purpose::STANDARD.encode(&content);

        return Ok(JsonRpcResponse::success(
            Value::String(base64_content),
            request_id.clone(),
        ));
    }

    // Split method on ':' to handle dynamic paths like "block::hash" -> "/block/{param}/hash"
    let path_parts: Vec<&str> = method.split(':').collect();
    let mut path_components: Vec<String> = vec![];
    let mut param_index = 0;

    for part in path_parts {
        if part.is_empty() {
            // Empty part means we need a parameter from params array
            if param_index < params.len() {
                if let Some(param_str) = params[param_index].as_str() {
                    path_components.push(param_str.to_string());
                } else {
                    path_components.push(params[param_index].to_string());
                }
                param_index += 1;
            }
        } else {
            // Non-empty part is a literal path component
            path_components.push(part.to_string());
        }
    }

    // Add any remaining params as path components
    while param_index < params.len() {
        if let Some(param_str) = params[param_index].as_str() {
            path_components.push(param_str.to_string());
        } else {
            path_components.push(params[param_index].to_string());
        }
        param_index += 1;
    }

    // Build path: "/component1/component2/..."
    let path = if path_components.is_empty() {
        "/".to_string()
    } else {
        format!("/{}", path_components.join("/"))
    };

    let result = proxy.fetch_ord_endpoint(&path).await?;
    Ok(JsonRpcResponse::success(result, request_id.clone()))
}

async fn handle_esplora_method(
    method: &str,
    params: &[Value],
    request_id: &Value,
    proxy: &ProxyClient,
) -> Result<JsonRpcResponse> {
    // Handle common underscore-based method aliases from lua scripts
    // e.g., "addressutxo" -> "address::utxo" (becomes /address/{param}/utxo)
    let normalized_method = match method {
        "addressutxo" => "address::utxo",
        "addresstxs" => "address::txs",
        "addresstxsmempool" => "address::txs:mempool",
        "addresstxschain" => "address::txs:chain",
        _ => method,
    };

    let path_parts: Vec<&str> = normalized_method.split(':').collect();
    let mut path = String::from("/");
    let mut param_index = 0;

    for (i, part) in path_parts.iter().enumerate() {
        if part.is_empty() {
            if param_index < params.len() {
                if let Some(param_str) = params[param_index].as_str() {
                    path.push_str(param_str);
                } else {
                    path.push_str(&params[param_index].to_string());
                }
                param_index += 1;
            }
        } else {
            path.push_str(part);
        }

        if i < path_parts.len() - 1 {
            path.push('/');
        }
    }

    while param_index < params.len() {
        path.push('/');
        if let Some(param_str) = params[param_index].as_str() {
            path.push_str(param_str);
        } else {
            path.push_str(&params[param_index].to_string());
        }
        param_index += 1;
    }

    let result = proxy.fetch_esplora_endpoint(&path).await?;
    Ok(JsonRpcResponse::success(result, request_id.clone()))
}

/// Recursively convert string numbers to actual numbers in JSON values
/// This handles cases where clients send "20000" instead of 20000
fn convert_string_numbers(value: Value) -> Value {
    match value {
        Value::String(s) => {
            // Try to parse as u64 first, then i64, then f64
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

/// Helper to parse a u128 from JSON value (handles both string and number)
fn parse_u128(value: &Value) -> Option<u128> {
    match value {
        Value::Number(n) => n.as_u64().map(|v| v as u128),
        Value::String(s) => s.parse::<u128>().ok(),
        _ => None,
    }
}

/// Helper to convert u128 to protobuf Uint128 (lo/hi split)
fn to_uint128(value: u128) -> Uint128 {
    Uint128 {
        lo: value as u64,
        hi: (value >> 64) as u64,
    }
}

/// Helper to convert protobuf Uint128 to u128
fn from_uint128(value: &Option<Uint128>) -> u128 {
    match value {
        Some(v) => (v.hi as u128) << 64 | (v.lo as u128),
        None => 0,
    }
}

/// Helper to convert protorune's Uint128 to u128
fn from_protorune_uint128(value: &Option<ProtoruneUint128>) -> u128 {
    match value {
        Some(v) => (v.hi as u128) << 64 | (v.lo as u128),
        None => 0,
    }
}

/// Format a storage key for display (matches TypeScript formatKey)
fn format_key(key: &[u8]) -> String {
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

/// Convert AlkaneTransfer to JSON
fn alkane_transfer_to_json(transfer: &AlkaneTransfer) -> Value {
    let id = transfer.id.as_ref();
    json!({
        "id": {
            "block": from_uint128(&id.and_then(|i| i.block.clone())).to_string(),
            "tx": from_uint128(&id.and_then(|i| i.tx.clone())).to_string()
        },
        "value": from_uint128(&transfer.value).to_string()
    })
}

/// Convert KeyValuePair (storage slot) to JSON
fn storage_slot_to_json(slot: &KeyValuePair) -> Value {
    json!({
        "key": format_key(&slot.key),
        "value": format!("0x{}", hex::encode(&slot.value))
    })
}

/// Decode SimulateResponse protobuf to JSON matching TypeScript format
fn decode_simulate_response(hex_response: &str) -> Result<Value> {
    let hex_data = hex_response.strip_prefix("0x").unwrap_or(hex_response);
    let bytes = hex::decode(hex_data)?;
    let response = SimulateResponse::decode(bytes.as_slice())?;

    // Check for error
    if !response.error.is_empty() {
        return Ok(json!({
            "status": 1, // REVERT
            "gasUsed": 0,
            "execution": {
                "alkanes": [],
                "storage": [],
                "data": "0x",
                "error": response.error
            }
        }));
    }

    // Check for missing execution
    let execution = match &response.execution {
        Some(exec) => exec,
        None => {
            return Ok(json!({
                "status": 1, // REVERT
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

    // Success response
    Ok(json!({
        "status": 0, // SUCCESS
        "gasUsed": response.gas_used,
        "execution": {
            "alkanes": execution.alkanes.iter().map(alkane_transfer_to_json).collect::<Vec<_>>(),
            "storage": execution.storage.iter().map(storage_slot_to_json).collect::<Vec<_>>(),
            "data": format!("0x{}", hex::encode(&execution.data)),
            "error": null
        }
    }))
}

/// Decode OutpointResponse protobuf to JSON for protorunesbyoutpoint
/// Returns format compatible with lua script expectations:
/// { balance_sheet: { cached: { balances: [{block, tx, amount}, ...] } }, outpoint: {txid, vout}, output: {value} }
fn decode_outpoint_response(hex_response: &str) -> Result<Value> {
    let hex_data = hex_response.strip_prefix("0x").unwrap_or(hex_response);
    let bytes = hex::decode(hex_data)?;
    let response = OutpointResponse::decode(bytes.as_slice())?;

    // Convert balances to the format expected by lua script
    // Protorune uses Rune.runeId with height/txindex which maps to block/tx
    let balances: Vec<Value> = response.balances.as_ref()
        .map(|bs| {
            bs.entries.iter().map(|entry| {
                let (block, tx) = entry.rune.as_ref()
                    .and_then(|r| r.rune_id.as_ref())
                    .map(|id| {
                        // ProtoruneRuneId uses protorune's Uint128 for height/txindex
                        let height = from_protorune_uint128(&id.height);
                        let txindex = from_protorune_uint128(&id.txindex);
                        (height as u64, txindex as u64)
                    })
                    .unwrap_or((0, 0));
                let amount = from_protorune_uint128(&entry.balance);
                // Return amount as a number for JSON compatibility (WASM expects u64)
                // For amounts > u64::MAX, this will truncate but that's an edge case
                json!({
                    "block": block,
                    "tx": tx,
                    "amount": amount as u64
                })
            }).collect()
        })
        .unwrap_or_default();

    // Format txid as hex string (reversed for display)
    let txid = response.outpoint.as_ref()
        .map(|op| {
            let mut txid_bytes = op.txid.clone();
            txid_bytes.reverse(); // Bitcoin txids are displayed reversed
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

/// Decode meta response (raw UTF-8 JSON string, not protobuf)
fn decode_meta_response(hex_response: &str) -> Result<Value> {
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

/// Encode meta request: just {target: {block, tx}} -> protobuf hex
fn encode_meta_request(params: &Value) -> Result<String> {
    let obj = params.as_object()
        .ok_or_else(|| anyhow::anyhow!("meta params must be an object"))?;

    // Parse target (required)
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

    // Serialize just the AlkaneId directly
    let mut buf = Vec::new();
    alkane_id.encode(&mut buf)?;
    Ok(format!("0x{}", hex::encode(buf)))
}

/// Encode alkanes_id_to_outpoint request: {block, tx, protocolTag?} -> protobuf hex
/// Note: protocolTag is accepted but currently not used in encoding
fn encode_alkanes_id_to_outpoint_request(params: &Value) -> Result<String> {
    let obj = params.as_object()
        .ok_or_else(|| anyhow::anyhow!("alkanesidtooutpoint params must be an object"))?;

    let block = parse_u128(obj.get("block")
        .ok_or_else(|| anyhow::anyhow!("Missing 'block' parameter"))?)
        .ok_or_else(|| anyhow::anyhow!("Invalid 'block' parameter"))?;

    let tx = parse_u128(obj.get("tx")
        .ok_or_else(|| anyhow::anyhow!("Missing 'tx' parameter"))?)
        .ok_or_else(|| anyhow::anyhow!("Invalid 'tx' parameter"))?;

    // protocolTag is accepted but not currently used in the protobuf encoding
    // let _protocol_tag = obj.get("protocolTag");

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

/// Decode AlkaneIdToOutpointResponse protobuf to JSON
fn decode_alkanes_id_to_outpoint_response(hex_response: &str) -> Result<Value> {
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

/// Encode trace request: {txid, vout} -> protobuf hex
fn encode_trace_request(params: &Value) -> Result<String> {
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

/// Encode protorunesbyoutpoint request from params to protobuf hex string.
///
/// Accepts either:
///   - Positional params: [txid_hex, vout, block_tag?, protocol_tag?]
///   - Object param: [{ txid: "...", vout: N, protocolTag?: "1" }, block_tag?]
///
/// protocol_tag defaults to 1 (alkanes) if not provided.
/// Returns the hex-encoded OutpointWithProtocol protobuf.
fn encode_protorunesbyoutpoint_request(params: &[Value]) -> Result<String> {
    let input = params.get(0)
        .ok_or_else(|| anyhow::anyhow!("protorunesbyoutpoint requires at least one parameter"))?;

    let (txid_hex, vout, protocol_tag) = if let Some(txid_str) = input.as_str() {
        // Positional format: [txid, vout, block_tag?, protocol_tag?]
        let vout = params.get(1)
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;
        let pt = params.get(3)
            .and_then(|v| v.as_u64().or_else(|| v.as_str().and_then(|s| s.parse::<u64>().ok())))
            .unwrap_or(1) as u128;
        (txid_str.to_string(), vout, pt)
    } else if let Some(obj) = input.as_object() {
        // Object format: { txid: "...", vout: N, protocolTag?: "1" }
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

    // Bitcoin txids are displayed in reverse byte order, so we need to reverse to get internal format
    let mut txid_bytes = hex::decode(&txid_hex)
        .map_err(|e| anyhow::anyhow!("Invalid txid hex: {}", e))?;
    txid_bytes.reverse(); // Convert from display format to internal little-endian format

    // Build OutpointWithProtocol
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

/// Encode protorunesbyaddress request from params to protobuf hex string.
///
/// Accepts either:
///   - A plain string address: params[0] = "bcrt1p..."
///   - A JSON object: params[0] = {"address": "bcrt1p...", "protocolTag": "1"}
///
/// protocol_tag defaults to 1 (alkanes) if not provided.
/// Returns the hex-encoded ProtorunesWalletRequest protobuf.
fn encode_protorunesbyaddress_request(params: &[Value]) -> Result<String> {
    let input = params.get(0).ok_or_else(|| anyhow::anyhow!("protorunesbyaddress requires an address parameter"))?;

    let (address, protocol_tag) = if let Some(addr_str) = input.as_str() {
        // Plain string address, protocol_tag from params[2] or default 1
        let pt = params.get(2)
            .and_then(|v| v.as_u64().or_else(|| v.as_str().and_then(|s| s.parse::<u64>().ok())))
            .unwrap_or(1) as u128;
        (addr_str.to_string(), pt)
    } else if let Some(obj) = input.as_object() {
        // JSON object: {"address": "...", "protocolTag": "1"}
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

/// Decode WalletResponse protobuf into JSON.
///
/// Returns an object with:
///   - outpoints: array of {balances, outpoint, output, height, txindex}
///   - balances: aggregated balance sheet {entries: [{block, tx, amount}]}
fn decode_wallet_response(hex_response: &str) -> Result<Value> {
    let hex_data = hex_response.strip_prefix("0x").unwrap_or(hex_response);
    if hex_data.is_empty() {
        return Ok(json!({ "outpoints": [], "balances": { "entries": [] } }));
    }
    let bytes = hex::decode(hex_data)?;
    if bytes.is_empty() {
        return Ok(json!({ "outpoints": [], "balances": { "entries": [] } }));
    }
    let response = WalletResponse::decode(bytes.as_slice())?;

    // Decode each outpoint (reuse the same logic as decode_outpoint_response)
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

    // Aggregate balance sheet
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

/// Convert call type enum to string (matches TypeScript fromCallType)
fn call_type_to_string(call_type: i32) -> &'static str {
    match call_type {
        1 => "call",
        2 => "delegatecall",
        3 => "staticcall",
        _ => "unknowncall",
    }
}

/// Convert AlkanesTraceEvent to JSON (matches TypeScript toEvent)
fn trace_event_to_json(event: &AlkanesTraceEvent) -> Value {
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

/// Decode AlkanesTrace protobuf to JSON array
fn decode_trace_response(hex_response: &str) -> Result<Value> {
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

/// Encode simulate request JSON to protobuf hex string
/// JSON format: { alkanes, transaction, block, height, txindex, target: {block, tx}, inputs, vout, pointer, refundPointer }
fn encode_simulate_request(params: &Value) -> Result<String> {
    let obj = params.as_object()
        .ok_or_else(|| anyhow::anyhow!("simulate params must be an object"))?;

    // Parse alkanes transfers (optional, defaults to empty)
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

    // Parse transaction (hex string, optional)
    let transaction: Vec<u8> = obj.get("transaction")
        .and_then(|v| v.as_str())
        .map(|s| {
            let s = s.strip_prefix("0x").unwrap_or(s);
            hex::decode(s).unwrap_or_default()
        })
        .unwrap_or_default();

    // Parse block (hex string, optional)
    let block: Vec<u8> = obj.get("block")
        .and_then(|v| v.as_str())
        .map(|s| {
            let s = s.strip_prefix("0x").unwrap_or(s);
            hex::decode(s).unwrap_or_default()
        })
        .unwrap_or_default();

    // Parse height
    let height = obj.get("height")
        .and_then(|v| match v {
            Value::Number(n) => n.as_u64(),
            Value::String(s) => s.parse::<u64>().ok(),
            _ => None,
        })
        .unwrap_or(0);

    // Parse txindex
    let txindex = obj.get("txindex")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;

    // Parse target and inputs to create calldata
    // calldata = encipher([target.block, target.tx, ...inputs])
    let mut calldata_values: Vec<u128> = Vec::new();

    if let Some(target) = obj.get("target") {
        // Handle target as object {block, tx} or string "block:tx"
        if let Some(target_obj) = target.as_object() {
            if let Some(block) = parse_u128(target_obj.get("block").unwrap_or(&Value::Null)) {
                calldata_values.push(block);
            }
            if let Some(tx) = parse_u128(target_obj.get("tx").unwrap_or(&Value::Null)) {
                calldata_values.push(tx);
            }
        } else if let Some(target_str) = target.as_str() {
            // Parse "block:tx" format
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

    // Parse inputs array
    if let Some(inputs) = obj.get("inputs").and_then(|v| v.as_array()) {
        for input in inputs {
            if let Some(val) = parse_u128(input) {
                calldata_values.push(val);
            }
        }
    }

    let calldata = encode_varint_list(&calldata_values);

    // Parse vout, pointer, refund_pointer
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

    // Create the protobuf message
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

    // Serialize to bytes and hex-encode
    let mut buf = Vec::new();
    parcel.encode(&mut buf)?;

    Ok(format!("0x{}", hex::encode(buf)))
}

async fn handle_alkanes_method(
    method: &str,
    params: &[Value],
    request_id: &Value,
    proxy: &ProxyClient,
) -> Result<JsonRpcResponse> {
    // The alkanes namespace methods should be forwarded to metashrew_view
    // following the same pattern as the TypeScript implementation:
    // metashrew_view(method_name, protobuf_hex_input, block_tag)

    let input = params.get(0).cloned().unwrap_or(Value::Null);

    // For protorunesbyoutpoint with positional params [txid, vout, block_tag, protocol_tag],
    // block_tag is at index 2. With object params [{txid, vout, protocolTag?}, block_tag],
    // block_tag is at index 1 (same as other methods).
    let block_tag = if method == "protorunesbyoutpoint" {
        let first = params.get(0);
        if first.map_or(false, |v| v.is_object()) {
            params.get(1)
                .and_then(|v| v.as_str())
                .unwrap_or("latest")
        } else {
            params.get(2)
                .and_then(|v| v.as_str())
                .unwrap_or("latest")
        }
    } else {
        params.get(1)
            .and_then(|v| v.as_str())
            .unwrap_or("latest")
    };

    // Encode request based on method type
    let (method_name, encoded_input, needs_decode) = match method {
        "simulate" => {
            match encode_simulate_request(&input) {
                Ok(hex) => ("simulate", Value::String(hex), "simulate"),
                Err(e) => {
                    return Ok(JsonRpcResponse::error(
                        INTERNAL_ERROR,
                        format!("Failed to encode simulate request: {}", e),
                        request_id.clone(),
                    ));
                }
            }
        }
        "meta" => {
            match encode_meta_request(&input) {
                Ok(hex) => ("meta", Value::String(hex), "meta"),
                Err(e) => {
                    return Ok(JsonRpcResponse::error(
                        INTERNAL_ERROR,
                        format!("Failed to encode meta request: {}", e),
                        request_id.clone(),
                    ));
                }
            }
        }
        "alkanesidtooutpoint" | "alkanes_id_to_outpoint" => {
            match encode_alkanes_id_to_outpoint_request(&input) {
                Ok(hex) => ("alkanes_id_to_outpoint", Value::String(hex), "alkanesidtooutpoint"),
                Err(e) => {
                    return Ok(JsonRpcResponse::error(
                        INTERNAL_ERROR,
                        format!("Failed to encode alkanesidtooutpoint request: {}", e),
                        request_id.clone(),
                    ));
                }
            }
        }
        "trace" => {
            match encode_trace_request(&input) {
                Ok(hex) => ("trace", Value::String(hex), "trace"),
                Err(e) => {
                    return Ok(JsonRpcResponse::error(
                        INTERNAL_ERROR,
                        format!("Failed to encode trace request: {}", e),
                        request_id.clone(),
                    ));
                }
            }
        }
        "protorunesbyoutpoint" => {
            match encode_protorunesbyoutpoint_request(params) {
                Ok(hex) => ("protorunesbyoutpoint", Value::String(hex), "protorunesbyoutpoint"),
                Err(e) => {
                    return Ok(JsonRpcResponse::error(
                        INTERNAL_ERROR,
                        format!("Failed to encode protorunesbyoutpoint request: {}", e),
                        request_id.clone(),
                    ));
                }
            }
        }
        "protorunesbyaddress" => {
            match encode_protorunesbyaddress_request(params) {
                Ok(hex) => ("protorunesbyaddress", Value::String(hex), "protorunesbyaddress"),
                Err(e) => {
                    return Ok(JsonRpcResponse::error(
                        INTERNAL_ERROR,
                        format!("Failed to encode protorunesbyaddress request: {}", e),
                        request_id.clone(),
                    ));
                }
            }
        }
        _ => (method, convert_string_numbers(input), "none")
    };

    let modified_request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: "metashrew_view".to_string(),
        params: vec![
            Value::String(method_name.to_string()),
            encoded_input,
            Value::String(block_tag.to_string()),
        ],
        id: request_id.clone(),
    };

    let response = proxy.forward_to_metashrew(&modified_request).await?;

    // Decode response if needed
    if needs_decode != "none" {
        if let JsonRpcResponse::Success { result, .. } = &response {
            if let Some(hex_str) = result.as_str() {
                let decoded = match needs_decode {
                    "simulate" => decode_simulate_response(hex_str),
                    "meta" => decode_meta_response(hex_str),
                    "alkanesidtooutpoint" => decode_alkanes_id_to_outpoint_response(hex_str),
                    "trace" => decode_trace_response(hex_str),
                    "protorunesbyoutpoint" => decode_outpoint_response(hex_str),
                    "protorunesbyaddress" => decode_wallet_response(hex_str),
                    _ => unreachable!()
                };

                match decoded {
                    Ok(json_result) => {
                        return Ok(JsonRpcResponse::success(json_result, request_id.clone()));
                    }
                    Err(e) => {
                        return Ok(JsonRpcResponse::error(
                            INTERNAL_ERROR,
                            format!("Failed to decode {} response: {}", method, e),
                            request_id.clone(),
                        ));
                    }
                }
            }
        }
    }

    Ok(response)
}

async fn handle_metashrew_method(
    request: &JsonRpcRequest,
    proxy: &ProxyClient,
) -> Result<JsonRpcResponse> {
    proxy.forward_to_metashrew(request).await
}

async fn handle_memshrew_method(
    request: &JsonRpcRequest,
    proxy: &ProxyClient,
) -> Result<JsonRpcResponse> {
    proxy.forward_to_memshrew(request).await
}

async fn handle_subfrost_method(
    request: &JsonRpcRequest,
    proxy: &ProxyClient,
) -> Result<JsonRpcResponse> {
    proxy.forward_to_subfrost(request).await
}

async fn handle_bitcoind_method(
    request: &JsonRpcRequest,
    proxy: &ProxyClient,
) -> Result<JsonRpcResponse> {
    let method_parts: Vec<&str> = request.method.split('_').collect();
    let actual_method = method_parts[method_parts.len() - 1];

    // Guard for generatetoaddress: cap at 1 block
    let params = if actual_method == "generatetoaddress" && !request.params.is_empty() {
        let mut modified_params = request.params.clone();
        // First parameter is number of blocks (nblocks)
        if let Some(nblocks) = modified_params[0].as_u64() {
            if nblocks > 1 {
                // Cap at 1 block
                modified_params[0] = json!(1);
            }
        } else if let Some(nblocks) = modified_params[0].as_i64() {
            if nblocks > 1 {
                // Cap at 1 block
                modified_params[0] = json!(1);
            }
        }
        modified_params
    } else {
        request.params.clone()
    };

    let modified_request = JsonRpcRequest {
        jsonrpc: request.jsonrpc.clone(),
        method: actual_method.to_string(),
        params,
        id: request.id.clone(),
    };

    proxy.forward_to_bitcoind(&modified_request).await
}
