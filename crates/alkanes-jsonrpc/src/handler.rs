//! alkanes-jsonrpc Handler Module
//!
//! ## Architecture Overview
//! This module routes JSON-RPC requests to various backend services:
//! - `ord_*` → ord service (ordinals indexer)
//! - `esplora_*` → esplora service (blockchain explorer)
//! - `alkanes_*` → metashrew_view conversions OR special handlers (like alkanes_simulate)
//! - `metashrew_*` → metashrew service (alkanes runtime)
//! - `memshrew_*` → memshrew service (mempool runtime)
//! - `lua_*` / `sandshrew_*` → sandshrew lua executor
//! - `btc_*` → bitcoind
//! - `subfrost_*` → subfrost-rpc service (FROST signing)
//! - everything else → bitcoind (default fallback)
//!
//! ## Special Handlers
//! ### alkanes_simulate
//! The `alkanes_simulate` method is a special case that requires custom handling.
//! Unlike other `alkanes_*` methods that simply convert to `metashrew_view` calls,
//! this method must:
//! 1. Parse SimulateRequest structure from alkanes-data-api
//! 2. Build MessageContextParcel with LEB128-encoded calldata
//! 3. Call metashrew_view("simulate", protobuf, "latest")
//! 4. Wrap response in SimulateResponse structure expected by alkanes-data-api
//!
//! This handler is critical for regtest environments where database indexing
//! may not be available, enabling pool queries via RPC simulation.
//!
//! ## Historical Context
//! This implementation was developed to solve the "/get-pools 500 error" issue
//! where alkanes-data-api couldn't query pools because:
//! - The REST endpoint /v4/subfrost/get-pools routes through alkanes-data-api
//! - alkanes-data-api calls alkanes_rpc.simulate() which makes alkanes_simulate RPC
//! - This RPC method wasn't implemented in alkanes-jsonrpc
//! - Without it, pool discovery failed, causing accidental pool creation
//!
//! The solution involved understanding the complete request flow:
//! Browser → OpenResty → alkanes-data-api:3000 → alkanes-jsonrpc:18888 → metashrew

use crate::jsonrpc::{JsonRpcRequest, JsonRpcResponse, INTERNAL_ERROR, INVALID_PARAMS, METHOD_NOT_FOUND};
use crate::proxy::ProxyClient;
use crate::sandshrew;
use anyhow::Result;
use prost::Message;
use serde_json::Value;

// Import protobuf types for encoding alkanes RPC params
use alkanes_cli_common::proto::protorune as protorune_pb;

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

    // Handle alkanes_simulate before namespace routing
    if request.method == "alkanes_simulate" {
        return handle_alkanes_simulate(request, proxy).await;
    }

    match namespace {
        "ord" => handle_ord_method(&method_name, &request.params, &request.id, proxy).await,
        "esplora" => handle_esplora_method(&method_name, &request.params, &request.id, proxy).await,
        "alkanes" => handle_alkanes_method(&method_name, &request.params, &request.id, proxy).await,
        "metashrew" => handle_metashrew_method(request, proxy).await,
        "memshrew" => handle_memshrew_method(request, proxy).await,
        "lua" => sandshrew::handle_sandshrew_method(&method_name, &request.params, &request.id, proxy, script_storage).await,
        "sandshrew" => sandshrew::handle_sandshrew_method(&method_name, &request.params, &request.id, proxy, script_storage).await,
        "btc" => handle_bitcoind_method(request, proxy).await,
        "subfrost" => handle_subfrost_method(request, proxy).await,
        _ => handle_bitcoind_method(request, proxy).await,
    }
}

/// Forward subfrost_* methods to the subfrost-rpc service
async fn handle_subfrost_method(
    request: &JsonRpcRequest,
    proxy: &ProxyClient,
) -> Result<JsonRpcResponse> {
    proxy.forward_to_subfrost(request).await
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

async fn handle_alkanes_method(
    method: &str,
    params: &[Value],
    request_id: &Value,
    proxy: &ProxyClient,
) -> Result<JsonRpcResponse> {
    // The alkanes namespace methods should be forwarded to metashrew_view
    // following the same pattern as the TypeScript implementation:
    // metashrew_view(method_name, input, block_tag)

    // Special handling for protorunesbyoutpoint - called from Lua with different params format:
    // _RPC.protorunes_by_outpoint(txid, vout, block_tag, protocol_tag)
    // params: [txid, vout, block_tag, protocol_tag]
    // This is used by batch_utxo_balances.lua to fetch alkane balances per UTXO
    if method == "protorunesbyoutpoint" {
        return handle_protorunesbyoutpoint(params, request_id, proxy).await;
    }

    let input = params.get(0).cloned().unwrap_or(Value::Null);
    let block_tag = params.get(1)
        .and_then(|v| v.as_str())
        .unwrap_or("latest");

    // For protorunesbyaddress, we need to encode JSON params to protobuf
    // The input can be either:
    // 1. Already hex-encoded protobuf (starts with "0x")
    // 2. JSON object with { address, protocolTag } that needs encoding
    let encoded_input = if method == "protorunesbyaddress" {
        encode_protorunesbyaddress_input(&input)?
    } else {
        input
    };

    let modified_request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: "metashrew_view".to_string(),
        params: vec![
            Value::String(method.to_string()),
            encoded_input,
            Value::String(block_tag.to_string()),
        ],
        id: request_id.clone(),
    };

    proxy.forward_to_metashrew(&modified_request).await
}

/// Encode protorunesbyaddress input to hex-encoded protobuf
/// Accepts either:
/// - Already encoded hex string (passed through)
/// - JSON object { address: string, protocolTag: string }
fn encode_protorunesbyaddress_input(input: &Value) -> Result<Value> {
    // If input is already a hex string, pass it through
    if let Some(s) = input.as_str() {
        if s.starts_with("0x") {
            return Ok(input.clone());
        }
    }

    // Extract address and protocolTag from JSON object
    let address = input.get("address")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("protorunesbyaddress requires 'address' field"))?;

    let protocol_tag_str = input.get("protocolTag")
        .and_then(|v| v.as_str())
        .unwrap_or("1");

    let protocol_tag: u128 = protocol_tag_str.parse()
        .unwrap_or(1);

    // Build protobuf request
    let request = protorune_pb::ProtorunesWalletRequest {
        wallet: address.as_bytes().to_vec(),
        protocol_tag: Some(protorune_pb::Uint128 {
            lo: protocol_tag as u64,
            hi: (protocol_tag >> 64) as u64,
        }),
    };

    // Encode to hex
    let encoded = request.encode_to_vec();
    let hex_input = format!("0x{}", hex::encode(encoded));

    Ok(Value::String(hex_input))
}

/// Handle protorunesbyoutpoint RPC method
///
/// ## Purpose
/// This handler enables Lua scripts (batch_utxo_balances.lua) to query alkane
/// balances for specific UTXOs. It's critical for the WASM execute flow to
/// correctly identify which UTXOs contain the required alkanes for a transaction.
///
/// ## Parameter Format (from Lua)
/// Called as: _RPC.protorunes_by_outpoint(txid, vout, block_tag, protocol_tag)
/// - params[0]: txid (string) - transaction ID in hex
/// - params[1]: vout (number) - output index
/// - params[2]: block_tag (string or null) - block height or "latest"
/// - params[3]: protocol_tag (number) - protocol ID (1 for alkanes)
///
/// ## Implementation
/// 1. Parse txid, vout, and protocol_tag from params
/// 2. Build OutpointWithProtocol protobuf message
/// 3. Call metashrew_view("protorunesbyoutpoint", encoded_protobuf, block_tag)
/// 4. Return the OutpointResponse which includes balance_sheet
///
/// ## Historical Context
/// This was implemented to fix the "Insufficient alkanes: have 0" error where
/// batch_utxo_balances.lua couldn't find alkane balances because the RPC method
/// wasn't properly encoding the outpoint query into protobuf format.
async fn handle_protorunesbyoutpoint(
    params: &[Value],
    request_id: &Value,
    proxy: &ProxyClient,
) -> Result<JsonRpcResponse> {
    // Support both positional params ["txid", vout] and object params [{"txid": "...", "vout": 0}]
    let (txid_hex, vout, block_tag, protocol_tag): (&str, u32, &str, u128) =
        if let Some(obj) = params.get(0).and_then(|v| v.as_object()) {
            // Object format: [{"txid": "...", "vout": 0, "block_tag": "latest", "protocol_tag": 1}]
            let txid = obj.get("txid").and_then(|v| v.as_str()).unwrap_or("");
            let vout = obj.get("vout").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            let block = obj.get("block_tag").and_then(|v| v.as_str()).unwrap_or("latest");
            let proto = obj.get("protocol_tag").and_then(|v| v.as_u64()).map(|v| v as u128).unwrap_or(1);
            (txid, vout, block, proto)
        } else {
            // Positional format: ["txid", vout, "block_tag", protocol_tag]
            let txid = params.get(0).and_then(|v| v.as_str()).unwrap_or("");
            let vout = params.get(1).and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            let block = params.get(2).and_then(|v| v.as_str()).unwrap_or("latest");
            let proto = params.get(3).and_then(|v| v.as_u64()).map(|v| v as u128).unwrap_or(1);
            (txid, vout, block, proto)
        };

    // Parse txid hex to bytes
    // Bitcoin txids are displayed in reverse byte order (big-endian display, little-endian internal)
    // Esplora returns display format, metashrew stores in internal format, so we reverse
    let txid_bytes: Vec<u8> = if txid_hex.is_empty() {
        vec![]
    } else {
        let mut bytes = hex::decode(txid_hex).unwrap_or_default();
        bytes.reverse(); // Convert display format -> internal format
        bytes
    };

    // Build OutpointWithProtocol protobuf message
    let request = protorune_pb::OutpointWithProtocol {
        txid: txid_bytes,
        vout,
        protocol: Some(protorune_pb::Uint128 {
            lo: protocol_tag as u64,
            hi: (protocol_tag >> 64) as u64,
        }),
    };

    // Encode to hex
    let encoded = request.encode_to_vec();
    let hex_input = format!("0x{}", hex::encode(encoded));

    // Forward to metashrew_view
    let modified_request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: "metashrew_view".to_string(),
        params: vec![
            Value::String("protorunesbyoutpoint".to_string()),
            Value::String(hex_input),
            Value::String(block_tag.to_string()),
        ],
        id: request_id.clone(),
    };

    let metashrew_response = proxy.forward_to_metashrew(&modified_request).await?;

    // Decode the protobuf response to JSON for Lua scripts
    // metashrew_view returns hex-encoded protobuf, but Lua expects JSON
    match metashrew_response {
        JsonRpcResponse::Success { result, .. } => {
            // Extract hex string from result
            let hex_str = result.as_str().unwrap_or("0x");
            let hex_data = hex_str.strip_prefix("0x").unwrap_or(hex_str);

            if hex_data.is_empty() {
                // Empty result - return empty balance sheet with debug info
                let json_response = serde_json::json!({
                    "balance_sheet": {
                        "cached": {
                            "balances": {}
                        }
                    },
                    "outpoint": {
                        "txid": txid_hex,
                        "vout": vout
                    },
                    "_debug": {
                        "raw_response": "empty",
                        "note": "metashrew returned empty response"
                    }
                });
                return Ok(JsonRpcResponse::success(json_response, request_id.clone()));
            }

            // Decode protobuf OutpointResponse
            let bytes = hex::decode(hex_data).unwrap_or_default();
            match protorune_pb::OutpointResponse::decode(bytes.as_slice()) {
                Ok(outpoint_response) => {
                    // Convert to JSON structure expected by Lua
                    // The Lua script iterates: for alkane_id, amount in pairs(balances)
                    // and accesses alkane_id.block, alkane_id.tx
                    // This expects the VALUE (not key) to have block/tx fields
                    // We use numeric keys (array indices) so pairs() iterates properly
                    let mut balances_array = Vec::new();

                    let has_balance_sheet = outpoint_response.balances.is_some();
                    if let Some(balance_sheet) = outpoint_response.balances {
                        for entry in balance_sheet.entries {
                            if let Some(rune) = entry.rune {
                                if let Some(rune_id) = rune.rune_id {
                                    // Extract values from uint128 structs
                                    // ProtoruneRuneId has height and txindex as uint128
                                    let block = rune_id.height.map(|h| h.lo).unwrap_or(0);
                                    let tx = rune_id.txindex.map(|t| t.lo).unwrap_or(0);

                                    // Extract balance amount from uint128
                                    let amount = if let Some(balance) = entry.balance {
                                        ((balance.hi as u128) << 64) | (balance.lo as u128)
                                    } else {
                                        0u128
                                    };
                                    // Store as object with block, tx, amount fields
                                    // The Lua pairs() will give: idx, { block, tx, amount }
                                    balances_array.push(serde_json::json!({
                                        "block": block,
                                        "tx": tx,
                                        "amount": amount.to_string()
                                    }));
                                }
                            }
                        }
                    }

                    // CRITICAL: The Lua script expects balance_sheet.cached.balances
                    // where iteration gives (alkane_id, amount) pairs
                    // Since Lua pairs() on array gives (idx, table), and script accesses
                    // alkane_id.block (where alkane_id is actually the full entry table),
                    // we return as array - but the script logic is still wrong.
                    //
                    // Alternative format: return as map where value contains block/tx/amount
                    // This matches how the script SHOULD work if it accessed value.block
                    let mut balances_map = serde_json::Map::new();
                    for (idx, entry) in balances_array.iter().enumerate() {
                        balances_map.insert(idx.to_string(), entry.clone());
                    }

                    let json_response = serde_json::json!({
                        "balance_sheet": {
                            "cached": {
                                "balances": balances_map
                            }
                        },
                        "outpoint": {
                            "txid": txid_hex,
                            "vout": vout
                        },
                        "height": outpoint_response.height,
                        "txindex": outpoint_response.txindex,
                        "entries_count": balances_array.len(),
                        "_debug": {
                            "raw_response_len": hex_data.len(),
                            "decoded_bytes_len": bytes.len(),
                            "has_balance_sheet": has_balance_sheet
                        }
                    });

                    Ok(JsonRpcResponse::success(json_response, request_id.clone()))
                }
                Err(e) => {
                    // Failed to decode - return empty with error info
                    let json_response = serde_json::json!({
                        "balance_sheet": {
                            "cached": {
                                "balances": {}
                            }
                        },
                        "decode_error": e.to_string(),
                        "_debug": {
                            "raw_response_len": hex_data.len(),
                            "raw_response_preview": if hex_data.len() > 100 { &hex_data[..100] } else { hex_data },
                            "decoded_bytes_len": bytes.len()
                        }
                    });
                    Ok(JsonRpcResponse::success(json_response, request_id.clone()))
                }
            }
        }
        JsonRpcResponse::Error { error, .. } => {
            // Forward error but still return valid JSON structure for Lua
            let json_response = serde_json::json!({
                "balance_sheet": {
                    "cached": {
                        "balances": {}
                    }
                },
                "error": error.message
            });
            Ok(JsonRpcResponse::success(json_response, request_id.clone()))
        }
    }
}

/// Handle alkanes_simulate RPC method
///
/// ## Purpose
/// This handler enables alkanes-data-api to query pool data via RPC simulation
/// without relying on database indexing. It's critical for regtest environments
/// where the database indexer may not be running.
///
/// ## Request Format
/// Accepts SimulateRequest from alkanes-data-api:
/// ```json
/// {
///   "target": { "block": "4", "tx": "65522" },  // Contract to simulate
///   "inputs": ["3"]                              // Opcode + params (3 = GET_ALL_POOLS)
/// }
/// ```
///
/// ## Implementation Details
/// 1. Parses target contract ID (block:tx) and inputs (opcodes/params)
/// 2. Builds MessageContextParcel with LEB128-encoded calldata:
///    - Encodes: [target_block, target_tx, ...inputs]
///    - LEB128 is variable-length integer encoding used by alkanes protocol
/// 3. Protobuf-encodes the MessageContextParcel
/// 4. Calls metashrew_view("simulate", hex_parcel, "latest")
///    - CRITICAL: View function is "simulate", NOT "{contract_id}/simulate"
///    - The contract_id is encoded IN the protobuf, not in the view path
/// 5. Wraps the raw metashrew response in SimulateResponse structure
///
/// ## Response Format
/// Returns SimulateResponse expected by alkanes-data-api:
/// ```json
/// {
///   "execution": {
///     "data": "0x...",      // Hex-encoded result
///     "error": null,        // Error message if any
///     "alkanes": [],        // Alkane transfers
///     "storage": []         // Storage changes
///   },
///   "gasUsed": 0,
///   "status": 1             // 1 = success
/// }
/// ```
///
/// ## Historical Context & Debugging Notes
/// - Initially tried calling "{contract_id}/simulate" - WRONG! The simulate
///   view function is a global metashrew runtime feature, not contract-specific.
/// - First implementation returned raw hex string - alkanes-data-api expects
///   SimulateResponse structure with execution.data field.
/// - The response must match SimulateResponse in alkanes-data-api/src/services/alkanes_rpc.rs
///
/// ## Related Code
/// - alkanes-data-api: src/services/alkanes_rpc.rs (SimulateResponse struct)
/// - alkanes-cli-common: src/alkanes/amm.rs (reference implementation)
/// - alkanes-web-sys: src/provider.rs (working WASM implementation)
async fn handle_alkanes_simulate(
    request: &JsonRpcRequest,
    proxy: &ProxyClient,
) -> Result<JsonRpcResponse> {
    use alkanes_cli_common::proto::alkanes as alkanes_pb;

    // Parse SimulateRequest from params
    let params = request.params.get(0)
        .ok_or_else(|| anyhow::anyhow!("Missing simulate request parameter"))?;

    // Extract target and inputs from the SimulateRequest
    let target = params.get("target")
        .ok_or_else(|| anyhow::anyhow!("Missing 'target' field in SimulateRequest"))?;
    let inputs = params.get("inputs")
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow::anyhow!("Missing or invalid 'inputs' field"))?;

    let target_block_str = target.get("block")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing 'block' in target"))?;
    let target_tx_str = target.get("tx")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing 'tx' in target"))?;

    let target_block: u64 = target_block_str.parse()
        .map_err(|_| anyhow::anyhow!("Invalid target block number"))?;
    let target_tx: u64 = target_tx_str.parse()
        .map_err(|_| anyhow::anyhow!("Invalid target tx number"))?;

    // Build MessageContextParcel with LEB128-encoded calldata
    // LEB128 (Little Endian Base 128) is a variable-length integer encoding
    // used throughout the alkanes protocol for compact serialization
    let mut calldata = Vec::new();

    // Encode target block:tx into calldata
    // This tells the simulator which contract to execute
    leb128::write::unsigned(&mut calldata, target_block)
        .map_err(|e| anyhow::anyhow!("Failed to encode target block: {}", e))?;
    leb128::write::unsigned(&mut calldata, target_tx)
        .map_err(|e| anyhow::anyhow!("Failed to encode target tx: {}", e))?;

    // Encode inputs (opcodes and parameters)
    // For GET_ALL_POOLS: inputs = ["3"] where 3 is the opcode
    for input in inputs {
        let val: u64 = if let Some(val_str) = input.as_str() {
            val_str.parse()
                .map_err(|_| anyhow::anyhow!("Failed to parse input as u64: {}", val_str))?
        } else if let Some(val_u64) = input.as_u64() {
            val_u64
        } else {
            return Err(anyhow::anyhow!("Input must be string or number"));
        };

        leb128::write::unsigned(&mut calldata, val)
            .map_err(|e| anyhow::anyhow!("Failed to encode input: {}", e))?;
    }

    // Build MessageContextParcel
    // This is the protobuf message that contains all context for simulation
    let context = alkanes_pb::MessageContextParcel {
        alkanes: vec![],      // No alkane transfers needed for view calls
        transaction: vec![],  // No transaction data needed
        block: vec![],        // No block data needed
        height: 0,            // Height not needed for simulation
        vout: 0,              // Output index not needed
        txindex: 0,           // TX index not needed
        calldata,             // The LEB128-encoded contract call data
        pointer: 0,           // Memory pointer (not used for view calls)
        refund_pointer: 0,    // Refund pointer (not used for view calls)
    };

    // Encode to protobuf
    let mut buf = Vec::new();
    context.encode(&mut buf)?;

    // Build metashrew_view request
    // CRITICAL: The view function is just "simulate", NOT "{contract_id}/simulate"
    // The contract_id is already encoded in the MessageContextParcel protobuf
    // This was a key insight from debugging - the simulate view function is a
    // global metashrew runtime feature that reads the contract ID from the parcel
    let params_hex = format!("0x{}", hex::encode(&buf));

    let modified_request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: "metashrew_view".to_string(),
        params: vec![
            Value::String("simulate".to_string()),  // View function name
            Value::String(params_hex),               // Hex-encoded protobuf parcel
            Value::String("latest".to_string()),     // Block tag
        ],
        id: request.id.clone(),
    };

    // Forward to metashrew and get raw response
    let metashrew_response = proxy.forward_to_metashrew(&modified_request).await?;

    // Transform the response into SimulateResponse format expected by alkanes-data-api
    // metashrew_view returns JsonRpcResponse enum: Success { result: "0x..." } or Error { error: ... }
    //
    // CRITICAL: The metashrew "simulate" view returns a protobuf-encoded ExtendedCallResponse,
    // NOT raw hex data. We must decode the protobuf to extract the actual `data` field.
    //
    // ExtendedCallResponse protobuf structure (from alkanes.proto):
    //   message ExtendedCallResponse {
    //     repeated AlkaneTransfer alkanes = 1;
    //     repeated KeyValuePair storage = 2;
    //     bytes data = 3;  <-- This is what we need to extract
    //   }
    //
    // We need to wrap the extracted data in SimulateResponse structure:
    // { "execution": { "data": "0x...", "error": null, ... }, "gasUsed": 0, "status": 1 }
    match metashrew_response {
        JsonRpcResponse::Success { result, .. } => {
            // Extract the hex data from metashrew response
            let raw_hex = if let Some(s) = result.as_str() {
                s.to_string()
            } else {
                // If result is not a string, serialize it
                serde_json::to_string(&result)?
            };

            // Decode the protobuf ExtendedCallResponse to extract the actual data field
            // The raw_hex contains the full protobuf, we need to extract field 3 (data)
            //
            // ExtendedCallResponse protobuf structure:
            //   field 1 (alkanes): repeated AlkaneTransfer, tag = 0x0a
            //   field 2 (storage): repeated KeyValuePair, tag = 0x12
            //   field 3 (data): bytes, tag = 0x1a
            //
            // We need to find field 3 and extract its value.
            let clean_hex = raw_hex.strip_prefix("0x").unwrap_or(&raw_hex);
            let (data_hex, alkanes_vec, storage_vec) = if let Ok(proto_bytes) = hex::decode(clean_hex) {
                // Try to decode as ExtendedCallResponse using prost
                use prost::Message;
                if let Ok(response) = alkanes_pb::ExtendedCallResponse::decode(&proto_bytes[..]) {
                    // Extract the data field and convert back to hex
                    let extracted_data = format!("0x{}", hex::encode(&response.data));

                    // Also extract alkanes transfers for completeness
                    let alkanes: Vec<Value> = response.alkanes.iter().map(|transfer| {
                        let id = transfer.id.as_ref().map(|id| {
                            let block = id.block.as_ref().map(|b| b.lo).unwrap_or(0);
                            let tx = id.tx.as_ref().map(|t| t.lo).unwrap_or(0);
                            serde_json::json!({ "block": block, "tx": tx })
                        }).unwrap_or(serde_json::json!(null));
                        let amount = transfer.value.as_ref()
                            .map(|v| ((v.hi as u128) << 64) | (v.lo as u128))
                            .unwrap_or(0);
                        serde_json::json!({
                            "id": id,
                            "amount": amount.to_string()
                        })
                    }).collect();

                    // Extract storage changes
                    let storage: Vec<Value> = response.storage.iter().map(|kv| {
                        serde_json::json!({
                            "key": format!("0x{}", hex::encode(&kv.key)),
                            "value": format!("0x{}", hex::encode(&kv.value))
                        })
                    }).collect();

                    (extracted_data, alkanes, storage)
                } else {
                    // Prost decode failed - manually extract field 3 (data)
                    // Field 3 with wire type 2 (length-delimited) has tag 0x1a
                    log::debug!("Prost decode failed, manually extracting field 3 from protobuf");

                    let mut extracted_data = None;
                    let mut i = 0;
                    while i < proto_bytes.len() {
                        // Read field tag
                        let tag = proto_bytes[i];
                        i += 1;

                        if i >= proto_bytes.len() {
                            break;
                        }

                        // Read length (varint)
                        let mut length: usize = 0;
                        let mut shift = 0;
                        loop {
                            if i >= proto_bytes.len() {
                                break;
                            }
                            let b = proto_bytes[i];
                            i += 1;
                            length |= ((b & 0x7F) as usize) << shift;
                            if b & 0x80 == 0 {
                                break;
                            }
                            shift += 7;
                        }

                        // Check if this is field 3 (tag 0x1a)
                        if tag == 0x1a {
                            // This is the data field
                            if i + length <= proto_bytes.len() {
                                let data = &proto_bytes[i..i + length];
                                extracted_data = Some(format!("0x{}", hex::encode(data)));
                                log::debug!("Extracted data field: {} bytes", length);
                            }
                            break;
                        }

                        // Skip this field's data
                        i += length;
                    }

                    match extracted_data {
                        Some(data) => (data, vec![], vec![]),
                        None => {
                            log::warn!("Failed to extract data field from protobuf, returning raw data");
                            (raw_hex.clone(), vec![], vec![])
                        }
                    }
                }
            } else {
                // If hex decode fails, return as-is
                log::warn!("Failed to decode hex response, returning as-is");
                (raw_hex.clone(), vec![], vec![])
            };

            // Build SimulateResponse with the structure alkanes-data-api expects
            let simulate_response = serde_json::json!({
                "execution": {
                    "data": data_hex,        // The extracted data from ExtendedCallResponse
                    "error": null,           // No error
                    "alkanes": alkanes_vec,  // Alkane transfers from the response
                    "storage": storage_vec   // Storage changes from the response
                },
                "gasUsed": 0,                // Gas not tracked for view calls
                "status": 1                  // 1 = success
            });

            Ok(JsonRpcResponse::success(simulate_response, request.id.clone()))
        }
        JsonRpcResponse::Error { error, .. } => {
            // If metashrew returned an error, wrap it in SimulateResponse format
            let simulate_response = serde_json::json!({
                "execution": {
                    "data": null,
                    "error": error.message,
                    "alkanes": [],
                    "storage": []
                },
                "gasUsed": 0,
                "status": 0  // 0 = error
            });

            Ok(JsonRpcResponse::success(simulate_response, request.id.clone()))
        }
    }
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

async fn handle_bitcoind_method(
    request: &JsonRpcRequest,
    proxy: &ProxyClient,
) -> Result<JsonRpcResponse> {
    let method_parts: Vec<&str> = request.method.split('_').collect();
    let actual_method = method_parts[method_parts.len() - 1];

    let modified_request = JsonRpcRequest {
        jsonrpc: request.jsonrpc.clone(),
        method: actual_method.to_string(),
        params: request.params.clone(),
        id: request.id.clone(),
    };

    proxy.forward_to_bitcoind(&modified_request).await
}
