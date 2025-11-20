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
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap();
        
        Self { base_url, client }
    }

    async fn post<T: Serialize, R: serde::de::DeserializeOwned>(
        &self,
        endpoint: &str,
        body: &T,
    ) -> Result<ApiResponse<R>> {
        let url = format!("{}/api/v1/{}", self.base_url, endpoint);
        let response = self.client
            .post(&url)
            .json(body)
            .send()
            .await
            .context("Failed to send request")?;
        
        let api_response = response
            .json::<ApiResponse<R>>()
            .await
            .context("Failed to parse response")?;
        
        if api_response.status_code != 200 {
            return Err(anyhow::anyhow!(
                "API error: {}",
                api_response.error.unwrap_or_else(|| "Unknown error".to_string())
            ));
        }
        
        Ok(api_response)
    }

    pub async fn health(&self) -> Result<()> {
        let url = format!("{}/api/v1/health", self.base_url);
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
        
        let response = self.post::<_, AlkanesResponse>("get-alkanes", &body).await?;
        Ok(response.data)
    }

    pub async fn get_alkanes_by_address(&self, address: &str) -> Result<Vec<AlkaneToken>> {
        let body = json!({ "address": address });
        let response = self.post::<_, Vec<AlkaneToken>>("get-alkanes-by-address", &body).await?;
        Ok(response.data)
    }

    pub async fn get_alkane_details(&self, id: &AlkaneId) -> Result<AlkaneToken> {
        let body = json!({ "id": { "block": id.block.to_string(), "tx": id.tx.to_string() } });
        let response = self.post::<_, AlkaneToken>("get-alkane-details", &body).await?;
        Ok(response.data)
    }

    // Pool endpoints
    pub async fn get_pools(&self, factory_id: &AlkaneId) -> Result<Vec<Pool>> {
        let body = json!({
            "factoryId": { "block": factory_id.block.to_string(), "tx": factory_id.tx.to_string() }
        });
        let response = self.post::<_, PoolsResponse>("get-pools", &body).await?;
        Ok(response.data.pools)
    }

    pub async fn get_pool_by_id(&self, pool_id: &AlkaneId) -> Result<Option<Pool>> {
        let body = json!({
            "poolId": { "block": pool_id.block.to_string(), "tx": pool_id.tx.to_string() }
        });
        let response = self.post::<_, Option<Pool>>("get-pool-by-id", &body).await?;
        Ok(response.data)
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
        let response = self.post::<_, HistoryResponse>("get-pool-history", &body).await?;
        Ok(response.data)
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
        let response = self.post::<_, HistoryResponse>("get-all-history", &body).await?;
        Ok(response.data)
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
        let response = self.post::<_, SwapHistoryResponse>("get-swap-history", &body).await?;
        Ok(response.data)
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
        let response = self.post::<_, MintHistoryResponse>("get-mint-history", &body).await?;
        Ok(response.data)
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
        let response = self.post::<_, BurnHistoryResponse>("get-burn-history", &body).await?;
        Ok(response.data)
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
        let response = self.post::<_, PoolCreationHistoryResponse>("get-pool-creation-history", &body).await?;
        Ok(response.data)
    }

    // Price endpoints
    pub async fn get_bitcoin_price(&self) -> Result<BitcoinPrice> {
        let response = self.post::<_, BitcoinPriceResponse>("get-bitcoin-price", &json!({})).await?;
        Ok(response.data.bitcoin)
    }

    pub async fn get_bitcoin_market_chart(&self, days: &str) -> Result<MarketChart> {
        let body = json!({ "days": days });
        let response = self.post::<_, MarketChart>("get-bitcoin-market-chart", &body).await?;
        Ok(response.data)
    }
}
