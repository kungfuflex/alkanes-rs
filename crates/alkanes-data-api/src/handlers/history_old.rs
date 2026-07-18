use actix_web::{web, HttpResponse, Responder};
use serde_json::json;

use crate::{
    models::{ApiResponse, ErrorResponse, HistoryRequest},
    services::{history::HistoryService, AppState},
};

pub async fn get_pool_swap_history(
    state: web::Data<AppState>,
    req: web::Json<HistoryRequest>,
) -> impl Responder {
    let pool_id = match &req.pool_id {
        Some(id) => id,
        None => {
            return HttpResponse::BadRequest().json(ErrorResponse::new(
                400,
                "poolId is required".to_string(),
            ));
        }
    };

    let history_service = HistoryService::new(state.db_pool.clone());
    let limit = req.count.unwrap_or(50);
    let offset = req.offset.unwrap_or(0);
    let successful_only = req.successful.unwrap_or(true);

    match history_service
        .get_pool_swap_history(pool_id, limit, offset, successful_only)
        .await
    {
        Ok((swaps, total)) => {
            let response = ApiResponse::ok(json!({
                "swaps": swaps,
                "total": total
            }));
            HttpResponse::Ok().json(response)
        }
        Err(e) => {
            let error = ErrorResponse::with_stack(
                500,
                "Failed to get pool swap history".to_string(),
                e.to_string(),
            );
            HttpResponse::InternalServerError().json(error)
        }
    }
}

pub async fn get_token_swap_history(
    state: web::Data<AppState>,
    req: web::Json<HistoryRequest>,
) -> impl Responder {
    if req.alkane_id.is_none() {
        return HttpResponse::BadRequest().json(ErrorResponse::new(
            400,
            "alkaneId is required".to_string(),
        ));
    }

    // TODO: Query swap history for token
    let response = ApiResponse::ok(json!({
        "swaps": [],
        "total": 0
    }));
    HttpResponse::Ok().json(response)
}

pub async fn get_pool_mint_history(
    state: web::Data<AppState>,
    req: web::Json<HistoryRequest>,
) -> impl Responder {
    if req.pool_id.is_none() {
        return HttpResponse::BadRequest().json(ErrorResponse::new(
            400,
            "poolId is required".to_string(),
        ));
    }

    // TODO: Query mint (add liquidity) history for pool
    let response = ApiResponse::ok(json!({
        "mints": [],
        "total": 0
    }));
    HttpResponse::Ok().json(response)
}

pub async fn get_pool_burn_history(
    state: web::Data<AppState>,
    req: web::Json<HistoryRequest>,
) -> impl Responder {
    if req.pool_id.is_none() {
        return HttpResponse::BadRequest().json(ErrorResponse::new(
            400,
            "poolId is required".to_string(),
        ));
    }

    // TODO: Query burn (remove liquidity) history for pool
    let response = ApiResponse::ok(json!({
        "burns": [],
        "total": 0
    }));
    HttpResponse::Ok().json(response)
}

pub async fn get_pool_creation_history(
    state: web::Data<AppState>,
    req: web::Json<HistoryRequest>,
) -> impl Responder {
    // TODO: Query pool creation events
    let response = ApiResponse::ok(json!({
        "creations": [],
        "total": 0
    }));
    HttpResponse::Ok().json(response)
}

pub async fn get_address_swap_history_for_pool(
    state: web::Data<AppState>,
    req: web::Json<HistoryRequest>,
) -> impl Responder {
    if req.address.is_none() || req.pool_id.is_none() {
        return HttpResponse::BadRequest().json(ErrorResponse::new(
            400,
            "address and poolId are required".to_string(),
        ));
    }

    // TODO: Query address-specific swap history for pool
    let response = ApiResponse::ok(json!({
        "swaps": [],
        "total": 0
    }));
    HttpResponse::Ok().json(response)
}

pub async fn get_address_swap_history_for_token(
    state: web::Data<AppState>,
    req: web::Json<HistoryRequest>,
) -> impl Responder {
    if req.address.is_none() || req.alkane_id.is_none() {
        return HttpResponse::BadRequest().json(ErrorResponse::new(
            400,
            "address and alkaneId are required".to_string(),
        ));
    }

    // TODO: Query address-specific swap history for token
    let response = ApiResponse::ok(json!({
        "swaps": [],
        "total": 0
    }));
    HttpResponse::Ok().json(response)
}

pub async fn get_address_wrap_history(
    state: web::Data<AppState>,
    req: web::Json<HistoryRequest>,
) -> impl Responder {
    if req.address.is_none() {
        return HttpResponse::BadRequest().json(ErrorResponse::new(
            400,
            "address is required".to_string(),
        ));
    }

    // TODO: Query wrap transaction history for address
    let response = ApiResponse::ok(json!({
        "wraps": [],
        "total": 0
    }));
    HttpResponse::Ok().json(response)
}

pub async fn get_address_unwrap_history(
    state: web::Data<AppState>,
    req: web::Json<HistoryRequest>,
) -> impl Responder {
    if req.address.is_none() {
        return HttpResponse::BadRequest().json(ErrorResponse::new(
            400,
            "address is required".to_string(),
        ));
    }

    // TODO: Query unwrap transaction history for address
    let response = ApiResponse::ok(json!({
        "unwraps": [],
        "total": 0
    }));
    HttpResponse::Ok().json(response)
}

pub async fn get_all_wrap_history(
    state: web::Data<AppState>,
    req: web::Json<HistoryRequest>,
) -> impl Responder {
    // TODO: Query all wrap transactions
    let response = ApiResponse::ok(json!({
        "wraps": [],
        "total": 0
    }));
    HttpResponse::Ok().json(response)
}

pub async fn get_all_unwrap_history(
    state: web::Data<AppState>,
    req: web::Json<HistoryRequest>,
) -> impl Responder {
    // TODO: Query all unwrap transactions
    let response = ApiResponse::ok(json!({
        "unwraps": [],
        "total": 0
    }));
    HttpResponse::Ok().json(response)
}

pub async fn get_total_unwrap_amount(
    state: web::Data<AppState>,
    req: web::Json<serde_json::Value>,
) -> impl Responder {
    // TODO: Calculate total amount unwrapped
    let response = ApiResponse::ok(json!({
        "totalAmount": "0"
    }));
    HttpResponse::Ok().json(response)
}

pub async fn get_address_pool_creation_history(
    state: web::Data<AppState>,
    req: web::Json<HistoryRequest>,
) -> impl Responder {
    if req.address.is_none() {
        return HttpResponse::BadRequest().json(ErrorResponse::new(
            400,
            "address is required".to_string(),
        ));
    }

    // TODO: Query pools created by address
    let response = ApiResponse::ok(json!({
        "creations": [],
        "total": 0
    }));
    HttpResponse::Ok().json(response)
}

pub async fn get_address_pool_mint_history(
    state: web::Data<AppState>,
    req: web::Json<HistoryRequest>,
) -> impl Responder {
    if req.address.is_none() {
        return HttpResponse::BadRequest().json(ErrorResponse::new(
            400,
            "address is required".to_string(),
        ));
    }

    // TODO: Query liquidity additions by address
    let response = ApiResponse::ok(json!({
        "mints": [],
        "total": 0
    }));
    HttpResponse::Ok().json(response)
}

pub async fn get_address_pool_burn_history(
    state: web::Data<AppState>,
    req: web::Json<HistoryRequest>,
) -> impl Responder {
    if req.address.is_none() {
        return HttpResponse::BadRequest().json(ErrorResponse::new(
            400,
            "address is required".to_string(),
        ));
    }

    // TODO: Query liquidity removals by address
    let response = ApiResponse::ok(json!({
        "burns": [],
        "total": 0
    }));
    HttpResponse::Ok().json(response)
}

pub async fn get_all_address_amm_tx_history(
    state: web::Data<AppState>,
    req: web::Json<HistoryRequest>,
) -> impl Responder {
    if req.address.is_none() {
        return HttpResponse::BadRequest().json(ErrorResponse::new(
            400,
            "address is required".to_string(),
        ));
    }

    if let Some(ref tx_type) = req.transaction_type {
        let valid_types = ["swap", "mint", "burn", "creation", "wrap", "unwrap"];
        if !valid_types.contains(&tx_type.as_str()) {
            return HttpResponse::BadRequest().json(ErrorResponse::new(
                400,
                "transactionType must be one of: swap, mint, burn, creation, wrap, unwrap"
                    .to_string(),
            ));
        }
    }

    // TODO: Query all AMM transactions for address
    let response = ApiResponse::ok(json!({
        "transactions": [],
        "total": 0
    }));
    HttpResponse::Ok().json(response)
}

pub async fn get_all_amm_tx_history(
    state: web::Data<AppState>,
    req: web::Json<HistoryRequest>,
) -> impl Responder {
    if let Some(ref tx_type) = req.transaction_type {
        let valid_types = ["swap", "mint", "burn", "creation", "wrap", "unwrap"];
        if !valid_types.contains(&tx_type.as_str()) {
            return HttpResponse::BadRequest().json(ErrorResponse::new(
                400,
                "transactionType must be one of: swap, mint, burn, creation, wrap, unwrap"
                    .to_string(),
            ));
        }
    }

    // TODO: Query all AMM transactions
    let response = ApiResponse::ok(json!({
        "transactions": [],
        "total": 0
    }));
    HttpResponse::Ok().json(response)
}
