use crate::jsonrpc::{JsonRpcRequest, JsonRpcResponse, INVALID_PARAMS, INTERNAL_ERROR};
use crate::proxy::ProxyClient;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

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

pub async fn handle_sandshrew_method(
    method: &str,
    params: &[Value],
    request_id: &Value,
    proxy: &ProxyClient,
) -> Result<JsonRpcResponse> {
    match method {
        "multicall" => handle_multicall(params, request_id, proxy).await,
        "balances" => handle_balances(params, request_id, proxy).await,
        _ => Ok(JsonRpcResponse::error(
            INTERNAL_ERROR,
            format!("sandshrew method not supported: {}", method),
            request_id.clone(),
        )),
    }
}

async fn handle_multicall(
    params: &[Value],
    request_id: &Value,
    proxy: &ProxyClient,
) -> Result<JsonRpcResponse> {
    if params.is_empty() {
        return Ok(JsonRpcResponse::error(
            INVALID_PARAMS,
            "multicall requires array of [method, params] pairs".to_string(),
            request_id.clone(),
        ));
    }

    let calls: Vec<(String, Vec<Value>)> = match serde_json::from_value(params[0].clone()) {
        Ok(c) => c,
        Err(e) => {
            return Ok(JsonRpcResponse::error(
                INVALID_PARAMS,
                format!("Invalid multicall params: {}", e),
                request_id.clone(),
            ));
        }
    };

    let mut results = Vec::new();
    
    for (method, call_params) in calls {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: method.clone(),
            params: call_params,
            id: Value::Number(0.into()),
        };

        let response = Box::pin(crate::handler::handle_request(&req, proxy)).await?;
        
        let result_value = match response {
            JsonRpcResponse::Success { result, .. } => {
                serde_json::json!({ "result": result })
            }
            JsonRpcResponse::Error { error, .. } => {
                serde_json::json!({ "error": error })
            }
        };
        
        results.push(result_value);
    }

    Ok(JsonRpcResponse::success(
        serde_json::to_value(results)?,
        request_id.clone(),
    ))
}

async fn handle_balances(
    params: &[Value],
    request_id: &Value,
    proxy: &ProxyClient,
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
    let addresses: Vec<String> = if let Some(asset_addr) = &balance_req.asset_address {
        vec![balance_req.address.clone(), asset_addr.clone()]
    } else {
        vec![balance_req.address.clone()]
    };

    let unique_addresses: Vec<String> = addresses.into_iter().collect();

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
            params: vec![serde_json::json!({
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
    for req in rpc_calls {
        let response = Box::pin(crate::handler::handle_request(&req, proxy)).await?;
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

    let ord_height: u64 = serde_json::from_value(results[results.len() - 2].clone())
        .map_err(|e| anyhow::anyhow!("Failed to parse ord height: {}", e))?;
    
    let metashrew_height_str: String = serde_json::from_value(results[results.len() - 1].clone())
        .map_err(|e| anyhow::anyhow!("Failed to parse metashrew height: {}", e))?;
    let metashrew_height: u64 = metashrew_height_str.parse()
        .map_err(|e| anyhow::anyhow!("Failed to parse metashrew height as u64: {}", e))?;

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
