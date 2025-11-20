use actix_web::{web, HttpResponse, Responder};
use serde_json::json;

use crate::{
    models::{
        AddressRequest, ApiResponse, ErrorResponse, IntentHistoryRequest, TaprootHistoryRequest,
        UtxoRequest,
    },
    services::{bitcoin::BitcoinService, AppState},
};

pub async fn get_address_balance(
    state: web::Data<AppState>,
    req: web::Json<AddressRequest>,
) -> impl Responder {
    if req.address.is_empty() {
        return HttpResponse::BadRequest().json(ErrorResponse::new(
            400,
            "address is required".to_string(),
        ));
    }

    let bitcoin_service = BitcoinService::new(state.alkanes_rpc.clone());

    match bitcoin_service.get_address_balance(&req.address).await {
        Ok(balance) => {
            let response = ApiResponse::ok(balance);
            HttpResponse::Ok().json(response)
        }
        Err(e) => {
            let error = ErrorResponse::with_stack(
                500,
                "Failed to get address balance".to_string(),
                e.to_string(),
            );
            HttpResponse::InternalServerError().json(error)
        }
    }
}

pub async fn get_taproot_balance(
    state: web::Data<AppState>,
    req: web::Json<AddressRequest>,
) -> impl Responder {
    if req.address.is_empty() {
        return HttpResponse::BadRequest().json(ErrorResponse::new(
            400,
            "address is required".to_string(),
        ));
    }

    let bitcoin_service = BitcoinService::new(state.alkanes_rpc.clone());

    match bitcoin_service.get_address_balance(&req.address).await {
        Ok(balance) => {
            let response = ApiResponse::ok(json!({
                "balance": balance.balance
            }));
            HttpResponse::Ok().json(response)
        }
        Err(e) => {
            let error = ErrorResponse::with_stack(
                500,
                "Failed to get taproot balance".to_string(),
                e.to_string(),
            );
            HttpResponse::InternalServerError().json(error)
        }
    }
}

pub async fn get_address_utxos(
    state: web::Data<AppState>,
    req: web::Json<UtxoRequest>,
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

    let bitcoin_service = BitcoinService::new(state.alkanes_rpc.clone());

    match bitcoin_service.get_address_utxos(address, req.spend_strategy.clone()).await {
        Ok(utxos) => {
            let response = ApiResponse::ok(json!({
                "utxos": utxos
            }));
            HttpResponse::Ok().json(response)
        }
        Err(e) => {
            let error = ErrorResponse::with_stack(
                500,
                "Failed to get address UTXOs".to_string(),
                e.to_string(),
            );
            HttpResponse::InternalServerError().json(error)
        }
    }
}

pub async fn get_account_utxos(
    state: web::Data<AppState>,
    req: web::Json<UtxoRequest>,
) -> impl Responder {
    let account = match &req.account {
        Some(acc) => acc,
        None => {
            return HttpResponse::BadRequest().json(ErrorResponse::new(
                400,
                "account is required".to_string(),
            ));
        }
    };

    let bitcoin_service = BitcoinService::new(state.alkanes_rpc.clone());

    // Account is expected to be a string containing multiple addresses
    // For now, treat account as a single address
    match bitcoin_service.get_address_utxos(account, req.spend_strategy.clone()).await {
        Ok(utxos) => {
            let response = ApiResponse::ok(json!({
                "utxos": utxos
            }));
            return HttpResponse::Ok().json(response);
        }
        Err(e) => {
            let error = ErrorResponse::with_stack(
                500,
                "Failed to get account UTXOs".to_string(),
                e.to_string(),
            );
            return HttpResponse::InternalServerError().json(error);
        }
    }

}

pub async fn get_account_balance(
    state: web::Data<AppState>,
    req: web::Json<UtxoRequest>,
) -> impl Responder {
    let account = match &req.account {
        Some(acc) => acc,
        None => {
            return HttpResponse::BadRequest().json(ErrorResponse::new(
                400,
                "account is required".to_string(),
            ));
        }
    };

    let bitcoin_service = BitcoinService::new(state.alkanes_rpc.clone());

    // Account is treated as a single address for balance
    match bitcoin_service.get_address_balance(account).await {
        Ok(balance) => {
            let response = ApiResponse::ok(json!({
                "balance": balance.balance
            }));
            HttpResponse::Ok().json(response)
        }
        Err(e) => {
            let error = ErrorResponse::with_stack(
                500,
                "Failed to get account balance".to_string(),
                e.to_string(),
            );
            HttpResponse::InternalServerError().json(error)
        }
    }
}

pub async fn get_taproot_history(
    state: web::Data<AppState>,
    req: web::Json<TaprootHistoryRequest>,
) -> impl Responder {
    if req.taproot_address.is_empty() || req.total_txs <= 0 {
        return HttpResponse::BadRequest().json(ErrorResponse::new(
            400,
            "taprootAddress and totalTxs > 0 are required".to_string(),
        ));
    }

    let bitcoin_service = BitcoinService::new(state.alkanes_rpc.clone());

    match bitcoin_service
        .get_taproot_history(&req.taproot_address, req.total_txs)
        .await
    {
        Ok(transactions) => {
            let response = ApiResponse::ok(json!({
                "transactions": transactions
            }));
            HttpResponse::Ok().json(response)
        }
        Err(e) => {
            let error = ErrorResponse::with_stack(
                500,
                "Failed to get taproot history".to_string(),
                e.to_string(),
            );
            HttpResponse::InternalServerError().json(error)
        }
    }
}

pub async fn get_intent_history(
    state: web::Data<AppState>,
    req: web::Json<IntentHistoryRequest>,
) -> impl Responder {
    if req.address.is_empty() {
        return HttpResponse::BadRequest().json(ErrorResponse::new(
            400,
            "address is required".to_string(),
        ));
    }

    // Transaction intents are wallet-specific and not stored in the indexer
    // Return the transaction history instead, which can be used for intent tracking
    let bitcoin_service = BitcoinService::new(state.alkanes_rpc.clone());

    let limit = req.total_txs.unwrap_or(50);
    match bitcoin_service.get_taproot_history(&req.address, limit).await {
        Ok(transactions) => {
            let response = ApiResponse::ok(json!({
                "intents": transactions
            }));
            HttpResponse::Ok().json(response)
        }
        Err(e) => {
            let error = ErrorResponse::with_stack(
                500,
                "Failed to get intent history".to_string(),
                e.to_string(),
            );
            HttpResponse::InternalServerError().json(error)
        }
    }
}
