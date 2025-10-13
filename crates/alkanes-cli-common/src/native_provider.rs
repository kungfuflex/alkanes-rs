
use crate::traits::*;
use crate::{
    AlkanesError, Result,
};

use log;
use async_trait::async_trait;
use alloc::format;
use alloc::string::ToString;
use alloc::boxed::Box;
use url::Url;

// Import deezel-rpgp types for PGP functionality

#[cfg(target_arch = "wasm32")]
use rand::rngs::OsRng;

// Import Bitcoin and BIP39 for wallet functionality

// Additional imports for wallet functionality
use crate::provider::ConcreteProvider;

pub struct NativeProvider {
    pub provider: ConcreteProvider,
    pub http_client: reqwest::Client,
}

impl NativeProvider {
    pub fn new(provider: ConcreteProvider) -> Self {
        Self {
            provider,
            http_client: reqwest::Client::new(),
        }
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl JsonRpcProvider for NativeProvider {
    async fn call(
        &self,
        url: &str,
        method: &str,
        params: serde_json::Value,
        id: u64,
    ) -> Result<serde_json::Value> {
        // Info logging for JsonRpcProvider call - logs all RPC payloads sent
        log::info!(
            "JsonRpcProvider::call -> URL: {}, Method: {}, Params: {}",
            url,
            method,
            serde_json::to_string_pretty(&params).unwrap_or_else(|_| "INVALID_JSON".to_string()),
        );
        
        use crate::rpc::RpcRequest;
        let request = RpcRequest::new(method, params, id);

        let mut parsed_url = Url::parse(url)
            .map_err(|e| AlkanesError::InvalidParameters(format!("Invalid RPC URL in call: {e}")))?;

        let username = parsed_url.username().to_string();
        let password = parsed_url.password().map(|p| p.to_string());

        // Remove user/pass from the URL before sending
        parsed_url.set_username("").map_err(|_| AlkanesError::InvalidParameters("Failed to strip username".into()))?;
        parsed_url.set_password(None).map_err(|_| AlkanesError::InvalidParameters("Failed to strip password".into()))?;

        let mut request_builder = self.http_client.post(parsed_url);

        if !username.is_empty() {
            request_builder = request_builder.basic_auth(username, password);
        }

        log::debug!("Request builder: {:?}", request_builder);
        let response = request_builder
            .json(&request)
            .send()
            .await
            .map_err(|e| AlkanesError::Network(e.to_string()))?;
        let response_text = response.text().await.map_err(|e| AlkanesError::Network(e.to_string()))?;
        
        log::info!("JsonRpcProvider::call <- Raw RPC response: {response_text}");
        
        if response_text.starts_with("Json deserialize error") {
            return Err(AlkanesError::RpcError(format!("Server-side JSON deserialization error: {}", response_text)));
        }

        // First, try to parse as a standard RpcResponse
        // A more robust parsing logic that handles different RPC response structures.
        if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(&response_text) {
            // Check for a standard JSON-RPC error object.
            if let Some(error_obj) = json_value.get("error") {
                if !error_obj.is_null() {
                    let code = error_obj.get("code").and_then(|c| c.as_i64()).unwrap_or(0);
                    let message = error_obj.get("message").and_then(|m| m.as_str()).unwrap_or("Unknown RPC error");
                    return Err(AlkanesError::RpcError(format!("Code {}: {}", code, message)));
                }
            }

            // Check for a standard JSON-RPC result.
            if let Some(result) = json_value.get("result") {
                return Ok(result.clone());
            }
            
            // Fallback for non-standard responses that are just the result value.
            return Ok(json_value);
        }

        // If that fails, try to parse as a raw JsonValue (for non-compliant servers)
        if let Ok(mut raw_result) = serde_json::from_str::<serde_json::Value>(&response_text) {
            // Handle cases where the actual result is nested inside a "result" field
            if let Some(obj) = raw_result.as_object_mut() {
                if obj.contains_key("result") {
                    if let Some(val) = obj.remove("result") {
                        return Ok(val);
                    }
                }
            }
            return Ok(raw_result);
        }

        // If that also fails, check if the response is just a plain string
        // This is needed for some Esplora endpoints that return plain text
        if !response_text.starts_with('{') && !response_text.starts_with('[') {
            // It's likely a plain string, wrap it in a JsonValue
            return Ok(serde_json::Value::String(response_text));
        }

        // If all attempts fail, return a generic error
        Err(AlkanesError::Network(format!("Failed to decode RPC response: {response_text}")))
    }
}
