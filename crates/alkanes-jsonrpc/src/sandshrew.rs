use alkanes_rpc_core::types::{JsonRpcResponse, INVALID_PARAMS, INTERNAL_ERROR};
use anyhow::Result;
use serde_json::Value;
use std::sync::Arc;

use crate::handler::ProdDispatcher;
use crate::proxy::ProxyClient;

/// Handle lua/sandshrew eval methods that require mlua (not in rpc-core).
///
/// Methods: evalscript, savescript, evalsaved
pub async fn handle_lua_method(
    method: &str,
    params: &[Value],
    request_id: &Value,
    dispatcher: &Arc<ProdDispatcher>,
    proxy: &ProxyClient,
    script_storage: Option<&crate::lua_executor::ScriptStorage>,
) -> Result<JsonRpcResponse> {
    match method {
        "evalscript" => handle_evalscript(params, request_id, dispatcher, proxy).await,
        "savescript" => {
            if let Some(storage) = script_storage {
                handle_savescript(params, request_id, storage).await
            } else {
                Ok(JsonRpcResponse::error(
                    INTERNAL_ERROR,
                    "Script storage not available".to_string(),
                    request_id.clone(),
                ))
            }
        }
        "evalsaved" => {
            if let Some(storage) = script_storage {
                handle_evalsaved(params, request_id, dispatcher, proxy, storage).await
            } else {
                Ok(JsonRpcResponse::error(
                    INTERNAL_ERROR,
                    "Script storage not available".to_string(),
                    request_id.clone(),
                ))
            }
        }
        _ => Ok(JsonRpcResponse::error(
            INTERNAL_ERROR,
            format!("lua/sandshrew method not supported: {}", method),
            request_id.clone(),
        )),
    }
}

async fn handle_evalscript(
    params: &[Value],
    request_id: &Value,
    dispatcher: &Arc<ProdDispatcher>,
    proxy: &ProxyClient,
) -> Result<JsonRpcResponse> {
    if params.is_empty() {
        return Ok(JsonRpcResponse::error(
            INVALID_PARAMS,
            "evalscript requires at least a script parameter".to_string(),
            request_id.clone(),
        ));
    }

    let script = params[0].as_str().ok_or_else(|| {
        anyhow::anyhow!("First parameter must be a string (Lua script)")
    })?;

    let args = params[1..].to_vec();

    let result = crate::lua_executor::execute_lua_script(script, args, dispatcher, proxy).await?;

    Ok(JsonRpcResponse::success(
        serde_json::to_value(result)?,
        request_id.clone(),
    ))
}

async fn handle_savescript(
    params: &[Value],
    request_id: &Value,
    storage: &crate::lua_executor::ScriptStorage,
) -> Result<JsonRpcResponse> {
    if params.is_empty() {
        return Ok(JsonRpcResponse::error(
            INVALID_PARAMS,
            "savescript requires a script parameter".to_string(),
            request_id.clone(),
        ));
    }

    let script = params[0].as_str().ok_or_else(|| {
        anyhow::anyhow!("First parameter must be a string (Lua script)")
    })?;

    let hash = storage.save(script.to_string()).await;

    Ok(JsonRpcResponse::success(
        serde_json::json!({ "hash": hash }),
        request_id.clone(),
    ))
}

async fn handle_evalsaved(
    params: &[Value],
    request_id: &Value,
    dispatcher: &Arc<ProdDispatcher>,
    proxy: &ProxyClient,
    storage: &crate::lua_executor::ScriptStorage,
) -> Result<JsonRpcResponse> {
    if params.is_empty() {
        return Ok(JsonRpcResponse::error(
            INVALID_PARAMS,
            "evalsaved requires at least a script hash parameter".to_string(),
            request_id.clone(),
        ));
    }

    let hash = params[0].as_str().ok_or_else(|| {
        anyhow::anyhow!("First parameter must be a string (script hash)")
    })?;

    let script = storage.get(hash).await.ok_or_else(|| {
        anyhow::anyhow!("Script not found for hash: {}", hash)
    })?;

    let args = params[1..].to_vec();

    let result = crate::lua_executor::execute_lua_script(&script, args, dispatcher, proxy).await?;

    Ok(JsonRpcResponse::success(
        serde_json::to_value(result)?,
        request_id.clone(),
    ))
}
