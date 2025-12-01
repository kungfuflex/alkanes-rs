use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use chrono::{DateTime, Utc};

/// Query service for balance data using optimized trace transform tables
pub struct BalanceQueryService {
    pool: PgPool,
}

impl BalanceQueryService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
    
    /// Get aggregate balances for an address
    pub async fn get_address_balances(&self, address: &str) -> Result<Vec<BalanceInfo>> {
        let rows = sqlx::query(
            r#"SELECT alkane_block, alkane_tx, total_amount::TEXT 
               FROM "TraceBalanceAggregate"
               WHERE address = $1 AND total_amount > 0"#
        )
        .bind(address)
        .fetch_all(&self.pool)
        .await?;
        
        let mut balances = Vec::new();
        for row in rows {
            let block: i32 = row.get(0);
            let tx: i64 = row.get(1);
            let amount_str: String = row.get(2);
            let amount: u128 = amount_str.parse().unwrap_or(0);
            
            balances.push(BalanceInfo {
                alkane_id: format!("{}:{}", block, tx),
                amount,
            });
        }
        
        Ok(balances)
    }
    
    /// Get UTXO-level balances for an address
    pub async fn get_address_utxos(&self, address: &str) -> Result<Vec<UtxoBalanceInfo>> {
        let rows = sqlx::query(
            r#"SELECT outpoint_txid, outpoint_vout, alkane_block, alkane_tx, amount::TEXT, block_height
               FROM "TraceBalanceUtxo"
               WHERE address = $1 AND NOT spent"#
        )
        .bind(address)
        .fetch_all(&self.pool)
        .await?;
        
        let mut utxos = Vec::new();
        for row in rows {
            let txid: String = row.get(0);
            let vout: i32 = row.get(1);
            let block: i32 = row.get(2);
            let tx: i64 = row.get(3);
            let amount_str: String = row.get(4);
            let block_height: i32 = row.get(5);
            
            let amount: u128 = amount_str.parse().unwrap_or(0);
            
            utxos.push(UtxoBalanceInfo {
                outpoint: format!("{}:{}", txid, vout),
                alkane_id: format!("{}:{}", block, tx),
                amount,
                block_height,
            });
        }
        
        Ok(utxos)
    }
    
    /// Get holders for an alkane
    pub async fn get_holders(&self, alkane_block: i32, alkane_tx: i64, limit: i64) -> Result<Vec<HolderInfo>> {
        let rows = sqlx::query(
            r#"SELECT address, total_amount::TEXT
               FROM "TraceHolder"
               WHERE alkane_block = $1 AND alkane_tx = $2 AND total_amount > 0
               ORDER BY total_amount DESC
               LIMIT $3"#
        )
        .bind(alkane_block)
        .bind(alkane_tx)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        
        let mut holders = Vec::new();
        for row in rows {
            let address: String = row.get(0);
            let amount_str: String = row.get(1);
            let amount: u128 = amount_str.parse().unwrap_or(0);
            
            holders.push(HolderInfo {
                address,
                amount,
            });
        }
        
        Ok(holders)
    }
    
    /// Get holder count for an alkane
    pub async fn get_holder_count(&self, alkane_block: i32, alkane_tx: i64) -> Result<i64> {
        let count: i64 = sqlx::query_scalar(
            r#"SELECT count FROM "TraceHolderCount"
               WHERE alkane_block = $1 AND alkane_tx = $2"#
        )
        .bind(alkane_block)
        .bind(alkane_tx)
        .fetch_optional(&self.pool)
        .await?
        .unwrap_or(0);
        
        Ok(count)
    }
}

/// Query service for AMM data using optimized trace transform tables
pub struct AmmQueryService {
    pool: PgPool,
}

impl AmmQueryService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
    
    /// Get trades for a pool
    pub async fn get_pool_trades(&self, pool_block: i32, pool_tx: i64, limit: i64) -> Result<Vec<TradeInfo>> {
        let rows = sqlx::query(
            r#"SELECT txid, vout, token0_block, token0_tx, token1_block, token1_tx,
                      amount0_in::TEXT, amount1_in::TEXT, amount0_out::TEXT, amount1_out::TEXT,
                      reserve0_after::TEXT, reserve1_after::TEXT, timestamp, block_height
               FROM "TraceTrade"
               WHERE pool_block = $1 AND pool_tx = $2
               ORDER BY block_height DESC, vout DESC
               LIMIT $3"#
        )
        .bind(pool_block)
        .bind(pool_tx)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        
        let mut trades = Vec::new();
        for row in rows {
            trades.push(TradeInfo {
                txid: row.get(0),
                vout: row.get(1),
                token0_id: format!("{}:{}", row.get::<i32, _>(2), row.get::<i64, _>(3)),
                token1_id: format!("{}:{}", row.get::<i32, _>(4), row.get::<i64, _>(5)),
                amount0_in: row.get::<String, _>(6).parse().unwrap_or(0),
                amount1_in: row.get::<String, _>(7).parse().unwrap_or(0),
                amount0_out: row.get::<String, _>(8).parse().unwrap_or(0),
                amount1_out: row.get::<String, _>(9).parse().unwrap_or(0),
                reserve0_after: row.get::<String, _>(10).parse().unwrap_or(0),
                reserve1_after: row.get::<String, _>(11).parse().unwrap_or(0),
                timestamp: row.get(12),
                block_height: row.get(13),
            });
        }
        
        Ok(trades)
    }
    
    /// Get latest reserves for a pool
    pub async fn get_pool_reserves(&self, pool_block: i32, pool_tx: i64) -> Result<Option<ReserveInfo>> {
        let row = sqlx::query(
            r#"SELECT reserve0::TEXT, reserve1::TEXT, timestamp, block_height
               FROM "TraceReserveSnapshot"
               WHERE pool_block = $1 AND pool_tx = $2
               ORDER BY block_height DESC, timestamp DESC
               LIMIT 1"#
        )
        .bind(pool_block)
        .bind(pool_tx)
        .fetch_optional(&self.pool)
        .await?;
        
        if let Some(row) = row {
            Ok(Some(ReserveInfo {
                reserve0: row.get::<String, _>(0).parse().unwrap_or(0),
                reserve1: row.get::<String, _>(1).parse().unwrap_or(0),
                timestamp: row.get(2),
                block_height: row.get(3),
            }))
        } else {
            Ok(None)
        }
    }
    
    /// Get candles for a pool
    pub async fn get_pool_candles(
        &self,
        pool_block: i32,
        pool_tx: i64,
        interval: &str,
        limit: i64,
    ) -> Result<Vec<CandleInfo>> {
        let rows = sqlx::query(
            r#"SELECT open_time, close_time, open, high, low, close,
                      volume0::TEXT, volume1::TEXT, trade_count
               FROM "TraceCandle"
               WHERE pool_block = $1 AND pool_tx = $2 AND interval = $3
               ORDER BY open_time DESC
               LIMIT $4"#
        )
        .bind(pool_block)
        .bind(pool_tx)
        .bind(interval)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        
        let mut candles = Vec::new();
        for row in rows {
            candles.push(CandleInfo {
                open_time: row.get(0),
                close_time: row.get(1),
                open: row.get(2),
                high: row.get(3),
                low: row.get(4),
                close: row.get(5),
                volume0: row.get::<String, _>(6).parse().unwrap_or(0),
                volume1: row.get::<String, _>(7).parse().unwrap_or(0),
                trade_count: row.get(8),
            });
        }
        
        Ok(candles)
    }
}

// Response types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceInfo {
    pub alkane_id: String,
    pub amount: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UtxoBalanceInfo {
    pub outpoint: String,
    pub alkane_id: String,
    pub amount: u128,
    pub block_height: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HolderInfo {
    pub address: String,
    pub amount: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeInfo {
    pub txid: String,
    pub vout: i32,
    pub token0_id: String,
    pub token1_id: String,
    pub amount0_in: u128,
    pub amount1_in: u128,
    pub amount0_out: u128,
    pub amount1_out: u128,
    pub reserve0_after: u128,
    pub reserve1_after: u128,
    pub timestamp: DateTime<Utc>,
    pub block_height: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReserveInfo {
    pub reserve0: u128,
    pub reserve1: u128,
    pub timestamp: DateTime<Utc>,
    pub block_height: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandleInfo {
    pub open_time: DateTime<Utc>,
    pub close_time: DateTime<Utc>,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume0: u128,
    pub volume1: u128,
    pub trade_count: i32,
}
