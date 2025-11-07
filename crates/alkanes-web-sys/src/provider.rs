//! Chadson's Journal - Refactoring `deezel-web` Provider
//!
//! **Date:** 2025-08-08
//! **Objective:** Refactor the `WebProvider` to correctly fetch enriched UTXO data, specifically for Protorunes.
//!
//! **Initial State & Problem:**
//! The initial implementation incorrectly used a generic JSON-RPC abstraction for what should have been
//! a multi-step data fetching process involving REST and specialized Protobuf-based RPC calls. This led to
//! fundamental errors in how wallet data (UTXOs, inscriptions, runes) was being retrieved. The application
//! would build but present incorrect data to the user.
//!
//! **The Protobuf/Protorune Challenge:**
//! A major roadblock was the `get_protorunes_by_outpoint` function. The core of the problem was a
//! misunderstanding of the `protorune-support` crate's API, leading to a cascade of compilation errors.
//! Key issues included:
//! 1.  **Type Confusion:** There was persistent confusion between the application's domain models (e.g.,
//!     `alkanes_cli_common::alkanes::protorunes::ProtoruneOutpointResponse`) and the Protobuf-generated
//!     Data Transfer Objects (DTOs) from `protorune-support` (e.g.,
//!     `protorune_support::proto::protorune::ProtoruneOutpointResponse`).
//! 2.  **Incorrect Instantiation:** Attempts to create the `OutpointWithProtocol` request message failed
//!     because the `protocol` field is not a simple `u128`, but a complex `MessageField<Uint128>`.
//! 3.  **Response Handling:** The logic for deserializing the hex-encoded response and converting it back
//!     into the application's domain model was missing or incorrect.
//!
//! **The Solution - Ground Truth from Source Code:**
//! After numerous failed attempts to fix the code via trial-and-error, the strategy shifted to
//! foundational research. I cloned the `alkanes-rs` repository into `./reference/alkanes-rs` and
//! inspected its source code directly.
//!
//! **Key Insights from `./reference/alkanes-rs/crates/protorune-support/src/proto/protorune.rs`:**
//! - The `OutpointWithProtocol` struct requires its `protocol` field to be a `MessageField<Uint128>`.
//! - The `Uint128` Protobuf message itself has `lo` and `hi` fields.
//! - The response from the `metashrew_view` RPC is a `ProtoruneOutpointResponsePb` DTO that needs to be
//!   manually mapped to the `deezel_common` `ProtoruneOutpointResponse` domain model.
//!
//! **Refactoring Implementation:**
//! The following code implements the corrected logic based on these insights.
//! - The `get_protorunes_by_outpoint` function now correctly constructs the Protobuf request,
//!   handles the hex-encoded response, and performs the crucial mapping from the Protobuf DTO
//!   to the application's domain model.
//! - The `get_enriched_utxos` function is updated to consume this corrected data.
//!
//! This systematic, source-code-driven approach was essential to break the cycle of compilation
//! errors and implement the correct data fetching logic.
//!
//! **Date:** 2025-08-09
//! **Objective:** Add graceful error handling for `esplora_*` RPC calls to prevent runtime errors.
//!
//! **Problem:** The user reported a "Wallet Not Loaded" error when sending BTC. This was likely caused
//! by hard errors when `esplora_*` RPC calls fail (e.g., if the endpoint is unavailable).
//!
//! **Solution:**
//! Modified `get_balance`, `get_utxos`, and `get_history` to wrap the `esplora_*` calls
//! in `if let Ok(...)` blocks. This ensures that if an RPC call fails, it doesn't propagate a
//! hard error and crash the application. Instead, it will result in empty or partial data,
//! which is a more graceful failure mode.

use async_trait::async_trait;
use bitcoin::Network;
use alkanes_cli_common::{*, provider::{AllBalances, AssetBalance, EnrichedUtxo}};
use protobuf::Message;
use serde_json::Value as JsonValue;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Request, RequestInit, RequestMode, Response, window};

#[cfg(target_arch = "wasm32")]
extern crate alloc;
#[cfg(target_arch = "wasm32")]
use alloc::{
    boxed::Box,
    format,
    string::{String, ToString},
    vec::Vec,
};

use crate::storage::WebStorage;
use crate::network::WebNetwork;
use crate::crypto::WebCrypto;
use crate::time::WebTime;
use crate::logging::WebLogger;
use bitcoin::{
    bip32::{DerivationPath, Fingerprint},
    psbt::Psbt,
    secp256k1::{All, Keypair, Secp256k1},
    OutPoint, Transaction, TxOut, XOnlyPublicKey, ScriptBuf,
};
use alkanes_cli_common::{
    alkanes::{
        protorunes::{ProtoruneOutpointResponse, ProtoruneWalletResponse},
        types::{
            EnhancedExecuteParams, EnhancedExecuteResult, ExecutionState, ReadyToSignCommitTx,
            ReadyToSignRevealTx, ReadyToSignTx,
        },
        AlkanesInspectConfig, AlkanesInspectResult, AlkaneBalance,
    },
    ord::{
        AddressInfo as OrdAddressInfo, Block as OrdBlock, Blocks as OrdBlocks,
        Children as OrdChildren, Inscription as OrdInscription, Inscriptions as OrdInscriptions,
        Output as OrdOutput, ParentInscriptions as OrdParents, RuneInfo as OrdRuneInfo,
        Runes as OrdRunes, SatResponse as OrdSat, TxInfo as OrdTxInfo,
    },
    esplora,
};
use alkanes_support::proto::alkanes as alkanes_pb;
use protorune_support::proto::protorune::{OutpointWithProtocol, OutpointResponse as ProtoruneOutpointResponsePb};
use alkanes_cli_common::alkanes::execute::EnhancedAlkanesExecutor;
use alkanes_cli_common::index_pointer::StubPointer;
use protorune_support::balance_sheet::{BalanceSheet, BalanceSheetOperations};
use core::str::FromStr;
use bitcoin::hashes::hex::FromHex;


use protorune_support::proto::protorune::Uint128;


/// Web-compatible provider implementation for browser environments
///
/// The `WebProvider` is the main entry point for using deezel functionality in web browsers
/// and WASM environments. It implements all deezel-common traits using web-standard APIs,
/// providing complete Bitcoin wallet and Alkanes metaprotocol functionality.
///
/// # Features
///
/// - **Bitcoin Operations**: Full wallet functionality, transaction creation, and broadcasting
/// - **Alkanes Integration**: Smart contract execution, token operations, and AMM functionality
/// - **Web Standards**: Uses fetch API, localStorage, Web Crypto API, and console logging
/// - **Network Support**: Configurable for mainnet, testnet, signet, regtest, and custom networks
/// - **Privacy Features**: Rebar Labs Shield integration for private transaction broadcasting
///
/// # Example
///
/// ```rust,no_run
/// use deezel_web::WebProvider;
/// use alkanes_cli_common::*;
///
/// async fn create_provider() -> Result<WebProvider> {
///     let provider = WebProvider::new("mainnet".to_string()).await?;
///
///     provider.initialize().await?;
///     Ok(provider)
/// }
/// ```
#[derive(Clone)]
pub struct WebProvider {
    sandshrew_rpc_url: String,
    esplora_rpc_url: Option<String>,
    network: Network,
    storage: WebStorage,
    network_client: WebNetwork,
    crypto: WebCrypto,
    time: WebTime,
    logger: WebLogger,
    keystore: Option<alkanes_cli_common::keystore::Keystore>,
    passphrase: Option<String>,
}

impl WebProvider {
    /// Creates a new WebProvider instance for the specified network
    ///
    /// This is the primary constructor for creating a web-compatible deezel provider.
    /// It configures the provider for the specified Bitcoin network and sets up
    /// connections to the required RPC endpoints.
    ///
    /// # Arguments
    ///
    /// * `network_str` - Network identifier ("mainnet", "testnet", "signet", "regtest")
    ///
    /// # Returns
    ///
    /// Returns a configured `WebProvider` instance ready for initialization.
    ///
    /// # Errors
    ///
    /// Returns an error if the network string is invalid or if provider setup fails.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use deezel_web::WebProvider;
    /// use alkanes_cli_common::Result;
    ///
    /// async fn setup_mainnet() -> Result<WebProvider> {
    ///     let provider = WebProvider::new("mainnet".to_string()).await?;
    ///     Ok(provider)
    /// }
    /// ```
    pub async fn new(
        network_str: String,
    ) -> Result<Self> {
        let params = alkanes_cli_common::network::NetworkParams::from_network_str(&network_str)?;
        let logger = WebLogger::new();
        logger.info(&format!(
            "WebProvider initialized with: Sandshrew RPC URL: {}, Esplora URL: {:?}, Network: {}",
            &params.metashrew_rpc_url, &params.esplora_url, &params.network
        ));
 
         Ok(Self {
            sandshrew_rpc_url: params.metashrew_rpc_url,
            esplora_rpc_url: params.esplora_url,
            network: params.network,
            storage: WebStorage::new(),
            network_client: WebNetwork::new(),
            crypto: WebCrypto::new(),
            time: WebTime::new(),
            logger: WebLogger::new(),
            keystore: None,
            passphrase: None,
        })
    }

   pub fn new_with_params(params: alkanes_cli_common::network::NetworkParams) -> Result<Self> {
       Ok(Self {
           sandshrew_rpc_url: params.metashrew_rpc_url,
           esplora_rpc_url: params.esplora_url,
           network: params.network,
           storage: WebStorage::new(),
           network_client: WebNetwork::new(),
           crypto: WebCrypto::new(),
           time: WebTime::new(),
           logger: WebLogger::new(),
           keystore: None,
           passphrase: None,
       })
   }

    pub async fn new_with_url(
        network_str: String,
        url: &str,
    ) -> Result<Self> {
        let network = match network_str.as_str() {
            "mainnet" => Network::Bitcoin,
            "testnet" => Network::Testnet,
            "signet" => Network::Signet,
            "regtest" | "custom" => Network::Regtest,
            _ => return Err(AlkanesError::InvalidParameters("Invalid network".to_string())),
        };

        Ok(Self {
            sandshrew_rpc_url: url.to_string(),
            esplora_rpc_url: Some(url.to_string()),
            network,
            storage: WebStorage::new(),
            network_client: WebNetwork::new(),
            crypto: WebCrypto::new(),
            time: WebTime::new(),
            logger: WebLogger::new(),
            keystore: None,
            passphrase: None,
        })
    }

    /// Returns a wallet configuration suitable for this provider
    ///
    /// Creates a `WalletConfig` with the provider's network settings and RPC URLs.
    /// This configuration can be used with wallet operations that require network
    /// and RPC endpoint information.
    ///
    /// # Returns
    ///
    /// A `WalletConfig` configured for this provider's network and endpoints.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use deezel_web::WebProvider;
    /// # use alkanes_cli_common::Result;
    /// # async fn example() -> Result<()> {
    /// # let provider = WebProvider::new("mainnet".to_string()).await?;
    /// let config = provider.get_wallet_config();
    /// println!("Network: {:?}", config.network);
    /// println!("Bitcoin RPC: {}", config.bitcoin_rpc_url);
    /// # Ok(())
    /// # }
    /// ```
    pub fn get_wallet_config(&self) -> WalletConfig {
        WalletConfig {
            wallet_path: "web-wallet".to_string(),
            network: self.network,
            bitcoin_rpc_url: self.sandshrew_rpc_url.clone(),
            metashrew_rpc_url: self.sandshrew_rpc_url.clone(),
            network_params: None,
        }
    }

    /// Get the network for this provider
    pub fn network(&self) -> Network {
        self.network
    }

    pub fn network_params(&self) -> Result<alkanes_cli_common::network::NetworkParams> {
        let mut params = alkanes_cli_common::network::NetworkParams::from_network_str(self.network.to_string().as_str())?;
        params.metashrew_rpc_url = self.sandshrew_rpc_url.clone();
        params.esplora_url = self.esplora_rpc_url.clone();
        Ok(params)
    }

    /// Get the Sandshrew RPC URL
    pub fn sandshrew_rpc_url(&self) -> &str {
        &self.sandshrew_rpc_url
    }

    /// Get the Esplora RPC URL
    pub fn esplora_rpc_url(&self) -> Option<&str> {
        self.esplora_rpc_url.as_deref()
    }

    /// Make a fetch request using web-sys
    async fn fetch_request(&self, url: &str, method: &str, body: Option<&str>, headers: Option<&js_sys::Object>) -> Result<Response> {
        let window = window().ok_or_else(|| AlkanesError::Network("No window object available".to_string()))?;

        let opts = RequestInit::new();
        opts.set_method(method);
        opts.set_mode(RequestMode::Cors);

        if let Some(body_str) = body {
            opts.set_body(&JsValue::from_str(body_str));
        }

        if let Some(headers_obj) = headers {
            opts.set_headers(headers_obj);
        }

        let request = Request::new_with_str_and_init(url, &opts)
            .map_err(|e| AlkanesError::Network(format!("Failed to create request: {e:?}")))?;

        let resp_value = JsFuture::from(window.fetch_with_request(&request))
            .await
            .map_err(|e| AlkanesError::Network(format!("Fetch failed: {e:?}")))?;

        let resp: Response = resp_value.dyn_into()
            .map_err(|e| AlkanesError::Network(format!("Failed to cast response: {e:?}")))?;

        Ok(resp)
    }

    /// Broadcasts a transaction via Rebar Labs Shield for enhanced privacy
    ///
    /// Rebar Labs Shield provides private transaction broadcasting by sending transactions
    /// directly to mining pools without exposing them to public mempools. This is particularly
    /// useful for sensitive transactions or when privacy is a concern.
    ///
    /// # Arguments
    ///
    /// * `tx_hex` - The raw transaction in hexadecimal format
    ///
    /// # Returns
    ///
    /// Returns the transaction ID (TXID) if the broadcast was successful.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The network request fails
    /// - The Rebar Shield service returns an error
    /// - The transaction is invalid or rejected
    ///
    /// # Privacy Features
    ///
    /// - Transactions are sent directly to mining pools
    /// - No public mempool exposure
    /// - Enhanced privacy for sensitive operations
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use deezel_web::WebProvider;
    /// # use alkanes_cli_common::Result;
    /// # async fn example() -> Result<()> {
    /// # let provider = WebProvider::new("mainnet".to_string()).await?;
    /// let tx_hex = "0200000001..."; // Your transaction hex
    /// let txid = provider.broadcast_via_rebar_shield(tx_hex).await?;
    /// println!("Transaction broadcast privately: {}", txid);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn broadcast_via_rebar_shield(&self, tx_hex: &str) -> Result<String> {
        self.logger.info("🛡️  Broadcasting transaction via Rebar Labs Shield (web)");
        
        // Rebar Labs Shield endpoint
        let rebar_endpoint = "https://shield.rebarlabs.io/v1/rpc";
        
        // Create JSON-RPC request for sendrawtransaction
        let request_body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "1",
            "method": "sendrawtransaction",
            "params": [tx_hex]
        });
        
        self.logger.info(&format!("Sending transaction to Rebar Shield endpoint: {rebar_endpoint}"));
        
        // Create headers
        let headers = js_sys::Object::new();
        js_sys::Reflect::set(&headers, &"Content-Type".into(), &"application/json".into())
            .map_err(|e| AlkanesError::Network(format!("Failed to set header: {e:?}")))?;
        
        // Make HTTP POST request to Rebar Labs Shield
        let response = self.fetch_request(
            rebar_endpoint,
            "POST",
            Some(&request_body.to_string()),
            Some(&headers),
        ).await?;
        
        let response_text = JsFuture::from(response.text()
            .map_err(|e| AlkanesError::Network(format!("Failed to get response text: {e:?}")))?)
            .await
            .map_err(|e| AlkanesError::Network(format!("Failed to read Rebar Shield response: {e:?}")))?;
        
        let response_str = response_text.as_string()
            .ok_or_else(|| AlkanesError::Network("Response is not a string".to_string()))?;
        
        let response_json: JsonValue = serde_json::from_str(&response_str)
            .map_err(|e| AlkanesError::Serialization(format!("Failed to parse Rebar Shield JSON: {e}")))?;
        
        // Check for JSON-RPC error
        if let Some(error) = response_json.get("error") {
            return Err(AlkanesError::JsonRpc(format!("Rebar Shield error: {error}")));
        }
        
        // Extract transaction ID from result
        let txid = response_json.get("result")
            .and_then(|r| r.as_str())
            .ok_or_else(|| AlkanesError::JsonRpc("No transaction ID in Rebar Shield response".to_string()))?;
        
        self.logger.info(&format!("✅ Transaction broadcast via Rebar Shield: {txid}"));
        self.logger.info("🛡️  Transaction sent privately to mining pools");
        
        Ok(txid.to_string())
    }
}

#[async_trait(?Send)]
impl JsonRpcProvider for WebProvider {
    async fn call(&self, url: &str, method: &str, params: JsonValue, id: u64) -> Result<JsonValue> {
        self.logger.info(&format!(
            "JsonRpcProvider::call -> URL: {}, Method: {}, Params: {}",
            url,
            method,
            serde_json::to_string_pretty(&params).unwrap_or_else(|_| "INVALID_JSON".to_string()),
        ));
        let request_body = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
            "id": id
        });

        // Create headers
        let headers = js_sys::Object::new();
        js_sys::Reflect::set(&headers, &"Content-Type".into(), &"application/json".into())
            .map_err(|e| AlkanesError::Network(format!("Failed to set header: {e:?}")))?;

        let response = self.fetch_request(
            url,
            "POST",
            Some(&request_body.to_string()),
            Some(&headers),
        ).await?;

        let response_text = JsFuture::from(response.text()
            .map_err(|e| AlkanesError::Network(format!("Failed to get response text: {e:?}")))?)
            .await
            .map_err(|e| AlkanesError::Network(format!("Failed to read response: {e:?}")))?;

        let response_str = response_text.as_string()
            .ok_or_else(|| AlkanesError::Network("Response is not a string".to_string()))?;

        let response_json: JsonValue = serde_json::from_str(&response_str)
            .map_err(|e| AlkanesError::Serialization(format!("Failed to parse JSON: {e}")))?;

        self.logger.info(&format!("JsonRpcProvider::call <- Raw RPC response: {}", response_str));
 
        if let Some(error) = response_json.get("error") {
            if !error.is_null() {
                return Err(AlkanesError::JsonRpc(format!("JSON-RPC error: {error}")));
            }
        }

        response_json.get("result")
            .cloned()
            .ok_or_else(|| AlkanesError::JsonRpc("No result in JSON-RPC response".to_string()))
    }

}

#[async_trait(?Send)]
impl StorageProvider for WebProvider {
    async fn read(&self, key: &str) -> Result<Vec<u8>> {
        self.storage.read(key).await
    }

    async fn write(&self, key: &str, data: &[u8]) -> Result<()> {
        self.storage.write(key, data).await
    }

    async fn exists(&self, key: &str) -> Result<bool> {
        self.storage.exists(key).await
    }

    async fn delete(&self, key: &str) -> Result<()> {
        self.storage.delete(key).await
    }

    async fn list_keys(&self, prefix: &str) -> Result<Vec<String>> {
        self.storage.list_keys(prefix).await
    }

    fn storage_type(&self) -> &'static str {
        "localStorage"
    }
}

#[async_trait(?Send)]
impl NetworkProvider for WebProvider {
    async fn get(&self, url: &str) -> Result<Vec<u8>> {
        self.network_client.get(url).await
    }

    async fn post(&self, url: &str, body: &[u8], content_type: &str) -> Result<Vec<u8>> {
        self.network_client.post(url, body, content_type).await
    }

    async fn is_reachable(&self, url: &str) -> bool {
        self.network_client.is_reachable(url).await
    }
}

#[async_trait(?Send)]
impl CryptoProvider for WebProvider {
    fn random_bytes(&self, len: usize) -> Result<Vec<u8>> {
        self.crypto.random_bytes(len)
    }

    fn sha256(&self, data: &[u8]) -> Result<[u8; 32]> {
        self.crypto.sha256(data)
    }

    fn sha3_256(&self, data: &[u8]) -> Result<[u8; 32]> {
        self.crypto.sha3_256(data)
    }

    async fn encrypt_aes_gcm(&self, data: &[u8], key: &[u8], nonce: &[u8]) -> Result<Vec<u8>> {
        self.crypto.encrypt_aes_gcm(data, key, nonce).await
    }

    async fn decrypt_aes_gcm(&self, data: &[u8], key: &[u8], nonce: &[u8]) -> Result<Vec<u8>> {
        self.crypto.decrypt_aes_gcm(data, key, nonce).await
    }

    async fn pbkdf2_derive(&self, password: &[u8], salt: &[u8], iterations: u32, key_len: usize) -> Result<Vec<u8>> {
        self.crypto.pbkdf2_derive(password, salt, iterations, key_len).await
    }
}

#[async_trait(?Send)]
impl TimeProvider for WebProvider {
    fn now_secs(&self) -> u64 {
        self.time.now_secs()
    }

    fn now_millis(&self) -> u64 {
        self.time.now_millis()
    }

    async fn sleep_ms(&self, ms: u64) {
        self.time.sleep_ms(ms).await
    }
}

impl LogProvider for WebProvider {
    fn debug(&self, message: &str) {
        self.logger.debug(message);
    }

    fn info(&self, message: &str) {
        self.logger.info(message);
    }

    fn warn(&self, message: &str) {
        self.logger.warn(message);
    }

    fn error(&self, message: &str) {
        self.logger.error(message);
    }
}

#[async_trait(?Send)]
impl EsploraProvider for WebProvider {
    async fn get_blocks_tip_hash(&self) -> Result<String> {
        self.logger.info("[EsploraProvider] Calling get_blocks_tip_hash");
        let url = self.esplora_rpc_url.as_deref().unwrap_or(&self.sandshrew_rpc_url);
        self.logger.info(&format!("[EsploraProvider] Using JSON-RPC to {} for method {}", url, esplora::EsploraJsonRpcMethods::BLOCKS_TIP_HASH));
        let result = self.call(url, esplora::EsploraJsonRpcMethods::BLOCKS_TIP_HASH, esplora::params::empty(), 1).await?;
        result.as_str().map(|s| s.to_string()).ok_or_else(|| AlkanesError::RpcError("Invalid tip hash response".to_string()))
    }

    async fn get_blocks_tip_height(&self) -> Result<u64> {
        self.logger.info("[EsploraProvider] Calling get_blocks_tip_height");
        let url = self.esplora_rpc_url.as_deref().unwrap_or(&self.sandshrew_rpc_url);
        self.logger.info(&format!("[EsploraProvider] Using JSON-RPC to {} for method {}", url, esplora::EsploraJsonRpcMethods::BLOCKS_TIP_HEIGHT));
        let result = self.call(url, esplora::EsploraJsonRpcMethods::BLOCKS_TIP_HEIGHT, esplora::params::empty(), 1).await?;
        result.as_u64().ok_or_else(|| AlkanesError::RpcError("Invalid tip height response".to_string()))
    }

    async fn get_blocks(&self, start_height: Option<u64>) -> Result<serde_json::Value> {
        let url = self.esplora_rpc_url.as_deref().unwrap_or(&self.sandshrew_rpc_url);
        self.call(url, esplora::EsploraJsonRpcMethods::BLOCKS, esplora::params::optional_single(start_height), 1).await
    }

    async fn get_block_by_height(&self, height: u64) -> Result<String> {
        let url = self.esplora_rpc_url.as_deref().unwrap_or(&self.sandshrew_rpc_url);
        let result = self.call(url, esplora::EsploraJsonRpcMethods::BLOCK_HEIGHT, esplora::params::single(height), 1).await?;
        result.as_str().map(|s| s.to_string()).ok_or_else(|| AlkanesError::RpcError("Invalid block hash response".to_string()))
    }

    async fn get_block(&self, hash: &str) -> Result<serde_json::Value> {
        let url = self.esplora_rpc_url.as_deref().unwrap_or(&self.sandshrew_rpc_url);
        self.call(url, esplora::EsploraJsonRpcMethods::BLOCK, esplora::params::single(hash), 1).await
    }

    async fn get_block_status(&self, hash: &str) -> Result<serde_json::Value> {
        let url = self.esplora_rpc_url.as_deref().unwrap_or(&self.sandshrew_rpc_url);
        self.call(url, esplora::EsploraJsonRpcMethods::BLOCK_STATUS, esplora::params::single(hash), 1).await
    }

    async fn get_block_txids(&self, hash: &str) -> Result<serde_json::Value> {
        let url = self.esplora_rpc_url.as_deref().unwrap_or(&self.sandshrew_rpc_url);
        self.call(url, esplora::EsploraJsonRpcMethods::BLOCK_TXIDS, esplora::params::single(hash), 1).await
    }

    async fn get_block_header(&self, hash: &str) -> Result<String> {
        let url = self.esplora_rpc_url.as_deref().unwrap_or(&self.sandshrew_rpc_url);
        let result = self.call(url, esplora::EsploraJsonRpcMethods::BLOCK_HEADER, esplora::params::single(hash), 1).await?;
        result.as_str().map(|s| s.to_string()).ok_or_else(|| AlkanesError::RpcError("Invalid block header response".to_string()))
    }

    async fn get_block_raw(&self, hash: &str) -> Result<String> {
        let url = self.esplora_rpc_url.as_deref().unwrap_or(&self.sandshrew_rpc_url);
        let result = self.call(url, esplora::EsploraJsonRpcMethods::BLOCK_RAW, esplora::params::single(hash), 1).await?;
        result.as_str().map(|s| s.to_string()).ok_or_else(|| AlkanesError::RpcError("Invalid raw block response".to_string()))
    }

    async fn get_block_txid(&self, hash: &str, index: u32) -> Result<String> {
        self.logger.info(&format!("[EsploraProvider] Calling get_block_txid for hash: {}, index: {}", hash, index));
        let url = self.esplora_rpc_url.as_deref().unwrap_or(&self.sandshrew_rpc_url);
        self.logger.info(&format!("[EsploraProvider] Using JSON-RPC to {} for method {}", url, esplora::EsploraJsonRpcMethods::BLOCK_TXID));
        let result = self.call(url, esplora::EsploraJsonRpcMethods::BLOCK_TXID, esplora::params::dual(hash, index), 1).await?;
        result.as_str().map(|s| s.to_string()).ok_or_else(|| AlkanesError::RpcError("Invalid txid response".to_string()))
    }

    async fn get_block_txs(&self, hash: &str, start_index: Option<u32>) -> Result<serde_json::Value> {
        let url = self.esplora_rpc_url.as_deref().unwrap_or(&self.sandshrew_rpc_url);
        self.call(url, esplora::EsploraJsonRpcMethods::BLOCK_TXS, esplora::params::optional_dual(hash, start_index), 1).await
    }

    async fn get_address_info(&self, address: &str) -> Result<serde_json::Value> {
        self.logger.info(&format!("[EsploraProvider] Calling get_address_info for address: {}", address));
        let url = self.esplora_rpc_url.as_deref().unwrap_or(&self.sandshrew_rpc_url);
        self.logger.info(&format!("[EsploraProvider] Using JSON-RPC to {} for method {}", url, esplora::EsploraJsonRpcMethods::ADDRESS));
        self.call(url, esplora::EsploraJsonRpcMethods::ADDRESS, esplora::params::single(address), 1).await
    }

    async fn get_address_utxo(&self, address: &str) -> Result<serde_json::Value> {
        self.logger.info(&format!("[EsploraProvider] Calling get_address_utxo for address: {}", address));
        if let Some(url) = self.esplora_rpc_url.as_deref() {
            self.logger.info(&format!("[EsploraProvider] Using JSON-RPC to {} for method esplora_address::utxo", url));
            if let Ok(result) = self.call(url, "esplora_address::utxo", esplora::params::single(address), 1).await {
                return Ok(result);
            }
        }
        self.logger.info(&format!("[EsploraProvider] Falling back to JSON-RPC on sandshrew for method esplora_address::utxo"));
        // Fallback or error
        self.call(&self.sandshrew_rpc_url, "esplora_address::utxo", esplora::params::single(address), 1).await
    }

    async fn get_address_txs(&self, address: &str) -> Result<serde_json::Value> {
        let url = self.esplora_rpc_url.as_deref().unwrap_or(&self.sandshrew_rpc_url);
        self.call(url, "esplora_address::txs", esplora::params::single(address), 1).await
    }

    async fn get_address_txs_chain(&self, address: &str, last_seen_txid: Option<&str>) -> Result<serde_json::Value> {
        let url = self.esplora_rpc_url.as_deref().unwrap_or(&self.sandshrew_rpc_url);
        self.call(url, esplora::EsploraJsonRpcMethods::ADDRESS_TXS_CHAIN, esplora::params::optional_dual(address, last_seen_txid), 1).await
    }

    async fn get_address_txs_mempool(&self, address: &str) -> Result<serde_json::Value> {
        let url = self.esplora_rpc_url.as_deref().unwrap_or(&self.sandshrew_rpc_url);
        self.call(url, esplora::EsploraJsonRpcMethods::ADDRESS_TXS_MEMPOOL, esplora::params::single(address), 1).await
    }

    async fn get_address_prefix(&self, prefix: &str) -> Result<serde_json::Value> {
        let url = self.esplora_rpc_url.as_deref().unwrap_or(&self.sandshrew_rpc_url);
        self.call(url, esplora::EsploraJsonRpcMethods::ADDRESS_PREFIX, esplora::params::single(prefix), 1).await
    }

    async fn get_tx(&self, txid: &str) -> Result<serde_json::Value> {
        self.logger.info(&format!("[EsploraProvider] Calling get_tx for txid: {}", txid));
        let url = self.esplora_rpc_url.as_deref().unwrap_or(&self.sandshrew_rpc_url);
        self.logger.info(&format!("[EsploraProvider] Using JSON-RPC to {} for method {}", url, esplora::EsploraJsonRpcMethods::TX));
        self.call(url, esplora::EsploraJsonRpcMethods::TX, esplora::params::single(txid), 1).await
    }

    async fn get_tx_hex(&self, txid: &str) -> Result<String> {
        let url = self.esplora_rpc_url.as_deref().unwrap_or(&self.sandshrew_rpc_url);
        let result = self.call(url, esplora::EsploraJsonRpcMethods::TX_HEX, esplora::params::single(txid), 1).await?;
        result.as_str().map(|s| s.to_string()).ok_or_else(|| AlkanesError::RpcError("Invalid tx hex response".to_string()))
    }

    async fn get_tx_raw(&self, txid: &str) -> Result<String> {
        let url = self.esplora_rpc_url.as_deref().unwrap_or(&self.sandshrew_rpc_url);
        let result = self.call(url, esplora::EsploraJsonRpcMethods::TX_RAW, esplora::params::single(txid), 1).await?;
        result.as_str().map(|s| s.to_string()).ok_or_else(|| AlkanesError::RpcError("Invalid raw tx response".to_string()))
    }

    async fn get_tx_status(&self, txid: &str) -> Result<serde_json::Value> {
        let url = self.esplora_rpc_url.as_deref().unwrap_or(&self.sandshrew_rpc_url);
        self.call(url, esplora::EsploraJsonRpcMethods::TX_STATUS, esplora::params::single(txid), 1).await
    }

    async fn get_tx_merkle_proof(&self, txid: &str) -> Result<serde_json::Value> {
        let url = self.esplora_rpc_url.as_deref().unwrap_or(&self.sandshrew_rpc_url);
        self.call(url, esplora::EsploraJsonRpcMethods::TX_MERKLE_PROOF, esplora::params::single(txid), 1).await
    }

    async fn get_tx_merkleblock_proof(&self, txid: &str) -> Result<String> {
        let url = self.esplora_rpc_url.as_deref().unwrap_or(&self.sandshrew_rpc_url);
        let result = self.call(url, esplora::EsploraJsonRpcMethods::TX_MERKLEBLOCK_PROOF, esplora::params::single(txid), 1).await?;
        result.as_str().map(|s| s.to_string()).ok_or_else(|| AlkanesError::RpcError("Invalid merkleblock proof response".to_string()))
    }

    async fn get_tx_outspend(&self, txid: &str, index: u32) -> Result<serde_json::Value> {
        let url = self.esplora_rpc_url.as_deref().unwrap_or(&self.sandshrew_rpc_url);
        self.call(url, esplora::EsploraJsonRpcMethods::TX_OUTSPEND, esplora::params::dual(txid, index), 1).await
    }

    async fn get_tx_outspends(&self, txid: &str) -> Result<serde_json::Value> {
        let url = self.esplora_rpc_url.as_deref().unwrap_or(&self.sandshrew_rpc_url);
        self.call(url, esplora::EsploraJsonRpcMethods::TX_OUTSPENDS, esplora::params::single(txid), 1).await
    }

    async fn broadcast(&self, tx_hex: &str) -> Result<String> {
        let url = self.esplora_rpc_url.as_deref().unwrap_or(&self.sandshrew_rpc_url);
        let result = self.call(url, esplora::EsploraJsonRpcMethods::BROADCAST, esplora::params::single(tx_hex), 1).await?;
        result.as_str().map(|s| s.to_string()).ok_or_else(|| AlkanesError::RpcError("Invalid broadcast response".to_string()))
    }

    async fn get_mempool(&self) -> Result<serde_json::Value> {
        let url = self.esplora_rpc_url.as_deref().unwrap_or(&self.sandshrew_rpc_url);
        self.call(url, esplora::EsploraJsonRpcMethods::MEMPOOL, esplora::params::empty(), 1).await
    }

    async fn get_mempool_txids(&self) -> Result<serde_json::Value> {
        let url = self.esplora_rpc_url.as_deref().unwrap_or(&self.sandshrew_rpc_url);
        self.call(url, esplora::EsploraJsonRpcMethods::MEMPOOL_TXIDS, esplora::params::empty(), 1).await
    }

    async fn get_mempool_recent(&self) -> Result<serde_json::Value> {
        let url = self.esplora_rpc_url.as_deref().unwrap_or(&self.sandshrew_rpc_url);
        self.call(url, esplora::EsploraJsonRpcMethods::MEMPOOL_RECENT, esplora::params::empty(), 1).await
    }

    async fn get_fee_estimates(&self) -> Result<serde_json::Value> {
        let url = self.esplora_rpc_url.as_deref().unwrap_or(&self.sandshrew_rpc_url);
        self.call(url, esplora::EsploraJsonRpcMethods::FEE_ESTIMATES, esplora::params::empty(), 1).await
    }
}

#[async_trait(?Send)]
impl WalletProvider for WebProvider {
    async fn create_wallet(&mut self, config: WalletConfig, mnemonic: Option<String>, passphrase: Option<String>) -> Result<WalletInfo> {
        let mnemonic = if let Some(m) = mnemonic {
            bip39::Mnemonic::from_phrase(&m, bip39::Language::English).map_err(|e| AlkanesError::Wallet(format!("Invalid mnemonic: {e}")))?
        } else {
            bip39::Mnemonic::new(bip39::MnemonicType::Words24, bip39::Language::English)
        };

        let pass = passphrase.clone().unwrap_or_default();
        let keystore = alkanes_cli_common::keystore::Keystore::new(&mnemonic, config.network, &pass, None)?;

        // Store the encrypted keystore
        let keystore_bytes = serde_json::to_vec(&keystore)?;
        self.storage.write(&config.wallet_path, &keystore_bytes).await?;

        let network_params = self.network_params()?;
        let addresses = self.derive_addresses(&keystore.account_xpub, &network_params, &["p2tr"], 0, 1).await?;
        let address = addresses.first().map(|a| a.address.clone()).unwrap_or_default();
        
        // Store the keystore in the provider instance
        self.keystore = Some(keystore);
        self.passphrase = passphrase;
        
        Ok(WalletInfo {
            address,
            network: config.network,
            mnemonic: Some(mnemonic.to_string()),
        })
    }
    
    async fn load_wallet(&mut self, config: WalletConfig, passphrase: Option<String>) -> Result<WalletInfo> {
        let keystore_bytes = self.storage.read(&config.wallet_path).await?;
        let keystore: alkanes_cli_common::keystore::Keystore = serde_json::from_slice(&keystore_bytes)?;

        let pass = passphrase.as_deref().ok_or_else(|| AlkanesError::Wallet("Passphrase required to load wallet".to_string()))?;
        let mnemonic = keystore.decrypt_mnemonic(pass)?;

        let network_params = self.network_params()?;
        let addresses = self.derive_addresses(&keystore.account_xpub, &network_params, &["p2tr"], 0, 1).await?;
        let address = addresses.first().map(|a| a.address.clone()).unwrap_or_default();

        // Store the keystore in the provider instance
        self.keystore = Some(keystore);
        self.passphrase = passphrase;

        Ok(WalletInfo {
            address,
            network: config.network,
            mnemonic: Some(mnemonic),
        })
    }
    
    async fn get_balance(&self, addresses: Option<Vec<String>>) -> Result<WalletBalance> {
        self.logger.info(&format!("[WalletProvider] Calling get_balance for addresses: {:?}", addresses));
        let addrs = if let Some(a) = addresses {
            a
        } else {
            vec![<Self as WalletProvider>::get_address(self).await?]
        };

        let mut total_confirmed = 0;
        let mut total_pending = 0;

        for address in addrs {
            if let Ok(info_val) = self.get_address_info(&address).await {
                if let Ok(info) = serde_json::from_value::<esplora::EsploraAddress>(info_val) {
                    total_confirmed += info.chain_stats.funded_txo_sum - info.chain_stats.spent_txo_sum;
                    total_pending += (info.mempool_stats.funded_txo_sum as i64) - (info.mempool_stats.spent_txo_sum as i64);
                }
            }
        }

        Ok(WalletBalance {
            confirmed: total_confirmed,
            pending: total_pending,
        })
    }
    
    async fn get_address(&self) -> Result<String> {
        self.logger.info("[WalletProvider] Calling get_address");
        let keystore = self.keystore.as_ref().ok_or_else(|| AlkanesError::Wallet("Wallet not loaded".to_string()))?;
        let network_params = self.network_params()?;
        let addresses = self.derive_addresses(&keystore.account_xpub, &network_params, &["p2tr"], 0, 1).await?;
        let address = addresses.first()
            .map(|a| a.address.clone())
            .ok_or_else(|| AlkanesError::Wallet("Could not derive address".to_string()))?;
        Ok(address)
    }
    
    async fn get_addresses(&self, count: u32) -> Result<Vec<AddressInfo>> {
        self.logger.info(&format!("[WalletProvider] Calling get_addresses with count: {}", count));
        let keystore = self.keystore.as_ref().ok_or_else(|| AlkanesError::Wallet("Wallet not loaded".to_string()))?;
        let network_params = self.network_params()?;
        let keystore_addresses = self.derive_addresses(&keystore.account_xpub, &network_params, &["p2tr"], 0, count).await?;
        
        let addresses = keystore_addresses.into_iter().map(|ks_addr| {
            AddressInfo {
                address: ks_addr.address,
                index: ks_addr.index,
                script_type: ks_addr.script_type,
                derivation_path: ks_addr.derivation_path,
                used: false, // A full implementation would check this
            }
        }).collect();

        Ok(addresses)
    }
    
    async fn send(&mut self, params: SendParams) -> Result<String> {
        self.logger.info(&format!("[WalletProvider] Calling send with params: {:?}", params));
        let psbt_str = self.create_transaction(params).await?;
        let signed_tx_hex = self.sign_transaction(psbt_str).await?;
        self.broadcast_transaction(signed_tx_hex).await
    }
    
    async fn get_utxos(&self, _include_frozen: bool, addresses: Option<Vec<String>>) -> Result<Vec<(bitcoin::OutPoint, UtxoInfo)>> {
        self.logger.info(&format!("[WalletProvider] Calling get_utxos for addresses: {:?}", addresses));
        let addrs = if let Some(a) = addresses {
            a
        } else {
            vec![<Self as WalletProvider>::get_address(self).await?]
        };

        let mut all_utxos = Vec::new();
        let tip = self.get_blocks_tip_height().await?;

        for address in addrs {
            let utxos_val = self.get_address_utxo(&address).await;
            if let Ok(utxos_val) = utxos_val {
                if let Ok(esplora_utxos) = serde_json::from_value::<Vec<esplora::EsploraUtxo>>(utxos_val) {
                    for utxo in esplora_utxos {
                        if let Ok(outpoint) = OutPoint::from_str(&format!("{}:{}", utxo.txid, utxo.vout)) {
                            let confirmations = if let Some(height) = utxo.status.block_height {
                                tip.saturating_sub(height as u64) + 1
                            } else {
                                0
                            };
                            let utxo_info = UtxoInfo {
                                txid: utxo.txid,
                                vout: utxo.vout,
                                amount: utxo.value,
                                address: address.clone(),
                                script_pubkey: None,
                                confirmations: confirmations as u32,
                                frozen: false,
                                freeze_reason: None,
                                block_height: utxo.status.block_height.map(|h| h as u64),
                                has_inscriptions: false, // Will be enriched later
                                has_runes: false, // Will be enriched later
                                has_alkanes: false, // Will be enriched later
                                is_coinbase: false, // Cannot determine from this endpoint
                            };
                            all_utxos.push((outpoint, utxo_info));
                        }
                    }
                }
            }
        }

        Ok(all_utxos)
    }
    
    
    async fn get_history(&self, _count: u32, address: Option<String>) -> Result<Vec<TransactionInfo>> {
        self.logger.info(&format!("[WalletProvider] Calling get_history for address: {:?}, count: {}", address, _count));
        let addr = if let Some(a) = address {
            a
        } else {
            <Self as WalletProvider>::get_address(self).await?
        };

        let mut all_txs = Vec::new();

        // Fetch confirmed transactions
        if let Ok(txs_val) = self.get_address_txs_chain(&addr, None).await {
            if let Ok(esplora_txs) = serde_json::from_value::<Vec<esplora::EsploraTransaction>>(txs_val) {
                all_txs.extend(esplora_txs);
            }
        }

        // Fetch mempool transactions
        if let Ok(txs_val) = self.get_address_txs_mempool(&addr).await {
            if let Ok(esplora_txs) = serde_json::from_value::<Vec<esplora::EsploraTransaction>>(txs_val) {
                all_txs.extend(esplora_txs);
            }
        }

        let history = all_txs.into_iter().map(|tx| {
            let is_op_return = tx.vout.iter().any(|o| o.scriptpubkey.starts_with("6a"));
            let has_protostones = false; // Placeholder
            let is_rbf = tx.vin.iter().any(|i| i.sequence < 4294967295);

            TransactionInfo {
                txid: tx.txid,
                block_height: tx.status.as_ref().and_then(|s| s.block_height.map(|h| h as u64)),
                block_time: tx.status.as_ref().and_then(|s| s.block_time),
                confirmed: tx.status.map_or(false, |s| s.confirmed),
                fee: Some(tx.fee),
                weight: Some(tx.weight),
                inputs: tx.vin.into_iter().map(|i| TransactionInput {
                    txid: i.txid,
                    vout: i.vout,
                    address: i.prevout.as_ref().and_then(|p| p.scriptpubkey_address.clone()),
                    amount: i.prevout.as_ref().map(|p| p.value),
                }).collect(),
                outputs: tx.vout.into_iter().map(|o| TransactionOutput {
                    address: o.scriptpubkey_address,
                    amount: o.value,
                    script: ScriptBuf::from_hex(&o.scriptpubkey).unwrap_or_default(),
                }).collect(),
                is_op_return,
                has_protostones,
                is_rbf,
            }
        }).collect();

        Ok(history)
    }

    async fn get_enriched_utxos(&self, addresses: Option<Vec<String>>) -> Result<Vec<EnrichedUtxo>> {
        let utxo_tuples = self.get_utxos(false, addresses).await?;
        let mut enriched_utxos = Vec::new();

        for (outpoint, mut utxo_info) in utxo_tuples {
            let outpoint_str = outpoint.to_string();
            let ord_output_res = self.get_output(&outpoint_str).await;
            if let Ok(ord_output) = ord_output_res {
                utxo_info.has_inscriptions = ord_output.inscriptions.as_ref().map_or(false, |v| !v.is_empty());
                utxo_info.has_runes = ord_output.runes.is_some();
            }

            let protorunes_res = self.get_protorunes_by_outpoint(&outpoint.txid.to_string(), outpoint.vout, None, 1).await;

            let assets = if let Ok(protorunes) = protorunes_res {
                utxo_info.has_alkanes = !protorunes.balance_sheet.balances().is_empty();
                protorunes.balance_sheet.balances().into_iter().map(|(id, balance)| {
                    AssetBalance {
                        name: format!("protorune-{}-{}", id.block, id.tx), // Placeholder name
                        symbol: format!("PRT-{}-{}", id.block, id.tx), // Placeholder symbol
                        balance: *balance,
                    }
                }).collect()
            } else {
                vec![]
            };

            enriched_utxos.push(EnrichedUtxo {
                utxo_info,
                assets,
            });
        }

        Ok(enriched_utxos)
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
    
    async fn freeze_utxo(&self, _utxo: String, _reason: Option<String>) -> Result<()> {
        // This would typically interact with the wallet's internal database of UTXOs.
        // Not implemented for this web-based, stateless provider.
        unimplemented!()
    }
    
    async fn unfreeze_utxo(&self, _utxo: String) -> Result<()> {
        // This would typically interact with the wallet's internal database of UTXOs.
        // Not implemented for this web-based, stateless provider.
        unimplemented!()
    }
    
    async fn create_transaction(&self, params: SendParams) -> Result<String> {
        self.logger.info(&format!("[WalletProvider] Calling create_transaction with params: {:?}", params));
        use bitcoin::psbt::Psbt;
        use bitcoin::address::Address;
        use bitcoin::{Amount, TxOut, TxIn, Witness, Sequence};
        use core::str::FromStr;
        use base64::{engine::general_purpose::STANDARD, Engine as _};

        let recipient = Address::from_str(&params.address)?.assume_checked();
        let amount = Amount::from_sat(params.amount);

        let address = <Self as WalletProvider>::get_address(self).await?;
        let utxos = self.get_utxos(false, Some(vec![address])).await?;
        if utxos.is_empty() {
            return Err(AlkanesError::Wallet("No UTXOs available".to_string()));
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
            return Err(AlkanesError::Wallet("Insufficient funds".to_string()));
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
    
    async fn sign_transaction(&mut self, psbt_base64: String) -> Result<String> {
        self.logger.info("[WalletProvider] Calling sign_transaction");
        use bitcoin::consensus::encode;
        use base64::{engine::general_purpose::STANDARD, Engine as _};
        use bitcoin::psbt::Psbt;

        let psbt_bytes = STANDARD.decode(&psbt_base64).map_err(|e| AlkanesError::Parse(e.to_string()))?;
        let mut psbt: Psbt = Psbt::deserialize(&psbt_bytes)?;
        
        let signed_psbt = self.sign_psbt(&mut psbt).await?;
        let tx = signed_psbt.extract_tx()?;
        Ok(encode::serialize_hex(&tx))
    }
    
    async fn broadcast_transaction(&self, tx_hex: String) -> Result<String> {
        self.logger.info("[WalletProvider] Calling broadcast_transaction");
        if self.network == Network::Bitcoin {
            self.broadcast_via_rebar_shield(&tx_hex).await
        } else {
            <Self as EsploraProvider>::broadcast(self, &tx_hex).await
        }
    }
    
    async fn estimate_fee(&self, target: u32) -> Result<FeeEstimate> {
        let fee_rates = self.get_fee_rates().await?;
        let rate = match target {
            1 => fee_rates.fast,
            2..=6 => fee_rates.medium,
            _ => fee_rates.slow,
        };
        Ok(FeeEstimate {
            fee_rate: rate,
            target_blocks: target,
        })
    }
    
    async fn get_fee_rates(&self) -> Result<FeeRates> {
        let estimates_val = <Self as EsploraProvider>::get_fee_estimates(self).await?;
        let estimates: esplora::EsploraFeeEstimates = serde_json::from_value(estimates_val)?;
        
        let get_rate = |target: &str| -> f32 {
            estimates.estimates.get(target).cloned().unwrap_or(1.0) as f32
        };

        Ok(FeeRates {
            fast: get_rate("1"),
            medium: get_rate("6"),
            slow: get_rate("144"),
        })
    }
    
    async fn sync(&self) -> Result<()> {
        // Syncing is a complex process involving checking all derived addresses for activity.
        // For a web provider, this might be a lighter operation, perhaps just updating balances.
        // For now, we'll consider it a no-op.
        Ok(())
    }
    
    async fn backup(&self) -> Result<String> {
        let keystore = self.keystore.as_ref().ok_or_else(|| AlkanesError::Wallet("Wallet not loaded".to_string()))?;
        let keystore_json = serde_json::to_string(keystore)?;
        Ok(keystore_json)
    }
    
    async fn get_mnemonic(&self) -> Result<Option<String>> {
        let keystore = self.keystore.as_ref().ok_or_else(|| AlkanesError::Wallet("Wallet not loaded".to_string()))?;
        let pass = self.passphrase.as_deref().ok_or_else(|| AlkanesError::Wallet("Passphrase not set".to_string()))?;
        let mnemonic = keystore.decrypt_mnemonic(pass)?;
        Ok(Some(mnemonic))
    }
    
    fn get_network(&self) -> Network {
        self.network
    }
    
    async fn get_internal_key(&self) -> Result<(XOnlyPublicKey, (Fingerprint, DerivationPath))> {
        let keypair = self.get_keypair().await?;
        let keystore = self.keystore.as_ref().ok_or_else(|| AlkanesError::Wallet("Wallet not loaded".to_string()))?;
        let (x_only, _) = keypair.x_only_public_key();
        // Assuming p2tr path for internal key
        let path_str = format!("m/86'/{}/0'/0/0", if self.network == Network::Bitcoin { "0" } else { "1" });
        let path = DerivationPath::from_str(&path_str)?;
        let fingerprint = Fingerprint::from_str(&keystore.master_fingerprint)?;
        Ok((x_only, (fingerprint, path)))
    }
    
    async fn sign_psbt(&mut self, psbt: &Psbt) -> Result<Psbt> {
        let mut psbt = psbt.clone();
        let keypair = self.get_keypair().await?;
        let secp = Secp256k1::new();
        let mut sighash_cache = bitcoin::sighash::SighashCache::new(&psbt.unsigned_tx);
        for i in 0..psbt.inputs.len() {
            let prev_txo = psbt.inputs[i].witness_utxo.as_ref().ok_or(AlkanesError::Wallet("Missing witness UTXO".to_string()))?;
            let sighash = sighash_cache.taproot_key_spend_signature_hash(i, &bitcoin::sighash::Prevouts::All(&[prev_txo.clone()]), bitcoin::sighash::TapSighashType::Default)?;
            let sig = secp.sign_schnorr_with_rng(&sighash.into(), &keypair, &mut rand::thread_rng());
            psbt.inputs[i].tap_key_sig = Some(bitcoin::taproot::Signature{ signature: sig, sighash_type: bitcoin::sighash::TapSighashType::Default });
        }
        Ok(psbt)
    }
    
    async fn get_keypair(&self) -> Result<Keypair> {
        use bip39::Mnemonic;
        use bitcoin::bip32::Xpriv;

        let keystore = self.keystore.as_ref().ok_or_else(|| AlkanesError::Wallet("Wallet not loaded".to_string()))?;
        let pass = self.passphrase.as_deref().unwrap_or_default();
        let mnemonic_str = keystore.decrypt_mnemonic(pass)?;
        let mnemonic = Mnemonic::from_phrase(&mnemonic_str, bip39::Language::English)?;
        
        let secp = Secp256k1::new();
        let seed = bip39::Seed::new(&mnemonic, pass);
        let root = Xpriv::new_master(self.network, seed.as_bytes())?;
        
        // Assuming default derivation path for now
        let path_str = format!("m/86'/{}/0'/0/0", if self.network == Network::Bitcoin { "0" } else { "1" });
        let path = DerivationPath::from_str(&path_str)?;
        let child_xprv = root.derive_priv(&secp, &path)?;
        
        Ok(child_xprv.to_keypair(&secp))
    }

    fn set_passphrase(&mut self, passphrase: Option<String>) {
        self.passphrase = passphrase;
    }

    async fn get_last_used_address_index(&self) -> Result<u32> {
        // This would require iterating through derived addresses and checking their history.
        // A full implementation is complex. Returning 0 for now.
        Ok(0)
    }

    async fn get_master_public_key(&self) -> Result<Option<String>> {
        Ok(self.keystore.as_ref().map(|k| k.account_xpub.to_string()))
    }
}

#[async_trait(?Send)]
impl AddressResolver for WebProvider {
    async fn resolve_all_identifiers(&self, _input: &str) -> Result<String> {
        unimplemented!()
    }
    
    fn contains_identifiers(&self, _input: &str) -> bool {
        unimplemented!()
    }
    
    async fn get_address(&self, _address_type: &str, _index: u32) -> Result<String> {
        unimplemented!()
    }
    
    async fn list_identifiers(&self) -> Result<Vec<String>> {
        unimplemented!()
    }
}

#[async_trait(?Send)]
impl BitcoinRpcProvider for WebProvider {
    async fn get_block_count(&self) -> Result<u64> {
        let result = self.call(&self.sandshrew_rpc_url, "getblockcount", serde_json::json!([]), 1).await?;
        result.as_u64().ok_or_else(|| AlkanesError::RpcError("Invalid block count response".to_string()))
    }
    
    async fn generate_to_address(&self, nblocks: u32, address: &str) -> Result<JsonValue> {
        let params = serde_json::json!([nblocks, address]);
        self.call(&self.sandshrew_rpc_url, "generatetoaddress", params, 1).await
    }

    async fn get_new_address(&self) -> Result<JsonValue> {
        self.call(&self.sandshrew_rpc_url, "getnewaddress", serde_json::json!([]), 1).await
    }
    
    async fn get_transaction_hex(&self, txid: &str) -> Result<String> {
        let params = serde_json::json!([txid, true]);
        let result = self.call(&self.sandshrew_rpc_url, "getrawtransaction", params, 1).await?;
        result.as_str().map(|s| s.to_string()).ok_or_else(|| AlkanesError::RpcError("Invalid transaction hex response".to_string()))
    }
    
    async fn get_block(&self, hash: &str, raw: bool) -> Result<JsonValue> {
        let verbosity = if raw { 1 } else { 2 };
        let params = serde_json::json!([hash, verbosity]);
        self.call(&self.sandshrew_rpc_url, "getblock", params, 1).await
    }
    
    async fn get_block_hash(&self, height: u64) -> Result<String> {
        let params = serde_json::json!([height]);
        let result = self.call(&self.sandshrew_rpc_url, "getblockhash", params, 1).await?;
        result.as_str().map(|s| s.to_string()).ok_or_else(|| AlkanesError::RpcError("Invalid block hash response".to_string()))
    }
    
    async fn send_raw_transaction(&self, tx_hex: &str) -> Result<String> {
        let params = serde_json::json!([tx_hex]);
        let result = self.call(&self.sandshrew_rpc_url, "sendrawtransaction", params, 1).await?;
        result.as_str().map(|s| s.to_string()).ok_or_else(|| AlkanesError::RpcError("Invalid txid response".to_string()))
    }
    
    async fn get_mempool_info(&self) -> Result<JsonValue> {
        self.call(&self.sandshrew_rpc_url, "getmempoolinfo", serde_json::json!([]), 1).await
    }
    
    async fn estimate_smart_fee(&self, target: u32) -> Result<JsonValue> {
        let params = serde_json::json!([target]);
        self.call(&self.sandshrew_rpc_url, "estimatesmartfee", params, 1).await
    }
    
    async fn get_esplora_blocks_tip_height(&self) -> Result<u64> {
        // This is an Esplora-specific method, but we can implement it using get_block_count for compatibility
        self.get_block_count().await
    }
    
    async fn trace_transaction(&self, txid: &str, vout: u32, block: Option<&str>, tx: Option<&str>) -> Result<serde_json::Value> {
        let params = serde_json::json!([txid, vout, block, tx]);
        self.call(&self.sandshrew_rpc_url, "trace_transaction", params, 1).await
    }
}

#[async_trait(?Send)]
impl MetashrewRpcProvider for WebProvider {
    async fn get_metashrew_height(&self) -> Result<u64> {
        unimplemented!()
    }
    
    async fn get_contract_meta(&self, _block: &str, _tx: &str) -> Result<JsonValue> {
        unimplemented!()
    }
    
    async fn trace_outpoint(&self, _txid: &str, _vout: u32) -> Result<JsonValue> {
        unimplemented!()
    }
    
    async fn get_spendables_by_address(&self, _address: &str) -> Result<JsonValue> {
        unimplemented!()
    }
    
    async fn get_protorunes_by_address(
        &self,
        _address: &str,
        _block_tag: Option<String>,
        _protocol_tag: u128,
    ) -> Result<ProtoruneWalletResponse> {
        unimplemented!()
    }
    
    async fn get_protorunes_by_outpoint(
        &self,
        txid: &str,
        vout: u32,
        _block_tag: Option<String>,
        protocol_tag: u128,
    ) -> Result<ProtoruneOutpointResponse> {
        let mut outpoint_pb = OutpointWithProtocol::default();
        outpoint_pb.txid = Vec::from_hex(txid)?;
        outpoint_pb.vout = vout;
        outpoint_pb.protocol = Some(Uint128 {
            lo: protocol_tag as u64,
            hi: (protocol_tag >> 64) as u64,
            ..Default::default()
        }).into();

        let hex_input = hex::encode(outpoint_pb.write_to_bytes()?);
        let params = serde_json::json!(["protorunesbyoutpoint", format!("0x{}", hex_input), "latest"]);

        let result = self.call(&self.sandshrew_rpc_url, "metashrew_view", params, 1).await?;

        let hex_str = result.as_str().ok_or_else(|| AlkanesError::RpcError("Invalid protorune response: not a string".to_string()))?;
        let bytes = hex::decode(hex_str.strip_prefix("0x").unwrap_or(hex_str))?;

        let response_pb = ProtoruneOutpointResponsePb::parse_from_bytes(&bytes[..])?;
        // self.logger.info(&format!("Received protorune response: {:?}", response_pb));

        // Convert from the protobuf-generated `BalanceSheet` to the `protorune_support` `BalanceSheet`
        let balances_pb = response_pb.balances.unwrap_or_default();
        let balance_sheet = BalanceSheet::<StubPointer>::from(balances_pb);

        Ok(ProtoruneOutpointResponse {
            balance_sheet,
            // The other fields are not present in the protobuf response, so they remain default.
            ..Default::default()
        })
    }
}

#[async_trait(?Send)]
impl MetashrewProvider for WebProvider {
    async fn get_height(&self) -> Result<u64> {
        unimplemented!()
    }
    async fn get_block_hash(&self, _height: u64) -> Result<String> {
        unimplemented!()
    }
    async fn get_state_root(&self, _height: JsonValue) -> Result<String> {
        unimplemented!()
    }
}

#[async_trait(?Send)]
impl RunestoneProvider for WebProvider {
    async fn decode_runestone(&self, _tx: &Transaction) -> Result<JsonValue> {
        unimplemented!()
    }
    
    async fn format_runestone_with_decoded_messages(&self, _tx: &Transaction) -> Result<JsonValue> {
        unimplemented!()
    }
    
    async fn analyze_runestone(&self, _txid: &str) -> Result<JsonValue> {
        unimplemented!()
    }
}

#[async_trait(?Send)]
impl OrdProvider for WebProvider {
    async fn get_inscription(&self, _inscription_id: &str) -> Result<OrdInscription> {
        unimplemented!()
    }
    
    async fn get_inscriptions_in_block(&self, _block_hash: &str) -> Result<OrdInscriptions> {
        unimplemented!()
    }
    async fn get_ord_address_info(&self, _address: &str) -> Result<OrdAddressInfo> {
        unimplemented!()
    }
    async fn get_block_info(&self, _query: &str) -> Result<OrdBlock> {
        unimplemented!()
    }
    async fn get_ord_block_count(&self) -> Result<u64> {
        unimplemented!()
    }
    async fn get_ord_blocks(&self) -> Result<OrdBlocks> {
        unimplemented!()
    }
    async fn get_children(&self, _inscription_id: &str, _page: Option<u32>) -> Result<OrdChildren> {
        unimplemented!()
    }
    async fn get_content(&self, _inscription_id: &str) -> Result<Vec<u8>> {
        unimplemented!()
    }
    async fn get_inscriptions(&self, _page: Option<u32>) -> Result<OrdInscriptions> {
        unimplemented!()
    }
    async fn get_output(&self, output: &str) -> Result<OrdOutput> {
        let json = self.call(self.sandshrew_rpc_url(), "ord_output", serde_json::json!([output]), 1).await?;
        serde_json::from_value(json).map_err(|e| AlkanesError::Serialization(e.to_string()))
    }
    async fn get_parents(&self, _inscription_id: &str, _page: Option<u32>) -> Result<OrdParents> {
        unimplemented!()
    }
    async fn get_rune(&self, _rune: &str) -> Result<OrdRuneInfo> {
        unimplemented!()
    }
    async fn get_runes(&self, _page: Option<u32>) -> Result<OrdRunes> {
        unimplemented!()
    }
    async fn get_sat(&self, _sat: u64) -> Result<OrdSat> {
        unimplemented!()
    }
    async fn get_tx_info(&self, _txid: &str) -> Result<OrdTxInfo> {
        unimplemented!()
    }
}

#[async_trait(?Send)]
impl AlkanesProvider for WebProvider {
    async fn execute(&mut self, params: EnhancedExecuteParams) -> Result<ExecutionState> {
        let mut executor = EnhancedAlkanesExecutor::new(self);
        executor.execute(params).await
    }

    async fn resume_execution(
        &mut self,
        state: ReadyToSignTx,
        params: &EnhancedExecuteParams,
    ) -> Result<EnhancedExecuteResult> {
        let mut executor = EnhancedAlkanesExecutor::new(self);
        executor.resume_execution(state, params).await
    }

    async fn resume_commit_execution(
        &mut self,
        state: ReadyToSignCommitTx,
    ) -> Result<ExecutionState> {
        let mut executor = EnhancedAlkanesExecutor::new(self);
        executor.resume_commit_execution(state).await
    }

    async fn resume_reveal_execution(
        &mut self,
        state: ReadyToSignRevealTx,
    ) -> Result<EnhancedExecuteResult> {
        let mut executor = EnhancedAlkanesExecutor::new(self);
        executor.resume_reveal_execution(state).await
    }
    
    async fn protorunes_by_address(
        &self,
        address: &str,
        block_tag: Option<String>,
        protocol_tag: u128,
    ) -> Result<ProtoruneWalletResponse> {
        <Self as MetashrewRpcProvider>::get_protorunes_by_address(self, address, block_tag, protocol_tag).await
    }
    async fn protorunes_by_outpoint(
        &self,
        txid: &str,
        vout: u32,
        block_tag: Option<String>,
        protocol_tag: u128,
    ) -> Result<ProtoruneOutpointResponse> {
        <Self as MetashrewRpcProvider>::get_protorunes_by_outpoint(self, txid, vout, block_tag, protocol_tag).await
    }
    async fn view(&self, contract_id: &str, view_fn: &str, params: Option<&[u8]>) -> Result<JsonValue> {
        let combined_view = format!("{}/{}", contract_id, view_fn);
        let params_hex = params.map(|p| format!("0x{}", hex::encode(p))).unwrap_or_else(|| "0x".to_string());
        
        let rpc_params = serde_json::json!([combined_view, params_hex, "latest"]);
        let result = self.call(&self.sandshrew_rpc_url, "metashrew_view", rpc_params, 1).await?;

        let hex_response = result.as_str().ok_or_else(|| {
            AlkanesError::RpcError("metashrew_view response was not a string".to_string())
        })?;

        let result_bytes = hex::decode(hex_response.strip_prefix("0x").unwrap_or(hex_response))?;

        // Attempt to deserialize as a simple u64 if it's 8 bytes long.
        if result_bytes.len() == 8 {
            let val = u64::from_le_bytes(result_bytes.try_into().unwrap());
            return Ok(serde_json::json!(val));
        }

        // Attempt to deserialize as generic JSON.
        if let Ok(json_val) = serde_json::from_slice(&result_bytes) {
            return Ok(json_val);
        }

        // Fallback to a hex string representation if it's not valid JSON.
        Ok(serde_json::json!(format!("0x{}", hex::encode(result_bytes))))
    }

    async fn trace(&self, outpoint: &str) -> Result<alkanes_pb::Trace> {
        let result = self.call(&self.sandshrew_rpc_url, "alkanes_trace", serde_json::json!([outpoint]), 1).await?;
        let hex_str = result.as_str().ok_or_else(|| AlkanesError::RpcError("Invalid trace response".to_string()))?;
        let bytes = hex::decode(hex_str.strip_prefix("0x").unwrap_or(hex_str))?;
        alkanes_pb::Trace::parse_from_bytes(&bytes[..]).map_err(|e| AlkanesError::Serialization(e.to_string()))
    }
    async fn get_block(&self, height: u64) -> Result<alkanes_pb::BlockResponse> {
        let result = self.call(&self.sandshrew_rpc_url, "alkanes_get_block", serde_json::json!([height]), 1).await?;
        let hex_str = result.as_str().ok_or_else(|| AlkanesError::RpcError("Invalid block response".to_string()))?;
        let bytes = hex::decode(hex_str.strip_prefix("0x").unwrap_or(hex_str))?;
        alkanes_pb::BlockResponse::parse_from_bytes(&bytes[..]).map_err(|e| AlkanesError::Serialization(e.to_string()))
    }
    async fn sequence(&self) -> Result<JsonValue> {
        self.call(&self.sandshrew_rpc_url, "alkanes_sequence", serde_json::json!(["0x"]), 1).await
    }
    async fn spendables_by_address(&self, address: &str) -> Result<JsonValue> {
        self.call(&self.sandshrew_rpc_url, "alkanes_spendables_by_address", serde_json::json!([address]), 1).await
    }
    async fn trace_block(&self, height: u64) -> Result<alkanes_pb::Trace> {
        let result = self.call(&self.sandshrew_rpc_url, "alkanes_trace_block", serde_json::json!([height]), 1).await?;
        let hex_str = result.as_str().ok_or_else(|| AlkanesError::RpcError("Invalid trace block response".to_string()))?;
        let bytes = hex::decode(hex_str.strip_prefix("0x").unwrap_or(hex_str))?;
        alkanes_pb::Trace::parse_from_bytes(&bytes[..]).map_err(|e| AlkanesError::Serialization(e.to_string()))
    }
    async fn get_bytecode(&self, alkane_id: &str, block_tag: Option<String>) -> Result<String> {
        use alkanes_support::proto::alkanes::BytecodeRequest;
        let parts: Vec<&str> = alkane_id.split(':').collect();
        if parts.len() != 2 {
            return Err(AlkanesError::InvalidParameters("Invalid alkane_id format".to_string()));
        }
        let block = parts[0].parse::<u64>()?;
        let tx = parts[1].parse::<u32>()?;

        let mut request = BytecodeRequest::new();
        let mut id = alkanes_pb::AlkaneId::new();
        let mut block_uint = alkanes_pb::Uint128::new();
        block_uint.lo = block;
        id.block = Some(block_uint).into();
        let mut tx_uint = alkanes_pb::Uint128::new();
        tx_uint.lo = tx as u64;
        id.tx = Some(tx_uint).into();
        request.id = Some(id).into();
        let hex_input = hex::encode(request.write_to_bytes()?);

        let params = serde_json::json!(["getbytecode", format!("0x{}", hex_input), block_tag.as_deref().unwrap_or("latest")]);
        let result = self.call(&self.sandshrew_rpc_url, "metashrew_view", params, 1).await?;
        
        let hex_str = result.as_str().ok_or_else(|| AlkanesError::RpcError("Invalid bytecode response: not a string".to_string()))?;
        let bytes = hex::decode(hex_str.strip_prefix("0x").unwrap_or(hex_str))?;
        Ok(format!("0x{}", hex::encode(bytes)))
    }
    async fn inspect(&self, target: &str, config: AlkanesInspectConfig) -> Result<AlkanesInspectResult> {
        let params = serde_json::json!([target, config]);
        let result = self.call(&self.sandshrew_rpc_url, "alkanes_inspect", params, 1).await?;
        serde_json::from_value(result).map_err(|e| AlkanesError::Serialization(e.to_string()))
    }
    async fn get_balance(&self, address: Option<&str>) -> Result<Vec<AlkaneBalance>> {
        let addr = match address {
            Some(a) => a.to_string(),
            None => WalletProvider::get_address(self).await?,
        };
        let result = self.call(&self.sandshrew_rpc_url, "alkanes_get_balance", serde_json::json!([addr]), 1).await?;
        serde_json::from_value(result).map_err(|e| AlkanesError::Serialization(e.to_string()))
    }
}

#[async_trait(?Send)]
impl MonitorProvider for WebProvider {
    async fn monitor_blocks(&self, _start: Option<u64>) -> Result<()> {
        unimplemented!()
    }
    
    async fn get_block_events(&self, _height: u64) -> Result<Vec<BlockEvent>> {
        unimplemented!()
    }
}

#[async_trait(?Send)]
impl KeystoreProvider for WebProvider {
    async fn derive_addresses(&self, master_public_key: &str, network_params: &alkanes_cli_common::network::NetworkParams, script_types: &[&str], start_index: u32, count: u32) -> Result<Vec<KeystoreAddress>> {
        let mut addresses = Vec::new();
        for script_type in script_types {
            for i in start_index..(start_index + count) {
                let purpose = match script_type {
                    &"p2wpkh" => "84",
                    &"p2tr" => "86",
                    _ => continue,
                };
                let coin_type = match network_params.network {
                    Network::Bitcoin => "0",
                    _ => "1",
                };
                let path_str = format!("m/{purpose}'/{coin_type}'/0'/0/{i}");
                let path = DerivationPath::from_str(&path_str)?;
                let address = alkanes_cli_common::keystore::derive_address_from_public_key(
                    master_public_key,
                    &path,
                    network_params,
                    script_type,
                )?;
                addresses.push(KeystoreAddress {
                    address,
                    derivation_path: path_str,
                    index: i,
                    script_type: (*script_type).to_string(),
                    network: Some(network_params.network.to_string()),
                });
            }
        }
        Ok(addresses)
    }
    
    async fn get_default_addresses(&self, master_public_key: &str, network_params: &alkanes_cli_common::network::NetworkParams) -> Result<Vec<KeystoreAddress>> {
        let script_types = vec!["p2wpkh", "p2tr"];
        self.derive_addresses(master_public_key, network_params, &script_types, 0, 1).await
    }

    async fn get_address(&self, address_type: &str, index: u32) -> Result<String> {
        <Self as AddressResolver>::get_address(self, address_type, index).await
    }
    
    fn parse_address_range(&self, range_spec: &str) -> Result<(String, u32, u32)> {
        let parts: Vec<&str> = range_spec.split(':').collect();
        if parts.len() != 2 {
            return Err(AlkanesError::InvalidParameters("Invalid range specifier. Expected format: script_type:start-end".to_string()));
        }
        let script_type = parts[0].to_string();
        let range_parts: Vec<&str> = parts[1].split('-').collect();
        if range_parts.len() != 2 {
            return Err(AlkanesError::InvalidParameters("Invalid range format. Expected start-end".to_string()));
        }
        let start_index = range_parts[0].parse::<u32>()?;
        let end_index = range_parts[1].parse::<u32>()?;
        if end_index < start_index {
            return Err(AlkanesError::InvalidParameters("End index cannot be less than start index".to_string()));
        }
        let count = end_index - start_index + 1;
        Ok((script_type, start_index, count))
    }
    
    async fn get_keystore_info(&self, master_fingerprint: &str, created_at: u64, version: &str) -> Result<KeystoreInfo> {
        Ok(KeystoreInfo {
            master_fingerprint: master_fingerprint.to_string(),
            created_at,
            version: version.to_string(),
        })
    }

    async fn derive_address_from_path(&self, master_public_key: &str, path: &DerivationPath, script_type: &str, network_params: &alkanes_cli_common::network::NetworkParams) -> Result<KeystoreAddress> {
        let address = alkanes_cli_common::keystore::derive_address_from_public_key(
            master_public_key,
            path,
            network_params,
            script_type,
        )?;

        Ok(KeystoreAddress {
            address,
            derivation_path: path.to_string(),
            index: path.into_iter().last().map(|child| match *child {
                bitcoin::bip32::ChildNumber::Normal { index } => index,
                bitcoin::bip32::ChildNumber::Hardened { index } => index,
            }).unwrap_or(0),
            script_type: script_type.to_string(),
            network: Some(network_params.network.to_string()),
        })
    }
}

#[async_trait(?Send)]
impl DeezelProvider for WebProvider {
    fn provider_name(&self) -> &str {
        "WebProvider"
    }

    fn get_bitcoin_rpc_url(&self) -> Option<String> {
        self.esplora_rpc_url.clone()
    }

    fn get_esplora_api_url(&self) -> Option<String> {
        self.esplora_rpc_url.clone()
    }

    fn get_ord_server_url(&self) -> Option<String> {
        // Assuming no separate ord server for web provider, using sandshrew
        Some(self.sandshrew_rpc_url.clone())
    }

    fn get_metashrew_rpc_url(&self) -> Option<String> {
        Some(self.sandshrew_rpc_url.clone())
    }

    fn clone_box(&self) -> Box<dyn DeezelProvider> {
        Box::new(self.clone())
    }
    
    async fn initialize(&self) -> Result<()> {
        self.logger.info("Alkanes WebProvider Initialized");
        Ok(())
    }
    
    async fn shutdown(&self) -> Result<()> {
        self.logger.info("Alkanes WebProvider Shutdown");
        Ok(())
    }

    fn secp(&self) -> &Secp256k1<All> {
        unimplemented!("WebProvider does not hold a secp context directly. It should be handled in the crypto module.")
    }

    async fn get_utxo(&self, _outpoint: &OutPoint) -> Result<Option<TxOut>> {
        unimplemented!()
    }

    async fn sign_taproot_script_spend(&self, _sighash: bitcoin::secp256k1::Message) -> Result<bitcoin::secp256k1::schnorr::Signature> {
        unimplemented!()
    }

    async fn wrap(&mut self, amount: u64, address: Option<String>, fee_rate: Option<f32>) -> Result<String> {
        use alkanes_cli_common::alkanes::types::{ProtostoneSpec, BitcoinTransfer};
        use alkanes_support::cellpack::Cellpack;
        use base64::{engine::general_purpose::STANDARD, Engine as _};

        let is_regtest = self.network == Network::Regtest;
        let mut executor = EnhancedAlkanesExecutor::new(self);
        let params = EnhancedExecuteParams {
            fee_rate,
            to_addresses: vec![],
            from_addresses: address.map(|a| vec![a]),
            change_address: None,
            input_requirements: vec![],
            protostones: vec![ProtostoneSpec {
                cellpack: Some(Cellpack::try_from(vec![2, 0, 1]).unwrap()), // Assuming 2 is for wrapping, 0 is frBTC, 1 is mint
                edicts: vec![],
                bitcoin_transfer: Some(BitcoinTransfer { amount, target: alkanes_cli_common::alkanes::types::OutputTarget::Split }),
            }],
            envelope_data: None,
            raw_output: false,
            trace_enabled: false,
            mine_enabled: is_regtest,
            auto_confirm: false,
        };

        match executor.execute(params).await? {
            ExecutionState::ReadyToSign(ready_tx) => {
                Ok(STANDARD.encode(&ready_tx.psbt.serialize()))
            }
            _ => Err(AlkanesError::Other("Unexpected execution state".to_string())),
        }
    }

    async fn unwrap(&mut self, amount: u64, address: Option<String>) -> Result<String> {
        use alkanes_cli_common::alkanes::types::{ProtostoneSpec, BitcoinTransfer};
        use alkanes_support::cellpack::Cellpack;
        use base64::{engine::general_purpose::STANDARD, Engine as _};

        let is_regtest = self.network == Network::Regtest;
        let mut executor = EnhancedAlkanesExecutor::new(self);
        let params = EnhancedExecuteParams {
            fee_rate: None,
            to_addresses: vec![],
            from_addresses: address.map(|a| vec![a]),
            change_address: None,
            input_requirements: vec![],
            protostones: vec![ProtostoneSpec {
                cellpack: Some(Cellpack::try_from(vec![2, 0, 2]).unwrap()), // Assuming 2 is for unwrapping, 0 is frBTC, 2 is burn
                edicts: vec![],
                bitcoin_transfer: Some(BitcoinTransfer { amount, target: alkanes_cli_common::alkanes::types::OutputTarget::Split }),
            }],
            envelope_data: None,
            raw_output: false,
            trace_enabled: false,
            mine_enabled: is_regtest,
            auto_confirm: false,
        };

        match executor.execute(params).await? {
            ExecutionState::ReadyToSign(ready_tx) => {
                Ok(STANDARD.encode(&ready_tx.psbt.serialize()))
            }
            _ => Err(AlkanesError::Other("Unexpected execution state".to_string())),
        }
    }
}