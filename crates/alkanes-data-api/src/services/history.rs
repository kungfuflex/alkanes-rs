use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use super::alkanes_rpc::AlkaneId;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[sqlx(rename_all = "camelCase")]
pub struct Swap {
    pub id: String,
    pub transaction_id: String,
    pub block_height: i32,
    pub transaction_index: i32,
    pub pool_block_id: String,
    pub pool_tx_id: String,
    pub sold_token_block_id: String,
    pub sold_token_tx_id: String,
    pub bought_token_block_id: String,
    pub bought_token_tx_id: String,
    pub sold_amount: f64,
    pub bought_amount: f64,
    pub seller_address: Option<String>,
    pub successful: bool,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[sqlx(rename_all = "camelCase")]
pub struct Mint {
    pub id: String,
    pub transaction_id: String,
    pub block_height: i32,
    pub transaction_index: i32,
    pub pool_block_id: String,
    pub pool_tx_id: String,
    pub lp_token_amount: String,
    pub token0_block_id: String,
    pub token0_tx_id: String,
    pub token1_block_id: String,
    pub token1_tx_id: String,
    pub token0_amount: String,
    pub token1_amount: String,
    pub minter_address: Option<String>,
    pub successful: bool,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[sqlx(rename_all = "camelCase")]
pub struct Burn {
    pub id: String,
    pub transaction_id: String,
    pub block_height: i32,
    pub transaction_index: i32,
    pub pool_block_id: String,
    pub pool_tx_id: String,
    pub lp_token_amount: String,
    pub token0_block_id: String,
    pub token0_tx_id: String,
    pub token1_block_id: String,
    pub token1_tx_id: String,
    pub token0_amount: String,
    pub token1_amount: String,
    pub burner_address: Option<String>,
    pub successful: bool,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[sqlx(rename_all = "camelCase")]
pub struct PoolCreation {
    pub id: String,
    pub transaction_id: String,
    pub block_height: i32,
    pub transaction_index: i32,
    pub pool_block_id: String,
    pub pool_tx_id: String,
    pub token0_block_id: String,
    pub token0_tx_id: String,
    pub token1_block_id: String,
    pub token1_tx_id: String,
    pub token0_amount: String,
    pub token1_amount: String,
    pub token_supply: String,
    pub creator_address: Option<String>,
    pub successful: bool,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[sqlx(rename_all = "camelCase")]
pub struct Wrap {
    pub id: String,
    pub transaction_id: String,
    pub block_height: i32,
    pub transaction_index: i32,
    pub address: Option<String>,
    pub amount: String,
    pub successful: bool,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AmmTransaction {
    Swap(Swap),
    Mint(Mint),
    Burn(Burn),
    Creation(PoolCreation),
    Wrap(Wrap),
}

pub struct HistoryService {
    db: PgPool,
}

impl HistoryService {
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }

    /// Get pool swap history
    pub async fn get_pool_swap_history(
        &self,
        pool_id: &AlkaneId,
        limit: i32,
        offset: i32,
        _successful_only: bool,
    ) -> Result<(Vec<Swap>, i64)> {
        // Query TraceTrade table instead of PoolSwap
        let query = r#"
            SELECT 
                id::text as id,
                txid as "transactionId",
                block_height as "blockHeight",
                0 as "transactionIndex",
                pool_block::text as "poolBlockId",
                pool_tx::text as "poolTxId",
                token0_block::text as "soldTokenBlockId",
                token0_tx::text as "soldTokenTxId",
                token1_block::text as "boughtTokenBlockId",
                token1_tx::text as "boughtTokenTxId",
                amount0_in::float8 as "soldAmount",
                amount1_out::float8 as "boughtAmount",
                NULL as "sellerAddress",
                true as successful,
                timestamp
            FROM "TraceTrade"
            WHERE pool_block = $1 AND pool_tx = $2
            ORDER BY block_height DESC, vout DESC
            LIMIT $3 OFFSET $4
        "#;

        let pool_block: i32 = pool_id.block.parse().unwrap_or(0);
        let pool_tx: i64 = pool_id.tx.parse().unwrap_or(0);

        let swaps = sqlx::query_as::<_, Swap>(query)
            .bind(pool_block)
            .bind(pool_tx)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.db)
            .await?;

        let count_query = r#"SELECT COUNT(*) as count FROM "TraceTrade" WHERE pool_block = $1 AND pool_tx = $2"#;
        let total: (i64,) = sqlx::query_as(count_query)
            .bind(pool_block)
            .bind(pool_tx)
            .fetch_one(&self.db)
            .await?;

        Ok((swaps, total.0))
    }

    /// Get token swap history
    pub async fn get_token_swap_history(
        &self,
        token_id: &AlkaneId,
        limit: i32,
        offset: i32,
        _successful_only: bool,
    ) -> Result<(Vec<Swap>, i64)> {
        // Query TraceTrade table for trades involving this token
        let query = r#"
            SELECT 
                id::text as id,
                txid as "transactionId",
                block_height as "blockHeight",
                0 as "transactionIndex",
                pool_block::text as "poolBlockId",
                pool_tx::text as "poolTxId",
                token0_block::text as "soldTokenBlockId",
                token0_tx::text as "soldTokenTxId",
                token1_block::text as "boughtTokenBlockId",
                token1_tx::text as "boughtTokenTxId",
                amount0_in::float8 as "soldAmount",
                amount1_out::float8 as "boughtAmount",
                NULL as "sellerAddress",
                true as successful,
                timestamp
            FROM "TraceTrade"
            WHERE (token0_block = $1 AND token0_tx = $2)
               OR (token1_block = $1 AND token1_tx = $2)
            ORDER BY block_height DESC, vout DESC
            LIMIT $3 OFFSET $4
        "#;

        let token_block: i32 = token_id.block.parse().unwrap_or(0);
        let token_tx: i64 = token_id.tx.parse().unwrap_or(0);

        let swaps = sqlx::query_as::<_, Swap>(query)
            .bind(token_block)
            .bind(token_tx)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.db)
            .await?;

        let count_query = r#"SELECT COUNT(*) as count FROM "TraceTrade" 
               WHERE (token0_block = $1 AND token0_tx = $2)
                  OR (token1_block = $1 AND token1_tx = $2)"#;
        let total: (i64,) = sqlx::query_as(count_query)
            .bind(token_block)
            .bind(token_tx)
            .fetch_one(&self.db)
            .await?;

        Ok((swaps, total.0))
    }

    /// Get pool mint history
    pub async fn get_pool_mint_history(
        &self,
        pool_id: &AlkaneId,
        limit: i32,
        offset: i32,
        successful_only: bool,
    ) -> Result<(Vec<Mint>, i64)> {
        let success_clause = if successful_only {
            "AND successful = true"
        } else {
            ""
        };

        let query = format!(
            r#"
            SELECT * FROM "PoolMint"
            WHERE "poolBlockId" = $1 AND "poolTxId" = $2 {}
            ORDER BY "blockHeight" DESC, "transactionIndex" DESC
            LIMIT $3 OFFSET $4
            "#,
            success_clause
        );

        let mints = sqlx::query_as::<_, Mint>(&query)
            .bind(&pool_id.block)
            .bind(&pool_id.tx)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.db)
            .await?;

        let count_query = format!(
            r#"SELECT COUNT(*) as count FROM "PoolMint" WHERE "poolBlockId" = $1 AND "poolTxId" = $2 {}"#,
            success_clause
        );
        let total: (i64,) = sqlx::query_as(&count_query)
            .bind(&pool_id.block)
            .bind(&pool_id.tx)
            .fetch_one(&self.db)
            .await?;

        Ok((mints, total.0))
    }

    /// Get pool burn history
    pub async fn get_pool_burn_history(
        &self,
        pool_id: &AlkaneId,
        limit: i32,
        offset: i32,
        successful_only: bool,
    ) -> Result<(Vec<Burn>, i64)> {
        let success_clause = if successful_only {
            "AND successful = true"
        } else {
            ""
        };

        let query = format!(
            r#"
            SELECT * FROM "PoolBurn"
            WHERE "poolBlockId" = $1 AND "poolTxId" = $2 {}
            ORDER BY "blockHeight" DESC, "transactionIndex" DESC
            LIMIT $3 OFFSET $4
            "#,
            success_clause
        );

        let burns = sqlx::query_as::<_, Burn>(&query)
            .bind(&pool_id.block)
            .bind(&pool_id.tx)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.db)
            .await?;

        let count_query = format!(
            r#"SELECT COUNT(*) as count FROM "PoolBurn" WHERE "poolBlockId" = $1 AND "poolTxId" = $2 {}"#,
            success_clause
        );
        let total: (i64,) = sqlx::query_as(&count_query)
            .bind(&pool_id.block)
            .bind(&pool_id.tx)
            .fetch_one(&self.db)
            .await?;

        Ok((burns, total.0))
    }

    /// Get pool creation history
    pub async fn get_pool_creation_history(
        &self,
        factory_id: &AlkaneId,
        limit: i32,
        offset: i32,
    ) -> Result<(Vec<PoolCreation>, i64)> {
        // Get pools for this factory first, then get creations
        let creations = sqlx::query_as::<_, PoolCreation>(
            r#"
            SELECT pc.* FROM "PoolCreation" pc
            JOIN "Pool" p ON pc."poolBlockId" = p."poolBlockId" AND pc."poolTxId" = p."poolTxId"
            WHERE p."factoryBlockId" = $1 AND p."factoryTxId" = $2
            ORDER BY pc."blockHeight" DESC, pc."transactionIndex" DESC
            LIMIT $3 OFFSET $4
            "#
        )
        .bind(&factory_id.block)
        .bind(&factory_id.tx)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.db)
        .await?;

        let total: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*) as count FROM "PoolCreation" pc
            JOIN "Pool" p ON pc."poolBlockId" = p."poolBlockId" AND pc."poolTxId" = p."poolTxId"
            WHERE p."factoryBlockId" = $1 AND p."factoryTxId" = $2
            "#
        )
        .bind(&factory_id.block)
        .bind(&factory_id.tx)
        .fetch_one(&self.db)
        .await?;

        Ok((creations, total.0))
    }

    /// Get address swap history for a pool
    pub async fn get_address_swap_history_for_pool(
        &self,
        address: &str,
        pool_id: &AlkaneId,
        limit: i32,
        offset: i32,
        successful_only: bool,
    ) -> Result<(Vec<Swap>, i64)> {
        let success_clause = if successful_only {
            "AND successful = true"
        } else {
            ""
        };

        let query = format!(
            r#"
            SELECT * FROM "PoolSwap"
            WHERE "poolBlockId" = $1 AND "poolTxId" = $2 AND "sellerAddress" = $3 {}
            ORDER BY "blockHeight" DESC, "transactionIndex" DESC
            LIMIT $4 OFFSET $5
            "#,
            success_clause
        );

        let swaps = sqlx::query_as::<_, Swap>(&query)
            .bind(&pool_id.block)
            .bind(&pool_id.tx)
            .bind(address)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.db)
            .await?;

        let count_query = format!(
            r#"SELECT COUNT(*) as count FROM "PoolSwap" 
               WHERE "poolBlockId" = $1 AND "poolTxId" = $2 AND "sellerAddress" = $3 {}"#,
            success_clause
        );
        let total: (i64,) = sqlx::query_as(&count_query)
            .bind(&pool_id.block)
            .bind(&pool_id.tx)
            .bind(address)
            .fetch_one(&self.db)
            .await?;

        Ok((swaps, total.0))
    }

    /// Get address swap history for a token
    pub async fn get_address_swap_history_for_token(
        &self,
        address: &str,
        token_id: &AlkaneId,
        limit: i32,
        offset: i32,
        successful_only: bool,
    ) -> Result<(Vec<Swap>, i64)> {
        let success_clause = if successful_only {
            "AND successful = true"
        } else {
            ""
        };

        let query = format!(
            r#"
            SELECT * FROM "PoolSwap"
            WHERE "sellerAddress" = $1
                  AND (("soldTokenBlockId" = $2 AND "soldTokenTxId" = $3)
                       OR ("boughtTokenBlockId" = $2 AND "boughtTokenTxId" = $3)) {}
            ORDER BY "blockHeight" DESC, "transactionIndex" DESC
            LIMIT $4 OFFSET $5
            "#,
            success_clause
        );

        let swaps = sqlx::query_as::<_, Swap>(&query)
            .bind(address)
            .bind(&token_id.block)
            .bind(&token_id.tx)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.db)
            .await?;

        let count_query = format!(
            r#"SELECT COUNT(*) as count FROM "PoolSwap" 
               WHERE "sellerAddress" = $1
                     AND (("soldTokenBlockId" = $2 AND "soldTokenTxId" = $3)
                          OR ("boughtTokenBlockId" = $2 AND "boughtTokenTxId" = $3)) {}"#,
            success_clause
        );
        let total: (i64,) = sqlx::query_as(&count_query)
            .bind(address)
            .bind(&token_id.block)
            .bind(&token_id.tx)
            .fetch_one(&self.db)
            .await?;

        Ok((swaps, total.0))
    }

    /// Get wrap history for address
    pub async fn get_address_wrap_history(
        &self,
        address: &str,
        limit: i32,
        offset: i32,
        successful_only: bool,
    ) -> Result<(Vec<Wrap>, i64)> {
        let success_clause = if successful_only {
            "AND successful = true"
        } else {
            ""
        };

        let query = format!(
            r#"
            SELECT * FROM "SubfrostWrap"
            WHERE "address" = $1 {}
            ORDER BY "blockHeight" DESC, "transactionIndex" DESC
            LIMIT $2 OFFSET $3
            "#,
            success_clause
        );

        let wraps = sqlx::query_as::<_, Wrap>(&query)
            .bind(address)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.db)
            .await?;

        let count_query = format!(
            r#"SELECT COUNT(*) as count FROM "SubfrostWrap" WHERE "address" = $1 {}"#,
            success_clause
        );
        let total: (i64,) = sqlx::query_as(&count_query)
            .bind(address)
            .fetch_one(&self.db)
            .await?;

        Ok((wraps, total.0))
    }

    /// Get unwrap history for address
    pub async fn get_address_unwrap_history(
        &self,
        address: &str,
        limit: i32,
        offset: i32,
        successful_only: bool,
    ) -> Result<(Vec<Wrap>, i64)> {
        let success_clause = if successful_only {
            "AND successful = true"
        } else {
            ""
        };

        let query = format!(
            r#"
            SELECT * FROM "SubfrostUnwrap"
            WHERE "address" = $1 {}
            ORDER BY "blockHeight" DESC, "transactionIndex" DESC
            LIMIT $2 OFFSET $3
            "#,
            success_clause
        );

        let wraps = sqlx::query_as::<_, Wrap>(&query)
            .bind(address)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.db)
            .await?;

        let count_query = format!(
            r#"SELECT COUNT(*) as count FROM "SubfrostUnwrap" WHERE "address" = $1 {}"#,
            success_clause
        );
        let total: (i64,) = sqlx::query_as(&count_query)
            .bind(address)
            .fetch_one(&self.db)
            .await?;

        Ok((wraps, total.0))
    }

    /// Get all wrap history
    pub async fn get_all_wrap_history(
        &self,
        limit: i32,
        offset: i32,
        successful_only: bool,
    ) -> Result<(Vec<Wrap>, i64)> {
        let where_clause = if successful_only {
            "WHERE successful = true"
        } else {
            ""
        };

        let query = format!(
            r#"
            SELECT * FROM "SubfrostWrap"
            {}
            ORDER BY "blockHeight" DESC, "transactionIndex" DESC
            LIMIT $1 OFFSET $2
            "#,
            where_clause
        );

        let wraps = sqlx::query_as::<_, Wrap>(&query)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.db)
            .await?;

        let count_query = if successful_only {
            r#"SELECT COUNT(*) as count FROM "SubfrostWrap" WHERE successful = true"#
        } else {
            r#"SELECT COUNT(*) as count FROM "SubfrostWrap""#
        };
        
        let total: (i64,) = sqlx::query_as(count_query)
            .fetch_one(&self.db)
            .await?;

        Ok((wraps, total.0))
    }

    /// Get all unwrap history
    pub async fn get_all_unwrap_history(
        &self,
        limit: i32,
        offset: i32,
        successful_only: bool,
    ) -> Result<(Vec<Wrap>, i64)> {
        let where_clause = if successful_only {
            "WHERE successful = true"
        } else {
            ""
        };

        let query = format!(
            r#"
            SELECT * FROM "SubfrostUnwrap"
            {}
            ORDER BY "blockHeight" DESC, "transactionIndex" DESC
            LIMIT $1 OFFSET $2
            "#,
            where_clause
        );

        let wraps = sqlx::query_as::<_, Wrap>(&query)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.db)
            .await?;

        let count_query = if successful_only {
            r#"SELECT COUNT(*) as count FROM "SubfrostUnwrap" WHERE successful = true"#
        } else {
            r#"SELECT COUNT(*) as count FROM "SubfrostUnwrap""#
        };
        
        let total: (i64,) = sqlx::query_as(count_query)
            .fetch_one(&self.db)
            .await?;

        Ok((wraps, total.0))
    }

    /// Get total unwrap amount
    pub async fn get_total_unwrap_amount(&self) -> Result<String> {
        let result: (Option<sqlx::types::BigDecimal>,) = sqlx::query_as(
            r#"
            SELECT SUM(CAST(amount AS NUMERIC)) as total
            FROM "SubfrostUnwrap"
            WHERE successful = true
            "#
        )
        .fetch_one(&self.db)
        .await?;

        Ok(result.0
            .map(|t| t.to_string())
            .unwrap_or_else(|| "0".to_string()))
    }

    /// Get all AMM transactions for an address (combined query)
    pub async fn get_all_address_amm_tx_history(
        &self,
        address: &str,
        limit: i32,
        offset: i32,
        successful_only: bool,
        transaction_type: Option<&str>,
    ) -> Result<(Vec<serde_json::Value>, i64)> {
        let type_filter = match transaction_type {
            Some("swap") => "tx_type = 'swap'",
            Some("mint") => "tx_type = 'mint'",
            Some("burn") => "tx_type = 'burn'",
            Some("creation") => "tx_type = 'creation'",
            Some("wrap") => "tx_type = 'wrap'",
            Some("unwrap") => "tx_type = 'unwrap'",
            _ => "1=1",
        };

        let success_filter = if successful_only {
            "AND successful = true"
        } else {
            ""
        };

        let query = format!(
            r#"
            WITH combined AS (
                SELECT 
                    'swap' as tx_type,
                    s.id,
                    s."blockHeight" as block_height,
                    s.successful,
                    s."timestamp",
                    json_build_object(
                        'id', s.id,
                        'poolBlockId', s."poolBlockId",
                        'poolTxId', s."poolTxId",
                        'sellerAddress', s."sellerAddress",
                        'soldTokenBlockId', s."soldTokenBlockId",
                        'soldTokenTxId', s."soldTokenTxId",
                        'boughtTokenBlockId', s."boughtTokenBlockId",
                        'boughtTokenTxId', s."boughtTokenTxId",
                        'soldAmount', s."soldAmount",
                        'boughtAmount', s."boughtAmount",
                        'blockHeight', s."blockHeight",
                        'transactionId', s."transactionId",
                        'successful', s.successful,
                        'timestamp', s."timestamp"
                    ) as data
                FROM "PoolSwap" s
                WHERE s."sellerAddress" = $1 {}
                
                UNION ALL
                
                SELECT 
                    'mint' as tx_type,
                    m.id,
                    m."blockHeight" as block_height,
                    m.successful,
                    m."timestamp",
                    json_build_object(
                        'id', m.id,
                        'poolBlockId', m."poolBlockId",
                        'poolTxId', m."poolTxId",
                        'minterAddress', m."minterAddress",
                        'token0Amount', m."token0Amount",
                        'token1Amount', m."token1Amount",
                        'lpTokenAmount', m."lpTokenAmount",
                        'blockHeight', m."blockHeight",
                        'transactionId', m."transactionId",
                        'successful', m.successful,
                        'timestamp', m."timestamp"
                    ) as data
                FROM "PoolMint" m
                WHERE m."minterAddress" = $1 {}
                
                UNION ALL
                
                SELECT 
                    'burn' as tx_type,
                    b.id,
                    b."blockHeight" as block_height,
                    b.successful,
                    b."timestamp",
                    json_build_object(
                        'id', b.id,
                        'poolBlockId', b."poolBlockId",
                        'poolTxId', b."poolTxId",
                        'burnerAddress', b."burnerAddress",
                        'token0Amount', b."token0Amount",
                        'token1Amount', b."token1Amount",
                        'lpTokenAmount', b."lpTokenAmount",
                        'blockHeight', b."blockHeight",
                        'transactionId', b."transactionId",
                        'successful', b.successful,
                        'timestamp', b."timestamp"
                    ) as data
                FROM "PoolBurn" b
                WHERE b."burnerAddress" = $1 {}
                
                UNION ALL
                
                SELECT 
                    'creation' as tx_type,
                    pc.id,
                    pc."blockHeight" as block_height,
                    pc.successful,
                    pc."timestamp",
                    json_build_object(
                        'id', pc.id,
                        'poolBlockId', pc."poolBlockId",
                        'poolTxId', pc."poolTxId",
                        'creatorAddress', pc."creatorAddress",
                        'token0Amount', pc."token0Amount",
                        'token1Amount', pc."token1Amount",
                        'tokenSupply', pc."tokenSupply",
                        'blockHeight', pc."blockHeight",
                        'transactionId', pc."transactionId",
                        'timestamp', pc."timestamp"
                    ) as data
                FROM "PoolCreation" pc
                WHERE pc."creatorAddress" = $1
                
                UNION ALL
                
                SELECT 
                    'wrap' as tx_type,
                    w.id,
                    w."blockHeight" as block_height,
                    w.successful,
                    w."timestamp",
                    json_build_object(
                        'id', w.id,
                        'address', w."address",
                        'amount', w.amount,
                        'blockHeight', w."blockHeight",
                        'transactionId', w."transactionId",
                        'successful', w.successful,
                        'timestamp', w."timestamp"
                    ) as data
                FROM "SubfrostWrap" w
                WHERE w."address" = $1 {}

                UNION ALL
                
                SELECT 
                    'unwrap' as tx_type,
                    u.id,
                    u."blockHeight" as block_height,
                    u.successful,
                    u."timestamp",
                    json_build_object(
                        'id', u.id,
                        'address', u."address",
                        'amount', u.amount,
                        'blockHeight', u."blockHeight",
                        'transactionId', u."transactionId",
                        'successful', u.successful,
                        'timestamp', u."timestamp"
                    ) as data
                FROM "SubfrostUnwrap" u
                WHERE u."address" = $1 {}
            )
            SELECT 
                tx_type,
                data
            FROM combined
            WHERE {}
            ORDER BY block_height DESC, "timestamp" DESC
            LIMIT $2 OFFSET $3
            "#,
            success_filter, success_filter, success_filter, success_filter, success_filter, type_filter
        );

        #[derive(sqlx::FromRow)]
        struct TxRow {
            tx_type: String,
            data: serde_json::Value,
        }

        let rows: Vec<TxRow> = sqlx::query_as(&query)
            .bind(address)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.db)
            .await?;

        // Get total count
        let count_query = format!(
            r#"
            SELECT COUNT(*) FROM (
                SELECT id FROM "PoolSwap" WHERE "sellerAddress" = $1 {}
                UNION ALL
                SELECT id FROM "PoolMint" WHERE "minterAddress" = $1 {}
                UNION ALL
                SELECT id FROM "PoolBurn" WHERE "burnerAddress" = $1 {}
                UNION ALL
                SELECT id FROM "PoolCreation" WHERE "creatorAddress" = $1
                UNION ALL
                SELECT id FROM "SubfrostWrap" WHERE "address" = $1 {}
                UNION ALL
                SELECT id FROM "SubfrostUnwrap" WHERE "address" = $1 {}
            ) AS all_txs
            "#,
            success_filter, success_filter, success_filter, success_filter, success_filter
        );

        let total: (i64,) = sqlx::query_as(&count_query)
            .bind(address)
            .fetch_one(&self.db)
            .await?;

        let transactions: Vec<serde_json::Value> = rows
            .into_iter()
            .map(|row| {
                let mut obj = row.data;
                obj["type"] = serde_json::Value::String(row.tx_type);
                obj
            })
            .collect();

        Ok((transactions, total.0))
    }

    /// Get address pool creation history
    pub async fn get_address_pool_creation_history(
        &self,
        address: &str,
        limit: i32,
        offset: i32,
    ) -> Result<(Vec<PoolCreation>, i64)> {
        let creations = sqlx::query_as::<_, PoolCreation>(
            r#"
            SELECT * FROM "PoolCreation"
            WHERE "creatorAddress" = $1
            ORDER BY "blockHeight" DESC, "transactionIndex" DESC
            LIMIT $2 OFFSET $3
            "#
        )
        .bind(address)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.db)
        .await?;

        let total: (i64,) = sqlx::query_as(
            r#"SELECT COUNT(*) as count FROM "PoolCreation" WHERE "creatorAddress" = $1"#
        )
        .bind(address)
        .fetch_one(&self.db)
        .await?;

        Ok((creations, total.0))
    }

    /// Get address pool mint history
    pub async fn get_address_pool_mint_history(
        &self,
        address: &str,
        limit: i32,
        offset: i32,
        successful_only: bool,
    ) -> Result<(Vec<Mint>, i64)> {
        let success_clause = if successful_only {
            "AND successful = true"
        } else {
            ""
        };

        let query = format!(
            r#"
            SELECT * FROM "PoolMint"
            WHERE "minterAddress" = $1 {}
            ORDER BY "blockHeight" DESC, "transactionIndex" DESC
            LIMIT $2 OFFSET $3
            "#,
            success_clause
        );

        let mints = sqlx::query_as::<_, Mint>(&query)
            .bind(address)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.db)
            .await?;

        let count_query = format!(
            r#"SELECT COUNT(*) as count FROM "PoolMint" WHERE "minterAddress" = $1 {}"#,
            success_clause
        );
        let total: (i64,) = sqlx::query_as(&count_query)
            .bind(address)
            .fetch_one(&self.db)
            .await?;

        Ok((mints, total.0))
    }

    /// Get address pool burn history
    pub async fn get_address_pool_burn_history(
        &self,
        address: &str,
        limit: i32,
        offset: i32,
        successful_only: bool,
    ) -> Result<(Vec<Burn>, i64)> {
        let success_clause = if successful_only {
            "AND successful = true"
        } else {
            ""
        };

        let query = format!(
            r#"
            SELECT * FROM "PoolBurn"
            WHERE "burnerAddress" = $1 {}
            ORDER BY "blockHeight" DESC, "transactionIndex" DESC
            LIMIT $2 OFFSET $3
            "#,
            success_clause
        );

        let burns = sqlx::query_as::<_, Burn>(&query)
            .bind(address)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.db)
            .await?;

        let count_query = format!(
            r#"SELECT COUNT(*) as count FROM "PoolBurn" WHERE "burnerAddress" = $1 {}"#,
            success_clause
        );
        let total: (i64,) = sqlx::query_as(&count_query)
            .bind(address)
            .fetch_one(&self.db)
            .await?;

        Ok((burns, total.0))
    }

    /// Get all AMM transactions (not filtered by address)
    pub async fn get_all_amm_tx_history(
        &self,
        limit: i32,
        offset: i32,
        successful_only: bool,
        transaction_type: Option<&str>,
    ) -> Result<(Vec<serde_json::Value>, i64)> {
        let type_filter = match transaction_type {
            Some("swap") => "tx_type = 'swap'",
            Some("mint") => "tx_type = 'mint'",
            Some("burn") => "tx_type = 'burn'",
            Some("creation") => "tx_type = 'creation'",
            Some("wrap") => "tx_type = 'wrap'",
            Some("unwrap") => "tx_type = 'unwrap'",
            _ => "1=1",
        };

        let success_filter = if successful_only {
            "AND successful = true"
        } else {
            ""
        };

        let where_filter_swap = if successful_only { "WHERE successful = true" } else { "" };
        let where_filter_mint = if successful_only { "WHERE successful = true" } else { "" };
        let where_filter_burn = if successful_only { "WHERE successful = true" } else { "" };
        let where_filter_wrap = if successful_only { "WHERE successful = true" } else { "" };
        let where_filter_unwrap = if successful_only { "WHERE successful = true" } else { "" };

        let query = format!(
            r#"
            WITH combined AS (
                SELECT 
                    'swap' as tx_type,
                    s.id,
                    s."blockHeight" as block_height,
                    s.successful,
                    s."timestamp",
                    json_build_object(
                        'id', s.id,
                        'poolBlockId', s."poolBlockId",
                        'poolTxId', s."poolTxId",
                        'sellerAddress', s."sellerAddress",
                        'soldTokenBlockId', s."soldTokenBlockId",
                        'soldTokenTxId', s."soldTokenTxId",
                        'boughtTokenBlockId', s."boughtTokenBlockId",
                        'boughtTokenTxId', s."boughtTokenTxId",
                        'soldAmount', s."soldAmount",
                        'boughtAmount', s."boughtAmount",
                        'blockHeight', s."blockHeight",
                        'transactionId', s."transactionId",
                        'successful', s.successful,
                        'timestamp', s."timestamp"
                    ) as data
                FROM "PoolSwap" s
                {}
                
                UNION ALL
                
                SELECT 
                    'mint' as tx_type,
                    m.id,
                    m."blockHeight" as block_height,
                    m.successful,
                    m."timestamp",
                    json_build_object(
                        'id', m.id,
                        'poolBlockId', m."poolBlockId",
                        'poolTxId', m."poolTxId",
                        'minterAddress', m."minterAddress",
                        'token0Amount', m."token0Amount",
                        'token1Amount', m."token1Amount",
                        'lpTokenAmount', m."lpTokenAmount",
                        'blockHeight', m."blockHeight",
                        'transactionId', m."transactionId",
                        'successful', m.successful,
                        'timestamp', m."timestamp"
                    ) as data
                FROM "PoolMint" m
                {}
                
                UNION ALL
                
                SELECT 
                    'burn' as tx_type,
                    b.id,
                    b."blockHeight" as block_height,
                    b.successful,
                    b."timestamp",
                    json_build_object(
                        'id', b.id,
                        'poolBlockId', b."poolBlockId",
                        'poolTxId', b."poolTxId",
                        'burnerAddress', b."burnerAddress",
                        'token0Amount', b."token0Amount",
                        'token1Amount', b."token1Amount",
                        'lpTokenAmount', b."lpTokenAmount",
                        'blockHeight', b."blockHeight",
                        'transactionId', b."transactionId",
                        'successful', b.successful,
                        'timestamp', b."timestamp"
                    ) as data
                FROM "PoolBurn" b
                {}
                
                UNION ALL
                
                SELECT 
                    'creation' as tx_type,
                    pc.id,
                    pc."blockHeight" as block_height,
                    pc.successful,
                    pc."timestamp",
                    json_build_object(
                        'id', pc.id,
                        'poolBlockId', pc."poolBlockId",
                        'poolTxId', pc."poolTxId",
                        'creatorAddress', pc."creatorAddress",
                        'token0Amount', pc."token0Amount",
                        'token1Amount', pc."token1Amount",
                        'tokenSupply', pc."tokenSupply",
                        'blockHeight', pc."blockHeight",
                        'transactionId', pc."transactionId",
                        'timestamp', pc."timestamp"
                    ) as data
                FROM "PoolCreation" pc
                
                UNION ALL
                
                SELECT 
                    'wrap' as tx_type,
                    w.id,
                    w."blockHeight" as block_height,
                    w.successful,
                    w."timestamp",
                    json_build_object(
                        'id', w.id,
                        'address', w."address",
                        'amount', w.amount,
                        'blockHeight', w."blockHeight",
                        'transactionId', w."transactionId",
                        'successful', w.successful,
                        'timestamp', w."timestamp"
                    ) as data
                FROM "SubfrostWrap" w
                {}

                UNION ALL
                
                SELECT 
                    'unwrap' as tx_type,
                    u.id,
                    u."blockHeight" as block_height,
                    u.successful,
                    u."timestamp",
                    json_build_object(
                        'id', u.id,
                        'address', u."address",
                        'amount', u.amount,
                        'blockHeight', u."blockHeight",
                        'transactionId', u."transactionId",
                        'successful', u.successful,
                        'timestamp', u."timestamp"
                    ) as data
                FROM "SubfrostUnwrap" u
                {}
            )
            SELECT 
                tx_type,
                data
            FROM combined
            WHERE {}
            ORDER BY block_height DESC, "timestamp" DESC
            LIMIT $1 OFFSET $2
            "#,
            where_filter_swap, where_filter_mint, where_filter_burn, where_filter_wrap, where_filter_unwrap, type_filter
        );

        #[derive(sqlx::FromRow)]
        struct TxRow {
            tx_type: String,
            data: serde_json::Value,
        }

        let rows: Vec<TxRow> = sqlx::query_as(&query)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.db)
            .await?;

        // Get total count
        let count_query = format!(
            r#"
            SELECT COUNT(*) FROM (
                SELECT id FROM "PoolSwap" {}
                UNION ALL
                SELECT id FROM "PoolMint" {}
                UNION ALL
                SELECT id FROM "PoolBurn" {}
                UNION ALL
                SELECT id FROM "PoolCreation"
                UNION ALL
                SELECT id FROM "SubfrostWrap" {}
                UNION ALL
                SELECT id FROM "SubfrostUnwrap" {}
            ) AS all_txs
            "#,
            where_filter_swap, where_filter_mint, where_filter_burn, where_filter_wrap, where_filter_unwrap
        );

        let total: (i64,) = sqlx::query_as(&count_query)
            .fetch_one(&self.db)
            .await?;

        let transactions: Vec<serde_json::Value> = rows
            .into_iter()
            .map(|row| {
                let mut obj = row.data;
                obj["type"] = serde_json::Value::String(row.tx_type);
                obj
            })
            .collect();

        Ok((transactions, total.0))
    }
}
