use alkanes_rpc_core::types::{JsonRpcRequest, JsonRpcResponse};
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

/// Handle a JSON-RPC request using the core dispatcher with pre-dispatch
/// interception for memshrew, subfrost, and lua/sandshrew eval methods.
pub async fn handle_request(
    request: &JsonRpcRequest,
    dispatcher: &Arc<ProdDispatcher>,
    proxy: &ProxyClient,
) -> Result<JsonRpcResponse> {
    handle_request_with_storage(request, dispatcher, proxy, None).await
}

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
        "memshrew" => return proxy.forward_to_memshrew(request).await,
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
