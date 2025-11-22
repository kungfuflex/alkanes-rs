use anyhow::Result;
use chrono::{DateTime, Utc};
use serde_json::Value as JsonValue;
use sqlx::PgPool;
use std::collections::HashMap;
use tracing::debug;

#[cfg(test)]
mod tests;

#[derive(Debug, Clone)]
pub struct TradeEvent {
    pub txid: String,
    pub vout: i32,
    pub pool_id_block: i32,
    pub pool_id_tx: i64,
    pub token0_id_block: i32,
    pub token0_id_tx: i64,
    pub token1_id_block: i32,
    pub token1_id_tx: i64,
    pub amount0_in: String,
    pub amount1_in: String,
    pub amount0_out: String,
    pub amount1_out: String,
    pub reserve0_after: String,
    pub reserve1_after: String,
    pub timestamp: DateTime<Utc>,
    pub block_height: i32,
}

#[derive(Debug, Clone)]
pub struct ReserveSnapshot {
    pub pool_id_block: i32,
    pub pool_id_tx: i64,
    pub reserve0: String,
    pub reserve1: String,
    pub timestamp: DateTime<Utc>,
    pub block_height: i32,
}

/// Extract trade events from trace events
/// Looks for ReceiveIntent + ValueTransfer patterns that indicate swaps
pub fn extract_trade_events(
    tx: &JsonValue,
    trace_events: &[super::protostone::TraceEventItem],
    timestamp: DateTime<Utc>,
    block_height: i32,
) -> Result<Vec<TradeEvent>> {
    let mut trades = Vec::new();
    
    let txid = tx.get("txid")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing txid"))?
        .to_string();
    
    // Group events by vout
    let mut events_by_vout: HashMap<i32, Vec<&super::protostone::TraceEventItem>> = HashMap::new();
    for event in trace_events {
        events_by_vout.entry(event.vout).or_default().push(event);
    }
    
    // Process each vout's events
    for (vout, events) in events_by_vout {
        let mut receive_intent: Option<&JsonValue> = None;
        let mut value_transfers: Vec<&JsonValue> = Vec::new();
        let mut pool_id: Option<(i32, i64)> = None;
        
        for event in events {
            match event.event_type.as_str() {
                "receive_intent" => {
                    receive_intent = Some(&event.data);
                    pool_id = Some((
                        event.alkane_address_block.parse().unwrap_or(0),
                        event.alkane_address_tx.parse().unwrap_or(0),
                    ));
                }
                "value_transfer" => {
                    value_transfers.push(&event.data);
                }
                _ => {}
            }
        }
        
        // If we have both receive_intent and value_transfer, this is a swap
        if let (Some(intent), Some((pool_block, pool_tx))) = (receive_intent, pool_id) {
            if !value_transfers.is_empty() {
                let trade = parse_trade_from_events(
                    &txid,
                    vout,
                    pool_block,
                    pool_tx,
                    intent,
                    &value_transfers,
                    timestamp,
                    block_height,
                );
                
                if let Ok(t) = trade {
                    trades.push(t);
                }
            }
        }
    }
    
    Ok(trades)
}

fn parse_trade_from_events(
    txid: &str,
    vout: i32,
    pool_block: i32,
    pool_tx: i64,
    intent: &JsonValue,
    transfers: &[&JsonValue],
    timestamp: DateTime<Utc>,
    block_height: i32,
) -> Result<TradeEvent> {
    // Extract input amounts from receive_intent
    let inputs = intent.get("inputs")
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow::anyhow!("Missing inputs"))?;
    
    let mut amount0_in = "0".to_string();
    let mut amount1_in = "0".to_string();
    let mut token0_id: Option<(i32, i64)> = None;
    let mut token1_id: Option<(i32, i64)> = None;
    
    for input in inputs {
        let alkane_id = input.get("id").or_else(|| input.get("alkaneId"));
        if let Some(id) = alkane_id {
            let block = id.get("block").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let tx = id.get("tx").and_then(|v| v.as_i64()).unwrap_or(0);
            let amount = input.get("amount")
                .or_else(|| input.get("value"))
                .and_then(|v| v.as_str().map(|s| s.to_string())
                    .or_else(|| v.as_u64().map(|n| n.to_string())))
                .unwrap_or_else(|| "0".to_string());
            
            if token0_id.is_none() {
                token0_id = Some((block, tx));
                amount0_in = amount;
            } else if token1_id.is_none() {
                token1_id = Some((block, tx));
                amount1_in = amount;
            }
        }
    }
    
    // Extract output amounts from value_transfers
    let mut amount0_out = "0".to_string();
    let mut amount1_out = "0".to_string();
    
    for transfer_data in transfers {
        if let Some(transfers_arr) = transfer_data.get("transfers").and_then(|v| v.as_array()) {
            for transfer in transfers_arr {
                let alkane_id = transfer.get("id").or_else(|| transfer.get("alkaneId"));
                if let Some(id) = alkane_id {
                    let block = id.get("block").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                    let tx = id.get("tx").and_then(|v| v.as_i64()).unwrap_or(0);
                    let amount = transfer.get("amount")
                        .or_else(|| transfer.get("value"))
                        .and_then(|v| v.as_str().map(|s| s.to_string())
                            .or_else(|| v.as_u64().map(|n| n.to_string())))
                        .unwrap_or_else(|| "0".to_string());
                    
                    if token0_id == Some((block, tx)) {
                        amount0_out = amount;
                    } else if token1_id == Some((block, tx)) {
                        amount1_out = amount;
                    } else if token0_id.is_none() {
                        token0_id = Some((block, tx));
                        amount0_out = amount;
                    } else if token1_id.is_none() {
                        token1_id = Some((block, tx));
                        amount1_out = amount;
                    }
                }
            }
        }
    }
    
    let (token0_block, token0_tx) = token0_id.unwrap_or((0, 0));
    let (token1_block, token1_tx) = token1_id.unwrap_or((0, 0));
    
    // For now, we don't have live reserve tracking, set to "0"
    // These will be populated by reserve extraction or storage queries
    Ok(TradeEvent {
        txid: txid.to_string(),
        vout,
        pool_id_block: pool_block,
        pool_id_tx: pool_tx,
        token0_id_block: token0_block,
        token0_id_tx: token0_tx,
        token1_id_block: token1_block,
        token1_id_tx: token1_tx,
        amount0_in,
        amount1_in,
        amount0_out,
        amount1_out,
        reserve0_after: "0".to_string(),
        reserve1_after: "0".to_string(),
        timestamp,
        block_height,
    })
}

/// Extract reserve snapshots from storage
pub fn extract_reserves_from_storage(
    storage_changes: &[super::storage_tracker::StorageChange],
    timestamp: DateTime<Utc>,
    block_height: i32,
) -> Vec<ReserveSnapshot> {
    let mut reserves: HashMap<(i32, i64), (String, String)> = HashMap::new();
    
    for change in storage_changes {
        let key = change.key.as_str();
        
        // Look for reserve keys (common patterns: "reserve0", "reserve1", "reserves", etc.)
        if key.contains("reserve") {
            let pool_key = (change.alkane_id_block, change.alkane_id_tx);
            let entry = reserves.entry(pool_key).or_insert_with(|| ("0".to_string(), "0".to_string()));
            
            if key.contains("0") || key == "reserve0" {
                entry.0 = change.value.clone();
            } else if key.contains("1") || key == "reserve1" {
                entry.1 = change.value.clone();
            }
        }
    }
    
    reserves.into_iter()
        .map(|((block, tx), (reserve0, reserve1))| ReserveSnapshot {
            pool_id_block: block,
            pool_id_tx: tx,
            reserve0,
            reserve1,
            timestamp,
            block_height,
        })
        .collect()
}

/// Insert trade events into database
pub async fn insert_trade_events(
    pool: &PgPool,
    trades: &[TradeEvent],
) -> Result<()> {
    if trades.is_empty() {
        return Ok(());
    }
    
    let mut dbtx = pool.begin().await?;
    
    for trade in trades {
        sqlx::query(
            r#"
            insert into "AmmTrade"
            ("txid", "vout", "poolIdBlock", "poolIdTx", "token0IdBlock", "token0IdTx", 
             "token1IdBlock", "token1IdTx", "amount0In", "amount1In", "amount0Out", "amount1Out",
             "reserve0After", "reserve1After", "timestamp", "blockHeight")
            values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
            "#
        )
        .bind(&trade.txid)
        .bind(trade.vout)
        .bind(trade.pool_id_block)
        .bind(trade.pool_id_tx)
        .bind(trade.token0_id_block)
        .bind(trade.token0_id_tx)
        .bind(trade.token1_id_block)
        .bind(trade.token1_id_tx)
        .bind(&trade.amount0_in)
        .bind(&trade.amount1_in)
        .bind(&trade.amount0_out)
        .bind(&trade.amount1_out)
        .bind(&trade.reserve0_after)
        .bind(&trade.reserve1_after)
        .bind(trade.timestamp)
        .bind(trade.block_height)
        .execute(&mut *dbtx)
        .await?;
    }
    
    dbtx.commit().await?;
    debug!("Inserted {} trade events", trades.len());
    Ok(())
}

/// Insert reserve snapshots
pub async fn insert_reserve_snapshots(
    pool: &PgPool,
    reserves: &[ReserveSnapshot],
) -> Result<()> {
    if reserves.is_empty() {
        return Ok(());
    }
    
    let mut dbtx = pool.begin().await?;
    
    for reserve in reserves {
        sqlx::query(
            r#"
            insert into "AmmReserveSnapshot"
            ("poolIdBlock", "poolIdTx", "reserve0", "reserve1", "timestamp", "blockHeight")
            values ($1, $2, $3, $4, $5, $6)
            "#
        )
        .bind(reserve.pool_id_block)
        .bind(reserve.pool_id_tx)
        .bind(&reserve.reserve0)
        .bind(&reserve.reserve1)
        .bind(reserve.timestamp)
        .bind(reserve.block_height)
        .execute(&mut *dbtx)
        .await?;
    }
    
    dbtx.commit().await?;
    debug!("Inserted {} reserve snapshots", reserves.len());
    Ok(())
}

/// Aggregate trades into OHLCV candles
pub async fn aggregate_candles(
    pool: &PgPool,
    start_time: DateTime<Utc>,
    end_time: DateTime<Utc>,
) -> Result<()> {
    // 1-minute candles
    sqlx::query(
        r#"
        insert into "AmmCandle" ("poolIdBlock", "poolIdTx", "interval", "openTime", "closeTime", 
                                  "open", "high", "low", "close", "volume0", "volume1", "tradeCount")
        select 
            "poolIdBlock", "poolIdTx",
            '1m' as interval,
            date_trunc('minute', "timestamp") as "openTime",
            date_trunc('minute', "timestamp") + interval '1 minute' as "closeTime",
            (array_agg("price" order by "timestamp" asc))[1] as "open",
            max("price") as "high",
            min("price") as "low",
            (array_agg("price" order by "timestamp" desc))[1] as "close",
            sum(("amount0In"::numeric + "amount0Out"::numeric)) as "volume0",
            sum(("amount1In"::numeric + "amount1Out"::numeric)) as "volume1",
            count(*) as "tradeCount"
        from (
            select *,
                case 
                    when "amount1In"::numeric > 0 then "amount1In"::numeric / nullif("amount0Out"::numeric, 0)
                    when "amount0In"::numeric > 0 then "amount1Out"::numeric / nullif("amount0In"::numeric, 0)
                    else 0
                end as "price"
            from "AmmTrade"
            where "timestamp" >= $1 and "timestamp" < $2
        ) t
        group by "poolIdBlock", "poolIdTx", date_trunc('minute', "timestamp")
        on conflict ("poolIdBlock", "poolIdTx", "interval", "openTime") do update
        set "close" = excluded."close",
            "high" = greatest("AmmCandle"."high", excluded."high"),
            "low" = least("AmmCandle"."low", excluded."low"),
            "volume0" = excluded."volume0",
            "volume1" = excluded."volume1",
            "tradeCount" = excluded."tradeCount"
        "#
    )
    .bind(start_time)
    .bind(end_time)
    .execute(pool)
    .await?;
    
    debug!("Aggregated candles for period {:?} to {:?}", start_time, end_time);
    Ok(())
}
