use actix_web::{web, HttpResponse};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::services::AppState;

#[derive(Debug, Deserialize)]
pub struct AddressBalancesRequest {
    pub address: String,
    #[serde(default)]
    pub include_outpoints: bool,
}

#[derive(Debug, Serialize)]
pub struct AddressBalancesResponse {
    pub ok: bool,
    pub address: String,
    pub balances: HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outpoints: Option<Vec<OutpointInfo>>,
}

#[derive(Debug, Serialize)]
pub struct OutpointInfo {
    pub outpoint: String,
    pub entries: Vec<BalanceEntry>,
}

#[derive(Debug, Serialize)]
pub struct BalanceEntry {
    pub alkane: String,
    pub amount: String,
}

pub async fn get_address_balances(
    state: web::Data<AppState>,
    req: web::Json<AddressBalancesRequest>,
) -> HttpResponse {
    let pool = &state.db_pool;
    
    // Try new trace transform tables first, fall back to old tables
    let balances: HashMap<String, String> = match state.balance_query.get_address_balances(&req.address).await {
        Ok(trace_balances) if !trace_balances.is_empty() => {
            log::info!("Using trace transform balances for address: {}", req.address);
            trace_balances.into_iter()
                .map(|b| (b.alkane_id, b.amount.to_string()))
                .collect()
        },
        Ok(_) | Err(_) => {
            // Fall back to old AlkaneBalance table
            log::info!("Using legacy balances for address: {}", req.address);
            let balances_result = sqlx::query_as::<_, (i32, i64, String)>(
                r#"select "alkaneIdBlock", "alkaneIdTx", "amount" from "AlkaneBalance" where "address" = $1"#
            )
            .bind(&req.address)
            .fetch_all(pool)
            .await;

            match balances_result {
                Ok(rows) => {
                    rows.into_iter()
                        .map(|(block, tx, amount)| (format!("{}:{}", block, tx), amount))
                        .collect()
                },
                Err(e) => {
                    log::error!("Failed to fetch address balances: {}", e);
                    return HttpResponse::InternalServerError().json(serde_json::json!({
                        "ok": false,
                        "error": "internal_error"
                    }));
                }
            }
        }
    };

    let outpoints = if req.include_outpoints {
        let utxo_result = sqlx::query_as::<_, (String, i32, i32, i64, String)>(
            r#"select "outpointTxid", "outpointVout", "alkaneIdBlock", "alkaneIdTx", "amount" 
               from "AlkaneBalanceUtxo" 
               where "address" = $1 and not "spent" 
               order by "outpointTxid", "outpointVout""#
        )
        .bind(&req.address)
        .fetch_all(pool)
        .await;

        match utxo_result {
            Ok(rows) => {
                let mut outpoint_map: HashMap<String, Vec<BalanceEntry>> = HashMap::new();
                for (txid, vout, block, tx, amount) in rows {
                    let outpoint = format!("{}:{}", txid, vout);
                    outpoint_map.entry(outpoint).or_default().push(BalanceEntry {
                        alkane: format!("{}:{}", block, tx),
                        amount,
                    });
                }

                Some(
                    outpoint_map
                        .into_iter()
                        .map(|(outpoint, entries)| OutpointInfo { outpoint, entries })
                        .collect(),
                )
            }
            Err(e) => {
                log::error!("Failed to fetch UTXO balances: {}", e);
                None
            }
        }
    } else {
        None
    };

    HttpResponse::Ok().json(AddressBalancesResponse {
        ok: true,
        address: req.address.clone(),
        balances,
        outpoints,
    })
}

#[derive(Debug, Deserialize)]
pub struct OutpointBalancesRequest {
    pub outpoint: String,
}

#[derive(Debug, Serialize)]
pub struct OutpointBalancesResponse {
    pub ok: bool,
    pub outpoint: String,
    pub items: Vec<OutpointItem>,
}

#[derive(Debug, Serialize)]
pub struct OutpointItem {
    pub outpoint: String,
    pub address: Option<String>,
    pub entries: Vec<BalanceEntry>,
}

pub async fn get_outpoint_balances(
    state: web::Data<AppState>,
    req: web::Json<OutpointBalancesRequest>,
) -> HttpResponse {
    let pool = &state.db_pool;
    
    let parts: Vec<&str> = req.outpoint.split(':').collect();
    if parts.len() != 2 {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "ok": false,
            "error": "invalid_outpoint_format"
        }));
    }

    let txid = parts[0];
    let vout: i32 = match parts[1].parse() {
        Ok(v) => v,
        Err(_) => {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "ok": false,
                "error": "invalid_vout"
            }));
        }
    };

    let entries_result = sqlx::query_as::<_, (String, i32, i64, String)>(
        r#"select "address", "alkaneIdBlock", "alkaneIdTx", "amount" 
           from "AlkaneBalanceUtxo" 
           where "outpointTxid" = $1 and "outpointVout" = $2"#
    )
    .bind(txid)
    .bind(vout)
    .fetch_all(pool)
    .await;

    let rows = match entries_result {
        Ok(rows) => rows,
        Err(e) => {
            log::error!("Failed to fetch outpoint balances: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "ok": false,
                "error": "internal_error"
            }));
        }
    };

    if rows.is_empty() {
        return HttpResponse::Ok().json(OutpointBalancesResponse {
            ok: true,
            outpoint: req.outpoint.clone(),
            items: vec![],
        });
    }

    let address = rows.first().map(|(addr, _, _, _)| addr.clone());
    let entries: Vec<BalanceEntry> = rows
        .into_iter()
        .map(|(_, block, tx, amount)| BalanceEntry {
            alkane: format!("{}:{}", block, tx),
            amount,
        })
        .collect();

    HttpResponse::Ok().json(OutpointBalancesResponse {
        ok: true,
        outpoint: req.outpoint.clone(),
        items: vec![OutpointItem {
            outpoint: req.outpoint.clone(),
            address,
            entries,
        }],
    })
}

#[derive(Debug, Deserialize)]
pub struct HoldersRequest {
    pub alkane: String,
    #[serde(default = "default_page")]
    pub page: i64,
    #[serde(default = "default_limit")]
    pub limit: i64,
}

fn default_page() -> i64 {
    1
}

fn default_limit() -> i64 {
    100
}

#[derive(Debug, Serialize)]
pub struct HoldersResponse {
    pub ok: bool,
    pub alkane: String,
    pub page: i64,
    pub limit: i64,
    pub total: i64,
    pub has_more: bool,
    pub items: Vec<HolderInfo>,
}

#[derive(Debug, Serialize)]
pub struct HolderInfo {
    pub address: String,
    pub amount: String,
}

pub async fn get_holders(
    state: web::Data<AppState>,
    req: web::Json<HoldersRequest>,
) -> HttpResponse {
    let pool = &state.db_pool;
    
    let parts: Vec<&str> = req.alkane.split(':').collect();
    if parts.len() != 2 {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "ok": false,
            "error": "invalid_alkane_format"
        }));
    }

    let block: i32 = match parts[0].parse() {
        Ok(b) => b,
        Err(_) => {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "ok": false,
                "error": "invalid_block"
            }));
        }
    };

    let tx: i64 = match parts[1].parse() {
        Ok(t) => t,
        Err(_) => {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "ok": false,
                "error": "invalid_tx"
            }));
        }
    };

    let limit = req.limit.min(1000);
    let offset = (req.page - 1) * limit;

    // Try TraceHolder first (trace transform tables), then fallback to AlkaneHolder
    let count_result = sqlx::query_as::<_, (i64,)>(
        r#"select count(*) from "TraceHolder" where alkane_block = $1 and alkane_tx = $2 and total_amount > 0"#
    )
    .bind(block)
    .bind(tx)
    .fetch_one(pool)
    .await;

    let (total, use_trace_table) = match count_result {
        Ok((count,)) if count > 0 => {
            log::info!("Using TraceHolder table for holders");
            (count, true)
        },
        _ => {
            // Fallback to AlkaneHolder
            log::info!("Using AlkaneHolder table for holders");
            let count_result = sqlx::query_as::<_, (i64,)>(
                r#"select count(*) from "AlkaneHolder" where "alkaneIdBlock" = $1 and "alkaneIdTx" = $2"#
            )
            .bind(block)
            .bind(tx)
            .fetch_one(pool)
            .await;
            match count_result {
                Ok((count,)) => (count, false),
                Err(e) => {
                    log::error!("Failed to count holders: {}", e);
                    return HttpResponse::InternalServerError().json(serde_json::json!({
                        "ok": false,
                        "error": "internal_error"
                    }));
                }
            }
        }
    };

    let rows: Vec<(String, String)> = if use_trace_table {
        let holders_result = sqlx::query_as::<_, (String, String)>(
            r#"select address, total_amount::TEXT
               from "TraceHolder"
               where alkane_block = $1 and alkane_tx = $2 and total_amount > 0
               order by total_amount desc
               limit $3 offset $4"#
        )
        .bind(block)
        .bind(tx)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await;

        match holders_result {
            Ok(rows) => rows,
            Err(e) => {
                log::error!("Failed to fetch holders from TraceHolder: {}", e);
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "ok": false,
                    "error": "internal_error"
                }));
            }
        }
    } else {
        let holders_result = sqlx::query_as::<_, (String, String)>(
            r#"select "address", "totalAmount"
               from "AlkaneHolder"
               where "alkaneIdBlock" = $1 and "alkaneIdTx" = $2
               order by "totalAmount" desc
               limit $3 offset $4"#
        )
        .bind(block)
        .bind(tx)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await;

        match holders_result {
            Ok(rows) => rows,
            Err(e) => {
                log::error!("Failed to fetch holders: {}", e);
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "ok": false,
                    "error": "internal_error"
                }));
            }
        }
    };

    let items: Vec<HolderInfo> = rows
        .into_iter()
        .map(|(address, amount)| HolderInfo { address, amount })
        .collect();

    let has_more = (offset + limit) < total;

    HttpResponse::Ok().json(HoldersResponse {
        ok: true,
        alkane: req.alkane.clone(),
        page: req.page,
        limit,
        total,
        has_more,
        items,
    })
}

#[derive(Debug, Deserialize)]
pub struct HolderCountRequest {
    pub alkane: String,
}

#[derive(Debug, Serialize)]
pub struct HolderCountResponse {
    pub ok: bool,
    pub alkane: String,
    pub count: i64,
}

pub async fn get_holders_count(
    state: web::Data<AppState>,
    req: web::Json<HolderCountRequest>,
) -> HttpResponse {
    let pool = &state.db_pool;
    
    let parts: Vec<&str> = req.alkane.split(':').collect();
    if parts.len() != 2 {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "ok": false,
            "error": "invalid_alkane_format"
        }));
    }

    let block: i32 = match parts[0].parse() {
        Ok(b) => b,
        Err(_) => {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "ok": false,
                "error": "invalid_block"
            }));
        }
    };

    let tx: i64 = match parts[1].parse() {
        Ok(t) => t,
        Err(_) => {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "ok": false,
                "error": "invalid_tx"
            }));
        }
    };

    let count_result = sqlx::query_as::<_, (i64,)>(
        r#"select "count" from "AlkaneHolderCount" where "alkaneIdBlock" = $1 and "alkaneIdTx" = $2"#
    )
    .bind(block)
    .bind(tx)
    .fetch_optional(pool)
    .await;

    let count = match count_result {
        Ok(Some((c,))) => c,
        Ok(None) => 0,
        Err(e) => {
            log::error!("Failed to fetch holder count: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "ok": false,
                "error": "internal_error"
            }));
        }
    };

    HttpResponse::Ok().json(HolderCountResponse {
        ok: true,
        alkane: req.alkane.clone(),
        count,
    })
}

#[derive(Debug, Deserialize)]
pub struct AddressOutpointsRequest {
    pub address: String,
}

#[derive(Debug, Serialize)]
pub struct AddressOutpointsResponse {
    pub ok: bool,
    pub address: String,
    pub outpoints: Vec<OutpointInfo>,
}

pub async fn get_address_outpoints(
    state: web::Data<AppState>,
    req: web::Json<AddressOutpointsRequest>,
) -> HttpResponse {
    let pool = &state.db_pool;
    
    let utxo_result = sqlx::query_as::<_, (String, i32, i32, i64, String)>(
        r#"select "outpointTxid", "outpointVout", "alkaneIdBlock", "alkaneIdTx", "amount" 
           from "AlkaneBalanceUtxo" 
           where "address" = $1 and not "spent" 
           order by "outpointTxid", "outpointVout""#
    )
    .bind(&req.address)
    .fetch_all(pool)
    .await;

    let rows = match utxo_result {
        Ok(rows) => rows,
        Err(e) => {
            log::error!("Failed to fetch address outpoints: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "ok": false,
                "error": "internal_error"
            }));
        }
    };

    let mut outpoint_map: HashMap<String, Vec<BalanceEntry>> = HashMap::new();
    for (txid, vout, block, tx, amount) in rows {
        let outpoint = format!("{}:{}", txid, vout);
        outpoint_map.entry(outpoint).or_default().push(BalanceEntry {
            alkane: format!("{}:{}", block, tx),
            amount,
        });
    }

    let outpoints: Vec<OutpointInfo> = outpoint_map
        .into_iter()
        .map(|(outpoint, entries)| OutpointInfo { outpoint, entries })
        .collect();

    HttpResponse::Ok().json(AddressOutpointsResponse {
        ok: true,
        address: req.address.clone(),
        outpoints,
    })
}
