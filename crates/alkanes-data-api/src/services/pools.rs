use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashMap;

use super::alkanes_rpc::AlkaneId;
use super::redis as redis_mod;

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
}

impl PoolService {
    pub fn new(db: PgPool, redis: redis::Client, network_env: String) -> Self {
        Self {
            db,
            redis,
            network_env,
        }
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

    /// Get all pools for a factory
    pub async fn get_pools_by_factory(&self, factory_id: &AlkaneId) -> Result<Vec<Pool>> {
        let block_height = self.get_latest_block_height().await?;
        let cache_key = format!(
            "pools:{}:{}:block-{}",
            factory_id.block, factory_id.tx, block_height
        );

        // Try cache
        let mut conn = self.redis.get_async_connection().await?;
        if let Ok(Some(cached)) = redis::AsyncCommands::get::<_, Option<String>>(&mut conn, &cache_key).await {
            if let Ok(pools) = serde_json::from_str::<Vec<Pool>>(&cached) {
                return Ok(pools);
            }
        }

        // Query database using raw query to avoid compile-time validation
        let pools = sqlx::query_as::<_, Pool>(
            r#"
            SELECT 
                p.id,
                p.factory_block_id,
                p.factory_tx_id,
                p.pool_block_id,
                p.pool_tx_id,
                p.token0_block_id,
                p.token0_tx_id,
                p.token1_block_id,
                p.token1_tx_id,
                p.pool_name,
                ps.token0_amount,
                ps.token1_amount,
                ps.token_supply,
                pc.creator_address,
                pc.block_height as creation_block_height
            FROM "Pool" p
            LEFT JOIN LATERAL (
                SELECT "token0Amount", "token1Amount", "tokenSupply"
                FROM "PoolState"
                WHERE "poolId" = p.id
                ORDER BY "blockHeight" DESC
                LIMIT 1
            ) ps ON true
            LEFT JOIN "PoolCreation" pc ON pc."poolBlockId" = p."poolBlockId" AND pc."poolTxId" = p."poolTxId"
            WHERE p."factoryBlockId" = $1 AND p."factoryTxId" = $2
            "#
        )
        .bind(&factory_id.block)
        .bind(&factory_id.tx)
        .fetch_all(&self.db)
        .await?;

        // Cache result
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
                p.factory_block_id,
                p.factory_tx_id,
                p.pool_block_id,
                p.pool_tx_id,
                p.token0_block_id,
                p.token0_tx_id,
                p.token1_block_id,
                p.token1_tx_id,
                p.pool_name,
                ps.token0_amount,
                ps.token1_amount,
                ps.token_supply,
                pc.creator_address,
                pc.block_height as creation_block_height
            FROM "Pool" p
            LEFT JOIN LATERAL (
                SELECT "token0Amount", "token1Amount", "tokenSupply"
                FROM "PoolState"
                WHERE "poolId" = p.id
                ORDER BY "blockHeight" DESC
                LIMIT 1
            ) ps ON true
            LEFT JOIN pool_creation pc ON pc.pool_id = p.id
            WHERE p.pool_block_id = $1 AND p.pool_tx_id = $2
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
            pool_id: String,
            from_address: String,
            total_liquidity: Option<sqlx::types::BigDecimal>,
        }

        let mints = sqlx::query_as::<_, LiquiditySum>(
            r#"
            SELECT 
                pool_id,
                from_address,
                SUM(CAST(liquidity_amount AS NUMERIC)) as total_liquidity
            FROM mint
            WHERE from_address = $1 AND successful = true
            GROUP BY pool_id, from_address
            "#
        )
        .bind(address)
        .fetch_all(&self.db)
        .await?;

        // Query all burns for address
        let burns = sqlx::query_as::<_, LiquiditySum>(
            r#"
            SELECT 
                pool_id,
                from_address,
                SUM(CAST(liquidity_amount AS NUMERIC)) as total_liquidity
            FROM burn
            WHERE from_address = $1 AND successful = true
            GROUP BY pool_id, from_address
            "#
        )
        .bind(address)
        .fetch_all(&self.db)
        .await?;

        // Calculate net positions
        let mut positions: HashMap<String, f64> = HashMap::new();
        
        for mint in mints {
            let pool_id = mint.pool_id;
            if let Some(liq) = mint.total_liquidity {
                *positions.entry(pool_id).or_insert(0.0) += liq.to_string().parse::<f64>().unwrap_or(0.0);
            }
        }

        for burn in burns {
            let pool_id = burn.pool_id;
            if let Some(liq) = burn.total_liquidity {
                *positions.entry(pool_id).or_insert(0.0) -= liq.to_string().parse::<f64>().unwrap_or(0.0);
            }
        }

        // Filter out zero positions and fetch pool details
        let mut result = Vec::new();
        for (pool_id, liquidity) in positions {
            if liquidity > 0.0 {
                // Get pool details to calculate token amounts
                result.push(LiquidityPosition {
                    pool_id: pool_id.clone(),
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
                p.factory_block_id,
                p.factory_tx_id,
                p.pool_block_id,
                p.pool_tx_id,
                p.token0_block_id,
                p.token0_tx_id,
                p.token1_block_id,
                p.token1_tx_id,
                p.pool_name,
                ps.token0_amount,
                ps.token1_amount,
                ps.token_supply,
                pc.creator_address,
                pc.block_height as creation_block_height
            FROM "Pool" p
            LEFT JOIN LATERAL (
                SELECT "token0Amount", "token1Amount", "tokenSupply"
                FROM "PoolState"
                WHERE "poolId" = p.id
                ORDER BY "blockHeight" DESC
                LIMIT 1
            ) ps ON true
            LEFT JOIN pool_creation pc ON pc.pool_id = p.id
            WHERE p.factory_block_id = $1 
              AND p.factory_tx_id = $2
              AND (
                  (p.token0_block_id = $3 AND p.token0_tx_id = $4)
                  OR (p.token1_block_id = $3 AND p.token1_tx_id = $4)
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
