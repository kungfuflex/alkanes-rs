use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use super::alkanes_rpc::AlkaneId;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Swap {
    pub id: i32,
    pub pool_id: String,
    pub from_address: String,
    pub token_in_block_id: String,
    pub token_in_tx_id: String,
    pub token_out_block_id: String,
    pub token_out_tx_id: String,
    pub amount_in: String,
    pub amount_out: String,
    pub block_height: i32,
    pub txid: String,
    pub successful: bool,
    pub timestamp: Option<chrono::NaiveDateTime>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Mint {
    pub id: i32,
    pub pool_id: String,
    pub from_address: String,
    pub token0_amount: String,
    pub token1_amount: String,
    pub liquidity_amount: String,
    pub block_height: i32,
    pub txid: String,
    pub successful: bool,
    pub timestamp: Option<chrono::NaiveDateTime>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Burn {
    pub id: i32,
    pub pool_id: String,
    pub from_address: String,
    pub token0_amount: String,
    pub token1_amount: String,
    pub liquidity_amount: String,
    pub block_height: i32,
    pub txid: String,
    pub successful: bool,
    pub timestamp: Option<chrono::NaiveDateTime>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct PoolCreation {
    pub id: i32,
    pub pool_id: String,
    pub creator_address: String,
    pub token0_amount: String,
    pub token1_amount: String,
    pub block_height: i32,
    pub txid: String,
    pub timestamp: Option<chrono::NaiveDateTime>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Wrap {
    pub id: i32,
    pub from_address: String,
    pub amount: String,
    pub block_height: i32,
    pub txid: String,
    pub successful: bool,
    pub timestamp: Option<chrono::NaiveDateTime>,
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
        successful_only: bool,
    ) -> Result<(Vec<Swap>, i64)> {
        let pool_db_id = format!("{}:{}", pool_id.block, pool_id.tx);
        
        let where_clause = if successful_only {
            "WHERE pool_id = $1 AND successful = true"
        } else {
            "WHERE pool_id = $1"
        };

        let query = format!(
            r#"
            SELECT * FROM swap
            {}
            ORDER BY block_height DESC, id DESC
            LIMIT $2 OFFSET $3
            "#,
            where_clause
        );

        let swaps = sqlx::query_as::<_, Swap>(&query)
            .bind(&pool_db_id)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.db)
            .await?;

        let count_query = format!("SELECT COUNT(*) as count FROM swap {}", where_clause);
        let total: (i64,) = sqlx::query_as(&count_query)
            .bind(&pool_db_id)
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
        successful_only: bool,
    ) -> Result<(Vec<Swap>, i64)> {
        let where_clause = if successful_only {
            r#"WHERE (token_in_block_id = $1 AND token_in_tx_id = $2 
                    OR token_out_block_id = $1 AND token_out_tx_id = $2)
                   AND successful = true"#
        } else {
            r#"WHERE (token_in_block_id = $1 AND token_in_tx_id = $2 
                    OR token_out_block_id = $1 AND token_out_tx_id = $2)"#
        };

        let query = format!(
            r#"
            SELECT * FROM swap
            {}
            ORDER BY block_height DESC, id DESC
            LIMIT $3 OFFSET $4
            "#,
            where_clause
        );

        let swaps = sqlx::query_as::<_, Swap>(&query)
            .bind(&token_id.block)
            .bind(&token_id.tx)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.db)
            .await?;

        let count_query = format!("SELECT COUNT(*) as count FROM swap {}", where_clause);
        let total: (i64,) = sqlx::query_as(&count_query)
            .bind(&token_id.block)
            .bind(&token_id.tx)
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
        let pool_db_id = format!("{}:{}", pool_id.block, pool_id.tx);
        
        let where_clause = if successful_only {
            "WHERE pool_id = $1 AND successful = true"
        } else {
            "WHERE pool_id = $1"
        };

        let query = format!(
            r#"
            SELECT * FROM mint
            {}
            ORDER BY block_height DESC, id DESC
            LIMIT $2 OFFSET $3
            "#,
            where_clause
        );

        let mints = sqlx::query_as::<_, Mint>(&query)
            .bind(&pool_db_id)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.db)
            .await?;

        let count_query = format!("SELECT COUNT(*) as count FROM mint {}", where_clause);
        let total: (i64,) = sqlx::query_as(&count_query)
            .bind(&pool_db_id)
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
        let pool_db_id = format!("{}:{}", pool_id.block, pool_id.tx);
        
        let where_clause = if successful_only {
            "WHERE pool_id = $1 AND successful = true"
        } else {
            "WHERE pool_id = $1"
        };

        let query = format!(
            r#"
            SELECT * FROM burn
            {}
            ORDER BY block_height DESC, id DESC
            LIMIT $2 OFFSET $3
            "#,
            where_clause
        );

        let burns = sqlx::query_as::<_, Burn>(&query)
            .bind(&pool_db_id)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.db)
            .await?;

        let count_query = format!("SELECT COUNT(*) as count FROM burn {}", where_clause);
        let total: (i64,) = sqlx::query_as(&count_query)
            .bind(&pool_db_id)
            .fetch_one(&self.db)
            .await?;

        Ok((burns, total.0))
    }

    /// Get pool creation history
    pub async fn get_pool_creation_history(
        &self,
        limit: i32,
        offset: i32,
    ) -> Result<(Vec<PoolCreation>, i64)> {
        let creations = sqlx::query_as::<_, PoolCreation>(
            r#"
            SELECT * FROM pool_creation
            ORDER BY block_height DESC, id DESC
            LIMIT $1 OFFSET $2
            "#
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.db)
        .await?;

        let total: (i64,) = sqlx::query_as("SELECT COUNT(*) as count FROM pool_creation")
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
        let pool_db_id = format!("{}:{}", pool_id.block, pool_id.tx);
        
        let where_clause = if successful_only {
            "WHERE pool_id = $1 AND from_address = $2 AND successful = true"
        } else {
            "WHERE pool_id = $1 AND from_address = $2"
        };

        let query = format!(
            r#"
            SELECT * FROM swap
            {}
            ORDER BY block_height DESC, id DESC
            LIMIT $3 OFFSET $4
            "#,
            where_clause
        );

        let swaps = sqlx::query_as::<_, Swap>(&query)
            .bind(&pool_db_id)
            .bind(address)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.db)
            .await?;

        let count_query = format!("SELECT COUNT(*) as count FROM swap {}", where_clause);
        let total: (i64,) = sqlx::query_as(&count_query)
            .bind(&pool_db_id)
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
        let where_clause = if successful_only {
            r#"WHERE from_address = $1 
                   AND (token_in_block_id = $2 AND token_in_tx_id = $3 
                        OR token_out_block_id = $2 AND token_out_tx_id = $3)
                   AND successful = true"#
        } else {
            r#"WHERE from_address = $1 
                   AND (token_in_block_id = $2 AND token_in_tx_id = $3 
                        OR token_out_block_id = $2 AND token_out_tx_id = $3)"#
        };

        let query = format!(
            r#"
            SELECT * FROM swap
            {}
            ORDER BY block_height DESC, id DESC
            LIMIT $4 OFFSET $5
            "#,
            where_clause
        );

        let swaps = sqlx::query_as::<_, Swap>(&query)
            .bind(address)
            .bind(&token_id.block)
            .bind(&token_id.tx)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.db)
            .await?;

        let count_query = format!("SELECT COUNT(*) as count FROM swap {}", where_clause);
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
        let where_clause = if successful_only {
            "WHERE from_address = $1 AND successful = true"
        } else {
            "WHERE from_address = $1"
        };

        let query = format!(
            r#"
            SELECT * FROM wrap
            {}
            ORDER BY block_height DESC, id DESC
            LIMIT $2 OFFSET $3
            "#,
            where_clause
        );

        let wraps = sqlx::query_as::<_, Wrap>(&query)
            .bind(address)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.db)
            .await?;

        let count_query = format!("SELECT COUNT(*) as count FROM wrap {}", where_clause);
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
            SELECT * FROM wrap
            {}
            ORDER BY block_height DESC, id DESC
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
            "SELECT COUNT(*) as count FROM wrap WHERE successful = true"
        } else {
            "SELECT COUNT(*) as count FROM wrap"
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
            FROM wrap
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
        // Build the WHERE clause for transaction type filter
        let type_filter = match transaction_type {
            Some("swap") => "tx_type = 'swap'",
            Some("mint") => "tx_type = 'mint'",
            Some("burn") => "tx_type = 'burn'",
            Some("creation") => "tx_type = 'creation'",
            Some("wrap") => "tx_type = 'wrap'",
            Some("unwrap") => "tx_type = 'unwrap'",
            _ => "1=1", // no filter
        };

        let success_filter = if successful_only {
            "AND successful = true"
        } else {
            ""
        };

        // Combined query using UNION ALL for all transaction types
        let query = format!(
            r#"
            WITH combined AS (
                SELECT 
                    'swap' as tx_type,
                    s.id,
                    s.pool_id,
                    s.from_address,
                    s.txid,
                    s.block_height,
                    s.timestamp,
                    s.successful,
                    json_build_object(
                        'id', s.id,
                        'pool_id', s.pool_id,
                        'from_address', s.from_address,
                        'token_in_block_id', s.token_in_block_id,
                        'token_in_tx_id', s.token_in_tx_id,
                        'token_out_block_id', s.token_out_block_id,
                        'token_out_tx_id', s.token_out_tx_id,
                        'amount_in', s.amount_in,
                        'amount_out', s.amount_out,
                        'block_height', s.block_height,
                        'txid', s.txid,
                        'successful', s.successful,
                        'timestamp', s.timestamp
                    ) as data
                FROM swap s
                WHERE s.from_address = $1 {}
                
                UNION ALL
                
                SELECT 
                    'mint' as tx_type,
                    m.id,
                    m.pool_id,
                    m.from_address,
                    m.txid,
                    m.block_height,
                    m.timestamp,
                    m.successful,
                    json_build_object(
                        'id', m.id,
                        'pool_id', m.pool_id,
                        'from_address', m.from_address,
                        'token0_amount', m.token0_amount,
                        'token1_amount', m.token1_amount,
                        'liquidity_amount', m.liquidity_amount,
                        'block_height', m.block_height,
                        'txid', m.txid,
                        'successful', m.successful,
                        'timestamp', m.timestamp
                    ) as data
                FROM mint m
                WHERE m.from_address = $1 {}
                
                UNION ALL
                
                SELECT 
                    'burn' as tx_type,
                    b.id,
                    b.pool_id,
                    b.from_address,
                    b.txid,
                    b.block_height,
                    b.timestamp,
                    b.successful,
                    json_build_object(
                        'id', b.id,
                        'pool_id', b.pool_id,
                        'from_address', b.from_address,
                        'token0_amount', b.token0_amount,
                        'token1_amount', b.token1_amount,
                        'liquidity_amount', b.liquidity_amount,
                        'block_height', b.block_height,
                        'txid', b.txid,
                        'successful', b.successful,
                        'timestamp', b.timestamp
                    ) as data
                FROM burn b
                WHERE b.from_address = $1 {}
                
                UNION ALL
                
                SELECT 
                    'creation' as tx_type,
                    pc.id,
                    pc.pool_id,
                    pc.creator_address as from_address,
                    pc.txid,
                    pc.block_height,
                    pc.timestamp,
                    true as successful,
                    json_build_object(
                        'id', pc.id,
                        'pool_id', pc.pool_id,
                        'creator_address', pc.creator_address,
                        'token0_amount', pc.token0_amount,
                        'token1_amount', pc.token1_amount,
                        'block_height', pc.block_height,
                        'txid', pc.txid,
                        'timestamp', pc.timestamp
                    ) as data
                FROM pool_creation pc
                WHERE pc.creator_address = $1
                
                UNION ALL
                
                SELECT 
                    'wrap' as tx_type,
                    w.id,
                    '' as pool_id,
                    w.from_address,
                    w.txid,
                    w.block_height,
                    w.timestamp,
                    w.successful,
                    json_build_object(
                        'id', w.id,
                        'from_address', w.from_address,
                        'amount', w.amount,
                        'block_height', w.block_height,
                        'txid', w.txid,
                        'successful', w.successful,
                        'timestamp', w.timestamp
                    ) as data
                FROM wrap w
                WHERE w.from_address = $1 {}
            )
            SELECT 
                tx_type,
                data
            FROM combined
            WHERE {}
            ORDER BY block_height DESC, id DESC
            LIMIT $2 OFFSET $3
            "#,
            success_filter, success_filter, success_filter, success_filter, type_filter
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
                SELECT id FROM swap WHERE from_address = $1 {}
                UNION ALL
                SELECT id FROM mint WHERE from_address = $1 {}
                UNION ALL
                SELECT id FROM burn WHERE from_address = $1 {}
                UNION ALL
                SELECT id FROM pool_creation WHERE creator_address = $1
                UNION ALL
                SELECT id FROM wrap WHERE from_address = $1 {}
            ) AS all_txs
            "#,
            success_filter, success_filter, success_filter, success_filter
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

        let query = format!(
            r#"
            WITH combined AS (
                SELECT 
                    'swap' as tx_type,
                    s.id,
                    s.block_height,
                    s.successful,
                    json_build_object(
                        'id', s.id,
                        'pool_id', s.pool_id,
                        'from_address', s.from_address,
                        'token_in_block_id', s.token_in_block_id,
                        'token_in_tx_id', s.token_in_tx_id,
                        'token_out_block_id', s.token_out_block_id,
                        'token_out_tx_id', s.token_out_tx_id,
                        'amount_in', s.amount_in,
                        'amount_out', s.amount_out,
                        'block_height', s.block_height,
                        'txid', s.txid,
                        'successful', s.successful,
                        'timestamp', s.timestamp
                    ) as data
                FROM swap s
                WHERE 1=1 {}
                
                UNION ALL
                
                SELECT 
                    'mint' as tx_type,
                    m.id,
                    m.block_height,
                    m.successful,
                    json_build_object(
                        'id', m.id,
                        'pool_id', m.pool_id,
                        'from_address', m.from_address,
                        'token0_amount', m.token0_amount,
                        'token1_amount', m.token1_amount,
                        'liquidity_amount', m.liquidity_amount,
                        'block_height', m.block_height,
                        'txid', m.txid,
                        'successful', m.successful,
                        'timestamp', m.timestamp
                    ) as data
                FROM mint m
                WHERE 1=1 {}
                
                UNION ALL
                
                SELECT 
                    'burn' as tx_type,
                    b.id,
                    b.block_height,
                    b.successful,
                    json_build_object(
                        'id', b.id,
                        'pool_id', b.pool_id,
                        'from_address', b.from_address,
                        'token0_amount', b.token0_amount,
                        'token1_amount', b.token1_amount,
                        'liquidity_amount', b.liquidity_amount,
                        'block_height', b.block_height,
                        'txid', b.txid,
                        'successful', b.successful,
                        'timestamp', b.timestamp
                    ) as data
                FROM burn b
                WHERE 1=1 {}
                
                UNION ALL
                
                SELECT 
                    'creation' as tx_type,
                    pc.id,
                    pc.block_height,
                    true as successful,
                    json_build_object(
                        'id', pc.id,
                        'pool_id', pc.pool_id,
                        'creator_address', pc.creator_address,
                        'token0_amount', pc.token0_amount,
                        'token1_amount', pc.token1_amount,
                        'block_height', pc.block_height,
                        'txid', pc.txid,
                        'timestamp', pc.timestamp
                    ) as data
                FROM pool_creation pc
                
                UNION ALL
                
                SELECT 
                    'wrap' as tx_type,
                    w.id,
                    w.block_height,
                    w.successful,
                    json_build_object(
                        'id', w.id,
                        'from_address', w.from_address,
                        'amount', w.amount,
                        'block_height', w.block_height,
                        'txid', w.txid,
                        'successful', w.successful,
                        'timestamp', w.timestamp
                    ) as data
                FROM wrap w
                WHERE 1=1 {}
            )
            SELECT 
                tx_type,
                data
            FROM combined
            WHERE {}
            ORDER BY block_height DESC, id DESC
            LIMIT $1 OFFSET $2
            "#,
            success_filter, success_filter, success_filter, success_filter, type_filter
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
                SELECT id FROM swap WHERE 1=1 {}
                UNION ALL
                SELECT id FROM mint WHERE 1=1 {}
                UNION ALL
                SELECT id FROM burn WHERE 1=1 {}
                UNION ALL
                SELECT id FROM pool_creation
                UNION ALL
                SELECT id FROM wrap WHERE 1=1 {}
            ) AS all_txs
            "#,
            success_filter, success_filter, success_filter, success_filter
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

    /// Get address pool creation history
    pub async fn get_address_pool_creation_history(
        &self,
        address: &str,
        limit: i32,
        offset: i32,
    ) -> Result<(Vec<PoolCreation>, i64)> {
        let creations = sqlx::query_as::<_, PoolCreation>(
            r#"
            SELECT * FROM pool_creation
            WHERE creator_address = $1
            ORDER BY block_height DESC, id DESC
            LIMIT $2 OFFSET $3
            "#
        )
        .bind(address)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.db)
        .await?;

        let total: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) as count FROM pool_creation WHERE creator_address = $1"
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
        let where_clause = if successful_only {
            "WHERE from_address = $1 AND successful = true"
        } else {
            "WHERE from_address = $1"
        };

        let query = format!(
            r#"
            SELECT * FROM mint
            {}
            ORDER BY block_height DESC, id DESC
            LIMIT $2 OFFSET $3
            "#,
            where_clause
        );

        let mints = sqlx::query_as::<_, Mint>(&query)
            .bind(address)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.db)
            .await?;

        let count_query = format!("SELECT COUNT(*) as count FROM mint {}", where_clause);
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
        let where_clause = if successful_only {
            "WHERE from_address = $1 AND successful = true"
        } else {
            "WHERE from_address = $1"
        };

        let query = format!(
            r#"
            SELECT * FROM burn
            {}
            ORDER BY block_height DESC, id DESC
            LIMIT $2 OFFSET $3
            "#,
            where_clause
        );

        let burns = sqlx::query_as::<_, Burn>(&query)
            .bind(address)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.db)
            .await?;

        let count_query = format!("SELECT COUNT(*) as count FROM burn {}", where_clause);
        let total: (i64,) = sqlx::query_as(&count_query)
            .bind(address)
            .fetch_one(&self.db)
            .await?;

        Ok((burns, total.0))
    }
}
