use anyhow::{Context, Result};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::PgPool;
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
    db: sqlx::PgPool,
}

impl AlkanesService {
    pub fn new(rpc: AlkanesRpcClient, redis: redis::Client, db: sqlx::PgPool) -> Self {
        Self { rpc, redis, db }
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

    /// Get static alkane data (name, symbol, cap, mintAmount, image)
    /// Opcodes: 99=name, 100=symbol, 102=cap, 104=mintAmount, 1000=image
    pub async fn get_static_alkane_data(&self, id: &AlkaneId) -> Result<AlkaneToken> {
        // Try cache first
        let cache_key = format!("ALKANE-{}-{}", id.block, id.tx);
        
        if let Ok(mut conn) = self.redis.get_async_connection().await {
            if let Ok(Some(cached)) = redis::AsyncCommands::get::<_, Option<String>>(&mut conn, &cache_key).await {
                if let Ok(alkane) = serde_json::from_str::<AlkaneToken>(&cached) {
                    return Ok(alkane);
                }
            }
        }

        // Simulate calls to get static data using OYL API opcodes
        // staticOpcodes = ["99", "100", "102", "104", "1000"] 
        // opcodesHRV = ["name", "symbol", "cap", "mintAmount", "image"]
        let static_opcodes = vec!["99", "100", "102", "104", "1000"];
        let opcode_names = vec!["name", "symbol", "cap", "mintAmount", "image"];

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

        let mut has_valid_result = false;

        for (i, opcode) in static_opcodes.iter().enumerate() {
            let request = SimulateRequest {
                target: id.clone(),
                inputs: vec![json!(opcode)],
            };

            if let Ok(result) = self.rpc.simulate(&request).await {
                if result.status == 0 {
                    if let Some(parsed) = result.parsed {
                        has_valid_result = true;
                        match opcode_names[i] {
                            "name" => {
                                alkane_data.name = parsed.get("string")
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.to_string());
                            }
                            "symbol" => {
                                alkane_data.symbol = parsed.get("string")
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.to_string())
                                    .or(Some(String::new()));
                            }
                            "cap" => {
                                alkane_data.cap = parsed.get("le")
                                    .and_then(|v| v.as_u64())
                                    .map(|v| v.to_string());
                            }
                            "mintAmount" => {
                                alkane_data.max = parsed.get("le")
                                    .and_then(|v| v.as_u64())
                                    .map(|v| v.to_string());
                            }
                            "image" => {
                                alkane_data.image = parsed.get("string")
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.to_string())
                                    .or(Some(String::new()));
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        // Cache the result if we got valid data
        if has_valid_result {
            if let Ok(mut conn) = self.redis.get_async_connection().await {
                let json = serde_json::to_string(&alkane_data).unwrap_or_default();
                let _: Result<(), _> = redis::AsyncCommands::set(&mut conn, &cache_key, json).await;
            }
        }

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
        let limit = limit.unwrap_or(100).min(500);
        let offset = offset.unwrap_or(0);
        
        // Query TraceAlkane table for all registered alkanes
        let alkanes = sqlx::query_as::<_, (i32, i64, Option<i32>)>(
            r#"
            SELECT alkane_block, alkane_tx, created_at_height
            FROM "TraceAlkane"
            ORDER BY created_at_height DESC
            LIMIT $1 OFFSET $2
            "#
        )
        .bind(limit as i64)
        .bind(offset as i64)
        .fetch_all(&self.db)
        .await
        .context("Failed to query TraceAlkane")?;
        
        let total: (i64,) = sqlx::query_as(r#"SELECT COUNT(*) as count FROM "TraceAlkane""#)
            .fetch_one(&self.db)
            .await
            .context("Failed to count alkanes")?;
        let total = total.0 as usize;
        
        // Convert to AlkaneToken and enrich with metadata
        let mut tokens = Vec::new();
        for (alkane_block, alkane_tx, _created_at_height) in alkanes {
            let alkane_id = AlkaneId {
                block: alkane_block.to_string(),
                tx: alkane_tx.to_string(),
            };
            
            // Try to get metadata from reflect-alkane
            let mut token = AlkaneToken {
                id: alkane_id.clone(),
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
            
            // Try to enrich with metadata (non-blocking)
            if let Ok(metadata) = self.get_static_alkane_data(&alkane_id).await {
                token.name = metadata.name;
                token.symbol = metadata.symbol;
                token.decimals = metadata.decimals;
                token.max = metadata.max;
                token.cap = metadata.cap;
                token.premine = metadata.premine;
            }
            
            tokens.push(token);
        }
        
        Ok((tokens, total))
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
