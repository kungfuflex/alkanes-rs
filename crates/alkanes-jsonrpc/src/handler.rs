use crate::jsonrpc::{JsonRpcRequest, JsonRpcResponse, INTERNAL_ERROR, METHOD_NOT_FOUND};
use crate::proxy::ProxyClient;
use crate::sandshrew;
use anyhow::Result;
use serde_json::Value;
use prost::Message;
use alkanes_cli_common::proto::alkanes::{MessageContextParcel, AlkaneTransfer, AlkaneId, Uint128};
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

    match namespace {
        "ord" => handle_ord_method(&method_name, &request.params, &request.id, proxy).await,
        "esplora" => handle_esplora_method(&method_name, &request.params, &request.id, proxy).await,
        "alkanes" => handle_alkanes_method(&method_name, &request.params, &request.id, proxy).await,
        "metashrew" => handle_metashrew_method(request, proxy).await,
        "memshrew" => handle_memshrew_method(request, proxy).await,
        "lua" => sandshrew::handle_sandshrew_method(&method_name, &request.params, &request.id, proxy, script_storage).await,
        "sandshrew" => sandshrew::handle_sandshrew_method(&method_name, &request.params, &request.id, proxy, script_storage).await,
        "btc" => handle_bitcoind_method(request, proxy).await,
        _ => handle_bitcoind_method(request, proxy).await,
    }
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
    let path_parts: Vec<&str> = method.split(':').collect();
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
    let block_tag = params.get(1)
        .and_then(|v| v.as_str())
        .unwrap_or("latest");

    // For simulate method, encode JSON params to protobuf
    let encoded_input = if method == "simulate" {
        match encode_simulate_request(&input) {
            Ok(hex) => Value::String(hex),
            Err(e) => {
                return Ok(JsonRpcResponse::error(
                    INTERNAL_ERROR,
                    format!("Failed to encode simulate request: {}", e),
                    request_id.clone(),
                ));
            }
        }
    } else {
        // For other methods, convert string numbers and pass through
        // Note: Other methods may also need protobuf encoding in the future
        convert_string_numbers(input)
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
