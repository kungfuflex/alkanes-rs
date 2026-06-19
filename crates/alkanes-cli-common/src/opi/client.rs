/// OPI (Open Protocol Indexer) HTTP client
/// Based on https://github.com/bestinslot-xyz/OPI/blob/main/modules/brc20_api/api.js

use anyhow::{Context, Result};
use super::types::*;

#[cfg(feature = "std")]
pub struct OpiClient {
    base_url: String,
    client: reqwest::Client,
    headers: Vec<(String, String)>,
}

#[cfg(feature = "std")]
impl OpiClient {
    pub fn new(base_url: String) -> Self {
        Self::with_headers(base_url, vec![])
    }

    pub fn with_headers(base_url: String, headers: Vec<String>) -> Self {
        let mut builder = reqwest::Client::builder();

        #[cfg(not(target_arch = "wasm32"))]
        {
            builder = builder.timeout(std::time::Duration::from_secs(30));
        }

        let client = builder.build().unwrap();

        // Parse headers into (name, value) tuples
        let parsed_headers: Vec<(String, String)> = headers
            .into_iter()
            .filter_map(|h| {
                let parts: Vec<&str> = h.splitn(2, ':').collect();
                if parts.len() == 2 {
                    Some((parts[0].trim().to_string(), parts[1].trim().to_string()))
                } else {
                    None
                }
            })
            .collect();

        Self { base_url, client, headers: parsed_headers }
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
        let mut request = self.client.get(&url);

        // Add custom headers
        for (name, value) in &self.headers {
            request = request.header(name.as_str(), value.as_str());
        }

        let response = request
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
        let mut request = self.client.get(&url);

        // Add custom headers
        for (name, value) in &self.headers {
            request = request.header(name.as_str(), value.as_str());
        }

        let response = request
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
        if text == "null" || text.is_empty() || text == "-1" {
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

    // ==================== RUNES API ====================

    /// Get Runes block height
    pub async fn get_runes_block_height(&self) -> Result<Option<u64>> {
        let text = self.get_text("v1/runes/block_height").await?;
        if text == "null" || text.is_empty() || text == "-1" {
            return Ok(None);
        }
        let height: u64 = text.parse().context("Failed to parse runes block height")?;
        Ok(Some(height))
    }

    /// Get Runes balance at a specific block height
    pub async fn get_runes_balance_on_block(
        &self,
        block_height: u64,
        pkscript: &str,
        rune_id: &str,
    ) -> Result<OpiResponse<serde_json::Value>> {
        let endpoint = format!(
            "v1/runes/balance_on_block?block_height={}&pkscript={}&rune_id={}",
            block_height, pkscript, rune_id
        );
        self.get(&endpoint).await
    }

    /// Get all Runes activity for a block
    pub async fn get_runes_activity_on_block(&self, block_height: u64) -> Result<OpiResponse<serde_json::Value>> {
        let endpoint = format!("v1/runes/activity_on_block?block_height={}", block_height);
        self.get(&endpoint).await
    }

    /// Get current Runes balance of a wallet
    pub async fn get_runes_current_balance_of_wallet(
        &self,
        address: Option<&str>,
        pkscript: Option<&str>,
    ) -> Result<OpiResponse<serde_json::Value>> {
        let mut endpoint = "v1/runes/get_current_balance_of_wallet?".to_string();
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

    /// Get unspent rune outpoints of a wallet
    pub async fn get_runes_unspent_outpoints_of_wallet(
        &self,
        address: Option<&str>,
        pkscript: Option<&str>,
    ) -> Result<OpiResponse<serde_json::Value>> {
        let mut endpoint = "v1/runes/get_unspent_rune_outpoints_of_wallet?".to_string();
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

    /// Get Runes holders
    pub async fn get_runes_holders(&self, rune_id: &str) -> Result<OpiResponse<serde_json::Value>> {
        let endpoint = format!("v1/runes/holders?rune_id={}", rune_id);
        self.get(&endpoint).await
    }

    /// Get Runes hash of all activity
    pub async fn get_runes_hash_of_all_activity(&self, block_height: u64) -> Result<OpiResponse<serde_json::Value>> {
        let endpoint = format!("v1/runes/get_hash_of_all_activity?block_height={}", block_height);
        self.get(&endpoint).await
    }

    /// Get Runes event by txid
    pub async fn get_runes_event(&self, txid: &str) -> Result<OpiResponse<serde_json::Value>> {
        let endpoint = format!("v1/runes/event?txid={}", txid);
        self.get(&endpoint).await
    }

    // ==================== BITMAP API ====================

    /// Get Bitmap block height
    pub async fn get_bitmap_block_height(&self) -> Result<Option<u64>> {
        let text = self.get_text("v1/bitmap/block_height").await?;
        if text == "null" || text.is_empty() || text == "-1" {
            return Ok(None);
        }
        let height: u64 = text.parse().context("Failed to parse bitmap block height")?;
        Ok(Some(height))
    }

    /// Get Bitmap hash of all activity
    pub async fn get_bitmap_hash_of_all_activity(&self, block_height: u64) -> Result<OpiResponse<serde_json::Value>> {
        let endpoint = format!("v1/bitmap/get_hash_of_all_activity?block_height={}", block_height);
        self.get(&endpoint).await
    }

    /// Get Bitmap hash of all bitmaps
    pub async fn get_bitmap_hash_of_all_bitmaps(&self) -> Result<OpiResponse<serde_json::Value>> {
        self.get("v1/bitmap/get_hash_of_all_bitmaps").await
    }

    /// Get inscription ID of bitmap
    pub async fn get_bitmap_inscription_id(&self, bitmap: &str) -> Result<OpiResponse<serde_json::Value>> {
        let endpoint = format!("v1/bitmap/get_inscription_id_of_bitmap?bitmap={}", bitmap);
        self.get(&endpoint).await
    }

    // ==================== POW20 API ====================

    /// Get POW20 block height
    pub async fn get_pow20_block_height(&self) -> Result<Option<u64>> {
        let text = self.get_text("v1/pow20/block_height").await?;
        if text == "null" || text.is_empty() || text == "-1" {
            return Ok(None);
        }
        let height: u64 = text.parse().context("Failed to parse pow20 block height")?;
        Ok(Some(height))
    }

    /// Get POW20 balance at a specific block height
    pub async fn get_pow20_balance_on_block(
        &self,
        block_height: u64,
        pkscript: &str,
        ticker: &str,
    ) -> Result<OpiResponse<serde_json::Value>> {
        let endpoint = format!(
            "v1/pow20/balance_on_block?block_height={}&pkscript={}&ticker={}",
            block_height, pkscript, ticker
        );
        self.get(&endpoint).await
    }

    /// Get all POW20 activity for a block
    pub async fn get_pow20_activity_on_block(&self, block_height: u64) -> Result<OpiResponse<serde_json::Value>> {
        let endpoint = format!("v1/pow20/activity_on_block?block_height={}", block_height);
        self.get(&endpoint).await
    }

    /// Get current POW20 balance of a wallet
    pub async fn get_pow20_current_balance_of_wallet(
        &self,
        ticker: &str,
        address: Option<&str>,
        pkscript: Option<&str>,
    ) -> Result<OpiResponse<serde_json::Value>> {
        let mut endpoint = format!("v1/pow20/get_current_balance_of_wallet?ticker={}", ticker);
        if let Some(addr) = address {
            endpoint.push_str(&format!("&address={}", addr));
        }
        if let Some(pk) = pkscript {
            endpoint.push_str(&format!("&pkscript={}", pk));
        }
        self.get(&endpoint).await
    }

    /// Get POW20 valid TX notes of wallet
    pub async fn get_pow20_valid_tx_notes_of_wallet(
        &self,
        address: Option<&str>,
        pkscript: Option<&str>,
    ) -> Result<OpiResponse<serde_json::Value>> {
        let mut endpoint = "v1/pow20/get_valid_tx_notes_of_wallet?".to_string();
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

    /// Get POW20 valid TX notes of ticker
    pub async fn get_pow20_valid_tx_notes_of_ticker(&self, ticker: &str) -> Result<OpiResponse<serde_json::Value>> {
        let endpoint = format!("v1/pow20/get_valid_tx_notes_of_ticker?ticker={}", ticker);
        self.get(&endpoint).await
    }

    /// Get POW20 holders
    pub async fn get_pow20_holders(&self, ticker: &str) -> Result<OpiResponse<serde_json::Value>> {
        let endpoint = format!("v1/pow20/holders?ticker={}", ticker);
        self.get(&endpoint).await
    }

    /// Get POW20 hash of all activity
    pub async fn get_pow20_hash_of_all_activity(&self, block_height: u64) -> Result<OpiResponse<serde_json::Value>> {
        let endpoint = format!("v1/pow20/get_hash_of_all_activity?block_height={}", block_height);
        self.get(&endpoint).await
    }

    /// Get POW20 hash of all current balances
    pub async fn get_pow20_hash_of_all_current_balances(&self) -> Result<OpiResponse<serde_json::Value>> {
        self.get("v1/pow20/get_hash_of_all_current_balances").await
    }

    // ==================== SNS API ====================

    /// Get SNS block height
    pub async fn get_sns_block_height(&self) -> Result<Option<u64>> {
        let text = self.get_text("v1/sns/block_height").await?;
        if text == "null" || text.is_empty() || text == "-1" {
            return Ok(None);
        }
        let height: u64 = text.parse().context("Failed to parse sns block height")?;
        Ok(Some(height))
    }

    /// Get SNS hash of all activity
    pub async fn get_sns_hash_of_all_activity(&self, block_height: u64) -> Result<OpiResponse<serde_json::Value>> {
        let endpoint = format!("v1/sns/get_hash_of_all_activity?block_height={}", block_height);
        self.get(&endpoint).await
    }

    /// Get SNS hash of all registered names
    pub async fn get_sns_hash_of_all_registered_names(&self) -> Result<OpiResponse<serde_json::Value>> {
        self.get("v1/sns/get_hash_of_all_registered_names").await
    }

    /// Get SNS info
    pub async fn get_sns_info(&self, name: &str) -> Result<OpiResponse<serde_json::Value>> {
        let endpoint = format!("v1/sns/get_info_of_sns?name={}", name);
        self.get(&endpoint).await
    }

    /// Get SNS inscriptions of domain
    pub async fn get_sns_inscriptions_of_domain(&self, domain: &str) -> Result<OpiResponse<serde_json::Value>> {
        let endpoint = format!("v1/sns/get_inscriptions_of_domain?domain={}", domain);
        self.get(&endpoint).await
    }

    /// Get SNS registered namespaces
    pub async fn get_sns_registered_namespaces(&self) -> Result<OpiResponse<serde_json::Value>> {
        self.get("v1/sns/get_registered_namespaces").await
    }
}
