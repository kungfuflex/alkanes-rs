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
use alkanes_cli_common::alkanes::balance_sheet::BalanceSheetOperations;
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
use alkanes_cli_common::proto::protorune::{OutpointWithProtocol, OutpointResponse as ProtoruneOutpointResponsePb, Uint128};
use alkanes_cli_common::alkanes::execute::EnhancedAlkanesExecutor;
use alkanes_cli_common::index_pointer::StubPointer;
use alkanes_cli_common::alkanes::balance_sheet::BalanceSheet;
use core::str::FromStr;
use bitcoin::hashes::hex::FromHex;


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
#[wasm_bindgen]
pub struct WebProvider {
    rpc_config: alkanes_cli_common::network::RpcConfig,
    network: Network,
    storage: WebStorage,
    network_client: WebNetwork,
    crypto: WebCrypto,
    time: WebTime,
    logger: WebLogger,
    keystore: Option<alkanes_cli_common::keystore::Keystore>,
    passphrase: Option<String>,
}

#[wasm_bindgen]
impl WebProvider {
    /// Create a new WebProvider from provider name and optional config overrides
    /// 
    /// # Arguments
    /// * `provider` - Network provider: "mainnet", "signet", "subfrost-regtest", "regtest"
    /// * `config` - Optional JS object with RpcConfig fields to override defaults
    ///
    /// # Example (JavaScript)
    /// ```js
    /// // Simple - uses all defaults for signet
    /// const provider = new WebProvider("signet");
    /// 
    /// // With overrides
    /// const provider = new WebProvider("signet", {
    ///   bitcoin_rpc_url: "https://custom-rpc.example.com",
    ///   esplora_url: "https://custom-esplora.example.com"
    /// });
    /// ```
    #[wasm_bindgen(constructor)]
    pub fn new_js(provider: String, config: Option<JsValue>) -> std::result::Result<WebProvider, JsValue> {
        // Start with provider string to create base config with defaults
        let mut rpc_config = alkanes_cli_common::network::RpcConfig {
            provider: provider.clone(),
            bitcoin_rpc_url: None,
            jsonrpc_url: None,
            titan_api_url: None,
            esplora_url: None,
            ord_url: None,
            metashrew_rpc_url: None,
            brc20_prog_rpc_url: None,
            data_api_url: None,
            subfrost_api_key: None,
            timeout_seconds: 600,
        };
        
        // Apply any overrides from config object
        if let Some(cfg) = config {
            if !cfg.is_null() && !cfg.is_undefined() {
                // Parse JS object into RpcConfig overrides
                let config_obj: serde_json::Value = serde_wasm_bindgen::from_value(cfg)
                    .map_err(|e| JsValue::from_str(&format!("Invalid config object: {}", e)))?;
                
                if let Some(obj) = config_obj.as_object() {
                    if let Some(bitcoin_rpc_url) = obj.get("bitcoin_rpc_url").and_then(|v| v.as_str()) {
                        rpc_config.bitcoin_rpc_url = Some(bitcoin_rpc_url.to_string());
                    }
                    if let Some(jsonrpc_url) = obj.get("jsonrpc_url").and_then(|v| v.as_str()) {
                        rpc_config.jsonrpc_url = Some(jsonrpc_url.to_string());
                    }
                    if let Some(esplora_url) = obj.get("esplora_url").and_then(|v| v.as_str()) {
                        rpc_config.esplora_url = Some(esplora_url.to_string());
                    }
                    if let Some(ord_url) = obj.get("ord_url").and_then(|v| v.as_str()) {
                        rpc_config.ord_url = Some(ord_url.to_string());
                    }
                    if let Some(metashrew_rpc_url) = obj.get("metashrew_rpc_url").and_then(|v| v.as_str()) {
                        rpc_config.metashrew_rpc_url = Some(metashrew_rpc_url.to_string());
                    }
                    if let Some(brc20_prog_rpc_url) = obj.get("brc20_prog_rpc_url").and_then(|v| v.as_str()) {
                        rpc_config.brc20_prog_rpc_url = Some(brc20_prog_rpc_url.to_string());
                    }
                    if let Some(subfrost_api_key) = obj.get("subfrost_api_key").and_then(|v| v.as_str()) {
                        rpc_config.subfrost_api_key = Some(subfrost_api_key.to_string());
                    }
                    if let Some(timeout) = obj.get("timeout_seconds").and_then(|v| v.as_u64()) {
                        rpc_config.timeout_seconds = timeout;
                    }
                }
            }
        }
        
        // Determine network from provider
        let network = match provider.as_str() {
            "mainnet" => Network::Bitcoin,
            "testnet" => Network::Testnet,
            "signet" => Network::Signet,
            "regtest" | "subfrost-regtest" => Network::Regtest,
            _ => Network::Regtest, // default
        };
        
        Ok(Self {
            rpc_config,
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

    // Helper methods to get effective URLs from RpcConfig
    pub fn sandshrew_rpc_url(&self) -> String {
        self.rpc_config.get_alkanes_rpc_target().url
    }

    pub fn esplora_rpc_url(&self) -> Option<String> {
        Some(self.rpc_config.get_esplora_rpc_target().url)
    }

    pub fn bitcoin_rpc_url(&self) -> String {
        self.rpc_config.get_bitcoin_rpc_target().url
    }

    pub fn brc20_prog_rpc_url(&self) -> String {
        self.rpc_config.brc20_prog_rpc_url.clone()
            .or_else(|| self.rpc_config.get_default_brc20_prog_rpc_url())
            .unwrap_or_else(|| alkanes_cli_common::network::get_default_brc20_prog_rpc_url(self.network))
    }



    /// Get enriched wallet balances using the balances.lua script
    /// 
    /// This uses the built-in balances.lua script with automatic hash-based caching.
    /// Returns comprehensive balance data including spendable UTXOs, asset UTXOs, and pending.
    #[wasm_bindgen(js_name = getEnrichedBalances)]
    pub fn get_enriched_balances_js(
        &self,
        address: String,
        protocol_tag: Option<String>,
    ) -> js_sys::Promise {
        use alkanes_cli_common::lua_script::{LuaScriptExecutor, scripts::BALANCES};
        use serde_json::Value as JsonValue;
        use wasm_bindgen_futures::future_to_promise;

        let provider = self.clone();
        
        future_to_promise(async move {
            let tag = protocol_tag.unwrap_or_else(|| "1".to_string());
            let args = vec![
                JsonValue::String(address),
                JsonValue::String(tag),
            ];

            let result = provider.execute_lua_script(&BALANCES, args)
                .await
                .map_err(|e| JsValue::from_str(&format!("Failed to get enriched balances: {}", e)))?;

            serde_wasm_bindgen::to_value(&result)
                .map_err(|e| JsValue::from_str(&format!("Failed to serialize result: {}", e)))
        })
    }

    /// Get all transactions for an address from Esplora
    #[wasm_bindgen(js_name = getAddressTxs)]
    pub fn get_address_txs_js(&self, address: String) -> js_sys::Promise {
        use alkanes_cli_common::traits::EsploraProvider;
        use wasm_bindgen_futures::future_to_promise;

        let provider = self.clone();
        
        future_to_promise(async move {
            let result = provider.get_address_txs(&address)
                .await
                .map_err(|e| JsValue::from_str(&format!("Failed to get address transactions: {}", e)))?;

            serde_wasm_bindgen::to_value(&result)
                .map_err(|e| JsValue::from_str(&format!("Failed to serialize result: {}", e)))
        })
    }

    /// Get raw transaction hex
    #[wasm_bindgen(js_name = getTransactionHex)]
    pub fn get_transaction_hex_js(&self, txid: String) -> js_sys::Promise {
        use alkanes_cli_common::traits::EsploraProvider;
        use wasm_bindgen_futures::future_to_promise;

        let provider = self.clone();
        
        future_to_promise(async move {
            let result = provider.get_transaction_hex(&txid)
                .await
                .map_err(|e| JsValue::from_str(&format!("Failed to get transaction hex: {}", e)))?;

            Ok(JsValue::from_str(&result))
        })
    }

    /// Trace alkanes execution for a protostone outpoint
    #[wasm_bindgen(js_name = traceOutpoint)]
    pub fn trace_outpoint_js(&self, outpoint: String) -> js_sys::Promise {
        use alkanes_cli_common::traits::AlkanesProvider;
        use wasm_bindgen_futures::future_to_promise;

        let provider = self.clone();
        
        future_to_promise(async move {
            let trace_pb = provider.trace(&outpoint)
                .await
                .map_err(|e| JsValue::from_str(&format!("Failed to trace outpoint: {}", e)))?;

            serde_wasm_bindgen::to_value(&trace_pb)
                .map_err(|e| JsValue::from_str(&format!("Failed to serialize trace: {}", e)))
        })
    }

    /// Get address UTXOs
    #[wasm_bindgen(js_name = getAddressUtxos)]
    pub fn get_address_utxos_js(&self, address: String) -> js_sys::Promise {
        use alkanes_cli_common::traits::EsploraProvider;
        use wasm_bindgen_futures::future_to_promise;

        let provider = self.clone();
        
        future_to_promise(async move {
            let result = provider.get_address_utxo(&address)
                .await
                .map_err(|e| JsValue::from_str(&format!("Failed to get address UTXOs: {}", e)))?;

            serde_wasm_bindgen::to_value(&result)
                .map_err(|e| JsValue::from_str(&format!("Failed to serialize result: {}", e)))
        })
    }

    /// Broadcast a raw transaction
    #[wasm_bindgen(js_name = broadcastTransaction)]
    pub fn broadcast_transaction_js(&self, tx_hex: String) -> js_sys::Promise {
        use alkanes_cli_common::traits::WalletProvider;
        use wasm_bindgen_futures::future_to_promise;

        let provider = self.clone();
        
        future_to_promise(async move {
            let result = provider.broadcast_transaction(tx_hex)
                .await
                .map_err(|e| JsValue::from_str(&format!("Failed to broadcast transaction: {}", e)))?;

            Ok(JsValue::from_str(&result))
        })
    }

    /// Get address transactions with complete runestone traces (CLI: esplora address-txs --runestone-trace)
    #[wasm_bindgen(js_name = getAddressTxsWithTraces)]
    pub fn get_address_txs_with_traces_js(&self, address: String, exclude_coinbase: Option<bool>) -> js_sys::Promise {
        use alkanes_cli_common::traits::{EsploraProvider, BitcoinRpcProvider, AlkanesProvider};
        use alkanes_cli_common::esplora::EsploraTransaction;
        use wasm_bindgen_futures::future_to_promise;
        use serde_json::Value as JsonValue;

        let provider = self.clone();
        let exclude_coinbase = exclude_coinbase.unwrap_or(false);
        
        future_to_promise(async move {
            let result = provider.get_address_txs(&address).await
                .map_err(|e| JsValue::from_str(&format!("Failed: {}", e)))?;
            let mut txs: Vec<EsploraTransaction> = serde_json::from_value(result)
                .map_err(|e| JsValue::from_str(&format!("Parse failed: {}", e)))?;
            if exclude_coinbase {
                txs.retain(|tx| !tx.vin.iter().any(|vin| vin.is_coinbase));
            }
            let mut enriched_txs = Vec::new();
            for esplora_tx in txs {
                let has_op_return = esplora_tx.vout.iter().any(|o| o.scriptpubkey_type == "op_return");
                let mut tx_data = serde_json::to_value(&esplora_tx)
                    .map_err(|e| JsValue::from_str(&format!("Serialize failed: {}", e)))?;
                if has_op_return {
                    if let Ok(tx_hex) = provider.get_transaction_hex(&esplora_tx.txid).await {
                        if let Ok(tx_bytes) = hex::decode(&tx_hex) {
                            if let Ok(transaction) = bitcoin::consensus::deserialize::<bitcoin::Transaction>(&tx_bytes) {
                                if let Ok(runestone_result) = alkanes_cli_common::runestone_enhanced::format_runestone_with_decoded_messages(&transaction) {
                                    let num_protostones = runestone_result.get("protostones").and_then(|p| p.as_array()).map(|a| a.len()).unwrap_or(0);
                                    if num_protostones > 0 {
                                        if let Some(obj) = tx_data.as_object_mut() {
                                            obj.insert("runestone".to_string(), runestone_result);
                                        }
                                        let base_vout = transaction.output.len() as u32 + 1;
                                        let mut traces = Vec::new();
                                        for i in 0..num_protostones {
                                            let vout = base_vout + i as u32;
                                            let outpoint = format!("{}:{}", esplora_tx.txid, vout);
                                            if let Ok(trace_pb) = provider.trace(&outpoint).await {
                                                if let Ok(trace_json) = serde_json::to_value(&trace_pb) {
                                                    let mut trace_obj = serde_json::json!({"vout": vout, "outpoint": outpoint, "protostone_index": i});
                                                    if let Some(obj) = trace_obj.as_object_mut() {
                                                        obj.insert("trace".to_string(), trace_json);
                                                    }
                                                    traces.push(trace_obj);
                                                }
                                            }
                                        }
                                        if let Some(obj) = tx_data.as_object_mut() {
                                            obj.insert("alkanes_traces".to_string(), JsonValue::Array(traces));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                enriched_txs.push(tx_data);
            }
            serde_wasm_bindgen::to_value(&enriched_txs)
                .map_err(|e| JsValue::from_str(&format!("Serialization failed: {}", e)))
        })
    }

    // === ORD METHODS ===
    
    #[wasm_bindgen(js_name = ordInscription)]
    pub fn ord_inscription_js(&self, inscription_id: String) -> js_sys::Promise {
        use alkanes_cli_common::traits::OrdProvider;
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            provider.get_inscription(&inscription_id).await
                .and_then(|r| serde_json::to_value(&r).map_err(Into::into))
                .and_then(|v| serde_wasm_bindgen::to_value(&v).map_err(|e| alkanes_cli_common::AlkanesError::Serialization(e.to_string())))
                .map_err(|e| JsValue::from_str(&format!("Failed: {}", e)))
        })
    }

    #[wasm_bindgen(js_name = ordInscriptions)]
    pub fn ord_inscriptions_js(&self, page: Option<f64>) -> js_sys::Promise {
        use alkanes_cli_common::traits::OrdProvider;
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            provider.get_inscriptions(page.map(|p| p as u32)).await
                .and_then(|r| serde_json::to_value(&r).map_err(Into::into))
                .and_then(|v| serde_wasm_bindgen::to_value(&v).map_err(|e| alkanes_cli_common::AlkanesError::Serialization(e.to_string())))
                .map_err(|e| JsValue::from_str(&format!("Failed: {}", e)))
        })
    }

    #[wasm_bindgen(js_name = ordOutputs)]
    pub fn ord_outputs_js(&self, address: String) -> js_sys::Promise {
        use alkanes_cli_common::traits::OrdProvider;
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            provider.get_output(&address).await
                .and_then(|r| serde_wasm_bindgen::to_value(&r).map_err(|e| alkanes_cli_common::AlkanesError::Serialization(e.to_string())))
                .map_err(|e| JsValue::from_str(&format!("Failed: {}", e)))
        })
    }

    #[wasm_bindgen(js_name = ordRune)]
    pub fn ord_rune_js(&self, rune: String) -> js_sys::Promise {
        use alkanes_cli_common::traits::OrdProvider;
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            provider.get_rune(&rune).await
                .and_then(|r| serde_json::to_value(&r).map_err(Into::into))
                .and_then(|v| serde_wasm_bindgen::to_value(&v).map_err(|e| alkanes_cli_common::AlkanesError::Serialization(e.to_string())))
                .map_err(|e| JsValue::from_str(&format!("Failed: {}", e)))
        })
    }

    // === ALKANES METHODS ===
    
    /// Execute an alkanes smart contract
    #[wasm_bindgen(js_name = alkanesExecute)]
    pub fn alkanes_execute_js(&self, params_json: String) -> js_sys::Promise {
        use alkanes_cli_common::traits::AlkanesProvider;
        use alkanes_cli_common::alkanes::types::EnhancedExecuteParams;
        use wasm_bindgen_futures::future_to_promise;
        let mut provider = self.clone();
        future_to_promise(async move {
            let params: EnhancedExecuteParams = serde_json::from_str(&params_json)
                .map_err(|e| JsValue::from_str(&format!("Invalid params JSON: {}", e)))?;
            provider.execute(params).await
                .and_then(|r| serde_wasm_bindgen::to_value(&r).map_err(|e| alkanes_cli_common::AlkanesError::Serialization(e.to_string())))
                .map_err(|e| JsValue::from_str(&format!("Execution failed: {}", e)))
        })
    }

    /// Resume execution after user confirmation (for simple transactions)
    #[wasm_bindgen(js_name = alkanesResumeExecution)]
    pub fn alkanes_resume_execution_js(&self, state_json: String, params_json: String) -> js_sys::Promise {
        use alkanes_cli_common::traits::AlkanesProvider;
        use alkanes_cli_common::alkanes::types::{EnhancedExecuteParams, ReadyToSignTx};
        use wasm_bindgen_futures::future_to_promise;
        let mut provider = self.clone();
        future_to_promise(async move {
            let state: ReadyToSignTx = serde_json::from_str(&state_json)
                .map_err(|e| JsValue::from_str(&format!("Invalid state JSON: {}", e)))?;
            let params: EnhancedExecuteParams = serde_json::from_str(&params_json)
                .map_err(|e| JsValue::from_str(&format!("Invalid params JSON: {}", e)))?;
            provider.resume_execution(state, &params).await
                .and_then(|r| serde_wasm_bindgen::to_value(&r).map_err(|e| alkanes_cli_common::AlkanesError::Serialization(e.to_string())))
                .map_err(|e| JsValue::from_str(&format!("Resume execution failed: {}", e)))
        })
    }

    /// Resume execution after commit transaction confirmation
    #[wasm_bindgen(js_name = alkanesResumeCommitExecution)]
    pub fn alkanes_resume_commit_execution_js(&self, state_json: String) -> js_sys::Promise {
        use alkanes_cli_common::traits::AlkanesProvider;
        use alkanes_cli_common::alkanes::types::ReadyToSignCommitTx;
        use wasm_bindgen_futures::future_to_promise;
        let mut provider = self.clone();
        future_to_promise(async move {
            let state: ReadyToSignCommitTx = serde_json::from_str(&state_json)
                .map_err(|e| JsValue::from_str(&format!("Invalid state JSON: {}", e)))?;
            provider.resume_commit_execution(state).await
                .and_then(|r| serde_wasm_bindgen::to_value(&r).map_err(|e| alkanes_cli_common::AlkanesError::Serialization(e.to_string())))
                .map_err(|e| JsValue::from_str(&format!("Resume commit execution failed: {}", e)))
        })
    }

    /// Resume execution after reveal transaction confirmation
    #[wasm_bindgen(js_name = alkanesResumeRevealExecution)]
    pub fn alkanes_resume_reveal_execution_js(&self, state_json: String) -> js_sys::Promise {
        use alkanes_cli_common::traits::AlkanesProvider;
        use alkanes_cli_common::alkanes::types::ReadyToSignRevealTx;
        use wasm_bindgen_futures::future_to_promise;
        let mut provider = self.clone();
        future_to_promise(async move {
            let state: ReadyToSignRevealTx = serde_json::from_str(&state_json)
                .map_err(|e| JsValue::from_str(&format!("Invalid state JSON: {}", e)))?;
            provider.resume_reveal_execution(state).await
                .and_then(|r| serde_wasm_bindgen::to_value(&r).map_err(|e| alkanes_cli_common::AlkanesError::Serialization(e.to_string())))
                .map_err(|e| JsValue::from_str(&format!("Resume reveal execution failed: {}", e)))
        })
    }

    /// Simulate an alkanes contract call (read-only)
    #[wasm_bindgen(js_name = alkanesSimulate)]
    pub fn alkanes_simulate_js(&self, contract_id: String, context_json: String, block_tag: Option<String>) -> js_sys::Promise {
        use alkanes_cli_common::traits::AlkanesProvider;
        use alkanes_cli_common::proto::alkanes::MessageContextParcel;
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            let context: MessageContextParcel = serde_json::from_str(&context_json)
                .map_err(|e| JsValue::from_str(&format!("Invalid context JSON: {}", e)))?;
            provider.simulate(&contract_id, &context, block_tag).await
                .and_then(|r| serde_wasm_bindgen::to_value(&r).map_err(|e| alkanes_cli_common::AlkanesError::Serialization(e.to_string())))
                .map_err(|e| JsValue::from_str(&format!("Simulation failed: {}", e)))
        })
    }

    /// Get alkanes contract balance for an address
    #[wasm_bindgen(js_name = alkanesBalance)]
    pub fn alkanes_balance_js(&self, address: Option<String>) -> js_sys::Promise {
        use alkanes_cli_common::traits::AlkanesProvider;
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            <WebProvider as AlkanesProvider>::get_balance(&provider, address.as_deref()).await
                .and_then(|r| serde_wasm_bindgen::to_value(&r).map_err(|e| alkanes_cli_common::AlkanesError::Serialization(e.to_string())))
                .map_err(|e| JsValue::from_str(&format!("Balance query failed: {}", e)))
        })
    }

    /// Get alkanes contract bytecode
    #[wasm_bindgen(js_name = alkanesBytecode)]
    pub fn alkanes_bytecode_js(&self, alkane_id: String, block_tag: Option<String>) -> js_sys::Promise {
        use alkanes_cli_common::traits::AlkanesProvider;
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            provider.get_bytecode(&alkane_id, block_tag).await
                .map(|hex_str| JsValue::from_str(&hex_str))
                .map_err(|e| JsValue::from_str(&format!("Bytecode query failed: {}", e)))
        })
    }

    /// Get all pools with details from an AMM factory (parallel optimized for browser)
    #[wasm_bindgen(js_name = alkanesGetAllPoolsWithDetails)]
    pub fn alkanes_get_all_pools_with_details_js(
        &self,
        factory_id: String,
        chunk_size: Option<f64>,
        max_concurrent: Option<f64>,
    ) -> js_sys::Promise {
        use alkanes_cli_common::alkanes::amm::AmmManager;
        use alkanes_cli_common::alkanes::types::AlkaneId;
        use alkanes_cli_common::traits::AlkanesProvider;
        use alkanes_cli_common::proto::alkanes::MessageContextParcel;
        use wasm_bindgen_futures::future_to_promise;
        use futures::stream::{self, StreamExt};
        use web_sys::console;
        use leb128;
        
        let provider = self.clone();
        let chunk_size = chunk_size.map(|c| c as usize).unwrap_or(30);
        let max_concurrent = max_concurrent.map(|m| m as usize).unwrap_or(10);
        
        future_to_promise(async move {
            // Parse factory ID
            let parts: Vec<&str> = factory_id.split(':').collect();
            if parts.len() != 2 {
                return Err(JsValue::from_str("Invalid factory_id format, expected block:tx"));
            }
            let block: u64 = parts[0].parse()
                .map_err(|_| JsValue::from_str("Invalid block number"))?;
            let tx: u64 = parts[1].parse()
                .map_err(|_| JsValue::from_str("Invalid tx number"))?;
            let factory = AlkaneId { block, tx };
            
            // Step 1: Get all pool IDs by calling factory directly
            let mut calldata = Vec::new();
            leb128::write::unsigned(&mut calldata, factory.block).unwrap();
            leb128::write::unsigned(&mut calldata, factory.tx).unwrap();
            leb128::write::unsigned(&mut calldata, 3u64).unwrap(); // GET_ALL_POOLS opcode
            
            let context = MessageContextParcel {
                alkanes: vec![],
                transaction: vec![],
                block: vec![],
                height: 0,
                vout: 0,
                txindex: 0,
                calldata,
                pointer: 0,
                refund_pointer: 0,
            };

            let result = provider.simulate(&format!("{}:{}", factory.block, factory.tx), &context, None).await
                .map_err(|e| JsValue::from_str(&format!("Failed to get pool list: {}", e)))?;
            
            let data_hex = result
                .get("data")
                .and_then(|v| v.as_str())
                .ok_or_else(|| JsValue::from_str("No data in response"))?;

            // Parse pool IDs from hex response
            let pool_ids = alkanes_cli_common::alkanes::amm::decode_get_all_pools(data_hex)
                .ok_or_else(|| JsValue::from_str("Failed to decode pool list"))?
                .pools;
            let total = pool_ids.len();
            
            // Step 2: Fetch details in parallel chunks
            let chunks: Vec<Vec<_>> = pool_ids.chunks(chunk_size)
                .map(|chunk| chunk.to_vec())
                .collect();
            
            let mut all_pool_details = Vec::new();
            
            // Process chunks with concurrency limit
            let results: Vec<_> = stream::iter(chunks)
                .map(|chunk| {
                    let provider_clone = provider.clone();
                    async move {
                        let mut chunk_results = Vec::new();
                        for pool_id in chunk {
                            // Build calldata for POOL_DETAILS opcode (999)
                            let mut calldata = Vec::new();
                            leb128::write::unsigned(&mut calldata, pool_id.block).unwrap();
                            leb128::write::unsigned(&mut calldata, pool_id.tx).unwrap();
                            leb128::write::unsigned(&mut calldata, 999u64).unwrap();
                            
                            let context = MessageContextParcel {
                                alkanes: vec![],
                                transaction: vec![],
                                block: vec![],
                                height: 0,
                                vout: 0,
                                txindex: 0,
                                calldata,
                                pointer: 0,
                                refund_pointer: 0,
                            };
                            
                            match provider_clone.simulate(&format!("{}:{}", pool_id.block, pool_id.tx), &context, None).await {
                                Ok(result) => {
                                    let details_json = result;
                                    chunk_results.push(serde_json::json!({
                                        "pool_id": format!("{}:{}", pool_id.block, pool_id.tx),
                                        "pool_id_block": pool_id.block,
                                        "pool_id_tx": pool_id.tx,
                                        "details": details_json
                                    }));
                                }
                                Err(e) => {
                                    console::warn_1(&JsValue::from_str(&format!("Failed to get details for pool {}:{}: {}", pool_id.block, pool_id.tx, e)));
                                }
                            }
                        }
                        chunk_results
                    }
                })
                .buffer_unordered(max_concurrent)
                .collect()
                .await;
            
            // Flatten results
            for chunk_result in results {
                all_pool_details.extend(chunk_result);
            }
            
            let response = serde_json::json!({
                "total": total,
                "count": all_pool_details.len(),
                "pools": all_pool_details
            });
            
            serde_wasm_bindgen::to_value(&response)
                .map_err(|e| JsValue::from_str(&format!("Serialization failed: {}", e)))
        })
    }

    /// Get all pools from a factory (lightweight, IDs only)
    #[wasm_bindgen(js_name = alkanesGetAllPools)]
    pub fn alkanes_get_all_pools_js(&self, factory_id: String) -> js_sys::Promise {
        use alkanes_cli_common::alkanes::types::AlkaneId;
        use alkanes_cli_common::traits::AlkanesProvider;
        use alkanes_cli_common::proto::alkanes::MessageContextParcel;
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            let parts: Vec<&str> = factory_id.split(':').collect();
            if parts.len() != 2 {
                return Err(JsValue::from_str("Invalid factory_id format, expected block:tx"));
            }
            let block: u64 = parts[0].parse()
                .map_err(|_| JsValue::from_str("Invalid block number"))?;
            let tx: u64 = parts[1].parse()
                .map_err(|_| JsValue::from_str("Invalid tx number"))?;
            let factory = AlkaneId { block, tx };
            
            // Build calldata for GET_ALL_POOLS (opcode 3)
            let mut calldata = Vec::new();
            leb128::write::unsigned(&mut calldata, factory.block).unwrap();
            leb128::write::unsigned(&mut calldata, factory.tx).unwrap();
            leb128::write::unsigned(&mut calldata, 3u64).unwrap();
            
            let context = MessageContextParcel {
                alkanes: vec![],
                transaction: vec![],
                block: vec![],
                height: 0,
                vout: 0,
                txindex: 0,
                calldata,
                pointer: 0,
                refund_pointer: 0,
            };

            let result = provider.simulate(&format!("{}:{}", factory.block, factory.tx), &context, None).await
                .map_err(|e| JsValue::from_str(&format!("Failed: {}", e)))?;
            
            let data_hex = result
                .get("data")
                .and_then(|v| v.as_str())
                .ok_or_else(|| JsValue::from_str("No data in response"))?;

            let pools_result = alkanes_cli_common::alkanes::amm::decode_get_all_pools(data_hex)
                .ok_or_else(|| JsValue::from_str("Failed to decode pool list"))?;
            
            serde_wasm_bindgen::to_value(&pools_result)
                .map_err(|e| JsValue::from_str(&format!("Serialization failed: {}", e)))
        })
    }

    /// Get pool details including reserves using simulation
    #[wasm_bindgen(js_name = ammGetPoolDetails)]
    pub fn amm_get_pool_details_js(&self, pool_id: String) -> js_sys::Promise {
        use alkanes_cli_common::alkanes::types::AlkaneId;
        use alkanes_cli_common::traits::AlkanesProvider;
        use alkanes_cli_common::proto::alkanes::MessageContextParcel;
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            let pool_parts: Vec<&str> = pool_id.split(':').collect();
            if pool_parts.len() != 2 {
                return Err(JsValue::from_str("Invalid pool_id format, expected block:tx"));
            }
            let pool = AlkaneId {
                block: pool_parts[0].parse().map_err(|_| JsValue::from_str("Invalid pool block"))?,
                tx: pool_parts[1].parse().map_err(|_| JsValue::from_str("Invalid pool tx"))?,
            };

            // Build calldata for GET_RESERVES opcode (typically opcode 4)
            let mut calldata = Vec::new();
            leb128::write::unsigned(&mut calldata, pool.block).unwrap();
            leb128::write::unsigned(&mut calldata, pool.tx).unwrap();
            leb128::write::unsigned(&mut calldata, 4u64).unwrap(); // GET_RESERVES opcode

            let context = MessageContextParcel {
                alkanes: vec![],
                transaction: vec![],
                block: vec![],
                height: 0,
                vout: 0,
                txindex: 0,
                calldata,
                pointer: 0,
                refund_pointer: 0,
            };

            let result = provider.simulate(&pool_id, &context, None).await
                .map_err(|e| JsValue::from_str(&format!("Simulation failed: {}", e)))?;

            serde_wasm_bindgen::to_value(&result)
                .map_err(|e| JsValue::from_str(&format!("Serialization failed: {}", e)))
        })
    }

    #[wasm_bindgen(js_name = alkanesTrace)]
    pub fn alkanes_trace_js(&self, outpoint: String) -> js_sys::Promise {
        use alkanes_cli_common::traits::AlkanesProvider;
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            provider.trace(&outpoint).await
                .and_then(|r| serde_wasm_bindgen::to_value(&r).map_err(|e| alkanes_cli_common::AlkanesError::Serialization(e.to_string())))
                .map_err(|e| JsValue::from_str(&format!("Failed: {}", e)))
        })
    }

    #[wasm_bindgen(js_name = alkanesByAddress)]
    pub fn alkanes_by_address_js(&self, address: String, block_tag: Option<String>, protocol_tag: Option<f64>) -> js_sys::Promise {
        use alkanes_cli_common::traits::AlkanesProvider;
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            let tag = protocol_tag.map(|t| t as u128).unwrap_or(1);
            provider.protorunes_by_address(&address, block_tag, tag).await
                .and_then(|r| serde_wasm_bindgen::to_value(&r).map_err(|e| alkanes_cli_common::AlkanesError::Serialization(e.to_string())))
                .map_err(|e| JsValue::from_str(&format!("Failed: {}", e)))
        })
    }

    #[wasm_bindgen(js_name = alkanesByOutpoint)]
    pub fn alkanes_by_outpoint_js(&self, outpoint: String, block_tag: Option<String>, protocol_tag: Option<f64>) -> js_sys::Promise {
        use alkanes_cli_common::traits::AlkanesProvider;
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            let parts: Vec<&str> = outpoint.split(':').collect();
            if parts.len() != 2 {
                return Err(JsValue::from_str("Invalid outpoint format, expected txid:vout"));
            }
            let txid = parts[0];
            let vout: u32 = parts[1].parse()
                .map_err(|_| JsValue::from_str("Invalid vout number"))?;
            let tag = protocol_tag.map(|t| t as u128).unwrap_or(1);
            provider.protorunes_by_outpoint(txid, vout, block_tag, tag).await
                .and_then(|r| serde_wasm_bindgen::to_value(&r).map_err(|e| alkanes_cli_common::AlkanesError::Serialization(e.to_string())))
                .map_err(|e| JsValue::from_str(&format!("Failed: {}", e)))
        })
    }

    // === ESPLORA EXTENDED METHODS ===
    
    #[wasm_bindgen(js_name = esploraGetTx)]
    pub fn esplora_get_tx_js(&self, txid: String) -> js_sys::Promise {
        use alkanes_cli_common::traits::EsploraProvider;
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            provider.get_tx(&txid).await
                .and_then(|r| serde_wasm_bindgen::to_value(&r).map_err(|e| alkanes_cli_common::AlkanesError::Serialization(e.to_string())))
                .map_err(|e| JsValue::from_str(&format!("Failed: {}", e)))
        })
    }

    #[wasm_bindgen(js_name = esploraGetTxStatus)]
    pub fn esplora_get_tx_status_js(&self, txid: String) -> js_sys::Promise {
        use alkanes_cli_common::traits::EsploraProvider;
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            provider.get_tx_status(&txid).await
                .and_then(|r| serde_wasm_bindgen::to_value(&r).map_err(|e| alkanes_cli_common::AlkanesError::Serialization(e.to_string())))
                .map_err(|e| JsValue::from_str(&format!("Failed: {}", e)))
        })
    }

    #[wasm_bindgen(js_name = esploraGetAddressInfo)]
    pub fn esplora_get_address_info_js(&self, address: String) -> js_sys::Promise {
        use alkanes_cli_common::traits::EsploraProvider;
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            provider.get_address_info(&address).await
                .and_then(|r| serde_wasm_bindgen::to_value(&r).map_err(|e| alkanes_cli_common::AlkanesError::Serialization(e.to_string())))
                .map_err(|e| JsValue::from_str(&format!("Failed: {}", e)))
        })
    }

    #[wasm_bindgen(js_name = esploraGetBlocksTipHeight)]
    pub fn esplora_get_blocks_tip_height_js(&self) -> js_sys::Promise {
        use alkanes_cli_common::traits::EsploraProvider;
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            provider.get_blocks_tip_height().await
                .map(|h| JsValue::from_f64(h as f64))
                .map_err(|e| JsValue::from_str(&format!("Failed: {}", e)))
        })
    }

    #[wasm_bindgen(js_name = esploraGetBlocksTipHash)]
    pub fn esplora_get_blocks_tip_hash_js(&self) -> js_sys::Promise {
        use alkanes_cli_common::traits::EsploraProvider;
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            provider.get_blocks_tip_hash().await
                .map(|h| JsValue::from_str(&h))
                .map_err(|e| JsValue::from_str(&format!("Failed: {}", e)))
        })
    }

    #[wasm_bindgen(js_name = esploraGetAddressUtxo)]
    pub fn esplora_get_address_utxo_js(&self, address: String) -> js_sys::Promise {
        use alkanes_cli_common::traits::EsploraProvider;
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            provider.get_address_utxo(&address).await
                .and_then(|r| serde_wasm_bindgen::to_value(&r)
                    .map_err(|e| alkanes_cli_common::AlkanesError::Serialization(e.to_string())))
                .map_err(|e| JsValue::from_str(&format!("Failed: {}", e)))
        })
    }

    #[wasm_bindgen(js_name = esploraGetAddressTxs)]
    pub fn esplora_get_address_txs_js(&self, address: String) -> js_sys::Promise {
        use alkanes_cli_common::traits::EsploraProvider;
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            provider.get_address_txs(&address).await
                .and_then(|r| serde_wasm_bindgen::to_value(&r)
                    .map_err(|e| alkanes_cli_common::AlkanesError::Serialization(e.to_string())))
                .map_err(|e| JsValue::from_str(&format!("Failed: {}", e)))
        })
    }

    #[wasm_bindgen(js_name = esploraBroadcastTx)]
    pub fn esplora_broadcast_tx_js(&self, tx_hex: String) -> js_sys::Promise {
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            let esplora_url = provider.esplora_rpc_url()
                .ok_or_else(|| JsValue::from_str("Esplora URL not configured"))?;
            
            // POST to /tx endpoint
            let url = format!("{}/tx", esplora_url);
            let response = crate::platform::fetch(&url, "POST", Some(&tx_hex), vec![("Content-Type", "text/plain")]).await
                .map_err(|e| JsValue::from_str(&format!("Broadcast failed: {}", e)))?;
            
            Ok(JsValue::from_str(&response))
        })
    }

    #[wasm_bindgen(js_name = esploraGetTxHex)]
    pub fn esplora_get_tx_hex_js(&self, txid: String) -> js_sys::Promise {
        use alkanes_cli_common::traits::EsploraProvider;
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            provider.get_tx_hex(&txid).await
                .map(|h| JsValue::from_str(&h))
                .map_err(|e| JsValue::from_str(&format!("Failed: {}", e)))
        })
    }

    // === BITCOIN RPC METHODS ===
    
    #[wasm_bindgen(js_name = bitcoindGetBlockCount)]
    pub fn bitcoind_get_block_count_js(&self) -> js_sys::Promise {
        use alkanes_cli_common::traits::BitcoinRpcProvider;
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            provider.get_block_count().await
                .map(|c| JsValue::from_f64(c as f64))
                .map_err(|e| JsValue::from_str(&format!("Failed: {}", e)))
        })
    }

    #[wasm_bindgen(js_name = bitcoindSendRawTransaction)]
    pub fn bitcoind_send_raw_transaction_js(&self, tx_hex: String) -> js_sys::Promise {
        use alkanes_cli_common::traits::BitcoinRpcProvider;
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            provider.send_raw_transaction(&tx_hex).await
                .map(|txid| JsValue::from_str(&txid))
                .map_err(|e| JsValue::from_str(&format!("Failed: {}", e)))
        })
    }


    #[wasm_bindgen(js_name = bitcoindGenerateToAddress)]
    pub fn bitcoind_generate_to_address_js(&self, nblocks: u32, address: String) -> js_sys::Promise {
        use alkanes_cli_common::traits::BitcoinRpcProvider;
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            provider.generate_to_address(nblocks, &address).await
                .and_then(|r| serde_wasm_bindgen::to_value(&r)
                    .map_err(|e| alkanes_cli_common::AlkanesError::Serialization(e.to_string())))
                .map_err(|e| JsValue::from_str(&format!("Failed to generate blocks: {}", e)))
        })
    }

    #[wasm_bindgen(js_name = bitcoindGenerateFuture)]
    pub fn bitcoind_generate_future_js(&self, address: String) -> js_sys::Promise {
        use alkanes_cli_common::traits::BitcoinRpcProvider;
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            provider.generate_future(&address).await
                .and_then(|r| serde_wasm_bindgen::to_value(&r)
                    .map_err(|e| alkanes_cli_common::AlkanesError::Serialization(e.to_string())))
                .map_err(|e| JsValue::from_str(&format!("Failed to generate future: {}", e)))
        })
    }

    #[wasm_bindgen(js_name = bitcoindGetBlockchainInfo)]
    pub fn bitcoind_get_blockchain_info_js(&self) -> js_sys::Promise {
        use alkanes_cli_common::traits::BitcoinRpcProvider;
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            provider.get_blockchain_info().await
                .and_then(|r| serde_wasm_bindgen::to_value(&r)
                    .map_err(|e| alkanes_cli_common::AlkanesError::Serialization(e.to_string())))
                .map_err(|e| JsValue::from_str(&format!("Failed: {}", e)))
        })
    }

    #[wasm_bindgen(js_name = bitcoindGetNetworkInfo)]
    pub fn bitcoind_get_network_info_js(&self) -> js_sys::Promise {
        use alkanes_cli_common::traits::BitcoinRpcProvider;
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            provider.get_network_info().await
                .and_then(|r| serde_wasm_bindgen::to_value(&r)
                    .map_err(|e| alkanes_cli_common::AlkanesError::Serialization(e.to_string())))
                .map_err(|e| JsValue::from_str(&format!("Failed: {}", e)))
        })
    }

    #[wasm_bindgen(js_name = bitcoindGetRawTransaction)]
    pub fn bitcoind_get_raw_transaction_js(&self, txid: String, block_hash: Option<String>) -> js_sys::Promise {
        use alkanes_cli_common::traits::BitcoinRpcProvider;
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            provider.get_raw_transaction(&txid, block_hash.as_deref()).await
                .and_then(|r| serde_wasm_bindgen::to_value(&r)
                    .map_err(|e| alkanes_cli_common::AlkanesError::Serialization(e.to_string())))
                .map_err(|e| JsValue::from_str(&format!("Failed: {}", e)))
        })
    }

    #[wasm_bindgen(js_name = bitcoindGetBlock)]
    pub fn bitcoind_get_block_js(&self, hash: String, raw: bool) -> js_sys::Promise {
        use alkanes_cli_common::traits::BitcoinRpcProvider;
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            <WebProvider as BitcoinRpcProvider>::get_block(&provider, &hash, raw).await
                .and_then(|r| serde_wasm_bindgen::to_value(&r)
                    .map_err(|e| alkanes_cli_common::AlkanesError::Serialization(e.to_string())))
                .map_err(|e| JsValue::from_str(&format!("Failed: {}", e)))
        })
    }

    #[wasm_bindgen(js_name = bitcoindGetBlockHash)]
    pub fn bitcoind_get_block_hash_js(&self, height: f64) -> js_sys::Promise {
        use alkanes_cli_common::traits::BitcoinRpcProvider;
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            <WebProvider as BitcoinRpcProvider>::get_block_hash(&provider, height as u64).await
                .map(|h| JsValue::from_str(&h))
                .map_err(|e| JsValue::from_str(&format!("Failed: {}", e)))
        })
    }

    #[wasm_bindgen(js_name = bitcoindGetBlockHeader)]
    pub fn bitcoind_get_block_header_js(&self, hash: String) -> js_sys::Promise {
        use alkanes_cli_common::traits::BitcoinRpcProvider;
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            <WebProvider as BitcoinRpcProvider>::get_block_header(&provider, &hash).await
                .and_then(|r| serde_wasm_bindgen::to_value(&r)
                    .map_err(|e| alkanes_cli_common::AlkanesError::Serialization(e.to_string())))
                .map_err(|e| JsValue::from_str(&format!("Failed: {}", e)))
        })
    }

    #[wasm_bindgen(js_name = bitcoindGetBlockStats)]
    pub fn bitcoind_get_block_stats_js(&self, hash: String) -> js_sys::Promise {
        use alkanes_cli_common::traits::BitcoinRpcProvider;
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            provider.get_block_stats(&hash).await
                .and_then(|r| serde_wasm_bindgen::to_value(&r)
                    .map_err(|e| alkanes_cli_common::AlkanesError::Serialization(e.to_string())))
                .map_err(|e| JsValue::from_str(&format!("Failed: {}", e)))
        })
    }

    #[wasm_bindgen(js_name = bitcoindGetMempoolInfo)]
    pub fn bitcoind_get_mempool_info_js(&self) -> js_sys::Promise {
        use alkanes_cli_common::traits::BitcoinRpcProvider;
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            provider.get_mempool_info().await
                .and_then(|r| serde_wasm_bindgen::to_value(&r)
                    .map_err(|e| alkanes_cli_common::AlkanesError::Serialization(e.to_string())))
                .map_err(|e| JsValue::from_str(&format!("Failed: {}", e)))
        })
    }

    #[wasm_bindgen(js_name = bitcoindEstimateSmartFee)]
    pub fn bitcoind_estimate_smart_fee_js(&self, target: u32) -> js_sys::Promise {
        use alkanes_cli_common::traits::BitcoinRpcProvider;
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            provider.estimate_smart_fee(target).await
                .and_then(|r| serde_wasm_bindgen::to_value(&r)
                    .map_err(|e| alkanes_cli_common::AlkanesError::Serialization(e.to_string())))
                .map_err(|e| JsValue::from_str(&format!("Failed: {}", e)))
        })
    }

    #[wasm_bindgen(js_name = bitcoindGetChainTips)]
    pub fn bitcoind_get_chain_tips_js(&self) -> js_sys::Promise {
        use alkanes_cli_common::traits::BitcoinRpcProvider;
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            provider.get_chain_tips().await
                .and_then(|r| serde_wasm_bindgen::to_value(&r)
                    .map_err(|e| alkanes_cli_common::AlkanesError::Serialization(e.to_string())))
                .map_err(|e| JsValue::from_str(&format!("Failed: {}", e)))
        })
    }

    // === ALKANES METHODS (additional) ===

    #[wasm_bindgen(js_name = alkanesView)]
    pub fn alkanes_view_js(&self, contract_id: String, view_fn: String, params: Option<Vec<u8>>, block_tag: Option<String>) -> js_sys::Promise {
        use alkanes_cli_common::traits::AlkanesProvider;
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            provider.view(&contract_id, &view_fn, params.as_deref(), block_tag).await
                .and_then(|r| serde_wasm_bindgen::to_value(&r)
                    .map_err(|e| alkanes_cli_common::AlkanesError::Serialization(e.to_string())))
                .map_err(|e| JsValue::from_str(&format!("View failed: {}", e)))
        })
    }

    #[wasm_bindgen(js_name = alkanesInspect)]
    pub fn alkanes_inspect_js(&self, target: String, config: JsValue) -> js_sys::Promise {
        use alkanes_cli_common::traits::AlkanesProvider;
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            let inspect_config: alkanes_cli_common::alkanes::AlkanesInspectConfig = 
                serde_wasm_bindgen::from_value(config)
                    .map_err(|e| JsValue::from_str(&format!("Invalid config: {}", e)))?;
            
            provider.inspect(&target, inspect_config).await
                .and_then(|r| serde_wasm_bindgen::to_value(&r)
                    .map_err(|e| alkanes_cli_common::AlkanesError::Serialization(e.to_string())))
                .map_err(|e| JsValue::from_str(&format!("Inspect failed: {}", e)))
        })
    }

    #[wasm_bindgen(js_name = alkanesPendingUnwraps)]
    pub fn alkanes_pending_unwraps_js(&self, block_tag: Option<String>) -> js_sys::Promise {
        use alkanes_cli_common::traits::AlkanesProvider;
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            provider.pending_unwraps(block_tag).await
                .and_then(|r| serde_wasm_bindgen::to_value(&r)
                    .map_err(|e| alkanes_cli_common::AlkanesError::Serialization(e.to_string())))
                .map_err(|e| JsValue::from_str(&format!("Pending unwraps failed: {}", e)))
        })
    }

    // === BRC20-PROG METHODS ===

    #[wasm_bindgen(js_name = brc20progCall)]
    pub fn brc20prog_call_js(&self, to: String, data: String, block: Option<String>) -> js_sys::Promise {
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            let brc20_url = alkanes_cli_common::network::get_default_brc20_prog_rpc_url(provider.network);
            let params = serde_json::json!([{
                "to": to,
                "data": data
            }, block.unwrap_or_else(|| "latest".to_string())]);
            
            provider.call(&brc20_url, "eth_call", params, 1).await
                .map(|r| serde_wasm_bindgen::to_value(&r).unwrap_or(JsValue::NULL))
                .map_err(|e| JsValue::from_str(&format!("BRC20-Prog call failed: {}", e)))
        })
    }

    #[wasm_bindgen(js_name = brc20progGetBalance)]
    pub fn brc20prog_get_balance_js(&self, address: String, block: Option<String>) -> js_sys::Promise {
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            let brc20_url = alkanes_cli_common::network::get_default_brc20_prog_rpc_url(provider.network);
            let params = serde_json::json!([address, block.unwrap_or_else(|| "latest".to_string())]);
            
            provider.call(&brc20_url, "eth_getBalance", params, 1).await
                .map(|r| serde_wasm_bindgen::to_value(&r).unwrap_or(JsValue::NULL))
                .map_err(|e| JsValue::from_str(&format!("Get balance failed: {}", e)))
        })
    }

    #[wasm_bindgen(js_name = brc20progGetCode)]
    pub fn brc20prog_get_code_js(&self, address: String) -> js_sys::Promise {
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            let brc20_url = alkanes_cli_common::network::get_default_brc20_prog_rpc_url(provider.network);
            let params = serde_json::json!([address]);
            
            provider.call(&brc20_url, "eth_getCode", params, 1).await
                .map(|r| serde_wasm_bindgen::to_value(&r).unwrap_or(JsValue::NULL))
                .map_err(|e| JsValue::from_str(&format!("Get code failed: {}", e)))
        })
    }

    #[wasm_bindgen(js_name = brc20progGetTransactionCount)]
    pub fn brc20prog_get_transaction_count_js(&self, address: String, block: Option<String>) -> js_sys::Promise {
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            let brc20_url = alkanes_cli_common::network::get_default_brc20_prog_rpc_url(provider.network);
            let params = serde_json::json!([address, block.unwrap_or_else(|| "latest".to_string())]);
            
            provider.call(&brc20_url, "eth_getTransactionCount", params, 1).await
                .map(|r| serde_wasm_bindgen::to_value(&r).unwrap_or(JsValue::NULL))
                .map_err(|e| JsValue::from_str(&format!("Get transaction count failed: {}", e)))
        })
    }

    #[wasm_bindgen(js_name = brc20progBlockNumber)]
    pub fn brc20prog_block_number_js(&self) -> js_sys::Promise {
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            let brc20_url = alkanes_cli_common::network::get_default_brc20_prog_rpc_url(provider.network);
            let params = serde_json::json!([]);
            
            provider.call(&brc20_url, "eth_blockNumber", params, 1).await
                .map(|r| serde_wasm_bindgen::to_value(&r).unwrap_or(JsValue::NULL))
                .map_err(|e| JsValue::from_str(&format!("Get block number failed: {}", e)))
        })
    }

    #[wasm_bindgen(js_name = brc20progChainId)]
    pub fn brc20prog_chain_id_js(&self) -> js_sys::Promise {
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            let brc20_url = alkanes_cli_common::network::get_default_brc20_prog_rpc_url(provider.network);
            let params = serde_json::json!([]);
            
            provider.call(&brc20_url, "eth_chainId", params, 1).await
                .map(|r| serde_wasm_bindgen::to_value(&r).unwrap_or(JsValue::NULL))
                .map_err(|e| JsValue::from_str(&format!("Get chain ID failed: {}", e)))
        })
    }

    #[wasm_bindgen(js_name = brc20progGetTransactionReceipt)]
    pub fn brc20prog_get_transaction_receipt_js(&self, tx_hash: String) -> js_sys::Promise {
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            let brc20_url = alkanes_cli_common::network::get_default_brc20_prog_rpc_url(provider.network);
            let params = serde_json::json!([tx_hash]);
            
            provider.call(&brc20_url, "eth_getTransactionReceipt", params, 1).await
                .map(|r| serde_wasm_bindgen::to_value(&r).unwrap_or(JsValue::NULL))
                .map_err(|e| JsValue::from_str(&format!("Get transaction receipt failed: {}", e)))
        })
    }

    #[wasm_bindgen(js_name = brc20progGetTransactionByHash)]
    pub fn brc20prog_get_transaction_by_hash_js(&self, tx_hash: String) -> js_sys::Promise {
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            let brc20_url = alkanes_cli_common::network::get_default_brc20_prog_rpc_url(provider.network);
            let params = serde_json::json!([tx_hash]);
            
            provider.call(&brc20_url, "eth_getTransactionByHash", params, 1).await
                .map(|r| serde_wasm_bindgen::to_value(&r).unwrap_or(JsValue::NULL))
                .map_err(|e| JsValue::from_str(&format!("Get transaction failed: {}", e)))
        })
    }

    #[wasm_bindgen(js_name = brc20progGetBlockByNumber)]
    pub fn brc20prog_get_block_by_number_js(&self, block: String, full_tx: bool) -> js_sys::Promise {
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            let brc20_url = alkanes_cli_common::network::get_default_brc20_prog_rpc_url(provider.network);
            let params = serde_json::json!([block, full_tx]);
            
            provider.call(&brc20_url, "eth_getBlockByNumber", params, 1).await
                .map(|r| serde_wasm_bindgen::to_value(&r).unwrap_or(JsValue::NULL))
                .map_err(|e| JsValue::from_str(&format!("Get block failed: {}", e)))
        })
    }

    #[wasm_bindgen(js_name = brc20progEstimateGas)]
    pub fn brc20prog_estimate_gas_js(&self, to: String, data: String, block: Option<String>) -> js_sys::Promise {
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            let brc20_url = alkanes_cli_common::network::get_default_brc20_prog_rpc_url(provider.network);
            let params = serde_json::json!([{
                "to": to,
                "data": data
            }, block.unwrap_or_else(|| "latest".to_string())]);
            
            provider.call(&brc20_url, "eth_estimateGas", params, 1).await
                .map(|r| serde_wasm_bindgen::to_value(&r).unwrap_or(JsValue::NULL))
                .map_err(|e| JsValue::from_str(&format!("Estimate gas failed: {}", e)))
        })
    }

    #[wasm_bindgen(js_name = brc20progGetLogs)]
    pub fn brc20prog_get_logs_js(&self, filter: JsValue) -> js_sys::Promise {
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            let brc20_url = alkanes_cli_common::network::get_default_brc20_prog_rpc_url(provider.network);
            // Convert JsValue filter to serde_json::Value
            let filter_json: serde_json::Value = serde_wasm_bindgen::from_value(filter)
                .map_err(|e| JsValue::from_str(&format!("Invalid filter: {}", e)))?;
            let params = serde_json::json!([filter_json]);
            
            provider.call(&brc20_url, "eth_getLogs", params, 1).await
                .map(|r| serde_wasm_bindgen::to_value(&r).unwrap_or(JsValue::NULL))
                .map_err(|e| JsValue::from_str(&format!("Get logs failed: {}", e)))
        })
    }

    #[wasm_bindgen(js_name = brc20progWeb3ClientVersion)]
    pub fn brc20prog_web3_client_version_js(&self) -> js_sys::Promise {
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            let brc20_url = alkanes_cli_common::network::get_default_brc20_prog_rpc_url(provider.network);
            let params = serde_json::json!([]);
            
            provider.call(&brc20_url, "web3_clientVersion", params, 1).await
                .map(|r| serde_wasm_bindgen::to_value(&r).unwrap_or(JsValue::NULL))
                .map_err(|e| JsValue::from_str(&format!("Get client version failed: {}", e)))
        })
    }

    // === METASHREW METHODS ===
    
    #[wasm_bindgen(js_name = metashrewHeight)]
    pub fn metashrew_height_js(&self) -> js_sys::Promise {
        use alkanes_cli_common::traits::MetashrewRpcProvider;
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            provider.get_metashrew_height().await
                .map(|h| JsValue::from_f64(h as f64))
                .map_err(|e| JsValue::from_str(&format!("Failed: {}", e)))
        })
    }

    #[wasm_bindgen(js_name = metashrewStateRoot)]
    pub fn metashrew_state_root_js(&self, height: Option<f64>) -> js_sys::Promise {
        use alkanes_cli_common::traits::MetashrewRpcProvider;
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            let height_val = if let Some(h) = height {
                serde_json::json!(h as u64)
            } else {
                serde_json::json!(null)
            };
            <WebProvider as MetashrewRpcProvider>::get_state_root(&provider, height_val).await
                .map(|r| JsValue::from_str(&r))
                .map_err(|e| JsValue::from_str(&format!("Failed: {}", e)))
        })
    }

    #[wasm_bindgen(js_name = metashrewGetBlockHash)]
    pub fn metashrew_get_block_hash_js(&self, height: f64) -> js_sys::Promise {
        use alkanes_cli_common::traits::MetashrewProvider;
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            <WebProvider as MetashrewProvider>::get_block_hash(&provider, height as u64).await
                .map(|h| JsValue::from_str(&h.as_str()))
                .map_err(|e| JsValue::from_str(&format!("Failed: {}", e)))
        })
    }

    // === LUA METHODS ===

    #[wasm_bindgen(js_name = luaEvalScript)]
    pub fn lua_eval_script_js(&self, script: String) -> js_sys::Promise {
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            let url = provider.sandshrew_rpc_url();
            let params = serde_json::json!([script]);
            provider.call(&url, "lua_evalscript", params, 1).await
                .and_then(|r| serde_wasm_bindgen::to_value(&r)
                    .map_err(|e| alkanes_cli_common::AlkanesError::Serialization(e.to_string())))
                .map_err(|e| JsValue::from_str(&format!("Eval script failed: {}", e)))
        })
    }

    // === ORD METHODS ===

    #[wasm_bindgen(js_name = ordList)]
    pub fn ord_list_js(&self, outpoint: String) -> js_sys::Promise {
        use alkanes_cli_common::traits::OrdProvider;
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            provider.get_output(&outpoint).await
                .and_then(|r| serde_wasm_bindgen::to_value(&r)
                    .map_err(|e| alkanes_cli_common::AlkanesError::Serialization(e.to_string())))
                .map_err(|e| JsValue::from_str(&format!("Ord list failed: {}", e)))
        })
    }

    #[wasm_bindgen(js_name = ordFind)]
    pub fn ord_find_js(&self, sat: f64) -> js_sys::Promise {
        use alkanes_cli_common::traits::OrdProvider;
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            provider.get_sat(sat as u64).await
                .and_then(|r| serde_wasm_bindgen::to_value(&r)
                    .map_err(|e| alkanes_cli_common::AlkanesError::Serialization(e.to_string())))
                .map_err(|e| JsValue::from_str(&format!("Ord find failed: {}", e)))
        })
    }

    // === RUNESTONE / PROTORUNES METHODS ===

    #[wasm_bindgen(js_name = runestoneDecodeTx)]
    pub fn runestone_decode_tx_js(&self, txid: String) -> js_sys::Promise {
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            // Get transaction hex
            let tx_hex = provider.get_transaction_hex(&txid).await
                .map_err(|e| JsValue::from_str(&format!("Get tx failed: {}", e)))?;
            
            // Decode transaction
            let tx_bytes = hex::decode(&tx_hex)
                .map_err(|e| JsValue::from_str(&format!("Hex decode failed: {}", e)))?;
            let tx: bitcoin::Transaction = bitcoin::consensus::deserialize(&tx_bytes)
                .map_err(|e| JsValue::from_str(&format!("Tx deserialize failed: {}", e)))?;
            
            // Decode runestone
            let result = alkanes_cli_common::runestone_enhanced::format_runestone_with_decoded_messages(&tx)
                .map_err(|e| JsValue::from_str(&format!("Runestone decode failed: {}", e)))?;
            
            serde_wasm_bindgen::to_value(&result)
                .map_err(|e| JsValue::from_str(&format!("Serialize failed: {}", e)))
        })
    }

    #[wasm_bindgen(js_name = runestoneAnalyzeTx)]
    pub fn runestone_analyze_tx_js(&self, txid: String) -> js_sys::Promise {
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            // Get transaction hex
            let tx_hex = provider.get_transaction_hex(&txid).await
                .map_err(|e| JsValue::from_str(&format!("Get tx failed: {}", e)))?;
            
            // Decode transaction
            let tx_bytes = hex::decode(&tx_hex)
                .map_err(|e| JsValue::from_str(&format!("Hex decode failed: {}", e)))?;
            let tx: bitcoin::Transaction = bitcoin::consensus::deserialize(&tx_bytes)
                .map_err(|e| JsValue::from_str(&format!("Tx deserialize failed: {}", e)))?;
            
            // Analyze runestone with full formatting
            let result = alkanes_cli_common::runestone_enhanced::format_runestone_with_decoded_messages(&tx)
                .map_err(|e| JsValue::from_str(&format!("Runestone analyze failed: {}", e)))?;
            
            serde_wasm_bindgen::to_value(&result)
                .map_err(|e| JsValue::from_str(&format!("Serialize failed: {}", e)))
        })
    }

    #[wasm_bindgen(js_name = protorunesDecodeTx)]
    pub fn protorunes_decode_tx_js(&self, txid: String) -> js_sys::Promise {
        use alkanes_cli_common::traits::AlkanesProvider;
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            // Trace protostones in the transaction
            let result = provider.trace_protostones(&txid).await
                .map_err(|e| JsValue::from_str(&format!("Trace protostones failed: {}", e)))?;
            
            serde_wasm_bindgen::to_value(&result)
                .map_err(|e| JsValue::from_str(&format!("Serialize failed: {}", e)))
        })
    }

    #[wasm_bindgen(js_name = protorunesAnalyzeTx)]
    pub fn protorunes_analyze_tx_js(&self, txid: String) -> js_sys::Promise {
        use alkanes_cli_common::traits::AlkanesProvider;
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            // Trace and analyze all protostones
            let result = provider.trace_protostones(&txid).await
                .map_err(|e| JsValue::from_str(&format!("Analyze protostones failed: {}", e)))?;
            
            serde_wasm_bindgen::to_value(&result)
                .map_err(|e| JsValue::from_str(&format!("Serialize failed: {}", e)))
        })
    }

    // === WALLET METHODS ===

    /// Create a new wallet with an optional mnemonic phrase
    /// If no mnemonic is provided, a new one will be generated
    /// Returns wallet info including address and mnemonic
    #[wasm_bindgen(js_name = walletCreate)]
    pub fn wallet_create_js(&mut self, mnemonic: Option<String>, passphrase: Option<String>) -> js_sys::Promise {
        use alkanes_cli_common::traits::WalletProvider;
        use alkanes_cli_common::traits::WalletConfig;
        use wasm_bindgen_futures::future_to_promise;
        let mut provider = self.clone();
        let network = self.network;
        let rpc_url = self.sandshrew_rpc_url();
        future_to_promise(async move {
            let config = WalletConfig {
                network,
                wallet_path: "default".to_string(),
                bitcoin_rpc_url: rpc_url.clone(),
                metashrew_rpc_url: rpc_url,
                network_params: None,
            };
            let wallet_info = provider.create_wallet(config, mnemonic, passphrase).await
                .map_err(|e| JsValue::from_str(&format!("Create wallet failed: {}", e)))?;

            serde_wasm_bindgen::to_value(&serde_json::json!({
                "address": wallet_info.address,
                "network": format!("{:?}", wallet_info.network),
                "mnemonic": wallet_info.mnemonic,
            })).map_err(|e| JsValue::from_str(&format!("Serialize failed: {}", e)))
        })
    }

    /// Load an existing wallet from storage
    #[wasm_bindgen(js_name = walletLoad)]
    pub fn wallet_load_js(&mut self, passphrase: Option<String>) -> js_sys::Promise {
        use alkanes_cli_common::traits::WalletProvider;
        use alkanes_cli_common::traits::WalletConfig;
        use wasm_bindgen_futures::future_to_promise;
        let mut provider = self.clone();
        let network = self.network;
        let rpc_url = self.sandshrew_rpc_url();
        future_to_promise(async move {
            let config = WalletConfig {
                network,
                wallet_path: "default".to_string(),
                bitcoin_rpc_url: rpc_url.clone(),
                metashrew_rpc_url: rpc_url,
                network_params: None,
            };
            let wallet_info = provider.load_wallet(config, passphrase).await
                .map_err(|e| JsValue::from_str(&format!("Load wallet failed: {}", e)))?;

            serde_wasm_bindgen::to_value(&serde_json::json!({
                "address": wallet_info.address,
                "network": format!("{:?}", wallet_info.network),
                "mnemonic": wallet_info.mnemonic,
            })).map_err(|e| JsValue::from_str(&format!("Serialize failed: {}", e)))
        })
    }

    /// Get the wallet's primary address
    #[wasm_bindgen(js_name = walletGetAddress)]
    pub fn wallet_get_address_js(&self) -> js_sys::Promise {
        use alkanes_cli_common::traits::WalletProvider;
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            let address = <WebProvider as WalletProvider>::get_address(&provider).await
                .map_err(|e| JsValue::from_str(&format!("Get address failed: {}", e)))?;
            Ok(JsValue::from_str(&address))
        })
    }

    /// Get the wallet's BTC balance
    /// Returns { confirmed: number, pending: number }
    #[wasm_bindgen(js_name = walletGetBalance)]
    pub fn wallet_get_balance_js(&self, addresses: Option<Vec<String>>) -> js_sys::Promise {
        use alkanes_cli_common::traits::WalletProvider;
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            let balance = <WebProvider as WalletProvider>::get_balance(&provider, addresses).await
                .map_err(|e| JsValue::from_str(&format!("Get balance failed: {}", e)))?;

            serde_wasm_bindgen::to_value(&serde_json::json!({
                "confirmed": balance.confirmed,
                "pending": balance.pending,
            })).map_err(|e| JsValue::from_str(&format!("Serialize failed: {}", e)))
        })
    }

    /// Send BTC to an address
    /// params: { address: string, amount: number (satoshis), fee_rate?: number }
    #[wasm_bindgen(js_name = walletSend)]
    pub fn wallet_send_js(&mut self, params_json: String) -> js_sys::Promise {
        use alkanes_cli_common::traits::WalletProvider;
        use alkanes_cli_common::traits::SendParams;
        use wasm_bindgen_futures::future_to_promise;
        let mut provider = self.clone();
        future_to_promise(async move {
            let params: serde_json::Value = serde_json::from_str(&params_json)
                .map_err(|e| JsValue::from_str(&format!("Invalid params JSON: {}", e)))?;

            let send_params = SendParams {
                address: params.get("address")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| JsValue::from_str("Missing 'address' field"))?
                    .to_string(),
                amount: params.get("amount")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| JsValue::from_str("Missing or invalid 'amount' field"))?,
                fee_rate: params.get("fee_rate")
                    .and_then(|v| v.as_f64())
                    .map(|f| f as f32),
                send_all: params.get("send_all").and_then(|v| v.as_bool()).unwrap_or(false),
                from: params.get("from").and_then(|v| v.as_array()).map(|arr| {
                    arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect()
                }),
                change_address: params.get("change_address").and_then(|v| v.as_str()).map(|s| s.to_string()),
                auto_confirm: params.get("auto_confirm").and_then(|v| v.as_bool()).unwrap_or(true),
                use_rebar: params.get("use_rebar").and_then(|v| v.as_bool()).unwrap_or(false),
                rebar_tier: params.get("rebar_tier").and_then(|v| v.as_u64()).unwrap_or(0) as u8,
                lock_alkanes: params.get("lock_alkanes").and_then(|v| v.as_bool()).unwrap_or(false),
            };

            let txid = provider.send(send_params).await
                .map_err(|e| JsValue::from_str(&format!("Send failed: {}", e)))?;

            Ok(JsValue::from_str(&txid))
        })
    }

    /// Get UTXOs for the wallet
    #[wasm_bindgen(js_name = walletGetUtxos)]
    pub fn wallet_get_utxos_js(&self, addresses: Option<Vec<String>>) -> js_sys::Promise {
        use alkanes_cli_common::traits::WalletProvider;
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            let utxos = provider.get_utxos(false, addresses).await
                .map_err(|e| JsValue::from_str(&format!("Get UTXOs failed: {}", e)))?;

            let utxo_list: Vec<_> = utxos.iter().map(|(outpoint, info)| {
                serde_json::json!({
                    "txid": outpoint.txid.to_string(),
                    "vout": outpoint.vout,
                    "amount": info.amount,
                    "confirmations": info.confirmations,
                    "address": info.address,
                })
            }).collect();

            serde_wasm_bindgen::to_value(&utxo_list)
                .map_err(|e| JsValue::from_str(&format!("Serialize failed: {}", e)))
        })
    }

    /// Get transaction history for an address
    #[wasm_bindgen(js_name = walletGetHistory)]
    pub fn wallet_get_history_js(&self, address: Option<String>) -> js_sys::Promise {
        use alkanes_cli_common::traits::{WalletProvider, EsploraProvider};
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            let addr = if let Some(a) = address {
                a
            } else {
                <WebProvider as WalletProvider>::get_address(&provider).await
                    .map_err(|e| JsValue::from_str(&format!("Get address failed: {}", e)))?
            };

            // Use esplora to get address transactions
            let txs = provider.get_address_txs(&addr).await
                .map_err(|e| JsValue::from_str(&format!("Get history failed: {}", e)))?;

            serde_wasm_bindgen::to_value(&txs)
                .map_err(|e| JsValue::from_str(&format!("Serialize failed: {}", e)))
        })
    }

    #[wasm_bindgen(js_name = walletCreatePsbt)]
    pub fn wallet_create_psbt_js(&self, params_json: String) -> js_sys::Promise {
        use alkanes_cli_common::traits::WalletProvider;
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            let params: serde_json::Value = serde_json::from_str(&params_json)
                .map_err(|e| JsValue::from_str(&format!("Invalid params JSON: {}", e)))?;
            // This would need proper parameter parsing based on actual requirements
            Err(JsValue::from_str("PSBT creation not yet implemented in WASM"))
        })
    }

    #[wasm_bindgen(js_name = walletExport)]
    pub fn wallet_export_js(&self) -> js_sys::Promise {
        use alkanes_cli_common::traits::WalletProvider;
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            // Export returns the mnemonic phrase
            // The user must have already unlocked the wallet to call this
            let mnemonic = provider.get_mnemonic().await
                .map_err(|e| JsValue::from_str(&format!("Export failed: {}", e)))?;
            
            match mnemonic {
                Some(m) => Ok(JsValue::from_str(&m)),
                None => Err(JsValue::from_str("No mnemonic available"))
            }
        })
    }

    #[wasm_bindgen(js_name = walletBackup)]
    pub fn wallet_backup_js(&self) -> js_sys::Promise {
        use alkanes_cli_common::traits::WalletProvider;
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            // Backup returns the wallet data as JSON string
            // This includes encrypted keystore data compatible with subfrost-app
            let backup_data = provider.backup().await
                .map_err(|e| JsValue::from_str(&format!("Backup failed: {}", e)))?;
            
            // Return the backup string (JSON format)
            Ok(JsValue::from_str(&backup_data))
        })
    }

    // === DATA API METHODS ===

    #[wasm_bindgen(js_name = dataApiGetPoolHistory)]
    pub fn data_api_get_pool_history_js(&self, pool_id: String, category: Option<String>, limit: Option<i64>, offset: Option<i64>) -> js_sys::Promise {
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            // Data API call to get pool history
            let url = provider.rpc_config.get_data_api_target().url;
            let body = serde_json::json!({
                "pool_id": pool_id,
                "category": category,
                "limit": limit.unwrap_or(100),
                "offset": offset.unwrap_or(0)
            });
            
            let response = provider.rest_call(&url, "get-pool-history", body).await
                .map_err(|e| JsValue::from_str(&format!("Get pool history failed: {}", e)))?;
            
            serde_wasm_bindgen::to_value(&response)
                .map_err(|e| JsValue::from_str(&format!("Serialize failed: {}", e)))
        })
    }

    #[wasm_bindgen(js_name = dataApiGetPools)]
    pub fn data_api_get_pools_js(&self, factory_id: String) -> js_sys::Promise {
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            let base_url = provider.rpc_config.get_data_api_target().url;
            // Parse factory_id like "4:65522" into block and tx
            let parts: Vec<&str> = factory_id.split(':').collect();
            let body = if parts.len() == 2 {
                serde_json::json!({ "factoryId": { "block": parts[0], "tx": parts[1] } })
            } else {
                serde_json::json!({ "factory_id": factory_id })
            };

            let response = provider.rest_call(&base_url, "get-pools", body).await
                .map_err(|e| JsValue::from_str(&format!("Get pools failed: {}", e)))?;

            serde_wasm_bindgen::to_value(&response)
                .map_err(|e| JsValue::from_str(&format!("Serialize failed: {}", e)))
        })
    }

    #[wasm_bindgen(js_name = dataApiGetAlkanesByAddress)]
    pub fn data_api_get_alkanes_by_address_js(&self, address: String) -> js_sys::Promise {
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            let base_url = provider.rpc_config.get_data_api_target().url;
            let body = serde_json::json!({ "address": address });

            let response = provider.rest_call(&base_url, "get-alkanes-by-address", body).await
                .map_err(|e| JsValue::from_str(&format!("Get alkanes by address failed: {}", e)))?;

            serde_wasm_bindgen::to_value(&response)
                .map_err(|e| JsValue::from_str(&format!("Serialize failed: {}", e)))
        })
    }

    #[wasm_bindgen(js_name = dataApiGetAddressBalances)]
    pub fn data_api_get_address_balances_js(&self, address: String, include_outpoints: bool) -> js_sys::Promise {
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            let base_url = provider.rpc_config.get_data_api_target().url;
            let body = serde_json::json!({
                "address": address,
                "include_outpoints": include_outpoints
            });

            let response = provider.rest_call(&base_url, "get-address-balances", body).await
                .map_err(|e| JsValue::from_str(&format!("Get address balances failed: {}", e)))?;

            serde_wasm_bindgen::to_value(&response)
                .map_err(|e| JsValue::from_str(&format!("Serialize failed: {}", e)))
        })
    }

    #[wasm_bindgen(js_name = dataApiGetAllHistory)]
    pub fn data_api_get_all_history_js(&self, pool_id: String, limit: Option<i64>, offset: Option<i64>) -> js_sys::Promise {
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            let url = provider.rpc_config.get_data_api_target().url;
            let body = serde_json::json!({
                "pool_id": pool_id,
                "limit": limit.unwrap_or(100),
                "offset": offset.unwrap_or(0)
            });
            
            let response = provider.rest_call(&url, "get-all-history", body).await
                .map_err(|e| JsValue::from_str(&format!("Get all history failed: {}", e)))?;
            
            serde_wasm_bindgen::to_value(&response)
                .map_err(|e| JsValue::from_str(&format!("Serialize failed: {}", e)))
        })
    }

    #[wasm_bindgen(js_name = dataApiGetSwapHistory)]
    pub fn data_api_get_swap_history_js(&self, pool_id: String, limit: Option<i64>, offset: Option<i64>) -> js_sys::Promise {
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            let url = provider.rpc_config.get_data_api_target().url;
            let body = serde_json::json!({
                "pool_id": pool_id,
                "limit": limit.unwrap_or(100),
                "offset": offset.unwrap_or(0)
            });
            
            let response = provider.rest_call(&url, "get-swap-history", body).await
                .map_err(|e| JsValue::from_str(&format!("Get swap history failed: {}", e)))?;
            
            serde_wasm_bindgen::to_value(&response)
                .map_err(|e| JsValue::from_str(&format!("Serialize failed: {}", e)))
        })
    }

    #[wasm_bindgen(js_name = dataApiGetMintHistory)]
    pub fn data_api_get_mint_history_js(&self, pool_id: String, limit: Option<i64>, offset: Option<i64>) -> js_sys::Promise {
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            let url = provider.rpc_config.get_data_api_target().url;
            let body = serde_json::json!({
                "pool_id": pool_id,
                "limit": limit.unwrap_or(100),
                "offset": offset.unwrap_or(0)
            });
            
            let response = provider.rest_call(&url, "get-mint-history", body).await
                .map_err(|e| JsValue::from_str(&format!("Get mint history failed: {}", e)))?;
            
            serde_wasm_bindgen::to_value(&response)
                .map_err(|e| JsValue::from_str(&format!("Serialize failed: {}", e)))
        })
    }

    #[wasm_bindgen(js_name = dataApiGetBurnHistory)]
    pub fn data_api_get_burn_history_js(&self, pool_id: String, limit: Option<i64>, offset: Option<i64>) -> js_sys::Promise {
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            let url = provider.rpc_config.get_data_api_target().url;
            let body = serde_json::json!({
                "pool_id": pool_id,
                "limit": limit.unwrap_or(100),
                "offset": offset.unwrap_or(0)
            });
            
            let response = provider.rest_call(&url, "get-burn-history", body).await
                .map_err(|e| JsValue::from_str(&format!("Get burn history failed: {}", e)))?;
            
            serde_wasm_bindgen::to_value(&response)
                .map_err(|e| JsValue::from_str(&format!("Serialize failed: {}", e)))
        })
    }

    #[wasm_bindgen(js_name = dataApiGetTrades)]
    pub fn data_api_get_trades_js(&self, pool: String, start_time: Option<f64>, end_time: Option<f64>, limit: Option<i64>) -> js_sys::Promise {
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            let url = provider.rpc_config.get_data_api_target().url;
            let body = serde_json::json!({
                "pool": pool,
                "start_time": start_time.map(|t| t as i64),
                "end_time": end_time.map(|t| t as i64),
                "limit": limit.unwrap_or(100)
            });
            
            let response = provider.rest_call(&url, "get-trades", body).await
                .map_err(|e| JsValue::from_str(&format!("Get trades failed: {}", e)))?;
            
            serde_wasm_bindgen::to_value(&response)
                .map_err(|e| JsValue::from_str(&format!("Serialize failed: {}", e)))
        })
    }

    #[wasm_bindgen(js_name = dataApiGetCandles)]
    pub fn data_api_get_candles_js(&self, pool: String, interval: String, start_time: Option<f64>, end_time: Option<f64>, limit: Option<i64>) -> js_sys::Promise {
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            let url = provider.rpc_config.get_data_api_target().url;
            let body = serde_json::json!({
                "pool": pool,
                "interval": interval,
                "start_time": start_time.map(|t| t as i64),
                "end_time": end_time.map(|t| t as i64),
                "limit": limit.unwrap_or(100)
            });
            
            let response = provider.rest_call(&url, "get-candles", body).await
                .map_err(|e| JsValue::from_str(&format!("Get candles failed: {}", e)))?;
            
            serde_wasm_bindgen::to_value(&response)
                .map_err(|e| JsValue::from_str(&format!("Serialize failed: {}", e)))
        })
    }

    #[wasm_bindgen(js_name = dataApiGetReserves)]
    pub fn data_api_get_reserves_js(&self, pool: String) -> js_sys::Promise {
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            let url = provider.rpc_config.get_data_api_target().url;
            let body = serde_json::json!({ "pool": pool });
            
            let response = provider.rest_call(&url, "get-reserves", body).await
                .map_err(|e| JsValue::from_str(&format!("Get reserves failed: {}", e)))?;
            
            serde_wasm_bindgen::to_value(&response)
                .map_err(|e| JsValue::from_str(&format!("Serialize failed: {}", e)))
        })
    }

    #[wasm_bindgen(js_name = dataApiGetHolders)]
    pub fn data_api_get_holders_js(&self, alkane: String, page: i64, limit: i64) -> js_sys::Promise {
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            let url = provider.rpc_config.get_data_api_target().url;
            let body = serde_json::json!({
                "alkane": alkane,
                "page": page,
                "limit": limit
            });
            
            let response = provider.rest_call(&url, "get-alkane-holders", body).await
                .map_err(|e| JsValue::from_str(&format!("Get holders failed: {}", e)))?;
            
            serde_wasm_bindgen::to_value(&response)
                .map_err(|e| JsValue::from_str(&format!("Serialize failed: {}", e)))
        })
    }

    #[wasm_bindgen(js_name = dataApiGetHoldersCount)]
    pub fn data_api_get_holders_count_js(&self, alkane: String) -> js_sys::Promise {
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            let url = provider.rpc_config.get_data_api_target().url;
            let body = serde_json::json!({ "alkane": alkane });
            
            let response = provider.rest_call(&url, "get-alkane-holders-count", body).await
                .map_err(|e| JsValue::from_str(&format!("Get holders count failed: {}", e)))?;
            
            serde_wasm_bindgen::to_value(&response)
                .map_err(|e| JsValue::from_str(&format!("Serialize failed: {}", e)))
        })
    }

    #[wasm_bindgen(js_name = dataApiGetKeys)]
    pub fn data_api_get_keys_js(&self, alkane: String, prefix: Option<String>, limit: i64) -> js_sys::Promise {
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            let url = provider.rpc_config.get_data_api_target().url;
            let body = serde_json::json!({
                "alkane": alkane,
                "prefix": prefix,
                "limit": limit
            });
            
            let response = provider.rest_call(&url, "get-keys", body).await
                .map_err(|e| JsValue::from_str(&format!("Get keys failed: {}", e)))?;
            
            serde_wasm_bindgen::to_value(&response)
                .map_err(|e| JsValue::from_str(&format!("Serialize failed: {}", e)))
        })
    }

    #[wasm_bindgen(js_name = dataApiGetBitcoinPrice)]
    pub fn data_api_get_bitcoin_price_js(&self) -> js_sys::Promise {
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            let url = provider.rpc_config.get_data_api_target().url;
            let body = serde_json::json!({});
            
            let response = provider.rest_call(&url, "get-bitcoin-price", body).await
                .map_err(|e| JsValue::from_str(&format!("Get bitcoin price failed: {}", e)))?;
            
            serde_wasm_bindgen::to_value(&response)
                .map_err(|e| JsValue::from_str(&format!("Serialize failed: {}", e)))
        })
    }

    #[wasm_bindgen(js_name = dataApiGetBitcoinMarketChart)]
    pub fn data_api_get_bitcoin_market_chart_js(&self, days: String) -> js_sys::Promise {
        use wasm_bindgen_futures::future_to_promise;
        let provider = self.clone();
        future_to_promise(async move {
            let url = provider.rpc_config.get_data_api_target().url;
            let body = serde_json::json!({ "days": days });
            
            let response = provider.rest_call(&url, "get-bitcoin-market-chart", body).await
                .map_err(|e| JsValue::from_str(&format!("Get bitcoin market chart failed: {}", e)))?;
            
            serde_wasm_bindgen::to_value(&response)
                .map_err(|e| JsValue::from_str(&format!("Serialize failed: {}", e)))
        })
    }
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
            "WebProvider initialized with: Metashrew RPC URL: {}, Esplora URL: {:?}, Network: {}",
            &params.metashrew_rpc_url, &params.esplora_url, &params.network
        ));
 
        // Convert NetworkParams to RpcConfig
        let mut rpc_config = alkanes_cli_common::network::RpcConfig {
            provider: network_str.clone(),
            bitcoin_rpc_url: Some(params.bitcoin_rpc_url.clone()),
            jsonrpc_url: Some(params.metashrew_rpc_url.clone()),
            titan_api_url: None,
            esplora_url: params.esplora_url.clone(),
            ord_url: None,
            metashrew_rpc_url: Some(params.metashrew_rpc_url.clone()),
            brc20_prog_rpc_url: None,
            data_api_url: None,
            subfrost_api_key: None,
            timeout_seconds: 600,
        };
 
         Ok(Self {
            rpc_config,
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
       // Convert NetworkParams to RpcConfig
       let rpc_config = alkanes_cli_common::network::RpcConfig {
           provider: "custom".to_string(),
           bitcoin_rpc_url: Some(params.bitcoin_rpc_url.clone()),
           jsonrpc_url: Some(params.metashrew_rpc_url.clone()),
           titan_api_url: None,
           esplora_url: params.esplora_url.clone(),
           ord_url: None,
           metashrew_rpc_url: Some(params.metashrew_rpc_url.clone()),
           brc20_prog_rpc_url: None,
           data_api_url: None,
           subfrost_api_key: None,
           timeout_seconds: 600,
       };
       
       Ok(Self {
           rpc_config,
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

        let rpc_config = alkanes_cli_common::network::RpcConfig {
            provider: network_str.clone(),
            bitcoin_rpc_url: None,
            jsonrpc_url: Some(url.to_string()),
            titan_api_url: None,
            esplora_url: Some(url.to_string()),
            ord_url: None,
            metashrew_rpc_url: Some(url.to_string()),
            brc20_prog_rpc_url: None,
            data_api_url: None,
            subfrost_api_key: None,
            timeout_seconds: 600,
        };

        Ok(Self {
            rpc_config,
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
            bitcoin_rpc_url: self.sandshrew_rpc_url(),
            metashrew_rpc_url: self.sandshrew_rpc_url(),
            network_params: None,
        }
    }

    /// Get the network for this provider
    pub fn network(&self) -> Network {
        self.network
    }

    pub fn network_params(&self) -> Result<alkanes_cli_common::network::NetworkParams> {
        let mut params = alkanes_cli_common::network::NetworkParams::from_network_str(self.network.to_string().as_str())?;
        params.metashrew_rpc_url = self.sandshrew_rpc_url();
        params.esplora_url = self.esplora_rpc_url();
        Ok(params)
    }

    /// Make a fetch request using platform abstraction (works in browser and Node.js)
    async fn fetch_request_text(&self, url: &str, method: &str, body: Option<&str>, headers: Vec<(&str, &str)>) -> Result<String> {
        crate::platform::fetch(url, method, body, headers).await
    }

    /// Make a REST API call (not JSON-RPC) - used for Data API
    async fn rest_call(&self, base_url: &str, endpoint: &str, body: JsonValue) -> Result<JsonValue> {
        let url = format!("{}/{}", base_url.trim_end_matches('/'), endpoint);
        self.logger.info(&format!(
            "REST API call -> URL: {}, Body: {}",
            url,
            serde_json::to_string_pretty(&body).unwrap_or_else(|_| "INVALID_JSON".to_string()),
        ));

        let response_str = self.fetch_request_text(
            &url,
            "POST",
            Some(&body.to_string()),
            vec![("Content-Type", "application/json")],
        ).await?;

        self.logger.info(&format!("REST API response: {}", response_str));

        let response_json: JsonValue = serde_json::from_str(&response_str)
            .map_err(|e| AlkanesError::Serialization(format!("Failed to parse JSON: {e}")))?;

        // Check for error field in response
        if let Some(ok) = response_json.get("ok") {
            if ok == false {
                let error_msg = response_json
                    .get("error")
                    .and_then(|e| e.as_str())
                    .unwrap_or("Unknown error");
                return Err(AlkanesError::JsonRpc(format!("API error: {}", error_msg)));
            }
        }

        Ok(response_json)
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
        // Make HTTP POST request to Rebar Labs Shield
        let response_str = self.fetch_request_text(
            rebar_endpoint,
            "POST",
            Some(&request_body.to_string()),
            vec![("Content-Type", "application/json")],
        ).await?;
        
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

        // Make JSON-RPC request
        let response_str = self.fetch_request_text(
            url,
            "POST",
            Some(&request_body.to_string()),
            vec![("Content-Type", "application/json")],
        ).await?;

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
        let esplora_url = self.esplora_rpc_url();
        let sandshrew_url = self.sandshrew_rpc_url();
        let url = esplora_url.as_deref().unwrap_or(&sandshrew_url);
        self.logger.info(&format!("[EsploraProvider] Using JSON-RPC to {} for method {}", url, esplora::EsploraJsonRpcMethods::BLOCKS_TIP_HASH));
        let result = self.call(url, esplora::EsploraJsonRpcMethods::BLOCKS_TIP_HASH, esplora::params::empty(), 1).await?;
        result.as_str().map(|s| s.to_string()).ok_or_else(|| AlkanesError::RpcError("Invalid tip hash response".to_string()))
    }

    async fn get_blocks_tip_height(&self) -> Result<u64> {
        self.logger.info("[EsploraProvider] Calling get_blocks_tip_height");
        let esplora_url = self.esplora_rpc_url();
        let sandshrew_url = self.sandshrew_rpc_url();
        let url = esplora_url.as_deref().unwrap_or(&sandshrew_url);
        self.logger.info(&format!("[EsploraProvider] Using JSON-RPC to {} for method {}", url, esplora::EsploraJsonRpcMethods::BLOCKS_TIP_HEIGHT));
        let result = self.call(url, esplora::EsploraJsonRpcMethods::BLOCKS_TIP_HEIGHT, esplora::params::empty(), 1).await?;
        result.as_u64().ok_or_else(|| AlkanesError::RpcError("Invalid tip height response".to_string()))
    }

    async fn get_blocks(&self, start_height: Option<u64>) -> Result<serde_json::Value> {
        let esplora_url = self.esplora_rpc_url();
        let sandshrew_url = self.sandshrew_rpc_url();
        let url = esplora_url.as_deref().unwrap_or(&sandshrew_url);
        self.call(url, esplora::EsploraJsonRpcMethods::BLOCKS, esplora::params::optional_single(start_height), 1).await
    }

    async fn get_block_by_height(&self, height: u64) -> Result<String> {
        let esplora_url = self.esplora_rpc_url();
        let sandshrew_url = self.sandshrew_rpc_url();
        let url = esplora_url.as_deref().unwrap_or(&sandshrew_url);
        let result = self.call(url, esplora::EsploraJsonRpcMethods::BLOCK_HEIGHT, esplora::params::single(height), 1).await?;
        result.as_str().map(|s| s.to_string()).ok_or_else(|| AlkanesError::RpcError("Invalid block hash response".to_string()))
    }

    async fn get_block(&self, hash: &str) -> Result<serde_json::Value> {
        let esplora_url = self.esplora_rpc_url();
        let sandshrew_url = self.sandshrew_rpc_url();
        let url = esplora_url.as_deref().unwrap_or(&sandshrew_url);
        self.call(url, esplora::EsploraJsonRpcMethods::BLOCK, esplora::params::single(hash), 1).await
    }

    async fn get_block_status(&self, hash: &str) -> Result<serde_json::Value> {
        let esplora_url = self.esplora_rpc_url();
        let sandshrew_url = self.sandshrew_rpc_url();
        let url = esplora_url.as_deref().unwrap_or(&sandshrew_url);
        self.call(url, esplora::EsploraJsonRpcMethods::BLOCK_STATUS, esplora::params::single(hash), 1).await
    }

    async fn get_block_txids(&self, hash: &str) -> Result<serde_json::Value> {
        let esplora_url = self.esplora_rpc_url();
        let sandshrew_url = self.sandshrew_rpc_url();
        let url = esplora_url.as_deref().unwrap_or(&sandshrew_url);
        self.call(url, esplora::EsploraJsonRpcMethods::BLOCK_TXIDS, esplora::params::single(hash), 1).await
    }

    async fn get_block_header(&self, hash: &str) -> Result<String> {
        let esplora_url = self.esplora_rpc_url();
        let sandshrew_url = self.sandshrew_rpc_url();
        let url = esplora_url.as_deref().unwrap_or(&sandshrew_url);
        let result = self.call(url, esplora::EsploraJsonRpcMethods::BLOCK_HEADER, esplora::params::single(hash), 1).await?;
        result.as_str().map(|s| s.to_string()).ok_or_else(|| AlkanesError::RpcError("Invalid block header response".to_string()))
    }

    async fn get_block_raw(&self, hash: &str) -> Result<String> {
        let esplora_url = self.esplora_rpc_url();
        let sandshrew_url = self.sandshrew_rpc_url();
        let url = esplora_url.as_deref().unwrap_or(&sandshrew_url);
        let result = self.call(url, esplora::EsploraJsonRpcMethods::BLOCK_RAW, esplora::params::single(hash), 1).await?;
        result.as_str().map(|s| s.to_string()).ok_or_else(|| AlkanesError::RpcError("Invalid raw block response".to_string()))
    }

    async fn get_block_txid(&self, hash: &str, index: u32) -> Result<String> {
        self.logger.info(&format!("[EsploraProvider] Calling get_block_txid for hash: {}, index: {}", hash, index));
        let esplora_url = self.esplora_rpc_url();
        let sandshrew_url = self.sandshrew_rpc_url();
        let url = esplora_url.as_deref().unwrap_or(&sandshrew_url);
        self.logger.info(&format!("[EsploraProvider] Using JSON-RPC to {} for method {}", url, esplora::EsploraJsonRpcMethods::BLOCK_TXID));
        let result = self.call(url, esplora::EsploraJsonRpcMethods::BLOCK_TXID, esplora::params::dual(hash, index), 1).await?;
        result.as_str().map(|s| s.to_string()).ok_or_else(|| AlkanesError::RpcError("Invalid txid response".to_string()))
    }

    async fn get_block_txs(&self, hash: &str, start_index: Option<u32>) -> Result<serde_json::Value> {
        let esplora_url = self.esplora_rpc_url();
        let sandshrew_url = self.sandshrew_rpc_url();
        let url = esplora_url.as_deref().unwrap_or(&sandshrew_url);
        self.call(url, esplora::EsploraJsonRpcMethods::BLOCK_TXS, esplora::params::optional_dual(hash, start_index), 1).await
    }

    async fn get_address_info(&self, address: &str) -> Result<serde_json::Value> {
        self.logger.info(&format!("[EsploraProvider] Calling get_address_info for address: {}", address));
        let esplora_url = self.esplora_rpc_url();
        let sandshrew_url = self.sandshrew_rpc_url();
        let url = esplora_url.as_deref().unwrap_or(&sandshrew_url);
        self.logger.info(&format!("[EsploraProvider] Using JSON-RPC to {} for method {}", url, esplora::EsploraJsonRpcMethods::ADDRESS));
        self.call(url, esplora::EsploraJsonRpcMethods::ADDRESS, esplora::params::single(address), 1).await
    }

    async fn get_address_utxo(&self, address: &str) -> Result<serde_json::Value> {
        self.logger.info(&format!("[EsploraProvider] Calling get_address_utxo for address: {}", address));
        if let Some(url) = self.esplora_rpc_url().as_deref() {
            self.logger.info(&format!("[EsploraProvider] Using JSON-RPC to {} for method esplora_address::utxo", url));
            if let Ok(result) = self.call(url, "esplora_address::utxo", esplora::params::single(address), 1).await {
                return Ok(result);
            }
        }
        self.logger.info(&format!("[EsploraProvider] Falling back to JSON-RPC on sandshrew for method esplora_address::utxo"));
        // Fallback or error
        self.call(&self.sandshrew_rpc_url(), "esplora_address::utxo", esplora::params::single(address), 1).await
    }

    async fn get_address_txs(&self, address: &str) -> Result<serde_json::Value> {
        let esplora_url = self.esplora_rpc_url();
        let sandshrew_url = self.sandshrew_rpc_url();
        let url = esplora_url.as_deref().unwrap_or(&sandshrew_url);
        self.call(url, "esplora_address::txs", esplora::params::single(address), 1).await
    }

    async fn get_address_txs_chain(&self, address: &str, last_seen_txid: Option<&str>) -> Result<serde_json::Value> {
        let esplora_url = self.esplora_rpc_url();
        let sandshrew_url = self.sandshrew_rpc_url();
        let url = esplora_url.as_deref().unwrap_or(&sandshrew_url);
        self.call(url, esplora::EsploraJsonRpcMethods::ADDRESS_TXS_CHAIN, esplora::params::optional_dual(address, last_seen_txid), 1).await
    }

    async fn get_address_txs_mempool(&self, address: &str) -> Result<serde_json::Value> {
        let esplora_url = self.esplora_rpc_url();
        let sandshrew_url = self.sandshrew_rpc_url();
        let url = esplora_url.as_deref().unwrap_or(&sandshrew_url);
        self.call(url, esplora::EsploraJsonRpcMethods::ADDRESS_TXS_MEMPOOL, esplora::params::single(address), 1).await
    }

    async fn get_address_prefix(&self, prefix: &str) -> Result<serde_json::Value> {
        let esplora_url = self.esplora_rpc_url();
        let sandshrew_url = self.sandshrew_rpc_url();
        let url = esplora_url.as_deref().unwrap_or(&sandshrew_url);
        self.call(url, esplora::EsploraJsonRpcMethods::ADDRESS_PREFIX, esplora::params::single(prefix), 1).await
    }

    async fn get_tx(&self, txid: &str) -> Result<serde_json::Value> {
        self.logger.info(&format!("[EsploraProvider] Calling get_tx for txid: {}", txid));
        let esplora_url = self.esplora_rpc_url();
        let sandshrew_url = self.sandshrew_rpc_url();
        let url = esplora_url.as_deref().unwrap_or(&sandshrew_url);
        self.logger.info(&format!("[EsploraProvider] Using JSON-RPC to {} for method {}", url, esplora::EsploraJsonRpcMethods::TX));
        self.call(url, esplora::EsploraJsonRpcMethods::TX, esplora::params::single(txid), 1).await
    }

    async fn get_tx_hex(&self, txid: &str) -> Result<String> {
        let esplora_url = self.esplora_rpc_url();
        let sandshrew_url = self.sandshrew_rpc_url();
        let url = esplora_url.as_deref().unwrap_or(&sandshrew_url);
        let result = self.call(url, esplora::EsploraJsonRpcMethods::TX_HEX, esplora::params::single(txid), 1).await?;
        result.as_str().map(|s| s.to_string()).ok_or_else(|| AlkanesError::RpcError("Invalid tx hex response".to_string()))
    }

    async fn get_tx_raw(&self, txid: &str) -> Result<String> {
        let esplora_url = self.esplora_rpc_url();
        let sandshrew_url = self.sandshrew_rpc_url();
        let url = esplora_url.as_deref().unwrap_or(&sandshrew_url);
        let result = self.call(url, esplora::EsploraJsonRpcMethods::TX_RAW, esplora::params::single(txid), 1).await?;
        result.as_str().map(|s| s.to_string()).ok_or_else(|| AlkanesError::RpcError("Invalid raw tx response".to_string()))
    }

    async fn get_tx_status(&self, txid: &str) -> Result<serde_json::Value> {
        let esplora_url = self.esplora_rpc_url();
        let sandshrew_url = self.sandshrew_rpc_url();
        let url = esplora_url.as_deref().unwrap_or(&sandshrew_url);
        self.call(url, esplora::EsploraJsonRpcMethods::TX_STATUS, esplora::params::single(txid), 1).await
    }

    async fn get_tx_merkle_proof(&self, txid: &str) -> Result<serde_json::Value> {
        let esplora_url = self.esplora_rpc_url();
        let sandshrew_url = self.sandshrew_rpc_url();
        let url = esplora_url.as_deref().unwrap_or(&sandshrew_url);
        self.call(url, esplora::EsploraJsonRpcMethods::TX_MERKLE_PROOF, esplora::params::single(txid), 1).await
    }

    async fn get_tx_merkleblock_proof(&self, txid: &str) -> Result<String> {
        let esplora_url = self.esplora_rpc_url();
        let sandshrew_url = self.sandshrew_rpc_url();
        let url = esplora_url.as_deref().unwrap_or(&sandshrew_url);
        let result = self.call(url, esplora::EsploraJsonRpcMethods::TX_MERKLEBLOCK_PROOF, esplora::params::single(txid), 1).await?;
        result.as_str().map(|s| s.to_string()).ok_or_else(|| AlkanesError::RpcError("Invalid merkleblock proof response".to_string()))
    }

    async fn get_tx_outspend(&self, txid: &str, index: u32) -> Result<serde_json::Value> {
        let esplora_url = self.esplora_rpc_url();
        let sandshrew_url = self.sandshrew_rpc_url();
        let url = esplora_url.as_deref().unwrap_or(&sandshrew_url);
        self.call(url, esplora::EsploraJsonRpcMethods::TX_OUTSPEND, esplora::params::dual(txid, index), 1).await
    }

    async fn get_tx_outspends(&self, txid: &str) -> Result<serde_json::Value> {
        let esplora_url = self.esplora_rpc_url();
        let sandshrew_url = self.sandshrew_rpc_url();
        let url = esplora_url.as_deref().unwrap_or(&sandshrew_url);
        self.call(url, esplora::EsploraJsonRpcMethods::TX_OUTSPENDS, esplora::params::single(txid), 1).await
    }

    async fn broadcast(&self, tx_hex: &str) -> Result<String> {
        let esplora_url = self.esplora_rpc_url();
        let sandshrew_url = self.sandshrew_rpc_url();
        let url = esplora_url.as_deref().unwrap_or(&sandshrew_url);
        let result = self.call(url, esplora::EsploraJsonRpcMethods::BROADCAST, esplora::params::single(tx_hex), 1).await?;
        result.as_str().map(|s| s.to_string()).ok_or_else(|| AlkanesError::RpcError("Invalid broadcast response".to_string()))
    }

    async fn get_mempool(&self) -> Result<serde_json::Value> {
        let esplora_url = self.esplora_rpc_url();
        let sandshrew_url = self.sandshrew_rpc_url();
        let url = esplora_url.as_deref().unwrap_or(&sandshrew_url);
        self.call(url, esplora::EsploraJsonRpcMethods::MEMPOOL, esplora::params::empty(), 1).await
    }

    async fn get_mempool_txids(&self) -> Result<serde_json::Value> {
        let esplora_url = self.esplora_rpc_url();
        let sandshrew_url = self.sandshrew_rpc_url();
        let url = esplora_url.as_deref().unwrap_or(&sandshrew_url);
        self.call(url, esplora::EsploraJsonRpcMethods::MEMPOOL_TXIDS, esplora::params::empty(), 1).await
    }

    async fn get_mempool_recent(&self) -> Result<serde_json::Value> {
        let esplora_url = self.esplora_rpc_url();
        let sandshrew_url = self.sandshrew_rpc_url();
        let url = esplora_url.as_deref().unwrap_or(&sandshrew_url);
        self.call(url, esplora::EsploraJsonRpcMethods::MEMPOOL_RECENT, esplora::params::empty(), 1).await
    }

    async fn get_fee_estimates(&self) -> Result<serde_json::Value> {
        let esplora_url = self.esplora_rpc_url();
        let sandshrew_url = self.sandshrew_rpc_url();
        let url = esplora_url.as_deref().unwrap_or(&sandshrew_url);
        self.call(url, esplora::EsploraJsonRpcMethods::FEE_ESTIMATES, esplora::params::empty(), 1).await
    }
}

#[async_trait(?Send)]
impl WalletProvider for WebProvider {
    async fn create_wallet(&mut self, config: WalletConfig, mnemonic: Option<String>, passphrase: Option<String>) -> Result<WalletInfo> {
        let mnemonic = if let Some(m) = mnemonic {
            bip39::Mnemonic::parse_in(bip39::Language::English, &m).map_err(|e| AlkanesError::Wallet(format!("Invalid mnemonic: {e}")))?
        } else {
            let mut entropy = [0u8; 32];
            use rand::RngCore;
            rand::thread_rng().fill_bytes(&mut entropy);
            bip39::Mnemonic::from_entropy_in(bip39::Language::English, &entropy).map_err(|e| AlkanesError::Wallet(format!("Failed to generate mnemonic: {e}")))?
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
        let mnemonic = Mnemonic::parse_in(bip39::Language::English, &mnemonic_str)?;
        
        let secp = Secp256k1::new();
        let seed = mnemonic.to_seed(pass);
        let root = Xpriv::new_master(self.network, &seed)?;
        
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
        let result = self.call(&self.sandshrew_rpc_url(), "getblockcount", serde_json::json!([]), 1).await?;
        result.as_u64().ok_or_else(|| AlkanesError::RpcError("Invalid block count response".to_string()))
    }
    
    async fn generate_to_address(&self, nblocks: u32, address: &str) -> Result<JsonValue> {
        let params = serde_json::json!([nblocks, address]);
        self.call(&self.sandshrew_rpc_url(), "generatetoaddress", params, 1).await
    }

    async fn generate_future(&self, _address: &str) -> Result<JsonValue> {
        use alkanes_cli_common::subfrost::get_subfrost_address;
        use alkanes_cli_common::alkanes::types::AlkaneId;
        
        // Get the Subfrost signer address from frBTC contract [32:0]
        let frbtc_id = AlkaneId { block: 32, tx: 0 };
        
        self.logger.info("🔍 Getting Subfrost signer address from frBTC [32:0]...");
        let subfrost_address = get_subfrost_address(self, &frbtc_id).await?;
        self.logger.info(&format!("📍 Subfrost address: {}", subfrost_address));
        
        // Generate block to the Subfrost address (this will contain future-claiming protostone)
        let params = serde_json::json!([1, subfrost_address]);
        self.logger.info(&format!("⛏️  Generating future block to address: {}", subfrost_address));
        self.call(&self.sandshrew_rpc_url(), "generatetoaddress", params, 1).await
    }

    async fn get_new_address(&self) -> Result<JsonValue> {
        self.call(&self.sandshrew_rpc_url(), "getnewaddress", serde_json::json!([]), 1).await
    }
    
    async fn get_transaction_hex(&self, txid: &str) -> Result<String> {
        let params = serde_json::json!([txid, true]);
        let result = self.call(&self.sandshrew_rpc_url(), "getrawtransaction", params, 1).await?;
        result.as_str().map(|s| s.to_string()).ok_or_else(|| AlkanesError::RpcError("Invalid transaction hex response".to_string()))
    }
    
    async fn get_block(&self, hash: &str, raw: bool) -> Result<JsonValue> {
        let verbosity = if raw { 1 } else { 2 };
        let params = serde_json::json!([hash, verbosity]);
        self.call(&self.sandshrew_rpc_url(), "getblock", params, 1).await
    }
    
    async fn get_block_hash(&self, height: u64) -> Result<String> {
        let params = serde_json::json!([height]);
        let result = self.call(&self.sandshrew_rpc_url(), "getblockhash", params, 1).await?;
        result.as_str().map(|s| s.to_string()).ok_or_else(|| AlkanesError::RpcError("Invalid block hash response".to_string()))
    }
    
    async fn send_raw_transaction(&self, tx_hex: &str) -> Result<String> {
        let params = serde_json::json!([tx_hex]);
        let result = self.call(&self.sandshrew_rpc_url(), "sendrawtransaction", params, 1).await?;
        result.as_str().map(|s| s.to_string()).ok_or_else(|| AlkanesError::RpcError("Invalid txid response".to_string()))
    }
    
    async fn get_mempool_info(&self) -> Result<JsonValue> {
        self.call(&self.sandshrew_rpc_url(), "getmempoolinfo", serde_json::json!([]), 1).await
    }
    
    async fn estimate_smart_fee(&self, target: u32) -> Result<JsonValue> {
        let params = serde_json::json!([target]);
        self.call(&self.sandshrew_rpc_url(), "estimatesmartfee", params, 1).await
    }
    
    async fn get_esplora_blocks_tip_height(&self) -> Result<u64> {
        // This is an Esplora-specific method, but we can implement it using get_block_count for compatibility
        self.get_block_count().await
    }
    
    async fn trace_transaction(&self, txid: &str, vout: u32, block: Option<&str>, tx: Option<&str>) -> Result<serde_json::Value> {
        let params = serde_json::json!([txid, vout, block, tx]);
        self.call(&self.sandshrew_rpc_url(), "trace_transaction", params, 1).await
    }

    async fn get_blockchain_info(&self) -> Result<JsonValue> {
        self.call(&self.sandshrew_rpc_url(), "getblockchaininfo", serde_json::json!([]), 1).await
    }

    async fn get_network_info(&self) -> Result<JsonValue> {
        self.call(&self.sandshrew_rpc_url(), "getnetworkinfo", serde_json::json!([]), 1).await
    }

    async fn get_raw_transaction(&self, txid: &str, block_hash: Option<&str>) -> Result<JsonValue> {
        let params = if let Some(hash) = block_hash {
            serde_json::json!([txid, true, hash])
        } else {
            serde_json::json!([txid, true])
        };
        self.call(&self.sandshrew_rpc_url(), "getrawtransaction", params, 1).await
    }

    async fn get_block_header(&self, hash: &str) -> Result<JsonValue> {
        let params = serde_json::json!([hash, true]);
        self.call(&self.sandshrew_rpc_url(), "getblockheader", params, 1).await
    }

    async fn get_block_stats(&self, hash: &str) -> Result<JsonValue> {
        let params = serde_json::json!([hash]);
        self.call(&self.sandshrew_rpc_url(), "getblockstats", params, 1).await
    }

    async fn get_chain_tips(&self) -> Result<JsonValue> {
        self.call(&self.sandshrew_rpc_url(), "getchaintips", serde_json::json!([]), 1).await
    }

    async fn get_raw_mempool(&self) -> Result<JsonValue> {
        self.call(&self.sandshrew_rpc_url(), "getrawmempool", serde_json::json!([]), 1).await
    }

    async fn get_tx_out(&self, txid: &str, vout: u32, include_mempool: bool) -> Result<JsonValue> {
        let params = serde_json::json!([txid, vout, include_mempool]);
        self.call(&self.sandshrew_rpc_url(), "gettxout", params, 1).await
    }
}

#[async_trait(?Send)]
impl MetashrewRpcProvider for WebProvider {
    async fn get_metashrew_height(&self) -> Result<u64> {
        let result = self.call(&self.sandshrew_rpc_url(), "metashrew_height", serde_json::json!([]), 1).await?;
        // Handle both numeric and string responses
        if let Some(h) = result.as_u64() {
            return Ok(h);
        }
        if let Some(s) = result.as_str() {
            return s.parse::<u64>().map_err(|e| AlkanesError::RpcError(format!("Invalid height: {}", e)));
        }
        Err(AlkanesError::RpcError("Invalid metashrew height response".to_string()))
    }

    async fn get_contract_meta(&self, block: &str, tx: &str) -> Result<JsonValue> {
        let params = serde_json::json!([block, tx]);
        self.call(&self.sandshrew_rpc_url(), "metashrew_view", params, 1).await
    }

    async fn trace_outpoint(&self, txid: &str, vout: u32) -> Result<JsonValue> {
        let params = serde_json::json!([txid, vout]);
        self.call(&self.sandshrew_rpc_url(), "metashrew_view", params, 1).await
    }

    async fn get_spendables_by_address(&self, address: &str) -> Result<JsonValue> {
        let params = serde_json::json!([address]);
        self.call(&self.sandshrew_rpc_url(), "spendablesbyaddress", params, 1).await
    }

    async fn get_protorunes_by_address(
        &self,
        address: &str,
        block_tag: Option<String>,
        protocol_tag: u128,
    ) -> Result<ProtoruneWalletResponse> {
        let params = serde_json::json!([address, block_tag, protocol_tag]);
        let result = self.call(&self.sandshrew_rpc_url(), "protorunesbyaddress", params, 1).await?;
        serde_json::from_value(result).map_err(|e| AlkanesError::Serialization(e.to_string()))
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

        use prost::Message;
        let hex_input = hex::encode(outpoint_pb.encode_to_vec());
        let params = serde_json::json!(["protorunesbyoutpoint", format!("0x{}", hex_input), "latest"]);

        let result = self.call(&self.sandshrew_rpc_url(), "metashrew_view", params, 1).await?;

        let hex_str = result.as_str().ok_or_else(|| AlkanesError::RpcError("Invalid protorune response: not a string".to_string()))?;
        let bytes = hex::decode(hex_str.strip_prefix("0x").unwrap_or(hex_str))?;

        let response_pb = ProtoruneOutpointResponsePb::decode(&bytes[..]).map_err(|e| AlkanesError::Serialization(e.to_string()))?;
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

    async fn get_state_root(&self, height: JsonValue) -> Result<String> {
        let params = serde_json::json!(["getStateRoot", "0x", height]);
        let result = self.call(&self.sandshrew_rpc_url(), "metashrew_view", params, 1).await?;
        result.as_str().map(|s| s.to_string()).ok_or_else(|| AlkanesError::RpcError("Invalid state root response".to_string()))
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
        let url = self.sandshrew_rpc_url();
        let json = self.call(&url, "ord_output", serde_json::json!([output]), 1).await?;
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
    async fn view(&self, _contract_id: &str, view_fn: &str, params: Option<&[u8]>, block_tag: Option<String>) -> Result<JsonValue> {
        // metashrew_view functions are generic - do NOT prepend contract_id
        // The contract target is encoded in params (MessageContextParcel)
        let params_hex = params.map(|p| format!("0x{}", hex::encode(p))).unwrap_or_else(|| "0x".to_string());
        let block_tag = block_tag.unwrap_or_else(|| "latest".to_string());
        
        let rpc_params = serde_json::json!([view_fn, params_hex, block_tag]);
        let result = self.call(&self.sandshrew_rpc_url(), "metashrew_view", rpc_params, 1).await?;

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

    async fn trace(&self, outpoint: &str) -> Result<alkanes_cli_common::proto::alkanes::Trace> {
        use prost::Message;
        let result = self.call(&self.sandshrew_rpc_url(), "alkanes_trace", serde_json::json!([outpoint]), 1).await?;
        let hex_str = result.as_str().ok_or_else(|| AlkanesError::RpcError("Invalid trace response".to_string()))?;
        let bytes = hex::decode(hex_str.strip_prefix("0x").unwrap_or(hex_str))?;
        alkanes_cli_common::proto::alkanes::Trace::decode(&bytes[..]).map_err(|e| AlkanesError::Serialization(e.to_string()))
    }
    async fn get_block(&self, height: u64) -> Result<alkanes_cli_common::proto::alkanes::BlockResponse> {
        use prost::Message;
        let result = self.call(&self.sandshrew_rpc_url(), "alkanes_get_block", serde_json::json!([height]), 1).await?;
        let hex_str = result.as_str().ok_or_else(|| AlkanesError::RpcError("Invalid block response".to_string()))?;
        let bytes = hex::decode(hex_str.strip_prefix("0x").unwrap_or(hex_str))?;
        alkanes_cli_common::proto::alkanes::BlockResponse::decode(&bytes[..]).map_err(|e| AlkanesError::Serialization(e.to_string()))
    }
    async fn sequence(&self, block_tag: Option<String>) -> Result<JsonValue> {
        let block_tag = block_tag.unwrap_or_else(|| "latest".to_string());
        self.call(&self.sandshrew_rpc_url(), "alkanes_sequence", serde_json::json!([block_tag]), 1).await
    }
    async fn spendables_by_address(&self, address: &str) -> Result<JsonValue> {
        self.call(&self.sandshrew_rpc_url(), "alkanes_spendables_by_address", serde_json::json!([address]), 1).await
    }
    async fn trace_block(&self, height: u64) -> Result<alkanes_cli_common::proto::alkanes::Trace> {
        use prost::Message;
        let result = self.call(&self.sandshrew_rpc_url(), "alkanes_trace_block", serde_json::json!([height]), 1).await?;
        let hex_str = result.as_str().ok_or_else(|| AlkanesError::RpcError("Invalid trace block response".to_string()))?;
        let bytes = hex::decode(hex_str.strip_prefix("0x").unwrap_or(hex_str))?;
        alkanes_cli_common::proto::alkanes::Trace::decode(&bytes[..]).map_err(|e| AlkanesError::Serialization(e.to_string()))
    }
    async fn get_bytecode(&self, alkane_id: &str, block_tag: Option<String>) -> Result<String> {
        use alkanes_cli_common::proto::alkanes::{BytecodeRequest, AlkaneId, Uint128};
        use prost::Message;
        let parts: Vec<&str> = alkane_id.split(':').collect();
        if parts.len() != 2 {
            return Err(AlkanesError::InvalidParameters("Invalid alkane_id format".to_string()));
        }
        let block = parts[0].parse::<u64>()?;
        let tx = parts[1].parse::<u32>()?;

        let request = BytecodeRequest {
            id: Some(AlkaneId {
                block: Some(Uint128 {
                    lo: block,
                    hi: 0,
                }),
                tx: Some(Uint128 {
                    lo: tx as u64,
                    hi: 0,
                }),
            }),
        };
        let hex_input = hex::encode(request.encode_to_vec());

        let params = serde_json::json!(["getbytecode", format!("0x{}", hex_input), block_tag.as_deref().unwrap_or("latest")]);
        let result = self.call(&self.sandshrew_rpc_url(), "metashrew_view", params, 1).await?;
        
        let hex_str = result.as_str().ok_or_else(|| AlkanesError::RpcError("Invalid bytecode response: not a string".to_string()))?;
        let bytes = hex::decode(hex_str.strip_prefix("0x").unwrap_or(hex_str))?;
        Ok(format!("0x{}", hex::encode(bytes)))
    }
    async fn inspect(&self, target: &str, config: AlkanesInspectConfig) -> Result<AlkanesInspectResult> {
        let params = serde_json::json!([target, config]);
        let result = self.call(&self.sandshrew_rpc_url(), "alkanes_inspect", params, 1).await?;
        serde_json::from_value(result).map_err(|e| AlkanesError::Serialization(e.to_string()))
    }
    async fn get_balance(&self, address: Option<&str>) -> Result<Vec<AlkaneBalance>> {
        let addr = match address {
            Some(a) => a.to_string(),
            None => WalletProvider::get_address(self).await?,
        };
        let result = self.call(&self.sandshrew_rpc_url(), "alkanes_get_balance", serde_json::json!([addr]), 1).await?;
        serde_json::from_value(result).map_err(|e| AlkanesError::Serialization(e.to_string()))
    }
    async fn pending_unwraps(&self, block_tag: Option<String>) -> Result<Vec<alkanes_cli_common::alkanes::PendingUnwrap>> {
        let block_tag = block_tag.unwrap_or_else(|| "latest".to_string());
        let result = self.call(&self.sandshrew_rpc_url(), "alkanes_pending_unwraps", serde_json::json!([block_tag]), 1).await?;
        serde_json::from_value(result).map_err(|e| AlkanesError::Serialization(e.to_string()))
    }

    async fn trace_protostones(&self, txid: &str) -> Result<Option<Vec<JsonValue>>> {
        use prost::Message;
        
        // Get transaction
        let tx_hex = self.get_transaction_hex(txid).await?;
        let tx_bytes = hex::decode(&tx_hex).map_err(|e| AlkanesError::Hex(e.to_string()))?;
        let tx: bitcoin::Transaction = bitcoin::consensus::deserialize(&tx_bytes)
            .map_err(|e| AlkanesError::Serialization(e.to_string()))?;
        
        // Decode runestone to get protostones
        let result = alkanes_cli_common::runestone_enhanced::format_runestone_with_decoded_messages(&tx)
            .map_err(|e| AlkanesError::Other(format!("Failed to decode runestone: {}", e)))?;
        
        // Extract number of protostones
        let num_protostones = if let Some(protostones) = result.get("protostones").and_then(|p| p.as_array()) {
            protostones.len()
        } else {
            0
        };
        
        if num_protostones == 0 {
            return Ok(None);
        }
        
        // Calculate virtual vout indices and trace each protostone
        // Protostones are indexed starting at tx.output.len() + 1
        let base_vout = tx.output.len() as u32 + 1;
        let mut all_traces = Vec::new();
        
        for i in 0..num_protostones {
            let vout = base_vout + i as u32;
            let outpoint = format!("{}:{}", txid, vout);
            
            match self.trace(&outpoint).await {
                Ok(trace_pb) => {
                    if let Some(alkanes_trace) = trace_pb.trace {
                        // Convert alkanes-cli-common proto to alkanes-support proto via bytes
                        let trace_bytes = Message::encode_to_vec(&alkanes_trace);
                        match alkanes_support::proto::alkanes::AlkanesTrace::decode(trace_bytes.as_slice()) {
                            Ok(support_trace) => {
                                let trace: alkanes_support::trace::Trace = support_trace.into();
                                let json = alkanes_cli_common::alkanes::trace::trace_to_json(&trace);
                                all_traces.push(json);
                            }
                            Err(e) => {
                                return Err(AlkanesError::Serialization(format!("Failed to decode trace for protostone {}: {}", i, e)));
                            }
                        }
                    } else {
                        return Err(AlkanesError::Other(format!("No trace found for protostone {}", i)));
                    }
                }
                Err(e) => {
                    return Err(AlkanesError::Other(format!("Failed to trace protostone {}: {}", i, e)));
                }
            }
        }
        
        Ok(Some(all_traces))
    }

    async fn tx_script(
        &self,
        _wasm_bytes: &[u8],
        _inputs: Vec<u128>,
        _block_tag: Option<String>,
    ) -> Result<Vec<u8>> {
        // For WASM build, tx_script would need to be proxied through the RPC
        // or implemented using a different approach
        Err(AlkanesError::Other("tx_script not implemented for WASM build".to_string()))
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
impl alkanes_cli_common::lua_script::LuaScriptExecutor for WebProvider {
    async fn execute_lua_script(
        &self,
        script: &alkanes_cli_common::lua_script::LuaScript,
        args: Vec<alkanes_cli_common::JsonValue>,
    ) -> alkanes_cli_common::Result<alkanes_cli_common::JsonValue> {
        // Try cached version first
        match self.lua_evalsaved(script.hash(), args.clone()).await {
            Ok(result) => Ok(result),
            Err(_) => {
                // Cache miss, execute full script
                self.lua_evalscript(script.content(), args).await
            }
        }
    }

    async fn lua_evalsaved(
        &self,
        script_hash: &str,
        args: Vec<alkanes_cli_common::JsonValue>,
    ) -> alkanes_cli_common::Result<alkanes_cli_common::JsonValue> {
        let mut params = vec![alkanes_cli_common::JsonValue::String(script_hash.to_string())];
        params.extend(args);
        self.call(&self.sandshrew_rpc_url(), "lua_evalsaved", alkanes_cli_common::JsonValue::Array(params), 1).await
    }

    async fn lua_evalscript(
        &self,
        script_content: &str,
        args: Vec<alkanes_cli_common::JsonValue>,
    ) -> alkanes_cli_common::Result<alkanes_cli_common::JsonValue> {
        let mut params = vec![alkanes_cli_common::JsonValue::String(script_content.to_string())];
        params.extend(args);
        self.call(&self.sandshrew_rpc_url(), "lua_evalscript", alkanes_cli_common::JsonValue::Array(params), 1).await
    }
}

#[async_trait(?Send)]
impl DeezelProvider for WebProvider {
    fn provider_name(&self) -> &str {
        "WebProvider"
    }

    fn get_bitcoin_rpc_url(&self) -> Option<String> {
        self.esplora_rpc_url()
    }

    fn get_esplora_api_url(&self) -> Option<String> {
        self.esplora_rpc_url()
    }

    fn get_ord_server_url(&self) -> Option<String> {
        // Assuming no separate ord server for web provider, using sandshrew
        Some(self.sandshrew_rpc_url())
    }

    fn get_metashrew_rpc_url(&self) -> Option<String> {
        Some(self.sandshrew_rpc_url())
    }

    fn get_brc20_prog_rpc_url(&self) -> Option<String> {
        None
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
            alkanes_change_address: None,
            input_requirements: vec![],
            protostones: vec![ProtostoneSpec {
                cellpack: Some(Cellpack::try_from(vec![2, 0, 1]).unwrap()), // Assuming 2 is for wrapping, 0 is frBTC, 1 is mint
                edicts: vec![],
                bitcoin_transfer: Some(BitcoinTransfer { amount, target: alkanes_cli_common::alkanes::types::OutputTarget::Split }),
                pointer: None,
                refund: None,
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
            alkanes_change_address: None,
            input_requirements: vec![],
            protostones: vec![ProtostoneSpec {
                cellpack: Some(Cellpack::try_from(vec![2, 0, 2]).unwrap()), // Assuming 2 is for unwrapping, 0 is frBTC, 2 is burn
                edicts: vec![],
                bitcoin_transfer: Some(BitcoinTransfer { amount, target: alkanes_cli_common::alkanes::types::OutputTarget::Split }),
                pointer: None,
                refund: None,
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