use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    #[serde(rename = "statusCode")]
    pub status_code: u16,
    pub data: T,
}

impl<T> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        Self {
            status_code: 200,
            data,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    #[serde(rename = "statusCode")]
    pub status_code: u16,
    pub error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stack: Option<String>,
}

impl ErrorResponse {
    pub fn new(status_code: u16, error: String) -> Self {
        Self {
            status_code,
            error,
            stack: None,
        }
    }

    pub fn with_stack(status_code: u16, error: String, stack: String) -> Self {
        Self {
            status_code,
            error,
            stack: Some(stack),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AlkaneId {
    pub block: String,
    pub tx: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PoolId {
    pub block: String,
    pub tx: String,
}

// Request/Response types for various endpoints

#[derive(Debug, Deserialize)]
pub struct AddressRequest {
    pub address: String,
}

#[derive(Debug, Deserialize)]
pub struct AlkaneDetailsRequest {
    #[serde(rename = "alkaneId")]
    pub alkane_id: AlkaneId,
}

#[derive(Debug, Deserialize)]
pub struct PoolDetailsRequest {
    #[serde(rename = "poolId")]
    pub pool_id: PoolId,
}

#[derive(Debug, Deserialize)]
pub struct SearchRequest {
    pub query: String,
    #[serde(default)]
    pub limit: Option<i32>,
    #[serde(default)]
    pub offset: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct PaginationRequest {
    #[serde(default)]
    pub limit: Option<i32>,
    #[serde(default)]
    pub offset: Option<i32>,
    #[serde(default)]
    pub count: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct MarketChartRequest {
    pub days: String,
}

#[derive(Debug, Deserialize)]
pub struct HistoryRequest {
    #[serde(rename = "poolId")]
    pub pool_id: Option<PoolId>,
    #[serde(rename = "alkaneId")]
    pub alkane_id: Option<AlkaneId>,
    pub address: Option<String>,
    pub count: Option<i32>,
    pub offset: Option<i32>,
    pub successful: Option<bool>,
    #[serde(rename = "includeTotal")]
    pub include_total: Option<bool>,
    #[serde(rename = "transactionType")]
    pub transaction_type: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TokenPairsRequest {
    #[serde(rename = "factoryId")]
    pub factory_id: AlkaneId,
    #[serde(rename = "alkaneId")]
    pub alkane_id: Option<AlkaneId>,
    #[serde(rename = "sort_by")]
    pub sort_by: Option<String>,
    pub limit: Option<i32>,
    pub offset: Option<i32>,
    #[serde(rename = "searchQuery")]
    pub search_query: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SwapPairDetailsRequest {
    #[serde(rename = "factoryId")]
    pub factory_id: AlkaneId,
    #[serde(rename = "tokenAId")]
    pub token_a_id: AlkaneId,
    #[serde(rename = "tokenBId")]
    pub token_b_id: AlkaneId,
}

#[derive(Debug, Deserialize)]
pub struct TaprootHistoryRequest {
    #[serde(rename = "taprootAddress")]
    pub taproot_address: String,
    #[serde(rename = "totalTxs")]
    pub total_txs: i32,
    pub testnet: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UtxoRequest {
    pub address: Option<String>,
    pub account: Option<String>,
    #[serde(rename = "spendStrategy")]
    pub spend_strategy: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct IntentHistoryRequest {
    pub address: String,
    pub testnet: Option<bool>,
    #[serde(rename = "lastSeenTxId")]
    pub last_seen_tx_id: Option<String>,
    #[serde(rename = "totalTxs")]
    pub total_txs: Option<i32>,
}
