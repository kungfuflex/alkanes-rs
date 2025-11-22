use actix_web::{web, HttpResponse};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::services::AppState;

#[derive(Debug, Deserialize)]
pub struct GetKeysRequest {
    pub alkane: String,
    #[serde(default)]
    pub prefix: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: i64,
}

fn default_limit() -> i64 {
    100
}

#[derive(Debug, Serialize)]
pub struct GetKeysResponse {
    pub ok: bool,
    pub alkane: String,
    pub keys: HashMap<String, KeyValue>,
}

#[derive(Debug, Serialize)]
pub struct KeyValue {
    pub key: String,
    pub value: String,
    pub last_txid: String,
    pub last_vout: i32,
    pub block_height: i32,
    pub updated_at: String,
}

pub async fn get_keys(
    state: web::Data<AppState>,
    req: web::Json<GetKeysRequest>,
) -> HttpResponse {
    let pool = &state.db_pool;
    
    let parts: Vec<&str> = req.alkane.split(':').collect();
    if parts.len() != 2 {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "ok": false,
            "error": "invalid_alkane_format",
            "hint": "expected \"<block>:<tx>\""
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

    let query = if let Some(prefix) = &req.prefix {
        sqlx::query_as::<_, (String, String, String, i32, i32, String)>(
            r#"
            select "key", "value", "lastTxid", "lastVout", "blockHeight", "updatedAt"::text
            from "AlkaneStorage"
            where "alkaneIdBlock" = $1 and "alkaneIdTx" = $2 and "key" like $3
            order by "key"
            limit $4
            "#
        )
        .bind(block)
        .bind(tx)
        .bind(format!("{}%", prefix))
        .bind(limit)
    } else {
        sqlx::query_as::<_, (String, String, String, i32, i32, String)>(
            r#"
            select "key", "value", "lastTxid", "lastVout", "blockHeight", "updatedAt"::text
            from "AlkaneStorage"
            where "alkaneIdBlock" = $1 and "alkaneIdTx" = $2
            order by "key"
            limit $3
            "#
        )
        .bind(block)
        .bind(tx)
        .bind(limit)
    };

    let rows_result = query.fetch_all(pool).await;

    let rows = match rows_result {
        Ok(rows) => rows,
        Err(e) => {
            log::error!("Failed to fetch storage keys: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "ok": false,
                "error": "internal_error"
            }));
        }
    };

    let mut keys = HashMap::new();
    for (key, value, last_txid, last_vout, block_height, updated_at) in rows {
        keys.insert(
            key.clone(),
            KeyValue {
                key,
                value,
                last_txid,
                last_vout,
                block_height,
                updated_at,
            },
        );
    }

    HttpResponse::Ok().json(GetKeysResponse {
        ok: true,
        alkane: req.alkane.clone(),
        keys,
    })
}
