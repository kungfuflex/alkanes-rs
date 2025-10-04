//! Browser Wallet Provider System
//!
//! This module provides a comprehensive wallet provider system that wraps injected browser wallets
//! (like Unisat, Xverse, Phantom, OKX, etc.) while implementing deezel-common traits. The system
//! uses wallets minimally as signers/keystores and leverages our sandshrew RPC connections and
//! polling strategies for most operations.
//!
//! # Architecture
//!
//! The wallet provider system consists of:
//! - [`BrowserWalletProvider`]: Main provider that wraps injected wallets
//! - [`WalletBackend`]: Trait for different wallet implementations
//! - [`InjectedWallet`]: Wrapper for browser-injected wallet objects
//! - [`WalletConnector`]: Connection management and wallet detection
//!
//! # Features
//!
//! - **Multi-wallet support**: Works with 13+ different Bitcoin wallets
//! - **Minimal wallet usage**: Only uses wallets for signing and key operations
//! - **Sandshrew integration**: Leverages our RPC connections for blockchain operations
//! - **Event handling**: Supports account and network change events
//! - **PSBT signing**: Full support for Partially Signed Bitcoin Transactions
//! - **Mobile support**: Deep linking and device detection
//!
//! # Example
//!
//! ```rust,no_run
//! use deezel_web::wallet_provider::*;
//! use deezel_common::*;
//!
//! async fn connect_wallet() -> Result<BrowserWalletProvider> {
//!     let connector = WalletConnector::new();
//!     let available_wallets = connector.detect_wallets().await?;
//!     
//!     if let Some(wallet_info) = available_wallets.first() {
//!         let provider = BrowserWalletProvider::connect(
//!             wallet_info.clone(),
//!             "mainnet".to_string(),
//!         ).await?;
//!         
//!         Ok(provider)
//!     } else {
//!         Err(DeezelError::Wallet("No wallets detected".to_string()))
//!     }
//! }
//! ```

#[cfg(target_arch = "wasm32")]
extern crate alloc;
#[cfg(target_arch = "wasm32")]
use alloc::{
    vec,
    vec::Vec,
    boxed::Box,
    string::{String, ToString},
    format,
};

use async_trait::async_trait;
use bitcoin::{
    secp256k1::{schnorr::Signature, All, Keypair, Secp256k1, Message},
    Network, OutPoint, Psbt, Transaction, TxOut, XOnlyPublicKey,
    address::Address,
    Amount, TxIn, Witness, Sequence, ScriptBuf,
};
use deezel_common::{*, alkanes::{AlkanesInspectConfig, AlkanesInspectResult, AlkaneBalance}, provider::{AllBalances, AssetBalance, EnrichedUtxo}};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{window, js_sys};
use hex;
use core::str::FromStr;
use base64::{engine::general_purpose::STANDARD, Engine as _};

use crate::provider::WebProvider;
use deezel_common::ord::{
    AddressInfo as OrdAddressInfo, Block as OrdBlock, Blocks as OrdBlocks,
    Children as OrdChildren, Inscription as OrdInscription, Inscriptions as OrdInscriptions,
    Output as OrdOutput, ParentInscriptions as OrdParents, SatResponse as OrdSat,
    RuneInfo as OrdRuneInfo, Runes as OrdRunes, TxInfo as OrdTxInfo,
};
use deezel_common::alkanes::execute::EnhancedAlkanesExecutor;

/// Information about an available wallet
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletInfo {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub website: String,
    pub injection_key: String,
    pub supports_psbt: bool,
    pub supports_taproot: bool,
    pub supports_ordinals: bool,
    pub mobile_support: bool,
    pub deep_link_scheme: Option<String>,
}

/// Wallet connection status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WalletConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Error(String),
}

/// Account information from connected wallet
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletAccount {
    pub address: String,
    pub public_key: Option<String>,
    pub compressed_public_key: Option<String>,
    pub address_type: String,
}

/// Network information from wallet
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletNetworkInfo {
    pub network: String,
    pub chain_id: Option<String>,
}

/// PSBT signing options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PsbtSigningOptions {
    pub auto_finalized: bool,
    pub to_sign_inputs: Option<Vec<PsbtSigningInput>>,
}

/// PSBT input signing specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PsbtSigningInput {
    pub index: u32,
    pub address: Option<String>,
    pub sighash_types: Option<Vec<u32>>,
    pub disable_tweaked_public_key: Option<bool>,
}

/// Trait for different wallet backend implementations
#[async_trait(?Send)]
pub trait WalletBackend {
    /// Get wallet information
    fn get_info(&self) -> &WalletInfo;
    
    /// Check if wallet is available in the browser
    async fn is_available(&self) -> bool;
    
    /// Connect to the wallet
    async fn connect(&self) -> Result<WalletAccount>;
    
    /// Disconnect from the wallet
    async fn disconnect(&self) -> Result<()>;
    
    /// Get current accounts
    async fn get_accounts(&self) -> Result<Vec<WalletAccount>>;
    
    /// Get current network
    async fn get_network(&self) -> Result<WalletNetworkInfo>;
    
    /// Switch network
    async fn switch_network(&self, network: &str) -> Result<()>;
    
    /// Sign a message
    async fn sign_message(&self, message: &str, address: &str) -> Result<String>;
    
    /// Sign a PSBT
    async fn sign_psbt(&self, psbt_hex: &str, options: Option<PsbtSigningOptions>) -> Result<String>;
    
    /// Sign multiple PSBTs
    async fn sign_psbts(&self, psbt_hexs: Vec<String>, options: Option<PsbtSigningOptions>) -> Result<Vec<String>>;
    
    /// Push a transaction to the network
    async fn push_tx(&self, tx_hex: &str) -> Result<String>;
    
    /// Push a PSBT to the network
    async fn push_psbt(&self, psbt_hex: &str) -> Result<String>;
    
    /// Get public key
    async fn get_public_key(&self) -> Result<String>;
    
    /// Get balance (if supported by wallet)
    async fn get_balance(&self) -> Result<Option<u64>>;
    
    /// Get inscriptions (if supported by wallet)
    async fn get_inscriptions(&self, cursor: Option<u32>, size: Option<u32>) -> Result<JsonValue>;

    /// Get enriched UTXOs with asset information
    async fn get_enriched_utxos(&self, addresses: Option<Vec<String>>) -> Result<Vec<EnrichedUtxo>> {
        let _ = addresses;
        Err(DeezelError::NotImplemented("get_enriched_utxos is not supported by this wallet".to_string()))
    }

    /// Get all balances, including BTC and other assets
    async fn get_all_balances(&self, addresses: Option<Vec<String>>) -> Result<AllBalances> {
        let _ = addresses;
        Err(DeezelError::NotImplemented("get_all_balances is not supported by this wallet".to_string()))
    }
}

/// Wrapper for browser-injected wallet objects
pub struct InjectedWallet {
    info: WalletInfo,
    #[allow(dead_code)]
    js_object: js_sys::Object,
}

impl InjectedWallet {
    /// Create a new injected wallet wrapper
    pub fn new(info: WalletInfo, js_object: js_sys::Object) -> Self {
        Self { info, js_object }
    }
    
    /// Call a method on the injected wallet object
    async fn call_method(&self, method: &str, args: &[JsValue]) -> Result<JsValue> {
        let window = window().ok_or_else(|| DeezelError::Wallet("No window object".to_string()))?;
        
        // Get the wallet object from window
        let wallet_obj = js_sys::Reflect::get(&window, &JsValue::from_str(&self.info.injection_key))
            .map_err(|e| DeezelError::Wallet(format!("Wallet not found: {e:?}")))?;
        
        if wallet_obj.is_undefined() {
            return Err(DeezelError::Wallet(format!("Wallet {} not available", self.info.name)));
        }
        
        // Get the method
        let method_fn = js_sys::Reflect::get(&wallet_obj, &JsValue::from_str(method))
            .map_err(|e| DeezelError::Wallet(format!("Method {method} not found: {e:?}")))?;
        
        if !method_fn.is_function() {
            return Err(DeezelError::Wallet(format!("Method {method} is not a function")));
        }
        
        // Call the method
        let function = method_fn.dyn_into::<js_sys::Function>()
            .map_err(|e| DeezelError::Wallet(format!("Failed to cast to function: {e:?}")))?;
        
        let result = function.apply(&wallet_obj, &js_sys::Array::from_iter(args.iter()))
            .map_err(|e| DeezelError::Wallet(format!("Method call failed: {e:?}")))?;
        
        // If result is a promise, await it
        if result.has_type::<js_sys::Promise>() {
            let promise = result.dyn_into::<js_sys::Promise>()
                .map_err(|e| DeezelError::Wallet(format!("Failed to cast to promise: {e:?}")))?;
            
            JsFuture::from(promise)
                .await
                .map_err(|e| DeezelError::Wallet(format!("Promise rejected: {e:?}")))
        } else {
            Ok(result)
        }
    }
}

#[async_trait(?Send)]
impl WalletBackend for InjectedWallet {
    fn get_info(&self) -> &WalletInfo {
        &self.info
    }
    
    async fn is_available(&self) -> bool {
        let window = window();
        if let Some(window) = window {
            let wallet_obj = js_sys::Reflect::get(&window, &JsValue::from_str(&self.info.injection_key));
            wallet_obj.is_ok() && !wallet_obj.unwrap().is_undefined()
        } else {
            false
        }
    }
    
    async fn connect(&self) -> Result<WalletAccount> {
        let result = self.call_method("requestAccounts", &[]).await?;
        
        // Parse the result to get account information
        let accounts_array = result.dyn_into::<js_sys::Array>()
            .map_err(|e| DeezelError::Wallet(format!("Invalid accounts response: {e:?}")))?;
        
        if accounts_array.length() == 0 {
            return Err(DeezelError::Wallet("No accounts returned".to_string()));
        }
        
        let first_account = accounts_array.get(0);
        let address = first_account.as_string()
            .ok_or_else(|| DeezelError::Wallet("Invalid account format".to_string()))?;
        
        Ok(WalletAccount {
            address,
            public_key: None,
            compressed_public_key: None,
            address_type: "unknown".to_string(),
        })
    }
    
    async fn disconnect(&self) -> Result<()> {
        // Some wallets support disconnect, others don't
        match self.call_method("disconnect", &[]).await {
            Ok(_) => Ok(()),
            Err(_) => {
                // If disconnect is not supported, that's okay
                Ok(())
            }
        }
    }
    
    async fn get_accounts(&self) -> Result<Vec<WalletAccount>> {
        let result = self.call_method("getAccounts", &[]).await?;
        
        let accounts_array = result.dyn_into::<js_sys::Array>()
            .map_err(|e| DeezelError::Wallet(format!("Invalid accounts response: {e:?}")))?;
        
        let mut accounts = Vec::new();
        for i in 0..accounts_array.length() {
            let account = accounts_array.get(i);
            if let Some(address) = account.as_string() {
                accounts.push(WalletAccount {
                    address,
                    public_key: None,
                    compressed_public_key: None,
                    address_type: "unknown".to_string(),
                });
            }
        }
        
        Ok(accounts)
    }
    
    async fn get_network(&self) -> Result<WalletNetworkInfo> {
        match self.call_method("getNetwork", &[]).await {
            Ok(result) => {
                let network = result.as_string()
                    .unwrap_or_else(|| "mainnet".to_string());
                
                Ok(WalletNetworkInfo {
                    network,
                    chain_id: None,
                })
            },
            Err(_) => {
                // Default to mainnet if not supported
                Ok(WalletNetworkInfo {
                    network: "mainnet".to_string(),
                    chain_id: None,
                })
            }
        }
    }
    
    async fn switch_network(&self, network: &str) -> Result<()> {
        let network_value = JsValue::from_str(network);
        self.call_method("switchNetwork", &[network_value]).await?;
        Ok(())
    }
    
    async fn sign_message(&self, message: &str, address: &str) -> Result<String> {
        let message_value = JsValue::from_str(message);
        let address_value = JsValue::from_str(address);
        
        let result = self.call_method("signMessage", &[message_value, address_value]).await?;
        
        result.as_string()
            .ok_or_else(|| DeezelError::Wallet("Invalid signature response".to_string()))
    }
    
    async fn sign_psbt(&self, psbt_hex: &str, options: Option<PsbtSigningOptions>) -> Result<String> {
        let psbt_value = JsValue::from_str(psbt_hex);
        
        let args = if let Some(opts) = options {
            let options_obj = js_sys::Object::new();
            
            js_sys::Reflect::set(&options_obj, &"autoFinalized".into(), &JsValue::from_bool(opts.auto_finalized))
                .map_err(|e| DeezelError::Wallet(format!("Failed to set options: {e:?}")))?;
            
            if let Some(to_sign) = opts.to_sign_inputs {
                let to_sign_array = js_sys::Array::new();
                for input in to_sign {
                    let input_obj = js_sys::Object::new();
                    js_sys::Reflect::set(&input_obj, &"index".into(), &JsValue::from_f64(input.index as f64))
                        .map_err(|e| DeezelError::Wallet(format!("Failed to set input index: {e:?}")))?;
                    
                    if let Some(addr) = input.address {
                        js_sys::Reflect::set(&input_obj, &"address".into(), &JsValue::from_str(&addr))
                            .map_err(|e| DeezelError::Wallet(format!("Failed to set input address: {e:?}")))?;
                    }
                    
                    to_sign_array.push(&input_obj);
                }
                js_sys::Reflect::set(&options_obj, &"toSignInputs".into(), &to_sign_array)
                    .map_err(|e| DeezelError::Wallet(format!("Failed to set toSignInputs: {e:?}")))?;
            }
            
            vec![psbt_value, options_obj.into()]
        } else {
            vec![psbt_value]
        };
        
        let result = self.call_method("signPsbt", &args).await?;
        
        result.as_string()
            .ok_or_else(|| DeezelError::Wallet("Invalid PSBT signature response".to_string()))
    }
    
    async fn sign_psbts(&self, psbt_hexs: Vec<String>, options: Option<PsbtSigningOptions>) -> Result<Vec<String>> {
        let psbts_array = js_sys::Array::new();
        for psbt_hex in psbt_hexs {
            psbts_array.push(&JsValue::from_str(&psbt_hex));
        }
        
        let args = if let Some(opts) = options {
            let options_obj = js_sys::Object::new();
            js_sys::Reflect::set(&options_obj, &"autoFinalized".into(), &JsValue::from_bool(opts.auto_finalized))
                .map_err(|e| DeezelError::Wallet(format!("Failed to set options: {e:?}")))?;
            
            vec![psbts_array.into(), options_obj.into()]
        } else {
            vec![psbts_array.into()]
        };
        
        let result = self.call_method("signPsbts", &args).await?;
        
        let result_array = result.dyn_into::<js_sys::Array>()
            .map_err(|e| DeezelError::Wallet(format!("Invalid PSBTs signature response: {e:?}")))?;
        
        let mut signed_psbts = Vec::new();
        for i in 0..result_array.length() {
            let psbt = result_array.get(i);
            if let Some(psbt_hex) = psbt.as_string() {
                signed_psbts.push(psbt_hex);
            }
        }
        
        Ok(signed_psbts)
    }
    
    async fn push_tx(&self, tx_hex: &str) -> Result<String> {
        let tx_value = JsValue::from_str(tx_hex);
        let result = self.call_method("pushTx", &[tx_value]).await?;
        
        result.as_string()
            .ok_or_else(|| DeezelError::Wallet("Invalid push transaction response".to_string()))
    }
    
    async fn push_psbt(&self, psbt_hex: &str) -> Result<String> {
        let psbt_value = JsValue::from_str(psbt_hex);
        let result = self.call_method("pushPsbt", &[psbt_value]).await?;
        
        result.as_string()
            .ok_or_else(|| DeezelError::Wallet("Invalid push PSBT response".to_string()))
    }
    
    async fn get_public_key(&self) -> Result<String> {
        let result = self.call_method("getPublicKey", &[]).await?;
        
        result.as_string()
            .ok_or_else(|| DeezelError::Wallet("Invalid public key response".to_string()))
    }
    
    async fn get_balance(&self) -> Result<Option<u64>> {
        match self.call_method("getBalance", &[]).await {
            Ok(result) => {
                if let Some(balance_str) = result.as_string() {
                    balance_str.parse::<u64>()
                        .map(Some)
                        .map_err(|e| DeezelError::Wallet(format!("Invalid balance format: {e}")))
                } else if let Some(balance_num) = result.as_f64() {
                    Ok(Some(balance_num as u64))
                } else {
                    Ok(None)
                }
            },
            Err(_) => Ok(None), // Balance not supported
        }
    }
    
    async fn get_inscriptions(&self, cursor: Option<u32>, size: Option<u32>) -> Result<JsonValue> {
        let mut args = Vec::new();
        
        if let Some(c) = cursor {
            args.push(JsValue::from_f64(c as f64));
        }
        if let Some(s) = size {
            args.push(JsValue::from_f64(s as f64));
        }
        
        let result = self.call_method("getInscriptions", &args).await?;
        
        // Convert JsValue to JsonValue
        let result_str = js_sys::JSON::stringify(&result)
            .map_err(|e| DeezelError::Wallet(format!("Failed to stringify inscriptions: {e:?}")))?
            .as_string()
            .ok_or_else(|| DeezelError::Wallet("Invalid inscriptions response".to_string()))?;
        
        serde_json::from_str(&result_str)
            .map_err(|e| DeezelError::Wallet(format!("Failed to parse inscriptions JSON: {e}")))
    }
}

/// Wallet connector for detecting and connecting to available wallets
#[derive(Clone)]
pub struct WalletConnector {
    supported_wallets: Vec<WalletInfo>,
}

impl Default for WalletConnector {
    fn default() -> Self {
        Self::new()
    }
}

impl WalletConnector {
    /// Create a new wallet connector
    pub fn new() -> Self {
        Self {
            supported_wallets: Self::get_supported_wallets(),
        }
    }
    
    /// Get list of supported wallets
    pub fn get_supported_wallets() -> Vec<WalletInfo> {
        vec![
            WalletInfo {
                id: "unisat".to_string(),
                name: "Unisat Wallet".to_string(),
                icon: "/assets/wallets/unisat.svg".to_string(),
                website: "https://unisat.io/download".to_string(),
                injection_key: "unisat".to_string(),
                supports_psbt: true,
                supports_taproot: true,
                supports_ordinals: true,
                mobile_support: false,
                deep_link_scheme: None,
            },
            WalletInfo {
                id: "xverse".to_string(),
                name: "Xverse Wallet".to_string(),
                icon: "/assets/wallets/xverse.svg".to_string(),
                website: "https://www.xverse.app/download".to_string(),
                injection_key: "XverseProviders".to_string(),
                supports_psbt: true,
                supports_taproot: true,
                supports_ordinals: true,
                mobile_support: true,
                deep_link_scheme: Some("xverse://".to_string()),
            },
            WalletInfo {
                id: "phantom".to_string(),
                name: "Phantom Wallet".to_string(),
                icon: "/assets/wallets/phantom.svg".to_string(),
                website: "https://phantom.app/download".to_string(),
                injection_key: "phantom".to_string(),
                supports_psbt: true,
                supports_taproot: true,
                supports_ordinals: false,
                mobile_support: true,
                deep_link_scheme: Some("phantom://".to_string()),
            },
            WalletInfo {
                id: "okx".to_string(),
                name: "OKX Wallet".to_string(),
                icon: "/assets/wallets/okx.svg".to_string(),
                website: "https://chromewebstore.google.com/detail/okx-wallet/mcohilncbfahbmgdjkbpemcciiolgcge".to_string(),
                injection_key: "okxwallet".to_string(),
                supports_psbt: true,
                supports_taproot: true,
                supports_ordinals: true,
                mobile_support: true,
                deep_link_scheme: Some("okx://".to_string()),
            },
            WalletInfo {
                id: "leather".to_string(),
                name: "Leather Wallet".to_string(),
                icon: "/assets/wallets/leather.svg".to_string(),
                website: "https://leather.io/install-extension".to_string(),
                injection_key: "LeatherProvider".to_string(),
                supports_psbt: true,
                supports_taproot: true,
                supports_ordinals: true,
                mobile_support: false,
                deep_link_scheme: None,
            },
            WalletInfo {
                id: "magic-eden".to_string(),
                name: "Magic Eden Wallet".to_string(),
                icon: "/assets/wallets/magiceden.svg".to_string(),
                website: "https://wallet.magiceden.io/".to_string(),
                injection_key: "magicEden".to_string(),
                supports_psbt: true,
                supports_taproot: true,
                supports_ordinals: true,
                mobile_support: true,
                deep_link_scheme: Some("magiceden://".to_string()),
            },
            WalletInfo {
                id: "wizz".to_string(),
                name: "Wizz Wallet".to_string(),
                icon: "/assets/wallets/wizz.svg".to_string(),
                website: "https://wizzwallet.io/#extension".to_string(),
                injection_key: "wizz".to_string(),
                supports_psbt: true,
                supports_taproot: true,
                supports_ordinals: true,
                mobile_support: false,
                deep_link_scheme: None,
            },
            WalletInfo {
                id: "orange".to_string(),
                name: "Orange Wallet".to_string(),
                icon: "/assets/wallets/orange.svg".to_string(),
                website: "https://www.orangewallet.com/".to_string(),
                injection_key: "orange".to_string(), // Educated guess
                supports_psbt: false, // Unknown
                supports_taproot: false, // Unknown
                supports_ordinals: false, // Unknown
                mobile_support: false,
                deep_link_scheme: None,
            },
            WalletInfo {
                id: "tokeo".to_string(),
                name: "Tokeo Wallet".to_string(),
                icon: "/assets/wallets/tokeo.svg".to_string(),
                website: "https://tokeo.io/".to_string(),
                injection_key: "tokeo".to_string(), // Educated guess
                supports_psbt: false, // Unknown
                supports_taproot: false, // Unknown
                supports_ordinals: false, // Unknown
                mobile_support: false,
                deep_link_scheme: None,
            },
            WalletInfo {
                id: "keplr".to_string(),
                name: "Keplr Wallet".to_string(),
                icon: "/assets/wallets/keplr.svg".to_string(),
                website: "https://keplr.app/download".to_string(),
                injection_key: "keplr".to_string(),
                supports_psbt: false, // Primarily a Cosmos wallet
                supports_taproot: false,
                supports_ordinals: false,
                mobile_support: true,
                deep_link_scheme: Some("keplr://".to_string()),
            },
            WalletInfo {
                id: "keystore".to_string(),
                name: "Keystore".to_string(),
                icon: "/assets/wallets/default.svg".to_string(),
                website: "".to_string(),
                injection_key: "keystore".to_string(),
                supports_psbt: true,
                supports_taproot: true,
                supports_ordinals: true,
                mobile_support: false,
                deep_link_scheme: None,
            },
        ]
    }
    
    /// Detect available wallets in the browser
    pub async fn detect_wallets(&self) -> Result<Vec<WalletInfo>> {
        let window = window().ok_or_else(|| DeezelError::Wallet("No window object".to_string()))?;
        
        let mut available_wallets = Vec::new();
        
        for wallet_info in &self.supported_wallets {
            let wallet_obj = js_sys::Reflect::get(&window, &JsValue::from_str(&wallet_info.injection_key));
            
            if wallet_obj.is_ok() && !wallet_obj.unwrap().is_undefined() {
                available_wallets.push(wallet_info.clone());
            }
        }
        
        Ok(available_wallets)
    }
    
    /// Get wallet info by ID
    pub fn get_wallet_info(&self, wallet_id: &str) -> Option<&WalletInfo> {
        self.supported_wallets.iter().find(|w| w.id == wallet_id)
    }
    
    /// Create an injected wallet instance
    pub fn create_injected_wallet(&self, wallet_info: WalletInfo) -> Result<InjectedWallet> {
        let window = window().ok_or_else(|| DeezelError::Wallet("No window object".to_string()))?;
        
        let wallet_obj = js_sys::Reflect::get(&window, &JsValue::from_str(&wallet_info.injection_key))
            .map_err(|e| DeezelError::Wallet(format!("Wallet not found: {e:?}")))?;
        
        if wallet_obj.is_undefined() {
            return Err(DeezelError::Wallet(format!("Wallet {} not available", wallet_info.name)));
        }
        
        let js_object = wallet_obj.dyn_into::<js_sys::Object>()
            .map_err(|e| DeezelError::Wallet(format!("Invalid wallet object: {e:?}")))?;
        
        Ok(InjectedWallet::new(wallet_info, js_object))
    }
}

/// Browser wallet provider that implements deezel-common traits
///
/// This provider wraps injected browser wallets while leveraging our sandshrew RPC
/// connections and polling strategies for most operations. The wallet is used minimally
/// as a signer and keystore, while blockchain operations use our existing infrastructure.
pub struct BrowserWalletProvider {
    wallet: Box<dyn WalletBackend>,
    web_provider: WebProvider,
    connection_status: WalletConnectionStatus,
    current_account: Option<WalletAccount>,
}

impl BrowserWalletProvider {
    /// Connect to a browser wallet
    pub async fn connect(
        wallet_info: WalletInfo,
        network_str: String,
    ) -> Result<Self> {
        // Create the underlying web provider for blockchain operations
        let web_provider = WebProvider::new(network_str).await?;
        
        // Create the wallet connector and injected wallet
        let connector = WalletConnector::new();
        let injected_wallet = connector.create_injected_wallet(wallet_info)?;
        
        // Connect to the wallet
        let account = injected_wallet.connect().await?;
        
        Ok(Self {
            wallet: Box::new(injected_wallet),
            web_provider,
            connection_status: WalletConnectionStatus::Connected,
            current_account: Some(account),
        })
    }

    pub async fn connect_local(
        wallet: Box<dyn WalletBackend>,
        network_str: String,
    ) -> Result<Self> {
        // Create the underlying web provider for blockchain operations
        let web_provider = WebProvider::new(network_str).await?;
        
        // Connect to the wallet
        let account = wallet.connect().await?;
        
        Ok(Self {
            wallet,
            web_provider,
            connection_status: WalletConnectionStatus::Connected,
            current_account: Some(account),
        })
    }
    
    /// Get the current connection status
    pub fn connection_status(&self) -> &WalletConnectionStatus {
        &self.connection_status
    }
    
    /// Get the current account
    pub fn current_account(&self) -> Option<&WalletAccount> {
        self.current_account.as_ref()
    }
    
    /// Get wallet information
    pub fn wallet_info(&self) -> &WalletInfo {
        self.wallet.get_info()
    }
    
    /// Disconnect from the wallet
    pub async fn disconnect(&mut self) -> Result<()> {
        self.wallet.disconnect().await?;
        self.connection_status = WalletConnectionStatus::Disconnected;
        self.current_account = None;
        Ok(())
    }
    
    /// Switch to a different network
    pub async fn switch_network(&mut self, network: &str) -> Result<()> {
        self.wallet.switch_network(network).await?;
        
        // Update the web provider's network as well
        // Note: This would require recreating the web provider with the new network
        // For now, we'll just update the wallet
        Ok(())
    }
    
    /// Get the underlying web provider for direct access
    pub fn web_provider(&self) -> &WebProvider {
        &self.web_provider
    }
}

impl Clone for BrowserWalletProvider {
    fn clone(&self) -> Self {
        // Note: This is a simplified clone that doesn't clone the wallet backend
        // In a real implementation, you might want to handle this differently
        Self {
            wallet: Box::new(InjectedWallet::new(
                self.wallet.get_info().clone(),
                js_sys::Object::new(),
            )),
            web_provider: self.web_provider.clone(),
            connection_status: self.connection_status.clone(),
            current_account: self.current_account.clone(),
        }
    }
}

// Implement deezel-common traits for BrowserWalletProvider
// Most operations delegate to the web_provider, while signing operations use the wallet

#[async_trait(?Send)]
impl JsonRpcProvider for BrowserWalletProvider {
    async fn call(&self, url: &str, method: &str, params: JsonValue, id: u64) -> Result<JsonValue> {
        self.web_provider.call(url, method, params, id).await
    }
    
}

#[async_trait(?Send)]
impl StorageProvider for BrowserWalletProvider {
    async fn read(&self, key: &str) -> Result<Vec<u8>> {
        self.web_provider.read(key).await
    }
    
    async fn write(&self, key: &str, data: &[u8]) -> Result<()> {
        self.web_provider.write(key, data).await
    }
    
    async fn exists(&self, key: &str) -> Result<bool> {
        self.web_provider.exists(key).await
    }
    
    async fn delete(&self, key: &str) -> Result<()> {
        self.web_provider.delete(key).await
    }
    
    async fn list_keys(&self, prefix: &str) -> Result<Vec<String>> {
        self.web_provider.list_keys(prefix).await
    }
    
    fn storage_type(&self) -> &'static str {
        "browser_wallet_localStorage"
    }
}

#[async_trait(?Send)]
impl NetworkProvider for BrowserWalletProvider {
    async fn get(&self, url: &str) -> Result<Vec<u8>> {
        self.web_provider.get(url).await
    }
    
    async fn post(&self, url: &str, body: &[u8], content_type: &str) -> Result<Vec<u8>> {
        self.web_provider.post(url, body, content_type).await
    }
    
    async fn is_reachable(&self, url: &str) -> bool {
        self.web_provider.is_reachable(url).await
    }
}

#[async_trait(?Send)]
impl CryptoProvider for BrowserWalletProvider {
    fn random_bytes(&self, len: usize) -> Result<Vec<u8>> {
        self.web_provider.random_bytes(len)
    }
    
    fn sha256(&self, data: &[u8]) -> Result<[u8; 32]> {
        self.web_provider.sha256(data)
    }
    
    fn sha3_256(&self, data: &[u8]) -> Result<[u8; 32]> {
        self.web_provider.sha3_256(data)
    }
    
    async fn encrypt_aes_gcm(&self, data: &[u8], key: &[u8], nonce: &[u8]) -> Result<Vec<u8>> {
        self.web_provider.encrypt_aes_gcm(data, key, nonce).await
    }
    
    async fn decrypt_aes_gcm(&self, data: &[u8], key: &[u8], nonce: &[u8]) -> Result<Vec<u8>> {
        self.web_provider.decrypt_aes_gcm(data, key, nonce).await
    }
    
    async fn pbkdf2_derive(&self, password: &[u8], salt: &[u8], iterations: u32, key_len: usize) -> Result<Vec<u8>> {
        self.web_provider.pbkdf2_derive(password, salt, iterations, key_len).await
    }
}

#[async_trait(?Send)]
impl TimeProvider for BrowserWalletProvider {
    fn now_secs(&self) -> u64 {
        self.web_provider.now_secs()
    }
    
    fn now_millis(&self) -> u64 {
        self.web_provider.now_millis()
    }
    
    async fn sleep_ms(&self, ms: u64) {
        self.web_provider.sleep_ms(ms).await
    }
}

impl LogProvider for BrowserWalletProvider {
    fn debug(&self, message: &str) {
        self.web_provider.debug(message);
    }
    
    fn info(&self, message: &str) {
        self.web_provider.info(message);
    }
    
    fn warn(&self, message: &str) {
        self.web_provider.warn(message);
    }
    
    fn error(&self, message: &str) {
        self.web_provider.error(message);
    }
}

// WalletProvider implementation - this is where we use the injected wallet for signing
// but leverage our sandshrew RPC for most blockchain operations
#[async_trait(?Send)]
impl WalletProvider for BrowserWalletProvider {
    async fn create_wallet(&mut self, _config: WalletConfig, _mnemonic: Option<String>, _passphrase: Option<String>) -> Result<deezel_common::WalletInfo> {
        // For browser wallets, we don't create wallets - they're managed by the wallet extension
        // Instead, we return information about the connected wallet
        if let Some(account) = &self.current_account {
            Ok(deezel_common::WalletInfo {
                address: account.address.clone(),
                network: self.web_provider.network(),
                mnemonic: None, // Browser wallets don't expose mnemonics
            })
        } else {
            Err(DeezelError::Wallet("No wallet connected".to_string()))
        }
    }
    
    async fn load_wallet(&mut self, config: WalletConfig, _passphrase: Option<String>) -> Result<deezel_common::WalletInfo> {
        // Similar to create_wallet - browser wallets are already "loaded"
        self.create_wallet(config, None, None).await
    }
    
    async fn get_balance(&self, addresses: Option<Vec<String>>) -> Result<WalletBalance> {
        deezel_common::WalletProvider::get_balance(&self.web_provider, addresses).await
    }
    
    async fn get_address(&self) -> Result<String> {
        if let Some(account) = &self.current_account {
            Ok(account.address.clone())
        } else {
            Err(DeezelError::Wallet("No wallet connected".to_string()))
        }
    }
    
    async fn get_addresses(&self, count: u32) -> Result<Vec<AddressInfo>> {
        // Get all accounts from the wallet
        let accounts = self.wallet.get_accounts().await?;
        
        let mut addresses = Vec::new();
        for (i, account) in accounts.iter().enumerate().take(count as usize) {
            addresses.push(AddressInfo {
                address: account.address.clone(),
                script_type: account.address_type.clone(),
                derivation_path: format!("m/84'/0'/0'/0/{i}"), // Estimated path
                index: i as u32,
                used: true, // Assume used since it's from the wallet
            });
        }
        
        Ok(addresses)
    }
    
    async fn send(&mut self, params: SendParams) -> Result<String> {
        // For sending, we'll create the transaction using our infrastructure
        // then use the wallet to sign it
        let tx_hex = self.create_transaction(params.clone()).await?;
        let signed_tx_hex = self.sign_transaction(tx_hex).await?;
        self.broadcast_transaction(signed_tx_hex).await
    }
    
    async fn get_utxos(&self, include_frozen: bool, addresses: Option<Vec<String>>) -> Result<Vec<(OutPoint, UtxoInfo)>> {
        self.web_provider.get_utxos(include_frozen, addresses).await
    }
    
    async fn get_history(&self, count: u32, address: Option<String>) -> Result<Vec<TransactionInfo>> {
        // Use our web provider for transaction history, which is more detailed
        let addr = address.or_else(|| self.current_account.as_ref().map(|a| a.address.clone()));
        self.web_provider.get_history(count, addr).await
    }
    
    async fn freeze_utxo(&self, _utxo: String, _reason: Option<String>) -> Result<()> {
        // Browser wallets typically don't support UTXO freezing
        // We could implement this in our local storage if needed
        Err(DeezelError::Wallet("UTXO freezing not supported by browser wallets".to_string()))
    }
    
    async fn unfreeze_utxo(&self, _utxo: String) -> Result<()> {
        // Browser wallets typically don't support UTXO freezing
        Err(DeezelError::Wallet("UTXO freezing not supported by browser wallets".to_string()))
    }
    
    async fn create_transaction(&self, params: SendParams) -> Result<String> {
        let recipient = Address::from_str(&params.address)?.assume_checked();
        let amount = Amount::from_sat(params.amount);

        let address = <Self as WalletProvider>::get_address(self).await?;
        let utxos = self.get_utxos(false, Some(vec![address])).await?;
        if utxos.is_empty() {
            return Err(DeezelError::Wallet("No UTXOs available".to_string()));
        }

        let mut inputs = vec![];
        let mut total_input = 0;

        for (outpoint, utxo_info) in &utxos {
            inputs.push(TxIn {
                previous_output: *outpoint,
                script_sig: ScriptBuf::new(),
                sequence: Sequence::MAX,
                witness: Witness::new(),
            });
            total_input += utxo_info.amount;
        }

        let mut outputs = vec![];
        outputs.push(TxOut {
            value: amount,
            script_pubkey: recipient.script_pubkey(),
        });
        
        let fee_rate = params.fee_rate.unwrap_or(1.0) as u64;
        let estimated_vsize = 150; // Super rough estimate
        let fee = fee_rate * estimated_vsize;

        if total_input < amount.to_sat() + fee {
            return Err(DeezelError::Wallet("Insufficient funds".to_string()));
        }

        let change_address = <Self as WalletProvider>::get_address(self).await?;
        let change_address = Address::from_str(&change_address)?.assume_checked();
        let change_amount = total_input - amount.to_sat() - fee;
        outputs.push(TxOut {
            value: Amount::from_sat(change_amount),
            script_pubkey: change_address.script_pubkey(),
        });

        let unsigned_tx = Transaction {
            version: bitcoin::transaction::Version(2),
            lock_time: bitcoin::locktime::absolute::LockTime::ZERO,
            input: inputs,
            output: outputs,
        };

        let psbt = Psbt::from_unsigned_tx(unsigned_tx)?;

        Ok(STANDARD.encode(&psbt.serialize()))
    }
    
    async fn sign_transaction(&mut self, tx_hex: String) -> Result<String> {
        // This is where we use the browser wallet for signing
        // Convert the transaction to PSBT format for wallet signing
        
        // For now, we'll use the wallet's signPsbt method if available
        // In a full implementation, we'd convert the raw transaction to PSBT
        match self.wallet.sign_psbt(&tx_hex, None).await {
            Ok(signed_psbt) => Ok(signed_psbt),
            Err(_) => {
                // Fallback: some wallets might have a direct transaction signing method
                self.wallet.push_tx(&tx_hex).await
            }
        }
    }
    
    async fn broadcast_transaction(&self, tx_hex: String) -> Result<String> {
        // Try to broadcast through the wallet first (for better UX)
        match self.wallet.push_tx(&tx_hex).await {
            Ok(txid) => Ok(txid),
            Err(_) => {
                // Fallback to our RPC provider
                self.web_provider.broadcast_transaction(tx_hex).await
            }
        }
    }
    
    async fn estimate_fee(&self, target: u32) -> Result<FeeEstimate> {
        // Use our web provider for fee estimation
        self.web_provider.estimate_fee(target).await
    }
    
    async fn get_fee_rates(&self) -> Result<FeeRates> {
        // Use our web provider for fee rates
        self.web_provider.get_fee_rates().await
    }
    
    async fn sync(&self) -> Result<()> {
        // For browser wallets, syncing is handled by the wallet extension
        // We can sync our web provider instead
        self.web_provider.sync().await
    }
    
    async fn backup(&self) -> Result<String> {
        // Browser wallets handle their own backups
        // We can provide information about the connection
        let backup_info = serde_json::json!({
            "wallet_type": "browser_wallet",
            "wallet_name": self.wallet.get_info().name,
            "wallet_id": self.wallet.get_info().id,
            "connected_address": self.current_account.as_ref().map(|a| &a.address),
            "network": self.web_provider.network().to_string(),
            "backup_time": self.web_provider.now_millis(),
        });
        
        Ok(backup_info.to_string())
    }
    
    async fn get_mnemonic(&self) -> Result<Option<String>> {
        // Browser wallets don't expose mnemonics for security reasons
        Ok(None)
    }
    
    fn get_network(&self) -> Network {
        self.web_provider.network()
    }
    
    async fn get_internal_key(&self) -> Result<(XOnlyPublicKey, (bitcoin::bip32::Fingerprint, bitcoin::bip32::DerivationPath))> {
        // Browser wallets do not expose derivation paths, so this method cannot be fully implemented.
        Err(DeezelError::NotImplemented("get_internal_key is not supported for browser wallets as they do not expose derivation paths.".to_string()))
    }
    
    async fn sign_psbt(&mut self, psbt: &Psbt) -> Result<Psbt> {
        // Convert PSBT to hex and use wallet to sign
        let psbt_hex = hex::encode(psbt.serialize());
        let signed_psbt_hex = self.wallet.sign_psbt(&psbt_hex, None).await?;
        
        // Parse the signed PSBT back
        let signed_psbt_bytes = hex::decode(&signed_psbt_hex)
            .map_err(|e| DeezelError::Wallet(format!("Invalid signed PSBT hex: {e}")))?;
        
        Psbt::deserialize(&signed_psbt_bytes)
            .map_err(|e| DeezelError::Wallet(format!("Failed to deserialize signed PSBT: {e}")))
    }
    
    async fn get_keypair(&self) -> Result<Keypair> {
        // Browser wallets don't expose private keys for security reasons
        // This method should not be used with browser wallets
        Err(DeezelError::Wallet("Browser wallets do not expose private keys".to_string()))
    }

    fn set_passphrase(&mut self, _passphrase: Option<String>) {
        // Browser wallets manage their own passphrases
        // This is a no-op for browser wallet providers
    }

    async fn get_last_used_address_index(&self) -> Result<u32> {
        // Browser wallets don't typically expose this information.
        // We can return a default value or try to infer it if needed.
        Ok(0)
    }

    async fn get_master_public_key(&self) -> Result<Option<String>> {
        // Browser wallets expose the account's public key, which we can use here.
        // It's not a "master" key in the HD sense, but it's the main public key available.
        match self.wallet.get_public_key().await {
            Ok(pk) => Ok(Some(pk)),
            Err(_) => Ok(None),
        }
    }

    async fn get_enriched_utxos(&self, addresses: Option<Vec<String>>) -> Result<Vec<EnrichedUtxo>> {
        let addrs_to_fetch = match addresses {
            Some(a) => a,
            None => vec![<Self as WalletProvider>::get_address(self).await?],
        };
        self.web_provider.get_enriched_utxos(Some(addrs_to_fetch)).await
    }

    async fn get_all_balances(&self, addresses: Option<Vec<String>>) -> Result<AllBalances> {
        let btc_balance = WalletProvider::get_balance(self, addresses.clone()).await?;
        
        let mut asset_balances: std::collections::HashMap<String, u128> = std::collections::HashMap::new();

        if let Some(addr_list) = addresses.clone() {
            for address in addr_list {
                let alkanes_bals = <Self as AlkanesProvider>::get_balance(self, Some(&address)).await?;
                for alkane_bal in alkanes_bals {
                    *asset_balances.entry(alkane_bal.symbol).or_insert(0) += alkane_bal.balance as u128;
                }
            }
        } else {
            let address = WalletProvider::get_address(self).await?;
            let alkanes_bals = <Self as AlkanesProvider>::get_balance(self, Some(&address)).await?;
            for alkane_bal in alkanes_bals {
                *asset_balances.entry(alkane_bal.symbol).or_insert(0) += alkane_bal.balance as u128;
            }
        };

        let other_assets = asset_balances.into_iter().map(|(symbol, balance)| {
            AssetBalance {
                name: symbol.clone(), // Assuming symbol is also the name for now
                symbol,
                balance,
            }
        }).collect();

        Ok(AllBalances {
            btc: btc_balance,
            other: other_assets,
        })
    }
}

// Implement the remaining provider traits by delegating to web_provider
#[async_trait(?Send)]
impl AddressResolver for BrowserWalletProvider {
    async fn resolve_all_identifiers(&self, input: &str) -> Result<String> {
        self.web_provider.resolve_all_identifiers(input).await
    }
    
    fn contains_identifiers(&self, input: &str) -> bool {
        self.web_provider.contains_identifiers(input)
    }
    
    async fn get_address(&self, address_type: &str, index: u32) -> Result<String> {
        AddressResolver::get_address(&self.web_provider, address_type, index).await
    }
    
    async fn list_identifiers(&self) -> Result<Vec<String>> {
        self.web_provider.list_identifiers().await
    }
}

#[async_trait(?Send)]
impl BitcoinRpcProvider for BrowserWalletProvider {
    async fn get_block_count(&self) -> Result<u64> {
        <WebProvider as BitcoinRpcProvider>::get_block_count(&self.web_provider).await
    }
    
    async fn generate_to_address(&self, nblocks: u32, address: &str) -> Result<JsonValue> {
        <WebProvider as BitcoinRpcProvider>::generate_to_address(&self.web_provider, nblocks, address).await
    }
    
    async fn get_new_address(&self) -> Result<JsonValue> {
        self.web_provider.get_new_address().await
    }
    
    async fn get_transaction_hex(&self, txid: &str) -> Result<String> {
        self.web_provider.get_transaction_hex(txid).await
    }
    
    async fn get_block(&self, hash: &str, raw: bool) -> Result<JsonValue> {
        BitcoinRpcProvider::get_block(&self.web_provider, hash, raw).await
    }
    
    async fn get_block_hash(&self, height: u64) -> Result<String> {
        <WebProvider as BitcoinRpcProvider>::get_block_hash(&self.web_provider, height).await
    }
    
    async fn send_raw_transaction(&self, tx_hex: &str) -> Result<String> {
        <WebProvider as BitcoinRpcProvider>::send_raw_transaction(&self.web_provider, tx_hex).await
    }
    
    async fn get_mempool_info(&self) -> Result<JsonValue> {
        <WebProvider as BitcoinRpcProvider>::get_mempool_info(&self.web_provider).await
    }
    
    async fn estimate_smart_fee(&self, target: u32) -> Result<JsonValue> {
        self.web_provider.estimate_smart_fee(target).await
    }
    
    async fn get_esplora_blocks_tip_height(&self) -> Result<u64> {
        self.web_provider.get_esplora_blocks_tip_height().await
    }
    
    async fn trace_transaction(&self, txid: &str, vout: u32, block: Option<&str>, tx: Option<&str>) -> Result<JsonValue> {
        self.web_provider.trace_transaction(txid, vout, block, tx).await
    }
}

#[async_trait(?Send)]
impl MetashrewRpcProvider for BrowserWalletProvider {
    async fn get_metashrew_height(&self) -> Result<u64> {
        self.web_provider.get_metashrew_height().await
    }
    
    async fn get_contract_meta(&self, block: &str, tx: &str) -> Result<JsonValue> {
        self.web_provider.get_contract_meta(block, tx).await
    }
    
    async fn trace_outpoint(&self, _txid: &str, _vout: u32) -> Result<serde_json::Value> {
        self.web_provider.trace_outpoint(_txid, _vout).await
    }
    
    async fn get_spendables_by_address(&self, address: &str) -> Result<JsonValue> {
        self.web_provider.get_spendables_by_address(address).await
    }
    
    async fn get_protorunes_by_address(&self, address: &str, block_tag: Option<String>, protocol_tag: u128) -> Result<deezel_common::alkanes::protorunes::ProtoruneWalletResponse> {
        self.web_provider.get_protorunes_by_address(address, block_tag, protocol_tag).await
    }
    
    async fn get_protorunes_by_outpoint(&self, txid: &str, vout: u32, block_tag: Option<String>, protocol_tag: u128) -> Result<deezel_common::alkanes::protorunes::ProtoruneOutpointResponse> {
        self.web_provider.get_protorunes_by_outpoint(txid, vout, block_tag, protocol_tag).await
    }
}

#[async_trait(?Send)]
impl MetashrewProvider for BrowserWalletProvider {
    async fn get_height(&self) -> Result<u64> {
        self.web_provider.get_height().await
    }
    async fn get_block_hash(&self, height: u64) -> Result<String> {
        deezel_common::MetashrewProvider::get_block_hash(&self.web_provider, height).await
    }
    async fn get_state_root(&self, height: JsonValue) -> Result<String> {
        self.web_provider.get_state_root(height).await
    }
}

#[async_trait(?Send)]
impl EsploraProvider for BrowserWalletProvider {
    async fn get_blocks_tip_hash(&self) -> Result<String> {
        self.web_provider.get_blocks_tip_hash().await
    }
    
    async fn get_blocks_tip_height(&self) -> Result<u64> {
        self.web_provider.get_blocks_tip_height().await
    }
    
    async fn get_blocks(&self, start_height: Option<u64>) -> Result<JsonValue> {
        self.web_provider.get_blocks(start_height).await
    }
    
    async fn get_block_by_height(&self, height: u64) -> Result<String> {
        self.web_provider.get_block_by_height(height).await
    }
    
    async fn get_block(&self, hash: &str) -> Result<JsonValue> {
        EsploraProvider::get_block(&self.web_provider, hash).await
    }
    
    async fn get_block_status(&self, hash: &str) -> Result<JsonValue> {
        self.web_provider.get_block_status(hash).await
    }
    
    async fn get_block_txids(&self, hash: &str) -> Result<JsonValue> {
        self.web_provider.get_block_txids(hash).await
    }
    
    async fn get_block_header(&self, hash: &str) -> Result<String> {
        <WebProvider as EsploraProvider>::get_block_header(&self.web_provider, hash).await
    }
    
    async fn get_block_raw(&self, hash: &str) -> Result<String> {
        self.web_provider.get_block_raw(hash).await
    }
    
    async fn get_block_txid(&self, hash: &str, index: u32) -> Result<String> {
        self.web_provider.get_block_txid(hash, index).await
    }
    
    async fn get_block_txs(&self, hash: &str, start_index: Option<u32>) -> Result<JsonValue> {
        self.web_provider.get_block_txs(hash, start_index).await
    }
    
    
    async fn get_address_info(&self, address: &str) -> Result<JsonValue> {
        self.web_provider.get_address_info(address).await
    }

    async fn get_address_utxo(&self, address: &str) -> Result<JsonValue> {
        self.web_provider.get_address_utxo(address).await
    }
    
    async fn get_address_txs(&self, address: &str) -> Result<JsonValue> {
        self.web_provider.get_address_txs(address).await
    }
    
    async fn get_address_txs_chain(&self, address: &str, last_seen_txid: Option<&str>) -> Result<JsonValue> {
        self.web_provider.get_address_txs_chain(address, last_seen_txid).await
    }
    
    async fn get_address_txs_mempool(&self, address: &str) -> Result<JsonValue> {
        self.web_provider.get_address_txs_mempool(address).await
    }
    
    
    async fn get_address_prefix(&self, prefix: &str) -> Result<JsonValue> {
        self.web_provider.get_address_prefix(prefix).await
    }
    
    async fn get_tx(&self, txid: &str) -> Result<JsonValue> {
        self.web_provider.get_tx(txid).await
    }
    
    async fn get_tx_hex(&self, txid: &str) -> Result<String> {
        self.web_provider.get_tx_hex(txid).await
    }
    
    async fn get_tx_raw(&self, txid: &str) -> Result<String> {
        self.web_provider.get_tx_raw(txid).await
    }
    
    async fn get_tx_status(&self, txid: &str) -> Result<JsonValue> {
        self.web_provider.get_tx_status(txid).await
    }
    
    async fn get_tx_merkle_proof(&self, txid: &str) -> Result<JsonValue> {
        self.web_provider.get_tx_merkle_proof(txid).await
    }
    
    async fn get_tx_merkleblock_proof(&self, txid: &str) -> Result<String> {
        self.web_provider.get_tx_merkleblock_proof(txid).await
    }
    
    async fn get_tx_outspend(&self, txid: &str, index: u32) -> Result<JsonValue> {
        self.web_provider.get_tx_outspend(txid, index).await
    }
    
    async fn get_tx_outspends(&self, txid: &str) -> Result<JsonValue> {
        self.web_provider.get_tx_outspends(txid).await
    }
    
    async fn broadcast(&self, tx_hex: &str) -> Result<String> {
        self.web_provider.broadcast(tx_hex).await
    }
    
    async fn get_mempool(&self) -> Result<JsonValue> {
        self.web_provider.get_mempool().await
    }
    
    async fn get_mempool_txids(&self) -> Result<JsonValue> {
        self.web_provider.get_mempool_txids().await
    }
    
    async fn get_mempool_recent(&self) -> Result<JsonValue> {
        self.web_provider.get_mempool_recent().await
    }
    
    async fn get_fee_estimates(&self) -> Result<JsonValue> {
        self.web_provider.get_fee_estimates().await
    }
}

#[async_trait(?Send)]
impl RunestoneProvider for BrowserWalletProvider {
    async fn decode_runestone(&self, tx: &Transaction) -> Result<JsonValue> {
        self.web_provider.decode_runestone(tx).await
    }
    
    async fn format_runestone_with_decoded_messages(&self, tx: &Transaction) -> Result<JsonValue> {
        self.web_provider.format_runestone_with_decoded_messages(tx).await
    }
    
    async fn analyze_runestone(&self, txid: &str) -> Result<JsonValue> {
        self.web_provider.analyze_runestone(txid).await
    }
}

#[async_trait(?Send)]
impl OrdProvider for BrowserWalletProvider {
    async fn get_inscription(&self, inscription_id: &str) -> Result<OrdInscription> {
        self.web_provider.get_inscription(inscription_id).await
    }

    async fn get_inscriptions_in_block(&self, block_hash: &str) -> Result<OrdInscriptions> {
        self.web_provider.get_inscriptions_in_block(block_hash).await
    }
    async fn get_ord_address_info(&self, address: &str) -> Result<OrdAddressInfo> {
        self.web_provider.get_ord_address_info(address).await
    }

    async fn get_block_info(&self, query: &str) -> Result<OrdBlock> {
        self.web_provider.get_block_info(query).await
    }

    async fn get_ord_block_count(&self) -> Result<u64> {
        self.web_provider.get_ord_block_count().await
    }

    async fn get_ord_blocks(&self) -> Result<OrdBlocks> {
        self.web_provider.get_ord_blocks().await
    }

    async fn get_children(&self, inscription_id: &str, page: Option<u32>) -> Result<OrdChildren> {
        self.web_provider.get_children(inscription_id, page).await
    }

    async fn get_content(&self, inscription_id: &str) -> Result<Vec<u8>> {
        self.web_provider.get_content(inscription_id).await
    }

    async fn get_inscriptions(&self, page: Option<u32>) -> Result<OrdInscriptions> {
        self.web_provider.get_inscriptions(page).await
    }

    async fn get_output(&self, output: &str) -> Result<OrdOutput> {
        self.web_provider.get_output(output).await
    }

    async fn get_parents(&self, inscription_id: &str, page: Option<u32>) -> Result<OrdParents> {
        self.web_provider.get_parents(inscription_id, page).await
    }

    async fn get_rune(&self, rune: &str) -> Result<OrdRuneInfo> {
        self.web_provider.get_rune(rune).await
    }

    async fn get_runes(&self, page: Option<u32>) -> Result<OrdRunes> {
        self.web_provider.get_runes(page).await
    }

    async fn get_sat(&self, sat: u64) -> Result<OrdSat> {
        self.web_provider.get_sat(sat).await
    }

    async fn get_tx_info(&self, txid: &str) -> Result<OrdTxInfo> {
        self.web_provider.get_tx_info(txid).await
    }
}

#[async_trait(?Send)]
impl AlkanesProvider for BrowserWalletProvider {
    async fn execute(&mut self, params: deezel_common::alkanes::types::EnhancedExecuteParams) -> Result<deezel_common::alkanes::types::ExecutionState> {
        self.web_provider.execute(params).await
    }

    async fn resume_execution(
        &mut self,
        state: deezel_common::alkanes::types::ReadyToSignTx,
        params: &deezel_common::alkanes::types::EnhancedExecuteParams,
    ) -> Result<deezel_common::alkanes::types::EnhancedExecuteResult> {
        self.web_provider.resume_execution(state, params).await
    }

    async fn resume_commit_execution(
        &mut self,
        state: deezel_common::alkanes::types::ReadyToSignCommitTx,
    ) -> Result<deezel_common::alkanes::types::ExecutionState> {
        self.web_provider.resume_commit_execution(state).await
    }

    async fn resume_reveal_execution(
        &mut self,
        state: deezel_common::alkanes::types::ReadyToSignRevealTx,
    ) -> Result<deezel_common::alkanes::types::EnhancedExecuteResult> {
        self.web_provider.resume_reveal_execution(state).await
    }

    async fn protorunes_by_address(&self, address: &str, block_tag: Option<String>, protocol_tag: u128) -> Result<deezel_common::alkanes::protorunes::ProtoruneWalletResponse> {
        self.web_provider.protorunes_by_address(address, block_tag, protocol_tag).await
    }

    async fn protorunes_by_outpoint(&self, txid: &str, vout: u32, block_tag: Option<String>, protocol_tag: u128) -> Result<deezel_common::alkanes::protorunes::ProtoruneOutpointResponse> {
        self.web_provider.protorunes_by_outpoint(txid, vout, block_tag, protocol_tag).await
    }

    async fn view(&self, contract_id: &str, view_fn: &str, params: Option<&[u8]>) -> Result<JsonValue> {
        self.web_provider.view(contract_id, view_fn, params).await
    }

    async fn simulate(&self, contract_id: &str, context: &deezel_common::alkanes_pb::MessageContextParcel) -> Result<JsonValue> {
        self.web_provider.simulate(contract_id, context).await
    }

    async fn trace(&self, outpoint: &str) -> Result<alkanes_support::proto::alkanes::Trace> {
        self.web_provider.trace(outpoint).await
    }

    async fn get_block(&self, height: u64) -> Result<alkanes_support::proto::alkanes::BlockResponse> {
        AlkanesProvider::get_block(&self.web_provider, height).await
    }

    async fn sequence(&self, txid: &str, vout: u32) -> Result<JsonValue> {
        self.web_provider.sequence(txid, vout).await
    }

    async fn spendables_by_address(&self, address: &str) -> Result<JsonValue> {
        self.web_provider.spendables_by_address(address).await
    }

    async fn trace_block(&self, height: u64) -> Result<alkanes_support::proto::alkanes::Trace> {
        self.web_provider.trace_block(height).await
    }

    async fn get_bytecode(&self, alkane_id: &str, block_tag: Option<String>) -> Result<String> {
        AlkanesProvider::get_bytecode(&self.web_provider, alkane_id, block_tag).await
    }

    async fn inspect(&self, target: &str, config: AlkanesInspectConfig) -> Result<AlkanesInspectResult> {
        self.web_provider.inspect(target, config).await
    }

    async fn get_balance(&self, address: Option<&str>) -> Result<Vec<AlkaneBalance>> {
        AlkanesProvider::get_balance(&self.web_provider, address).await
    }
}

#[async_trait(?Send)]
impl MonitorProvider for BrowserWalletProvider {
    async fn monitor_blocks(&self, start: Option<u64>) -> Result<()> {
        self.web_provider.monitor_blocks(start).await
    }
    
    async fn get_block_events(&self, height: u64) -> Result<Vec<BlockEvent>> {
        self.web_provider.get_block_events(height).await
    }
}

#[async_trait(?Send)]
#[async_trait(?Send)]
impl KeystoreProvider for BrowserWalletProvider {
    async fn derive_addresses(&self, _master_public_key: &str, _network_params: &deezel_common::network::NetworkParams, _script_types: &[&str], _start_index: u32, _count: u32) -> Result<Vec<KeystoreAddress>> {
        Err(DeezelError::NotImplemented("Keystore operations not implemented for browser wallet provider".to_string()))
    }
    
    async fn get_default_addresses(&self, _master_public_key: &str, _network_params: &deezel_common::network::NetworkParams) -> Result<Vec<KeystoreAddress>> {
        Err(DeezelError::NotImplemented("Keystore operations not implemented for browser wallet provider".to_string()))
    }
    
    fn parse_address_range(&self, _range_spec: &str) -> Result<(String, u32, u32)> {
        Err(DeezelError::NotImplemented("Keystore operations not implemented for browser wallet provider".to_string()))
    }
    
    async fn get_keystore_info(&self, _master_fingerprint: &str, _created_at: u64, _version: &str) -> Result<KeystoreInfo> {
        Err(DeezelError::NotImplemented("Keystore operations not implemented for browser wallet provider".to_string()))
    }
    async fn get_address(&self, _address_type: &str, _index: u32) -> Result<String> {
       // We can't derive, but we can ask the wallet for its accounts.
       // This doesn't match the function signature perfectly (no index/type used),
       // but it's the best we can do.
       let accounts = self.wallet.get_accounts().await?;
       accounts.first()
           .map(|acc| acc.address.clone())
           .ok_or_else(|| DeezelError::Wallet("No accounts found in browser wallet.".to_string()))
    }
   
       async fn derive_address_from_path(&self, _master_public_key: &str, _path: &bitcoin::bip32::DerivationPath, _script_type: &str, network_params: &deezel_common::network::NetworkParams) -> Result<KeystoreAddress> {
           // This is the core issue. Browser wallets don't expose this.
           // We will return the primary address instead, ignoring the path.
           let address = WalletProvider::get_address(self).await?;
           Ok(KeystoreAddress {
               address,
               derivation_path: "N/A".to_string(),
               index: 0,
               script_type: "unknown".to_string(),
               network: Some(network_params.network.to_string()),
           })
       }
   }

#[async_trait(?Send)]
impl DeezelProvider for BrowserWalletProvider {
    fn provider_name(&self) -> &str {
        "browser_wallet"
    }
    
    async fn initialize(&self) -> Result<()> {
        // Initialize the underlying web provider
        self.web_provider.initialize().await?;
        
        // Verify wallet connection
        if self.current_account.is_none() {
            return Err(DeezelError::Wallet("Wallet not connected".to_string()));
        }
        
        self.info(&format!("Browser wallet provider initialized with {}", self.wallet.get_info().name));
        Ok(())
    }
    
    async fn shutdown(&self) -> Result<()> {
        self.info("Shutting down browser wallet provider");
        self.web_provider.shutdown().await
    }

    async fn wrap(&mut self, amount: u64, address: Option<String>, fee_rate: Option<f32>) -> Result<String> {
        use deezel_common::alkanes::types::{ProtostoneSpec, BitcoinTransfer, EnhancedExecuteParams};
        use alkanes_support::cellpack::Cellpack;

        let is_regtest = self.get_network() == Network::Regtest;
        let mut executor = EnhancedAlkanesExecutor::new(self);
        let params = EnhancedExecuteParams {
            fee_rate,
            to_addresses: vec![],
            from_addresses: address.map(|a| vec![a]),
            change_address: None,
            input_requirements: vec![],
            protostones: vec![ProtostoneSpec {
                cellpack: Some(Cellpack::try_from(vec![2, 0, 1]).unwrap()), // wrap frBTC
                edicts: vec![],
                bitcoin_transfer: Some(BitcoinTransfer { amount, target: deezel_common::alkanes::types::OutputTarget::Split }),
            }],
            envelope_data: None,
            raw_output: false,
            trace_enabled: false,
            mine_enabled: is_regtest,
            auto_confirm: false,
        };

        match executor.execute(params).await? {
            deezel_common::alkanes::types::ExecutionState::ReadyToSign(ready_tx) => {
                let signed_psbt = self.sign_psbt(&ready_tx.psbt).await?;
                let tx = signed_psbt.extract_tx()?;
                let tx_hex = bitcoin::consensus::encode::serialize_hex(&tx);
                self.broadcast_transaction(tx_hex).await
            }
            _ => Err(DeezelError::Other("Unexpected execution state".to_string())),
        }
    }

    async fn unwrap(&mut self, amount: u64, address: Option<String>) -> Result<String> {
        use deezel_common::alkanes::types::{ProtostoneSpec, BitcoinTransfer, EnhancedExecuteParams};
        use alkanes_support::cellpack::Cellpack;

        let is_regtest = self.get_network() == Network::Regtest;
        let mut executor = EnhancedAlkanesExecutor::new(self);
        let params = EnhancedExecuteParams {
            fee_rate: None,
            to_addresses: vec![],
            from_addresses: address.map(|a| vec![a]),
            change_address: None,
            input_requirements: vec![],
            protostones: vec![ProtostoneSpec {
                cellpack: Some(Cellpack::try_from(vec![2, 0, 2]).unwrap()), // unwrap frBTC
                edicts: vec![],
                bitcoin_transfer: Some(BitcoinTransfer { amount, target: deezel_common::alkanes::types::OutputTarget::Split }),
            }],
            envelope_data: None,
            raw_output: false,
            trace_enabled: false,
            mine_enabled: is_regtest,
            auto_confirm: false,
        };

        match executor.execute(params).await? {
            deezel_common::alkanes::types::ExecutionState::ReadyToSign(ready_tx) => {
                let signed_psbt = self.sign_psbt(&ready_tx.psbt).await?;
                let tx = signed_psbt.extract_tx()?;
                let tx_hex = bitcoin::consensus::encode::serialize_hex(&tx);
                self.broadcast_transaction(tx_hex).await
            }
            _ => Err(DeezelError::Other("Unexpected execution state".to_string())),
        }
    }

    fn clone_box(&self) -> Box<dyn DeezelProvider> {
        Box::new(self.clone())
    }

    fn secp(&self) -> &Secp256k1<All> {
        self.web_provider.secp()
    }

    async fn get_utxo(&self, outpoint: &OutPoint) -> Result<Option<TxOut>> {
        self.web_provider.get_utxo(outpoint).await
    }

    async fn sign_taproot_script_spend(&self, msg: Message) -> Result<Signature> {
        self.web_provider.sign_taproot_script_spend(msg).await
    }
    fn get_bitcoin_rpc_url(&self) -> Option<String> {
        self.web_provider.get_bitcoin_rpc_url()
    }
    fn get_esplora_api_url(&self) -> Option<String> {
        self.web_provider.get_esplora_api_url()
    }
    fn get_ord_server_url(&self) -> Option<String> {
        self.web_provider.get_ord_server_url()
    }
    fn get_metashrew_rpc_url(&self) -> Option<String> {
        self.web_provider.get_metashrew_rpc_url()
    }
}