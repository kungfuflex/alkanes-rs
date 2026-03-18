use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::backend::{BitcoinBackend, MetashrewBackend, EsploraBackend, OrdBackend};
use crate::codec;
use crate::types::*;

/// Core RPC dispatcher that routes JSON-RPC requests to the appropriate backend.
///
/// Generic over backends so both production (reqwest) and devnet (in-process)
/// can share the same dispatch and codec logic.
pub struct RpcDispatcher<B, M, E, O> {
    pub bitcoin: B,
    pub metashrew: M,
    pub esplora: E,
    pub ord: O,
}

impl<B, M, E, O> RpcDispatcher<B, M, E, O>
where
    B: BitcoinBackend,
    M: MetashrewBackend,
    E: EsploraBackend,
    O: OrdBackend,
{
    pub fn new(bitcoin: B, metashrew: M, esplora: E, ord: O) -> Self {
        Self { bitcoin, metashrew, esplora, ord }
    }

    /// Dispatch a JSON-RPC request to the appropriate backend.
    ///
    /// Returns a boxed future (not Send) for WASM compatibility and to
    /// support recursive dispatch in multicall/balances.
    pub fn dispatch<'a>(
        &'a self,
        request: &'a JsonRpcRequest,
    ) -> Pin<Box<dyn Future<Output = Result<JsonRpcResponse>> + 'a>> {
        Box::pin(async move {
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

            // Special non-namespaced methods
            if request.method == "spendablesbyaddress" {
                return self.handle_spendables_by_address(&request.params, &request.id).await;
            }

            match namespace {
                "ord" => self.handle_ord_method(&method_name, &request.params, &request.id).await,
                "esplora" => self.handle_esplora_method(&method_name, &request.params, &request.id).await,
                "alkanes" => self.handle_alkanes_method(&method_name, &request.params, &request.id).await,
                "metashrew" => self.metashrew.forward(request).await,
                "sandshrew" => self.handle_sandshrew_method(&method_name, &request.params, &request.id).await,
                "btc" => self.handle_bitcoind_method(&method_name, &request.params, &request.id).await,
                _ => {
                    // Default: forward to bitcoind with last underscore-separated part as method
                    let actual_method = method_parts.last().unwrap_or(&"");
                    self.bitcoin.call(actual_method, request.params.clone(), request.id.clone()).await
                }
            }
        })
    }

    // -----------------------------------------------------------------------
    // Bitcoin
    // -----------------------------------------------------------------------

    async fn handle_bitcoind_method(
        &self,
        method: &str,
        params: &[Value],
        request_id: &Value,
    ) -> Result<JsonRpcResponse> {
        self.bitcoin.call(method, params.to_vec(), request_id.clone()).await
    }

    // -----------------------------------------------------------------------
    // Esplora
    // -----------------------------------------------------------------------

    async fn handle_esplora_method(
        &self,
        method: &str,
        params: &[Value],
        request_id: &Value,
    ) -> Result<JsonRpcResponse> {
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

        let result = self.esplora.fetch(&path).await?;
        Ok(JsonRpcResponse::success(result, request_id.clone()))
    }

    // -----------------------------------------------------------------------
    // Ord
    // -----------------------------------------------------------------------

    async fn handle_ord_method(
        &self,
        method: &str,
        params: &[Value],
        request_id: &Value,
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

            let content = self.ord.fetch_content(inscription_id).await?;
            use base64::Engine;
            let base64_content = base64::engine::general_purpose::STANDARD.encode(&content);

            return Ok(JsonRpcResponse::success(
                Value::String(base64_content),
                request_id.clone(),
            ));
        }

        // Split method on ':' to handle dynamic paths
        let path_parts: Vec<&str> = method.split(':').collect();
        let mut path_components: Vec<String> = vec![];
        let mut param_index = 0;

        for part in path_parts {
            if part.is_empty() {
                if param_index < params.len() {
                    if let Some(param_str) = params[param_index].as_str() {
                        path_components.push(param_str.to_string());
                    } else {
                        path_components.push(params[param_index].to_string());
                    }
                    param_index += 1;
                }
            } else {
                path_components.push(part.to_string());
            }
        }

        while param_index < params.len() {
            if let Some(param_str) = params[param_index].as_str() {
                path_components.push(param_str.to_string());
            } else {
                path_components.push(params[param_index].to_string());
            }
            param_index += 1;
        }

        let path = if path_components.is_empty() {
            "/".to_string()
        } else {
            format!("/{}", path_components.join("/"))
        };

        let result = self.ord.fetch(&path).await?;
        Ok(JsonRpcResponse::success(result, request_id.clone()))
    }

    // -----------------------------------------------------------------------
    // Alkanes (encode → metashrew_view → decode)
    // -----------------------------------------------------------------------

    async fn handle_alkanes_method(
        &self,
        method: &str,
        params: &[Value],
        request_id: &Value,
    ) -> Result<JsonRpcResponse> {
        let input = params.get(0).cloned().unwrap_or(Value::Null);

        // For protorunesbyoutpoint with positional params, block_tag is at index 2
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

        let (method_name, encoded_input, needs_decode) = match method {
            "simulate" => {
                match codec::encode_simulate_request(&input) {
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
                match codec::encode_meta_request(&input) {
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
                match codec::encode_alkanes_id_to_outpoint_request(&input) {
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
                match codec::encode_trace_request(&input) {
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
                match codec::encode_protorunesbyoutpoint_request(params) {
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
                match codec::encode_protorunesbyaddress_request(params) {
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
            _ => (method, codec::convert_string_numbers(input), "none")
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

        let response = self.metashrew.forward(&modified_request).await?;

        // Decode response if needed
        if needs_decode != "none" {
            if let JsonRpcResponse::Success { result, .. } = &response {
                if let Some(hex_str) = result.as_str() {
                    let decoded = match needs_decode {
                        "simulate" => codec::decode_simulate_response(hex_str),
                        "meta" => codec::decode_meta_response(hex_str),
                        "alkanesidtooutpoint" => codec::decode_alkanes_id_to_outpoint_response(hex_str),
                        "trace" => codec::decode_trace_response(hex_str),
                        "protorunesbyoutpoint" => codec::decode_outpoint_response(hex_str),
                        "protorunesbyaddress" => codec::decode_wallet_response(hex_str),
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

    // -----------------------------------------------------------------------
    // Spendables by address
    // -----------------------------------------------------------------------

    async fn handle_spendables_by_address(
        &self,
        params: &[Value],
        request_id: &Value,
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

        let path = format!("/address/{}/utxo", address);
        let utxos = self.esplora.fetch(&path).await?;

        let empty_vec = vec![];
        let utxo_array = utxos.as_array().unwrap_or(&empty_vec);
        let outpoints: Vec<Value> = utxo_array.iter().map(|utxo| {
            json!({
                "outpoint": {
                    "txid": utxo.get("txid").and_then(|v| v.as_str()).unwrap_or(""),
                    "vout": utxo.get("vout").and_then(|v| v.as_u64()).unwrap_or(0)
                },
                "value": utxo.get("value").and_then(|v| v.as_u64()).unwrap_or(0),
                "height": utxo.get("status").and_then(|s| s.get("block_height")).and_then(|v| v.as_u64()).unwrap_or(0)
            })
        }).collect();

        let result = json!({ "outpoints": outpoints });
        Ok(JsonRpcResponse::success(result, request_id.clone()))
    }

    // -----------------------------------------------------------------------
    // Sandshrew (multicall + balances only; lua stays in jsonrpc)
    // -----------------------------------------------------------------------

    async fn handle_sandshrew_method(
        &self,
        method: &str,
        params: &[Value],
        request_id: &Value,
    ) -> Result<JsonRpcResponse> {
        match method {
            "multicall" => self.handle_multicall(params, request_id).await,
            "balances" => self.handle_balances(params, request_id).await,
            _ => Ok(JsonRpcResponse::error(
                METHOD_NOT_FOUND,
                format!("sandshrew method not supported in core: {}", method),
                request_id.clone(),
            )),
        }
    }

    async fn handle_multicall(
        &self,
        params: &[Value],
        request_id: &Value,
    ) -> Result<JsonRpcResponse> {
        if params.is_empty() {
            return Ok(JsonRpcResponse::error(
                INVALID_PARAMS,
                "multicall requires array of [method, params] pairs".to_string(),
                request_id.clone(),
            ));
        }

        let mut requests = Vec::new();

        for call in params {
            let call_tuple = match call.as_array() {
                Some(arr) if arr.len() == 2 => arr,
                _ => {
                    return Ok(JsonRpcResponse::error(
                        INVALID_PARAMS,
                        "Each multicall entry must be a tuple of [method, params]".to_string(),
                        request_id.clone(),
                    ));
                }
            };

            let method = match call_tuple[0].as_str() {
                Some(m) => m.to_string(),
                None => {
                    return Ok(JsonRpcResponse::error(
                        INVALID_PARAMS,
                        "Method name must be a string".to_string(),
                        request_id.clone(),
                    ));
                }
            };

            let call_params = match call_tuple[1].as_array() {
                Some(p) => p.clone(),
                None => {
                    return Ok(JsonRpcResponse::error(
                        INVALID_PARAMS,
                        "Method params must be an array".to_string(),
                        request_id.clone(),
                    ));
                }
            };

            requests.push(JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                method,
                params: call_params,
                id: Value::Number(0.into()),
            });
        }

        // Execute all requests in parallel
        let futures: Vec<_> = requests.iter()
            .map(|req| self.dispatch(req))
            .collect();

        let results = futures::future::join_all(futures).await;

        let formatted: Vec<Value> = results.into_iter().map(|r| {
            match r {
                Ok(JsonRpcResponse::Success { result, .. }) => {
                    json!({ "result": result })
                }
                Ok(JsonRpcResponse::Error { error, .. }) => {
                    json!({ "error": error })
                }
                Err(e) => {
                    json!({
                        "error": {
                            "code": INTERNAL_ERROR,
                            "message": e.to_string()
                        }
                    })
                }
            }
        }).collect();

        Ok(JsonRpcResponse::success(
            serde_json::to_value(formatted)?,
            request_id.clone(),
        ))
    }

    // -----------------------------------------------------------------------
    // Balances
    // -----------------------------------------------------------------------

    async fn handle_balances(
        &self,
        params: &[Value],
        request_id: &Value,
    ) -> Result<JsonRpcResponse> {
        if params.is_empty() || !params[0].is_object() {
            return Ok(JsonRpcResponse::error(
                INVALID_PARAMS,
                "balances requires an object with 'address' field".to_string(),
                request_id.clone(),
            ));
        }

        let balance_req: BalanceRequest = match serde_json::from_value(params[0].clone()) {
            Ok(req) => req,
            Err(e) => {
                return Ok(JsonRpcResponse::error(
                    INVALID_PARAMS,
                    format!("Invalid balance request: {}", e),
                    request_id.clone(),
                ));
            }
        };

        let protocol_tag = balance_req.protocol_tag.as_deref().unwrap_or("1");
        let unique_addresses: Vec<String> = if let Some(asset_addr) = &balance_req.asset_address {
            vec![balance_req.address.clone(), asset_addr.clone()]
        } else {
            vec![balance_req.address.clone()]
        };

        let mut rpc_calls = Vec::new();

        for addr in &unique_addresses {
            rpc_calls.push(JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                method: "esplora_address::utxo".to_string(),
                params: vec![Value::String(addr.clone())],
                id: Value::Number(0.into()),
            });

            rpc_calls.push(JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                method: "alkanes_protorunesbyaddress".to_string(),
                params: vec![json!({
                    "address": addr,
                    "protocolTag": protocol_tag
                })],
                id: Value::Number(0.into()),
            });

            rpc_calls.push(JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                method: "ord_outputs".to_string(),
                params: vec![Value::String(addr.clone())],
                id: Value::Number(0.into()),
            });
        }

        rpc_calls.push(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "ord_blockheight".to_string(),
            params: vec![],
            id: Value::Number(0.into()),
        });

        rpc_calls.push(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "metashrew_height".to_string(),
            params: vec![],
            id: Value::Number(0.into()),
        });

        let mut results = Vec::new();
        for req in &rpc_calls {
            let response = self.dispatch(req).await?;
            match response {
                JsonRpcResponse::Success { result, .. } => results.push(result),
                JsonRpcResponse::Error { error, .. } => {
                    return Ok(JsonRpcResponse::error(
                        error.code,
                        error.message,
                        request_id.clone(),
                    ));
                }
            }
        }

        let expected_len = unique_addresses.len() * 3 + 2;
        if results.len() != expected_len {
            return Ok(JsonRpcResponse::error(
                INTERNAL_ERROR,
                format!("Unexpected number of results: expected {}, got {}", expected_len, results.len()),
                request_id.clone(),
            ));
        }

        // Parse heights with graceful fallbacks for missing backends (e.g., NoOrd)
        let ord_height: u64 = serde_json::from_value(results[results.len() - 2].clone())
            .unwrap_or(0);

        let metashrew_height: u64 = results[results.len() - 1]
            .as_str()
            .and_then(|s| s.parse::<u64>().ok())
            .or_else(|| results[results.len() - 1].as_u64())
            .unwrap_or(0);

        let mut all_spendable = Vec::new();
        let mut all_assets = Vec::new();
        let mut all_pending = Vec::new();

        for i in 0..unique_addresses.len() {
            let utxos_result = &results[i * 3];
            let protorunes_result = &results[i * 3 + 1];
            let ord_outputs_result = &results[i * 3 + 2];

            let address_info = process_address_info(
                utxos_result,
                protorunes_result,
                ord_outputs_result,
                ord_height,
                metashrew_height,
            )?;

            all_spendable.extend(address_info.spendable);
            all_assets.extend(address_info.assets);
            all_pending.extend(address_info.pending);
        }

        let final_info = AddressInfo {
            spendable: all_spendable,
            assets: all_assets,
            pending: all_pending,
            ord_height,
            metashrew_height,
        };

        Ok(JsonRpcResponse::success(
            serde_json::to_value(final_info)?,
            request_id.clone(),
        ))
    }
}

// ---------------------------------------------------------------------------
// Balance types (extracted from alkanes-jsonrpc/src/sandshrew.rs)
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
pub struct BalanceRequest {
    pub address: String,
    #[serde(rename = "protocolTag")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protocol_tag: Option<String>,
    #[serde(rename = "assetAddress")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asset_address: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UTXO {
    pub outpoint: String,
    pub value: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runes: Option<Vec<Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inscriptions: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ord_runes: Option<HashMap<String, Value>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AddressInfo {
    pub spendable: Vec<UTXO>,
    pub assets: Vec<UTXO>,
    pub pending: Vec<UTXO>,
    #[serde(rename = "ordHeight")]
    pub ord_height: u64,
    #[serde(rename = "metashrewHeight")]
    pub metashrew_height: u64,
}

fn reverse_txid(txid: &str) -> Result<String> {
    let bytes = hex::decode(txid)?;
    let mut reversed = bytes;
    reversed.reverse();
    Ok(hex::encode(reversed))
}

fn process_address_info(
    utxos_result: &Value,
    protorunes_result: &Value,
    ord_outputs_result: &Value,
    ord_height: u64,
    metashrew_height: u64,
) -> Result<AddressInfo> {
    let mut runes_map: HashMap<String, Vec<Value>> = HashMap::new();

    if let Some(outpoints) = protorunes_result.get("outpoints").and_then(|v| v.as_array()) {
        for outpoint in outpoints {
            if let (Some(op), Some(runes)) = (
                outpoint.get("outpoint"),
                outpoint.get("runes").and_then(|r| r.as_array()),
            ) {
                if let (Some(txid), Some(vout)) = (
                    op.get("txid").and_then(|t| t.as_str()),
                    op.get("vout").and_then(|v| v.as_u64()),
                ) {
                    let reversed_txid = reverse_txid(txid)?;
                    let key = format!("{}:{}", reversed_txid, vout);
                    runes_map.insert(key, runes.clone());
                }
            }
        }
    }

    let mut ord_outputs_map: HashMap<String, (Vec<String>, HashMap<String, Value>)> = HashMap::new();

    if let Some(outputs) = ord_outputs_result.as_array() {
        for output in outputs {
            if let Some(outpoint) = output.get("outpoint").and_then(|o| o.as_str()) {
                let inscriptions = output
                    .get("inscriptions")
                    .and_then(|i| i.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default();

                let ord_runes = output
                    .get("runes")
                    .and_then(|r| r.as_object())
                    .map(|obj| {
                        obj.iter()
                            .map(|(k, v)| (k.clone(), v.clone()))
                            .collect()
                    })
                    .unwrap_or_default();

                ord_outputs_map.insert(outpoint.to_string(), (inscriptions, ord_runes));
            }
        }
    }

    let max_indexed_height = ord_height.max(metashrew_height);

    let mut spendable = Vec::new();
    let mut assets = Vec::new();
    let mut pending = Vec::new();

    if let Some(utxos) = utxos_result.as_array() {
        for utxo in utxos {
            let txid = utxo.get("txid").and_then(|t| t.as_str()).unwrap_or("");
            let vout = utxo.get("vout").and_then(|v| v.as_u64()).unwrap_or(0);
            let value = utxo.get("value").and_then(|v| v.as_u64()).unwrap_or(0);

            let key = format!("{}:{}", txid, vout);

            let runes = runes_map.get(&key).cloned();
            let (inscriptions, ord_runes) = ord_outputs_map
                .get(&key)
                .cloned()
                .unwrap_or((Vec::new(), HashMap::new()));

            let height = utxo
                .get("status")
                .and_then(|s| s.get("block_height"))
                .and_then(|h| h.as_u64());

            let mut optimized_utxo = UTXO {
                outpoint: key,
                value,
                height,
                runes: None,
                inscriptions: None,
                ord_runes: None,
            };

            if let Some(r) = runes {
                if !r.is_empty() {
                    optimized_utxo.runes = Some(r);
                }
            }

            if !inscriptions.is_empty() {
                optimized_utxo.inscriptions = Some(inscriptions);
            }

            if !ord_runes.is_empty() {
                optimized_utxo.ord_runes = Some(ord_runes);
            }

            let has_assets = optimized_utxo.runes.is_some()
                || optimized_utxo.inscriptions.is_some()
                || optimized_utxo.ord_runes.is_some();

            let is_pending = height.map(|h| h > max_indexed_height).unwrap_or(false);

            if is_pending {
                pending.push(optimized_utxo);
            } else if has_assets {
                assets.push(optimized_utxo);
            } else {
                spendable.push(optimized_utxo);
            }
        }
    }

    let sort_utxos = |utxos: &mut Vec<UTXO>| {
        utxos.sort_by(|a, b| {
            match (a.height, b.height) {
                (None, None) => std::cmp::Ordering::Equal,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (Some(_), None) => std::cmp::Ordering::Less,
                (Some(ah), Some(bh)) => ah.cmp(&bh),
            }
        });
    };

    sort_utxos(&mut spendable);
    sort_utxos(&mut assets);
    sort_utxos(&mut pending);

    Ok(AddressInfo {
        spendable,
        assets,
        pending,
        ord_height,
        metashrew_height,
    })
}
