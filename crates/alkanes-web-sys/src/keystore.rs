//! Keystore data structures for alkanes-web-sys
//
// This module defines the structures used for storing and managing
// wallet keystores, including encrypted seeds and public metadata,
// with wasm-bindgen compatibility.

extern crate alloc;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use js_sys::Promise;
use wasm_bindgen_futures::future_to_promise;
use crate::crypto::WebCrypto;
use alkanes_cli_common::{DeezelError, CryptoProvider, traits::KeystoreProvider, KeystoreAddress, KeystoreInfo};
use alkanes_cli_asc;
use alloc::{vec::Vec, string::{String, ToString}, format, collections::BTreeMap};
use async_trait::async_trait;
use bip39::{Mnemonic, Seed};
use bitcoin::{
    network::Network,
    bip32::{DerivationPath, Xpriv, Xpub},
    secp256k1::{Secp256k1, All},
};
use core::str::FromStr;
use wasm_bindgen::JsValue;

const SALT_SIZE: usize = 16;
const NONCE_SIZE: usize = 12;
const PBKDF_ITERATIONS: u32 = 600;

/// Represents the entire JSON keystore, compatible with wasm-bindgen.
#[wasm_bindgen]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Keystore {
    #[wasm_bindgen(skip)]
    pub encrypted_mnemonic: String,
    #[wasm_bindgen(skip)]
    pub master_fingerprint: String,
    #[wasm_bindgen(skip)]
    pub created_at: u64,
    #[wasm_bindgen(skip)]
    pub version: String,
    #[wasm_bindgen(skip)]
    pub pbkdf2_params: PbkdfParams,
    #[wasm_bindgen(skip)]
    pub account_xpub: String,
    #[wasm_bindgen(skip)]
    pub hd_paths: BTreeMap<String, String>,
    #[serde(skip, default)]
    #[wasm_bindgen(skip)]
    pub seed: Option<Seed>,
}

/// Parameters for the PBKDF2/S2K key derivation function.
#[wasm_bindgen]
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct PbkdfParams {
    #[wasm_bindgen(skip)]
    pub salt: String,
    #[wasm_bindgen(skip)]
    pub nonce: Option<String>,
    #[wasm_bindgen(skip)]
    pub iterations: u32,
    #[wasm_bindgen(skip)]
    pub algorithm: Option<String>,
}

#[wasm_bindgen]
impl PbkdfParams {
    #[wasm_bindgen(constructor)]
    pub fn from_js(val: JsValue) -> std::result::Result<PbkdfParams, JsValue> {
        let params: PbkdfParams = serde_wasm_bindgen::from_value(val)?;
        Ok(params)
    }

    #[wasm_bindgen]
    pub fn to_js(&self) -> std::result::Result<JsValue, JsValue> {
        Ok(serde_wasm_bindgen::to_value(self)?)
    }
}

#[wasm_bindgen]
impl Keystore {
    #[wasm_bindgen(constructor)]
    pub fn from_js(val: JsValue) -> std::result::Result<Keystore, JsValue> {
        let keystore: Keystore = serde_wasm_bindgen::from_value(val)?;
        Ok(keystore)
    }

    #[wasm_bindgen]
    pub fn to_js(&self) -> std::result::Result<JsValue, JsValue> {
        Ok(serde_wasm_bindgen::to_value(self)?)
    }

   #[wasm_bindgen(js_name = accountXpub)]
   pub fn account_xpub(&self) -> String {
       self.account_xpub.clone()
   }

   #[wasm_bindgen(js_name = hdPaths)]
   pub fn hd_paths(&self) -> JsValue {
       serde_wasm_bindgen::to_value(&self.hd_paths).unwrap()
   }

   #[wasm_bindgen(js_name = masterFingerprint)]
   pub fn master_fingerprint(&self) -> String {
       self.master_fingerprint.clone()
   }

    #[wasm_bindgen(js_name = decryptMnemonic)]
    pub fn decrypt_mnemonic(&self, passphrase: &str) -> Promise {
        let self_clone = self.clone();
        let passphrase_clone = passphrase.to_string();
        future_to_promise(async move {
            async fn decrypt_internal(keystore: Keystore, passphrase: &str) -> Result<String, AlkanesError> {
                let crypto = WebCrypto::new();
    
                let salt = hex::decode(&keystore.pbkdf2_params.salt)
                    .map_err(|e| AlkanesError::Crypto(e.to_string()))?;
                let nonce = match &keystore.pbkdf2_params.nonce {
                    Some(n) => hex::decode(n).map_err(|e| AlkanesError::Crypto(e.to_string()))?,
                    None => return Err(AlkanesError::Crypto("Nonce is missing".to_string())),
                };
    
                let key = crypto.pbkdf2_derive(passphrase.as_bytes(), &salt, PBKDF_ITERATIONS, 32).await?;
                
                let (_, _, encrypted_bytes) = alkanes_cli_asc::armor::reader::decode(keystore.encrypted_mnemonic.as_bytes())
                    .map_err(|e| AlkanesError::Armor(format!("Failed to dearmor mnemonic: {e}")))?;
    
                let decrypted_bytes = crypto.decrypt_aes_gcm(&encrypted_bytes, &key, &nonce).await?;
                
                let mnemonic_str = String::from_utf8(decrypted_bytes)
                    .map_err(|e| AlkanesError::Wallet(format!("Failed to convert decrypted data to string: {e}")))?;
    
                Ok(mnemonic_str)
            }

            match decrypt_internal(self_clone, &passphrase_clone).await {
                Ok(mnemonic) => Ok(JsValue::from_str(&mnemonic)),
                Err(e) => Err(JsValue::from_str(&e.to_string())),
            }
        })
    }
}

#[async_trait(?Send)]
impl KeystoreProvider for Keystore {
    async fn derive_addresses(&self, _master_public_key: &str, _network_params: &alkanes_cli_common::network::NetworkParams, _script_types: &[&str], _start_index: u32, _count: u32) -> Result<Vec<KeystoreAddress>, AlkanesError> {
        todo!()
    }

    async fn get_default_addresses(&self, _master_public_key: &str, _network_params: &protorune_support::network::NetworkParams) -> Result<Vec<KeystoreAddress>, AlkanesError> {
        todo!()
    }

    async fn get_address(&self, _address_type: &str, _index: u32) -> Result<String, AlkanesError> {
        todo!()
    }

    fn parse_address_range(&self, _range_spec: &str) -> Result<(String, u32, u32), AlkanesError> {
        todo!()
    }

    async fn get_keystore_info(&self, _master_fingerprint: &str, _created_at: u64, _version: &str) -> Result<KeystoreInfo, AlkanesError> {
        todo!()
    }

    async fn derive_address_from_path(&self, master_public_key: &str, path: &DerivationPath, script_type: &str, network_params: &protorune_support::network::NetworkParams) -> Result<KeystoreAddress, AlkanesError> {
        let address = alkanes_cli_common::keystore::derive_address_from_public_key(
            master_public_key,
            path,
            network_params,
            script_type,
        )?;

        let account_path = get_account_derivation_path(script_type, network_params.network)?;
        let mut full_path = account_path.to_string();
        let relative_path = path.to_string();
        if let Some(stripped) = relative_path.strip_prefix("m/") {
             full_path.push_str(&format!("/{}", stripped));
        }

        Ok(KeystoreAddress {
            address,
            derivation_path: full_path,
            index: path.into_iter().last().map_or(0, |c| u32::from(*c)),
            script_type: script_type.to_string(),
            network: Some(network_params.network.to_string()),
        })
    }
}

fn get_account_derivation_path(script_type: &str, network: Network) -> Result<DerivationPath, AlkanesError> {
    let network_path = match network {
        Network::Bitcoin => "0",
        Network::Testnet => "1",
        _ => "1", // Regtest, Signet
    };

    let path_str = match script_type {
        "p2tr" => format!("m/86'/{network_path}'/0'"),
        "p2wpkh" => format!("m/84'/{network_path}'/0'"),
        "p2sh-p2wpkh" => format!("m/49'/{network_path}'/0'"),
        "p2pkh" => format!("m/44'/{network_path}'/0'"),
        _ => return Err(AlkanesError::InvalidParameters(format!("Invalid script type: {}", script_type))),
    };

    DerivationPath::from_str(&path_str).map_err(|e| AlkanesError::Wallet(e.to_string()))
}

/// Asynchronously encrypts data using the Web Crypto API.
#[wasm_bindgen(js_name = encryptMnemonic)]
pub fn encrypt_mnemonic(mnemonic: &str, passphrase: &str) -> Promise {
    let mnemonic_clone = mnemonic.to_string();
    let passphrase_clone = passphrase.to_string();

    future_to_promise(async move {
        async fn encrypt_internal(mnemonic_str: &str, passphrase: &str) -> Result<Keystore, AlkanesError> {
            let crypto = WebCrypto::new();
            let salt = crypto.random_bytes(SALT_SIZE)?;
            let nonce = crypto.random_bytes(NONCE_SIZE)?;
    
            let key = crypto.pbkdf2_derive(passphrase.as_bytes(), &salt, PBKDF_ITERATIONS, 32).await?;
            
            let encrypted_data = crypto.encrypt_aes_gcm(mnemonic_str.as_bytes(), &key, &nonce).await?;
    
            let mut armored_mnemonic = Vec::new();
            alkanes_cli_asc::armor::writer::write(
                &encrypted_data,
                alkanes_cli_asc::armor::reader::BlockType::EncryptedMnemonic,
                &mut armored_mnemonic,
                None,
                true,
            ).map_err(|e| AlkanesError::Armor(e.to_string()))?;

            let mnemonic = Mnemonic::from_phrase(mnemonic_str, bip39::Language::English)?;
            let seed = Seed::new(&mnemonic, "");
            let secp = Secp256k1::<All>::new();
            let root = Xpriv::new_master(Network::Regtest, seed.as_bytes())?;
            let primary_path = DerivationPath::from_str("m/86'/0'/0'")?;
            let xpub = Xpub::from_priv(&secp, &root.derive_priv(&secp, &primary_path)?);

            let mut hd_paths = BTreeMap::new();
            hd_paths.insert("p2tr".to_string(), "m/86'/0'/0'".to_string());
            hd_paths.insert("p2wpkh".to_string(), "m/84'/0'/0'".to_string());
            hd_paths.insert("p2sh-p2wpkh".to_string(), "m/49'/0'/0'".to_string());
            hd_paths.insert("p2pkh".to_string(), "m/44'/0'/0'".to_string());
    
            let keystore = Keystore {
                encrypted_mnemonic: String::from_utf8(armored_mnemonic).unwrap(),
                master_fingerprint: root.fingerprint(&secp).to_string(),
                created_at: web_sys::js_sys::Date::now() as u64 / 1000,
                version: "alkanes-web-sys-0.1.0".to_string(),
                pbkdf2_params: PbkdfParams {
                    salt: hex::encode(&salt),
                    nonce: Some(hex::encode(&nonce)),
                    iterations: PBKDF_ITERATIONS,
                    algorithm: Some("aes-256-gcm".to_string()),
                },
                account_xpub: xpub.to_string(),
                hd_paths,
                seed: Some(seed),
            };
            Ok(keystore)
        }

        match encrypt_internal(&mnemonic_clone, &passphrase_clone).await {
            Ok(keystore) => Ok(serde_wasm_bindgen::to_value(&keystore).unwrap()),
            Err(e) => Err(JsValue::from_str(&e.to_string())),
        }
    })
}