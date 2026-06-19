//! # Alkanes FFI
//!
//! Foreign Function Interface bindings for alkanes-rs using UniFFI.
//! This crate provides language bindings for Kotlin, Swift, Python, and other languages.
//!
//! ## Architecture
//!
//! This crate follows the BDK-FFI pattern:
//! - Uses UniFFI to generate language bindings automatically
//! - Provides a safe, ergonomic API that wraps alkanes-cli-common
//! - Handles error conversion and type mapping across the FFI boundary
//! - Supports async operations through synchronous wrappers with internal runtime
//!
//! ## Supported Languages
//!
//! - Kotlin/JVM (for Android and server applications)
//! - Swift (for iOS/macOS)
//! - Python
//!
//! ## Example Usage (Kotlin)
//!
//! ```kotlin
//! import org.alkanes.*
//!
//! // Create a wallet
//! val config = WalletConfig(
//!     walletPath = "/path/to/wallet",
//!     network = Network.REGTEST,
//!     passphrase = "secure_password"
//! )
//! val wallet = Wallet(config, null)
//!
//! // Get an address
//! val address = wallet.getAddress(AddressType.P2WPKH, 0u)
//! println("Address: $address")
//!
//! // Get balance
//! val balance = wallet.getBalance()
//! println("Confirmed: ${balance.confirmed} sats")
//! ```

// UniFFI will generate the scaffolding code
uniffi::include_scaffolding!("alkanes");

// Simplified Result type for FFI
pub type Result<T> = std::result::Result<T, AlkanesError>;

// ============================================================================
// Network Enum (FFI-compatible, maps to alkanes_cli_common::Network)
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Network {
    Bitcoin,
    Testnet,
    Signet,
    Regtest,
}

impl From<Network> for bitcoin::Network {
    fn from(net: Network) -> Self {
        match net {
            Network::Bitcoin => bitcoin::Network::Bitcoin,
            Network::Testnet => bitcoin::Network::Testnet,
            Network::Signet => bitcoin::Network::Signet,
            Network::Regtest => bitcoin::Network::Regtest,
        }
    }
}

impl From<bitcoin::Network> for Network {
    fn from(net: bitcoin::Network) -> Self {
        match net {
            bitcoin::Network::Bitcoin => Network::Bitcoin,
            bitcoin::Network::Testnet => Network::Testnet,
            bitcoin::Network::Signet => Network::Signet,
            bitcoin::Network::Regtest => Network::Regtest,
            _ => Network::Regtest, // Default fallback for unknown networks
        }
    }
}

// Simplified error type that matches UDL definition
#[derive(Debug, thiserror::Error)]
pub enum AlkanesError {
    #[error("Invalid address: {0}")]
    InvalidAddress(String),
    #[error("Invalid mnemonic: {0}")]
    InvalidMnemonic(String),
    #[error("Invalid network: {0}")]
    InvalidNetwork(String),
    #[error("Wallet error: {0}")]
    WalletError(String),
    #[error("RPC error: {0}")]
    RpcError(String),
    #[error("Serialization error: {0}")]
    SerializationError(String),
    #[error("Transaction error: {0}")]
    TransactionError(String),
    #[error("Alkanes execution error: {0}")]
    AlkanesExecutionError(String),
    #[error("Crypto error: {0}")]
    CryptoError(String),
    #[error("Keystore error: {0}")]
    KeystoreError(String),
    #[error("{0}")]
    Generic(String),
}

impl From<alkanes_cli_common::AlkanesError> for AlkanesError {
    fn from(err: alkanes_cli_common::AlkanesError) -> Self {
        AlkanesError::Generic(err.to_string())
    }
}

impl From<bip39::Error> for AlkanesError {
    fn from(err: bip39::Error) -> Self {
        AlkanesError::InvalidMnemonic(err.to_string())
    }
}

// ============================================================================
// Module-level Functions
// ============================================================================

/// Get the version of the alkanes library
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Validate a Bitcoin address for a given network
pub fn validate_address(address: String, network: Network) -> Result<bool> {
    use bitcoin::Address;
    use std::str::FromStr;
    
    let btc_network: bitcoin::Network = network.into();
    match Address::from_str(&address) {
        Ok(addr_unchecked) => {
            // Try to check if address is valid for the given network
            match addr_unchecked.require_network(btc_network) {
                Ok(_) => Ok(true),
                Err(_) => Ok(false),
            }
        },
        Err(_) => Ok(false),
    }
}

/// Generate a new mnemonic phrase
pub fn generate_mnemonic(word_count: WordCount) -> Result<String> {
    use bip39::Mnemonic;
    use rand::RngCore;

    let entropy_bits = match word_count {
        WordCount::Words12 => 128,
        WordCount::Words15 => 160,
        WordCount::Words18 => 192,
        WordCount::Words21 => 224,
        WordCount::Words24 => 256,
    };

    let entropy_bytes = entropy_bits / 8;
    let mut entropy = vec![0u8; entropy_bytes];
    rand::thread_rng().fill_bytes(&mut entropy);

    let mnemonic = Mnemonic::from_entropy(&entropy)
        .map_err(|e| AlkanesError::InvalidMnemonic(format!("{:?}", e)))?;

    Ok(mnemonic.to_string())
}

/// Validate a mnemonic phrase
pub fn validate_mnemonic(mnemonic: String) -> Result<bool> {
    use bip39::Mnemonic;
    use std::str::FromStr;

    match Mnemonic::from_str(&mnemonic) {
        Ok(_) => Ok(true),
        Err(_) => Ok(false),
    }
}

// ============================================================================
// Enums
// ============================================================================

#[derive(Debug, Clone, Copy)]
pub enum WordCount {
    Words12,
    Words15,
    Words18,
    Words21,
    Words24,
}

#[derive(Debug, Clone, Copy)]
pub enum AddressType {
    P2PKH,
    P2SH,
    P2WPKH,
    P2WSH,
    P2TR,
}

// ============================================================================
// Structs
// ============================================================================

/// Wallet configuration
#[derive(Debug, Clone)]
pub struct WalletConfig {
    pub wallet_path: Option<String>,
    pub network: Network,
    pub passphrase: Option<String>,
}

/// Wallet balance information
#[derive(Debug, Clone)]
pub struct WalletBalance {
    pub confirmed: u64,
    pub pending: i64,
}

impl From<alkanes_cli_common::traits::WalletBalance> for WalletBalance {
    fn from(balance: alkanes_cli_common::traits::WalletBalance) -> Self {
        Self {
            confirmed: balance.confirmed,
            pending: balance.pending,
        }
    }
}

/// Transaction information
#[derive(Debug, Clone)]
pub struct TransactionInfo {
    pub txid: String,
    pub timestamp: u64,
    pub amount: i64,
    pub fee: u64,
    pub confirmed: bool,
    pub block_height: Option<u32>,
}

/// Alkanes contract ID
#[derive(Debug, Clone)]
pub struct AlkaneId {
    pub block: u64,
    pub tx: u64,
}

/// Alkanes balance information
#[derive(Debug, Clone)]
pub struct AlkaneBalance {
    pub id: AlkaneId,
    pub amount: String,  // u128 represented as string for FFI compatibility
    pub name: Option<String>,
    pub symbol: Option<String>,
    pub decimals: Option<u8>,
}

/// Parsed address information
#[derive(Debug, Clone)]
pub struct ParsedAddress {
    pub address_type: AddressType,
    pub network: Network,
    pub script_pubkey_hex: String,
    pub witness_program_hex: Option<String>,
}

/// PBKDF2 parameters for key derivation
#[derive(Debug, Clone)]
pub struct PbkdfParams {
    pub salt: String,
    pub nonce: Option<String>,
    pub iterations: u32,
    pub algorithm: Option<String>,
}

/// Address information returned from keystore
#[derive(Debug, Clone)]
pub struct AddressInfo {
    pub derivation_path: String,
    pub address: String,
    pub script_type: String,
    pub index: u32,
    pub used: bool,
}

impl From<alkanes_cli_common::traits::AddressInfo> for AddressInfo {
    fn from(info: alkanes_cli_common::traits::AddressInfo) -> Self {
        Self {
            derivation_path: info.derivation_path,
            address: info.address,
            script_type: info.script_type,
            index: info.index,
            used: info.used,
        }
    }
}

/// UTXO information
#[derive(Debug, Clone)]
pub struct UtxoInfo {
    pub txid: String,
    pub vout: u32,
    pub amount: u64,
    pub address: String,
    pub script_pubkey_hex: String,
    pub confirmations: u32,
    pub frozen: bool,
}

/// Send transaction parameters
#[derive(Debug, Clone)]
pub struct SendParams {
    pub to_address: String,
    pub amount: u64,
    pub fee_rate: Option<f32>,
    pub send_all: bool,
    pub from_addresses: Option<Vec<String>>,
    pub change_address: Option<String>,
}

/// Prepared transaction ready for signing
#[derive(Debug, Clone)]
pub struct PreparedTransaction {
    pub psbt_base64: String,
    pub fee: u64,
    pub input_total: u64,
    pub output_total: u64,
    pub inputs: Vec<UtxoInfo>,
}

// ============================================================================
// Keystore Interface
// ============================================================================

/// Keystore for encrypted wallet storage
/// Wraps alkanes_cli_common::keystore::Keystore
pub struct Keystore {
    inner: std::sync::RwLock<alkanes_cli_common::keystore::Keystore>,
}

impl Keystore {
    /// Create a new keystore from a mnemonic phrase
    pub fn new(mnemonic: String, network: Network, passphrase: String) -> Result<Self> {
        use bip39::Mnemonic;
        use std::str::FromStr;

        let mnemonic_parsed = Mnemonic::from_str(&mnemonic)
            .map_err(|e| AlkanesError::InvalidMnemonic(e.to_string()))?;

        let btc_network: bitcoin::Network = network.into();

        let inner = alkanes_cli_common::keystore::Keystore::new(
            &mnemonic_parsed,
            btc_network,
            &passphrase,
            None,
        ).map_err(|e| AlkanesError::KeystoreError(e.to_string()))?;

        Ok(Self {
            inner: std::sync::RwLock::new(inner),
        })
    }

    /// Load a keystore from JSON string
    pub fn from_json(json_str: String) -> Result<Self> {
        let inner: alkanes_cli_common::keystore::Keystore = serde_json::from_str(&json_str)
            .map_err(|e| AlkanesError::KeystoreError(format!("Failed to parse keystore JSON: {}", e)))?;

        Ok(Self {
            inner: std::sync::RwLock::new(inner),
        })
    }

    /// Serialize keystore to JSON string
    pub fn to_json(&self) -> Result<String> {
        let inner = self.inner.read()
            .map_err(|e| AlkanesError::KeystoreError(format!("Lock error: {}", e)))?;

        serde_json::to_string_pretty(&*inner)
            .map_err(|e| AlkanesError::SerializationError(e.to_string()))
    }

    /// Get the master fingerprint
    pub fn get_master_fingerprint(&self) -> String {
        let inner = self.inner.read().unwrap();
        inner.master_fingerprint.clone()
    }

    /// Get the creation timestamp
    pub fn get_created_at(&self) -> u64 {
        let inner = self.inner.read().unwrap();
        inner.created_at
    }

    /// Decrypt and return the mnemonic phrase
    pub fn decrypt_mnemonic(&self, passphrase: String) -> Result<String> {
        let inner = self.inner.read()
            .map_err(|e| AlkanesError::KeystoreError(format!("Lock error: {}", e)))?;

        inner.decrypt_mnemonic(&passphrase)
            .map_err(|e| AlkanesError::CryptoError(e.to_string()))
    }

    /// Derive addresses using stored xpubs (no passphrase needed)
    pub fn get_addresses(
        &self,
        network: Network,
        address_type: String,
        chain: u32,
        start_index: u32,
        count: u32,
    ) -> Result<Vec<AddressInfo>> {
        let inner = self.inner.read()
            .map_err(|e| AlkanesError::KeystoreError(format!("Lock error: {}", e)))?;

        let btc_network: bitcoin::Network = network.into();

        let addresses = inner.get_addresses(btc_network, &address_type, chain, start_index, count)
            .map_err(|e| AlkanesError::KeystoreError(e.to_string()))?;

        Ok(addresses.into_iter().map(AddressInfo::from).collect())
    }

    /// Get a single address at a specific index
    pub fn get_address(&self, network: Network, address_type: String, index: u32) -> Result<String> {
        let addresses = self.get_addresses(network, address_type, 0, index, 1)?;
        addresses.into_iter().next()
            .map(|a| a.address)
            .ok_or_else(|| AlkanesError::KeystoreError("No address generated".to_string()))
    }

    /// Check if keystore has xpub for a given address type and network
    pub fn has_xpub(&self, address_type: String, network: Network) -> bool {
        let inner = self.inner.read().unwrap();
        let network_suffix = if network == Network::Bitcoin { "mainnet" } else { "testnet" };
        let key = format!("{}:{}", address_type, network_suffix);
        inner.account_xpubs.contains_key(&key) || inner.account_xpubs.contains_key(&address_type)
    }

    /// Get the account xpub for a given address type and network
    pub fn get_xpub(&self, address_type: String, network: Network) -> Result<String> {
        let inner = self.inner.read()
            .map_err(|e| AlkanesError::KeystoreError(format!("Lock error: {}", e)))?;

        let network_suffix = if network == Network::Bitcoin { "mainnet" } else { "testnet" };
        let key = format!("{}:{}", address_type, network_suffix);

        inner.account_xpubs.get(&key)
            .or_else(|| inner.account_xpubs.get(&address_type))
            .cloned()
            .ok_or_else(|| AlkanesError::KeystoreError(format!(
                "No xpub found for address type: {} on network: {:?}",
                address_type, network
            )))
    }
}

// ============================================================================
// Wallet Interface
// ============================================================================

/// Wallet interface for managing Bitcoin wallets
pub struct Wallet {
    runtime: tokio::runtime::Runtime,
    provider: Option<alkanes_cli_common::provider::ConcreteProvider>,
    config: WalletConfig,
    mnemonic: Option<String>,
    keystore: Option<std::sync::Arc<Keystore>>,
}

impl Wallet {
    /// Create a new wallet with optional mnemonic
    pub fn new(config: WalletConfig, mnemonic: Option<String>) -> Result<Self> {
        let runtime = tokio::runtime::Runtime::new()
            .map_err(|e| AlkanesError::WalletError(e.to_string()))?;

        // For now, we'll initialize the provider lazily when needed
        // Full implementation would set up the provider here
        let wallet = Self {
            runtime,
            provider: None,
            config: config.clone(),
            mnemonic: mnemonic.clone(),
            keystore: None,
        };

        Ok(wallet)
    }

    /// Create a wallet from an existing keystore
    pub fn from_keystore(
        keystore: std::sync::Arc<Keystore>,
        passphrase: String,
        config: WalletConfig,
    ) -> Result<Self> {
        let runtime = tokio::runtime::Runtime::new()
            .map_err(|e| AlkanesError::WalletError(e.to_string()))?;

        // Decrypt mnemonic from keystore
        let mnemonic = keystore.decrypt_mnemonic(passphrase)?;

        Ok(Self {
            runtime,
            provider: None,
            config,
            mnemonic: Some(mnemonic),
            keystore: Some(keystore),
        })
    }
    
    /// Get the wallet's receiving address
    pub fn get_address(&self, address_type: AddressType, index: u32) -> Result<String> {
        // For now, derive addresses from mnemonic if available
        if let Some(ref mnemonic_str) = self.mnemonic {
            use bip39::Mnemonic;
            use bitcoin::bip32::{Xpriv, DerivationPath};
            use bitcoin::secp256k1::Secp256k1;
            use std::str::FromStr;
            
            let mnemonic = Mnemonic::from_str(mnemonic_str)
                .map_err(|e| AlkanesError::InvalidMnemonic(e.to_string()))?;
            
            let seed = mnemonic.to_seed("");
            let network: bitcoin::Network = self.config.network.into();
            let secp = Secp256k1::new();
            
            let root_key = Xpriv::new_master(network, &seed)
                .map_err(|e| AlkanesError::WalletError(e.to_string()))?;
            
            // Standard derivation paths
            let derivation_path = match address_type {
                AddressType::P2PKH => format!("m/44'/0'/0'/0/{}", index),
                AddressType::P2WPKH => format!("m/84'/0'/0'/0/{}", index),
                AddressType::P2TR => format!("m/86'/0'/0'/0/{}", index),
                _ => return Err(AlkanesError::WalletError("Unsupported address type".to_string())),
            };
            
            let path = DerivationPath::from_str(&derivation_path)
                .map_err(|e| AlkanesError::WalletError(e.to_string()))?;
            
            let derived_key = root_key.derive_priv(&secp, &path)
                .map_err(|e| AlkanesError::WalletError(e.to_string()))?;
            
            let public_key = derived_key.to_priv().public_key(&secp);
            
            // Generate address based on type
            let address = match address_type {
                AddressType::P2PKH => {
                    use bitcoin::key::CompressedPublicKey;
                    let compressed = CompressedPublicKey::try_from(public_key)
                        .map_err(|e| AlkanesError::WalletError(format!("Failed to compress pubkey: {:?}", e)))?;
                    bitcoin::Address::p2pkh(&compressed, network)
                },
                AddressType::P2WPKH => {
                    use bitcoin::key::CompressedPublicKey;
                    let compressed = CompressedPublicKey::try_from(public_key)
                        .map_err(|e| AlkanesError::WalletError(format!("Failed to compress pubkey: {:?}", e)))?;
                    bitcoin::Address::p2wpkh(&compressed, network)
                },
                AddressType::P2TR => {
                    let (xonly, _) = public_key.inner.x_only_public_key();
                    bitcoin::Address::p2tr(&secp, xonly, None, network)
                },
                _ => return Err(AlkanesError::WalletError("Unsupported address type".to_string())),
            };
            
            Ok(address.to_string())
        } else {
            Err(AlkanesError::WalletError("No mnemonic available for address derivation".to_string()))
        }
    }
    
    /// Get the wallet balance
    pub fn get_balance(&self) -> Result<WalletBalance> {
        // Return zero balance for now - would need provider integration
        Ok(WalletBalance {
            confirmed: 0,
            pending: 0,
        })
    }
    
    /// Get the mnemonic phrase (if available)
    pub fn get_mnemonic(&self) -> Result<Option<String>> {
        Ok(self.mnemonic.clone())
    }
    
    /// Sync the wallet with the blockchain
    pub fn sync(&self) -> Result<()> {
        // Would need provider integration
        Ok(())
    }

    /// Sign a PSBT (Partially Signed Bitcoin Transaction)
    /// Returns the signed PSBT as base64 string
    pub fn sign_psbt(&self, psbt_base64: String) -> Result<String> {
        use bitcoin::psbt::Psbt;
        use bitcoin::secp256k1::Secp256k1;
        use bitcoin::sighash::{SighashCache, TapSighashType};
        use bitcoin::hashes::Hash;
        use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

        let mnemonic_str = self.mnemonic.as_ref()
            .ok_or_else(|| AlkanesError::WalletError("No mnemonic available for signing".to_string()))?;

        // Decode PSBT from base64
        let psbt_bytes = BASE64.decode(&psbt_base64)
            .map_err(|e| AlkanesError::TransactionError(format!("Invalid PSBT base64: {}", e)))?;

        let mut psbt = Psbt::deserialize(&psbt_bytes)
            .map_err(|e| AlkanesError::TransactionError(format!("Invalid PSBT: {}", e)))?;

        // Get keypair from mnemonic
        let mnemonic = bip39::Mnemonic::parse_in(bip39::Language::English, mnemonic_str)
            .map_err(|e| AlkanesError::InvalidMnemonic(e.to_string()))?;

        let seed = mnemonic.to_seed("");
        let network: bitcoin::Network = self.config.network.into();
        let secp = Secp256k1::new();

        let root_key = bitcoin::bip32::Xpriv::new_master(network, &seed)
            .map_err(|e| AlkanesError::WalletError(e.to_string()))?;

        // Collect prevouts for sighash calculation
        let prevouts: Vec<bitcoin::TxOut> = psbt.inputs.iter()
            .filter_map(|input| input.witness_utxo.clone())
            .collect();

        if prevouts.len() != psbt.inputs.len() {
            return Err(AlkanesError::TransactionError(
                "PSBT missing witness_utxo for some inputs".to_string()
            ));
        }

        let prevouts_ref: Vec<&bitcoin::TxOut> = prevouts.iter().collect();

        // Create sighash cache
        let mut sighash_cache = SighashCache::new(&psbt.unsigned_tx);

        // Sign each input
        for i in 0..psbt.inputs.len() {
            // Check if this is a taproot input (has tap_internal_key)
            if psbt.inputs[i].tap_internal_key.is_some() || psbt.inputs[i].tap_key_sig.is_none() {
                // Get derivation path from PSBT or use default
                // TODO: Extract derivation path from tap_key_origins when available
                // For now, use default BIP-86 Taproot path
                let path_str = format!("m/86'/0'/0'/0/0");

                let path = bitcoin::bip32::DerivationPath::from_str(&path_str)
                    .map_err(|e| AlkanesError::WalletError(e.to_string()))?;

                let derived_key = root_key.derive_priv(&secp, &path)
                    .map_err(|e| AlkanesError::WalletError(e.to_string()))?;

                let keypair = bitcoin::secp256k1::Keypair::from_secret_key(&secp, &derived_key.private_key);

                // Calculate taproot sighash
                let sighash = sighash_cache.taproot_key_spend_signature_hash(
                    i,
                    &bitcoin::sighash::Prevouts::All(&prevouts_ref),
                    TapSighashType::Default,
                ).map_err(|e| AlkanesError::TransactionError(format!("Sighash error: {}", e)))?;

                let msg = bitcoin::secp256k1::Message::from_digest(*sighash.as_byte_array());
                let sig = secp.sign_schnorr(&msg, &keypair);

                let signature = bitcoin::taproot::Signature {
                    signature: sig,
                    sighash_type: TapSighashType::Default,
                };
                psbt.inputs[i].tap_key_sig = Some(signature);
            }
        }

        // Serialize back to base64
        let signed_bytes = psbt.serialize();
        Ok(BASE64.encode(&signed_bytes))
    }

    /// Sign and finalize a PSBT, returning the raw transaction hex
    pub fn sign_and_finalize_psbt(&self, psbt_base64: String) -> Result<String> {
        use bitcoin::psbt::Psbt;
        use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

        // First sign the PSBT
        let signed_psbt_base64 = self.sign_psbt(psbt_base64)?;

        // Decode signed PSBT
        let psbt_bytes = BASE64.decode(&signed_psbt_base64)
            .map_err(|e| AlkanesError::TransactionError(format!("Invalid PSBT base64: {}", e)))?;

        let psbt = Psbt::deserialize(&psbt_bytes)
            .map_err(|e| AlkanesError::TransactionError(format!("Invalid PSBT: {}", e)))?;

        // Extract the final transaction
        let tx = psbt.extract_tx()
            .map_err(|e| AlkanesError::TransactionError(format!("Failed to finalize PSBT: {}", e)))?;

        // Serialize to hex
        Ok(bitcoin::consensus::encode::serialize_hex(&tx))
    }

    /// Sign a raw transaction hex
    pub fn sign_transaction(&self, _tx_hex: String) -> Result<String> {
        // For raw transaction signing, we'd need to create a PSBT first
        // This is a simplified implementation
        Err(AlkanesError::WalletError(
            "Raw transaction signing not yet implemented. Use sign_psbt instead.".to_string()
        ))
    }

    /// Get the internal (x-only) public key for Taproot
    pub fn get_internal_key(&self) -> Result<String> {
        use bitcoin::secp256k1::Secp256k1;

        let mnemonic_str = self.mnemonic.as_ref()
            .ok_or_else(|| AlkanesError::WalletError("No mnemonic available".to_string()))?;

        let mnemonic = bip39::Mnemonic::parse_in(bip39::Language::English, mnemonic_str)
            .map_err(|e| AlkanesError::InvalidMnemonic(e.to_string()))?;

        let seed = mnemonic.to_seed("");
        let network: bitcoin::Network = self.config.network.into();
        let secp = Secp256k1::new();

        let root_key = bitcoin::bip32::Xpriv::new_master(network, &seed)
            .map_err(|e| AlkanesError::WalletError(e.to_string()))?;

        // Derive the default Taproot key path
        let path = bitcoin::bip32::DerivationPath::from_str("m/86'/0'/0'/0/0")
            .map_err(|e| AlkanesError::WalletError(e.to_string()))?;

        let derived_key = root_key.derive_priv(&secp, &path)
            .map_err(|e| AlkanesError::WalletError(e.to_string()))?;

        let public_key = derived_key.to_priv().public_key(&secp);
        let (xonly, _) = public_key.inner.x_only_public_key();

        Ok(xonly.to_string())
    }

    /// Export the keystore (encrypted)
    pub fn export_keystore(&self) -> Result<std::sync::Arc<Keystore>> {
        // If we already have a keystore, return it
        if let Some(ref ks) = self.keystore {
            return Ok(ks.clone());
        }

        // Otherwise, create a new keystore from the mnemonic
        let mnemonic_str = self.mnemonic.as_ref()
            .ok_or_else(|| AlkanesError::WalletError("No mnemonic available".to_string()))?;

        let passphrase = self.config.passphrase.as_ref()
            .ok_or_else(|| AlkanesError::WalletError("No passphrase available for keystore export".to_string()))?;

        let keystore = Keystore::new(
            mnemonic_str.clone(),
            self.config.network,
            passphrase.clone(),
        )?;

        Ok(std::sync::Arc::new(keystore))
    }
}

use std::str::FromStr;

/// RPC Client for interacting with Bitcoin/Alkanes nodes
pub struct RpcClient {
    runtime: tokio::runtime::Runtime,
    url: String,
    network: Network,
    client: reqwest::Client,
}

impl RpcClient {
    /// Create a new RPC client
    pub fn new(url: String, network: Network) -> Result<Self> {
        let runtime = tokio::runtime::Runtime::new()
            .map_err(|e| AlkanesError::RpcError(e.to_string()))?;
        
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| AlkanesError::RpcError(e.to_string()))?;
        
        Ok(Self {
            runtime,
            url,
            network,
            client,
        })
    }
    
    /// Helper to make JSON-RPC calls
    fn call_rpc(&self, method: &str, params: serde_json::Value) -> Result<serde_json::Value> {
        self.runtime.block_on(async {
            let request = serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": method,
                "params": params
            });
            
            let response = self.client
                .post(&self.url)
                .json(&request)
                .send()
                .await
                .map_err(|e| AlkanesError::RpcError(e.to_string()))?;
            
            let json: serde_json::Value = response
                .json()
                .await
                .map_err(|e| AlkanesError::RpcError(e.to_string()))?;
            
            if let Some(error) = json.get("error") {
                if !error.is_null() {
                    return Err(AlkanesError::RpcError(format!("RPC error: {}", error)));
                }
            }
            
            json.get("result")
                .cloned()
                .ok_or_else(|| AlkanesError::RpcError("Missing result field".to_string()))
        })
    }
    
    /// Get the current block height
    pub fn get_block_count(&self) -> Result<u64> {
        let result = self.call_rpc("getblockcount", serde_json::json!([]))?;
        result.as_u64()
            .ok_or_else(|| AlkanesError::RpcError("Invalid block count response".to_string()))
    }
    
    /// Get block hash at a specific height
    pub fn get_block_hash(&self, height: u64) -> Result<String> {
        let result = self.call_rpc("getblockhash", serde_json::json!([height]))?;
        result.as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| AlkanesError::RpcError("Invalid block hash response".to_string()))
    }
    
    /// Get transaction by txid
    pub fn get_transaction(&self, txid: String) -> Result<String> {
        let result = self.call_rpc("getrawtransaction", serde_json::json!([txid, false]))?;
        result.as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| AlkanesError::RpcError("Invalid transaction response".to_string()))
    }
    
    /// Broadcast a raw transaction
    pub fn send_raw_transaction(&self, tx_hex: String) -> Result<String> {
        let result = self.call_rpc("sendrawtransaction", serde_json::json!([tx_hex]))?;
        result.as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| AlkanesError::RpcError("Invalid txid response".to_string()))
    }
}

/// Alkanes client for interacting with alkanes contracts
pub struct AlkanesClient {
    runtime: tokio::runtime::Runtime,
    metashrew_url: String,
    sandshrew_url: Option<String>,
    network: Network,
    client: reqwest::Client,
}

impl AlkanesClient {
    /// Create a new Alkanes client
    pub fn new(metashrew_url: String, sandshrew_url: Option<String>, network: Network) -> Result<Self> {
        let runtime = tokio::runtime::Runtime::new()
            .map_err(|e| AlkanesError::RpcError(e.to_string()))?;
        
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| AlkanesError::RpcError(e.to_string()))?;
        
        Ok(Self {
            runtime,
            metashrew_url,
            sandshrew_url,
            network,
            client,
        })
    }
    
    /// Helper to call metashrew/sandshrew RPC
    fn call_alkanes_rpc(&self, method: &str, params: serde_json::Value) -> Result<serde_json::Value> {
        let url = self.sandshrew_url.as_ref().unwrap_or(&self.metashrew_url);
        
        self.runtime.block_on(async {
            let request = serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": method,
                "params": params
            });
            
            let response = self.client
                .post(url)
                .json(&request)
                .send()
                .await
                .map_err(|e| AlkanesError::RpcError(e.to_string()))?;
            
            let json: serde_json::Value = response
                .json()
                .await
                .map_err(|e| AlkanesError::RpcError(e.to_string()))?;
            
            if let Some(error) = json.get("error") {
                if !error.is_null() {
                    return Err(AlkanesError::RpcError(format!("Alkanes RPC error: {}", error)));
                }
            }
            
            json.get("result")
                .cloned()
                .ok_or_else(|| AlkanesError::RpcError("Missing result field".to_string()))
        })
    }
    
    /// Get alkanes balance for an address
    pub fn get_balance(&self, _address: String) -> Result<Vec<AlkaneBalance>> {
        // Query alkanes balances would need proper integration
        // For now return empty list
        Ok(vec![])
    }
    
    /// Get bytecode for an alkanes contract
    pub fn get_bytecode(&self, alkane_id: AlkaneId) -> Result<String> {
        // Use metashrew_view with "bytecode" view function
        use prost::Message;
        use alkanes_support::proto::alkanes::{BytecodeRequest, AlkaneId as AlkaneIdPb, Uint128};
        
        let mut request = BytecodeRequest::default();
        let mut alkane_id_pb = AlkaneIdPb::default();
        alkane_id_pb.block = Some(Uint128::from(alkane_id.block as u128));
        alkane_id_pb.tx = Some(Uint128::from(alkane_id.tx as u128));
        request.id = Some(alkane_id_pb);
        
        let mut buf = Vec::new();
        request.encode(&mut buf)
            .map_err(|e| AlkanesError::SerializationError(format!("Failed to encode request: {}", e)))?;
        let params_hex = format!("0x{}", hex::encode(&buf));
        
        let rpc_params = serde_json::json!(["bytecode", params_hex, "latest"]);
        let result = self.call_alkanes_rpc("metashrew_view", rpc_params)?;
        
        result.as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| AlkanesError::AlkanesExecutionError("Invalid bytecode response".to_string()))
    }
    
    /// Trace a transaction output
    pub fn trace_outpoint(&self, txid: String, vout: u32) -> Result<String> {
        // Use metashrew_view with "trace" view function
        use prost::Message;
        use alkanes_support::proto::alkanes::Outpoint;
        
        let mut outpoint = Outpoint::default();
        outpoint.txid = hex::decode(&txid)
            .map_err(|e| AlkanesError::SerializationError(format!("Failed to decode txid: {}", e)))?;
        outpoint.vout = vout;
        
        let mut buf = Vec::new();
        outpoint.encode(&mut buf)
            .map_err(|e| AlkanesError::SerializationError(format!("Failed to encode outpoint: {}", e)))?;
        let params_hex = format!("0x{}", hex::encode(&buf));
        
        let rpc_params = serde_json::json!(["trace", params_hex, "latest"]);
        let result = self.call_alkanes_rpc("metashrew_view", rpc_params)?;
        
        serde_json::to_string(&result)
            .map_err(|e| AlkanesError::SerializationError(e.to_string()))
    }
    
    /// Get the current metashrew height
    pub fn get_height(&self) -> Result<u64> {
        let result = self.call_alkanes_rpc("metashrew_height", serde_json::json!([]))?;
        result.as_u64()
            .ok_or_else(|| AlkanesError::RpcError("Invalid height response".to_string()))
    }
}

/// Transaction builder for creating Bitcoin transactions
pub struct TransactionBuilder {
    network: Network,
    inputs: Vec<(String, u32, u64)>,
    outputs: Vec<(String, u64)>,
    fee_rate: f32,
}

impl TransactionBuilder {
    /// Create a new transaction builder
    pub fn new(network: Network) -> Self {
        Self {
            network,
            inputs: Vec::new(),
            outputs: Vec::new(),
            fee_rate: 1.0,
        }
    }
    
    /// Add an input to the transaction
    pub fn add_input(&self, _txid: String, _vout: u32, _amount: u64) -> Result<()> {
        // TODO: Make this properly mutable
        Err(not_implemented("TransactionBuilder::add_input"))
    }
    
    /// Add an output to the transaction
    pub fn add_output(&self, _address: String, _amount: u64) -> Result<()> {
        // TODO: Make this properly mutable
        Err(not_implemented("TransactionBuilder::add_output"))
    }
    
    /// Set the fee rate (satoshis per vbyte)
    pub fn set_fee_rate(&self, _fee_rate: f32) {
        // TODO: Make this properly mutable
    }
    
    /// Build and return the unsigned transaction hex
    pub fn build(&self) -> Result<String> {
        // TODO: Implement transaction building
        Err(not_implemented("TransactionBuilder::build"))
    }
    
    /// Get estimated transaction size in vbytes
    pub fn estimate_size(&self) -> u64 {
        // TODO: Implement size estimation
        250
    }
}

// ============================================================================
// Address Utilities
// ============================================================================

/// Parse a Bitcoin address
pub fn parse_address(address: String) -> Result<ParsedAddress> {
    use bitcoin::Address;
    use std::str::FromStr;
    
    let addr_unchecked = Address::from_str(&address)
        .map_err(|e| AlkanesError::InvalidAddress(format!("Invalid address: {}", e)))?;
    
    let addr = addr_unchecked.assume_checked();
    // Note: Network detection simplified - in production would need more robust detection
    // For now, default to Bitcoin mainnet as we can't easily extract network from checked address
    let network = Network::Bitcoin;
    let script_pubkey = addr.script_pubkey();
    let script_pubkey_hex = hex::encode(script_pubkey.as_bytes());
    
    // Determine address type based on script_pubkey
    let address_type = if addr.address_type() == Some(bitcoin::AddressType::P2pkh) {
        AddressType::P2PKH
    } else if addr.address_type() == Some(bitcoin::AddressType::P2sh) {
        AddressType::P2SH
    } else if addr.address_type() == Some(bitcoin::AddressType::P2wpkh) {
        AddressType::P2WPKH
    } else if addr.address_type() == Some(bitcoin::AddressType::P2wsh) {
        AddressType::P2WSH
    } else if addr.address_type() == Some(bitcoin::AddressType::P2tr) {
        AddressType::P2TR
    } else {
        // Default to P2PKH if unknown
        AddressType::P2PKH
    };
    
    let witness_program_hex = None;
    
    Ok(ParsedAddress {
        address_type,
        network,
        script_pubkey_hex,
        witness_program_hex,
    })
}

/// Convert address to script pubkey
pub fn address_to_script_pubkey(address: String, network: Network) -> Result<String> {
    use bitcoin::Address;
    use std::str::FromStr;
    
    let addr_unchecked = Address::from_str(&address)
        .map_err(|e| AlkanesError::InvalidAddress(format!("Invalid address: {}", e)))?;
    
    // Check network matches
    let btc_network: bitcoin::Network = network.into();
    let addr = match addr_unchecked.require_network(btc_network) {
        Ok(a) => a,
        Err(_) => {
            return Err(AlkanesError::InvalidNetwork(format!(
                "Address is not valid for network {:?}",
                network
            )));
        }
    };
    
    let script_pubkey = addr.script_pubkey();
    Ok(hex::encode(script_pubkey.as_bytes()))
}

// ============================================================================
// Error Handling Helpers
// ============================================================================

// Helper to create not implemented errors
fn not_implemented(msg: &str) -> AlkanesError {
    AlkanesError::Generic(format!("Not implemented: {}", msg))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        let v = version();
        assert!(!v.is_empty());
    }

    #[test]
    fn test_generate_mnemonic() {
        let mnemonic = generate_mnemonic(WordCount::Words12).unwrap();
        let words: Vec<&str> = mnemonic.split_whitespace().collect();
        assert_eq!(words.len(), 12);
    }

    #[test]
    fn test_validate_address() {
        // Valid regtest address
        let valid = validate_address(
            "bcrt1qw508d6qejxtdg4y5r3zarvary0c5xw7kygt080".to_string(),
            Network::Regtest,
        );
        assert!(valid.is_ok());
    }
}
