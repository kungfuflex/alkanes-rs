use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;

/// Alkane ID (block:tx)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AlkaneId {
    pub block: i32,
    pub tx: i64,
}

impl AlkaneId {
    pub fn new(block: i32, tx: i64) -> Self {
        Self { block, tx }
    }
    
    pub fn to_string(&self) -> String {
        format!("{}:{}", self.block, self.tx)
    }
    
    pub fn from_string(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() == 2 {
            Some(Self {
                block: parts[0].parse().ok()?,
                tx: parts[1].parse().ok()?,
            })
        } else {
            None
        }
    }
}

/// Trace event from alkanes runtime
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceEvent {
    pub event_type: String,
    pub vout: i32,
    pub alkane_address_block: String,
    pub alkane_address_tx: String,
    pub data: JsonValue,
}

/// Transaction context for processing traces
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionContext {
    pub txid: String,
    pub block_height: i32,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub vouts: Vec<VoutInfo>,
}

/// Output information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoutInfo {
    pub index: i32,
    pub address: Option<String>,
    pub value: u64,
}

/// Query parameters for fetching data
#[derive(Debug, Clone)]
pub struct QueryParams {
    pub filters: HashMap<String, String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    pub order_by: Option<String>,
    pub order_desc: bool,
}

impl Default for QueryParams {
    fn default() -> Self {
        Self {
            filters: HashMap::new(),
            limit: None,
            offset: None,
            order_by: None,
            order_desc: false,
        }
    }
}

/// Filter conditions for queries
#[derive(Debug, Clone)]
pub enum QueryFilter {
    Equals(Vec<u8>),
    In(Vec<Vec<u8>>),
    Range { min: Option<Vec<u8>>, max: Option<Vec<u8>> },
    Prefix(Vec<u8>),
}

/// Generic result type
pub type Result<T> = std::result::Result<T, anyhow::Error>;
