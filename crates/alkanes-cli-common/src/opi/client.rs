/// OPI (Open Protocol Indexer) HTTP client
/// Based on https://github.com/bestinslot-xyz/OPI/blob/main/modules/brc20_api/api.js

use anyhow::{Context, Result};
use super::types::*;

#[cfg(feature = "std")]
pub struct OpiClient {
    base_url: String,
    client: reqwest::Client,
}

#[cfg(feature = "std")]
impl OpiClient {
    pub fn new(base_url: String) -> Self {
        let mut builder = reqwest::Client::builder();

        #[cfg(not(target_arch = "wasm32"))]
        {
            builder = builder.timeout(std::time::Duration::from_secs(30));
        }

        let client = builder.build().unwrap();

        Self { base_url, client }
    }

    pub fn from_config(config: OpiConfig) -> Self {
        Self::new(config.base_url)
    }

    fn build_url(&self, endpoint: &str) -> String {
        let base = self.base_url.trim_end_matches('/');
        format!("{}/{}", base, endpoint)
    }

    async fn get_text(&self, endpoint: &str) -> Result<String> {
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

    async fn get<R: serde::de::DeserializeOwned>(&self, endpoint: &str) -> Result<OpiResponse<R>> {
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

        // Handle simple value responses (like block_height returns just a number)
        if json_value.is_number() || json_value.is_string() || json_value.is_null() {
            // Try to deserialize directly
            if let Ok(result) = serde_json::from_value::<R>(json_value.clone()) {
                return Ok(OpiResponse {
                    error: None,
                    result: Some(result),
                });
            }
        }

        // Try to deserialize as OpiResponse
        let result: OpiResponse<R> = serde_json::from_value(json_value)
            .context("Failed to deserialize response")?;

        Ok(result)
    }

    /// Get OPI BRC-20 block height
    pub async fn get_block_height(&self) -> Result<Option<u64>> {
        let text = self.get_text("v1/brc20/block_height").await?;
        if text == "null" || text.is_empty() {
            return Ok(None);
        }
        let height: u64 = text.parse().context("Failed to parse block height")?;
        Ok(Some(height))
    }

    /// Get OPI BRC-20 extras block height
    pub async fn get_extras_block_height(&self) -> Result<Option<u64>> {
        let text = self.get_text("v1/brc20/extras_block_height").await?;
        if text == "null" || text.is_empty() {
            return Ok(None);
        }
        let height: u64 = text.parse().context("Failed to parse extras block height")?;
        Ok(Some(height))
    }

    /// Get OPI BRC-20 DB version
    pub async fn get_db_version(&self) -> Result<String> {
        let text = self.get_text("v1/brc20/db_version").await?;
        Ok(text)
    }

    /// Get OPI BRC-20 event hash version
    pub async fn get_event_hash_version(&self) -> Result<String> {
        let text = self.get_text("v1/brc20/event_hash_version").await?;
        Ok(text)
    }

    /// Get balance at a specific block height
    pub async fn get_balance_on_block(
        &self,
        block_height: u64,
        pkscript: &str,
        ticker: &str,
    ) -> Result<OpiResponse<Balance>> {
        let endpoint = format!(
            "v1/brc20/balance_on_block?block_height={}&pkscript={}&ticker={}",
            block_height, pkscript, ticker
        );
        self.get(&endpoint).await
    }

    /// Get all BRC-20 activity for a block
    pub async fn get_activity_on_block(&self, block_height: u64) -> Result<OpiResponse<Vec<Brc20Event>>> {
        let endpoint = format!("v1/brc20/activity_on_block?block_height={}", block_height);
        self.get(&endpoint).await
    }

    /// Get Bitcoin RPC results for a block
    pub async fn get_bitcoin_rpc_results_on_block(&self, block_height: u64) -> Result<OpiResponse<Vec<BitcoinRpcResult>>> {
        let endpoint = format!("v1/brc20/bitcoin_rpc_results_on_block?block_height={}", block_height);
        self.get(&endpoint).await
    }

    /// Get current balance of a wallet
    pub async fn get_current_balance_of_wallet(
        &self,
        ticker: &str,
        address: Option<&str>,
        pkscript: Option<&str>,
    ) -> Result<OpiResponse<Balance>> {
        let mut endpoint = format!("v1/brc20/get_current_balance_of_wallet?ticker={}", ticker);
        if let Some(addr) = address {
            endpoint.push_str(&format!("&address={}", addr));
        }
        if let Some(pk) = pkscript {
            endpoint.push_str(&format!("&pkscript={}", pk));
        }
        self.get(&endpoint).await
    }

    /// Get valid TX notes for a wallet
    pub async fn get_valid_tx_notes_of_wallet(
        &self,
        address: Option<&str>,
        pkscript: Option<&str>,
    ) -> Result<OpiResponse<ValidTxNotesResponse>> {
        let mut endpoint = "v1/brc20/get_valid_tx_notes_of_wallet?".to_string();
        if let Some(addr) = address {
            endpoint.push_str(&format!("address={}", addr));
        }
        if let Some(pk) = pkscript {
            if address.is_some() {
                endpoint.push('&');
            }
            endpoint.push_str(&format!("pkscript={}", pk));
        }
        self.get(&endpoint).await
    }

    /// Get valid TX notes for a ticker
    pub async fn get_valid_tx_notes_of_ticker(&self, ticker: &str) -> Result<OpiResponse<ValidTxNotesResponse>> {
        let endpoint = format!("v1/brc20/get_valid_tx_notes_of_ticker?ticker={}", ticker);
        self.get(&endpoint).await
    }

    /// Get holders of a ticker
    pub async fn get_holders(&self, ticker: &str) -> Result<OpiResponse<HoldersResponse>> {
        let endpoint = format!("v1/brc20/holders?ticker={}", ticker);
        self.get(&endpoint).await
    }

    /// Get hash of all activity at a block
    pub async fn get_hash_of_all_activity(&self, block_height: u64) -> Result<OpiResponse<HashResponse>> {
        let endpoint = format!("v1/brc20/get_hash_of_all_activity?block_height={}", block_height);
        self.get(&endpoint).await
    }

    /// Get hash of all current balances
    pub async fn get_hash_of_all_current_balances(&self) -> Result<OpiResponse<BalancesHashResponse>> {
        let endpoint = "v1/brc20/get_hash_of_all_current_balances";
        self.get(endpoint).await
    }

    /// Get events by inscription ID
    pub async fn get_event(&self, inscription_id: &str) -> Result<OpiResponse<Vec<Brc20Event>>> {
        let endpoint = format!("v1/brc20/event?inscription_id={}", inscription_id);
        self.get(&endpoint).await
    }

    /// Get client IP (for debugging)
    pub async fn get_ip(&self) -> Result<String> {
        self.get_text("v1/brc20/ip").await
    }

    /// Make a raw GET request and return text
    pub async fn get_raw(&self, endpoint: &str) -> Result<String> {
        self.get_text(endpoint).await
    }
}
