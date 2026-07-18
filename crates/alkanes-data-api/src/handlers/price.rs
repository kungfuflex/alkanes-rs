use actix_web::{web, HttpResponse, Responder};
use serde_json::json;

use crate::{
    models::{ApiResponse, ErrorResponse, MarketChartRequest},
    services::AppState,
};

pub async fn get_bitcoin_price(state: web::Data<AppState>) -> impl Responder {
    match state.price_service.get_bitcoin_price().await {
        Ok(price) => {
            let response = ApiResponse::ok(json!({
                "bitcoin": {
                    "usd": price
                }
            }));
            HttpResponse::Ok().json(response)
        }
        Err(e) => {
            let error = ErrorResponse::with_stack(
                500,
                "Failed to get bitcoin price".to_string(),
                e.to_string(),
            );
            HttpResponse::InternalServerError().json(error)
        }
    }
}

pub async fn get_bitcoin_market_chart(
    state: web::Data<AppState>,
    req: web::Json<MarketChartRequest>,
) -> impl Responder {
    let days: u32 = match req.days.parse() {
        Ok(d) => d,
        Err(_) => {
            return HttpResponse::BadRequest().json(ErrorResponse::new(
                400,
                "days must be a valid number".to_string(),
            ));
        }
    };

    match state.price_service.get_market_chart(days).await {
        Ok(data) => {
            let response = ApiResponse::ok(data);
            HttpResponse::Ok().json(response)
        }
        Err(e) => {
            let error = ErrorResponse::with_stack(
                500,
                "Failed to get market chart".to_string(),
                e.to_string(),
            );
            HttpResponse::InternalServerError().json(error)
        }
    }
}

pub async fn get_bitcoin_market_weekly(state: web::Data<AppState>) -> impl Responder {
    match state.price_service.get_market_52w().await {
        Ok(data) => {
            let response = ApiResponse::ok(data);
            HttpResponse::Ok().json(response)
        }
        Err(e) => {
            let error = ErrorResponse::with_stack(
                500,
                "Failed to get bitcoin market weekly".to_string(),
                e.to_string(),
            );
            HttpResponse::InternalServerError().json(error)
        }
    }
}

pub async fn get_bitcoin_markets(state: web::Data<AppState>) -> impl Responder {
    match state.price_service.get_markets().await {
        Ok(data) => {
            let response = ApiResponse::ok(data);
            HttpResponse::Ok().json(response)
        }
        Err(e) => {
            let error = ErrorResponse::with_stack(
                500,
                "Failed to get bitcoin markets".to_string(),
                e.to_string(),
            );
            HttpResponse::InternalServerError().json(error)
        }
    }
}
