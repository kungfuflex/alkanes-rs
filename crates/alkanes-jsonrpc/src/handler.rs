use crate::alkanes;
use crate::jsonrpc::{JsonRpcRequest, JsonRpcResponse, INTERNAL_ERROR, METHOD_NOT_FOUND};
use crate::proxy::ProxyClient;
use crate::sandshrew;
use anyhow::Result;
use serde_json::Value;

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

/// Helper to call metashrew_view and get the result as a string
async fn call_metashrew_view(
    proxy: &ProxyClient,
    view_name: &str,
    encoded_input: &str,
    block_tag: &str,
) -> Result<String> {
    let request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: "metashrew_view".to_string(),
        params: vec![
            Value::String(view_name.to_string()),
            Value::String(encoded_input.to_string()),
            Value::String(block_tag.to_string()),
        ],
        id: Value::Number(1.into()),
    };

    let response = proxy.forward_to_metashrew(&request).await?;

    match response {
        JsonRpcResponse::Success { result, .. } => {
            result
                .as_str()
                .map(|s| s.to_string())
                .ok_or_else(|| anyhow::anyhow!("metashrew_view result is not a string"))
        }
        JsonRpcResponse::Error { error, .. } => {
            Err(anyhow::anyhow!("metashrew_view error: {}", error.message))
        }
    }
}

async fn handle_alkanes_method(
    method: &str,
    params: &[Value],
    request_id: &Value,
    proxy: &ProxyClient,
) -> Result<JsonRpcResponse> {
    // Get the input parameters (first param) and block_tag (second param)
    let input = params.get(0).cloned().unwrap_or(Value::Object(serde_json::Map::new()));
    let block_tag = params
        .get(1)
        .and_then(|v| v.as_str())
        .unwrap_or("latest");

    // Handle each method with proper encoding/decoding
    match method {
        "simulate" => {
            let req: alkanes::SimulateRequest = serde_json::from_value(input)
                .map_err(|e| anyhow::anyhow!("Invalid simulate request: {}", e))?;
            let encoded = alkanes::encode_simulate_request(&req)?;
            let response = call_metashrew_view(proxy, "simulate", &encoded, block_tag).await?;
            let decoded = alkanes::decode_simulate_response(&response)?;
            Ok(JsonRpcResponse::success(
                serde_json::to_value(decoded)?,
                request_id.clone(),
            ))
        }

        "meta" => {
            let req: alkanes::SimulateRequest = serde_json::from_value(input)
                .map_err(|e| anyhow::anyhow!("Invalid meta request: {}", e))?;
            let encoded = alkanes::encode_simulate_request(&req)?;
            let response = call_metashrew_view(proxy, "meta", &encoded, block_tag).await?;
            let decoded = alkanes::decode_meta_response(&response)?;
            Ok(JsonRpcResponse::success(decoded, request_id.clone()))
        }

        "trace" => {
            let req: alkanes::TraceRequest = serde_json::from_value(input)
                .map_err(|e| anyhow::anyhow!("Invalid trace request: {}", e))?;
            let encoded = alkanes::encode_trace_request(&req)?;
            let response = call_metashrew_view(proxy, "trace", &encoded, block_tag).await?;
            let decoded = alkanes::decode_trace_response(&response)?;
            Ok(JsonRpcResponse::success(
                serde_json::to_value(decoded)?,
                request_id.clone(),
            ))
        }

        "traceblock" => {
            let req: alkanes::TraceBlockRequest = serde_json::from_value(input)
                .map_err(|e| anyhow::anyhow!("Invalid traceblock request: {}", e))?;
            let encoded = alkanes::encode_traceblock_request(&req)?;
            let response = call_metashrew_view(proxy, "traceblock", &encoded, block_tag).await?;
            let decoded = alkanes::decode_traceblock_response(&response)?;
            Ok(JsonRpcResponse::success(
                serde_json::to_value(decoded)?,
                request_id.clone(),
            ))
        }

        "getbytecode" => {
            let req: alkanes::BytecodeRequest = serde_json::from_value(input)
                .map_err(|e| anyhow::anyhow!("Invalid getbytecode request: {}", e))?;
            let encoded = alkanes::encode_bytecode_request(&req)?;
            let response = call_metashrew_view(proxy, "getbytecode", &encoded, block_tag).await?;
            let decoded = alkanes::decode_bytecode_response(&response)?;
            Ok(JsonRpcResponse::success(
                Value::String(decoded),
                request_id.clone(),
            ))
        }

        "getblock" => {
            let req: alkanes::BlockRequest = serde_json::from_value(input)
                .map_err(|e| anyhow::anyhow!("Invalid getblock request: {}", e))?;
            let encoded = alkanes::encode_block_request(&req)?;
            let response = call_metashrew_view(proxy, "getblock", &encoded, block_tag).await?;
            let decoded = alkanes::decode_block_response(&response)?;
            Ok(JsonRpcResponse::success(
                Value::String(decoded),
                request_id.clone(),
            ))
        }

        "getinventory" => {
            let req: alkanes::InventoryRequest = serde_json::from_value(input)
                .map_err(|e| anyhow::anyhow!("Invalid getinventory request: {}", e))?;
            let encoded = alkanes::encode_inventory_request(&req)?;
            let response = call_metashrew_view(proxy, "getinventory", &encoded, block_tag).await?;
            let decoded = alkanes::decode_inventory_response(&response)?;
            Ok(JsonRpcResponse::success(
                serde_json::to_value(decoded)?,
                request_id.clone(),
            ))
        }

        "getstorageat" => {
            let req: alkanes::StorageRequest = serde_json::from_value(input)
                .map_err(|e| anyhow::anyhow!("Invalid getstorageat request: {}", e))?;
            let encoded = alkanes::encode_storage_request(&req)?;
            let response = call_metashrew_view(proxy, "getstorageat", &encoded, block_tag).await?;
            let decoded = alkanes::decode_storage_response(&response)?;
            Ok(JsonRpcResponse::success(
                Value::String(decoded),
                request_id.clone(),
            ))
        }

        "protorunesbyaddress" => {
            let req: alkanes::ProtorunesAddressRequest = serde_json::from_value(input)
                .map_err(|e| anyhow::anyhow!("Invalid protorunesbyaddress request: {}", e))?;
            let encoded = alkanes::encode_protorunesbyaddress_request(&req)?;
            let response =
                call_metashrew_view(proxy, "protorunesbyaddress", &encoded, block_tag).await?;
            let decoded = alkanes::decode_wallet_response(&response)?;
            Ok(JsonRpcResponse::success(
                serde_json::to_value(decoded)?,
                request_id.clone(),
            ))
        }

        "runesbyaddress" => {
            let req: alkanes::AddressRequest = serde_json::from_value(input)
                .map_err(|e| anyhow::anyhow!("Invalid runesbyaddress request: {}", e))?;
            let encoded = alkanes::encode_runesbyaddress_request(&req)?;
            let response =
                call_metashrew_view(proxy, "runesbyaddress", &encoded, block_tag).await?;
            let decoded = alkanes::decode_wallet_response(&response)?;
            Ok(JsonRpcResponse::success(
                serde_json::to_value(decoded)?,
                request_id.clone(),
            ))
        }

        "spendablesbyaddress" => {
            let req: alkanes::ProtorunesAddressRequest = serde_json::from_value(input)
                .map_err(|e| anyhow::anyhow!("Invalid spendablesbyaddress request: {}", e))?;
            let encoded = alkanes::encode_protorunesbyaddress_request(&req)?;
            let response =
                call_metashrew_view(proxy, "spendablesbyaddress", &encoded, block_tag).await?;
            let decoded = alkanes::decode_wallet_response(&response)?;
            Ok(JsonRpcResponse::success(
                serde_json::to_value(decoded)?,
                request_id.clone(),
            ))
        }

        "runesbyheight" => {
            let req: alkanes::HeightRequest = serde_json::from_value(input)
                .map_err(|e| anyhow::anyhow!("Invalid runesbyheight request: {}", e))?;
            let encoded = alkanes::encode_runesbyheight_request(&req)?;
            let response =
                call_metashrew_view(proxy, "runesbyheight", &encoded, block_tag).await?;
            let decoded = alkanes::decode_runes_response(&response)?;
            Ok(JsonRpcResponse::success(
                serde_json::to_value(decoded)?,
                request_id.clone(),
            ))
        }

        "protorunesbyheight" => {
            let req: alkanes::ProtorunesHeightRequest = serde_json::from_value(input)
                .map_err(|e| anyhow::anyhow!("Invalid protorunesbyheight request: {}", e))?;
            let encoded = alkanes::encode_protorunesbyheight_request(&req)?;
            let response =
                call_metashrew_view(proxy, "protorunesbyheight", &encoded, block_tag).await?;
            let decoded = alkanes::decode_runes_response(&response)?;
            Ok(JsonRpcResponse::success(
                serde_json::to_value(decoded)?,
                request_id.clone(),
            ))
        }

        "runesbyoutpoint" => {
            let req: alkanes::OutpointRequest = serde_json::from_value(input)
                .map_err(|e| anyhow::anyhow!("Invalid runesbyoutpoint request: {}", e))?;
            let encoded = alkanes::encode_runesbyoutpoint_request(&req)?;
            let response =
                call_metashrew_view(proxy, "protorunesbyoutpoint", &encoded, block_tag).await?;
            let decoded = alkanes::decode_outpoint_balances_response(&response)?;
            Ok(JsonRpcResponse::success(
                serde_json::to_value(decoded)?,
                request_id.clone(),
            ))
        }

        "protorunesbyoutpoint" => {
            let req: alkanes::ProtorunesOutpointRequest = serde_json::from_value(input)
                .map_err(|e| anyhow::anyhow!("Invalid protorunesbyoutpoint request: {}", e))?;
            let encoded = alkanes::encode_protorunesbyoutpoint_request(&req)?;
            let response =
                call_metashrew_view(proxy, "protorunesbyoutpoint", &encoded, block_tag).await?;
            let decoded = alkanes::decode_outpoint_balances_response(&response)?;
            Ok(JsonRpcResponse::success(
                serde_json::to_value(decoded)?,
                request_id.clone(),
            ))
        }

        "alkanesidtooutpoint" | "alkanes_id_to_outpoint" => {
            let req: alkanes::AlkaneIdToOutpointRequest = serde_json::from_value(input)
                .map_err(|e| anyhow::anyhow!("Invalid alkanesidtooutpoint request: {}", e))?;
            let encoded = alkanes::encode_alkaneid_to_outpoint_request(&req)?;
            let response =
                call_metashrew_view(proxy, "alkanes_id_to_outpoint", &encoded, block_tag).await?;
            let decoded = alkanes::decode_alkaneid_to_outpoint_response(&response)?;
            Ok(JsonRpcResponse::success(
                serde_json::to_value(decoded)?,
                request_id.clone(),
            ))
        }

        "transactionbyid" => {
            let req: alkanes::TransactionByIdRequest = serde_json::from_value(input)
                .map_err(|e| anyhow::anyhow!("Invalid transactionbyid request: {}", e))?;
            let encoded = alkanes::encode_transactionbyid_request(&req)?;
            let response =
                call_metashrew_view(proxy, "transactionbyid", &encoded, block_tag).await?;
            let decoded = alkanes::decode_transaction_response(&response)?;
            Ok(JsonRpcResponse::success(
                serde_json::to_value(decoded)?,
                request_id.clone(),
            ))
        }

        "runtime" => {
            let req: alkanes::RuntimeRequest = serde_json::from_value(input)
                .map_err(|e| anyhow::anyhow!("Invalid runtime request: {}", e))?;
            let encoded = alkanes::encode_runtime_request(&req)?;
            // runtime uses protorunesbyaddress as the view method
            let response =
                call_metashrew_view(proxy, "protorunesbyaddress", &encoded, block_tag).await?;
            let decoded = alkanes::decode_runtime_response(&response)?;
            Ok(JsonRpcResponse::success(
                serde_json::to_value(decoded)?,
                request_id.clone(),
            ))
        }

        "unwraps" | "unwrap" => {
            let req: alkanes::UnwrapsRequest = serde_json::from_value(input)
                .map_err(|e| anyhow::anyhow!("Invalid unwraps request: {}", e))?;
            let encoded = alkanes::encode_unwraps_request(&req)?;
            // unwraps uses the block height as block_tag
            let height_tag = req.block.to_string();
            let response = call_metashrew_view(proxy, "unwrap", &encoded, &height_tag).await?;
            let decoded = alkanes::decode_unwraps_response(&response)?;
            Ok(JsonRpcResponse::success(
                serde_json::to_value(decoded)?,
                request_id.clone(),
            ))
        }

        // Fallback: forward unknown methods to metashrew_view with raw params
        _ => {
            log::warn!("Unknown alkanes method '{}', forwarding raw to metashrew_view", method);
            let modified_request = JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                method: "metashrew_view".to_string(),
                params: vec![
                    Value::String(method.to_string()),
                    input,
                    Value::String(block_tag.to_string()),
                ],
                id: request_id.clone(),
            };
            proxy.forward_to_metashrew(&modified_request).await
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
