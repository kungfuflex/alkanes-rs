use actix_web::{web, HttpResponse, Responder};
use serde_json::json;

use crate::{
    models::{
        AddressRequest, ApiResponse, ErrorResponse, PoolDetailsRequest, SwapPairDetailsRequest,
        TokenPairsRequest,
    },
    services::{pools::PoolService, AppState},
};

pub async fn get_pools(
    state: web::Data<AppState>,
    req: web::Json<TokenPairsRequest>,
) -> impl Responder {
    let pool_service = PoolService::new(
        state.db_pool.clone(),
        state.redis_client.clone(),
        state.config.network_env.clone(),
    );

    match pool_service.get_pools_by_factory(&(&req.factory_id).into()).await {
        Ok(pools) => {
            let response = ApiResponse::ok(json!({
                "pools": pools
            }));
            HttpResponse::Ok().json(response)
        }
        Err(e) => {
            let error = ErrorResponse::with_stack(
                500,
                "Failed to get pools".to_string(),
                e.to_string(),
            );
            HttpResponse::InternalServerError().json(error)
        }
    }
}

pub async fn get_pool_details(
    state: web::Data<AppState>,
    req: web::Json<PoolDetailsRequest>,
) -> impl Responder {
    let pool_service = PoolService::new(
        state.db_pool.clone(),
        state.redis_client.clone(),
        state.config.network_env.clone(),
    );

    match pool_service.get_pool_by_id(&(&req.pool_id).into()).await {
        Ok(Some(pool)) => {
            let response = ApiResponse::ok(pool);
            HttpResponse::Ok().json(response)
        }
        Ok(None) => {
            let error = ErrorResponse::new(404, "Pool not found".to_string());
            HttpResponse::NotFound().json(error)
        }
        Err(e) => {
            let error = ErrorResponse::with_stack(
                500,
                "Failed to get pool details".to_string(),
                e.to_string(),
            );
            HttpResponse::InternalServerError().json(error)
        }
    }
}

pub async fn get_all_pools_details(
    state: web::Data<AppState>,
    req: web::Json<TokenPairsRequest>,
) -> impl Responder {
    let pool_service = PoolService::new(
        state.db_pool.clone(),
        state.redis_client.clone(),
        state.config.network_env.clone(),
    );

    match pool_service.get_pools_by_factory(&(&req.factory_id).into()).await {
        Ok(pools) => {
            let response = ApiResponse::ok(json!({
                "pools": pools
            }));
            HttpResponse::Ok().json(response)
        }
        Err(e) => {
            let error = ErrorResponse::with_stack(
                500,
                "Failed to get all pool details".to_string(),
                e.to_string(),
            );
            HttpResponse::InternalServerError().json(error)
        }
    }
}

pub async fn address_positions(
    state: web::Data<AppState>,
    req: web::Json<AddressRequest>,
) -> impl Responder {
    if req.address.is_empty() {
        return HttpResponse::BadRequest().json(ErrorResponse::new(
            400,
            "address is required".to_string(),
        ));
    }

    let pool_service = PoolService::new(
        state.db_pool.clone(),
        state.redis_client.clone(),
        state.config.network_env.clone(),
    );

    match pool_service.get_address_positions(&req.address).await {
        Ok(positions) => {
            let response = ApiResponse::ok(json!({
                "positions": positions
            }));
            HttpResponse::Ok().json(response)
        }
        Err(e) => {
            let error = ErrorResponse::with_stack(
                500,
                "Failed to get address positions".to_string(),
                e.to_string(),
            );
            HttpResponse::InternalServerError().json(error)
        }
    }
}

pub async fn get_all_token_pairs(
    state: web::Data<AppState>,
    req: web::Json<TokenPairsRequest>,
) -> impl Responder {
    let pool_service = PoolService::new(
        state.db_pool.clone(),
        state.redis_client.clone(),
        state.config.network_env.clone(),
    );

    match pool_service.get_all_token_pairs(&(&req.factory_id).into()).await {
        Ok(pairs) => {
            let response = ApiResponse::ok(json!({
                "pairs": pairs
            }));
            HttpResponse::Ok().json(response)
        }
        Err(e) => {
            let error = ErrorResponse::with_stack(
                500,
                "Failed to get all token pairs".to_string(),
                e.to_string(),
            );
            HttpResponse::InternalServerError().json(error)
        }
    }
}

pub async fn get_token_pairs(
    state: web::Data<AppState>,
    req: web::Json<TokenPairsRequest>,
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

    let pool_service = PoolService::new(
        state.db_pool.clone(),
        state.redis_client.clone(),
        state.config.network_env.clone(),
    );

    match pool_service
        .get_token_pairs(&(&req.factory_id).into(), &alkane_id.into())
        .await
    {
        Ok(pairs) => {
            let response = ApiResponse::ok(json!({
                "pairs": pairs
            }));
            HttpResponse::Ok().json(response)
        }
        Err(e) => {
            let error = ErrorResponse::with_stack(
                500,
                "Failed to get token pairs".to_string(),
                e.to_string(),
            );
            HttpResponse::InternalServerError().json(error)
        }
    }
}

pub async fn get_alkane_swap_pair_details(
    state: web::Data<AppState>,
    req: web::Json<SwapPairDetailsRequest>,
) -> impl Responder {
    let pool_service = PoolService::new(
        state.db_pool.clone(),
        state.redis_client.clone(),
        state.config.network_env.clone(),
    );

    // Get direct pool if exists
    match pool_service
        .get_token_pairs(&(&req.factory_id).into(), &(&req.token_a_id).into())
        .await
    {
        Ok(pairs) => {
            // Find if there's a direct pair
            let direct_pair = pairs.iter().find(|p| {
                (p.token0_block_id == req.token_b_id.block
                    && p.token0_tx_id == req.token_b_id.tx)
                    || (p.token1_block_id == req.token_b_id.block
                        && p.token1_tx_id == req.token_b_id.tx)
            });

            let response = ApiResponse::ok(json!({
                "paths": if direct_pair.is_some() {
                    vec![json!({
                        "type": "direct",
                        "pools": vec![direct_pair]
                    })]
                } else {
                    // TODO: Implement multi-hop routing
                    vec![]
                }
            }));
            HttpResponse::Ok().json(response)
        }
        Err(e) => {
            let error = ErrorResponse::with_stack(
                500,
                "Failed to get swap pair details".to_string(),
                e.to_string(),
            );
            HttpResponse::InternalServerError().json(error)
        }
    }
}
