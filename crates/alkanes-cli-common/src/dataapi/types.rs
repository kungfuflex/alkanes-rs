use serde::{Deserialize, Serialize};
use crate::alkanes::types::AlkaneId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlkaneToken {
    pub id: AlkaneId,
    pub name: Option<String>,
    pub symbol: Option<String>,
    pub decimals: Option<u32>,
    pub image: Option<String>,
    pub max: Option<String>,
    pub cap: Option<String>,
    pub premine: Option<String>,
    pub balance: Option<String>,
    #[serde(rename = "floorPrice")]
    pub floor_price: Option<f64>,
    #[serde(rename = "priceUsd")]
    pub price_usd: Option<f64>,
    #[serde(rename = "priceInSatoshi")]
    pub price_in_satoshi: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pool {
    pub id: String,
    pub factory_block_id: String,
    pub factory_tx_id: String,
    pub pool_block_id: String,
    pub pool_tx_id: String,
    pub token0_block_id: String,
    pub token0_tx_id: String,
    pub token1_block_id: String,
    pub token1_tx_id: String,
    pub pool_name: String,
    pub token0_amount: Option<String>,
    pub token1_amount: Option<String>,
    pub token_supply: Option<String>,
    pub creator_address: Option<String>,
    pub creation_block_height: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolCreation {
    pub id: String,
    pub transaction_id: String,
    pub block_height: i32,
    pub transaction_index: i32,
    pub pool_block_id: String,
    pub pool_tx_id: String,
    pub token0_block_id: String,
    pub token0_tx_id: String,
    pub token1_block_id: String,
    pub token1_tx_id: String,
    pub token0_amount: String,
    pub token1_amount: String,
    pub token_supply: String,
    pub creator_address: Option<String>,
    pub successful: bool,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolSwap {
    pub id: String,
    pub transaction_id: String,
    pub block_height: i32,
    pub transaction_index: i32,
    pub pool_block_id: String,
    pub pool_tx_id: String,
    pub sold_token_block_id: String,
    pub sold_token_tx_id: String,
    pub bought_token_block_id: String,
    pub bought_token_tx_id: String,
    pub sold_amount: f64,
    pub bought_amount: f64,
    pub seller_address: Option<String>,
    pub successful: bool,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolMint {
    pub id: String,
    pub transaction_id: String,
    pub block_height: i32,
    pub transaction_index: i32,
    pub pool_block_id: String,
    pub pool_tx_id: String,
    pub lp_token_amount: String,
    pub token0_block_id: String,
    pub token0_tx_id: String,
    pub token1_block_id: String,
    pub token1_tx_id: String,
    pub token0_amount: String,
    pub token1_amount: String,
    pub minter_address: Option<String>,
    pub successful: bool,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolBurn {
    pub id: String,
    pub transaction_id: String,
    pub block_height: i32,
    pub transaction_index: i32,
    pub pool_block_id: String,
    pub pool_tx_id: String,
    pub lp_token_amount: String,
    pub token0_block_id: String,
    pub token0_tx_id: String,
    pub token1_block_id: String,
    pub token1_tx_id: String,
    pub token0_amount: String,
    pub token1_amount: String,
    pub burner_address: Option<String>,
    pub successful: bool,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitcoinPrice {
    pub usd: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketChart {
    pub prices: Vec<[f64; 2]>,
    pub market_caps: Vec<[f64; 2]>,
    pub total_volumes: Vec<[f64; 2]>,
}

// Response wrappers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    #[serde(rename = "statusCode")]
    pub status_code: u16,
    pub data: T,
    pub error: Option<String>,
    pub stack: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlkanesResponse {
    pub count: usize,
    pub tokens: Vec<AlkaneToken>,
    pub total: usize,
    pub limit: Option<i32>,
    pub offset: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolsResponse {
    pub pools: Vec<Pool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolCreationHistoryResponse {
    pub creations: Vec<PoolCreation>,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwapHistoryResponse {
    pub swaps: Vec<PoolSwap>,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MintHistoryResponse {
    pub mints: Vec<PoolMint>,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BurnHistoryResponse {
    pub burns: Vec<PoolBurn>,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryResponse {
    pub transactions: Vec<HistoryTransaction>,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum HistoryTransaction {
    Swap(PoolSwap),
    Mint(PoolMint),
    Burn(PoolBurn),
    Creation(PoolCreation),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitcoinPriceResponse {
    pub bitcoin: BitcoinPrice,
}

// Helper response types for pretty printing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolHistoryResponse {
    pub swaps: Vec<PoolSwap>,
    pub mints: Vec<PoolMint>,
    pub burns: Vec<PoolBurn>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketChartResponse {
    pub prices: Vec<Vec<f64>>,
    pub market_caps: Vec<Vec<f64>>,
    pub total_volumes: Vec<Vec<f64>>,
}

// Indexer status response types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexerBlockHeightResponse {
    pub height: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexerBlockHashResponse {
    pub hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexerPositionResponse {
    pub height: i64,
    pub hash: String,
}
