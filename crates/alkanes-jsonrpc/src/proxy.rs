use crate::config::Config;
use crate::jsonrpc::{JsonRpcRequest, JsonRpcResponse, INTERNAL_ERROR};
use anyhow::Result;
use reqwest::Client;
use serde_json::Value;

#[derive(Clone)]
pub struct ProxyClient {
    client: Client,
    config: Config,
}

impl ProxyClient {
    pub fn new(config: Config) -> Self {
        Self {
            client: Client::new(),
            config,
        }
    }

    pub async fn forward_to_metashrew(&self, request: &JsonRpcRequest) -> Result<JsonRpcResponse> {
        let response = self
            .client
            .post(&self.config.metashrew_url)
            .json(request)
            .send()
            .await?;

        let json_response: Value = response.json().await?;

        // Handle metashrew responses which may include "error": null for success cases
        // This can't be deserialized directly into the untagged JsonRpcResponse enum
        if let Some(error) = json_response.get("error") {
            if !error.is_null() {
                return Ok(JsonRpcResponse::Error {
                    jsonrpc: "2.0".to_string(),
                    error: serde_json::from_value(error.clone())?,
                    id: request.id.clone(),
                });
            }
        }

        if let Some(result) = json_response.get("result") {
            Ok(JsonRpcResponse::success(result.clone(), request.id.clone()))
        } else {
            Ok(JsonRpcResponse::error(
                INTERNAL_ERROR,
                "Invalid response from metashrew".to_string(),
                request.id.clone(),
            ))
        }
    }

    pub async fn forward_to_memshrew(&self, request: &JsonRpcRequest) -> Result<JsonRpcResponse> {
        let response = self
            .client
            .post(&self.config.memshrew_url)
            .json(request)
            .send()
            .await?;

        let json_response: Value = response.json().await?;
        
        if let Some(result) = json_response.get("result") {
            Ok(JsonRpcResponse::success(result.clone(), request.id.clone()))
        } else {
            Ok(JsonRpcResponse::error(
                INTERNAL_ERROR,
                "Invalid response from memshrew".to_string(),
                request.id.clone(),
            ))
        }
    }

    pub async fn forward_to_bitcoind(&self, request: &JsonRpcRequest) -> Result<JsonRpcResponse> {
        let response = self
            .client
            .post(&self.config.bitcoin_rpc_url)
            .header("Authorization", self.config.bitcoin_rpc_auth_header())
            .header("Content-Type", "application/json")
            .json(request)
            .send()
            .await?;

        let json_response: Value = response.json().await?;

        // Handle bitcoind responses which include "error": null for success cases
        // This can't be deserialized directly into the untagged JsonRpcResponse enum
        if let Some(error) = json_response.get("error") {
            if !error.is_null() {
                return Ok(JsonRpcResponse::Error {
                    jsonrpc: "2.0".to_string(),
                    error: serde_json::from_value(error.clone())?,
                    id: request.id.clone(),
                });
            }
        }

        if let Some(result) = json_response.get("result") {
            Ok(JsonRpcResponse::success(result.clone(), request.id.clone()))
        } else {
            Ok(JsonRpcResponse::error(
                INTERNAL_ERROR,
                "Invalid response from bitcoind".to_string(),
                request.id.clone(),
            ))
        }
    }

    pub async fn fetch_ord_endpoint(&self, path: &str) -> Result<Value> {
        let url = format!("{}{}", self.config.ord_url, path);
        let response = self
            .client
            .get(&url)
            .header("Accept", "application/json")
            .send()
            .await?;

        let text = response.text().await?;
        
        match serde_json::from_str(&text) {
            Ok(json) => Ok(json),
            Err(_) => Ok(Value::String(text)),
        }
    }

    pub async fn fetch_ord_content(&self, inscription_id: &str) -> Result<Vec<u8>> {
        let url = format!("{}/content/{}", self.config.ord_url, inscription_id);
        let response = self.client.get(&url).send().await?;
        Ok(response.bytes().await?.to_vec())
    }

    pub async fn fetch_esplora_endpoint(&self, path: &str) -> Result<Value> {
        let url = format!("{}{}", self.config.esplora_url, path);
        let response = self
            .client
            .get(&url)
            .header("Accept", "application/json")
            .send()
            .await?;

        let text = response.text().await?;

        match serde_json::from_str(&text) {
            Ok(json) => Ok(json),
            Err(_) => Ok(Value::String(text)),
        }
    }

    pub async fn forward_to_subfrost(&self, request: &JsonRpcRequest) -> Result<JsonRpcResponse> {
        let url = self.config.subfrost_rpc_url.as_ref()
            .ok_or_else(|| anyhow::anyhow!("SUBFROST_RPC_URL not configured"))?;

        let response = self
            .client
            .post(url)
            .json(request)
            .send()
            .await?;

        let json_response: Value = response.json().await?;

        // Handle subfrost-rpc responses which may include "error": null for success cases
        if let Some(error) = json_response.get("error") {
            if !error.is_null() {
                return Ok(JsonRpcResponse::Error {
                    jsonrpc: "2.0".to_string(),
                    error: serde_json::from_value(error.clone())?,
                    id: request.id.clone(),
                });
            }
        }

        if let Some(result) = json_response.get("result") {
            Ok(JsonRpcResponse::success(result.clone(), request.id.clone()))
        } else {
            Ok(JsonRpcResponse::error(
                INTERNAL_ERROR,
                "Invalid response from subfrost-rpc".to_string(),
                request.id.clone(),
            ))
        }
    }
}
