use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashMap;
use tracing::{info, warn, debug};

use super::alkanes_rpc::{AlkaneId, AlkanesRpcClient, SimulateRequest};
use super::redis as redis_mod;
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Pool {
    pub id: String,
    pub factory_block_id: String,
    pub factory_tx_id: String,
    pub pool_block_id: String,
    pub pool_tx_id: String,
    pub token0_block_id: String,
    pub token0_tx_id: String,
    pub token1_block_id: String,
    pub token1_tx_id: String,
    pub pool_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token0_amount: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token1_amount: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_supply: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creator_address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creation_block_height: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolWithMetrics {
    #[serde(flatten)]
    pub pool: Pool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tvl: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volume_24h: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fee_24h: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidityPosition {
    pub pool_id: String,
    pub address: String,
    pub liquidity_amount: String,
    pub token0_amount: String,
    pub token1_amount: String,
}

pub struct PoolService {
    db: PgPool,
    redis: redis::Client,
    network_env: String,
    rpc_client: Option<AlkanesRpcClient>,
}

// Opcode constants for AMM operations
const FACTORY_OPCODE_GET_ALL_POOLS: u64 = 3;
const POOL_OPCODE_GET_DETAILS: u64 = 999;

impl PoolService {
    pub fn new(db: PgPool, redis: redis::Client, network_env: String) -> Self {
        Self {
            db,
            redis,
            network_env,
            rpc_client: None,
        }
    }

    pub fn with_rpc(mut self, rpc_client: AlkanesRpcClient) -> Self {
        self.rpc_client = Some(rpc_client);
        self
    }

    /// Get latest processed block height
    pub async fn get_latest_block_height(&self) -> Result<i32> {
        let cache_key = format!("indexer-{}-pools-lastblock", self.network_env);
        
        // Try cache first
        let mut conn = self.redis.get_async_connection().await?;
        if let Ok(Some(cached)) = redis::AsyncCommands::get::<_, Option<String>>(&mut conn, &cache_key).await {
            if let Ok(height) = cached.parse::<i32>() {
                return Ok(height);
            }
        }

        // Query from database
        let result: Option<(i32,)> = sqlx::query_as(
            r#"
            SELECT "blockHeight"
            FROM "ProcessedBlocks"
            ORDER BY "blockHeight" DESC
            LIMIT 1
            "#
        )
        .fetch_optional(&self.db)
        .await?;

        Ok(result.map(|r| r.0).unwrap_or(0))
    }

    /// Get all pools for a factory using RPC simulation (alkanes-cli-sys approach)
    /// This method fetches pools directly from the blockchain state via the factory
    /// contract's GET_ALL_POOLS opcode (3), then fetches details for each pool in
    /// parallel batches of 30 (matching alkanes-cli --experimental-asm-parallel).
    async fn get_pools_by_factory_rpc(&self, factory_id: &AlkaneId) -> Result<Vec<Pool>> {
        let rpc_client = self.rpc_client.as_ref()
            .ok_or_else(|| anyhow::anyhow!("RPC client not configured"))?;

        // Step 1: Call factory's GET_ALL_POOLS opcode via simulation
        let request = SimulateRequest {
            target: AlkaneId {
                block: factory_id.block.clone(),
                tx: factory_id.tx.clone(),
            },
            inputs: vec![json!(FACTORY_OPCODE_GET_ALL_POOLS.to_string())],
        };

        let result = rpc_client.simulate(&request).await
            .context("Failed to call factory GET_ALL_POOLS")?;

        // Parse pool IDs from the response
        let pool_ids = self.decode_pool_ids_from_response(&result.parsed)?;
        info!("Found {} pools from factory RPC", pool_ids.len());

        if pool_ids.is_empty() {
            return Ok(vec![]);
        }

        // Step 2: Fetch details for each pool in parallel batches of 30
        const BATCH_SIZE: usize = 30;
        let mut pools = Vec::new();

        for chunk in pool_ids.chunks(BATCH_SIZE) {
            let futures: Vec<_> = chunk
                .iter()
                .map(|pool_id| self.get_pool_details_rpc(rpc_client, factory_id, pool_id))
                .collect();

            let results = futures::future::join_all(futures).await;

            for (i, result) in results.into_iter().enumerate() {
                match result {
                    Ok(pool) => pools.push(pool),
                    Err(e) => {
                        let pool_id = &chunk[i];
                        warn!("Failed to get details for pool {}:{}: {}", pool_id.block, pool_id.tx, e);
                    }
                }
            }
        }

        Ok(pools)
    }

    /// Decode pool IDs from the factory GET_ALL_POOLS response
    fn decode_pool_ids_from_response(&self, parsed: &Option<Value>) -> Result<Vec<AlkaneId>> {
        let parsed = match parsed {
            Some(v) => v,
            None => return Ok(vec![]),
        };

        // The response format depends on how the factory encodes pools
        // This follows the alkanes-cli-common decode_get_all_pools pattern
        let mut pool_ids = Vec::new();

        if let Some(data) = parsed.get("data").and_then(|d| d.as_str()) {
            // Decode hex data: first 16 bytes is count, then 32 bytes per pool ID
            let clean = data.strip_prefix("0x").unwrap_or(data);
            if clean.len() >= 32 {
                // First 32 hex chars (16 bytes) = count, little-endian
                let count_bytes = hex::decode(&clean[0..32]).unwrap_or_default();
                let count = u128::from_le_bytes(count_bytes.try_into().unwrap_or([0u8; 16])) as usize;

                for i in 0..count {
                    let offset = 32 + i * 64; // 32 hex = 16 bytes per u128
                    if clean.len() < offset + 64 {
                        break;
                    }
                    // Each entry is two u128s: block and tx
                    let block_bytes = hex::decode(&clean[offset..offset+32]).unwrap_or_default();
                    let tx_bytes = hex::decode(&clean[offset+32..offset+64]).unwrap_or_default();

                    let block = u128::from_le_bytes(block_bytes.try_into().unwrap_or([0u8; 16]));
                    let tx = u128::from_le_bytes(tx_bytes.try_into().unwrap_or([0u8; 16]));

                    pool_ids.push(AlkaneId {
                        block: block.to_string(),
                        tx: tx.to_string(),
                    });
                }
            }
        } else if let Some(pools) = parsed.get("pools").and_then(|p| p.as_array()) {
            // Alternative format: { pools: [{ block: "...", tx: "..." }, ...] }
            for pool in pools {
                if let (Some(block), Some(tx)) = (
                    pool.get("block").and_then(|b| b.as_str()),
                    pool.get("tx").and_then(|t| t.as_str()),
                ) {
                    pool_ids.push(AlkaneId {
                        block: block.to_string(),
                        tx: tx.to_string(),
                    });
                }
            }
        }

        Ok(pool_ids)
    }

    /// Get pool details via RPC simulation (opcode 999)
    async fn get_pool_details_rpc(
        &self,
        rpc_client: &AlkanesRpcClient,
        factory_id: &AlkaneId,
        pool_id: &AlkaneId,
    ) -> Result<Pool> {
        let request = SimulateRequest {
            target: AlkaneId {
                block: pool_id.block.clone(),
                tx: pool_id.tx.clone(),
            },
            inputs: vec![json!(POOL_OPCODE_GET_DETAILS.to_string())],
        };

        let result = rpc_client.simulate(&request).await
            .context("Failed to call pool GET_DETAILS")?;

        // Parse pool details from response
        self.decode_pool_from_response(factory_id, pool_id, &result.parsed)
    }

    /// Decode a Pool struct from the GET_DETAILS response
    fn decode_pool_from_response(
        &self,
        factory_id: &AlkaneId,
        pool_id: &AlkaneId,
        parsed: &Option<Value>,
    ) -> Result<Pool> {
        let parsed = match parsed {
            Some(v) => v,
            None => return Err(anyhow::anyhow!("No response data")),
        };

        // Extract pool details from the parsed response
        // Format depends on the pool contract's GET_DETAILS return format
        let data_hex = parsed.get("data").and_then(|d| d.as_str()).unwrap_or("0x");
        let clean = data_hex.strip_prefix("0x").unwrap_or(data_hex);

        // Decode: token0_block, token0_tx, token1_block, token1_tx, reserve0, reserve1, total_supply, pool_name
        // Each u128 is 32 hex chars (16 bytes)
        if clean.len() < 32 * 7 {
            return Err(anyhow::anyhow!("Response too short"));
        }

        let decode_u128 = |start: usize| -> u128 {
            let bytes = hex::decode(&clean[start..start+32]).unwrap_or_default();
            u128::from_le_bytes(bytes.try_into().unwrap_or([0u8; 16]))
        };

        let token0_block = decode_u128(0);
        let token0_tx = decode_u128(32);
        let token1_block = decode_u128(64);
        let token1_tx = decode_u128(96);
        let token0_amount = decode_u128(128);
        let token1_amount = decode_u128(160);
        let token_supply = decode_u128(192);

        // Pool name is the remaining bytes as UTF-8
        let pool_name = if clean.len() > 224 {
            hex::decode(&clean[224..])
                .ok()
                .and_then(|bytes| String::from_utf8(bytes).ok())
                .unwrap_or_else(|| format!("Pool {}:{}", pool_id.block, pool_id.tx))
        } else {
            format!("Pool {}:{}", pool_id.block, pool_id.tx)
        };

        Ok(Pool {
            id: format!("{}:{}", pool_id.block, pool_id.tx),
            factory_block_id: factory_id.block.clone(),
            factory_tx_id: factory_id.tx.clone(),
            pool_block_id: pool_id.block.clone(),
            pool_tx_id: pool_id.tx.clone(),
            token0_block_id: token0_block.to_string(),
            token0_tx_id: token0_tx.to_string(),
            token1_block_id: token1_block.to_string(),
            token1_tx_id: token1_tx.to_string(),
            pool_name,
            token0_amount: Some(token0_amount.to_string()),
            token1_amount: Some(token1_amount.to_string()),
            token_supply: Some(token_supply.to_string()),
            creator_address: None,
            creation_block_height: None,
        })
    }

    /// Get all pools for a factory using alkanes-cli-sys RPC simulation
    /// Cache invalidates when metashrew block height changes
    pub async fn get_pools_by_factory(&self, factory_id: &AlkaneId) -> Result<Vec<Pool>> {
        let rpc_client = self.rpc_client.as_ref()
            .ok_or_else(|| anyhow::anyhow!("RPC client not configured"))?;

        // Get metashrew height via RPC for cache invalidation
        let block_height = rpc_client.get_block_height().await.unwrap_or(0);
        let cache_key = format!(
            "pools-rpc:{}:{}:height-{}",
            factory_id.block, factory_id.tx, block_height
        );

        // Check Redis cache first
        let mut conn = self.redis.get_async_connection().await?;
        if let Ok(Some(cached)) = redis::AsyncCommands::get::<_, Option<String>>(&mut conn, &cache_key).await {
            if let Ok(pools) = serde_json::from_str::<Vec<Pool>>(&cached) {
                debug!("Returning {} cached pools for height {}", pools.len(), block_height);
                return Ok(pools);
            }
        }

        // Fetch pools via RPC simulation (alkanes-cli-sys style)
        let pools = self.get_pools_by_factory_rpc(factory_id).await?;
        info!("Fetched {} pools via RPC at height {}", pools.len(), block_height);

        // Cache result (invalidates when block height changes)
        if !pools.is_empty() {
            let serialized = serde_json::to_string(&pools)?;
            let _: Result<(), _> = redis::AsyncCommands::set_ex(&mut conn, cache_key, serialized, 86400).await;
        }

        Ok(pools)
    }

    /// Get specific pool by ID
    pub async fn get_pool_by_id(&self, pool_id: &AlkaneId) -> Result<Option<Pool>> {
        let block_height = self.get_latest_block_height().await?;
        let cache_key = format!("pool:{}:{}:block-{}", pool_id.block, pool_id.tx, block_height);

        // Try cache
        let mut conn = self.redis.get_async_connection().await?;
        if let Ok(Some(cached)) = redis::AsyncCommands::get::<_, Option<String>>(&mut conn, &cache_key).await {
            if let Ok(pool) = serde_json::from_str::<Pool>(&cached) {
                return Ok(Some(pool));
            }
        }

        // Query database
        let pool = sqlx::query_as::<_, Pool>(
            r#"
            SELECT 
                p.id,
                p."factoryBlockId" as factory_block_id,
                p."factoryTxId" as factory_tx_id,
                p."poolBlockId" as pool_block_id,
                p."poolTxId" as pool_tx_id,
                p."token0BlockId" as token0_block_id,
                p."token0TxId" as token0_tx_id,
                p."token1BlockId" as token1_block_id,
                p."token1TxId" as token1_tx_id,
                p."poolName" as pool_name,
                ps."token0Amount" as token0_amount,
                ps."token1Amount" as token1_amount,
                ps."tokenSupply" as token_supply,
                pc."creatorAddress" as creator_address,
                pc."blockHeight" as creation_block_height
            FROM "Pool" p
            LEFT JOIN LATERAL (
                SELECT "token0Amount", "token1Amount", "tokenSupply"
                FROM "PoolState"
                WHERE "poolId" = p.id
                ORDER BY "blockHeight" DESC
                LIMIT 1
            ) ps ON true
            LEFT JOIN "PoolCreation" pc ON pc."poolBlockId" = p."poolBlockId" AND pc."poolTxId" = p."poolTxId"
            WHERE p."poolBlockId" = $1 AND p."poolTxId" = $2
            "#
        )
        .bind(&pool_id.block)
        .bind(&pool_id.tx)
        .fetch_optional(&self.db)
        .await?;

        // Cache result
        if let Some(ref p) = pool {
            let serialized = serde_json::to_string(p)?;
            let _: Result<(), _> = redis::AsyncCommands::set_ex(&mut conn, cache_key, serialized, 86400).await;
        }

        Ok(pool)
    }

    /// Get liquidity positions for an address
    pub async fn get_address_positions(&self, address: &str) -> Result<Vec<LiquidityPosition>> {
        // Query all mints for address
        #[derive(sqlx::FromRow)]
        struct LiquiditySum {
            pool_block_id: String,
            pool_tx_id: String,
            total_liquidity: Option<sqlx::types::BigDecimal>,
        }

        let mints = sqlx::query_as::<_, LiquiditySum>(
            r#"
            SELECT 
                "poolBlockId" as pool_block_id,
                "poolTxId" as pool_tx_id,
                SUM(CAST("lpTokenAmount" AS NUMERIC)) as total_liquidity
            FROM "PoolMint"
            WHERE "minterAddress" = $1 AND successful = true
            GROUP BY "poolBlockId", "poolTxId"
            "#
        )
        .bind(address)
        .fetch_all(&self.db)
        .await?;

        // Query all burns for address
        let burns = sqlx::query_as::<_, LiquiditySum>(
            r#"
            SELECT 
                "poolBlockId" as pool_block_id,
                "poolTxId" as pool_tx_id,
                SUM(CAST("lpTokenAmount" AS NUMERIC)) as total_liquidity
            FROM "PoolBurn"
            WHERE "burnerAddress" = $1 AND successful = true
            GROUP BY "poolBlockId", "poolTxId"
            "#
        )
        .bind(address)
        .fetch_all(&self.db)
        .await?;

        // Calculate net positions using pool_block_id:pool_tx_id as key
        let mut positions: HashMap<String, f64> = HashMap::new();
        
        for mint in mints {
            let pool_key = format!("{}:{}", mint.pool_block_id, mint.pool_tx_id);
            if let Some(liq) = mint.total_liquidity {
                *positions.entry(pool_key).or_insert(0.0) += liq.to_string().parse::<f64>().unwrap_or(0.0);
            }
        }

        for burn in burns {
            let pool_key = format!("{}:{}", burn.pool_block_id, burn.pool_tx_id);
            if let Some(liq) = burn.total_liquidity {
                *positions.entry(pool_key).or_insert(0.0) -= liq.to_string().parse::<f64>().unwrap_or(0.0);
            }
        }

        // Filter out zero positions and fetch pool details
        let mut result = Vec::new();
        for (pool_key, liquidity) in positions {
            if liquidity > 0.0 {
                // Get pool details to calculate token amounts
                result.push(LiquidityPosition {
                    pool_id: pool_key.clone(),
                    address: address.to_string(),
                    liquidity_amount: liquidity.to_string(),
                    token0_amount: "0".to_string(), // TODO: Calculate based on pool state
                    token1_amount: "0".to_string(), // TODO: Calculate based on pool state
                });
            }
        }

        Ok(result)
    }

    /// Get all token pairs for a factory
    pub async fn get_all_token_pairs(&self, factory_id: &AlkaneId) -> Result<Vec<(AlkaneId, AlkaneId)>> {
        let pools = self.get_pools_by_factory(factory_id).await?;
        
        let pairs = pools
            .into_iter()
            .map(|p| {
                (
                    AlkaneId {
                        block: p.token0_block_id,
                        tx: p.token0_tx_id,
                    },
                    AlkaneId {
                        block: p.token1_block_id,
                        tx: p.token1_tx_id,
                    },
                )
            })
            .collect();

        Ok(pairs)
    }

    /// Get pairs containing a specific token
    pub async fn get_token_pairs(
        &self,
        factory_id: &AlkaneId,
        token_id: &AlkaneId,
    ) -> Result<Vec<Pool>> {
        let pools = sqlx::query_as::<_, Pool>(
            r#"
            SELECT 
                p.id,
                p."factoryBlockId" as factory_block_id,
                p."factoryTxId" as factory_tx_id,
                p."poolBlockId" as pool_block_id,
                p."poolTxId" as pool_tx_id,
                p."token0BlockId" as token0_block_id,
                p."token0TxId" as token0_tx_id,
                p."token1BlockId" as token1_block_id,
                p."token1TxId" as token1_tx_id,
                p."poolName" as pool_name,
                ps."token0Amount" as token0_amount,
                ps."token1Amount" as token1_amount,
                ps."tokenSupply" as token_supply,
                pc."creatorAddress" as creator_address,
                pc."blockHeight" as creation_block_height
            FROM "Pool" p
            LEFT JOIN LATERAL (
                SELECT "token0Amount", "token1Amount", "tokenSupply"
                FROM "PoolState"
                WHERE "poolId" = p.id
                ORDER BY "blockHeight" DESC
                LIMIT 1
            ) ps ON true
            LEFT JOIN "PoolCreation" pc ON pc."poolBlockId" = p."poolBlockId" AND pc."poolTxId" = p."poolTxId"
            WHERE p."factoryBlockId" = $1 
              AND p."factoryTxId" = $2
              AND (
                  (p."token0BlockId" = $3 AND p."token0TxId" = $4)
                  OR (p."token1BlockId" = $3 AND p."token1TxId" = $4)
              )
            "#
        )
        .bind(&factory_id.block)
        .bind(&factory_id.tx)
        .bind(&token_id.block)
        .bind(&token_id.tx)
        .fetch_all(&self.db)
        .await?;

        Ok(pools)
    }
}
