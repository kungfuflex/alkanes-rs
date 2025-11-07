//! Keystore management functionality for deezel-sys
//!
//! This module provides keystore creation and management functionality
//! using PGP encryption for secure seed storage.

extern crate alloc;
use anyhow::{anyhow, Context, Result as AnyhowResult};
use bitcoin::{
    bip32::{DerivationPath, Xpub},
    secp256k1::Secp256k1,
    Address, CompressedPublicKey, Network, PublicKey, ScriptBuf,
};
use bip39::Mnemonic;
use std::str::FromStr;

use alkanes_cli_common::{
    keystore::{Keystore},
    traits::{KeystoreAddress, KeystoreInfo, KeystoreProvider},
    AlkanesError, Result as CommonResult,
};
use async_trait::async_trait;


/// Parameters for creating a new keystore
pub struct KeystoreCreateParams {
    /// Optional mnemonic (if None, a new one will be generated)
    pub mnemonic: Option<String>,
    /// Passphrase for keystore encryption.
    pub passphrase: Option<String>,
    /// Bitcoin network.
    pub network: Network,
    /// Number of addresses to derive for each script type
    pub address_count: u32,
    /// Optional HD derivation path
    pub hd_path: Option<String>,
}

/// Keystore manager that handles creation and management
#[derive(Clone)]
pub struct KeystoreManager {
    #[allow(dead_code)]
    _marker: core::marker::PhantomData<()>,
}

impl Default for KeystoreManager {
    fn default() -> Self {
        Self::new()
    }
}

impl KeystoreManager {
    pub fn new() -> Self {
        Self {
            _marker: core::marker::PhantomData,
        }
    }

    /// Create a new keystore with PGP-encrypted seed and master public key
    pub async fn create_keystore(&self, params: KeystoreCreateParams) -> AnyhowResult<(Keystore, String)> {
        // 1. Get passphrase, prompting if necessary
        let passphrase = if let Some(p) = params.passphrase {
            p
        } else {
            rpassword::prompt_password("Enter a new passphrase for the keystore: ")
                .map_err(|e| AlkanesError::Wallet(format!("{e}")))?
        };

        // 2. Generate or use provided mnemonic
        let mnemonic = if let Some(mnemonic_str) = params.mnemonic {
            Mnemonic::parse_in(bip39::Language::English, &mnemonic_str)
                .map_err(|e| AlkanesError::Wallet(format!("Invalid mnemonic: {e}")))?
        } else {
            Mnemonic::from_entropy(&rand::random::<[u8; 32]>())
                .map_err(|e| AlkanesError::Wallet(format!("Failed to generate mnemonic: {e}")))?
        };
        let mnemonic_str = mnemonic.to_string();

        // 3. Create the encrypted keystore
        let mut keystore = Keystore::new(
            &mnemonic,
            params.network,
            &passphrase,
            params.hd_path.as_deref(),
        )
        .map_err(|e| AlkanesError::Wallet(format!("{e}")))?;

        keystore.created_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| AlkanesError::Wallet(format!("{e}")))?
            .as_secs();

        Ok((keystore, mnemonic_str))
    }


    /// Load and decrypt a keystore
    pub async fn load_keystore(&self, keystore_data: &str, passphrase: &str) -> AnyhowResult<(Keystore, String)> {
        // Parse the keystore JSON
        let keystore: Keystore = serde_json::from_str(keystore_data)
            .map_err(|e| AlkanesError::Wallet(format!("{e}")))?;

        // Decrypt the mnemonic using the new crypto module
        let mnemonic = keystore.decrypt_mnemonic(passphrase)
            .map_err(|e| AlkanesError::Wallet(format!("{e}")))?;
        
        Ok((keystore, mnemonic))
    }
    

    /// Save keystore to file
    pub async fn save_keystore(&self, keystore: &Keystore, file_path: &str) -> AnyhowResult<()> {
        let keystore_json = serde_json::to_string_pretty(keystore)
            .map_err(|e| AlkanesError::Wallet(format!("{e}")))?;

        std::fs::write(file_path, keystore_json)
            .with_context(|| format!("Failed to write keystore to file: {file_path}"))?;

        Ok(())
    }

    /// Load keystore from file
    pub async fn load_keystore_from_file(&self, file_path: &str, passphrase: &str) -> AnyhowResult<(Keystore, String)> {
        let keystore_data = std::fs::read_to_string(file_path)
            .with_context(|| format!("Failed to read keystore file: {file_path}"))?;

        self.load_keystore(&keystore_data, passphrase).await
    }

    /// Load keystore metadata (master public key, fingerprint, etc.) without decryption
    pub async fn load_keystore_metadata_from_file(&self, file_path: &str) -> AnyhowResult<Keystore> {
        let keystore_data = std::fs::read_to_string(file_path)
            .with_context(|| format!("Failed to read keystore file: {file_path}"))?;

        // Parse the keystore JSON - we only need the metadata, not the encrypted seed
        let keystore: Keystore = serde_json::from_str(&keystore_data)
            .map_err(|e| AlkanesError::Wallet(format!("{e}")))?;

        Ok(keystore)
    }

    /// Derive addresses dynamically from master public key
    pub fn derive_addresses(&self, keystore: &Keystore, network: Network, script_types: &[&str], start_index: u32, count: u32) -> AnyhowResult<Vec<KeystoreAddress>> {
        let master_xpub = Xpub::from_str(&keystore.account_xpub)
            .map_err(|e| AlkanesError::Wallet(format!("{e}")))?;
        
        let secp = Secp256k1::new();
        let mut addresses = Vec::new();
        
        for script_type in script_types {
            for index in start_index..(start_index + count) {
                let address = self.derive_single_address(&master_xpub, &secp, network, script_type, 0, index)?;
                addresses.push(address);
            }
        }
        
        Ok(addresses)
    }
    
    /// Derive a single address from master public key
    /// Derive a single address from the account public key.
    /// The `master_xpub` parameter is the account-level xpub (e.g., from m/86'/1'/0').
    pub fn derive_single_address(&self, master_xpub: &Xpub, secp: &Secp256k1<bitcoin::secp256k1::All>, network: Network, script_type: &str, chain: u32, index: u32) -> AnyhowResult<KeystoreAddress> {
        // Get the correct coin type for the network for display purposes.
        let coin_type = match network {
            Network::Bitcoin => "0",
            Network::Testnet | Network::Signet | Network::Regtest => "1",
            _ => "0", // Default to mainnet for custom networks
        };

        // BIP-86 standard derivation from an account xpub uses m/0/* for receive and m/1/* for change.
        let relative_path_str = format!("m/{chain}/{index}");
        let relative_path = DerivationPath::from_str(&relative_path_str)
            .with_context(|| format!("Failed to create relative derivation path: {relative_path_str}"))?;

        // Derive the public key using the relative path from the account xpub.
        let derived_key = master_xpub.derive_pub(secp, &relative_path)
            .with_context(|| format!("Failed to derive public key for path: {relative_path}"))?;

        // The full derivation path for display depends on the script type's standard.
        // Since our account_xpub is for BIP-86, we'll show that path for p2tr.
        // For other types, we'll show a non-standard path to indicate what's happening.
        let (derivation_path, address) = match script_type {
            "p2tr" => {
                let full_path = format!("m/86'/{coin_type}'/0'/{chain}/{index}");
                let internal_key = bitcoin::key::UntweakedPublicKey::from(derived_key.public_key);
                let address = Address::p2tr(secp, internal_key, None, network);
                (full_path, address.to_string())
            }
            "p2wpkh" => {
                let full_path = format!("m/84'/{coin_type}'/0'/{chain}/{index}");
                let bitcoin_pubkey = PublicKey::new(derived_key.public_key);
                let compressed_pubkey = CompressedPublicKey::try_from(bitcoin_pubkey)
                    .map_err(|e| AlkanesError::Wallet(format!("{e}")))?;
                let address = Address::p2wpkh(&compressed_pubkey, network);
                (full_path, address.to_string())
            }
            "p2sh" => {
                let full_path = format!("m/49'/{coin_type}'/0'/{chain}/{index}");
                let bitcoin_pubkey = PublicKey::new(derived_key.public_key);
                let compressed_pubkey = CompressedPublicKey::try_from(bitcoin_pubkey)
                    .map_err(|e| AlkanesError::Wallet(format!("{e}")))?;
                let wpkh_script = ScriptBuf::new_p2wpkh(&compressed_pubkey.wpubkey_hash());
                let address = Address::p2sh(&wpkh_script, network)
                    .map_err(|e| AlkanesError::Wallet(format!("{e}")))?;
                (full_path, address.to_string())
            }
            "p2pkh" => {
                let full_path = format!("m/44'/{coin_type}'/0'/{chain}/{index}");
                let bitcoin_pubkey = PublicKey::new(derived_key.public_key);
                let compressed_pubkey = CompressedPublicKey::try_from(bitcoin_pubkey)
                    .map_err(|e| AlkanesError::Wallet(format!("{e}")))?;
                let address = Address::p2pkh(compressed_pubkey, network);
                (full_path, address.to_string())
            }
            "p2wsh" => {
                let full_path = format!("m/86'/{coin_type}'/0'/{chain}/{index} (p2wsh from p2tr account)");
                let bitcoin_pubkey = PublicKey::new(derived_key.public_key);
                let compressed_pubkey = CompressedPublicKey::try_from(bitcoin_pubkey)
                    .map_err(|e| AlkanesError::Wallet(format!("{e}")))?;
                let script = ScriptBuf::new_p2wpkh(&compressed_pubkey.wpubkey_hash());
                let address = Address::p2wsh(&script, network);
                (full_path, address.to_string())
            }
            _ => return Err(anyhow!("Unsupported script type: {}", script_type)),
        };
        
        Ok(KeystoreAddress {
            address,
            derivation_path,
            index,
            script_type: script_type.to_string(),
            network: None, // Will be set by caller if needed
        })
    }
    
    /// Get default addresses for display (first 5 of each type for given network)
    pub fn get_default_addresses(&self, keystore: &Keystore, network: Network) -> AnyhowResult<Vec<KeystoreAddress>> {
        let script_types = ["p2pkh", "p2sh", "p2wpkh", "p2wsh", "p2tr"];
        self.derive_addresses(keystore, network, &script_types, 0, 5)
    }

    /// Create a keystore info summary
    pub fn get_keystore_info(&self, keystore: &Keystore) -> KeystoreInfo {
        KeystoreInfo {
            master_fingerprint: keystore.master_fingerprint.clone(),
            created_at: keystore.created_at,
            version: keystore.version.clone(),
        }
    }
    
    /// Parse address range specification (e.g., "p2tr:0-1000", "p2sh:0-500", "p2tr:50")
    pub fn parse_address_range(&self, range_spec: &str) -> AnyhowResult<(String, u32, u32)> {
        let parts: Vec<&str> = range_spec.split(':').collect();
        if parts.len() != 2 {
            return Err(anyhow!("Invalid range specification. Expected format: script_type:start-end or script_type:index"));
        }
        
        let script_type = parts[0].to_string();
        let range_str = parts[1];

        if range_str.contains('-') {
            // Handle range format: start-end
            let range_parts: Vec<&str> = range_str.split('-').collect();
            if range_parts.len() != 2 {
                return Err(anyhow!("Invalid range format. Expected format: start-end"));
            }
            
            let start_index: u32 = range_parts[0].parse()
                .map_err(|_| anyhow!("Invalid start index: {}", range_parts[0]))?;
            let end_index: u32 = range_parts[1].parse()
                .map_err(|_| anyhow!("Invalid end index: {}", range_parts[1]))?;
                
            if end_index < start_index {
                return Err(anyhow!("End index must be greater than or equal to start index"));
            }
            
            Ok((script_type, start_index, (end_index - start_index) + 1))
        } else {
            // Handle single index format
            let index: u32 = range_str.parse()
                .map_err(|_| anyhow!("Invalid index: {}", range_str))?;
            Ok((script_type, index, 1))
        }
    }
    
    /// Derive addresses from keystore metadata without requiring decryption
    pub fn derive_addresses_from_metadata(&self, keystore_metadata: &Keystore, network: Network, script_types: &[&str], start_index: u32, count: u32, custom_network_params: Option<&alkanes_cli_common::network::NetworkParams>) -> AnyhowResult<Vec<KeystoreAddress>> {
        let master_xpub = Xpub::from_str(&keystore_metadata.account_xpub)
            .map_err(|e| AlkanesError::Wallet(format!("{e}")))?;
        
        let secp = Secp256k1::new();
        let mut addresses = Vec::new();
        
        for script_type in script_types {
            for index in start_index..(start_index + count) {
                let mut address = self.derive_single_address(&master_xpub, &secp, network, script_type, 0, index)?;
                
                // Apply custom network parameters if provided
                if let Some(network_params) = custom_network_params {
                    address = self.apply_custom_network_params(address, network_params)?;
                }
                
                addresses.push(address);
            }
        }
        
        Ok(addresses)
    }
    
    /// Get default addresses from keystore metadata without requiring decryption
    pub fn get_default_addresses_from_metadata(&self, keystore_metadata: &Keystore, network: Network, custom_network_params: Option<&alkanes_cli_common::network::NetworkParams>) -> AnyhowResult<Vec<KeystoreAddress>> {
        let script_types = ["p2pkh", "p2sh", "p2wpkh", "p2wsh", "p2tr"];
        self.derive_addresses_from_metadata(keystore_metadata, network, &script_types, 0, 5, custom_network_params)
    }
    
    /// Apply custom network parameters to an address (re-derive with custom magic bytes)
    fn apply_custom_network_params(&self, mut address: KeystoreAddress, network_params: &alkanes_cli_common::network::NetworkParams) -> AnyhowResult<KeystoreAddress> {
        // Re-derive the address using the custom network parameters
        // This is needed for networks like dogecoin that have different magic bytes
        
        // For bech32 addresses (P2WPKH, P2WSH, P2TR), we need to manually construct the address
        // with the custom HRP (Human Readable Part)
        match address.script_type.as_str() {
            "p2wpkh" | "p2wsh" | "p2tr" => {
                // For bech32 addresses, replace the HRP prefix
                if address.address.contains('1') {
                    // Find the separator and replace the HRP
                    if let Some(separator_pos) = address.address.find('1') {
                        let data_part = &address.address[separator_pos..];
                        address.address = format!("{}{}", network_params.bech32_prefix, data_part);
                    }
                }
            },
            "p2pkh" | "p2sh" => {
                // For legacy addresses, we would need to re-encode with custom version bytes
                // This is complex and requires parsing the address, so for now we'll keep the original
                // TODO: Implement proper legacy address re-encoding with custom version bytes
            },
            _ => {
                // For unknown script types, keep the original address
            }
        }
        
        Ok(address)
    }
}

/// Implementation of KeystoreProvider trait for KeystoreManager
#[async_trait(?Send)]
impl KeystoreProvider for KeystoreManager {
    async fn get_address(&self, _address_type: &str, _index: u32) -> CommonResult<String> {
        Err(AlkanesError::NotImplemented("get_address is not implemented for KeystoreManager".to_string()))
    }

    async fn derive_addresses(&self, master_public_key: &str, network_params: &alkanes_cli_common::network::NetworkParams, script_types: &[&str], start_index: u32, count: u32) -> CommonResult<Vec<KeystoreAddress>> {
        let master_xpub = Xpub::from_str(master_public_key)
            .map_err(|e| AlkanesError::Crypto(format!("Failed to parse master public key: {e}")))?;
        
        let secp = Secp256k1::new();
        let mut addresses = Vec::new();
        
        for script_type in script_types {
            for index in start_index..(start_index + count) {
                let address = self.derive_single_address(&master_xpub, &secp, network_params.network, script_type, 0, index)
                    .map_err(|e| AlkanesError::Crypto(format!("Failed to derive address: {e}")))?;
                addresses.push(address);
            }
        }
        
        Ok(addresses)
    }
    
    async fn get_default_addresses(&self, master_public_key: &str, network_params: &alkanes_cli_common::network::NetworkParams) -> CommonResult<Vec<KeystoreAddress>> {
        let script_types = ["p2pkh", "p2sh", "p2wpkh", "p2wsh", "p2tr"];
        // Call the trait method, not the struct method
        KeystoreProvider::derive_addresses(self, master_public_key, network_params, &script_types, 0, 5).await
    }
    
    fn parse_address_range(&self, range_spec: &str) -> CommonResult<(String, u32, u32)> {
        // Call the struct method directly to avoid infinite recursion
        KeystoreManager::parse_address_range(self, range_spec)
            .map_err(|e| AlkanesError::Parse(format!("Failed to parse address range: {e}")))
    }
    
    async fn get_keystore_info(&self, master_fingerprint: &str, created_at: u64, version: &str) -> CommonResult<KeystoreInfo> {
        Ok(KeystoreInfo {
            master_fingerprint: master_fingerprint.to_string(),
            created_at,
            version: version.to_string(),
        })
    }
    async fn derive_address_from_path(&self, _master_public_key: &str, _path: &DerivationPath, _script_type: &str, _network_params: &alkanes_cli_common::network::NetworkParams) -> CommonResult<KeystoreAddress> {
        unimplemented!()
    }
}

/// Create a keystore with the given parameters
pub async fn create_keystore(params: KeystoreCreateParams) -> AnyhowResult<(Keystore, String)> {
    let manager = KeystoreManager::new();
    manager.create_keystore(params).await
}

/// Load a keystore from file
pub async fn load_keystore_from_file(file_path: &str, passphrase: &str) -> AnyhowResult<(Keystore, String)> {
    let manager = KeystoreManager::new();
    manager.load_keystore_from_file(file_path, passphrase).await
}

/// Save a keystore to file
pub async fn save_keystore_to_file(keystore: &Keystore, file_path: &str) -> AnyhowResult<()> {
    let manager = KeystoreManager::new();
    manager.save_keystore(keystore, file_path).await
}