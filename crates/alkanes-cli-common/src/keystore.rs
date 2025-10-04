//! Keystore data structures for deezel
//!
//! This module defines the structures used for storing and managing
//! wallet keystores, including encrypted seeds and public metadata.

use serde::{Deserialize, Serialize};
use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

/// Represents the entire JSON keystore.
/// This structure is designed to be stored in a file, with the seed
/// encrypted using PGP.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Keystore {
    /// ASCII armored, encrypted mnemonic phrase.
    pub encrypted_mnemonic: String,
    /// Master fingerprint for identification.
    pub master_fingerprint: String,
    /// Creation timestamp (Unix epoch).
    pub created_at: u64,
    /// Version of the keystore format.
    pub version: String,
    /// PBKDF2 parameters for key derivation from passphrase.
    pub pbkdf2_params: PbkdfParams,
    /// Account-level extended public key (xpub) for deriving addresses without the private key.
    #[serde(default)]
    pub account_xpub: String,
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

use crate::{DeezelError, Result};
use bip39::{Mnemonic, MnemonicType, Seed};
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
        hd_path: Option<&str>,
    ) -> Result<Self> {
        // 1. Encrypt the mnemonic phrase
        let (encrypted_mnemonic_bytes, salt, nonce) =
            crate::crypto::encrypt_sync(mnemonic.phrase().as_bytes(), passphrase)?;

        // 2. Armor the encrypted mnemonic
        let mut armored_mnemonic = Vec::new();
        alkanes_cli_asc::armor::writer::write(
            &encrypted_mnemonic_bytes,
            alkanes_cli_asc::armor::reader::BlockType::EncryptedMnemonic,
            &mut armored_mnemonic,
            None,
            true,
        )?;

        // 3. Derive keys and fingerprint
        let seed = Seed::new(mnemonic, "");
        let secp = Secp256k1::new();
        let root = Xpriv::new_master(network, seed.as_bytes())?;

        // Use provided HD path or default to BIP-86 for the main account xpub
        let primary_path_str = hd_path.unwrap_or("m/86'/0'/0'");
        let primary_path = DerivationPath::from_str(primary_path_str)?;
        let xpub = Xpub::from_priv(&secp, &root.derive_priv(&secp, &primary_path)?);

        // 4. Populate standard HD paths
        let mut hd_paths = BTreeMap::new();
        hd_paths.insert("p2tr".to_string(), "m/86'/0'/0'".to_string());
        hd_paths.insert("p2wpkh".to_string(), "m/84'/0'/0'".to_string());
        hd_paths.insert("p2sh-p2wpkh".to_string(), "m/49'/0'/0'".to_string());
        hd_paths.insert("p2pkh".to_string(), "m/44'/0'/0'".to_string());

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
            account_xpub: xpub.to_string(),
            hd_paths,
        })
    }

    /// Load keystore from a file path.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn from_file(path: &Path) -> Result<Self> {
        let data = std::fs::read_to_string(path)
            .map_err(|e| DeezelError::Wallet(format!("Failed to read keystore file: {e}")))?;
        serde_json::from_str(&data)
            .map_err(|e| DeezelError::Wallet(format!("Failed to parse keystore: {e}")))
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
        let (_, _, encrypted_bytes) = alkanes_cli_asc::armor::reader::decode(self.encrypted_mnemonic.as_bytes())
            .map_err(|e| DeezelError::Crypto(format!("Failed to dearmor mnemonic: {e}")))?;

        // 2. Decode salt and nonce from hex
        let salt = hex::decode(&self.pbkdf2_params.salt)?;
        let nonce = match &self.pbkdf2_params.nonce {
            Some(n) => hex::decode(n)?,
            None => vec![], // Backwards compatibility for old keystores
        };

        // 3. Decrypt using the crypto module
        let decrypted_bytes = crate::crypto::decrypt_sync(&encrypted_bytes, passphrase, &salt, &nonce)?;

        let mnemonic_str = String::from_utf8(decrypted_bytes)
            .map_err(|e| DeezelError::Wallet(format!("Failed to convert decrypted data to string: {e}")))?;

        // 4. Validate that it's a valid mnemonic before returning
        Mnemonic::from_phrase(&mnemonic_str, bip39::Language::English)
            .map_err(|e| DeezelError::Wallet(format!("Decrypted data is not a valid mnemonic: {e}")))?;

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
        let xpub = Xpub::from_str(&self.account_xpub)?;
        let mut addresses = Vec::new();

        for i in start_index..start_index + count {
            let path_str = format!("m/{chain}/{i}");
            let path = DerivationPath::from_str(&path_str)?;
            let derived_xpub = xpub.derive_pub(&secp, &path)?;
            let (internal_key, _) = derived_xpub.public_key.x_only_public_key();
            let address = Address::p2tr(&secp, internal_key, None, network);
            
            // Construct the full path for display, assuming a BIP-86 structure.
            let coin_type = match network {
                Network::Bitcoin => "0",
                _ => "1",
            };
            let full_path_str = format!("m/86'/{}'/0'/{}", coin_type, path_str.strip_prefix("m/").unwrap());

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
    let mnemonic = Mnemonic::from_phrase(mnemonic_str, bip39::Language::English)
        .map_err(|e| DeezelError::Wallet(format!("Invalid mnemonic: {e}")))?;
    let seed = Seed::new(&mnemonic, "");
    let secp = Secp256k1::<All>::new();
    let root = Xpriv::new_master(network, seed.as_bytes())
        .map_err(|e| DeezelError::Wallet(format!("Failed to create master key: {e}")))?;
    let derived_xpriv = root.derive_priv(&secp, path)
        .map_err(|e| DeezelError::Wallet(format!("Failed to derive private key: {e}")))?;
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
        .map_err(|e| DeezelError::Wallet(format!("Invalid master public key: {e}")))?;

    let derived_xpub = root.derive_pub(&secp, path)
        .map_err(|e| DeezelError::Wallet(format!("Failed to derive public key: {e}. Note: Hardened derivation from a public key is not possible.")))?;
    
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
        _ => return Err(DeezelError::InvalidParameters(format!("Unsupported address type: {}", address_type))),
    };

    let hrp = Hrp::parse(&network_params.bech32_prefix)
        .map_err(|e| DeezelError::InvalidParameters(format!("Invalid bech32 HRP: {}", e)))?;

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
        let mnemonic = Mnemonic::new(MnemonicType::Words12, bip39::Language::English);
        Ok(Self {
            mnemonic,
            network: Network::Regtest, // Default to regtest for testing
        })
    }

    /// Returns the mnemonic phrase as a string slice.
    pub fn mnemonic_phrase(&self) -> &str {
        self.mnemonic.phrase()
    }

    /// Derives and returns a P2TR address for a given index.
    pub fn get_address(&self, index: u32) -> Result<Address> {
        // Using a standard P2TR derivation path for regtest (coin_type = 1)
        let path_str = format!("m/86'/1'/0'/0/{index}");
        let path = DerivationPath::from_str(&path_str)?;
        derive_address(self.mnemonic.phrase(), &path, self.network)
    }
}
