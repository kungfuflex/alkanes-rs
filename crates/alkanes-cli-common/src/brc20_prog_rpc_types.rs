//! Types for BRC20-Prog RPC responses
//! 
//! These types mirror the response structures from brc20-programmable-module
//! and are designed for pretty-printing in the CLI

use serde::{Deserialize, Serialize};
use alloc::string::String;
use alloc::vec::Vec;

/// EthCall parameters for eth_call and eth_estimateGas
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EthCallParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    pub to: String,
    pub data: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gas: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gas_price: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

/// Block information response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockInfo {
    pub number: String,
    pub hash: String,
    pub parent_hash: String,
    pub timestamp: String,
    pub transactions: Vec<serde_json::Value>,
    pub transactions_root: String,
    pub receipts_root: String,
    pub state_root: String,
    pub gas_limit: String,
    pub gas_used: String,
    pub base_fee_per_gas: Option<String>,
}

/// Transaction information response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionInfo {
    pub hash: String,
    pub nonce: String,
    pub block_hash: String,
    pub block_number: String,
    pub transaction_index: String,
    pub from: String,
    pub to: Option<String>,
    pub value: String,
    pub gas: String,
    pub gas_price: String,
    pub input: String,
    pub v: String,
    pub r: String,
    pub s: String,
}

/// Transaction receipt response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionReceipt {
    pub transaction_hash: String,
    pub transaction_index: String,
    pub block_hash: String,
    pub block_number: String,
    pub from: String,
    pub to: Option<String>,
    pub cumulative_gas_used: String,
    pub gas_used: String,
    pub contract_address: Option<String>,
    pub logs: Vec<LogEntry>,
    pub logs_bloom: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effective_gas_price: Option<String>,
}

/// Log entry in transaction receipt
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    pub address: String,
    pub topics: Vec<String>,
    pub data: String,
    pub block_number: String,
    pub transaction_hash: String,
    pub transaction_index: String,
    pub block_hash: String,
    pub log_index: String,
    pub removed: bool,
}

/// Get logs filter parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetLogsFilter {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_block: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_block: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topics: Option<Vec<Option<Vec<String>>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_hash: Option<String>,
}

/// Trace response for debug_traceTransaction
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceInfo {
    pub gas: u64,
    pub failed: bool,
    pub return_value: String,
    pub struct_logs: Vec<StructLog>,
}

/// Struct log entry in trace
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StructLog {
    pub pc: u64,
    pub op: String,
    pub gas: u64,
    pub gas_cost: u64,
    pub depth: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stack: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage: Option<serde_json::Value>,
}

/// Txpool content response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxpoolContent {
    pub pending: serde_json::Value,
    pub queued: serde_json::Value,
}
