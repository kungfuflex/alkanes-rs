use crate::backend::StorageBackend;
use crate::extractor::TraceExtractor;
use crate::tracker::StateTracker;
use crate::types::{AlkaneId, TraceEvent, TransactionContext, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Trade event extracted from traces
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeEvent {
    pub txid: String,
    pub vout: i32,
    pub pool_id: AlkaneId,
    pub token0_id: AlkaneId,
    pub token1_id: AlkaneId,
    pub amount0_in: u128,
    pub amount1_in: u128,
    pub amount0_out: u128,
    pub amount1_out: u128,
    pub reserve0_after: u128,
    pub reserve1_after: u128,
    pub timestamp: DateTime<Utc>,
    pub block_height: i32,
}

/// Reserve snapshot at a point in time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReserveSnapshot {
    pub pool_id: AlkaneId,
    pub reserve0: u128,
    pub reserve1: u128,
    pub timestamp: DateTime<Utc>,
    pub block_height: i32,
}

/// OHLCV candle for a time interval
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candle {
    pub pool_id: AlkaneId,
    pub interval: String, // "1m", "5m", "1h", "1d", etc.
    pub open_time: DateTime<Utc>,
    pub close_time: DateTime<Utc>,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume0: u128,
    pub volume1: u128,
    pub trade_count: u32,
}

/// Extracts trade events from receive_intent + value_transfer patterns
pub struct TradeEventExtractor {
    pub context: Option<TransactionContext>,
}

impl TradeEventExtractor {
    pub fn new() -> Self {
        Self { context: None }
    }
    
    pub fn with_context(context: TransactionContext) -> Self {
        Self { context: Some(context) }
    }
    
    /// Parse trade from receive_intent and value_transfer events
    fn parse_trade(&self, receive_intent: &serde_json::Value, value_transfers: &[&serde_json::Value], vout: i32, pool_id: AlkaneId) -> Option<TradeEvent> {
        let context = self.context.as_ref()?;
        
        // Extract input amounts from receive_intent
        let inputs = receive_intent.get("inputs")?.as_array()?;
        
        let mut amount0_in = 0u128;
        let mut amount1_in = 0u128;
        let mut token0_id: Option<AlkaneId> = None;
        let mut token1_id: Option<AlkaneId> = None;
        
        for (i, input) in inputs.iter().enumerate() {
            let id_obj = input.get("id").or_else(|| input.get("alkaneId"))?;
            let block = id_obj.get("block")?.as_i64()? as i32;
            let tx = id_obj.get("tx")?.as_i64()?;
            let amount_str = input.get("amount").or_else(|| input.get("value"))?;
            let amount = amount_str.as_str()?.parse::<u128>().ok()
                .or_else(|| amount_str.as_u64().map(|n| n as u128))?;
            
            let alkane_id = AlkaneId::new(block, tx);
            
            if i == 0 {
                token0_id = Some(alkane_id);
                amount0_in = amount;
            } else if i == 1 {
                token1_id = Some(alkane_id);
                amount1_in = amount;
            }
        }
        
        // Extract output amounts from value_transfers
        let mut amount0_out = 0u128;
        let mut amount1_out = 0u128;
        
        for transfer_data in value_transfers {
            let transfers = transfer_data.get("transfers")?.as_array()?;
            
            for transfer in transfers {
                let id_obj = transfer.get("id").or_else(|| transfer.get("alkaneId"))?;
                let block = id_obj.get("block")?.as_i64()? as i32;
                let tx = id_obj.get("tx")?.as_i64()?;
                let amount_str = transfer.get("amount").or_else(|| transfer.get("value"))?;
                let amount = amount_str.as_str()?.parse::<u128>().ok()
                    .or_else(|| amount_str.as_u64().map(|n| n as u128))?;
                
                let alkane_id = AlkaneId::new(block, tx);
                
                if Some(&alkane_id) == token0_id.as_ref() {
                    amount0_out += amount;
                } else if Some(&alkane_id) == token1_id.as_ref() {
                    amount1_out += amount;
                }
            }
        }
        
        // Calculate reserves after trade (simplified - in reality would need more context)
        let reserve0_after = amount0_in.saturating_sub(amount0_out);
        let reserve1_after = amount1_in.saturating_sub(amount1_out);
        
        Some(TradeEvent {
            txid: context.txid.clone(),
            vout,
            pool_id,
            token0_id: token0_id?,
            token1_id: token1_id?,
            amount0_in,
            amount1_in,
            amount0_out,
            amount1_out,
            reserve0_after,
            reserve1_after,
            timestamp: context.timestamp,
            block_height: context.block_height,
        })
    }
}

impl Default for TradeEventExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl TraceExtractor for TradeEventExtractor {
    type Output = Vec<TradeEvent>;
    
    fn extract(&self, _trace: &TraceEvent) -> Result<Option<Vec<TradeEvent>>> {
        // NOTE: This extractor needs to see ALL traces from a transaction
        // and correlate receive_intent with value_transfer events.
        // For now, return None - this would need a batch extraction API
        Ok(None)
    }
    
    fn name(&self) -> &'static str {
        "trade_event_extractor"
    }
}

/// Tracks AMM trades, reserves, and candles
pub struct AmmTracker {
    candle_intervals: Vec<String>,
}

impl AmmTracker {
    pub fn new() -> Self {
        Self {
            candle_intervals: vec!["1m".to_string(), "5m".to_string(), "1h".to_string(), "1d".to_string()],
        }
    }
    
    pub fn with_intervals(intervals: Vec<String>) -> Self {
        Self {
            candle_intervals: intervals,
        }
    }
    
    /// Encode key for trade: "trade:{pool_id}:{timestamp}:{txid}:{vout}"
    fn trade_key(pool_id: &AlkaneId, timestamp: &DateTime<Utc>, txid: &str, vout: i32) -> Vec<u8> {
        format!("trade:{}:{}:{}:{}", pool_id.to_string(), timestamp.timestamp(), txid, vout).into_bytes()
    }
    
    /// Encode key for reserve snapshot: "reserve:{pool_id}:{timestamp}"
    fn reserve_key(pool_id: &AlkaneId, timestamp: &DateTime<Utc>) -> Vec<u8> {
        format!("reserve:{}:{}", pool_id.to_string(), timestamp.timestamp()).into_bytes()
    }
    
    /// Encode key for candle: "candle:{pool_id}:{interval}:{open_time}"
    fn candle_key(pool_id: &AlkaneId, interval: &str, open_time: &DateTime<Utc>) -> Vec<u8> {
        format!("candle:{}:{}:{}", pool_id.to_string(), interval, open_time.timestamp()).into_bytes()
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
        let seconds = match interval {
            "1m" => 60,
            "5m" => 300,
            "15m" => 900,
            "1h" => 3600,
            "4h" => 14400,
            "1d" => 86400,
            _ => 3600, // default to 1h
        };
        
        let ts = timestamp.timestamp();
        let rounded = (ts / seconds) * seconds;
        DateTime::from_timestamp(rounded, 0).unwrap_or(timestamp)
    }
}

impl Default for AmmTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl StateTracker for AmmTracker {
    type Input = Vec<TradeEvent>;
    
    fn name(&self) -> &'static str {
        "trade_event_extractor"
    }
    
    fn update<B: StorageBackend>(&mut self, backend: &mut B, trades: Vec<TradeEvent>) -> Result<()> {
        for trade in trades {
            let trade_key = Self::trade_key(&trade.pool_id, &trade.timestamp, &trade.txid, trade.vout);
            let reserve_key = Self::reserve_key(&trade.pool_id, &trade.timestamp);
            
            // Store trade
            let trade_bytes = serde_json::to_vec(&trade)?;
            backend.set("trades", &trade_key, &trade_bytes)?;
            
            // Store reserve snapshot
            let snapshot = ReserveSnapshot {
                pool_id: trade.pool_id.clone(),
                reserve0: trade.reserve0_after,
                reserve1: trade.reserve1_after,
                timestamp: trade.timestamp,
                block_height: trade.block_height,
            };
            let snapshot_bytes = serde_json::to_vec(&snapshot)?;
            backend.set("reserves", &reserve_key, &snapshot_bytes)?;
            
            // Update candles for each interval
            let price = Self::calculate_price(trade.reserve0_after, trade.reserve1_after);
            
            for interval in &self.candle_intervals {
                let open_time = Self::round_to_interval(trade.timestamp, interval);
                let candle_key = Self::candle_key(&trade.pool_id, interval, &open_time);
                
                // Get or create candle
                let mut candle = backend.get("candles", &candle_key)?
                    .and_then(|bytes| serde_json::from_slice::<Candle>(&bytes).ok())
                    .unwrap_or_else(|| {
                        let close_time = Self::round_to_interval(
                            trade.timestamp + chrono::Duration::seconds(
                                match interval.as_str() {
                                    "1m" => 60,
                                    "5m" => 300,
                                    "15m" => 900,
                                    "1h" => 3600,
                                    "4h" => 14400,
                                    "1d" => 86400,
                                    _ => 3600,
                                }
                            ),
                            interval
                        );
                        
                        Candle {
                            pool_id: trade.pool_id.clone(),
                            interval: interval.clone(),
                            open_time,
                            close_time,
                            open: price,
                            high: price,
                            low: price,
                            close: price,
                            volume0: 0,
                            volume1: 0,
                            trade_count: 0,
                        }
                    });
                
                // Update candle
                candle.high = candle.high.max(price);
                candle.low = candle.low.min(price);
                candle.close = price;
                candle.volume0 += trade.amount0_in + trade.amount0_out;
                candle.volume1 += trade.amount1_in + trade.amount1_out;
                candle.trade_count += 1;
                
                let candle_bytes = serde_json::to_vec(&candle)?;
                backend.set("candles", &candle_key, &candle_bytes)?;
            }
        }
        
        Ok(())
    }
    
    fn reset<B: StorageBackend>(&mut self, backend: &mut B) -> Result<()> {
        for table in ["trades", "reserves", "candles"] {
            for (k, _) in backend.scan(table)? {
                backend.delete(table, &k)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::InMemoryBackend;
    
    #[test]
    fn test_amm_trade_tracking() {
        let mut backend = InMemoryBackend::new();
        let mut tracker = AmmTracker::new();
        
        let timestamp = Utc::now();
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
                reserve0_after: 10000,
                reserve1_after: 9000,
                timestamp,
                block_height: 100,
            },
        ];
        
        tracker.update(&mut backend, trades).unwrap();
        
        // Verify trade was stored
        let trade_key = AmmTracker::trade_key(&pool_id, &timestamp, "trade1", 0);
        let trade_bytes = backend.get("trades", &trade_key).unwrap().unwrap();
        let stored_trade: TradeEvent = serde_json::from_slice(&trade_bytes).unwrap();
        assert_eq!(stored_trade.amount0_in, 1000);
        
        // Verify reserve snapshot
        let reserve_key = AmmTracker::reserve_key(&pool_id, &timestamp);
        let reserve_bytes = backend.get("reserves", &reserve_key).unwrap().unwrap();
        let snapshot: ReserveSnapshot = serde_json::from_slice(&reserve_bytes).unwrap();
        assert_eq!(snapshot.reserve0, 10000);
        assert_eq!(snapshot.reserve1, 9000);
    }
    
    #[test]
    fn test_candle_aggregation() {
        let mut backend = InMemoryBackend::new();
        let mut tracker = AmmTracker::with_intervals(vec!["1h".to_string()]);
        
        let base_time = DateTime::from_timestamp(1609459200, 0).unwrap(); // 2021-01-01 00:00:00
        let pool_id = AlkaneId::new(4, 100);
        
        // Two trades in the same hour
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
                reserve0_after: 10000,
                reserve1_after: 9000,
                timestamp: base_time,
                block_height: 100,
            },
            TradeEvent {
                txid: "trade2".to_string(),
                vout: 0,
                pool_id: pool_id.clone(),
                token0_id: AlkaneId::new(4, 10),
                token1_id: AlkaneId::new(4, 20),
                amount0_in: 500,
                amount1_in: 0,
                amount0_out: 0,
                amount1_out: 450,
                reserve0_after: 10500,
                reserve1_after: 8550,
                timestamp: base_time + chrono::Duration::minutes(30),
                block_height: 100,
            },
        ];
        
        tracker.update(&mut backend, trades).unwrap();
        
        // Verify candle aggregation
        let open_time = AmmTracker::round_to_interval(base_time, "1h");
        let candle_key = AmmTracker::candle_key(&pool_id, "1h", &open_time);
        let candle_bytes = backend.get("candles", &candle_key).unwrap().unwrap();
        let candle: Candle = serde_json::from_slice(&candle_bytes).unwrap();
        
        assert_eq!(candle.trade_count, 2);
        assert_eq!(candle.volume0, 1500); // 1000 + 500
        assert!(candle.high >= candle.low);
    }
}
