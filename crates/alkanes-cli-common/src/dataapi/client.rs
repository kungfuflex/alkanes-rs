use anyhow::{Context, Result};
use serde::Serialize;
use serde_json::json;

use super::types::*;
use crate::alkanes::types::AlkaneId;

#[cfg(feature = "std")]
pub struct DataApiClient {
    base_url: String,
    client: reqwest::Client,
}

#[cfg(feature = "std")]
impl DataApiClient {
    pub fn new(base_url: String) -> Self {
        let mut builder = reqwest::Client::builder();
        
        // timeout() is not available in WASM
        #[cfg(not(target_arch = "wasm32"))]
        {
            builder = builder.timeout(std::time::Duration::from_secs(30));
        }
        
        let client = builder.build().unwrap();
        
        Self { base_url, client }
    }

    fn build_url(&self, endpoint: &str) -> String {
        let base = self.base_url.trim_end_matches('/');
        format!("{}/{}", base, endpoint)
    }

    async fn post<T: Serialize, R: serde::de::DeserializeOwned>(
        &self,
        endpoint: &str,
        body: &T,
    ) -> Result<R> {
        let url = self.build_url(endpoint);
        let response = self.client
            .post(&url)
            .json(body)
            .send()
            .await
            .context("Failed to send request")?;

        // First get the response as a generic JSON value to check for errors
        let json_value: serde_json::Value = response
            .json()
            .await
            .context("Failed to parse response")?;

        // Check if the response indicates an error
        if let Some(ok) = json_value.get("ok") {
            if ok == false {
                let error_msg = json_value
                    .get("error")
                    .and_then(|e| e.as_str())
                    .unwrap_or("Unknown error");
                return Err(anyhow::anyhow!("API error: {}", error_msg));
            }
        }

        // Handle wrapped responses with statusCode/data envelope
        let data_value = if let Some(status_code) = json_value.get("statusCode") {
            // This is a wrapped response
            if let Some(error) = json_value.get("error").and_then(|e| e.as_str()) {
                if !error.is_empty() {
                    return Err(anyhow::anyhow!("API error: {}", error));
                }
            }
            // Extract the data field
            json_value.get("data").cloned().unwrap_or(json_value.clone())
        } else {
            json_value
        };

        // Parse the data into the expected type
        let result: R = serde_json::from_value(data_value)
            .context("Failed to deserialize response")?;

        Ok(result)
    }

    /// Make a POST request and return the raw response text without parsing.
    /// Useful for debugging when response cannot be decoded.
    pub async fn post_raw<T: Serialize>(
        &self,
        endpoint: &str,
        body: &T,
    ) -> Result<String> {
        let url = self.build_url(endpoint);
        let response = self.client
            .post(&url)
            .json(body)
            .send()
            .await
            .context("Failed to send request")?;
        
        let text = response
            .text()
            .await
            .context("Failed to read response text")?;
        
        Ok(text)
    }

    /// Make a GET request and return the raw response text without parsing.
    /// Useful for debugging when response cannot be decoded.
    pub async fn get_raw(&self, endpoint: &str) -> Result<String> {
        let url = self.build_url(endpoint);
        let response = self.client
            .get(&url)
            .send()
            .await
            .context("Failed to send request")?;

        let text = response
            .text()
            .await
            .context("Failed to read response text")?;

        Ok(text)
    }

    async fn get<R: serde::de::DeserializeOwned>(&self, endpoint: &str) -> Result<R> {
        let url = self.build_url(endpoint);
        let response = self.client
            .get(&url)
            .send()
            .await
            .context("Failed to send request")?;

        let json_value: serde_json::Value = response
            .json()
            .await
            .context("Failed to parse response")?;

        // Handle wrapped responses with statusCode/data envelope
        let data_value = if json_value.get("statusCode").is_some() {
            if let Some(error) = json_value.get("error").and_then(|e| e.as_str()) {
                if !error.is_empty() {
                    return Err(anyhow::anyhow!("API error: {}", error));
                }
            }
            json_value.get("data").cloned().unwrap_or(json_value.clone())
        } else {
            json_value
        };

        let result: R = serde_json::from_value(data_value)
            .context("Failed to deserialize response")?;

        Ok(result)
    }

    pub async fn health(&self) -> Result<()> {
        let url = self.build_url("health");
        let response = self.client.get(&url).send().await?;
        
        if response.status().is_success() {
            Ok(())
        } else {
            Err(anyhow::anyhow!("Health check failed"))
        }
    }

    // Alkanes endpoints
    pub async fn get_alkanes(
        &self,
        limit: Option<i32>,
        offset: Option<i32>,
        sort_by: Option<String>,
        order: Option<String>,
        search_query: Option<String>,
    ) -> Result<AlkanesResponse> {
        let body = json!({
            "limit": limit,
            "offset": offset,
            "sortBy": sort_by,
            "order": order,
            "searchQuery": search_query,
        });

        self.post::<_, AlkanesResponse>("get-alkanes", &body).await
    }

    pub async fn get_alkanes_by_address(&self, address: &str) -> Result<Vec<AlkaneToken>> {
        let body = json!({ "address": address });
        self.post::<_, Vec<AlkaneToken>>("get-alkanes-by-address", &body).await
    }

    pub async fn get_alkane_details(&self, id: &AlkaneId) -> Result<AlkaneToken> {
        let body = json!({ "id": { "block": id.block.to_string(), "tx": id.tx.to_string() } });
        self.post::<_, AlkaneToken>("get-alkane-details", &body).await
    }

    // Pool endpoints
    pub async fn get_pools(&self, factory_id: &AlkaneId) -> Result<PoolsResponse> {
        let body = json!({
            "factoryId": { "block": factory_id.block.to_string(), "tx": factory_id.tx.to_string() }
        });
        self.post::<_, PoolsResponse>("get-pools", &body).await
    }

    pub async fn get_pool_by_id(&self, pool_id: &AlkaneId) -> Result<Option<Pool>> {
        let body = json!({
            "poolId": { "block": pool_id.block.to_string(), "tx": pool_id.tx.to_string() }
        });
        self.post::<_, Option<Pool>>("get-pool-by-id", &body).await
    }

    pub async fn get_pool_history(
        &self,
        pool_id: &AlkaneId,
        category: Option<String>,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<HistoryResponse> {
        let body = json!({
            "poolId": { "block": pool_id.block.to_string(), "tx": pool_id.tx.to_string() },
            "category": category,
            "limit": limit,
            "offset": offset,
        });
        self.post::<_, HistoryResponse>("get-pool-history", &body).await
    }

    // History endpoints
    pub async fn get_all_history(
        &self,
        limit: Option<i32>,
        offset: Option<i32>,
        successful_only: bool,
    ) -> Result<HistoryResponse> {
        let body = json!({
            "limit": limit,
            "offset": offset,
            "successfulOnly": successful_only,
        });
        self.post::<_, HistoryResponse>("get-all-history", &body).await
    }

    pub async fn get_swap_history(
        &self,
        pool_id: Option<&AlkaneId>,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<SwapHistoryResponse> {
        let body = json!({
            "poolId": pool_id.map(|id| json!({ "block": id.block.to_string(), "tx": id.tx.to_string() })),
            "limit": limit,
            "offset": offset,
        });
        self.post::<_, SwapHistoryResponse>("get-swap-history", &body).await
    }

    pub async fn get_mint_history(
        &self,
        pool_id: &AlkaneId,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<MintHistoryResponse> {
        let body = json!({
            "poolId": { "block": pool_id.block, "tx": pool_id.tx },
            "limit": limit,
            "offset": offset,
        });
        self.post::<_, MintHistoryResponse>("get-mint-history", &body).await
    }

    pub async fn get_burn_history(
        &self,
        pool_id: &AlkaneId,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<BurnHistoryResponse> {
        let body = json!({
            "poolId": { "block": pool_id.block, "tx": pool_id.tx },
            "limit": limit,
            "offset": offset,
        });
        self.post::<_, BurnHistoryResponse>("get-burn-history", &body).await
    }

    pub async fn get_pool_creation_history(
        &self,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<PoolCreationHistoryResponse> {
        let body = json!({
            "limit": limit,
            "offset": offset,
        });
        self.post::<_, PoolCreationHistoryResponse>("get-pool-creation-history", &body).await
    }

    // Price endpoints
    pub async fn get_bitcoin_price(&self) -> Result<BitcoinPriceResponse> {
        self.post::<_, BitcoinPriceResponse>("get-bitcoin-price", &json!({})).await
    }

    pub async fn get_bitcoin_market_chart(&self, days: &str) -> Result<MarketChart> {
        let body = json!({ "days": days });
        self.post::<_, MarketChart>("get-bitcoin-market-chart", &body).await
    }

    // New balance endpoints
    pub async fn get_address_balances(&self, address: &str, include_outpoints: bool) -> Result<serde_json::Value> {
        let body = serde_json::json!({
            "address": address,
            "include_outpoints": include_outpoints
        });
        self.post::<_, serde_json::Value>("get-address-balances", &body).await
    }

    pub async fn get_holders(&self, alkane: &str, page: i64, limit: i64) -> Result<serde_json::Value> {
        let body = serde_json::json!({
            "alkane": alkane,
            "page": page,
            "limit": limit
        });
        self.post::<_, serde_json::Value>("get-alkane-holders", &body).await
    }

    pub async fn get_holders_count(&self, alkane: &str) -> Result<serde_json::Value> {
        let body = serde_json::json!({"alkane": alkane});
        self.post::<_, serde_json::Value>("get-alkane-holders-count", &body).await
    }

    pub async fn get_outpoint_balances(&self, outpoint: &str) -> Result<serde_json::Value> {
        let body = serde_json::json!({"outpoint": outpoint});
        self.post::<_, serde_json::Value>("get-outpoint-balances", &body).await
    }

    // Storage endpoint
    pub async fn get_keys(&self, alkane: &str, prefix: Option<String>, limit: i64) -> Result<serde_json::Value> {
        let mut body = serde_json::json!({
            "alkane": alkane,
            "limit": limit
        });
        if let Some(p) = prefix {
            body["prefix"] = serde_json::Value::String(p);
        }
        self.post::<_, serde_json::Value>("get-keys", &body).await
    }

    // AMM endpoints
    pub async fn get_trades(&self, pool: &str, start_time: Option<i64>, end_time: Option<i64>, limit: i64) -> Result<serde_json::Value> {
        let mut body = serde_json::json!({
            "pool": pool,
            "limit": limit
        });
        if let Some(st) = start_time {
            body["start_time"] = serde_json::Value::from(st);
        }
        if let Some(et) = end_time {
            body["end_time"] = serde_json::Value::from(et);
        }
        self.post::<_, serde_json::Value>("get-trades", &body).await
    }

    pub async fn get_candles(&self, pool: &str, interval: &str, start_time: Option<i64>, end_time: Option<i64>, limit: i64) -> Result<serde_json::Value> {
        let mut body = serde_json::json!({
            "pool": pool,
            "interval": interval,
            "limit": limit
        });
        if let Some(st) = start_time {
            body["start_time"] = serde_json::Value::from(st);
        }
        if let Some(et) = end_time {
            body["end_time"] = serde_json::Value::from(et);
        }
        self.post::<_, serde_json::Value>("get-candles", &body).await
    }

    pub async fn get_reserves(&self, pool: &str) -> Result<serde_json::Value> {
        let body = serde_json::json!({"pool": pool});
        self.post::<_, serde_json::Value>("get-reserves", &body).await
    }

    // Indexer status endpoints
    pub async fn get_block_height(&self) -> Result<IndexerBlockHeightResponse> {
        self.get("blockheight").await
    }

    pub async fn get_block_hash(&self) -> Result<IndexerBlockHashResponse> {
        self.get("blockhash").await
    }

    pub async fn get_indexer_position(&self) -> Result<IndexerPositionResponse> {
        self.get("indexer-position").await
    }

    // Additional price endpoints
    pub async fn get_bitcoin_market_weekly(&self) -> Result<serde_json::Value> {
        self.post::<_, serde_json::Value>("get-bitcoin-market-weekly", &json!({})).await
    }

    pub async fn get_bitcoin_markets(&self) -> Result<serde_json::Value> {
        self.post::<_, serde_json::Value>("get-bitcoin-markets", &json!({})).await
    }

    // Alkanes UTXO endpoints
    pub async fn get_alkanes_utxo(&self, address: &str) -> Result<serde_json::Value> {
        let body = json!({ "address": address });
        self.post::<_, serde_json::Value>("get-alkanes-utxo", &body).await
    }

    pub async fn get_amm_utxos(&self, address: &str) -> Result<serde_json::Value> {
        let body = json!({ "address": address });
        self.post::<_, serde_json::Value>("get-amm-utxos", &body).await
    }

    // Search endpoint
    pub async fn global_alkanes_search(&self, query: &str, limit: Option<i32>, offset: Option<i32>) -> Result<serde_json::Value> {
        let body = json!({
            "searchQuery": query,
            "limit": limit,
            "offset": offset
        });
        self.post::<_, serde_json::Value>("global-alkanes-search", &body).await
    }

    // Address outpoints endpoint
    pub async fn get_address_outpoints(&self, address: &str) -> Result<serde_json::Value> {
        let body = json!({ "address": address });
        self.post::<_, serde_json::Value>("get-address-outpoints", &body).await
    }

    // Pathfind endpoint
    pub async fn pathfind(&self, token_in: &str, token_out: &str, amount_in: &str, max_hops: Option<i32>) -> Result<serde_json::Value> {
        let body = json!({
            "token_in": token_in,
            "token_out": token_out,
            "amount_in": amount_in,
            "max_hops": max_hops.unwrap_or(3)
        });
        self.post::<_, serde_json::Value>("pathfind", &body).await
    }

    // Pool detail endpoints
    pub async fn get_pool_details(&self, pool_id: &AlkaneId) -> Result<serde_json::Value> {
        let body = json!({
            "poolId": { "block": pool_id.block.to_string(), "tx": pool_id.tx.to_string() }
        });
        self.post::<_, serde_json::Value>("get-pool-details", &body).await
    }

    pub async fn get_all_pools_details(
        &self,
        factory_id: &AlkaneId,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<serde_json::Value> {
        let body = json!({
            "factoryId": { "block": factory_id.block.to_string(), "tx": factory_id.tx.to_string() },
            "limit": limit,
            "offset": offset
        });
        self.post::<_, serde_json::Value>("get-all-pools-details", &body).await
    }

    // Position endpoints
    pub async fn get_address_positions(&self, address: &str, factory_id: &AlkaneId) -> Result<serde_json::Value> {
        let body = json!({
            "address": address,
            "factoryId": { "block": factory_id.block.to_string(), "tx": factory_id.tx.to_string() }
        });
        self.post::<_, serde_json::Value>("address-positions", &body).await
    }

    // Token pairs endpoints
    pub async fn get_token_pairs(
        &self,
        factory_id: &AlkaneId,
        alkane_id: Option<&AlkaneId>,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<serde_json::Value> {
        let mut body = json!({
            "factoryId": { "block": factory_id.block.to_string(), "tx": factory_id.tx.to_string() },
            "limit": limit,
            "offset": offset
        });
        if let Some(id) = alkane_id {
            body["alkaneId"] = json!({ "block": id.block.to_string(), "tx": id.tx.to_string() });
        }
        self.post::<_, serde_json::Value>("get-token-pairs", &body).await
    }

    pub async fn get_all_token_pairs(
        &self,
        factory_id: &AlkaneId,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<serde_json::Value> {
        let body = json!({
            "factoryId": { "block": factory_id.block.to_string(), "tx": factory_id.tx.to_string() },
            "limit": limit,
            "offset": offset
        });
        self.post::<_, serde_json::Value>("get-all-token-pairs", &body).await
    }

    pub async fn get_alkane_swap_pair_details(
        &self,
        factory_id: &AlkaneId,
        token_a_id: &AlkaneId,
        token_b_id: &AlkaneId,
    ) -> Result<serde_json::Value> {
        let body = json!({
            "factoryId": { "block": factory_id.block.to_string(), "tx": factory_id.tx.to_string() },
            "tokenAId": { "block": token_a_id.block.to_string(), "tx": token_a_id.tx.to_string() },
            "tokenBId": { "block": token_b_id.block.to_string(), "tx": token_b_id.tx.to_string() }
        });
        self.post::<_, serde_json::Value>("get-alkane-swap-pair-details", &body).await
    }

    // Additional history endpoints
    pub async fn get_pool_swap_history(
        &self,
        pool_id: Option<&AlkaneId>,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<serde_json::Value> {
        let mut body = json!({
            "limit": limit,
            "offset": offset
        });
        if let Some(id) = pool_id {
            body["poolId"] = json!({ "block": id.block.to_string(), "tx": id.tx.to_string() });
        }
        self.post::<_, serde_json::Value>("get-pool-swap-history", &body).await
    }

    pub async fn get_token_swap_history(
        &self,
        alkane_id: &AlkaneId,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<serde_json::Value> {
        let body = json!({
            "alkaneId": { "block": alkane_id.block.to_string(), "tx": alkane_id.tx.to_string() },
            "limit": limit,
            "offset": offset
        });
        self.post::<_, serde_json::Value>("get-token-swap-history", &body).await
    }

    pub async fn get_pool_mint_history(
        &self,
        pool_id: Option<&AlkaneId>,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<serde_json::Value> {
        let mut body = json!({
            "limit": limit,
            "offset": offset
        });
        if let Some(id) = pool_id {
            body["poolId"] = json!({ "block": id.block.to_string(), "tx": id.tx.to_string() });
        }
        self.post::<_, serde_json::Value>("get-pool-mint-history", &body).await
    }

    pub async fn get_pool_burn_history(
        &self,
        pool_id: Option<&AlkaneId>,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<serde_json::Value> {
        let mut body = json!({
            "limit": limit,
            "offset": offset
        });
        if let Some(id) = pool_id {
            body["poolId"] = json!({ "block": id.block.to_string(), "tx": id.tx.to_string() });
        }
        self.post::<_, serde_json::Value>("get-pool-burn-history", &body).await
    }

    // Address-specific history endpoints
    pub async fn get_address_swap_history_for_pool(
        &self,
        address: &str,
        pool_id: &AlkaneId,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<serde_json::Value> {
        let body = json!({
            "address": address,
            "poolId": { "block": pool_id.block.to_string(), "tx": pool_id.tx.to_string() },
            "limit": limit,
            "offset": offset
        });
        self.post::<_, serde_json::Value>("get-address-swap-history-for-pool", &body).await
    }

    pub async fn get_address_swap_history_for_token(
        &self,
        address: &str,
        alkane_id: &AlkaneId,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<serde_json::Value> {
        let body = json!({
            "address": address,
            "alkaneId": { "block": alkane_id.block.to_string(), "tx": alkane_id.tx.to_string() },
            "limit": limit,
            "offset": offset
        });
        self.post::<_, serde_json::Value>("get-address-swap-history-for-token", &body).await
    }

    // Wrap/unwrap history endpoints
    pub async fn get_address_wrap_history(
        &self,
        address: &str,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<serde_json::Value> {
        let body = json!({
            "address": address,
            "limit": limit,
            "offset": offset
        });
        self.post::<_, serde_json::Value>("get-address-wrap-history", &body).await
    }

    pub async fn get_address_unwrap_history(
        &self,
        address: &str,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<serde_json::Value> {
        let body = json!({
            "address": address,
            "limit": limit,
            "offset": offset
        });
        self.post::<_, serde_json::Value>("get-address-unwrap-history", &body).await
    }

    pub async fn get_all_wrap_history(
        &self,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<serde_json::Value> {
        let body = json!({
            "limit": limit,
            "offset": offset
        });
        self.post::<_, serde_json::Value>("get-all-wrap-history", &body).await
    }

    pub async fn get_all_unwrap_history(
        &self,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<serde_json::Value> {
        let body = json!({
            "limit": limit,
            "offset": offset
        });
        self.post::<_, serde_json::Value>("get-all-unwrap-history", &body).await
    }

    pub async fn get_total_unwrap_amount(&self) -> Result<serde_json::Value> {
        self.post::<_, serde_json::Value>("get-total-unwrap-amount", &json!({})).await
    }

    // Address pool history endpoints
    pub async fn get_address_pool_creation_history(
        &self,
        address: &str,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<serde_json::Value> {
        let body = json!({
            "address": address,
            "limit": limit,
            "offset": offset
        });
        self.post::<_, serde_json::Value>("get-address-pool-creation-history", &body).await
    }

    pub async fn get_address_pool_mint_history(
        &self,
        address: &str,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<serde_json::Value> {
        let body = json!({
            "address": address,
            "limit": limit,
            "offset": offset
        });
        self.post::<_, serde_json::Value>("get-address-pool-mint-history", &body).await
    }

    pub async fn get_address_pool_burn_history(
        &self,
        address: &str,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<serde_json::Value> {
        let body = json!({
            "address": address,
            "limit": limit,
            "offset": offset
        });
        self.post::<_, serde_json::Value>("get-address-pool-burn-history", &body).await
    }

    // All AMM transaction history
    pub async fn get_all_address_amm_tx_history(
        &self,
        address: &str,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<serde_json::Value> {
        let body = json!({
            "address": address,
            "limit": limit,
            "offset": offset
        });
        self.post::<_, serde_json::Value>("get-all-address-amm-tx-history", &body).await
    }

    pub async fn get_all_amm_tx_history(
        &self,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<serde_json::Value> {
        let body = json!({
            "limit": limit,
            "offset": offset
        });
        self.post::<_, serde_json::Value>("get-all-amm-tx-history", &body).await
    }

    // Bitcoin/UTXO endpoints
    pub async fn get_address_balance(&self, address: &str) -> Result<serde_json::Value> {
        let body = json!({ "address": address });
        self.post::<_, serde_json::Value>("get-address-balance", &body).await
    }

    pub async fn get_taproot_balance(&self, address: &str) -> Result<serde_json::Value> {
        let body = json!({ "address": address });
        self.post::<_, serde_json::Value>("get-taproot-balance", &body).await
    }

    pub async fn get_address_utxos(&self, address: &str) -> Result<serde_json::Value> {
        let body = json!({ "address": address });
        self.post::<_, serde_json::Value>("get-address-utxos", &body).await
    }

    pub async fn get_account_utxos(&self, account: &str) -> Result<serde_json::Value> {
        let body = json!({ "account": account });
        self.post::<_, serde_json::Value>("get-account-utxos", &body).await
    }

    pub async fn get_account_balance(&self, account: &str) -> Result<serde_json::Value> {
        let body = json!({ "account": account });
        self.post::<_, serde_json::Value>("get-account-balance", &body).await
    }

    pub async fn get_taproot_history(&self, taproot_address: &str, total_txs: i32) -> Result<serde_json::Value> {
        let body = json!({
            "taprootAddress": taproot_address,
            "totalTxs": total_txs
        });
        self.post::<_, serde_json::Value>("get-taproot-history", &body).await
    }

    pub async fn get_intent_history(
        &self,
        address: &str,
        total_txs: Option<i32>,
        last_seen_tx_id: Option<&str>,
    ) -> Result<serde_json::Value> {
        let mut body = json!({ "address": address });
        if let Some(txs) = total_txs {
            body["totalTxs"] = json!(txs);
        }
        if let Some(tx_id) = last_seen_tx_id {
            body["lastSeenTxId"] = json!(tx_id);
        }
        self.post::<_, serde_json::Value>("get-intent-history", &body).await
    }
}
