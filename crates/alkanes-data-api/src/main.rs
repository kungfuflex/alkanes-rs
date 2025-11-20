use actix_cors::Cors;
use actix_web::{middleware as actix_middleware, web, App, HttpServer};
use anyhow::Result;
use dotenvy::dotenv;
use std::env;

mod config;
mod handlers;
mod models;
mod services;

use config::Config;

#[actix_web::main]
async fn main() -> Result<()> {
    dotenv().ok();
    env_logger::init();

    let config = Config::from_env()?;
    let port = config.port;
    let host = config.host.clone();

    log::info!("Starting alkanes-data-api on {}:{}", host, port);

    // Initialize services
    let db_pool = services::database::create_pool(&config.database_url).await?;
    let redis_client = services::redis::create_client(&config.redis_url)?;
    let price_service = services::price::PriceService::new(&config.infura_endpoint)?;
    let alkanes_rpc = services::alkanes_rpc::AlkanesRpcClient::new(&config)?;

    let app_state = web::Data::new(services::AppState {
        config: config.clone(),
        db_pool,
        redis_client,
        price_service,
        alkanes_rpc,
    });

    HttpServer::new(move || {
        let cors = Cors::permissive();

        App::new()
            .app_data(app_state.clone())
            .wrap(cors)
            .wrap(actix_middleware::Logger::default())
            .service(
                web::scope("/api/v1")
                    .route("/health", web::get().to(handlers::health::health_check))
                    // Bitcoin price endpoints
                    .route(
                        "/get-bitcoin-price",
                        web::post().to(handlers::price::get_bitcoin_price),
                    )
                    .route(
                        "/get-bitcoin-market-chart",
                        web::post().to(handlers::price::get_bitcoin_market_chart),
                    )
                    .route(
                        "/get-bitcoin-market-weekly",
                        web::post().to(handlers::price::get_bitcoin_market_weekly),
                    )
                    .route(
                        "/get-bitcoin-markets",
                        web::post().to(handlers::price::get_bitcoin_markets),
                    )
                    // Alkanes endpoints
                    .route(
                        "/get-alkanes",
                        web::post().to(handlers::alkanes::get_alkanes),
                    )
                    .route(
                        "/get-alkanes-by-address",
                        web::post().to(handlers::alkanes::get_alkanes_by_address),
                    )
                    .route(
                        "/get-alkane-details",
                        web::post().to(handlers::alkanes::get_alkane_details),
                    )
                    .route(
                        "/get-alkanes-utxo",
                        web::post().to(handlers::alkanes::get_alkanes_utxo),
                    )
                    .route(
                        "/get-amm-utxos",
                        web::post().to(handlers::alkanes::get_amm_utxos),
                    )
                    .route(
                        "/global-alkanes-search",
                        web::post().to(handlers::alkanes::global_alkanes_search),
                    )
                    // Pool endpoints
                    .route("/get-pools", web::post().to(handlers::pools::get_pools))
                    .route(
                        "/get-pool-details",
                        web::post().to(handlers::pools::get_pool_details),
                    )
                    .route(
                        "/get-all-pools-details",
                        web::post().to(handlers::pools::get_all_pools_details),
                    )
                    .route(
                        "/address-positions",
                        web::post().to(handlers::pools::address_positions),
                    )
                    .route(
                        "/get-all-token-pairs",
                        web::post().to(handlers::pools::get_all_token_pairs),
                    )
                    .route(
                        "/get-token-pairs",
                        web::post().to(handlers::pools::get_token_pairs),
                    )
                    .route(
                        "/get-alkane-swap-pair-details",
                        web::post().to(handlers::pools::get_alkane_swap_pair_details),
                    )
                    // History endpoints
                    .route(
                        "/get-pool-swap-history",
                        web::post().to(handlers::history::get_pool_swap_history),
                    )
                    .route(
                        "/get-token-swap-history",
                        web::post().to(handlers::history::get_token_swap_history),
                    )
                    .route(
                        "/get-pool-mint-history",
                        web::post().to(handlers::history::get_pool_mint_history),
                    )
                    .route(
                        "/get-pool-burn-history",
                        web::post().to(handlers::history::get_pool_burn_history),
                    )
                    .route(
                        "/get-pool-creation-history",
                        web::post().to(handlers::history::get_pool_creation_history),
                    )
                    .route(
                        "/get-address-swap-history-for-pool",
                        web::post().to(handlers::history::get_address_swap_history_for_pool),
                    )
                    .route(
                        "/get-address-swap-history-for-token",
                        web::post().to(handlers::history::get_address_swap_history_for_token),
                    )
                    .route(
                        "/get-address-wrap-history",
                        web::post().to(handlers::history::get_address_wrap_history),
                    )
                    .route(
                        "/get-address-unwrap-history",
                        web::post().to(handlers::history::get_address_unwrap_history),
                    )
                    .route(
                        "/get-all-wrap-history",
                        web::post().to(handlers::history::get_all_wrap_history),
                    )
                    .route(
                        "/get-all-unwrap-history",
                        web::post().to(handlers::history::get_all_unwrap_history),
                    )
                    .route(
                        "/get-total-unwrap-amount",
                        web::post().to(handlers::history::get_total_unwrap_amount),
                    )
                    .route(
                        "/get-address-pool-creation-history",
                        web::post().to(handlers::history::get_address_pool_creation_history),
                    )
                    .route(
                        "/get-address-pool-mint-history",
                        web::post().to(handlers::history::get_address_pool_mint_history),
                    )
                    .route(
                        "/get-address-pool-burn-history",
                        web::post().to(handlers::history::get_address_pool_burn_history),
                    )
                    .route(
                        "/get-all-address-amm-tx-history",
                        web::post().to(handlers::history::get_all_address_amm_tx_history),
                    )
                    .route(
                        "/get-all-amm-tx-history",
                        web::post().to(handlers::history::get_all_amm_tx_history),
                    )
                    // Bitcoin/UTXO endpoints
                    .route(
                        "/get-address-balance",
                        web::post().to(handlers::bitcoin::get_address_balance),
                    )
                    .route(
                        "/get-taproot-balance",
                        web::post().to(handlers::bitcoin::get_taproot_balance),
                    )
                    .route(
                        "/get-address-utxos",
                        web::post().to(handlers::bitcoin::get_address_utxos),
                    )
                    .route(
                        "/get-account-utxos",
                        web::post().to(handlers::bitcoin::get_account_utxos),
                    )
                    .route(
                        "/get-account-balance",
                        web::post().to(handlers::bitcoin::get_account_balance),
                    )
                    .route(
                        "/get-taproot-history",
                        web::post().to(handlers::bitcoin::get_taproot_history),
                    )
                    .route(
                        "/get-intent-history",
                        web::post().to(handlers::bitcoin::get_intent_history),
                    ),
            )
    })
    .bind((host.as_str(), port))?
    .run()
    .await?;

    Ok(())
}
