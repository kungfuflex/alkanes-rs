/// OPI (Open Protocol Indexer) types
/// Based on https://github.com/bestinslot-xyz/OPI/blob/main/modules/brc20_api/api.js

use serde::{Deserialize, Serialize};

/// Balance response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Balance {
    pub overall_balance: String,
    pub available_balance: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_height: Option<u64>,
}

/// BRC-20 Event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Brc20Event {
    #[serde(flatten)]
    pub event: serde_json::Value,
    pub event_type: String,
    pub inscription_id: String,
}

/// Activity response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityResponse {
    pub error: Option<String>,
    pub result: Option<Vec<Brc20Event>>,
}

/// Bitcoin RPC result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitcoinRpcResult {
    pub method: String,
    pub request: serde_json::Value,
    pub response: serde_json::Value,
}

/// Unused TX inscription
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnusedTxInscr {
    pub tick: String,
    pub inscription_id: String,
    pub amount: String,
    pub genesis_height: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_holder_pkscript: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_holder_wallet: Option<String>,
}

/// Valid TX notes response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidTxNotesResponse {
    pub unused_txes: Vec<UnusedTxInscr>,
    pub block_height: u64,
}

/// Holder info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Holder {
    pub pkscript: String,
    pub wallet: Option<String>,
    pub overall_balance: String,
    pub available_balance: String,
}

/// Holders response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HoldersResponse {
    pub unused_txes: Vec<Holder>,
    pub block_height: u64,
}

/// Hash response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HashResponse {
    pub cumulative_event_hash: String,
    pub block_event_hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cumulative_trace_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_trace_hash: Option<String>,
    pub indexer_version: String,
    pub block_height: u64,
}

/// Current balances hash response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalancesHashResponse {
    pub current_balances_hash: String,
    pub indexer_version: String,
    pub block_height: u64,
}

/// Generic OPI response wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpiResponse<T> {
    pub error: Option<String>,
    pub result: Option<T>,
}

/// OPI client configuration
#[derive(Debug, Clone)]
pub struct OpiConfig {
    pub base_url: String,
}

impl Default for OpiConfig {
    fn default() -> Self {
        Self {
            base_url: "https://regtest.subfrost.io/v4/opi".to_string(),
        }
    }
}

impl OpiConfig {
    pub fn new(base_url: String) -> Self {
        Self { base_url }
    }

    /// Get default OPI URL for a network
    pub fn default_url_for_network(network: &str) -> String {
        match network {
            "mainnet" => "https://mainnet.subfrost.io/v4/opi".to_string(),
            "signet" => "https://signet.subfrost.io/v4/opi".to_string(),
            "regtest" | "subfrost-regtest" => "https://regtest.subfrost.io/v4/opi".to_string(),
            _ => "https://regtest.subfrost.io/v4/opi".to_string(),
        }
    }
}
