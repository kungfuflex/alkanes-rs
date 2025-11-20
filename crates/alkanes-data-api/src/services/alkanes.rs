use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;

use super::alkanes_rpc::{AlkaneId, AlkanesRpcClient, SimulateRequest};
use super::redis as redis_mod;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlkaneToken {
    pub id: AlkaneId,
    pub name: Option<String>,
    pub symbol: Option<String>,
    pub decimals: Option<u32>,
    pub image: Option<String>,
    pub max: Option<String>,
    pub cap: Option<String>,
    pub premine: Option<String>,
    pub balance: Option<String>,
    #[serde(rename = "floorPrice")]
    pub floor_price: Option<f64>,
    #[serde(rename = "priceUsd")]
    pub price_usd: Option<f64>,
    #[serde(rename = "priceInSatoshi")]
    pub price_in_satoshi: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormattedUtxo {
    #[serde(rename = "txId")]
    pub tx_id: String,
    #[serde(rename = "scriptPk")]
    pub script_pk: String,
    #[serde(rename = "outputIndex")]
    pub output_index: u32,
    pub satoshis: u64,
    pub address: String,
    pub indexed: bool,
    pub inscriptions: Vec<Value>,
    pub runes: HashMap<String, Value>,
    pub confirmations: u32,
    pub alkanes: HashMap<String, AlkaneBalance>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlkaneBalance {
    pub balance: String,
    pub name: String,
    pub symbol: String,
}

pub struct AlkanesService {
    rpc: AlkanesRpcClient,
    redis: redis::Client,
}

impl AlkanesService {
    pub fn new(rpc: AlkanesRpcClient, redis: redis::Client) -> Self {
        Self { rpc, redis }
    }

    /// Get alkanes UTXOs for an address
    pub async fn get_alkanes_utxos(&self, address: &str) -> Result<Vec<FormattedUtxo>> {
        let rpc_response = self
            .rpc
            .get_alkanes_by_address(address)
            .await
            .context("Failed to get alkanes by address")?;

        let mut alkane_utxos = Vec::new();

        for outpoint in rpc_response {
            let mut alkanes_map = HashMap::new();

            for rune_item in outpoint.runes {
                let alkane_id_str = format!("{}:{}", rune_item.rune.id.block, rune_item.rune.id.tx);
                alkanes_map.insert(
                    alkane_id_str,
                    AlkaneBalance {
                        balance: rune_item.balance,
                        name: rune_item.rune.name,
                        symbol: rune_item.rune.symbol,
                    },
                );
            }

            alkane_utxos.push(FormattedUtxo {
                tx_id: outpoint.outpoint.txid,
                script_pk: outpoint.output.script,
                output_index: outpoint.outpoint.vout,
                satoshis: outpoint.output.value.parse().unwrap_or(0),
                address: address.to_string(),
                indexed: true,
                inscriptions: vec![],
                runes: HashMap::new(),
                confirmations: 3,
                alkanes: alkanes_map,
            });
        }

        Ok(alkane_utxos)
    }

    /// Get alkanes by address with balances
    pub async fn get_alkanes_by_address(
        &self,
        address: &str,
        _filter_lp_tokens: bool,
    ) -> Result<Vec<AlkaneToken>> {
        let rpc_response = self
            .rpc
            .get_alkanes_by_address(address)
            .await
            .context("Failed to get alkanes by address")?;

        let mut alkane_map: HashMap<String, AlkaneToken> = HashMap::new();

        // Aggregate balances
        for item in rpc_response {
            for rune_item in item.runes {
                let alkane_id_str = format!("{}:{}", rune_item.rune.id.block, rune_item.rune.id.tx);

                if let Some(existing) = alkane_map.get_mut(&alkane_id_str) {
                    // Add to existing balance
                    let current_balance: u128 = existing.balance.as_ref().unwrap_or(&"0".to_string()).parse().unwrap_or(0);
                    let new_balance: u128 = rune_item.balance.parse().unwrap_or(0);
                    existing.balance = Some((current_balance + new_balance).to_string());
                } else if !rune_item.rune.name.is_empty() {
                    // Create new entry
                    alkane_map.insert(
                        alkane_id_str,
                        AlkaneToken {
                            id: rune_item.rune.id,
                            name: Some(rune_item.rune.name),
                            symbol: Some(rune_item.rune.symbol),
                            balance: Some(rune_item.balance),
                            decimals: None,
                            image: None,
                            max: None,
                            cap: None,
                            premine: None,
                            floor_price: Some(0.0),
                            price_usd: Some(0.0),
                            price_in_satoshi: Some(0),
                        },
                    );
                }
            }
        }

        // TODO: Fetch static data (name, symbol, decimals, image) for each alkane
        // TODO: Fetch prices from pools
        // TODO: Filter LP tokens if requested

        Ok(alkane_map.into_values().collect())
    }

    /// Get static alkane data (name, symbol, decimals, image)
    pub async fn get_static_alkane_data(&self, id: &AlkaneId) -> Result<AlkaneToken> {
        // Try cache first
        let cache_key = format!("ALKANE-{}-{}", id.block, id.tx);
        
        // TODO: Check Redis cache

        // Simulate calls to get static data
        let static_opcodes = vec![
            json!([0, 0]), // name
            json!([0, 1]), // symbol
            json!([0, 2]), // decimals
            json!([0, 4]), // max
            json!([0, 5]), // cap
            json!([0, 6]), // premine
            json!([1, 2]), // image
        ];

        let mut alkane_data = AlkaneToken {
            id: id.clone(),
            name: None,
            symbol: None,
            decimals: None,
            image: None,
            max: None,
            cap: None,
            premine: None,
            balance: None,
            floor_price: None,
            price_usd: None,
            price_in_satoshi: None,
        };

        for (i, opcode) in static_opcodes.iter().enumerate() {
            let request = SimulateRequest {
                target: id.clone(),
                inputs: vec![opcode.clone()],
            };

            if let Ok(result) = self.rpc.simulate(&request).await {
                if result.status == 0 {
                    if let Some(parsed) = result.parsed {
                        match i {
                            0 => alkane_data.name = parsed.get("string").and_then(|v| v.as_str()).map(|s| s.to_string()),
                            1 => alkane_data.symbol = parsed.get("string").and_then(|v| v.as_str()).map(|s| s.to_string()),
                            2 => alkane_data.decimals = parsed.get("le").and_then(|v| v.as_u64()).map(|v| v as u32),
                            3 => alkane_data.max = parsed.get("le").and_then(|v| v.as_u64()).map(|v| v.to_string()),
                            4 => alkane_data.cap = parsed.get("le").and_then(|v| v.as_u64()).map(|v| v.to_string()),
                            5 => alkane_data.premine = parsed.get("le").and_then(|v| v.as_u64()).map(|v| v.to_string()),
                            6 => alkane_data.image = parsed.get("string").and_then(|v| v.as_str()).map(|s| s.to_string()),
                            _ => {}
                        }
                    }
                }
            }
        }

        // TODO: Store in Redis cache

        Ok(alkane_data)
    }

    /// Get all alkanes with pagination
    pub async fn get_alkanes(
        &self,
        limit: Option<i32>,
        offset: Option<i32>,
        _sort_by: Option<String>,
        _order: Option<String>,
        _search_query: Option<String>,
    ) -> Result<(Vec<AlkaneToken>, usize)> {
        // TODO: Implement full alkanes listing with caching
        // For now, return empty list
        
        Ok((vec![], 0))
    }

    /// Search alkanes globally
    pub async fn global_search(&self, query: &str) -> Result<Vec<AlkaneToken>> {
        if query.is_empty() {
            return Ok(vec![]);
        }

        // TODO: Implement search across alkanes by name, symbol, or ID
        // This should search the cache or database

        Ok(vec![])
    }

    /// Get alkane details by ID
    pub async fn get_alkane_details(&self, id: &AlkaneId) -> Result<AlkaneToken> {
        self.get_static_alkane_data(id).await
    }
}
