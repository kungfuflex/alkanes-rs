use actix_web::{web, HttpResponse};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use crate::services::AppState;

#[derive(Debug, Deserialize)]
pub struct GetTradesRequest {
    pub pool: String,
    #[serde(default)]
    pub start_time: Option<i64>,
    #[serde(default)]
    pub end_time: Option<i64>,
    #[serde(default = "default_limit")]
    pub limit: i64,
}

fn default_limit() -> i64 {
    100
}

#[derive(Debug, Serialize)]
pub struct GetTradesResponse {
    pub ok: bool,
    pub pool: String,
    pub trades: Vec<TradeInfo>,
}

#[derive(Debug, Serialize)]
pub struct TradeInfo {
    pub txid: String,
    pub vout: i32,
    pub token0: String,
    pub token1: String,
    pub amount0_in: String,
    pub amount1_in: String,
    pub amount0_out: String,
    pub amount1_out: String,
    pub reserve0_after: String,
    pub reserve1_after: String,
    pub timestamp: String,
    pub block_height: i32,
}

pub async fn get_trades(
    state: web::Data<AppState>,
    req: web::Json<GetTradesRequest>,
) -> HttpResponse {
    let pool = &state.db_pool;
    
    let parts: Vec<&str> = req.pool.split(':').collect();
    if parts.len() != 2 {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "ok": false,
            "error": "invalid_pool_format"
        }));
    }

    let pool_block: i32 = match parts[0].parse() {
        Ok(b) => b,
        Err(_) => {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "ok": false,
                "error": "invalid_block"
            }));
        }
    };

    let pool_tx: i64 = match parts[1].parse() {
        Ok(t) => t,
        Err(_) => {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "ok": false,
                "error": "invalid_tx"
            }));
        }
    };

    let limit = req.limit.min(1000);
    
    // Try new trace transform tables first
    if let Ok(trace_trades) = state.amm_query.get_pool_trades(pool_block, pool_tx, limit).await {
        if !trace_trades.is_empty() {
            log::info!("Using trace transform trades for pool: {}:{}", pool_block, pool_tx);
            let trades: Vec<TradeInfo> = trace_trades.into_iter().map(|t| TradeInfo {
                txid: t.txid,
                vout: t.vout,
                token0: t.token0_id,
                token1: t.token1_id,
                amount0_in: t.amount0_in.to_string(),
                amount1_in: t.amount1_in.to_string(),
                amount0_out: t.amount0_out.to_string(),
                amount1_out: t.amount1_out.to_string(),
                reserve0_after: t.reserve0_after.to_string(),
                reserve1_after: t.reserve1_after.to_string(),
                timestamp: t.timestamp.to_rfc3339(),
                block_height: t.block_height,
            }).collect();
            
            return HttpResponse::Ok().json(GetTradesResponse {
                ok: true,
                pool: req.pool.clone(),
                trades,
            });
        }
    }
    
    // Fall back to legacy AmmTrade table
    log::info!("Using legacy trades for pool: {}:{}", pool_block, pool_tx);

    let query = if req.start_time.is_some() || req.end_time.is_some() {
        let start = req.start_time
            .and_then(|ts| chrono::DateTime::from_timestamp(ts, 0))
            .unwrap_or_else(|| chrono::DateTime::from_timestamp(0, 0).unwrap());
        let end = req.end_time
            .and_then(|ts| chrono::DateTime::from_timestamp(ts, 0))
            .unwrap_or_else(|| chrono::Utc::now());

        sqlx::query_as::<_, (String, i32, i32, i64, i32, i64, String, String, String, String, String, String, String, i32)>(
            r#"
            select "txid", "vout", "token0IdBlock", "token0IdTx", "token1IdBlock", "token1IdTx",
                   "amount0In", "amount1In", "amount0Out", "amount1Out", "reserve0After", "reserve1After",
                   "timestamp"::text, "blockHeight"
            from "AmmTrade"
            where "poolIdBlock" = $1 and "poolIdTx" = $2 
              and "timestamp" >= $3 and "timestamp" <= $4
            order by "timestamp" desc
            limit $5
            "#
        )
        .bind(pool_block)
        .bind(pool_tx)
        .bind(start)
        .bind(end)
        .bind(limit)
    } else {
        sqlx::query_as::<_, (String, i32, i32, i64, i32, i64, String, String, String, String, String, String, String, i32)>(
            r#"
            select "txid", "vout", "token0IdBlock", "token0IdTx", "token1IdBlock", "token1IdTx",
                   "amount0In", "amount1In", "amount0Out", "amount1Out", "reserve0After", "reserve1After",
                   "timestamp"::text, "blockHeight"
            from "AmmTrade"
            where "poolIdBlock" = $1 and "poolIdTx" = $2
            order by "timestamp" desc
            limit $3
            "#
        )
        .bind(pool_block)
        .bind(pool_tx)
        .bind(limit)
    };

    let rows_result = query.fetch_all(pool).await;

    let rows = match rows_result {
        Ok(rows) => rows,
        Err(e) => {
            log::error!("Failed to fetch trades: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "ok": false,
                "error": "internal_error"
            }));
        }
    };

    let trades: Vec<TradeInfo> = rows
        .into_iter()
        .map(|(txid, vout, t0b, t0t, t1b, t1t, a0i, a1i, a0o, a1o, r0, r1, ts, bh)| TradeInfo {
            txid,
            vout,
            token0: format!("{}:{}", t0b, t0t),
            token1: format!("{}:{}", t1b, t1t),
            amount0_in: a0i,
            amount1_in: a1i,
            amount0_out: a0o,
            amount1_out: a1o,
            reserve0_after: r0,
            reserve1_after: r1,
            timestamp: ts,
            block_height: bh,
        })
        .collect();

    HttpResponse::Ok().json(GetTradesResponse {
        ok: true,
        pool: req.pool.clone(),
        trades,
    })
}

#[derive(Debug, Deserialize)]
pub struct GetCandlesRequest {
    pub pool: String,
    pub interval: String,
    #[serde(default)]
    pub start_time: Option<i64>,
    #[serde(default)]
    pub end_time: Option<i64>,
    #[serde(default = "default_candle_limit")]
    pub limit: i64,
}

fn default_candle_limit() -> i64 {
    500
}

#[derive(Debug, Serialize)]
pub struct GetCandlesResponse {
    pub ok: bool,
    pub pool: String,
    pub interval: String,
    pub candles: Vec<CandleInfo>,
}

#[derive(Debug, Serialize)]
pub struct CandleInfo {
    pub open_time: String,
    pub close_time: String,
    pub open: String,
    pub high: String,
    pub low: String,
    pub close: String,
    pub volume0: String,
    pub volume1: String,
    pub trade_count: i32,
}

pub async fn get_candles(
    state: web::Data<AppState>,
    req: web::Json<GetCandlesRequest>,
) -> HttpResponse {
    let pool = &state.db_pool;
    
    let parts: Vec<&str> = req.pool.split(':').collect();
    if parts.len() != 2 {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "ok": false,
            "error": "invalid_pool_format"
        }));
    }

    let pool_block: i32 = match parts[0].parse() {
        Ok(b) => b,
        Err(_) => {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "ok": false,
                "error": "invalid_block"
            }));
        }
    };

    let pool_tx: i64 = match parts[1].parse() {
        Ok(t) => t,
        Err(_) => {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "ok": false,
                "error": "invalid_tx"
            }));
        }
    };

    let limit = req.limit.min(2000);

    let query = if req.start_time.is_some() || req.end_time.is_some() {
        let start = req.start_time
            .and_then(|ts| chrono::DateTime::from_timestamp(ts, 0))
            .unwrap_or_else(|| chrono::DateTime::from_timestamp(0, 0).unwrap());
        let end = req.end_time
            .and_then(|ts| chrono::DateTime::from_timestamp(ts, 0))
            .unwrap_or_else(|| chrono::Utc::now());

        sqlx::query_as::<_, (String, String, String, String, String, String, String, String, i32)>(
            r#"
            select "openTime"::text, "closeTime"::text, "open", "high", "low", "close", 
                   "volume0", "volume1", "tradeCount"
            from "AmmCandle"
            where "poolIdBlock" = $1 and "poolIdTx" = $2 and "interval" = $3
              and "openTime" >= $4 and "openTime" <= $5
            order by "openTime" desc
            limit $6
            "#
        )
        .bind(pool_block)
        .bind(pool_tx)
        .bind(&req.interval)
        .bind(start)
        .bind(end)
        .bind(limit)
    } else {
        sqlx::query_as::<_, (String, String, String, String, String, String, String, String, i32)>(
            r#"
            select "openTime"::text, "closeTime"::text, "open", "high", "low", "close",
                   "volume0", "volume1", "tradeCount"
            from "AmmCandle"
            where "poolIdBlock" = $1 and "poolIdTx" = $2 and "interval" = $3
            order by "openTime" desc
            limit $4
            "#
        )
        .bind(pool_block)
        .bind(pool_tx)
        .bind(&req.interval)
        .bind(limit)
    };

    let rows_result = query.fetch_all(pool).await;

    let rows = match rows_result {
        Ok(rows) => rows,
        Err(e) => {
            log::error!("Failed to fetch candles: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "ok": false,
                "error": "internal_error"
            }));
        }
    };

    let candles: Vec<CandleInfo> = rows
        .into_iter()
        .map(|(ot, ct, o, h, l, c, v0, v1, tc)| CandleInfo {
            open_time: ot,
            close_time: ct,
            open: o,
            high: h,
            low: l,
            close: c,
            volume0: v0,
            volume1: v1,
            trade_count: tc,
        })
        .collect();

    HttpResponse::Ok().json(GetCandlesResponse {
        ok: true,
        pool: req.pool.clone(),
        interval: req.interval.clone(),
        candles,
    })
}

#[derive(Debug, Deserialize)]
pub struct GetReservesRequest {
    pub pool: String,
}

#[derive(Debug, Serialize)]
pub struct GetReservesResponse {
    pub ok: bool,
    pub pool: String,
    pub reserve0: String,
    pub reserve1: String,
    pub timestamp: String,
    pub block_height: i32,
}

pub async fn get_reserves(
    state: web::Data<AppState>,
    req: web::Json<GetReservesRequest>,
) -> HttpResponse {
    let pool = &state.db_pool;
    
    let parts: Vec<&str> = req.pool.split(':').collect();
    if parts.len() != 2 {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "ok": false,
            "error": "invalid_pool_format"
        }));
    }

    let pool_block: i32 = match parts[0].parse() {
        Ok(b) => b,
        Err(_) => {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "ok": false,
                "error": "invalid_block"
            }));
        }
    };

    let pool_tx: i64 = match parts[1].parse() {
        Ok(t) => t,
        Err(_) => {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "ok": false,
                "error": "invalid_tx"
            }));
        }
    };

    let row_result = sqlx::query_as::<_, (String, String, String, i32)>(
        r#"
        select "reserve0", "reserve1", "timestamp"::text, "blockHeight"
        from "AmmReserveSnapshot"
        where "poolIdBlock" = $1 and "poolIdTx" = $2
        order by "timestamp" desc
        limit 1
        "#
    )
    .bind(pool_block)
    .bind(pool_tx)
    .fetch_optional(pool)
    .await;

    match row_result {
        Ok(Some((reserve0, reserve1, timestamp, block_height))) => {
            HttpResponse::Ok().json(GetReservesResponse {
                ok: true,
                pool: req.pool.clone(),
                reserve0,
                reserve1,
                timestamp,
                block_height,
            })
        }
        Ok(None) => HttpResponse::Ok().json(GetReservesResponse {
            ok: true,
            pool: req.pool.clone(),
            reserve0: "0".to_string(),
            reserve1: "0".to_string(),
            timestamp: "".to_string(),
            block_height: 0,
        }),
        Err(e) => {
            log::error!("Failed to fetch reserves: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "ok": false,
                "error": "internal_error"
            }))
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct PathfindRequest {
    pub token_in: String,
    pub token_out: String,
    pub amount_in: String,
    #[serde(default = "default_max_hops")]
    pub max_hops: i32,
}

fn default_max_hops() -> i32 {
    3
}

#[derive(Debug, Serialize)]
pub struct PathfindResponse {
    pub ok: bool,
    pub paths: Vec<PathInfo>,
}

#[derive(Debug, Serialize)]
pub struct PathInfo {
    pub hops: Vec<String>,
    pub pools: Vec<String>,
    pub estimated_output: String,
}

pub async fn pathfind(
    state: web::Data<AppState>,
    req: web::Json<PathfindRequest>,
) -> HttpResponse {
    let pool = &state.db_pool;
    
    // Parse token IDs
    let token_in_parts: Vec<&str> = req.token_in.split(':').collect();
    let token_out_parts: Vec<&str> = req.token_out.split(':').collect();
    
    if token_in_parts.len() != 2 || token_out_parts.len() != 2 {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "ok": false,
            "error": "invalid_token_format"
        }));
    }

    // Basic pathfinding: Find direct pools connecting these tokens
    // Query all pools that have reserves with these tokens
    let pools_query = sqlx::query_as::<_, (i32, i64, String, String)>(
        r#"
        select distinct "poolIdBlock", "poolIdTx", "reserve0", "reserve1"
        from "AmmReserveSnapshot" AS r1
        where exists (
            select 1 from "AmmReserveSnapshot" AS r2
            where r1."poolIdBlock" = r2."poolIdBlock" 
              and r1."poolIdTx" = r2."poolIdTx"
        )
        order by "poolIdBlock" desc, "poolIdTx" desc
        limit 100
        "#
    )
    .fetch_all(pool)
    .await;

    let pools = match pools_query {
        Ok(p) => p,
        Err(e) => {
            log::error!("Failed to query pools for pathfinding: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "ok": false,
                "error": "internal_error"
            }));
        }
    };

    // Simple direct path check
    let mut paths = Vec::new();
    
    for (pool_block, pool_tx, reserve0, reserve1) in pools {
        let pool_id = format!("{}:{}", pool_block, pool_tx);
        
        // Check if this pool connects our tokens (simplified - would need token pair info)
        if !reserve0.is_empty() && !reserve1.is_empty() {
            // Rough estimation: constant product formula
            let r0: f64 = reserve0.parse().unwrap_or(0.0);
            let r1: f64 = reserve1.parse().unwrap_or(0.0);
            let amount_in: f64 = req.amount_in.parse().unwrap_or(0.0);
            
            if r0 > 0.0 && r1 > 0.0 {
                let amount_out = (amount_in * r1) / (r0 + amount_in);
                
                paths.push(PathInfo {
                    hops: vec![req.token_in.clone(), req.token_out.clone()],
                    pools: vec![pool_id],
                    estimated_output: amount_out.to_string(),
                });
            }
        }
        
        if paths.len() >= req.max_hops as usize {
            break;
        }
    }

    HttpResponse::Ok().json(PathfindResponse {
        ok: true,
        paths,
    })
}
