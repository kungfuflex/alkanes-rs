use crate::types::{AlkaneId, Result};
use crate::trackers::amm::{TradeEvent, ReserveSnapshot};
use chrono::{DateTime, Utc};
use sqlx::PgPool;

/// Optimized AMM tracker that writes directly to proper indexed tables
pub struct OptimizedAmmTracker {
    pool: PgPool,
    candle_intervals: Vec<String>,
}

impl OptimizedAmmTracker {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            candle_intervals: vec![
                "1m".to_string(),
                "5m".to_string(),
                "15m".to_string(),
                "1h".to_string(),
                "4h".to_string(),
                "1d".to_string(),
            ],
        }
    }
    
    pub fn with_intervals(pool: PgPool, intervals: Vec<String>) -> Self {
        Self {
            pool,
            candle_intervals: intervals,
        }
    }
    
    /// Process trade events and update all AMM tables
    pub async fn process_trades(&self, trades: Vec<TradeEvent>) -> Result<()> {
        if trades.is_empty() {
            return Ok(());
        }
        
        let mut tx = self.pool.begin().await?;
        
        for trade in trades {
            // Insert trade
            self.insert_trade(&mut tx, &trade).await?;
            
            // Insert reserve snapshot
            let snapshot = ReserveSnapshot {
                pool_id: trade.pool_id.clone(),
                reserve0: trade.reserve0_after,
                reserve1: trade.reserve1_after,
                timestamp: trade.timestamp,
                block_height: trade.block_height,
            };
            self.insert_reserve_snapshot(&mut tx, &snapshot).await?;
            
            // Update candles for each interval
            let price = Self::calculate_price(trade.reserve0_after, trade.reserve1_after);
            
            for interval in &self.candle_intervals {
                self.update_candle(&mut tx, &trade, price, interval).await?;
            }
        }
        
        tx.commit().await?;
        
        Ok(())
    }
    
    /// Insert trade into TraceTrade table
    async fn insert_trade(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        trade: &TradeEvent,
    ) -> Result<()> {
        sqlx::query(
            r#"INSERT INTO "TraceTrade"
               (txid, vout, pool_block, pool_tx, token0_block, token0_tx, token1_block, token1_tx,
                amount0_in, amount1_in, amount0_out, amount1_out, reserve0_after, reserve1_after,
                timestamp, block_height)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
               ON CONFLICT (txid, vout, pool_block, pool_tx) DO NOTHING"#
        )
        .bind(&trade.txid)
        .bind(trade.vout)
        .bind(trade.pool_id.block)
        .bind(trade.pool_id.tx)
        .bind(trade.token0_id.block)
        .bind(trade.token0_id.tx)
        .bind(trade.token1_id.block)
        .bind(trade.token1_id.tx)
        .bind(trade.amount0_in.to_string())
        .bind(trade.amount1_in.to_string())
        .bind(trade.amount0_out.to_string())
        .bind(trade.amount1_out.to_string())
        .bind(trade.reserve0_after.to_string())
        .bind(trade.reserve1_after.to_string())
        .bind(trade.timestamp)
        .bind(trade.block_height)
        .execute(&mut **tx)
        .await?;
        
        Ok(())
    }
    
    /// Insert reserve snapshot
    async fn insert_reserve_snapshot(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        snapshot: &ReserveSnapshot,
    ) -> Result<()> {
        sqlx::query(
            r#"INSERT INTO "TraceReserveSnapshot"
               (pool_block, pool_tx, reserve0, reserve1, timestamp, block_height)
               VALUES ($1, $2, $3, $4, $5, $6)"#
        )
        .bind(snapshot.pool_id.block)
        .bind(snapshot.pool_id.tx)
        .bind(snapshot.reserve0.to_string())
        .bind(snapshot.reserve1.to_string())
        .bind(snapshot.timestamp)
        .bind(snapshot.block_height)
        .execute(&mut **tx)
        .await?;
        
        Ok(())
    }
    
    /// Update candle for an interval
    async fn update_candle(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        trade: &TradeEvent,
        price: f64,
        interval: &str,
    ) -> Result<()> {
        let open_time = Self::round_to_interval(trade.timestamp, interval);
        let close_time = open_time + Self::interval_duration(interval);
        
        let volume0 = trade.amount0_in + trade.amount0_out;
        let volume1 = trade.amount1_in + trade.amount1_out;
        
        sqlx::query(
            r#"INSERT INTO "TraceCandle"
               (pool_block, pool_tx, interval, open_time, close_time, open, high, low, close,
                volume0, volume1, trade_count)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10::NUMERIC, $11::NUMERIC, $12)
               ON CONFLICT (pool_block, pool_tx, interval, open_time)
               DO UPDATE SET
                   close = EXCLUDED.close,
                   high = GREATEST("TraceCandle".high, EXCLUDED.high),
                   low = LEAST("TraceCandle".low, EXCLUDED.low),
                   volume0 = "TraceCandle".volume0 + EXCLUDED.volume0,
                   volume1 = "TraceCandle".volume1 + EXCLUDED.volume1,
                   trade_count = "TraceCandle".trade_count + 1"#
        )
        .bind(trade.pool_id.block)
        .bind(trade.pool_id.tx)
        .bind(interval)
        .bind(open_time)
        .bind(close_time)
        .bind(price) // open (will be overwritten on conflict, but that's ok)
        .bind(price) // high
        .bind(price) // low
        .bind(price) // close
        .bind(volume0.to_string())
        .bind(volume1.to_string())
        .bind(1i32) // trade_count
        .execute(&mut **tx)
        .await?;
        
        Ok(())
    }
    
    /// Calculate price from reserves (token1/token0)
    fn calculate_price(reserve0: u128, reserve1: u128) -> f64 {
        if reserve0 == 0 {
            return 0.0;
        }
        (reserve1 as f64) / (reserve0 as f64)
    }
    
    /// Round timestamp down to interval boundary
    fn round_to_interval(timestamp: DateTime<Utc>, interval: &str) -> DateTime<Utc> {
        let seconds = Self::interval_seconds(interval);
        let ts = timestamp.timestamp();
        let rounded = (ts / seconds) * seconds;
        DateTime::from_timestamp(rounded, 0).unwrap_or(timestamp)
    }
    
    /// Get interval duration in seconds
    fn interval_seconds(interval: &str) -> i64 {
        match interval {
            "1m" => 60,
            "5m" => 300,
            "15m" => 900,
            "1h" => 3600,
            "4h" => 14400,
            "1d" => 86400,
            _ => 3600, // default to 1h
        }
    }
    
    /// Get interval duration
    fn interval_duration(interval: &str) -> chrono::Duration {
        chrono::Duration::seconds(Self::interval_seconds(interval))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    #[ignore] // Requires database
    async fn test_optimized_amm_tracker() {
        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://localhost/alkanes_test".to_string());
        
        let pool = PgPool::connect(&database_url).await.unwrap();
        
        // Apply schema
        crate::schema::apply_schema(&pool).await.unwrap();
        
        let tracker = OptimizedAmmTracker::with_intervals(pool.clone(), vec!["1h".to_string()]);
        
        let timestamp = DateTime::from_timestamp(1609459200, 0).unwrap();
        let pool_id = AlkaneId::new(4, 100);
        
        let trades = vec![
            TradeEvent {
                txid: "trade1".to_string(),
                vout: 0,
                pool_id: pool_id.clone(),
                token0_id: AlkaneId::new(4, 10),
                token1_id: AlkaneId::new(4, 20),
                amount0_in: 1000,
                amount1_in: 0,
                amount0_out: 0,
                amount1_out: 900,
                reserve0_after: 101000,
                reserve1_after: 99100,
                timestamp,
                block_height: 100,
            },
        ];
        
        tracker.process_trades(trades).await.unwrap();
        
        // Verify trade was inserted
        let trade_count: i64 = sqlx::query_scalar(
            r#"SELECT COUNT(*) FROM "TraceTrade"
               WHERE pool_block = $1 AND pool_tx = $2"#
        )
        .bind(4)
        .bind(100i64)
        .fetch_one(&pool)
        .await
        .unwrap();
        
        assert_eq!(trade_count, 1);
        
        // Verify candle was created
        let candle_count: i64 = sqlx::query_scalar(
            r#"SELECT COUNT(*) FROM "TraceCandle"
               WHERE pool_block = $1 AND pool_tx = $2 AND interval = $3"#
        )
        .bind(4)
        .bind(100i64)
        .bind("1h")
        .fetch_one(&pool)
        .await
        .unwrap();
        
        assert_eq!(candle_count, 1);
        
        // Clean up
        crate::schema::drop_schema(&pool).await.unwrap();
    }
}
