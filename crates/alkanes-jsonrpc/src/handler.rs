use alkanes_rpc_core::types::{JsonRpcRequest, JsonRpcResponse, METHOD_NOT_FOUND};
use alkanes_rpc_core::RpcDispatcher;
use anyhow::Result;
use std::sync::Arc;

use crate::backends::*;
use crate::proxy::ProxyClient;
use crate::sandshrew;

/// Convenience type alias for the production dispatcher with reqwest backends.
pub type ProdDispatcher = RpcDispatcher<
    ReqwestBitcoinBackend,
    ReqwestMetashrewBackend,
    ReqwestEsploraBackend,
    ReqwestOrdBackend,
>;

/// memshrew-p2p JSON-RPC methods the gateway may forward (all read-only).
pub const MEMSHREW_ALLOWED_METHODS: &[&str] = &[
    "memshrew_build",
    "memshrew_getmempooltxs",
    "memshrew_getblocktemplates",
    "memshrew_estimatefees",
];

/// Handle a JSON-RPC request using the core dispatcher with pre-dispatch
/// interception for memshrew, subfrost, and lua/sandshrew eval methods.
pub async fn handle_request_with_storage(
    request: &JsonRpcRequest,
    dispatcher: &Arc<ProdDispatcher>,
    proxy: &ProxyClient,
    script_storage: Option<&crate::lua_executor::ScriptStorage>,
) -> Result<JsonRpcResponse> {
    let method_parts: Vec<&str> = request.method.split('_').collect();
    let namespace = method_parts.get(0).copied().unwrap_or("");
    let method_name = if method_parts.len() > 1 {
        method_parts[1..].join("_")
    } else {
        String::new()
    };

    // Pre-dispatch interception for methods not in rpc-core
    match namespace {
        "memshrew" => {
            // Deny-by-default, like every other backend passthrough: only the
            // read methods memshrew-p2p serves are forwarded.
            if MEMSHREW_ALLOWED_METHODS.contains(&request.method.as_str()) {
                return proxy.forward_to_memshrew(request).await;
            }
            return Ok(JsonRpcResponse::error(
                METHOD_NOT_FOUND,
                format!("method not found: {}", request.method),
                request.id.clone(),
            ));
        }
        "subfrost" => return proxy.forward_to_subfrost(request).await,
        "lua" => {
            return sandshrew::handle_lua_method(
                &method_name, &request.params, &request.id,
                dispatcher, proxy, script_storage,
            ).await;
        }
        "sandshrew" => {
            // Intercept eval methods; multicall/balances go to core dispatcher
            match method_name.as_str() {
                "evalscript" | "savescript" | "evalsaved" => {
                    return sandshrew::handle_lua_method(
                        &method_name, &request.params, &request.id,
                        dispatcher, proxy, script_storage,
                    ).await;
                }
                _ => {} // Fall through to core dispatcher
            }
        }
        _ => {} // Fall through to core dispatcher
    }

    dispatcher.dispatch(request).await
}
