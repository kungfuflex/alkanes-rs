use crate::cache::MetashrewViewCache;
use crate::config::Config;
use crate::jsonrpc::{JsonRpcRequest, JsonRpcResponse, INTERNAL_ERROR};
use anyhow::{anyhow, Result};
use reqwest::Client;
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;

/// Resilient POST → JSON helper. Retries up to 3 times with exponential backoff
/// (100ms, 300ms, 700ms) when either the send fails OR the response body fails
/// to decode as JSON. Bodies are read as text first so a decode failure has the
/// raw payload available for diagnostics rather than getting silently dropped.
///
/// Addresses the long-standing "error decoding response body" flake where
/// reqwest's body stream was getting cut mid-chunk under load — observed
/// 2026-06-11 at ~30% baseline failure rate on /v4/subfrost.
async fn post_json_with_retry(
    client: &Client,
    url: &str,
    body: &Value,
    extra_headers: &[(&str, &str)],
    label: &'static str,
) -> Result<Value> {
    let backoffs = [
        Duration::from_millis(100),
        Duration::from_millis(300),
        Duration::from_millis(700),
    ];
    let mut last_err: Option<anyhow::Error> = None;
    for (attempt, sleep) in std::iter::once(Duration::ZERO)
        .chain(backoffs.iter().copied())
        .enumerate()
    {
        if sleep > Duration::ZERO {
            tokio::time::sleep(sleep).await;
        }
        let mut req = client
            .post(url)
            .header("Content-Type", "application/json");
        for (k, v) in extra_headers {
            req = req.header(*k, *v);
        }
        let send_res = req.json(body).send().await;
        let response = match send_res {
            Ok(r) => r,
            Err(e) => {
                last_err = Some(anyhow!("{label} send attempt {attempt} failed: {e}"));
                continue;
            }
        };
        let text = match response.text().await {
            Ok(t) => t,
            Err(e) => {
                last_err = Some(anyhow!(
                    "{label} body read attempt {attempt} failed: {e}"
                ));
                continue;
            }
        };
        match serde_json::from_str::<Value>(&text) {
            Ok(v) => return Ok(v),
            Err(e) => {
                let snippet = text.chars().take(200).collect::<String>();
                last_err = Some(anyhow!(
                    "{label} decode attempt {attempt} failed: {e}; first 200 bytes: {snippet:?}"
                ));
                continue;
            }
        }
    }
    Err(last_err.unwrap_or_else(|| anyhow!("{label} failed with no captured error")))
}

#[derive(Clone)]
pub struct ProxyClient {
    client: Client,
    config: Config,
    /// Optional shared metashrew_view response cache (see cache.rs).
    /// None = cache disabled, forward_to_metashrew is pure passthrough.
    cache: Option<Arc<MetashrewViewCache>>,
}

impl ProxyClient {
    #[allow(dead_code)]
    pub fn new(config: Config) -> Self {
        Self::new_with_cache(config, None)
    }

    pub fn new_with_cache(config: Config, cache: Option<Arc<MetashrewViewCache>>) -> Self {
        Self {
            client: Client::new(),
            config,
            cache,
        }
    }

    /// Access the shared metashrew_view cache, if configured. Used by
    /// fan-out handlers (see protorunesbyaddress.rs) to read the pool
    /// watermark and pin H for the duration of a request.
    pub fn cache(&self) -> Option<&Arc<MetashrewViewCache>> {
        self.cache.as_ref()
    }

    /// Access the proxy config — handlers use this to call esplora /
    /// metashrew endpoints directly (e.g. the in-process fan-out helper
    /// reaches esplora_address::utxo without going through the JSON-RPC
    /// router because the upstream URL is the same one we already use).
    #[allow(dead_code)]
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Access the underlying reqwest client — handlers can reuse the
    /// connection pool instead of opening fresh sockets.
    #[allow(dead_code)]
    pub fn client(&self) -> &Client {
        &self.client
    }

    pub async fn forward_to_metashrew(&self, request: &JsonRpcRequest) -> Result<JsonRpcResponse> {
        // Cache fast-path: metashrew_view calls are deterministic per
        // (method, args, block_hash). Resolve the request's block_tag to a
        // stable (height, block_hash) pair; on hit return immediately; on
        // miss REWRITE the request's block_tag to the explicit height
        // before sending upstream — this guarantees upstream computes at
        // the exact height we keyed (otherwise openresty might rewrite
        // "latest" to a slightly-different served_height by the time the
        // upstream call lands, and we'd store a response under the wrong
        // key → asset-burn risk on the next lookup).
        //
        // Any cache-side failure (Redis down, block-hash resolution error)
        // degrades to passthrough — we never break a request on a cache
        // bug. Cache only applies to method == "metashrew_view"; other
        // methods (metashrew_height, etc.) bypass the cache layer.
        let mut upstream_request = request;
        let mut rewritten: Option<JsonRpcRequest> = None;
        let cache_keyed_hash = if let Some(cache) = self.cache.as_ref() {
            if request.method == "metashrew_view" {
                match cache.resolve_block_hash(request.params.get(2)).await {
                    Ok((height, hash)) => {
                        match cache.lookup(request, &hash).await {
                            Ok(Some(hit)) => return Ok(hit),
                            Ok(None) => {
                                // Rewrite the request body so upstream uses
                                // the explicit height. This is the
                                // critical safety step: it removes the
                                // upstream-race window where the upstream
                                // could compute against a different
                                // served_height than the one we used for
                                // the cache key.
                                let mut req = request.clone();
                                while req.params.len() < 3 {
                                    req.params.push(Value::Null);
                                }
                                req.params[2] = Value::String(height.to_string());
                                rewritten = Some(req);
                                upstream_request = rewritten.as_ref().unwrap();
                                Some(hash)
                            }
                            Err(e) => {
                                log::warn!("metashrew_view cache: lookup failed: {}", e);
                                None
                            }
                        }
                    }
                    Err(e) => {
                        log::debug!("metashrew_view cache: resolve_block_hash skipped: {}", e);
                        None
                    }
                }
            } else {
                None
            }
        } else {
            None
        };

        let body = serde_json::to_value(upstream_request)?;
        let json_response = post_json_with_retry(
            &self.client,
            &self.config.metashrew_url,
            &body,
            &[],
            "metashrew",
        )
        .await?;

        let parsed = if let Some(error) = json_response.get("error") {
            JsonRpcResponse::Error {
                jsonrpc: "2.0".to_string(),
                error: serde_json::from_value(error.clone())?,
                id: request.id.clone(),
            }
        } else if let Some(result) = json_response.get("result") {
            JsonRpcResponse::success(result.clone(), request.id.clone())
        } else {
            JsonRpcResponse::error(
                INTERNAL_ERROR,
                "Invalid response from metashrew".to_string(),
                request.id.clone(),
            )
        };

        // Store on cache miss path. Only Success responses are cached
        // (cache.store handles the Error filter internally).
        if let (Some(cache), Some(hash)) = (self.cache.as_ref(), cache_keyed_hash) {
            cache.store(request, &parsed, &hash).await;
        }

        Ok(parsed)
    }

    /// Forward to the dedicated metashrew-unwrap endpoint if configured,
    /// otherwise fall back to the main metashrew endpoint.
    pub async fn forward_to_metashrew_unwrap(&self, request: &JsonRpcRequest) -> Result<JsonRpcResponse> {
        let url = self.config.metashrew_unwrap_url.as_deref()
            .unwrap_or(&self.config.metashrew_url);

        let body = serde_json::to_value(request)?;
        let json_response = post_json_with_retry(
            &self.client,
            url,
            &body,
            &[],
            "metashrew_unwrap",
        )
        .await?;

        if let Some(error) = json_response.get("error") {
            Ok(JsonRpcResponse::Error {
                jsonrpc: "2.0".to_string(),
                error: serde_json::from_value(error.clone())?,
                id: request.id.clone(),
            })
        } else if let Some(result) = json_response.get("result") {
            Ok(JsonRpcResponse::success(result.clone(), request.id.clone()))
        } else {
            Ok(JsonRpcResponse::error(
                INTERNAL_ERROR,
                "Invalid response from metashrew-unwrap".to_string(),
                request.id.clone(),
            ))
        }
    }

    pub async fn forward_to_memshrew(&self, request: &JsonRpcRequest) -> Result<JsonRpcResponse> {
        let body = serde_json::to_value(request)?;
        let json_response = post_json_with_retry(
            &self.client,
            &self.config.memshrew_url,
            &body,
            &[],
            "memshrew",
        )
        .await?;

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

    pub async fn forward_to_subfrost(&self, request: &JsonRpcRequest) -> Result<JsonRpcResponse> {
        let body = serde_json::to_value(request)?;
        let json_response = post_json_with_retry(
            &self.client,
            &self.config.subfrost_url,
            &body,
            &[],
            "subfrost",
        )
        .await?;

        // Check for error first (error field is not null)
        if let Some(error) = json_response.get("error") {
            if !error.is_null() {
                return Ok(JsonRpcResponse::Error {
                    jsonrpc: "2.0".to_string(),
                    error: serde_json::from_value(error.clone())?,
                    id: request.id.clone(),
                });
            }
        }
        
        // Check for result
        if let Some(result) = json_response.get("result") {
            Ok(JsonRpcResponse::success(result.clone(), request.id.clone()))
        } else {
            Ok(JsonRpcResponse::error(
                INTERNAL_ERROR,
                "Invalid response from subfrost".to_string(),
                request.id.clone(),
            ))
        }
    }

    pub async fn forward_to_bitcoind(&self, request: &JsonRpcRequest) -> Result<JsonRpcResponse> {
        let auth = self.config.bitcoin_rpc_auth_header();
        let body = serde_json::to_value(request)?;
        let json_response = post_json_with_retry(
            &self.client,
            &self.config.bitcoin_rpc_url,
            &body,
            &[("Authorization", auth.as_str())],
            "bitcoind",
        )
        .await?;
        Ok(serde_json::from_value(json_response)?)
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
}
