use alkanes_rpc_core::backend::*;
use alkanes_rpc_core::types::*;
use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;
use serde_json::{Value, json};

use crate::config::Config;

// ---------------------------------------------------------------------------
// ReqwestBitcoinBackend
// ---------------------------------------------------------------------------

pub struct ReqwestBitcoinBackend {
    client: Client,
    url: String,
    auth_header: String,
}

impl ReqwestBitcoinBackend {
    pub fn new(client: Client, config: &Config) -> Self {
        Self {
            client,
            url: config.bitcoin_rpc_url.clone(),
            auth_header: config.bitcoin_rpc_auth_header(),
        }
    }
}

#[async_trait(?Send)]
impl BitcoinBackend for ReqwestBitcoinBackend {
    async fn call(&self, method: &str, mut params: Vec<Value>, id: Value) -> Result<JsonRpcResponse> {
        // Production safety: cap generatetoaddress at 1 block
        if method == "generatetoaddress" && !params.is_empty() {
            if let Some(nblocks) = params[0].as_u64() {
                if nblocks > 1 {
                    params[0] = json!(1);
                }
            } else if let Some(nblocks) = params[0].as_i64() {
                if nblocks > 1 {
                    params[0] = json!(1);
                }
            }
        }

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params,
            id: id.clone(),
        };

        let response = self.client
            .post(&self.url)
            .header("Authorization", &self.auth_header)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        let json_response: Value = response.json().await?;

        if let Some(error) = json_response.get("error") {
            if !error.is_null() {
                return Ok(JsonRpcResponse::Error {
                    jsonrpc: "2.0".to_string(),
                    error: serde_json::from_value(error.clone())?,
                    id,
                });
            }
        }

        if let Some(result) = json_response.get("result") {
            Ok(JsonRpcResponse::success(result.clone(), id))
        } else {
            Ok(JsonRpcResponse::error(
                INTERNAL_ERROR,
                "Invalid response from bitcoind".to_string(),
                id,
            ))
        }
    }
}

// ---------------------------------------------------------------------------
// ReqwestMetashrewBackend
// ---------------------------------------------------------------------------

pub struct ReqwestMetashrewBackend {
    client: Client,
    url: String,
}

impl ReqwestMetashrewBackend {
    pub fn new(client: Client, config: &Config) -> Self {
        Self {
            client,
            url: config.metashrew_url.clone(),
        }
    }
}

#[async_trait(?Send)]
impl MetashrewBackend for ReqwestMetashrewBackend {
    async fn forward(&self, request: &JsonRpcRequest) -> Result<JsonRpcResponse> {
        let response = self.client
            .post(&self.url)
            .json(request)
            .send()
            .await?;

        let json_response: Value = response.json().await?;

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
}

// ---------------------------------------------------------------------------
// ReqwestEsploraBackend
// ---------------------------------------------------------------------------

pub struct ReqwestEsploraBackend {
    client: Client,
    url: String,
}

impl ReqwestEsploraBackend {
    pub fn new(client: Client, config: &Config) -> Self {
        Self {
            client,
            url: config.esplora_url.clone(),
        }
    }
}

#[async_trait(?Send)]
impl EsploraBackend for ReqwestEsploraBackend {
    async fn fetch(&self, path: &str) -> Result<Value> {
        let url = format!("{}{}", self.url, path);
        let response = self.client
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
}

// ---------------------------------------------------------------------------
// ReqwestOrdBackend
// ---------------------------------------------------------------------------

pub struct ReqwestOrdBackend {
    client: Client,
    url: String,
}

impl ReqwestOrdBackend {
    pub fn new(client: Client, config: &Config) -> Self {
        Self {
            client,
            url: config.ord_url.clone(),
        }
    }
}

#[async_trait(?Send)]
impl OrdBackend for ReqwestOrdBackend {
    async fn fetch(&self, path: &str) -> Result<Value> {
        let url = format!("{}{}", self.url, path);
        let response = self.client
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

    async fn fetch_content(&self, inscription_id: &str) -> Result<Vec<u8>> {
        let url = format!("{}/content/{}", self.url, inscription_id);
        let response = self.client.get(&url).send().await?;
        Ok(response.bytes().await?.to_vec())
    }
}
