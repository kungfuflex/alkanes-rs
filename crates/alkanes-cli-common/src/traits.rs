//! Trait abstractions for platform-agnostic functionality
//
// This module defines the core traits that allow deezel-common to work
// across different environments (native, WASM, testing) by abstracting
// away platform-specific operations.
//
// The trait system is designed to support the complete deezel functionality:
// - Wallet operations (create, send, balance, UTXOs, etc.)
// - Bitcoin Core RPC operations
// - Metashrew/Sandshrew RPC operations  
// - Alkanes smart contract operations
// - Runestone analysis
// - Protorunes operations
// - Block monitoring
// - Esplora API operations
// - Address resolution
// - Network abstraction

use crate::Result;
use async_trait::async_trait;
use serde_json::Value as JsonValue;
use bitcoin::{Network, Transaction, ScriptBuf, bip32::{DerivationPath, Fingerprint}};
use crate::ord::{
    AddressInfo as OrdAddressInfo, Block as OrdBlock, Blocks as OrdBlocks, Children as OrdChildren,
    Inscription as OrdInscription, Inscriptions as OrdInscriptions, Output as OrdOutput,
    ParentInscriptions as OrdParents, SatResponse as OrdSat, RuneInfo as OrdRuneInfo,
    Runes as OrdRunes, TxInfo as OrdTxInfo,
};
use crate::alkanes::types::{
    EnhancedExecuteParams, EnhancedExecuteResult, ExecutionState, ReadyToSignCommitTx,
    ReadyToSignRevealTx, ReadyToSignTx,
};
use crate::alkanes::protorunes::{ProtoruneOutpointResponse, ProtoruneWalletResponse};
use crate::alkanes_pb;
use crate::network::NetworkParams;

#[cfg(not(target_arch = "wasm32"))]
use std::{vec::Vec, boxed::Box, string::String, pin::Pin, future::Future};
#[cfg(target_arch = "wasm32")]
use alloc::{vec::Vec, boxed::Box, string::String, pin::Pin, future::Future};

/// Trait for making JSON-RPC calls
pub trait JsonRpcProvider {
    /// Make a JSON-RPC call to the specified URL
    fn call<'a>(
        &'a self,
        url: &'a str,
        method: &'a str,
        params: JsonValue,
        id: u64,
    ) -> Pin<Box<dyn Future<Output = Result<JsonValue>> + Send + 'a>>;
    
    /// Get the timeout for requests (in seconds)
    fn timeout_seconds(&self) -> u64 {
        600 // Default 10 minutes
    }
    
    /// Check if the provider supports a specific URL scheme
    fn supports_url(&self, url: &str) -> bool {
        url.starts_with("http://") || url.starts_with("https://")
    }
}

/// Trait for storage operations (reading/writing files, configuration, etc.)
pub trait StorageProvider {
    /// Read data from storage
    fn read<'a>(&'a self, key: &'a str) -> Pin<Box<dyn Future<Output = Result<Vec<u8>>> + Send + 'a>>;
    
    /// Write data to storage
    fn write<'a>(&'a self, key: &'a str, data: &'a [u8]) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>>;
    
    /// Check if a key exists in storage
    fn exists<'a>(&'a self, key: &'a str) -> Pin<Box<dyn Future<Output = Result<bool>> + Send + 'a>>;
    
    /// Delete data from storage
    fn delete<'a>(&'a self, key: &'a str) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>>;
    
    /// List all keys with a given prefix
    fn list_keys<'a>(&'a self, prefix: &'a str) -> Pin<Box<dyn Future<Output = Result<Vec<String>>> + Send + 'a>>;
    
    /// Get the storage type identifier
    fn storage_type(&self) -> &'static str;
}

/// Trait for network operations beyond JSON-RPC
pub trait NetworkProvider {
    /// Make an HTTP GET request
    fn get<'a>(&'a self, url: &'a str) -> Pin<Box<dyn Future<Output = Result<Vec<u8>>> + Send + 'a>>;
    
    /// Make an HTTP POST request
    fn post<'a>(&'a self, url: &'a str, body: &'a [u8], content_type: &'a str) -> Pin<Box<dyn Future<Output = Result<Vec<u8>>> + Send + 'a>>;
    
    /// Download a file from a URL
    fn download<'a>(&'a self, url: &'a str) -> Pin<Box<dyn Future<Output = Result<Vec<u8>>> + Send + 'a>> {
        self.get(url)
    }
    
    /// Check if a URL is reachable
    fn is_reachable<'a>(&'a self, url: &'a str) -> Pin<Box<dyn Future<Output = bool> + Send + 'a>>;
    
    /// Get the user agent string
    fn user_agent(&self) -> &str {
        "deezel-common/0.1.0"
    }
}

/// Trait for cryptographic operations
pub trait CryptoProvider {
    /// Generate random bytes
    fn random_bytes(&self, len: usize) -> Result<Vec<u8>>;
    
    /// Hash data with SHA256
    fn sha256(&self, data: &[u8]) -> Result<[u8; 32]>;
    
    /// Hash data with SHA3-256 (Keccak256)
    fn sha3_256(&self, data: &[u8]) -> Result<[u8; 32]>;
    
    /// Encrypt data with AES-GCM
    fn encrypt_aes_gcm<'a>(&'a self, data: &'a [u8], key: &'a [u8], nonce: &'a [u8]) -> Pin<Box<dyn Future<Output = Result<Vec<u8>>> + Send + 'a>>;
    
    /// Decrypt data with AES-GCM
    fn decrypt_aes_gcm<'a>(&'a self, data: &'a [u8], key: &'a [u8], nonce: &'a [u8]) -> Pin<Box<dyn Future<Output = Result<Vec<u8>>> + Send + 'a>>;
    
    /// Derive key using PBKDF2
    fn pbkdf2_derive<'a>(&'a self, password: &'a [u8], salt: &'a [u8], iterations: u32, key_len: usize) -> Pin<Box<dyn Future<Output = Result<Vec<u8>>> + Send + 'a>>;
}

/// Trait for PGP operations
/// Trait for time operations
pub trait TimeProvider {
    /// Get current Unix timestamp in seconds
    fn now_secs(&self) -> u64;
    
    /// Get current Unix timestamp in milliseconds
    fn now_millis(&self) -> u64;

    /// Sleep for a specified duration in milliseconds
    fn sleep_ms<'a>(&'a self, ms: u64) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>>;
}

/// Trait for logging operations
pub trait LogProvider {
    /// Log a debug message
    fn debug(&self, message: &str);
    
    /// Log an info message
    fn info(&self, message: &str);
    
    /// Log a warning message
    fn warn(&self, message: &str);
    
    /// Log an error message
    fn error(&self, message: &str);
    
    /// Check if debug logging is enabled
    fn is_debug_enabled(&self) -> bool {
        true
    }
}

/// Trait for wallet operations
pub trait WalletProvider {
    /// Create a new wallet
    fn create_wallet<'a>(&'a mut self, config: WalletConfig, mnemonic: Option<String>, passphrase: Option<String>) -> Pin<Box<dyn Future<Output = Result<WalletInfo>> + Send + 'a>>;
    
    /// Load an existing wallet
    fn load_wallet<'a>(&'a mut self, config: WalletConfig, passphrase: Option<String>) -> Pin<Box<dyn Future<Output = Result<WalletInfo>> + Send + 'a>>;
    
    /// Get wallet balance
    fn get_balance<'a>(&'a self, addresses: Option<Vec<String>>) -> Pin<Box<dyn Future<Output = Result<WalletBalance>> + Send + 'a>>;
    
    /// Get wallet address
    fn get_address<'a>(&'a self) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>>;
    
    /// Get multiple addresses
    fn get_addresses<'a>(&'a self, count: u32) -> Pin<Box<dyn Future<Output = Result<Vec<AddressInfo>>> + Send + 'a>>;
    
    /// Send Bitcoin transaction
    fn send<'a>(&'a mut self, params: SendParams) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>>;
    
    /// Get UTXOs
    fn get_utxos<'a>(&'a self, include_frozen: bool, addresses: Option<Vec<String>>) -> Pin<Box<dyn Future<Output = Result<Vec<(bitcoin::OutPoint, UtxoInfo)>>> + Send + 'a>>;
    
    /// Get transaction history
    fn get_history<'a>(&'a self, count: u32, address: Option<String>) -> Pin<Box<dyn Future<Output = Result<Vec<TransactionInfo>>> + Send + 'a>>;
    
    /// Freeze a UTXO
    fn freeze_utxo<'a>(&'a self, utxo: String, reason: Option<String>) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>>;
    
    /// Unfreeze a UTXO
    fn unfreeze_utxo<'a>(&'a self, utxo: String) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>>;
    
    /// Create transaction without broadcasting
    fn create_transaction<'a>(&'a self, params: SendParams) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>>;
    
    /// Sign transaction
    fn sign_transaction<'a>(&'a mut self, tx_hex: String) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>>;
    
    /// Broadcast transaction
    fn broadcast_transaction<'a>(&'a self, tx_hex: String) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>>;
    
    /// Estimate fee
    fn estimate_fee<'a>(&'a self, target: u32) -> Pin<Box<dyn Future<Output = Result<FeeEstimate>> + Send + 'a>>;
    
    /// Get current fee rates
    fn get_fee_rates<'a>(&'a self) -> Pin<Box<dyn Future<Output = Result<FeeRates>> + Send + 'a>>;
    
    /// Synchronize wallet
    fn sync<'a>(&'a self) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>>;
    
    /// Backup wallet
    fn backup<'a>(&'a self) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>>;
    
    /// Get mnemonic
    fn get_mnemonic<'a>(&'a self) -> Pin<Box<dyn Future<Output = Result<Option<String>>> + Send + 'a>>;
    
    /// Get network
    fn get_network(&self) -> Network;

    /// Get master public key (xpub) if available
    fn get_master_public_key<'a>(&'a self) -> Pin<Box<dyn Future<Output = Result<Option<String>>> + Send + 'a>>;
    
    /// Get internal key for wallet
    fn get_internal_key<'a>(&'a self) -> Pin<Box<dyn Future<Output = Result<(bitcoin::XOnlyPublicKey, (Fingerprint, DerivationPath))>> + Send + 'a>>;
    
    /// Sign PSBT
    fn sign_psbt<'a>(&'a mut self, psbt: &bitcoin::psbt::Psbt) -> Pin<Box<dyn Future<Output = Result<bitcoin::psbt::Psbt>> + Send + 'a>>;
    
    /// Get keypair for wallet
    fn get_keypair<'a>(&'a self) -> Pin<Box<dyn Future<Output = Result<bitcoin::secp256k1::Keypair>> + Send + 'a>>;

    /// Set the passphrase for the wallet
    fn set_passphrase(&mut self, passphrase: Option<String>);

    /// Get the index of the last used address.
    fn get_last_used_address_index<'a>(&'a self) -> Pin<Box<dyn Future<Output = Result<u32>> + Send + 'a>>;

    fn get_enriched_utxos<'a>(&'a self, addresses: Option<Vec<String>>) -> Pin<Box<dyn Future<Output = Result<Vec<crate::provider::EnrichedUtxo>>> + Send + 'a>>;

    fn get_all_balances<'a>(&'a self, addresses: Option<Vec<String>>) -> Pin<Box<dyn Future<Output = Result<crate::provider::AllBalances>> + Send + 'a>>;

}

/// Wallet configuration
#[derive(Debug, Clone)]
pub struct WalletConfig {
    pub wallet_path: String,
    pub network: Network,
    pub bitcoin_rpc_url: String,
    pub metashrew_rpc_url: String,
    pub network_params: Option<NetworkParams>,
}

impl Default for WalletConfig {
    fn default() -> Self {
        Self {
            wallet_path: "default_wallet".to_string(),
            network: Network::Regtest,
            bitcoin_rpc_url: "http://localhost:18443".to_string(),
            metashrew_rpc_url: "http://localhost:18888".to_string(),
            network_params: None,
        }
    }
}

/// Wallet information
#[derive(Debug, Clone)]
pub struct WalletInfo {
    pub address: String,
    pub network: Network,
    pub mnemonic: Option<String>,
}

/// Wallet balance information
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WalletBalance {
    pub confirmed: u64,
    pub pending: i64,
}

/// Address information
#[derive(Debug, Clone)]
pub struct AddressInfo {
    pub address: String,
    pub script_type: String,
    pub derivation_path: String,
    pub index: u32,
    pub used: bool,
}

/// Send transaction parameters
#[derive(Debug, Clone)]
pub struct SendParams {
    pub address: String,
    pub amount: u64,
    pub fee_rate: Option<f32>,
    pub send_all: bool,
    pub from: Option<Vec<String>>,
    pub change_address: Option<String>,
    pub auto_confirm: bool,
}

/// UTXO information
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UtxoInfo {
    pub txid: String,
    pub vout: u32,
    pub amount: u64,
    pub address: String,
    pub script_pubkey: Option<ScriptBuf>,
    pub confirmations: u32,
    pub frozen: bool,
    pub freeze_reason: Option<String>,
    pub block_height: Option<u64>,
    pub has_inscriptions: bool,
    pub has_runes: bool,
    pub has_alkanes: bool,
    pub is_coinbase: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Utxo {
    pub txid: String,
    pub vout: u32,
    pub amount: u64,
    pub address: String,
}

/// Trait for providing UTXOs
pub trait UtxoProvider {
    fn get_utxos_by_spec<'a>(&'a self, spec: &'a [String]) -> Pin<Box<dyn Future<Output = Result<Vec<Utxo>>> + Send + 'a>>;
}


/// Transaction information
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TransactionInfo {
    pub txid: String,
    pub block_height: Option<u64>,
    pub block_time: Option<u64>,
    pub confirmed: bool,
    pub fee: Option<u64>,
    pub weight: Option<u64>,
    pub inputs: Vec<TransactionInput>,
    pub outputs: Vec<TransactionOutput>,
    pub is_op_return: bool,
    pub has_protostones: bool,
    pub is_rbf: bool,
}

/// Transaction input
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TransactionInput {
    pub txid: String,
    pub vout: u32,
    pub address: Option<String>,
    pub amount: Option<u64>,
}

/// Transaction output
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TransactionOutput {
    pub address: Option<String>,
    pub amount: u64,
    pub script: ScriptBuf,
}

/// Fee estimate
#[derive(Debug, Clone)]
pub struct FeeEstimate {
    pub fee_rate: f32,
    pub target_blocks: u32,
}

/// Fee rates
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FeeRates {
    pub fast: f32,
    pub medium: f32,
    pub slow: f32,
}




/// Trait for address resolution
pub trait AddressResolver {
    /// Resolve address identifiers in a string
    fn resolve_all_identifiers<'a>(&'a self, input: &'a str) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>>;
    
    /// Check if string contains identifiers
    fn contains_identifiers(&self, input: &str) -> bool;
    
    /// Get address for specific type and index
    fn get_address<'a>(&'a self, address_type: &'a str, index: u32) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>>;
    
    /// List available address identifiers
    fn list_identifiers<'a>(&'a self) -> Pin<Box<dyn Future<Output = Result<Vec<String>>> + Send + 'a>>;
}

/// Trait for dynamic address derivation from master public keys
pub trait KeystoreProvider {
    /// Derive addresses dynamically from master public key
    fn derive_addresses<'a>(&'a self, master_public_key: &'a str, network_params: &'a NetworkParams, script_types: &'a [&'a str], start_index: u32, count: u32) -> Pin<Box<dyn Future<Output = Result<Vec<KeystoreAddress>>> + Send + 'a>>;
    
    /// Get default addresses for display (first 5 of each type for given network)
    fn get_default_addresses<'a>(&'a self, master_public_key: &'a str, network_params: &'a NetworkParams) -> Pin<Box<dyn Future<Output = Result<Vec<KeystoreAddress>>> + Send + 'a>>;

    /// Get address for specific type and index
    fn get_address<'a>(&'a self, address_type: &'a str, index: u32) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>>;
    
    /// Parse address range specification (e.g., "p2tr:0-1000", "p2sh:0-500")
    fn parse_address_range(&self, range_spec: &str) -> Result<(String, u32, u32)>;
    
    /// Get keystore info from master public key
    fn get_keystore_info<'a>(&'a self, master_fingerprint: &'a str, created_at: u64, version: &'a str) -> Pin<Box<dyn Future<Output = Result<KeystoreInfo>> + Send + 'a>>;

    /// Derive a single address from a full derivation path
    fn derive_address_from_path<'a>(&'a self, master_public_key: &'a str, path: &'a DerivationPath, script_type: &'a str, network_params: &'a NetworkParams) -> Pin<Box<dyn Future<Output = Result<KeystoreAddress>> + Send + 'a>>;
}

/// Address information for keystore operations
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct KeystoreAddress {
    /// The Bitcoin address
    pub address: String,
    /// Derivation path used to generate this address
    pub derivation_path: String,
    /// Index in the derivation sequence
    pub index: u32,
    /// Script type (P2WPKH, P2TR, etc.)
    pub script_type: String,
    /// Network name (optional, for display purposes)
    pub network: Option<String>,
}

/// Summary information about a keystore
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct KeystoreInfo {
    pub master_fingerprint: String,
    pub created_at: u64,
    pub version: String,
}

/// Trait for Bitcoin Core RPC operations
pub trait BitcoinRpcProvider {
    /// Get current block count
    fn get_block_count<'a>(&'a self) -> Pin<Box<dyn Future<Output = Result<u64>> + Send + 'a>>;
    
    /// Generate blocks to address (regtest only)
    fn generate_to_address<'a>(&'a self, nblocks: u32, address: &'a str) -> Pin<Box<dyn Future<Output = Result<JsonValue>> + Send + 'a>>;

    // Get the state info
    fn get_blockchain_info<'a>(&'a self) -> Pin<Box<dyn Future<Output = Result<JsonValue>> + Send + 'a>>;

    /// Get a new address from the node's wallet
    fn get_new_address<'a>(&'a self) -> Pin<Box<dyn Future<Output = Result<JsonValue>> + Send + 'a>>;
    
    /// Get transaction hex
    fn get_transaction_hex<'a>(&'a self, txid: &'a str) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>>;
    
    /// Get block by hash
    fn get_block<'a>(&'a self, hash: &'a str, raw: bool) -> Pin<Box<dyn Future<Output = Result<JsonValue>> + Send + 'a>>;
    
    /// Get block hash by height
    fn get_block_hash<'a>(&'a self, height: u64) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>>;
    
    /// Send raw transaction
    fn send_raw_transaction<'a>(&'a self, tx_hex: &'a str) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>>;
    
    /// Get mempool info
    fn get_mempool_info<'a>(&'a self) -> Pin<Box<dyn Future<Output = Result<JsonValue>> + Send + 'a>>;
    
    /// Estimate smart fee
    fn estimate_smart_fee<'a>(&'a self, target: u32) -> Pin<Box<dyn Future<Output = Result<JsonValue>> + Send + 'a>>;
    
    /// Get Esplora blocks tip height
    fn get_esplora_blocks_tip_height<'a>(&'a self) -> Pin<Box<dyn Future<Output = Result<u64>> + Send + 'a>>;
    
    /// Trace transaction
    fn trace_transaction<'a>(&'a self, txid: &'a str, vout: u32, block: Option<&'a str>, tx: Option<&'a str>) -> Pin<Box<dyn Future<Output = Result<serde_json::Value>> + Send + 'a>>;

    /// Get network info
    fn get_network_info<'a>(&'a self) -> Pin<Box<dyn Future<Output = Result<JsonValue>> + Send + 'a>>;

    /// Get raw transaction
    fn get_raw_transaction<'a>(&'a self, txid: &'a str, block_hash: Option<&'a str>) -> Pin<Box<dyn Future<Output = Result<JsonValue>> + Send + 'a>>;

    /// Get block header
    fn get_block_header<'a>(&'a self, hash: &'a str) -> Pin<Box<dyn Future<Output = Result<JsonValue>> + Send + 'a>>;

    /// Get block stats
    fn get_block_stats<'a>(&'a self, hash: &'a str) -> Pin<Box<dyn Future<Output = Result<JsonValue>> + Send + 'a>>;

    /// Get chain tips
    fn get_chain_tips<'a>(&'a self) -> Pin<Box<dyn Future<Output = Result<JsonValue>> + Send + 'a>>;

    /// Get raw mempool
    fn get_raw_mempool<'a>(&'a self) -> Pin<Box<dyn Future<Output = Result<JsonValue>> + Send + 'a>>;

    /// Get tx out
    fn get_tx_out<'a>(&'a self, txid: &'a str, vout: u32, include_mempool: bool) -> Pin<Box<dyn Future<Output = Result<JsonValue>> + Send + 'a>>;
}

/// Trait for bitcoind RPC operations using bitcoincore_rpc_json types

/// Trait for Metashrew/Sandshrew RPC operations
pub trait MetashrewRpcProvider {
    /// Get Metashrew height
    fn get_metashrew_height<'a>(&'a self) -> Pin<Box<dyn Future<Output = Result<u64>> + Send + 'a>>;

    /// Get the state root for a given height.
    fn get_state_root<'a>(&'a self, height: JsonValue) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>>;
    
    /// Get contract metadata
    fn get_contract_meta<'a>(&'a self, block: &'a str, tx: &'a str) -> Pin<Box<dyn Future<Output = Result<JsonValue>> + Send + 'a>>;
    
    /// Trace transaction outpoint
    fn trace_outpoint<'a>(&'a self, txid: &'a str, vout: u32) -> Pin<Box<dyn Future<Output = Result<JsonValue>> + Send + 'a>>;
    
    /// Get spendables by address
    fn get_spendables_by_address<'a>(&'a self, address: &'a str) -> Pin<Box<dyn Future<Output = Result<JsonValue>> + Send + 'a>>;
    
    /// Get protorunes by address
    fn get_protorunes_by_address<'a>(
        &'a self,
        address: &'a str,
        block_tag: Option<String>,
        protocol_tag: u128,
    ) -> Pin<Box<dyn Future<Output = Result<ProtoruneWalletResponse>> + Send + 'a>>;
    
    /// Get protorunes by outpoint
    fn get_protorunes_by_outpoint<'a>(
        &'a self,
        txid: &'a str,
        vout: u32,
        block_tag: Option<String>,
        protocol_tag: u128,
    ) -> Pin<Box<dyn Future<Output = Result<ProtoruneOutpointResponse>> + Send + 'a>>;
}

/// Trait for Metashrew provider operations
pub trait MetashrewProvider {
    /// Get the current block height.
    fn get_height<'a>(&'a self) -> Pin<Box<dyn Future<Output = Result<u64>> + Send + 'a>>;
    /// Get the block hash for a given height.
    fn get_block_hash<'a>(&'a self, height: u64) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>>;
    /// Get the state root for a given height.
    fn get_state_root<'a>(&'a self, height: JsonValue) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>>;
}

/// Trait for Esplora API operations
pub trait EsploraProvider {
    /// Get blocks tip hash
    fn get_blocks_tip_hash<'a>(&'a self) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>>;
    
    /// Get blocks tip height
    fn get_blocks_tip_height<'a>(&'a self) -> Pin<Box<dyn Future<Output = Result<u64>> + Send + 'a>>;
    
    /// Get blocks starting from height
    fn get_blocks<'a>(&'a self, start_height: Option<u64>) -> Pin<Box<dyn Future<Output = Result<JsonValue>> + Send + 'a>>;
    
    /// Get block by height
    fn get_block_by_height<'a>(&'a self, height: u64) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>>;
    
    /// Get block information
    fn get_block<'a>(&'a self, hash: &'a str) -> Pin<Box<dyn Future<Output = Result<JsonValue>> + Send + 'a>>;
    
    /// Get block status
    fn get_block_status<'a>(&'a self, hash: &'a str) -> Pin<Box<dyn Future<Output = Result<JsonValue>> + Send + 'a>>;
    
    /// Get block transaction IDs
    fn get_block_txids<'a>(&'a self, hash: &'a str) -> Pin<Box<dyn Future<Output = Result<JsonValue>> + Send + 'a>>;
    
    /// Get block header
    fn get_block_header<'a>(&'a self, hash: &'a str) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>>;
    
    /// Get raw block data
    fn get_block_raw<'a>(&'a self, hash: &'a str) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>>;
    
    /// Get transaction ID by block hash and index
    fn get_block_txid<'a>(&'a self, hash: &'a str, index: u32) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>>;
    
    /// Get block transactions
    fn get_block_txs<'a>(&'a self, hash: &'a str, start_index: Option<u32>) -> Pin<Box<dyn Future<Output = Result<JsonValue>> + Send + 'a>>;
    
    /// Get address information
    fn get_address_info<'a>(&'a self, address: &'a str) -> Pin<Box<dyn Future<Output = Result<JsonValue>> + Send + 'a>>;

    /// Get address UTXOs
    fn get_address_utxo<'a>(&'a self, address: &'a str) -> Pin<Box<dyn Future<Output = Result<JsonValue>> + Send + 'a>>;

    /// Get address transactions
    fn get_address_txs<'a>(&'a self, address: &'a str) -> Pin<Box<dyn Future<Output = Result<JsonValue>> + Send + 'a>>;
    
    /// Get address chain transactions
    fn get_address_txs_chain<'a>(&'a self, address: &'a str, last_seen_txid: Option<&'a str>) -> Pin<Box<dyn Future<Output = Result<JsonValue>> + Send + 'a>>;
    
    /// Get address mempool transactions
    fn get_address_txs_mempool<'a>(&'a self, address: &'a str) -> Pin<Box<dyn Future<Output = Result<JsonValue>> + Send + 'a>>;
    
    /// Search addresses by prefix
    fn get_address_prefix<'a>(&'a self, prefix: &'a str) -> Pin<Box<dyn Future<Output = Result<JsonValue>> + Send + 'a>>;
    
    /// Get transaction information
    fn get_tx<'a>(&'a self, txid: &'a str) -> Pin<Box<dyn Future<Output = Result<JsonValue>> + Send + 'a>>;
    
    /// Get transaction hex
    fn get_tx_hex<'a>(&'a self, txid: &'a str) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>>;
    
    /// Get raw transaction
    fn get_tx_raw<'a>(&'a self, txid: &'a str) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>>;
    
    /// Get transaction status
    fn get_tx_status<'a>(&'a self, txid: &'a str) -> Pin<Box<dyn Future<Output = Result<JsonValue>> + Send + 'a>>;
    
    /// Get transaction merkle proof
    fn get_tx_merkle_proof<'a>(&'a self, txid: &'a str) -> Pin<Box<dyn Future<Output = Result<JsonValue>> + Send + 'a>>;
    
    /// Get transaction merkle block proof
    fn get_tx_merkleblock_proof<'a>(&'a self, txid: &'a str) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>>;
    
    /// Get transaction output spend status
    fn get_tx_outspend<'a>(&'a self, txid: &'a str, index: u32) -> Pin<Box<dyn Future<Output = Result<JsonValue>> + Send + 'a>>;
    
    /// Get transaction output spends
    fn get_tx_outspends<'a>(&'a self, txid: &'a str) -> Pin<Box<dyn Future<Output = Result<JsonValue>> + Send + 'a>>;
    
    /// Broadcast transaction
    fn broadcast<'a>(&'a self, tx_hex: &'a str) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>>;
    
    /// Get mempool information
    fn get_mempool<'a>(&'a self) -> Pin<Box<dyn Future<Output = Result<JsonValue>> + Send + 'a>>;
    
    /// Get mempool transaction IDs
    fn get_mempool_txids<'a>(&'a self) -> Pin<Box<dyn Future<Output = Result<JsonValue>> + Send + 'a>>;
    
    /// Get recent mempool transactions
    fn get_mempool_recent<'a>(&'a self) -> Pin<Box<dyn Future<Output = Result<JsonValue>> + Send + 'a>>;
    
    /// Get fee estimates
    fn get_fee_estimates<'a>(&'a self) -> Pin<Box<dyn Future<Output = Result<JsonValue>> + Send + 'a>>;
}

/// Trait for runestone operations
pub trait RunestoneProvider {
    /// Decode runestone from transaction
    fn decode_runestone<'a>(&'a self, tx: &'a Transaction) -> Pin<Box<dyn Future<Output = Result<JsonValue>> + Send + 'a>>;
    
    /// Format runestone with decoded messages
    fn format_runestone_with_decoded_messages<'a>(&'a self, tx: &'a Transaction) -> Pin<Box<dyn Future<Output = Result<JsonValue>> + Send + 'a>>;
    
    /// Analyze runestone from transaction ID
    fn analyze_runestone<'a>(&'a self, txid: &'a str) -> Pin<Box<dyn Future<Output = Result<JsonValue>> + Send + 'a>>;
}

/// Trait for ord operations
pub trait OrdProvider {
    /// Get inscription by ID
    fn get_inscription<'a>(&'a self, inscription_id: &'a str) -> Pin<Box<dyn Future<Output = Result<OrdInscription>> + Send + 'a>>;
    
    /// Get inscriptions for a block
    fn get_inscriptions_in_block<'a>(&'a self, block_hash: &'a str) -> Pin<Box<dyn Future<Output = Result<OrdInscriptions>> + Send + 'a>>;
    /// Get address information
    fn get_ord_address_info<'a>(&'a self, address: &'a str) -> Pin<Box<dyn Future<Output = Result<OrdAddressInfo>> + Send + 'a>>;
    /// Get block information
    fn get_block_info<'a>(&'a self, query: &'a str) -> Pin<Box<dyn Future<Output = Result<OrdBlock>> + Send + 'a>>;
    /// Get latest block count
    fn get_ord_block_count<'a>(&'a self) -> Pin<Box<dyn Future<Output = Result<u64>> + Send + 'a>>;
    /// Get latest blocks
    fn get_ord_blocks<'a>(&'a self) -> Pin<Box<dyn Future<Output = Result<OrdBlocks>> + Send + 'a>>;
    /// Get children of an inscription
    fn get_children<'a>(&'a self, inscription_id: &'a str, page: Option<u32>) -> Pin<Box<dyn Future<Output = Result<OrdChildren>> + Send + 'a>>;
    /// Get inscription content
    fn get_content<'a>(&'a self, inscription_id: &'a str) -> Pin<Box<dyn Future<Output = Result<Vec<u8>>> + Send + 'a>>;
    /// Get all inscriptions
    fn get_inscriptions<'a>(&'a self, page: Option<u32>) -> Pin<Box<dyn Future<Output = Result<OrdInscriptions>> + Send + 'a>>;
    /// Get output information
    fn get_output<'a>(&'a self, output: &'a str) -> Pin<Box<dyn Future<Output = Result<OrdOutput>> + Send + 'a>>;
    /// Get parents of an inscription
    fn get_parents<'a>(&'a self, inscription_id: &'a str, page: Option<u32>) -> Pin<Box<dyn Future<Output = Result<OrdParents>> + Send + 'a>>;
    /// Get rune information
    fn get_rune<'a>(&'a self, rune: &'a str) -> Pin<Box<dyn Future<Output = Result<OrdRuneInfo>> + Send + 'a>>;
    /// Get all runes
    fn get_runes<'a>(&'a self, page: Option<u32>) -> Pin<Box<dyn Future<Output = Result<OrdRunes>> + Send + 'a>>;
    /// Get sat information
    fn get_sat<'a>(&'a self, sat: u64) -> Pin<Box<dyn Future<Output = Result<OrdSat>> + Send + 'a>>;
    /// Get transaction information
    fn get_tx_info<'a>(&'a self, txid: &'a str) -> Pin<Box<dyn Future<Output = Result<OrdTxInfo>> + Send + 'a>>;
}

/// Trait for monitoring operations
pub trait MonitorProvider {
    /// Monitor blocks for events
    fn monitor_blocks<'a>(&'a self, start: Option<u64>) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>>;
    
    /// Get block events
    fn get_block_events<'a>(&'a self, height: u64) -> Pin<Box<dyn Future<Output = Result<Vec<BlockEvent>>> + Send + 'a>>;
}

/// Block event
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BlockEvent {
    pub event_type: String,
    pub block_height: u64,
    pub txid: String,
    pub data: JsonValue,
}

/// Combined provider trait that includes all functionality
pub trait AlkanesProvider:
    JsonRpcProvider +
    StorageProvider +
    NetworkProvider +
    CryptoProvider +
    TimeProvider +
    LogProvider +
    WalletProvider +
    AddressResolver +
    BitcoinRpcProvider +
    MetashrewRpcProvider +
    MetashrewProvider +
    EsploraProvider +
    RunestoneProvider +
    MonitorProvider +
    KeystoreProvider +
    OrdProvider +
    Send + Sync
{
    /// Get provider name/type
    fn provider_name(&self) -> &str;

    /// Get the Bitcoin RPC URL
    fn get_bitcoin_rpc_url(&self) -> Option<String>;

    /// Get the Esplora API URL
    fn get_esplora_api_url(&self) -> Option<String>;

    /// Get the Ord server URL
    fn get_ord_server_url(&self) -> Option<String>;

    /// Get the Metashrew RPC URL
    fn get_metashrew_rpc_url(&self) -> Option<String>;

    /// Create a boxed, clonable version of the provider
    fn clone_box(&self) -> Box<dyn AlkanesProvider>;
    
    /// Initialize the provider
    fn initialize<'a>(&'a self) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>>;
    
    /// Shutdown the provider
    fn shutdown<'a>(&'a self) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>>;

    /// Get a reference to the secp256k1 context
    fn secp(&self) -> &bitcoin::secp256k1::Secp256k1<bitcoin::secp256k1::All>;

    /// Get a single UTXO by its outpoint
    fn get_utxo<'a>(&'a self, outpoint: &'a bitcoin::OutPoint) -> Pin<Box<dyn Future<Output = Result<Option<bitcoin::TxOut>>> + Send + 'a>>;

    /// Sign a taproot script spend sighash
    fn sign_taproot_script_spend<'a>(&'a self, sighash: bitcoin::secp256k1::Message) -> Pin<Box<dyn Future<Output = Result<bitcoin::secp256k1::schnorr::Signature>> + Send + 'a>>;

    fn wrap<'a>(&'a mut self, amount: u64, address: Option<String>, fee_rate: Option<f32>) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>>;

    fn unwrap<'a>(&'a mut self, amount: u64, address: Option<String>) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>>;

    /// Execute alkanes smart contract
    fn execute<'a>(&'a mut self, params: EnhancedExecuteParams) -> Pin<Box<dyn Future<Output = Result<ExecutionState>> + Send + 'a>>;

    /// Resume execution after user confirmation (for simple transactions)
    fn resume_execution<'a>(
        &'a mut self,
        state: ReadyToSignTx,
        params: &'a EnhancedExecuteParams,
    ) -> Pin<Box<dyn Future<Output = Result<EnhancedExecuteResult>> + Send + 'a>>;

    /// Resume execution after commit transaction confirmation
    fn resume_commit_execution<'a>(
        &'a mut self,
        state: ReadyToSignCommitTx,
    ) -> Pin<Box<dyn Future<Output = Result<ExecutionState>> + Send + 'a>>;

    /// Resume execution after reveal transaction confirmation
    fn resume_reveal_execution<'a>(
        &'a mut self,
        state: ReadyToSignRevealTx,
    ) -> Pin<Box<dyn Future<Output = Result<EnhancedExecuteResult>> + Send + 'a>>;

    fn view<'a>(&'a self, contract_id: &'a str, view_fn: &'a str, params: Option<&'a [u8]>) -> Pin<Box<dyn Future<Output = Result<JsonValue>> + Send + 'a>>;
    
    fn protorunes_by_address<'a>(
        &'a self,
        address: &'a str,
        block_tag: Option<String>,
        protocol_tag: u128,
    ) -> Pin<Box<dyn Future<Output = Result<ProtoruneWalletResponse>> + Send + 'a>>;
    fn protorunes_by_outpoint<'a>(
        &'a self,
        txid: &'a str,
        vout: u32,
        block_tag: Option<String>,
        protocol_tag: u128,
    ) -> Pin<Box<dyn Future<Output = Result<ProtoruneOutpointResponse>> + Send + 'a>>;
    fn simulate<'a>(&'a self, contract_id: &'a str, context: &'a crate::alkanes_pb::MessageContextParcel) -> Pin<Box<dyn Future<Output = Result<JsonValue>> + Send + 'a>>;
    fn trace<'a>(&'a self, outpoint: &'a str) -> Pin<Box<dyn Future<Output = Result<alkanes_pb::Trace>> + Send + 'a>>;
    fn get_block<'a>(&'a self, height: u64) -> Pin<Box<dyn Future<Output = Result<alkanes_pb::BlockResponse>> + Send + 'a>>;
    fn sequence<'a>(&'a self) -> Pin<Box<dyn Future<Output = Result<JsonValue>> + Send + 'a>>;
    fn spendables_by_address<'a>(&'a self, address: &'a str) -> Pin<Box<dyn Future<Output = Result<JsonValue>> + Send + 'a>>;
    fn trace_block<'a>(&'a self, height: u64) -> Pin<Box<dyn Future<Output = Result<alkanes_pb::Trace>> + Send + 'a>>;
    fn get_bytecode<'a>(&'a self, alkane_id: &'a str, block_tag: Option<String>) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>>;
    fn inspect<'a>(&'a self, target: &'a str, config: crate::alkanes::AlkanesInspectConfig) -> Pin<Box<dyn Future<Output = Result<crate::alkanes::AlkanesInspectResult>> + Send + 'a>>;
    fn get_balance<'a>(&'a self, address: Option<&'a str>) -> Pin<Box<dyn Future<Output = Result<Vec<crate::alkanes::AlkaneBalance>>> + Send + 'a>>;
}

#[cfg(target_arch = "wasm32")]
#[async_trait(?Send)]
pub trait AlkanesProvider:
    JsonRpcProvider +
    StorageProvider +
    NetworkProvider +
    CryptoProvider +
    TimeProvider +
    LogProvider +
    WalletProvider +
    AddressResolver +
    BitcoinRpcProvider +
    MetashrewRpcProvider +
    MetashrewProvider +
    EsploraProvider +
    RunestoneProvider +
    MonitorProvider +
    KeystoreProvider +
    OrdProvider
{
    /// Get provider name/type
    fn provider_name(&self) -> &str;

    /// Get the Bitcoin RPC URL
    fn get_bitcoin_rpc_url(&self) -> Option<String>;

    /// Get the Esplora API URL
    fn get_esplora_api_url(&self) -> Option<String>;

    /// Get the Ord server URL
    fn get_ord_server_url(&self) -> Option<String>;

    /// Get the Metashrew RPC URL
    fn get_metashrew_rpc_url(&self) -> Option<String>;

    /// Create a boxed, clonable version of the provider
    fn clone_box(&self) -> Box<dyn AlkanesProvider>;
    
    /// Initialize the provider
    async fn initialize(&self) -> Result<()>;
    
    /// Shutdown the provider
    async fn shutdown(&self) -> Result<()>;

    /// Get a reference to the secp256k1 context
    fn secp(&self) -> &bitcoin::secp256k1::Secp256k1<bitcoin::secp256k1::All>;

    /// Get a single UTXO by its outpoint
    async fn get_utxo(&self, outpoint: &bitcoin::OutPoint) -> Result<Option<bitcoin::TxOut>>;

    /// Sign a taproot script spend sighash
    async fn sign_taproot_script_spend(&self, sighash: bitcoin::secp256k1::Message) -> Result<bitcoin::secp256k1::schnorr::Signature>;

    async fn wrap(&mut self, amount: u64, address: Option<String>, fee_rate: Option<f32>) -> Result<String>;

    async fn unwrap(&mut self, amount: u64, address: Option<String>) -> Result<String>;

    /// Execute alkanes smart contract
    async fn execute(&mut self, params: EnhancedExecuteParams) -> Result<ExecutionState>;

    /// Resume execution after user confirmation (for simple transactions)
    async fn resume_execution(
        &mut self,
        state: ReadyToSignTx,
        params: &EnhancedExecuteParams,
    ) -> Result<EnhancedExecuteResult>;

    /// Resume execution after commit transaction confirmation
    async fn resume_commit_execution(
        &mut self,
        state: ReadyToSignCommitTx,
    ) -> Result<ExecutionState>;

    /// Resume execution after reveal transaction confirmation
    async fn resume_reveal_execution(
        &mut self,
        state: ReadyToSignRevealTx,
    ) -> Result<EnhancedExecuteResult>;

    async fn view(&self, contract_id: &str, view_fn: &str, params: Option<&[u8]>) -> Result<JsonValue>;
    
    async fn protorunes_by_address(
        &self,
        address: &str,
        block_tag: Option<String>,
        protocol_tag: u128,
    ) -> Result<ProtoruneWalletResponse>;
    async fn protorunes_by_outpoint(
        &self,
        txid: &str,
        vout: u32,
        block_tag: Option<String>,
        protocol_tag: u128,
    ) -> Result<ProtoruneOutpointResponse>;
    async fn simulate(&self, contract_id: &str, context: &crate::alkanes_pb::MessageContextParcel) -> Result<JsonValue>;
    async fn trace(&self, outpoint: &str) -> Result<alkanes_pb::Trace>;
    async fn get_block(&self, height: u64) -> Result<alkanes_pb::BlockResponse>;
    async fn sequence(&self) -> Result<JsonValue>;
    async fn spendables_by_address(&self, address: &str) -> Result<JsonValue>;
    async fn trace_block(&self, height: u64) -> Result<alkanes_pb::Trace>;
    async fn get_bytecode(&self, alkane_id: &str, block_tag: Option<String>) -> Result<String>;
    async fn inspect(&self, target: &str, config: crate::alkanes::AlkanesInspectConfig) -> Result<crate::alkanes::AlkanesInspectResult>;
    async fn get_balance(&self, address: Option<&str>) -> Result<Vec<crate::alkanes::AlkaneBalance>>;
}

impl Clone for Box<dyn AlkanesProvider> {
   fn clone(&self) -> Self {
       AlkanesProvider::clone_box(self)
   }
}

#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
impl<T: AlkanesProvider + ?Sized + Send + Sync> JsonRpcProvider for Box<T> {
   async fn call(&self, url: &str, method: &str, params: serde_json::Value, id: u64) -> Result<serde_json::Value> {
       (**self).call(url, method, params, id).await
   }
}

#[cfg(target_arch = "wasm32")]
#[async_trait(?Send)]
impl<T: AlkanesProvider + ?Sized> JsonRpcProvider for Box<T> {
    async fn call(&self, url: &str, method: &str, params: serde_json::Value, id: u64) -> Result<serde_json::Value> {
        (**self).call(url, method, params, id).await
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
impl<T: AlkanesProvider + ?Sized + Send + Sync> StorageProvider for Box<T> {
   async fn read(&self, key: &str) -> Result<Vec<u8>> {
       (**self).read(key).await
   }
   async fn write(&self, key: &str, data: &[u8]) -> Result<()> {
       (**self).write(key, data).await
   }
   async fn exists(&self, key: &str) -> Result<bool> {
       (**self).exists(key).await
   }
   async fn delete(&self, key: &str) -> Result<()> {
       (**self).delete(key).await
   }
   async fn list_keys(&self, prefix: &str) -> Result<Vec<String>> {
       (**self).list_keys(prefix).await
   }
   fn storage_type(&self) -> &'static str {
       (**self).storage_type()
   }
}

#[cfg(target_arch = "wasm32")]
#[async_trait(?Send)]
impl<T: AlkanesProvider + ?Sized> StorageProvider for Box<T> {
    async fn read(&self, key: &str) -> Result<Vec<u8>> {
        (**self).read(key).await
    }
    async fn write(&self, key: &str, data: &[u8]) -> Result<()> {
        (**self).write(key, data).await
    }
    async fn exists(&self, key: &str) -> Result<bool> {
        (**self).exists(key).await
    }
    async fn delete(&self, key: &str) -> Result<()> {
        (**self).delete(key).await
    }
    async fn list_keys(&self, prefix: &str) -> Result<Vec<String>> {
        (**self).list_keys(prefix).await
    }
    fn storage_type(&self) -> &'static str {
        (**self).storage_type()
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
impl<T: AlkanesProvider + ?Sized + Send + Sync> NetworkProvider for Box<T> {
   async fn get(&self, url: &str) -> Result<Vec<u8>> {
       (**self).get(url).await
   }
   async fn post(&self, url: &str, body: &[u8], content_type: &str) -> Result<Vec<u8>> {
       (**self).post(url, body, content_type).await
   }
   async fn is_reachable(&self, url: &str) -> bool {
       (**self).is_reachable(url).await
   }
}

#[cfg(target_arch = "wasm32")]
#[async_trait(?Send)]
impl<T: AlkanesProvider + ?Sized> NetworkProvider for Box<T> {
    async fn get(&self, url: &str) -> Result<Vec<u8>> {
        (**self).get(url).await
    }
    async fn post(&self, url: &str, body: &[u8], content_type: &str) -> Result<Vec<u8>> {
        (**self).post(url, body, content_type).await
    }
    async fn is_reachable(&self, url: &str) -> bool {
        (**self).is_reachable(url).await
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
impl<T: AlkanesProvider + ?Sized + Send + Sync> CryptoProvider for Box<T> {
   fn random_bytes(&self, len: usize) -> Result<Vec<u8>> {
       (**self).random_bytes(len)
   }
   fn sha256(&self, data: &[u8]) -> Result<[u8; 32]> {
       (**self).sha256(data)
   }
   fn sha3_256(&self, data: &[u8]) -> Result<[u8; 32]> {
       (**self).sha3_256(data)
   }
   async fn encrypt_aes_gcm(&self, data: &[u8], key: &[u8], nonce: &[u8]) -> Result<Vec<u8>> {
       (**self).encrypt_aes_gcm(data, key, nonce).await
   }
   async fn decrypt_aes_gcm(&self, data: &[u8], key: &[u8], nonce: &[u8]) -> Result<Vec<u8>> {
       (**self).decrypt_aes_gcm(data, key, nonce).await
   }
   async fn pbkdf2_derive(&self, password: &[u8], salt: &[u8], iterations: u32, key_len: usize) -> Result<Vec<u8>> {
       (**self).pbkdf2_derive(password, salt, iterations, key_len).await
   }
}

#[cfg(target_arch = "wasm32")]
#[async_trait(?Send)]
impl<T: AlkanesProvider + ?Sized> CryptoProvider for Box<T> {
    fn random_bytes(&self, len: usize) -> Result<Vec<u8>> {
        (**self).random_bytes(len)
    }
    fn sha256(&self, data: &[u8]) -> Result<[u8; 32]> {
        (**self).sha256(data)
    }
    fn sha3_256(&self, data: &[u8]) -> Result<[u8; 32]> {
        (**self).sha3_256(data)
    }
    async fn encrypt_aes_gcm(&self, data: &[u8], key: &[u8], nonce: &[u8]) -> Result<Vec<u8>> {
        (**self).encrypt_aes_gcm(data, key, nonce).await
    }
    async fn decrypt_aes_gcm(&self, data: &[u8], key: &[u8], nonce: &[u8]) -> Result<Vec<u8>> {
        (**self).decrypt_aes_gcm(data, key, nonce).await
    }
    async fn pbkdf2_derive(&self, password: &[u8], salt: &[u8], iterations: u32, key_len: usize) -> Result<Vec<u8>> {
        (**self).pbkdf2_derive(password, salt, iterations, key_len).await
    }
}


#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
impl<T: AlkanesProvider + ?Sized + Send + Sync> TimeProvider for Box<T> {
   fn now_secs(&self) -> u64 {
       (**self).now_secs()
   }
   fn now_millis(&self) -> u64 {
       (**self).now_millis()
   }
   async fn sleep_ms(&self, ms: u64) {
       (**self).sleep_ms(ms).await
   }
}

#[cfg(target_arch = "wasm32")]
#[async_trait(?Send)]
impl<T: AlkanesProvider + ?Sized> TimeProvider for Box<T> {
    fn now_secs(&self) -> u64 {
        (**self).now_secs()
    }
    fn now_millis(&self) -> u64 {
        (**self).now_millis()
    }
    async fn sleep_ms(&self, ms: u64) {
        (**self).sleep_ms(ms).await
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl<T: AlkanesProvider + ?Sized + Send + Sync> LogProvider for Box<T> {
   fn debug(&self, message: &str) {
       (**self).debug(message)
   }
   fn info(&self, message: &str) {
       (**self).info(message)
   }
   fn warn(&self, message: &str) {
       (**self).warn(message)
   }
   fn error(&self, message: &str) {
       (**self).error(message)
   }
}

#[cfg(target_arch = "wasm32")]
impl<T: AlkanesProvider + ?Sized> LogProvider for Box<T> {
    fn debug(&self, message: &str) {
        (**self).debug(message)
    }
    fn info(&self, message: &str) {
        (**self).info(message)
    }
    fn warn(&self, message: &str) {
        (**self).warn(message)
    }
    fn error(&self, message: &str) {
        (**self).error(message)
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
impl<T: AlkanesProvider + ?Sized + Send + Sync> WalletProvider for Box<T> {
   async fn create_wallet(&mut self, config: WalletConfig, mnemonic: Option<String>, passphrase: Option<String>) -> Result<WalletInfo> {
       (**self).create_wallet(config, mnemonic, passphrase).await
   }
   async fn load_wallet(&mut self, config: WalletConfig, passphrase: Option<String>) -> Result<WalletInfo> {
       (**self).load_wallet(config, passphrase).await
   }
   async fn get_balance(&self, addresses: Option<Vec<String>>) -> Result<WalletBalance> {
       WalletProvider::get_balance(&**self, addresses).await
   }
   async fn get_address(&self) -> Result<String> {
       WalletProvider::get_address(&**self).await
   }
   async fn get_addresses(&self, count: u32) -> Result<Vec<AddressInfo>> {
       (**self).get_addresses(count).await
   }
   async fn send(&mut self, params: SendParams) -> Result<String> {
       (**self).send(params).await
   }
   async fn get_utxos(&self, include_frozen: bool, addresses: Option<Vec<String>>) -> Result<Vec<(bitcoin::OutPoint, UtxoInfo)>> {
       (**self).get_utxos(include_frozen, addresses).await
   }
   async fn get_history(&self, count: u32, address: Option<String>) -> Result<Vec<TransactionInfo>> {
       (**self).get_history(count, address).await
   }
   async fn freeze_utxo(&self, utxo: String, reason: Option<String>) -> Result<()> {
       (**self).freeze_utxo(utxo, reason).await
   }
   async fn unfreeze_utxo(&self, utxo: String) -> Result<()> {
       (**self).unfreeze_utxo(utxo).await
   }
   async fn create_transaction(&self, params: SendParams) -> Result<String> {
       (**self).create_transaction(params).await
   }
   async fn sign_transaction(&mut self, tx_hex: String) -> Result<String> {
       (**self).sign_transaction(tx_hex).await
   }
   async fn broadcast_transaction(&self, tx_hex: String) -> Result<String> {
       (**self).broadcast_transaction(tx_hex).await
   }
   async fn estimate_fee(&self, target: u32) -> Result<FeeEstimate> {
       (**self).estimate_fee(target).await
   }
   async fn get_fee_rates(&self) -> Result<FeeRates> {
       (**self).get_fee_rates().await
   }
   async fn sync(&self) -> Result<()> {
       (**self).sync().await
   }
   async fn backup(&self) -> Result<String> {
       (**self).backup().await
   }
   async fn get_mnemonic(&self) -> Result<Option<String>> {
       (**self).get_mnemonic().await
   }
   fn get_network(&self) -> bitcoin::Network {
       (**self).get_network()
   }
   async fn get_master_public_key(&self) -> Result<Option<String>> {
       (**self).get_master_public_key().await
   }
   async fn get_internal_key(&self) -> Result<(bitcoin::XOnlyPublicKey, (Fingerprint, DerivationPath))> {
       (**self).get_internal_key().await
   }
   async fn sign_psbt(&mut self, psbt: &bitcoin::psbt::Psbt) -> Result<bitcoin::psbt::Psbt> {
       (**self).sign_psbt(psbt).await
   }
   async fn get_keypair(&self) -> Result<bitcoin::secp256k1::Keypair> {
       (**self).get_keypair().await
   }
   fn set_passphrase(&mut self, passphrase: Option<String>) {
       (**self).set_passphrase(passphrase)
   }

   async fn get_last_used_address_index(&self) -> Result<u32> {
       (**self).get_last_used_address_index().await
   }

   async fn get_enriched_utxos(&self, addresses: Option<Vec<String>>) -> Result<Vec<crate::provider::EnrichedUtxo>> {
       (**self).get_enriched_utxos(addresses).await
   }

   async fn get_all_balances(&self, addresses: Option<Vec<String>>) -> Result<crate::provider::AllBalances> {
       (**self).get_all_balances(addresses).await
   }
}

#[cfg(target_arch = "wasm32")]
#[async_trait(?Send)]
impl<T: AlkanesProvider + ?Sized> WalletProvider for Box<T> {
    async fn create_wallet(&mut self, config: WalletConfig, mnemonic: Option<String>, passphrase: Option<String>) -> Result<WalletInfo> {
        (**self).create_wallet(config, mnemonic, passphrase).await
    }
    async fn load_wallet(&mut self, config: WalletConfig, passphrase: Option<String>) -> Result<WalletInfo> {
        (**self).load_wallet(config, passphrase).await
    }
    async fn get_balance(&self, addresses: Option<Vec<String>>) -> Result<WalletBalance> {
        WalletProvider::get_balance(&**self, addresses).await
    }
    async fn get_address(&self) -> Result<String> {
        WalletProvider::get_address(&**self).await
    }
    async fn get_addresses(&self, count: u32) -> Result<Vec<AddressInfo>> {
        (**self).get_addresses(count).await
    }
    async fn send(&mut self, params: SendParams) -> Result<String> {
        (**self).send(params).await
    }
    async fn get_utxos(&self, include_frozen: bool, addresses: Option<Vec<String>>) -> Result<Vec<(bitcoin::OutPoint, UtxoInfo)>> {
        (**self).get_utxos(include_frozen, addresses).await
    }
    async fn get_history(&self, count: u32, address: Option<String>) -> Result<Vec<TransactionInfo>> {
        (**self).get_history(count, address).await
    }
    async fn freeze_utxo(&self, utxo: String, reason: Option<String>) -> Result<()> {
        (**self).freeze_utxo(utxo, reason).await
    }
    async fn unfreeze_utxo(&self, utxo: String) -> Result<()> {
        (**self).unfreeze_utxo(utxo).await
    }
    async fn create_transaction(&self, params: SendParams) -> Result<String> {
        (**self).create_transaction(params).await
    }
    async fn sign_transaction(&mut self, tx_hex: String) -> Result<String> {
        (**self).sign_transaction(tx_hex).await
    }
    async fn broadcast_transaction(&self, tx_hex: String) -> Result<String> {
        (**self).broadcast_transaction(tx_hex).await
    }
    async fn estimate_fee(&self, target: u32) -> Result<FeeEstimate> {
        (**self).estimate_fee(target).await
    }
    async fn get_fee_rates(&self) -> Result<FeeRates> {
        (**self).get_fee_rates().await
    }
    async fn sync(&self) -> Result<()> {
        (**self).sync().await
    }
    async fn backup(&self) -> Result<String> {
        (**self).backup().await
    }
    async fn get_mnemonic(&self) -> Result<Option<String>> {
        (**self).get_mnemonic().await
    }
    fn get_network(&self) -> bitcoin::Network {
        (**self).get_network()
    }
    async fn get_master_public_key(&self) -> Result<Option<String>> {
        (**self).get_master_public_key().await
    }
    async fn get_internal_key(&self) -> Result<(bitcoin::XOnlyPublicKey, (Fingerprint, DerivationPath))> {
        (**self).get_internal_key().await
    }
    async fn sign_psbt(&mut self, psbt: &bitcoin::psbt::Psbt) -> Result<bitcoin::psbt::Psbt> {
        (**self).sign_psbt(psbt).await
    }
    async fn get_keypair(&self) -> Result<bitcoin::secp256k1::Keypair> {
        (**self).get_keypair().await
    }
    fn set_passphrase(&mut self, passphrase: Option<String>) {
        (**self).set_passphrase(passphrase)
    }
 
    async fn get_last_used_address_index(&self) -> Result<u32> {
        (**self).get_last_used_address_index().await
    }
 
    async fn get_enriched_utxos(&self, addresses: Option<Vec<String>>) -> Result<Vec<crate::provider::EnrichedUtxo>> {
        (**self).get_enriched_utxos(addresses).await
    }
 
    async fn get_all_balances(&self, addresses: Option<Vec<String>>) -> Result<crate::provider::AllBalances> {
        (**self).get_all_balances(addresses).await
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
impl<T: AlkanesProvider + ?Sized + Send + Sync> AddressResolver for Box<T> {
   async fn resolve_all_identifiers(&self, input: &str) -> Result<String> {
       (**self).resolve_all_identifiers(input).await
   }
   fn contains_identifiers(&self, input: &str) -> bool {
       (**self).contains_identifiers(input)
   }
   async fn get_address(&self, address_type: &str, index: u32) -> Result<String> {
       AddressResolver::get_address(&**self, address_type, index).await
   }
   async fn list_identifiers(&self) -> Result<Vec<String>> {
       (**self).list_identifiers().await
   }
}

#[cfg(target_arch = "wasm32")]
#[async_trait(?Send)]
impl<T: AlkanesProvider + ?Sized> AddressResolver for Box<T> {
    async fn resolve_all_identifiers(&self, input: &str) -> Result<String> {
        (**self).resolve_all_identifiers(input).await
    }
    fn contains_identifiers(&self, input: &str) -> bool {
        (**self).contains_identifiers(input)
    }
    async fn get_address(&self, address_type: &str, index: u32) -> Result<String> {
        AddressResolver::get_address(&**self, address_type, index).await
    }
    async fn list_identifiers(&self) -> Result<Vec<String>> {
        (**self).list_identifiers().await
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
impl<T: AlkanesProvider + ?Sized + Send + Sync> BitcoinRpcProvider for Box<T> {
   async fn get_block_count(&self) -> Result<u64> {
       (**self).get_block_count().await
   }
   async fn generate_to_address(&self, nblocks: u32, address: &str) -> Result<serde_json::Value> {
       (**self).generate_to_address(nblocks, address).await
   }
   async fn get_blockchain_info(&self) -> Result<serde_json::Value> {
        BitcoinRpcProvider::get_blockchain_info(&**self).await
   }
   async fn get_new_address(&self) -> Result<JsonValue> {
       (**self).get_new_address().await
   }
   async fn get_transaction_hex(&self, txid: &str) -> Result<String> {
       (**self).get_transaction_hex(txid).await
   }
   async fn get_block(&self, hash: &str, raw: bool) -> Result<serde_json::Value> {
       BitcoinRpcProvider::get_block(&**self, hash, raw).await
   }
   async fn get_block_hash(&self, height: u64) -> Result<String> {
       BitcoinRpcProvider::get_block_hash(&**self, height).await
   }
   async fn send_raw_transaction(&self, tx_hex: &str) -> Result<String> {
       BitcoinRpcProvider::send_raw_transaction(&**self, tx_hex).await
   }
   async fn get_mempool_info(&self) -> Result<serde_json::Value> {
       BitcoinRpcProvider::get_mempool_info(&**self).await
   }
   async fn estimate_smart_fee(&self, target: u32) -> Result<serde_json::Value> {
       (**self).estimate_smart_fee(target).await
   }
   async fn get_esplora_blocks_tip_height(&self) -> Result<u64> {
       (**self).get_esplora_blocks_tip_height().await
   }
   async fn trace_transaction(&self, txid: &str, vout: u32, block: Option<&str>, tx: Option<&str>) -> Result<serde_json::Value> {
       (**self).trace_transaction(txid, vout, block, tx).await
   }

   async fn get_network_info(&self) -> Result<JsonValue> {
       (**self).get_network_info().await
   }

   async fn get_raw_transaction(&self, txid: &str, block_hash: Option<&str>) -> Result<JsonValue> {
       (**self).get_raw_transaction(txid, block_hash).await
   }

   async fn get_block_header(&self, hash: &str) -> Result<JsonValue> {
       BitcoinRpcProvider::get_block_header(&**self, hash).await
   }

   async fn get_block_stats(&self, hash: &str) -> Result<JsonValue> {
       (**self).get_block_stats(hash).await
   }

   async fn get_chain_tips(&self) -> Result<JsonValue> {
       (**self).get_chain_tips().await
   }

   async fn get_raw_mempool(&self) -> Result<JsonValue> {
       (**self).get_raw_mempool().await
   }

   async fn get_tx_out(&self, txid: &str, vout: u32, include_mempool: bool) -> Result<JsonValue> {
       (**self).get_tx_out(txid, vout, include_mempool).await
   }
}

#[cfg(target_arch = "wasm32")]
#[async_trait(?Send)]
impl<T: AlkanesProvider + ?Sized> BitcoinRpcProvider for Box<T> {
    async fn get_block_count(&self) -> Result<u64> {
        (**self).get_block_count().await
    }
    async fn generate_to_address(&self, nblocks: u32, address: &str) -> Result<serde_json::Value> {
        (**self).generate_to_address(nblocks, address).await
    }
    async fn get_blockchain_info(&self) -> Result<serde_json::Value> {
         BitcoinRpcProvider::get_blockchain_info(&**self).await
    }
    async fn get_new_address(&self) -> Result<JsonValue> {
        (**self).get_new_address().await
    }
    async fn get_transaction_hex(&self, txid: &str) -> Result<String> {
        (**self).get_transaction_hex(txid).await
    }
    async fn get_block(&self, hash: &str, raw: bool) -> Result<serde_json::Value> {
        BitcoinRpcProvider::get_block(&**self, hash, raw).await
    }
    async fn get_block_hash(&self, height: u64) -> Result<String> {
        BitcoinRpcProvider::get_block_hash(&**self, height).await
    }
    async fn send_raw_transaction(&self, tx_hex: &str) -> Result<String> {
        BitcoinRpcProvider::send_raw_transaction(&**self, tx_hex).await
    }
    async fn get_mempool_info(&self) -> Result<serde_json::Value> {
        BitcoinRpcProvider::get_mempool_info(&**self).await
    }
    async fn estimate_smart_fee(&self, target: u32) -> Result<serde_json::Value> {
        (**self).estimate_smart_fee(target).await
    }
    async fn get_esplora_blocks_tip_height(&self) -> Result<u64> {
        (**self).get_esplora_blocks_tip_height().await
    }
    async fn trace_transaction(&self, txid: &str, vout: u32, block: Option<&str>, tx: Option<&str>) -> Result<serde_json::Value> {
        (**self).trace_transaction(txid, vout, block, tx).await
    }
 
    async fn get_network_info(&self) -> Result<JsonValue> {
        (**self).get_network_info().await
    }
 
    async fn get_raw_transaction(&self, txid: &str, block_hash: Option<&str>) -> Result<JsonValue> {
        (**self).get_raw_transaction(txid, block_hash).await
    }
 
    async fn get_block_header(&self, hash: &str) -> Result<JsonValue> {
        BitcoinRpcProvider::get_block_header(&**self, hash).await
    }
 
    async fn get_block_stats(&self, hash: &str) -> Result<JsonValue> {
        (**self).get_block_stats(hash).await
    }
 
    async fn get_chain_tips(&self) -> Result<JsonValue> {
        (**self).get_chain_tips().await
    }
 
    async fn get_raw_mempool(&self) -> Result<JsonValue> {
        (**self).get_raw_mempool().await
    }
 
    async fn get_tx_out(&self, txid: &str, vout: u32, include_mempool: bool) -> Result<JsonValue> {
        (**self).get_tx_out(txid, vout, include_mempool).await
    }
}


#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
impl<T: AlkanesProvider + ?Sized + Send + Sync> MetashrewRpcProvider for Box<T> {
   async fn get_metashrew_height(&self) -> Result<u64> {
       (**self).get_metashrew_height().await
   }
   async fn get_state_root(&self, height: JsonValue) -> Result<String> {
       MetashrewRpcProvider::get_state_root(&**self, height).await
   }
   async fn get_contract_meta(&self, block: &str, tx: &str) -> Result<serde_json::Value> {
       (**self).get_contract_meta(block, tx).await
   }
   async fn trace_outpoint(&self, txid: &str, vout: u32) -> Result<JsonValue> {
        (**self).trace_outpoint(txid, vout).await
    }
   async fn get_spendables_by_address(&self, address: &str) -> Result<serde_json::Value> {
       (**self).get_spendables_by_address(address).await
   }
    async fn get_protorunes_by_address(
        &self,
        address: &str,
        block_tag: Option<String>,
        protocol_tag: u128,
    ) -> Result<ProtoruneWalletResponse> {
        (**self).get_protorunes_by_address(address, block_tag, protocol_tag).await
    }
    async fn get_protorunes_by_outpoint(
        &self,
        txid: &str,
        vout: u32,
        block_tag: Option<String>,
        protocol_tag: u128,
    ) -> Result<ProtoruneOutpointResponse> {
        (**self).get_protorunes_by_outpoint(txid, vout, block_tag, protocol_tag).await
    }
}

#[cfg(target_arch = "wasm32")]
#[async_trait(?Send)]
impl<T: AlkanesProvider + ?Sized> MetashrewRpcProvider for Box<T> {
    async fn get_metashrew_height(&self) -> Result<u64> {
        (**self).get_metashrew_height().await
    }
    async fn get_state_root(&self, height: JsonValue) -> Result<String> {
        MetashrewRpcProvider::get_state_root(&**self, height).await
    }
    async fn get_contract_meta(&self, block: &str, tx: &str) -> Result<serde_json::Value> {
        (**self).get_contract_meta(block, tx).await
    }
    async fn trace_outpoint(&self, txid: &str, vout: u32) -> Result<JsonValue> {
         (**self).trace_outpoint(txid, vout).await
     }
    async fn get_spendables_by_address(&self, address: &str) -> Result<serde_json::Value> {
        (**self).get_spendables_by_address(address).await
    }
     async fn get_protorunes_by_address(
         &self,
         address: &str,
         block_tag: Option<String>,
         protocol_tag: u128,
     ) -> Result<ProtoruneWalletResponse> {
         (**self).get_protorunes_by_address(address, block_tag, protocol_tag).await
     }
     async fn get_protorunes_by_outpoint(
         &self,
         txid: &str,
         vout: u32,
         block_tag: Option<String>,
         protocol_tag: u128,
     ) -> Result<ProtoruneOutpointResponse> {
         (**self).get_protorunes_by_outpoint(txid, vout, block_tag, protocol_tag).await
     }
}

#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
impl<T: AlkanesProvider + ?Sized + Send + Sync> MetashrewProvider for Box<T> {
    async fn get_height(&self) -> Result<u64> {
        (**self).get_height().await
    }
    async fn get_block_hash(&self, height: u64) -> Result<String> {
        MetashrewProvider::get_block_hash(&**self, height).await
    }
    async fn get_state_root(&self, height: JsonValue) -> Result<String> {
        MetashrewProvider::get_state_root(&**self, height).await
    }
}

#[cfg(target_arch = "wasm32")]
#[async_trait(?Send)]
impl<T: AlkanesProvider + ?Sized> MetashrewProvider for Box<T> {
    async fn get_height(&self) -> Result<u64> {
        (**self).get_height().await
    }
    async fn get_block_hash(&self, height: u64) -> Result<String> {
        MetashrewProvider::get_block_hash(&**self, height).await
    }
    async fn get_state_root(&self, height: JsonValue) -> Result<String> {
        MetashrewProvider::get_state_root(&**self, height).await
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
impl<T: AlkanesProvider + ?Sized + Send + Sync> EsploraProvider for Box<T> {
   async fn get_blocks_tip_hash(&self) -> Result<String> {
       (**self).get_blocks_tip_hash().await
   }
   async fn get_blocks_tip_height(&self) -> Result<u64> {
       (**self).get_blocks_tip_height().await
   }
   async fn get_blocks(&self, start_height: Option<u64>) -> Result<serde_json::Value> {
       (**self).get_blocks(start_height).await
   }
   async fn get_block_by_height(&self, height: u64) -> Result<String> {
       (**self).get_block_by_height(height).await
   }
   async fn get_block(&self, hash: &str) -> Result<serde_json::Value> {
       EsploraProvider::get_block(&**self, hash).await
   }
   async fn get_block_status(&self, hash: &str) -> Result<serde_json::Value> {
       (**self).get_block_status(hash).await
   }
   async fn get_block_txids(&self, hash: &str) -> Result<serde_json::Value> {
       (**self).get_block_txids(hash).await
   }
   async fn get_block_header(&self, hash: &str) -> Result<String> {
       EsploraProvider::get_block_header(&**self, hash).await
   }
   async fn get_block_raw(&self, hash: &str) -> Result<String> {
       (**self).get_block_raw(hash).await
   }
   async fn get_block_txid(&self, hash: &str, index: u32) -> Result<String> {
       (**self).get_block_txid(hash, index).await
   }
   async fn get_block_txs(&self, hash: &str, start_index: Option<u32>) -> Result<serde_json::Value> {
       (**self).get_block_txs(hash, start_index).await
   }
   async fn get_address_info(&self, address: &str) -> Result<JsonValue> {
       (**self).get_address_info(address).await
   }
    async fn get_address_utxo(&self, address: &str) -> Result<JsonValue> {
        (**self).get_address_utxo(address).await
    }
   async fn get_address_txs(&self, address: &str) -> Result<serde_json::Value> {
       (**self).get_address_txs(address).await
   }
   async fn get_address_txs_chain(&self, address: &str, last_seen_txid: Option<&str>) -> Result<serde_json::Value> {
       (**self).get_address_txs_chain(address, last_seen_txid).await
   }
   async fn get_address_txs_mempool(&self, address: &str) -> Result<serde_json::Value> {
       (**self).get_address_txs_mempool(address).await
   }
   async fn get_address_prefix(&self, prefix: &str) -> Result<serde_json::Value> {
       (**self).get_address_prefix(prefix).await
   }
   async fn get_tx(&self, txid: &str) -> Result<serde_json::Value> {
       (**self).get_tx(txid).await
   }
   async fn get_tx_hex(&self, txid: &str) -> Result<String> {
       (**self).get_tx_hex(txid).await
   }
   async fn get_tx_raw(&self, txid: &str) -> Result<String> {
       (**self).get_tx_raw(txid).await
   }
   async fn get_tx_status(&self, txid: &str) -> Result<serde_json::Value> {
       (**self).get_tx_status(txid).await
   }
   async fn get_tx_merkle_proof(&self, txid: &str) -> Result<serde_json::Value> {
       (**self).get_tx_merkle_proof(txid).await
   }
   async fn get_tx_merkleblock_proof(&self, txid: &str) -> Result<String> {
       (**self).get_tx_merkleblock_proof(txid).await
   }
   async fn get_tx_outspend(&self, txid: &str, index: u32) -> Result<serde_json::Value> {
       (**self).get_tx_outspend(txid, index).await
   }
   async fn get_tx_outspends(&self, txid: &str) -> Result<serde_json::Value> {
       (**self).get_tx_outspends(txid).await
   }
   async fn broadcast(&self, tx_hex: &str) -> Result<String> {
       (**self).broadcast(tx_hex).await
   }
   async fn get_mempool(&self) -> Result<serde_json::Value> {
       (**self).get_mempool().await
   }
   async fn get_mempool_txids(&self) -> Result<serde_json::Value> {
       (**self).get_mempool_txids().await
   }
   async fn get_mempool_recent(&self) -> Result<serde_json::Value> {
       (**self).get_mempool_recent().await
   }
   async fn get_fee_estimates(&self) -> Result<serde_json::Value> {
       (**self).get_fee_estimates().await
   }
}

#[cfg(target_arch = "wasm32")]
#[async_trait(?Send)]
impl<T: AlkanesProvider + ?Sized> EsploraProvider for Box<T> {
    async fn get_blocks_tip_hash(&self) -> Result<String> {
        (**self).get_blocks_tip_hash().await
    }
    async fn get_blocks_tip_height(&self) -> Result<u64> {
        (**self).get_blocks_tip_height().await
    }
    async fn get_blocks(&self, start_height: Option<u64>) -> Result<serde_json::Value> {
        (**self).get_blocks(start_height).await
    }
    async fn get_block_by_height(&self, height: u64) -> Result<String> {
        (**self).get_block_by_height(height).await
    }
    async fn get_block(&self, hash: &str) -> Result<serde_json::Value> {
        EsploraProvider::get_block(&**self, hash).await
    }
    async fn get_block_status(&self, hash: &str) -> Result<serde_json::Value> {
        (**self).get_block_status(hash).await
    }
    async fn get_block_txids(&self, hash: &str) -> Result<serde_json::Value> {
        (**self).get_block_txids(hash).await
    }
    async fn get_block_header(&self, hash: &str) -> Result<String> {
        EsploraProvider::get_block_header(&**self, hash).await
    }
    async fn get_block_raw(&self, hash: &str) -> Result<String> {
        (**self).get_block_raw(hash).await
    }
    async fn get_block_txid(&self, hash: &str, index: u32) -> Result<String> {
        (**self).get_block_txid(hash, index).await
    }
    async fn get_block_txs(&self, hash: &str, start_index: Option<u32>) -> Result<serde_json::Value> {
        (**self).get_block_txs(hash, start_index).await
    }
    async fn get_address_info(&self, address: &str) -> Result<JsonValue> {
        (**self).get_address_info(address).await
    }
     async fn get_address_utxo(&self, address: &str) -> Result<JsonValue> {
         (**self).get_address_utxo(address).await
     }
    async fn get_address_txs(&self, address: &str) -> Result<serde_json::Value> {
        (**self).get_address_txs(address).await
    }
    async fn get_address_txs_chain(&self, address: &str, last_seen_txid: Option<&str>) -> Result<serde_json::Value> {
        (**self).get_address_txs_chain(address, last_seen_txid).await
    }
    async fn get_address_txs_mempool(&self, address: &str) -> Result<serde_json::Value> {
        (**self).get_address_txs_mempool(address).await
    }
    async fn get_address_prefix(&self, prefix: &str) -> Result<serde_json::Value> {
        (**self).get_address_prefix(prefix).await
    }
    async fn get_tx(&self, txid: &str) -> Result<serde_json::Value> {
        (**self).get_tx(txid).await
    }
    async fn get_tx_hex(&self, txid: &str) -> Result<String> {
        (**self).get_tx_hex(txid).await
    }
    async fn get_tx_raw(&self, txid: &str) -> Result<String> {
        (**self).get_tx_raw(txid).await
    }
    async fn get_tx_status(&self, txid: &str) -> Result<serde_json::Value> {
        (**self).get_tx_status(txid).await
    }
    async fn get_tx_merkle_proof(&self, txid: &str) -> Result<serde_json::Value> {
        (**self).get_tx_merkle_proof(txid).await
    }
    async fn get_tx_merkleblock_proof(&self, txid: &str) -> Result<String> {
        (**self).get_tx_merkleblock_proof(txid).await
    }
    async fn get_tx_outspend(&self, txid: &str, index: u32) -> Result<serde_json::Value> {
        (**self).get_tx_outspend(txid, index).await
    }
    async fn get_tx_outspends(&self, txid: &str) -> Result<serde_json::Value> {
        (**self).get_tx_outspends(txid).await
    }
    async fn broadcast(&self, tx_hex: &str) -> Result<String> {
        (**self).broadcast(tx_hex).await
    }
    async fn get_mempool(&self) -> Result<serde_json::Value> {
        (**self).get_mempool().await
    }
    async fn get_mempool_txids(&self) -> Result<serde_json::Value> {
        (**self).get_mempool_txids().await
    }
    async fn get_mempool_recent(&self) -> Result<serde_json::Value> {
        (**self).get_mempool_recent().await
    }
    async fn get_fee_estimates(&self) -> Result<serde_json::Value> {
        (**self).get_fee_estimates().await
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
impl<T: AlkanesProvider + ?Sized + Send + Sync> RunestoneProvider for Box<T> {
   async fn decode_runestone(&self, tx: &bitcoin::Transaction) -> Result<serde_json::Value> {
       (**self).decode_runestone(tx).await
   }
   async fn format_runestone_with_decoded_messages(&self, tx: &bitcoin::Transaction) -> Result<serde_json::Value> {
       (**self).format_runestone_with_decoded_messages(tx).await
   }
   async fn analyze_runestone(&self, txid: &str) -> Result<serde_json::Value> {
       (**self).analyze_runestone(txid).await
   }
}

#[cfg(target_arch = "wasm32")]
#[async_trait(?Send)]
impl<T: AlkanesProvider + ?Sized> RunestoneProvider for Box<T> {
    async fn decode_runestone(&self, tx: &bitcoin::Transaction) -> Result<serde_json::Value> {
        (**self).decode_runestone(tx).await
    }
    async fn format_runestone_with_decoded_messages(&self, tx: &bitcoin::Transaction) -> Result<serde_json::Value> {
        (**self).format_runestone_with_decoded_messages(tx).await
    }
    async fn analyze_runestone(&self, txid: &str) -> Result<serde_json::Value> {
        (**self).analyze_runestone(txid).await
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
impl<T: AlkanesProvider + ?Sized + Send + Sync> OrdProvider for Box<T> {
    async fn get_inscription(&self, inscription_id: &str) -> Result<OrdInscription> {
        (**self).get_inscription(inscription_id).await
    }

    async fn get_inscriptions_in_block(&self, block_hash: &str) -> Result<OrdInscriptions> {
        (**self).get_inscriptions_in_block(block_hash).await
    }
    async fn get_ord_address_info(&self, address: &str) -> Result<OrdAddressInfo> {
        (**self).get_ord_address_info(address).await
    }
    async fn get_block_info(&self, query: &str) -> Result<OrdBlock> {
        (**self).get_block_info(query).await
    }
    async fn get_ord_block_count(&self) -> Result<u64> {
        (**self).get_ord_block_count().await
    }
    async fn get_ord_blocks(&self) -> Result<OrdBlocks> {
        (**self).get_ord_blocks().await
    }
    async fn get_children(&self, inscription_id: &str, page: Option<u32>) -> Result<OrdChildren> {
        (**self).get_children(inscription_id, page).await
    }
    async fn get_content(&self, inscription_id: &str) -> Result<Vec<u8>> {
        (**self).get_content(inscription_id).await
    }
    async fn get_inscriptions(&self, page: Option<u32>) -> Result<OrdInscriptions> {
        (**self).get_inscriptions(page).await
    }
    async fn get_output(&self, output: &str) -> Result<OrdOutput> {
        (**self).get_output(output).await
    }
    async fn get_parents(&self, inscription_id: &str, page: Option<u32>) -> Result<OrdParents> {
        (**self).get_parents(inscription_id, page).await
    }
    async fn get_rune(&self, rune: &str) -> Result<OrdRuneInfo> {
        (**self).get_rune(rune).await
    }
    async fn get_runes(&self, page: Option<u32>) -> Result<OrdRunes> {
        (**self).get_runes(page).await
    }
    async fn get_sat(&self, sat: u64) -> Result<OrdSat> {
        (**self).get_sat(sat).await
    }
    async fn get_tx_info(&self, txid: &str) -> Result<OrdTxInfo> {
        (**self).get_tx_info(txid).await
    }
}

#[cfg(target_arch = "wasm32")]
#[async_trait(?Send)]
impl<T: AlkanesProvider + ?Sized> OrdProvider for Box<T> {
    async fn get_inscription(&self, inscription_id: &str) -> Result<OrdInscription> {
        (**self).get_inscription(inscription_id).await
    }

    async fn get_inscriptions_in_block(&self, block_hash: &str) -> Result<OrdInscriptions> {
        (**self).get_inscriptions_in_block(block_hash).await
    }
    async fn get_ord_address_info(&self, address: &str) -> Result<OrdAddressInfo> {
        (**self).get_ord_address_info(address).await
    }
    async fn get_block_info(&self, query: &str) -> Result<OrdBlock> {
        (**self).get_block_info(query).await
    }
    async fn get_ord_block_count(&self) -> Result<u64> {
        (**self).get_ord_block_count().await
    }
    async fn get_ord_blocks(&self) -> Result<OrdBlocks> {
        (**self).get_ord_blocks().await
    }
    async fn get_children(&self, inscription_id: &str, page: Option<u32>) -> Result<OrdChildren> {
        (**self).get_children(inscription_id, page).await
    }
    async fn get_content(&self, inscription_id: &str) -> Result<Vec<u8>> {
        (**self).get_content(inscription_id).await
    }
    async fn get_inscriptions(&self, page: Option<u32>) -> Result<OrdInscriptions> {
        (**self).get_inscriptions(page).await
    }
    async fn get_output(&self, output: &str) -> Result<OrdOutput> {
        (**self).get_output(output).await
    }
    async fn get_parents(&self, inscription_id: &str, page: Option<u32>) -> Result<OrdParents> {
        (**self).get_parents(inscription_id, page).await
    }
    async fn get_rune(&self, rune: &str) -> Result<OrdRuneInfo> {
        (**self).get_rune(rune).await
    }
    async fn get_runes(&self, page: Option<u32>) -> Result<OrdRunes> {
        (**self).get_runes(page).await
    }
    async fn get_sat(&self, sat: u64) -> Result<OrdSat> {
        (**self).get_sat(sat).await
    }
    async fn get_tx_info(&self, txid: &str) -> Result<OrdTxInfo> {
        (**self).get_tx_info(txid).await
    }
}


#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
impl<T: AlkanesProvider + ?Sized + Send + Sync> MonitorProvider for Box<T> {
   async fn monitor_blocks(&self, start: Option<u64>) -> Result<()> {
       (**self).monitor_blocks(start).await
   }
   async fn get_block_events(&self, height: u64) -> Result<Vec<BlockEvent>> {
       (**self).get_block_events(height).await
   }
}

#[cfg(target_arch = "wasm32")]
#[async_trait(?Send)]
impl<T: AlkanesProvider + ?Sized> MonitorProvider for Box<T> {
    async fn monitor_blocks(&self, start: Option<u64>) -> Result<()> {
        (**self).monitor_blocks(start).await
    }
    async fn get_block_events(&self, height: u64) -> Result<Vec<BlockEvent>> {
        (**self).get_block_events(height).await
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
impl<T: AlkanesProvider + ?Sized + Send + Sync> KeystoreProvider for Box<T> {
    async fn get_address(&self, address_type: &str, index: u32) -> Result<String> {
        KeystoreProvider::get_address(&**self, address_type, index).await
    }
   async fn derive_addresses(&self, master_public_key: &str, network_params: &NetworkParams, script_types: &[&str], start_index: u32, count: u32) -> Result<Vec<KeystoreAddress>> {
       (**self).derive_addresses(master_public_key, network_params, script_types, start_index, count).await
   }
   async fn get_default_addresses(&self, master_public_key: &str, network_params: &NetworkParams) -> Result<Vec<KeystoreAddress>> {
       (**self).get_default_addresses(master_public_key, network_params).await
   }
   fn parse_address_range(&self, range_spec: &str) -> Result<(String, u32, u32)> {
       (**self).parse_address_range(range_spec)
   }
   async fn get_keystore_info(&self, master_fingerprint: &str, created_at: u64, version: &str) -> Result<KeystoreInfo> {
       (**self).get_keystore_info(master_fingerprint, created_at, version).await
   }
   async fn derive_address_from_path(&self, master_public_key: &str, path: &DerivationPath, script_type: &str, network_params: &NetworkParams) -> Result<KeystoreAddress> {
       (**self).derive_address_from_path(master_public_key, path, script_type, network_params).await
   }
}

#[cfg(target_arch = "wasm32")]
#[async_trait(?Send)]
impl<T: AlkanesProvider + ?Sized> KeystoreProvider for Box<T> {
    async fn get_address(&self, address_type: &str, index: u32) -> Result<String> {
        KeystoreProvider::get_address(&**self, address_type, index).await
    }
   async fn derive_addresses(&self, master_public_key: &str, network_params: &NetworkParams, script_types: &[&str], start_index: u32, count: u32) -> Result<Vec<KeystoreAddress>> {
       (**self).derive_addresses(master_public_key, network_params, script_types, start_index, count).await
   }
   async fn get_default_addresses(&self, master_public_key: &str, network_params: &NetworkParams) -> Result<Vec<KeystoreAddress>> {
       (**self).get_default_addresses(master_public_key, network_params).await
   }
   fn parse_address_range(&self, range_spec: &str) -> Result<(String, u32, u32)> {
       (**self).parse_address_range(range_spec)
   }
   async fn get_keystore_info(&self, master_fingerprint: &str, created_at: u64, version: &str) -> Result<KeystoreInfo> {
       (**self).get_keystore_info(master_fingerprint, created_at, version).await
   }
   async fn derive_address_from_path(&self, master_public_key: &str, path: &DerivationPath, script_type: &str, network_params: &NetworkParams) -> Result<KeystoreAddress> {
       (**self).derive_address_from_path(master_public_key, path, script_type, network_params).await
   }
}

#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
impl<T: AlkanesProvider + ?Sized + Send + Sync> AlkanesProvider for Box<T> {
    fn provider_name(&self) -> &str {
        (**self).provider_name()
    }
    fn get_bitcoin_rpc_url(&self) -> Option<String> {
        (**self).get_bitcoin_rpc_url()
    }
    fn get_esplora_api_url(&self) -> Option<String> {
        (**self).get_esplora_api_url()
    }
    fn get_ord_server_url(&self) -> Option<String> {
        (**self).get_ord_server_url()
    }
    fn get_metashrew_rpc_url(&self) -> Option<String> {
        (**self).get_metashrew_rpc_url()
    }
    fn clone_box(&self) -> Box<dyn AlkanesProvider> {
        AlkanesProvider::clone_box(&**self)
    }
    async fn initialize(&self) -> Result<()> {
        (**self).initialize().await
    }
    async fn shutdown(&self) -> Result<()> {
        (**self).shutdown().await
    }
    fn secp(&self) -> &bitcoin::secp256k1::Secp256k1<bitcoin::secp256k1::All> {
        (**self).secp()
    }
    async fn get_utxo(&self, outpoint: &bitcoin::OutPoint) -> Result<Option<bitcoin::TxOut>> {
        (**self).get_utxo(outpoint).await
    }
    async fn sign_taproot_script_spend(&self, sighash: bitcoin::secp256k1::Message) -> Result<bitcoin::secp256k1::schnorr::Signature> {
        (**self).sign_taproot_script_spend(sighash).await
    }

    async fn wrap(&mut self, amount: u64, address: Option<String>, fee_rate: Option<f32>) -> Result<String> {
        (**self).wrap(amount, address, fee_rate).await
    }

    async fn unwrap(&mut self, amount: u64, address: Option<String>) -> Result<String> {
        (**self).unwrap(amount, address).await
    }

    async fn execute(&mut self, params: EnhancedExecuteParams) -> Result<ExecutionState> {
        (**self).execute(params).await
    }

    async fn resume_execution(
        &mut self,
        state: ReadyToSignTx,
        params: &EnhancedExecuteParams,
    ) -> Result<EnhancedExecuteResult> {
        (**self).resume_execution(state, params).await
    }

    async fn resume_commit_execution(
        &mut self,
        state: ReadyToSignCommitTx,
    ) -> Result<ExecutionState> {
        (**self).resume_commit_execution(state).await
    }

    async fn resume_reveal_execution(
        &mut self,
        state: ReadyToSignRevealTx,
    ) -> Result<EnhancedExecuteResult> {
        (**self).resume_reveal_execution(state).await
    }
    
    async fn protorunes_by_address(
        &self,
        address: &str,
        block_tag: Option<String>,
        protocol_tag: u128,
    ) -> Result<ProtoruneWalletResponse> {
        (**self).protorunes_by_address(address, block_tag, protocol_tag).await
    }

    async fn protorunes_by_outpoint(
        &self,
        txid: &str,
        vout: u32,
        block_tag: Option<String>,
        protocol_tag: u128,
    ) -> Result<ProtoruneOutpointResponse> {
        (**self).protorunes_by_outpoint(txid, vout, block_tag, protocol_tag).await
    }

    async fn simulate(&self, contract_id: &str, context: &crate::alkanes_pb::MessageContextParcel) -> Result<JsonValue> {
        (**self).simulate(contract_id, context).await
    }

    async fn trace(&self, outpoint: &str) -> Result<alkanes_pb::Trace> {
        (**self).trace(outpoint).await
    }

    async fn get_block(&self, height: u64) -> Result<alkanes_pb::BlockResponse> {
        AlkanesProvider::get_block(&**self, height).await
    }

    async fn sequence(&self) -> Result<JsonValue> {
        (**self).sequence().await
    }

    async fn spendables_by_address(&self, address: &str) -> Result<JsonValue> {
        (**self).spendables_by_address(address).await
    }

    async fn trace_block(&self, height: u64) -> Result<alkanes_pb::Trace> {
        (**self).trace_block(height).await
    }

    async fn get_bytecode(&self, alkane_id: &str, block_tag: Option<String>) -> Result<String> {
        (**self).get_bytecode(alkane_id, block_tag).await
    }

    async fn inspect(&self, target: &str, config: crate::alkanes::AlkanesInspectConfig) -> Result<crate::alkanes::AlkanesInspectResult> {
        (**self).inspect(target, config).await
    }

    async fn get_balance(&self, address: Option<&str>) -> Result<Vec<crate::alkanes::AlkaneBalance>> {
        AlkanesProvider::get_balance(&**self, address).await
    }

    async fn view(&self, contract_id: &str, view_fn: &str, params: Option<&[u8]>) -> Result<JsonValue> {
        (**self).view(contract_id, view_fn, params).await
    }
}

#[cfg(target_arch = "wasm32")]
#[async_trait(?Send)]
impl<T: AlkanesProvider + ?Sized> AlkanesProvider for Box<T> {
    fn provider_name(&self) -> &str {
        (**self).provider_name()
    }
    fn get_bitcoin_rpc_url(&self) -> Option<String> {
        (**self).get_bitcoin_rpc_url()
    }
    fn get_esplora_api_url(&self) -> Option<String> {
        (**self).get_esplora_api_url()
    }
    fn get_ord_server_url(&self) -> Option<String> {
        (**self).get_ord_server_url()
    }
    fn get_metashrew_rpc_url(&self) -> Option<String> {
        (**self).get_metashrew_rpc_url()
    }
    fn clone_box(&self) -> Box<dyn AlkanesProvider> {
        AlkanesProvider::clone_box(&**self)
    }
    async fn initialize(&self) -> Result<()> {
        (**self).initialize().await
    }
    async fn shutdown(&self) -> Result<()> {
        (**self).shutdown().await
    }
    fn secp(&self) -> &bitcoin::secp256k1::Secp256k1<bitcoin::secp256k1::All> {
        (**self).secp()
    }
    async fn get_utxo(&self, outpoint: &bitcoin::OutPoint) -> Result<Option<bitcoin::TxOut>> {
        (**self).get_utxo(outpoint).await
    }
    async fn sign_taproot_script_spend(&self, sighash: bitcoin::secp256k1::Message) -> Result<bitcoin::secp256k1::schnorr::Signature> {
        (**self).sign_taproot_script_spend(sighash).await
    }

    async fn wrap(&mut self, amount: u64, address: Option<String>, fee_rate: Option<f32>) -> Result<String> {
        (**self).wrap(amount, address, fee_rate).await
    }

    async fn unwrap(&mut self, amount: u64, address: Option<String>) -> Result<String> {
        (**self).unwrap(amount, address).await
    }

    async fn execute(&mut self, params: EnhancedExecuteParams) -> Result<ExecutionState> {
        (**self).execute(params).await
    }

    async fn resume_execution(
        &mut self,
        state: ReadyToSignTx,
        params: &EnhancedExecuteParams,
    ) -> Result<EnhancedExecuteResult> {
        (**self).resume_execution(state, params).await
    }

    async fn resume_commit_execution(
        &mut self,
        state: ReadyToSignCommitTx,
    ) -> Result<ExecutionState> {
        (**self).resume_commit_execution(state).await
    }

    async fn resume_reveal_execution(
        &mut self,
        state: ReadyToSignRevealTx,
    ) -> Result<EnhancedExecuteResult> {
        (**self).resume_reveal_execution(state).await
    }
    
    async fn protorunes_by_address(
        &self,
        address: &str,
        block_tag: Option<String>,
        protocol_tag: u128,
    ) -> Result<ProtoruneWalletResponse> {
        (**self).protorunes_by_address(address, block_tag, protocol_tag).await
    }

    async fn protorunes_by_outpoint(
        &self,
        txid: &str,
        vout: u32,
        block_tag: Option<String>,
        protocol_tag: u128,
    ) -> Result<ProtoruneOutpointResponse> {
        (**self).protorunes_by_outpoint(txid, vout, block_tag, protocol_tag).await
    }

    async fn simulate(&self, contract_id: &str, context: &crate::alkanes_pb::MessageContextParcel) -> Result<JsonValue> {
        (**self).simulate(contract_id, context).await
    }

    async fn trace(&self, outpoint: &str) -> Result<alkanes_pb::Trace> {
        (**self).trace(outpoint).await
    }

    async fn get_block(&self, height: u64) -> Result<alkanes_pb::BlockResponse> {
        AlkanesProvider::get_block(&**self, height).await
    }

    async fn sequence(&self) -> Result<JsonValue> {
        (**self).sequence().await
    }

    async fn spendables_by_address(&self, address: &str) -> Result<JsonValue> {
        (**self).spendables_by_address(address).await
    }

    async fn trace_block(&self, height: u64) -> Result<alkanes_pb::Trace> {
        (**self).trace_block(height).await
    }

    async fn get_bytecode(&self, alkane_id: &str, block_tag: Option<String>) -> Result<String> {
        (**self).get_bytecode(alkane_id, block_tag).await
    }

    async fn inspect(&self, target: &str, config: crate::alkanes::AlkanesInspectConfig) -> Result<crate::alkanes::AlkanesInspectResult> {
        (**self).inspect(target, config).await
    }

    async fn get_balance(&self, address: Option<&str>) -> Result<Vec<crate::alkanes::AlkaneBalance>> {
        AlkanesProvider::get_balance(&**self, address).await
    }

    async fn view(&self, contract_id: &str, view_fn: &str, params: Option<&[u8]>) -> Result<JsonValue> {
        (**self).view(contract_id, view_fn, params).await
    }
}

/// Trait for system-level wallet operations, corresponding to CLI commands
#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
pub trait SystemWallet: Send + Sync {
   /// Execute a wallet command
   async fn execute_wallet_command(&self, command: crate::commands::WalletCommands) -> Result<()>;
   /// Execute the legacy walletinfo command
   async fn execute_walletinfo_command(&self, raw: bool) -> Result<()>;
}

/// Trait for system-level Bitcoin Core RPC operations
#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
pub trait SystemBitcoind: Send + Sync {
   /// Execute a bitcoind command
   async fn execute_bitcoind_command(&self, command: crate::commands::BitcoindCommands) -> Result<()>;
}

/// Trait for system-level Metashrew RPC operations
#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
pub trait SystemMetashrew: Send + Sync {
   /// Execute a metashrew command
   async fn execute_metashrew_command(&self, command: crate::commands::MetashrewCommands) -> Result<()>;
}

/// Trait for system-level Alkanes operations
#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
pub trait SystemAlkanes: Send + Sync {
   /// Execute an alkanes command
   async fn execute_alkanes_command(&self, command: crate::commands::AlkanesCommands) -> Result<()>;
}

/// Trait for system-level Runestone operations
#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
pub trait SystemRunestone: Send + Sync {
   /// Execute a runestone command
   async fn execute_runestone_command(&self, command: crate::commands::RunestoneCommands) -> Result<()>;
}

/// Trait for system-level Protorunes operations
#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
pub trait SystemProtorunes: Send + Sync {
   /// Execute a protorunes command
   async fn execute_protorunes_command(&self, command: crate::commands::ProtorunesCommands) -> Result<()>;
}

/// Trait for system-level monitoring operations
#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
pub trait SystemMonitor: Send + Sync {
   /// Execute a monitor command
   async fn execute_monitor_command(&self, command: crate::commands::MonitorCommands) -> Result<()>;
}

/// Trait for system-level Esplora operations
#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
pub trait SystemEsplora: Send + Sync {
   /// Execute an esplora command
   async fn execute_esplora_command(&self, command: crate::commands::EsploraCommands) -> Result<()>;
}

/// Trait for system-level PGP operations
#[cfg(not(target_arch = "wasm32"))]

/// Combined system trait that includes all system-level functionality
#[cfg(not(target_arch = "wasm32"))]
pub trait System:
   SystemWallet +
   SystemBitcoind +
   SystemMetashrew +
   SystemAlkanes +
   SystemRunestone +
   SystemProtorunes +
   SystemMonitor +
   SystemEsplora +
   Send + Sync
{
   /// Get the underlying provider
   fn provider(&self) -> &dyn AlkanesProvider;
   /// Get the underlying provider mutably
   fn provider_mut(&mut self) -> &mut dyn AlkanesProvider;
}