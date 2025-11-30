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
        .get_pool_swap_history(&pool_id.into(), limit, offset, successful_only)
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
    let alkane_id = match &req.alkane_id {
        Some(id) => id,
        None => {
            return HttpResponse::BadRequest().json(ErrorResponse::new(
                400,
                "alkaneId is required".to_string(),
            ));
        }
    };

    let history_service = HistoryService::new(state.db_pool.clone());
    let limit = req.count.unwrap_or(50);
    let offset = req.offset.unwrap_or(0);
    let successful_only = req.successful.unwrap_or(true);

    match history_service
        .get_token_swap_history(&alkane_id.into(), limit, offset, successful_only)
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
                "Failed to get token swap history".to_string(),
                e.to_string(),
            );
            HttpResponse::InternalServerError().json(error)
        }
    }
}

pub async fn get_pool_mint_history(
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
        .get_pool_mint_history(&pool_id.into(), limit, offset, successful_only)
        .await
    {
        Ok((mints, total)) => {
            let response = ApiResponse::ok(json!({
                "mints": mints,
                "total": total
            }));
            HttpResponse::Ok().json(response)
        }
        Err(e) => {
            let error = ErrorResponse::with_stack(
                500,
                "Failed to get pool mint history".to_string(),
                e.to_string(),
            );
            HttpResponse::InternalServerError().json(error)
        }
    }
}

pub async fn get_pool_burn_history(
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
        .get_pool_burn_history(&pool_id.into(), limit, offset, successful_only)
        .await
    {
        Ok((burns, total)) => {
            let response = ApiResponse::ok(json!({
                "burns": burns,
                "total": total
            }));
            HttpResponse::Ok().json(response)
        }
        Err(e) => {
            let error = ErrorResponse::with_stack(
                500,
                "Failed to get pool burn history".to_string(),
                e.to_string(),
            );
            HttpResponse::InternalServerError().json(error)
        }
    }
}

pub async fn get_pool_creation_history(
    state: web::Data<AppState>,
    req: web::Json<HistoryRequest>,
) -> impl Responder {
    let factory_id = match &req.factory_id {
        Some(id) => id,
        None => {
            return HttpResponse::BadRequest().json(ErrorResponse::new(
                400,
                "factoryId is required".to_string(),
            ));
        }
    };

    let history_service = HistoryService::new(state.db_pool.clone());
    let limit = req.count.unwrap_or(50);
    let offset = req.offset.unwrap_or(0);

    match history_service
        .get_pool_creation_history(&factory_id.into(), limit, offset)
        .await
    {
        Ok((creations, total)) => {
            let response = ApiResponse::ok(json!({
                "creations": creations,
                "total": total
            }));
            HttpResponse::Ok().json(response)
        }
        Err(e) => {
            let error = ErrorResponse::with_stack(
                500,
                "Failed to get pool creation history".to_string(),
                e.to_string(),
            );
            HttpResponse::InternalServerError().json(error)
        }
    }
}

pub async fn get_address_swap_history_for_pool(
    state: web::Data<AppState>,
    req: web::Json<HistoryRequest>,
) -> impl Responder {
    let address = match &req.address {
        Some(addr) => addr,
        None => {
            return HttpResponse::BadRequest().json(ErrorResponse::new(
                400,
                "address is required".to_string(),
            ));
        }
    };

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
        .get_address_swap_history_for_pool(address, &pool_id.into(), limit, offset, successful_only)
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
                "Failed to get address swap history for pool".to_string(),
                e.to_string(),
            );
            HttpResponse::InternalServerError().json(error)
        }
    }
}

pub async fn get_address_swap_history_for_token(
    state: web::Data<AppState>,
    req: web::Json<HistoryRequest>,
) -> impl Responder {
    let address = match &req.address {
        Some(addr) => addr,
        None => {
            return HttpResponse::BadRequest().json(ErrorResponse::new(
                400,
                "address is required".to_string(),
            ));
        }
    };

    let alkane_id = match &req.alkane_id {
        Some(id) => id,
        None => {
            return HttpResponse::BadRequest().json(ErrorResponse::new(
                400,
                "alkaneId is required".to_string(),
            ));
        }
    };

    let history_service = HistoryService::new(state.db_pool.clone());
    let limit = req.count.unwrap_or(50);
    let offset = req.offset.unwrap_or(0);
    let successful_only = req.successful.unwrap_or(true);

    match history_service
        .get_address_swap_history_for_token(address, &alkane_id.into(), limit, offset, successful_only)
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
                "Failed to get address swap history for token".to_string(),
                e.to_string(),
            );
            HttpResponse::InternalServerError().json(error)
        }
    }
}

pub async fn get_address_wrap_history(
    state: web::Data<AppState>,
    req: web::Json<HistoryRequest>,
) -> impl Responder {
    let address = match &req.address {
        Some(addr) => addr,
        None => {
            return HttpResponse::BadRequest().json(ErrorResponse::new(
                400,
                "address is required".to_string(),
            ));
        }
    };

    let history_service = HistoryService::new(state.db_pool.clone());
    let limit = req.count.unwrap_or(50);
    let offset = req.offset.unwrap_or(0);
    let successful_only = req.successful.unwrap_or(true);

    match history_service
        .get_address_wrap_history(address, limit, offset, successful_only)
        .await
    {
        Ok((wraps, total)) => {
            let response = ApiResponse::ok(json!({
                "wraps": wraps,
                "total": total
            }));
            HttpResponse::Ok().json(response)
        }
        Err(e) => {
            let error = ErrorResponse::with_stack(
                500,
                "Failed to get address wrap history".to_string(),
                e.to_string(),
            );
            HttpResponse::InternalServerError().json(error)
        }
    }
}

pub async fn get_address_unwrap_history(
    state: web::Data<AppState>,
    req: web::Json<HistoryRequest>,
) -> impl Responder {
    let address = match &req.address {
        Some(addr) => addr,
        None => {
            return HttpResponse::BadRequest().json(ErrorResponse::new(
                400,
                "address is required".to_string(),
            ));
        }
    };

    let history_service = HistoryService::new(state.db_pool.clone());
    let limit = req.count.unwrap_or(50);
    let offset = req.offset.unwrap_or(0);
    let successful_only = req.successful.unwrap_or(true);

    match history_service
        .get_address_unwrap_history(address, limit, offset, successful_only)
        .await
    {
        Ok((unwraps, total)) => {
            let response = ApiResponse::ok(json!({
                "unwraps": unwraps,
                "total": total
            }));
            HttpResponse::Ok().json(response)
        }
        Err(e) => {
            let error = ErrorResponse::with_stack(
                500,
                "Failed to get address unwrap history".to_string(),
                e.to_string(),
            );
            HttpResponse::InternalServerError().json(error)
        }
    }
}

pub async fn get_all_wrap_history(
    state: web::Data<AppState>,
    req: web::Json<HistoryRequest>,
) -> impl Responder {
    let history_service = HistoryService::new(state.db_pool.clone());
    let limit = req.count.unwrap_or(50);
    let offset = req.offset.unwrap_or(0);
    let successful_only = req.successful.unwrap_or(true);

    match history_service
        .get_all_wrap_history(limit, offset, successful_only)
        .await
    {
        Ok((wraps, total)) => {
            let response = ApiResponse::ok(json!({
                "wraps": wraps,
                "total": total
            }));
            HttpResponse::Ok().json(response)
        }
        Err(e) => {
            let error = ErrorResponse::with_stack(
                500,
                "Failed to get all wrap history".to_string(),
                e.to_string(),
            );
            HttpResponse::InternalServerError().json(error)
        }
    }
}

pub async fn get_all_unwrap_history(
    state: web::Data<AppState>,
    req: web::Json<HistoryRequest>,
) -> impl Responder {
    let history_service = HistoryService::new(state.db_pool.clone());
    let limit = req.count.unwrap_or(50);
    let offset = req.offset.unwrap_or(0);
    let successful_only = req.successful.unwrap_or(true);

    match history_service
        .get_all_unwrap_history(limit, offset, successful_only)
        .await
    {
        Ok((unwraps, total)) => {
            let response = ApiResponse::ok(json!({
                "unwraps": unwraps,
                "total": total
            }));
            HttpResponse::Ok().json(response)
        }
        Err(e) => {
            let error = ErrorResponse::with_stack(
                500,
                "Failed to get all unwrap history".to_string(),
                e.to_string(),
            );
            HttpResponse::InternalServerError().json(error)
        }
    }
}

pub async fn get_total_unwrap_amount(
    state: web::Data<AppState>,
    _req: web::Json<serde_json::Value>,
) -> impl Responder {
    let history_service = HistoryService::new(state.db_pool.clone());

    match history_service.get_total_unwrap_amount().await {
        Ok(total_amount) => {
            let response = ApiResponse::ok(json!({
                "totalAmount": total_amount
            }));
            HttpResponse::Ok().json(response)
        }
        Err(e) => {
            let error = ErrorResponse::with_stack(
                500,
                "Failed to get total unwrap amount".to_string(),
                e.to_string(),
            );
            HttpResponse::InternalServerError().json(error)
        }
    }
}

pub async fn get_address_pool_creation_history(
    state: web::Data<AppState>,
    req: web::Json<HistoryRequest>,
) -> impl Responder {
    let address = match &req.address {
        Some(addr) => addr,
        None => {
            return HttpResponse::BadRequest().json(ErrorResponse::new(
                400,
                "address is required".to_string(),
            ));
        }
    };

    let history_service = HistoryService::new(state.db_pool.clone());
    let limit = req.count.unwrap_or(50);
    let offset = req.offset.unwrap_or(0);

    match history_service
        .get_address_pool_creation_history(address, limit, offset)
        .await
    {
        Ok((creations, total)) => {
            let response = ApiResponse::ok(json!({
                "creations": creations,
                "total": total
            }));
            HttpResponse::Ok().json(response)
        }
        Err(e) => {
            let error = ErrorResponse::with_stack(
                500,
                "Failed to get address pool creation history".to_string(),
                e.to_string(),
            );
            HttpResponse::InternalServerError().json(error)
        }
    }
}

pub async fn get_address_pool_mint_history(
    state: web::Data<AppState>,
    req: web::Json<HistoryRequest>,
) -> impl Responder {
    let address = match &req.address {
        Some(addr) => addr,
        None => {
            return HttpResponse::BadRequest().json(ErrorResponse::new(
                400,
                "address is required".to_string(),
            ));
        }
    };

    let history_service = HistoryService::new(state.db_pool.clone());
    let limit = req.count.unwrap_or(50);
    let offset = req.offset.unwrap_or(0);
    let successful_only = req.successful.unwrap_or(true);

    match history_service
        .get_address_pool_mint_history(address, limit, offset, successful_only)
        .await
    {
        Ok((mints, total)) => {
            let response = ApiResponse::ok(json!({
                "mints": mints,
                "total": total
            }));
            HttpResponse::Ok().json(response)
        }
        Err(e) => {
            let error = ErrorResponse::with_stack(
                500,
                "Failed to get address pool mint history".to_string(),
                e.to_string(),
            );
            HttpResponse::InternalServerError().json(error)
        }
    }
}

pub async fn get_address_pool_burn_history(
    state: web::Data<AppState>,
    req: web::Json<HistoryRequest>,
) -> impl Responder {
    let address = match &req.address {
        Some(addr) => addr,
        None => {
            return HttpResponse::BadRequest().json(ErrorResponse::new(
                400,
                "address is required".to_string(),
            ));
        }
    };

    let history_service = HistoryService::new(state.db_pool.clone());
    let limit = req.count.unwrap_or(50);
    let offset = req.offset.unwrap_or(0);
    let successful_only = req.successful.unwrap_or(true);

    match history_service
        .get_address_pool_burn_history(address, limit, offset, successful_only)
        .await
    {
        Ok((burns, total)) => {
            let response = ApiResponse::ok(json!({
                "burns": burns,
                "total": total
            }));
            HttpResponse::Ok().json(response)
        }
        Err(e) => {
            let error = ErrorResponse::with_stack(
                500,
                "Failed to get address pool burn history".to_string(),
                e.to_string(),
            );
            HttpResponse::InternalServerError().json(error)
        }
    }
}

pub async fn get_all_address_amm_tx_history(
    state: web::Data<AppState>,
    req: web::Json<HistoryRequest>,
) -> impl Responder {
    let address = match &req.address {
        Some(addr) => addr,
        None => {
            return HttpResponse::BadRequest().json(ErrorResponse::new(
                400,
                "address is required".to_string(),
            ));
        }
    };

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

    let history_service = HistoryService::new(state.db_pool.clone());
    let limit = req.count.unwrap_or(50);
    let offset = req.offset.unwrap_or(0);
    let successful_only = req.successful.unwrap_or(true);

    match history_service
        .get_all_address_amm_tx_history(
            address,
            limit,
            offset,
            successful_only,
            req.transaction_type.as_deref(),
        )
        .await
    {
        Ok((transactions, total)) => {
            let response = ApiResponse::ok(json!({
                "transactions": transactions,
                "total": total
            }));
            HttpResponse::Ok().json(response)
        }
        Err(e) => {
            let error = ErrorResponse::with_stack(
                500,
                "Failed to get all address AMM transaction history".to_string(),
                e.to_string(),
            );
            HttpResponse::InternalServerError().json(error)
        }
    }
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

    let history_service = HistoryService::new(state.db_pool.clone());
    let limit = req.count.unwrap_or(50);
    let offset = req.offset.unwrap_or(0);
    let successful_only = req.successful.unwrap_or(true);

    match history_service
        .get_all_amm_tx_history(limit, offset, successful_only, req.transaction_type.as_deref())
        .await
    {
        Ok((transactions, total)) => {
            let response = ApiResponse::ok(json!({
                "transactions": transactions,
                "total": total
            }));
            HttpResponse::Ok().json(response)
        }
        Err(e) => {
            let error = ErrorResponse::with_stack(
                500,
                "Failed to get all AMM transaction history".to_string(),
                e.to_string(),
            );
            HttpResponse::InternalServerError().json(error)
        }
    }
}
