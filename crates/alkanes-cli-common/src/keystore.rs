//! Keystore data structures for deezel
//!
//! This module defines the structures used for storing and managing
//! wallet keystores, including encrypted seeds and public metadata.

use serde::{Deserialize, Serialize};
use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

/// Custom deserializer for created_at that handles both u64 and ISO date strings
mod created_at_deserializer {
    use serde::{self, Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(value: &u64, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u64(*value)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<u64, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum CreatedAt {
            Timestamp(u64),
            IsoString(alloc::string::String),
        }

        match CreatedAt::deserialize(deserializer)? {
            CreatedAt::Timestamp(ts) => Ok(ts),
            CreatedAt::IsoString(s) => {
                // Try to parse ISO 8601 date string
                // Format: "2025-12-22T19:28:47.126Z"
                // We'll parse it manually since chrono may not be available
                parse_iso_date(&s).map_err(serde::de::Error::custom)
            }
        }
    }

    fn parse_iso_date(s: &str) -> Result<u64, &'static str> {
        // Simple ISO 8601 parser for format: YYYY-MM-DDTHH:MM:SS.sssZ
        // Returns Unix timestamp in seconds
        let s = s.trim();

        // Remove trailing Z if present
        let s = s.strip_suffix('Z').unwrap_or(s);

        // Split by T to get date and time parts
        let parts: alloc::vec::Vec<&str> = s.split('T').collect();
        if parts.len() != 2 {
            return Err("Invalid ISO date format: missing T separator");
        }

        let date_parts: alloc::vec::Vec<&str> = parts[0].split('-').collect();
        if date_parts.len() != 3 {
            return Err("Invalid date format");
        }

        let year: i64 = date_parts[0].parse().map_err(|_| "Invalid year")?;
        let month: u32 = date_parts[1].parse().map_err(|_| "Invalid month")?;
        let day: u32 = date_parts[2].parse().map_err(|_| "Invalid day")?;

        // Handle time part (may have milliseconds)
        let time_str = parts[1].split('.').next().unwrap_or(parts[1]);
        let time_parts: alloc::vec::Vec<&str> = time_str.split(':').collect();
        if time_parts.len() != 3 {
            return Err("Invalid time format");
        }

        let hour: u32 = time_parts[0].parse().map_err(|_| "Invalid hour")?;
        let minute: u32 = time_parts[1].parse().map_err(|_| "Invalid minute")?;
        let second: u32 = time_parts[2].parse().map_err(|_| "Invalid second")?;

        // Calculate Unix timestamp
        // Days from Unix epoch (1970-01-01) to the given date
        let days = days_since_epoch(year, month, day);
        let timestamp = (days as u64) * 86400 + (hour as u64) * 3600 + (minute as u64) * 60 + (second as u64);

        Ok(timestamp)
    }

    fn days_since_epoch(year: i64, month: u32, day: u32) -> i64 {
        // Calculate days since 1970-01-01
        let mut y = year;
        let m = month as i64;

        // Adjust for months before March
        let a = (14 - m) / 12;
        y -= a;
        let m = m + 12 * a - 3;

        // Julian day number calculation
        let jdn = day as i64 + (153 * m + 2) / 5 + 365 * y + y / 4 - y / 100 + y / 400 - 32045;

        // Unix epoch is Julian day 2440588
        jdn - 2440588
    }
}

/// Represents the entire JSON keystore.
/// This structure is designed to be stored in a file, with the seed
/// encrypted using PGP.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Keystore {
    /// ASCII armored, encrypted mnemonic phrase.
    pub encrypted_mnemonic: String,
    /// Master fingerprint for identification.
    pub master_fingerprint: String,
    /// Creation timestamp (Unix epoch or ISO 8601 string).
    #[serde(with = "created_at_deserializer")]
    pub created_at: u64,
    /// Version of the keystore format.
    pub version: String,
    /// PBKDF2 parameters for key derivation from passphrase.
    pub pbkdf2_params: PbkdfParams,
    /// Legacy field: Previously stored a single account xpub. Now deprecated in favor of account_xpubs map.
    /// Kept for backward compatibility with old keystores.
    #[serde(default)]
    pub account_xpub: String,
    /// Account-level extended public keys for each address type (e.g., "p2tr" -> xpub at m/86'/coin'/0')
    /// This allows deriving addresses for all supported BIP standards without the private key.
    #[serde(default)]
    pub account_xpubs: BTreeMap<String, String>,
    /// Derivation paths for different address types.
    #[serde(default)]
    pub hd_paths: BTreeMap<String, String>,
}

/// Parameters for the PBKDF2/S2K key derivation function.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct PbkdfParams {
    /// The salt used for PBKDF2 (hex encoded).
    pub salt: String,
    /// The nonce used for AES-GCM (hex encoded).
    #[serde(default)]
    pub nonce: Option<String>,
    /// The number of iterations for the PBKDF2 function.
    pub iterations: u32,
    /// The symmetric key algorithm used (e.g., "aes-256-gcm").
    #[serde(default)]
    pub algorithm: Option<String>,
}

use crate::{AlkanesError, Result};
use bip39::Mnemonic;
#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;

impl Keystore {
    // TODO: This is a temporary, insecure implementation. The seed is not encrypted.
    // This `new` function is now primarily for non-WASM contexts.
    // The `deezel-web` crate has its own `encrypt_mnemonic` for WASM.
    pub fn new(
        mnemonic: &Mnemonic,
        network: Network,
        passphrase: &str,
        _hd_path: Option<&str>,
    ) -> Result<Self> {
        // 1. Encrypt the mnemonic phrase
        let (encrypted_mnemonic_bytes, salt, nonce) =
            crate::crypto::encrypt_sync(mnemonic.to_string().as_bytes(), passphrase)?;

        // 2. Armor the encrypted mnemonic
        let mut armored_mnemonic = Vec::new();
        alkanes_asc::armor::writer::write(
            &encrypted_mnemonic_bytes,
            alkanes_asc::armor::reader::BlockType::EncryptedMnemonic,
            &mut armored_mnemonic,
            None,
            true,
        )?;

        // 3. Derive account xpubs for each address type and BOTH coin types (mainnet=0, testnet=1)
        let seed = mnemonic.to_seed("");
        let secp = Secp256k1::new();

        // Create separate root keys for mainnet and testnet to ensure correct xpub prefixes
        // Mainnet uses "xpub" prefix, testnet/regtest/signet use "tpub" prefix
        let mainnet_root = Xpriv::new_master(Network::Bitcoin, &seed)?;
        let testnet_root = Xpriv::new_master(Network::Testnet, &seed)?;

        // Use the appropriate root for fingerprint (matches the wallet's current network)
        let root = Xpriv::new_master(network, &seed)?;

        // Derive account-level xpubs for each BIP standard and both coin types
        // This allows the same keystore to work on any network
        let mut account_xpubs = BTreeMap::new();
        let bip_standards = [
            ("p2tr", "86"),       // BIP-86: Taproot
            ("p2wpkh", "84"),     // BIP-84: Native SegWit
            ("p2sh-p2wpkh", "49"), // BIP-49: Nested SegWit
            ("p2pkh", "44"),      // BIP-44: Legacy
        ];

        for (address_type, bip_number) in &bip_standards {
            // Mainnet (coin_type = 0) - derive from mainnet root for correct "xpub" prefix
            let mainnet_path_str = format!("m/{}'/{}'/{}", bip_number, "0", "0'");
            let mainnet_path = DerivationPath::from_str(&mainnet_path_str)?;
            let mainnet_xpriv = mainnet_root.derive_priv(&secp, &mainnet_path)?;
            let mainnet_xpub = Xpub::from_priv(&secp, &mainnet_xpriv);
            account_xpubs.insert(format!("{}:mainnet", address_type), mainnet_xpub.to_string());

            // Testnet (coin_type = 1) - derive from testnet root for correct "tpub" prefix
            // Used for testnet, signet, and regtest
            let testnet_path_str = format!("m/{}'/{}'/{}", bip_number, "1", "0'");
            let testnet_path = DerivationPath::from_str(&testnet_path_str)?;
            let testnet_xpriv = testnet_root.derive_priv(&secp, &testnet_path)?;
            let testnet_xpub = Xpub::from_priv(&secp, &testnet_xpriv);
            account_xpubs.insert(format!("{}:testnet", address_type), testnet_xpub.to_string());
        }

        // Set account_xpub for backward compatibility with old code paths
        // Always default to mainnet p2tr for maximum portability across networks
        // Modern code should use account_xpubs map to select network-specific xpubs
        let default_account_xpub = account_xpubs.get("p2tr:mainnet")
            .cloned()
            .unwrap_or_default();

        // 4. Store standard HD path templates (with placeholder for coin_type)
        let mut hd_paths = BTreeMap::new();
        hd_paths.insert("p2tr".to_string(), "m/86'/COIN'/0'/0/0".to_string());
        hd_paths.insert("p2wpkh".to_string(), "m/84'/COIN'/0'/0/0".to_string());
        hd_paths.insert("p2sh-p2wpkh".to_string(), "m/49'/COIN'/0'/0/0".to_string());
        hd_paths.insert("p2pkh".to_string(), "m/44'/COIN'/0'/0/0".to_string());

        Ok(Self {
            encrypted_mnemonic: String::from_utf8(armored_mnemonic)?,
            master_fingerprint: root.fingerprint(&secp).to_string(),
            // `created_at` should be set by the caller, as `std::time` is not always available.
            created_at: 0,
            version: env!("CARGO_PKG_VERSION").to_string(),
            pbkdf2_params: PbkdfParams {
                salt: hex::encode(salt),
                nonce: Some(hex::encode(nonce)),
                iterations: 600_000,
                algorithm: Some("aes-256-gcm".to_string()),
            },
            account_xpub: default_account_xpub,
            account_xpubs,
            hd_paths,
        })
    }

    /// Load keystore from a file path.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn from_file(path: &Path) -> Result<Self> {
        let data = std::fs::read_to_string(path)
            .map_err(|e| AlkanesError::Wallet(format!("Failed to read keystore file: {e}")))?;
        serde_json::from_str(&data)
            .map_err(|e| AlkanesError::Wallet(format!("Failed to parse keystore: {e}")))
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn save_to_file(&self, path: &Path) -> Result<()> {
        let data = serde_json::to_string_pretty(self)?;
        std::fs::write(path, data)?;
        Ok(())
    }

    /// Decodes the armored seed from the keystore.
    /// Note: This does not perform any decryption.
    /// Decrypts the mnemonic from the keystore using the provided passphrase.
    pub fn decrypt_mnemonic(&self, passphrase: &str) -> Result<String> {
        // 1. Dearmor the encrypted mnemonic
        let (_, _, encrypted_bytes) = alkanes_asc::armor::reader::decode(self.encrypted_mnemonic.as_bytes())
            .map_err(|e| AlkanesError::Crypto(format!("Failed to dearmor mnemonic: {e}")))?;

        // 2. Decode salt and nonce from hex
        let salt = hex::decode(&self.pbkdf2_params.salt)?;
        let nonce = match &self.pbkdf2_params.nonce {
            Some(n) => hex::decode(n)?,
            None => vec![], // Backwards compatibility for old keystores
        };

        // 3. Decrypt using the crypto module
        let decrypted_bytes = crate::crypto::decrypt_sync(&encrypted_bytes, passphrase, &salt, &nonce)?;

        let mnemonic_str = String::from_utf8(decrypted_bytes)
            .map_err(|e| AlkanesError::Wallet(format!("Failed to convert decrypted data to string: {e}")))?;

        // 4. Validate that it's a valid mnemonic before returning
        Mnemonic::parse_in(bip39::Language::English, &mnemonic_str)
            .map_err(|e| AlkanesError::Wallet(format!("Decrypted data is not a valid mnemonic: {e}")))?;

        Ok(mnemonic_str)
    }
    pub fn get_addresses(
        &self,
        network: Network,
        address_type: &str,
        chain: u32,
        start_index: u32,
        count: u32,
    ) -> Result<Vec<crate::traits::AddressInfo>> {
        let secp = Secp256k1::new();
        
        // Determine the network suffix for xpub lookup
        let network_suffix = if network == Network::Bitcoin { "mainnet" } else { "testnet" };
        let xpub_key = format!("{}:{}", address_type, network_suffix);
        
        // Get the account xpub for this address type and network
        // First try the new account_xpubs map with network suffix
        // Then try without suffix (for old keystores created with single network)
        // Finally fall back to legacy account_xpub field (if not empty)
        let account_xpub_str = self.account_xpubs.get(&xpub_key)
            .or_else(|| self.account_xpubs.get(address_type))
            .map(|s| s.as_str())
            .or_else(|| if !self.account_xpub.is_empty() { Some(self.account_xpub.as_str()) } else { None })
            .ok_or_else(|| AlkanesError::Wallet(format!("No xpub found for address type: {} on network: {}", address_type, network_suffix)))?;
        
        let account_xpub = Xpub::from_str(account_xpub_str)?;
        
        // Determine the coin type based on network
        let coin_type = match network {
            Network::Bitcoin => "0",
            Network::Testnet | Network::Signet | Network::Regtest => "1",
            _ => "1",
        };
        
        // Determine the BIP number from address type
        let bip_number = match address_type {
            "p2tr" => "86",
            "p2wpkh" => "84",
            "p2sh-p2wpkh" => "49",
            "p2pkh" => "44",
            _ => return Err(AlkanesError::Wallet(format!("Unsupported address type: {}", address_type))),
        };
        
        let mut addresses = Vec::new();

        for i in start_index..start_index + count {
            // Build the full derivation path for display
            let full_path_str = format!("m/{}'/{}'/{}", bip_number, coin_type, format!("0'/{}/{}", chain, i));
            
            // Derive from account xpub (which is at m/purpose'/coin_type'/account')
            // We only need to derive the non-hardened part: m/change/index
            let relative_path_str = format!("m/{}/{}", chain, i);
            let path = DerivationPath::from_str(&relative_path_str)?;
            
            // Derive from account xpub
            let derived_xpub = account_xpub.derive_pub(&secp, &path)?;
            let public_key = derived_xpub.public_key;
            
            // Create address based on type
            let address = match address_type {
                "p2tr" => {
                    let (internal_key, _) = public_key.x_only_public_key();
                    Address::p2tr(&secp, internal_key, None, network)
                }
                "p2wpkh" => {
                    use bitcoin::key::CompressedPublicKey;
                    let pk = bitcoin::PublicKey::new(public_key);
                    let compressed = CompressedPublicKey::try_from(pk)
                        .map_err(|e| AlkanesError::Wallet(format!("Failed to compress public key: {}", e)))?;
                    Address::p2wpkh(&compressed, network)
                }
                "p2sh-p2wpkh" => {
                    use bitcoin::key::CompressedPublicKey;
                    let pk = bitcoin::PublicKey::new(public_key);
                    let compressed = CompressedPublicKey::try_from(pk)
                        .map_err(|e| AlkanesError::Wallet(format!("Failed to compress public key: {}", e)))?;
                    Address::p2shwpkh(&compressed, network)
                }
                "p2pkh" => {
                    let pk = bitcoin::PublicKey::new(public_key);
                    Address::p2pkh(&pk, network)
                }
                _ => return Err(AlkanesError::Wallet(format!("Unsupported address type: {}", address_type))),
            };

            addresses.push(crate::traits::AddressInfo {
                derivation_path: full_path_str,
                address: address.to_string(),
                script_type: address_type.to_string(),
                index: i,
                used: false,
            });
        }
        Ok(addresses)
    }
}

use bitcoin::bip32::{DerivationPath, Xpub};
use bitcoin::{Network, Address};
use bitcoin::bip32::{Xpriv};
use bitcoin::secp256k1::{Secp256k1, All};
use crate::network::NetworkParams;
use core::str::FromStr;


/// Derives a Bitcoin address from a mnemonic and a derivation path.
pub fn derive_address(mnemonic_str: &str, path: &DerivationPath, network: Network) -> Result<Address> {
    let mnemonic = Mnemonic::parse_in(bip39::Language::English, mnemonic_str)
        .map_err(|e| AlkanesError::Wallet(format!("Invalid mnemonic: {e}")))?;
    let seed = mnemonic.to_seed("");
    let secp = Secp256k1::<All>::new();
    let root = Xpriv::new_master(network, &seed)
        .map_err(|e| AlkanesError::Wallet(format!("Failed to create master key: {e}")))?;
    let derived_xpriv = root.derive_priv(&secp, path)
        .map_err(|e| AlkanesError::Wallet(format!("Failed to derive private key: {e}")))?;
    let keypair = derived_xpriv.to_keypair(&secp);
    let (internal_key, _parity) = keypair.public_key().x_only_public_key();
    
    // Assuming Taproot (P2TR) addresses as that seems to be the standard in this project
    Ok(Address::p2tr(&secp, internal_key, None, network))
}

/// Derives a Bitcoin address from a master public key and a derivation path.
pub fn derive_address_from_public_key(
    master_public_key: &str,
    path: &DerivationPath,
    network_params: &NetworkParams,
    address_type: &str,
) -> Result<String> {
    use metashrew_support::address::{AddressEncoding, Payload};
    use bitcoin::bech32::Hrp;

    let secp = Secp256k1::<All>::new();
    let root = Xpub::from_str(master_public_key)
        .map_err(|e| AlkanesError::Wallet(format!("Invalid master public key: {e}")))?;

    let derived_xpub = root.derive_pub(&secp, path)
        .map_err(|e| AlkanesError::Wallet(format!("Failed to derive public key: {e}. Note: Hardened derivation from a public key is not possible.")))?;
    
    let public_key = derived_xpub.public_key;
    let pk = bitcoin::PublicKey::new(public_key);

    let payload = match address_type {
        "p2tr" => {
            let (internal_key, _) = public_key.x_only_public_key();
            Payload::p2tr(&secp, internal_key, None)
        }
        "p2wpkh" => Payload::p2wpkh(&pk)?,
        "p2sh-p2wpkh" => Payload::p2shwpkh(&pk)?,
        "p2pkh" => Payload::p2pkh(&pk),
        _ => return Err(AlkanesError::InvalidParameters(format!("Unsupported address type: {}", address_type))),
    };

    let hrp = Hrp::parse(&network_params.bech32_prefix)
        .map_err(|e| AlkanesError::InvalidParameters(format!("Invalid bech32 HRP: {}", e)))?;

    let address = AddressEncoding {
        payload: &payload,
        p2pkh_prefix: network_params.p2pkh_prefix,
        p2sh_prefix: network_params.p2sh_prefix,
        hrp,
    }.to_string();

    Ok(address)
}

/// A simple wallet structure for managing mnemonics and deriving addresses.
/// This is primarily used for testing purposes.
pub struct DeezelWallet {
    mnemonic: Mnemonic,
    network: Network,
}

impl DeezelWallet {
    /// Creates a new wallet with a fresh mnemonic.
    /// The passphrase is not used for generation but is kept for API consistency.
    pub fn new(_passphrase: &str) -> Result<Self> {
        let mnemonic = Mnemonic::from_entropy(&rand::random::<[u8; 16]>())
            .map_err(|e| AlkanesError::Wallet(format!("Failed to generate mnemonic: {e}")))?;
        Ok(Self {
            mnemonic,
            network: Network::Regtest, // Default to regtest for testing
        })
    }

    /// Returns the mnemonic phrase as a string slice.
    pub fn mnemonic_phrase(&self) -> String {
        self.mnemonic.to_string()
    }

    /// Derives and returns a P2TR address for a given index.
    pub fn get_address(&self, index: u32) -> Result<Address> {
        // Using a standard P2TR derivation path for regtest (coin_type = 1)
        let path_str = format!("m/86'/1'/0'/0/{index}");
        let path = DerivationPath::from_str(&path_str)?;
        derive_address(&self.mnemonic.to_string(), &path, self.network)
    }
}
