use actix_web::{web, HttpResponse, Responder};
use serde_json::json;

use crate::{
    models::{AddressRequest, AlkaneDetailsRequest, ApiResponse, ErrorResponse, SearchRequest, PaginationRequest},
    services::{alkanes::AlkanesService, AppState},
};

pub async fn get_alkanes(
    state: web::Data<AppState>,
    req: web::Json<PaginationRequest>,
) -> impl Responder {
    let alkanes_service = AlkanesService::new(
        state.alkanes_rpc.clone(),
        state.redis_client.clone(),
        state.db_pool.clone(),
    );

    match alkanes_service
        .get_alkanes(
            req.limit.or(req.count),
            req.offset,
            None,
            None,
            None,
        )
        .await
    {
        Ok((tokens, total)) => {
            let response = ApiResponse::ok(json!({
                "tokens": tokens,
                "total": total,
                "count": tokens.len(),
                "limit": req.limit.or(req.count),
                "offset": req.offset.unwrap_or(0)
            }));
            HttpResponse::Ok().json(response)
        }
        Err(e) => {
            let error = ErrorResponse::with_stack(
                500,
                "Failed to get alkanes".to_string(),
                e.to_string(),
            );
            HttpResponse::InternalServerError().json(error)
        }
    }
}

pub async fn get_alkanes_by_address(
    state: web::Data<AppState>,
    req: web::Json<AddressRequest>,
) -> impl Responder {
    if req.address.is_empty() {
        return HttpResponse::BadRequest().json(ErrorResponse::new(
            400,
            "address is required".to_string(),
        ));
    }

    let alkanes_service = AlkanesService::new(
        state.alkanes_rpc.clone(),
        state.redis_client.clone(),
        state.db_pool.clone(),
    );

    match alkanes_service
        .get_alkanes_by_address(&req.address, true)
        .await
    {
        Ok(alkanes) => {
            let response = ApiResponse::ok(alkanes);
            HttpResponse::Ok().json(response)
        }
        Err(e) => {
            let error = ErrorResponse::with_stack(
                500,
                "Failed to get alkanes by address".to_string(),
                e.to_string(),
            );
            HttpResponse::InternalServerError().json(error)
        }
    }
}

pub async fn get_alkane_details(
    state: web::Data<AppState>,
    req: web::Json<AlkaneDetailsRequest>,
) -> impl Responder {
    let alkanes_service = AlkanesService::new(
        state.alkanes_rpc.clone(),
        state.redis_client.clone(),
        state.db_pool.clone(),
    );

    match alkanes_service
        .get_alkane_details(&(&req.alkane_id).into())
        .await
    {
        Ok(alkane) => {
            let response = ApiResponse::ok(alkane);
            HttpResponse::Ok().json(response)
        }
        Err(e) => {
            let error = ErrorResponse::with_stack(
                500,
                "Failed to get alkane details".to_string(),
                e.to_string(),
            );
            HttpResponse::InternalServerError().json(error)
        }
    }
}

pub async fn get_alkanes_utxo(
    state: web::Data<AppState>,
    req: web::Json<AddressRequest>,
) -> impl Responder {
    if req.address.is_empty() {
        return HttpResponse::BadRequest().json(ErrorResponse::new(
            400,
            "address is required".to_string(),
        ));
    }

    let alkanes_service = AlkanesService::new(
        state.alkanes_rpc.clone(),
        state.redis_client.clone(),
        state.db_pool.clone(),
    );

    match alkanes_service.get_alkanes_utxos(&req.address).await {
        Ok(utxos) => {
            let response = ApiResponse::ok(utxos);
            HttpResponse::Ok().json(response)
        }
        Err(e) => {
            let error = ErrorResponse::with_stack(
                500,
                "Failed to get alkanes UTXOs".to_string(),
                e.to_string(),
            );
            HttpResponse::InternalServerError().json(error)
        }
    }
}

pub async fn get_amm_utxos(
    state: web::Data<AppState>,
    req: web::Json<AddressRequest>,
) -> impl Responder {
    if req.address.is_empty() {
        return HttpResponse::BadRequest().json(ErrorResponse::new(
            400,
            "address is required".to_string(),
        ));
    }

    let alkanes_service = AlkanesService::new(
        state.alkanes_rpc.clone(),
        state.redis_client.clone(),
        state.db_pool.clone(),
    );
    let bitcoin_service = crate::services::bitcoin::BitcoinService::new(state.alkanes_rpc.clone());

    match bitcoin_service
        .get_amm_utxos(&req.address, &alkanes_service)
        .await
    {
        Ok(utxos) => {
            let response = ApiResponse::ok(json!({ "utxos": utxos }));
            HttpResponse::Ok().json(response)
        }
        Err(e) => {
            let error = ErrorResponse::with_stack(
                500,
                "Failed to get AMM UTXOs".to_string(),
                e.to_string(),
            );
            HttpResponse::InternalServerError().json(error)
        }
    }
}

pub async fn global_alkanes_search(
    state: web::Data<AppState>,
    req: web::Json<SearchRequest>,
) -> impl Responder {
    if req.query.is_empty() {
        return HttpResponse::BadRequest().json(ErrorResponse::new(
            400,
            "query is required".to_string(),
        ));
    }

    let alkanes_service = AlkanesService::new(
        state.alkanes_rpc.clone(),
        state.redis_client.clone(),
        state.db_pool.clone(),
    );

    match alkanes_service.global_search(&req.query).await {
        Ok(results) => {
            let response = ApiResponse::ok(json!({
                "tokens": results,
                "pools": []  // TODO: Add pool search
            }));
            HttpResponse::Ok().json(response)
        }
        Err(e) => {
            let error = ErrorResponse::with_stack(
                500,
                "Failed to search alkanes".to_string(),
                e.to_string(),
            );
            HttpResponse::InternalServerError().json(error)
        }
    }
}
