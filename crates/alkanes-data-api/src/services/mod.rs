pub mod alkanes;
pub mod alkanes_rpc;
pub mod bitcoin;
pub mod database;
pub mod history;
pub mod pools;
pub mod price;
pub mod redis;

use crate::config::Config;
use sqlx::PgPool;

pub struct AppState {
    pub config: Config,
    pub db_pool: PgPool,
    pub redis_client: ::redis::Client,
    pub price_service: price::PriceService,
    pub alkanes_rpc: alkanes_rpc::AlkanesRpcClient,
}
