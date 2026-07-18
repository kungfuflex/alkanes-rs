use actix_web::{web, HttpResponse, Responder};
use serde::Serialize;

use crate::services::AppState;

#[derive(Serialize)]
struct BlockHeightResponse {
    height: i64,
}

#[derive(Serialize)]
struct BlockHashResponse {
    hash: String,
}

#[derive(Serialize)]
struct IndexerPositionResponse {
    height: i64,
    hash: String,
}

/// GET /blockheight - Returns the latest block height processed by the indexer
pub async fn get_block_height(state: web::Data<AppState>) -> impl Responder {
    let result: Result<Option<(i64,)>, _> = sqlx::query_as(
        "SELECT height FROM indexer_position WHERE id = 1"
    )
    .fetch_optional(&state.db_pool)
    .await;

    match result {
        Ok(Some((height,))) => HttpResponse::Ok().json(BlockHeightResponse { height }),
        Ok(None) => HttpResponse::Ok().json(BlockHeightResponse { height: 0 }),
        Err(e) => {
            log::error!("Failed to get block height: {}", e);
            HttpResponse::InternalServerError().body(format!("Database error: {}", e))
        }
    }
}

/// GET /blockhash - Returns the latest block hash processed by the indexer
pub async fn get_block_hash(state: web::Data<AppState>) -> impl Responder {
    let result: Result<Option<(String,)>, _> = sqlx::query_as(
        "SELECT block_hash FROM indexer_position WHERE id = 1"
    )
    .fetch_optional(&state.db_pool)
    .await;

    match result {
        Ok(Some((hash,))) => HttpResponse::Ok().json(BlockHashResponse { hash }),
        Ok(None) => HttpResponse::Ok().json(BlockHashResponse { hash: String::new() }),
        Err(e) => {
            log::error!("Failed to get block hash: {}", e);
            HttpResponse::InternalServerError().body(format!("Database error: {}", e))
        }
    }
}

/// GET /indexer-position - Returns both height and hash of the latest processed block
pub async fn get_indexer_position(state: web::Data<AppState>) -> impl Responder {
    let result: Result<Option<(i64, String)>, _> = sqlx::query_as(
        "SELECT height, block_hash FROM indexer_position WHERE id = 1"
    )
    .fetch_optional(&state.db_pool)
    .await;

    match result {
        Ok(Some((height, hash))) => HttpResponse::Ok().json(IndexerPositionResponse { height, hash }),
        Ok(None) => HttpResponse::Ok().json(IndexerPositionResponse { height: 0, hash: String::new() }),
        Err(e) => {
            log::error!("Failed to get indexer position: {}", e);
            HttpResponse::InternalServerError().body(format!("Database error: {}", e))
        }
    }
}
