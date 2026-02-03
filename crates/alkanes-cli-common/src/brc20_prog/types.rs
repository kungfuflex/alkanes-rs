// Types for BRC20-Prog functionality

use serde::{Deserialize, Serialize};
#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};
#[cfg(feature = "std")]
use std::{string::String, vec::Vec};

/// Anti-frontrunning strategy for BRC20-Prog transactions
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AntiFrontrunningStrategy {
    /// Use CheckLockTimeVerify to timelock reveal transaction
    CheckLockTimeVerify,
    /// Use Child-Pays-For-Parent to accelerate commit transaction
    Cpfp,
    /// Pre-sign all transactions and broadcast together
    Presign,
    /// Monitor mempool and use RBF to bump fees if frontrunning detected
    Rbf,
}

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
    /// Use 3-transaction activation pattern (commit-reveal-activation) instead of 2-transaction (commit-reveal)
    pub use_activation: bool,
    /// Use MARA Slipstream service for broadcasting
    pub use_slipstream: bool,
    /// Use Rebar Shield for private transaction relay
    pub use_rebar: bool,
    /// Rebar fee tier (1 or 2, default: 1). Tier 1: ~8% hashrate, Tier 2: ~16% hashrate
    pub rebar_tier: Option<u8>,
    /// Anti-frontrunning strategy to use
    pub strategy: Option<AntiFrontrunningStrategy>,
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
    /// Use MARA Slipstream service for broadcasting
    pub use_slipstream: bool,
    /// Use Rebar Shield for private transaction relay
    pub use_rebar: bool,
    /// Rebar fee tier (1 or 2, default: 1). Tier 1: ~8% hashrate, Tier 2: ~16% hashrate
    pub rebar_tier: Option<u8>,
    /// Anti-frontrunning strategy to use
    pub strategy: Option<AntiFrontrunningStrategy>,
}

/// Additional output to include in the activation transaction
/// Used for FrBTC wrap/unwrap operations where BTC must be sent to the signer address
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdditionalOutput {
    /// Destination address (p2tr, p2wpkh, etc.)
    pub address: String,
    /// Amount in satoshis
    pub amount: u64,
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
    /// Use 3-transaction activation pattern (commit-reveal-activation) instead of 2-transaction (commit-reveal)
    pub use_activation: bool,
    /// Use MARA Slipstream service for broadcasting
    pub use_slipstream: bool,
    /// Use Rebar Shield for private transaction relay
    pub use_rebar: bool,
    /// Rebar fee tier (1 or 2, default: 1). Tier 1: ~8% hashrate, Tier 2: ~16% hashrate
    pub rebar_tier: Option<u8>,
    /// Anti-frontrunning strategy to use
    pub strategy: Option<AntiFrontrunningStrategy>,
    /// Resume from existing commit transaction (commit txid)
    pub resume_from_commit: Option<String>,
    /// Additional outputs to include in the activation transaction
    /// Used for FrBTC wrap (send BTC to signer) or unwrap (dust to signer)
    pub additional_outputs: Option<Vec<AdditionalOutput>>,
    /// Enable mempool indexer for tracing inscription state of pending UTXOs
    /// When enabled, if we must use pending (unconfirmed) UTXOs, we'll trace back through
    /// parent transactions to determine inscription state from settled UTXOs
    #[serde(default)]
    pub mempool_indexer: bool,
    /// Mint DIESEL tokens in commit and reveal transactions
    #[serde(default)]
    pub mint_diesel: bool,
    /// Return unsigned PSBTs instead of signing and broadcasting
    /// When true, the execute method will return unsigned PSBTs that can be signed
    /// by an external signer (e.g., browser wallet)
    #[serde(default)]
    pub return_unsigned: bool,
}

/// Result of a BRC20-prog execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Brc20ProgExecuteResult {
    /// Split transaction ID (if inscribed UTXOs were split)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub split_txid: Option<String>,
    /// Split transaction fee (if split was needed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub split_fee: Option<u64>,
    /// Commit transaction ID
    pub commit_txid: String,
    /// Reveal transaction ID
    pub reveal_txid: String,
    /// Activation transaction ID (for deploy operations)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub activation_txid: Option<String>,
    /// Commit transaction fee
    pub commit_fee: u64,
    /// Reveal transaction fee
    pub reveal_fee: u64,
    /// Activation transaction fee (for deploy operations)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub activation_fee: Option<u64>,
    /// Inputs used in the transactions
    pub inputs_used: Vec<String>,
    /// Outputs created in the transactions
    pub outputs_created: Vec<String>,
    /// Trace results (if tracing was enabled)
    pub traces: Option<Vec<serde_json::Value>>,

    // === EXTERNAL SIGNER SUPPORT ===
    // When return_unsigned=true, these fields contain PSBTs/transactions for external signing.
    // NOTE: The reveal transaction is signed INTERNALLY with the ephemeral key (the SDK
    // generates this key and only it knows the secret). External signers (browser wallets)
    // only need to sign: split (if any), commit, and activation (if any).

    /// Unsigned split PSBT (base64, if split was needed) - sign with user wallet
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unsigned_split_psbt: Option<String>,
    /// Unsigned commit PSBT (base64) - sign with user wallet
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unsigned_commit_psbt: Option<String>,
    /// Unsigned reveal PSBT (base64) - DEPRECATED: reveal is now signed internally
    /// This field is kept for backwards compatibility but will always be None
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unsigned_reveal_psbt: Option<String>,
    /// Signed reveal transaction hex - ready to broadcast after commit confirms
    /// The reveal is signed internally with the ephemeral key (user wallet cannot sign it)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signed_reveal_tx_hex: Option<String>,
    /// Unsigned activation PSBT (base64, if activation is used) - sign with user wallet
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unsigned_activation_psbt: Option<String>,
    /// Whether this result contains unsigned PSBTs (for external signing)
    #[serde(default)]
    pub requires_signing: bool,
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
