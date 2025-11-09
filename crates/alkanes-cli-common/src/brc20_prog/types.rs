// Types for BRC20-Prog functionality

use serde::{Deserialize, Serialize};
#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};
#[cfg(feature = "std")]
use std::{string::String, vec::Vec};

/// Parameters for deploying a BRC20-prog contract
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Brc20ProgDeployParams {
    /// Path to Foundry build JSON file containing the contract bytecode
    pub foundry_json_path: String,
    /// Addresses to source UTXOs from
    pub from_addresses: Option<Vec<String>>,
    /// Change address
    pub change_address: Option<String>,
    /// Fee rate in sat/vB
    pub fee_rate: Option<f32>,
    /// Show raw JSON output
    pub raw_output: bool,
    /// Enable transaction tracing
    pub trace_enabled: bool,
    /// Mine a block after broadcasting (regtest only)
    pub mine_enabled: bool,
    /// Automatically confirm the transaction preview
    pub auto_confirm: bool,
}

/// Parameters for calling a BRC20-prog contract (transact subcommand)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Brc20ProgTransactParams {
    /// Contract address to call (0x prefixed hex)
    pub address: String,
    /// Function signature (e.g., "transfer(address,uint256)")
    pub signature: String,
    /// Calldata arguments as comma-separated values
    pub calldata: String,
    /// Addresses to source UTXOs from
    pub from_addresses: Option<Vec<String>>,
    /// Change address
    pub change_address: Option<String>,
    /// Fee rate in sat/vB
    pub fee_rate: Option<f32>,
    /// Show raw JSON output
    pub raw_output: bool,
    /// Enable transaction tracing
    pub trace_enabled: bool,
    /// Mine a block after broadcasting (regtest only)
    pub mine_enabled: bool,
    /// Automatically confirm the transaction preview
    pub auto_confirm: bool,
}

/// Generic execution parameters for BRC20-prog operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Brc20ProgExecuteParams {
    /// The inscription content (JSON payload)
    pub inscription_content: String,
    /// Addresses to source UTXOs from
    pub from_addresses: Option<Vec<String>>,
    /// Change address
    pub change_address: Option<String>,
    /// Fee rate in sat/vB
    pub fee_rate: Option<f32>,
    /// Show raw JSON output
    pub raw_output: bool,
    /// Enable transaction tracing
    pub trace_enabled: bool,
    /// Mine a block after broadcasting (regtest only)
    pub mine_enabled: bool,
    /// Automatically confirm the transaction preview
    pub auto_confirm: bool,
}

/// Result of a BRC20-prog execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Brc20ProgExecuteResult {
    /// Commit transaction ID
    pub commit_txid: String,
    /// Reveal transaction ID
    pub reveal_txid: String,
    /// Commit transaction fee
    pub commit_fee: u64,
    /// Reveal transaction fee
    pub reveal_fee: u64,
    /// Inputs used in the transactions
    pub inputs_used: Vec<String>,
    /// Outputs created in the transactions
    pub outputs_created: Vec<String>,
    /// Trace results (if tracing was enabled)
    pub traces: Option<Vec<serde_json::Value>>,
}

/// Type of BRC20-prog inscription
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Brc20ProgInscriptionType {
    /// Deploy a new contract
    Deploy,
    /// Call an existing contract
    Call,
    /// Send a raw signed transaction
    Transact,
}

/// BRC20-prog deploy inscription JSON structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Brc20ProgDeployInscription {
    /// Protocol identifier (always "brc20-prog")
    pub p: String,
    /// Operation (always "deploy" or "d")
    pub op: String,
    /// Contract bytecode + constructor args in hex (0x prefixed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub d: Option<String>,
    /// Base64 encoded bytecode with compression prefix
    #[serde(skip_serializing_if = "Option::is_none")]
    pub b: Option<String>,
}

impl Brc20ProgDeployInscription {
    pub fn new(bytecode_hex: String) -> Self {
        Self {
            p: "brc20-prog".to_string(),
            op: "deploy".to_string(),
            d: Some(bytecode_hex),
            b: None,
        }
    }
}

/// BRC20-prog call inscription JSON structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Brc20ProgCallInscription {
    /// Protocol identifier (always "brc20-prog")
    pub p: String,
    /// Operation (always "call" or "c")
    pub op: String,
    /// Contract address to call (0x prefixed hex)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub c: Option<String>,
    /// Inscription ID of the contract to call (alternative to "c")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub i: Option<String>,
    /// Call data (function selector + args) in hex (0x prefixed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub d: Option<String>,
    /// Base64 encoded call data with compression prefix
    #[serde(skip_serializing_if = "Option::is_none")]
    pub b: Option<String>,
}

impl Brc20ProgCallInscription {
    pub fn new(contract_address: String, calldata_hex: String) -> Self {
        Self {
            p: "brc20-prog".to_string(),
            op: "call".to_string(),
            c: Some(contract_address),
            i: None,
            d: Some(calldata_hex),
            b: None,
        }
    }
}

/// BRC20-prog transact inscription JSON structure (raw signed tx)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Brc20ProgTransactInscription {
    /// Protocol identifier (always "brc20-prog")
    pub p: String,
    /// Operation (always "transact" or "t")
    pub op: String,
    /// Raw signed transaction data in hex (0x prefixed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub d: Option<String>,
    /// Base64 encoded raw transaction data with compression prefix
    #[serde(skip_serializing_if = "Option::is_none")]
    pub b: Option<String>,
}

impl Brc20ProgTransactInscription {
    pub fn new(raw_tx_hex: String) -> Self {
        Self {
            p: "brc20-prog".to_string(),
            op: "transact".to_string(),
            d: Some(raw_tx_hex),
            b: None,
        }
    }
}
