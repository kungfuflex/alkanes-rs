//! The ConcreteProvider implementation for deezel.
//!
//! This module provides a concrete implementation of all provider traits
//! using deezel-rpgp for PGP operations and other concrete implementations.

use crate::traits::*;
use crate::{
    alkanes::types::{ExecutionState, ReadyToSignCommitTx, ReadyToSignRevealTx, ReadyToSignTx},
    DeezelError, JsonValue, Result,
};
use serde_json::json;
use crate::ord;
use crate::alkanes::execute::EnhancedAlkanesExecutor;
#[cfg(feature = "wasm-inspection")]
use crate::alkanes::inspector::{AlkaneInspector, InspectionConfig};
use crate::alkanes::types::{
	EnhancedExecuteParams, EnhancedExecuteResult, AlkanesInspectConfig, AlkanesInspectResult,
	AlkaneBalance, AlkaneId,
};
use crate::proto::alkanes as alkanes_pb;
use crate::proto::protorune as protorune_pb;
use std::collections::BTreeMap;
use protobuf::Message;
use log;
use async_trait::async_trait;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use alloc::boxed::Box;
#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;
use core::str::FromStr;
use crate::keystore::Keystore;
use crate::network::{NetworkParams, RpcConfig};
use crate::rpc::get_rpc_url;
use crate::rpc::{determine_rpc_call_type, RpcCallType};

// Import deezel-rpgp types for PGP functionality

#[cfg(not(target_arch = "wasm32"))]
use rand::thread_rng;
#[cfg(target_arch = "wasm32")]
use rand::rngs::OsRng;

// Import Bitcoin and BIP39 for wallet functionality
use bitcoin::Network;
use bip39::{Mnemonic, MnemonicType, Seed};

// Additional imports for wallet functionality
use hex::{self, FromHex};
use bitcoin::{
    Address, Amount, OutPoint, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Witness,
    bip32::{DerivationPath, Fingerprint, Xpriv},
    key::{TapTweak, UntweakedKeypair},
    secp256k1::{All, Secp256k1},
    sighash::{Prevouts, SighashCache, TapSighashType},
    taproot,
};
use bitcoin_hashes::Hash;
use ordinals::{Runestone, Artifact};
use serde::{Deserialize, Serialize};


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetBalance {
    pub name: String,
    pub symbol: String,
    pub balance: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichedUtxo {
    pub utxo_info: UtxoInfo,
    pub assets: Vec<AssetBalance>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllBalances {
    pub btc: WalletBalance,
    pub other: Vec<AssetBalance>,
}


/// Represents the state of the wallet within the provider
#[derive(Clone)]
pub enum WalletState {
    /// No wallet is loaded
    None,
    /// Keystore is loaded but locked (only public information is available)
    Locked(Keystore),
    /// Wallet is unlocked, with access to the decrypted mnemonic
    Unlocked {
        keystore: Keystore,
        mnemonic: String,
    },
}




use crate::commands::Commands;

#[derive(Clone)]
pub struct ConcreteProvider {
    pub rpc_config: RpcConfig,
    pub command: Commands,
    #[cfg(not(target_arch = "wasm32"))]
    pub _wallet_path: Option<PathBuf>,
    #[cfg(target_arch = "wasm32")]
    pub _wallet_path: Option<String>,
    pub passphrase: Option<String>,
    pub wallet_state: WalletState,
    #[cfg(feature = "native-deps")]
    pub http_client: reqwest::Client,
    pub secp: Secp256k1<All>,
}


impl ConcreteProvider {
    #[cfg(test)]
    pub fn new_for_test(rpc_config: RpcConfig, command: Commands) -> Self {
        Self {
            rpc_config,
            command,
            wallet_path: None,
            passphrase: None,
            wallet_state: WalletState::None,
            #[cfg(feature = "native-deps")]
            http_client: reqwest::Client::new(),
            secp: Secp256k1::new(),
        }
    }

    pub fn get_network(&self) -> bitcoin::Network {
        self.rpc_config.network.0
    }

    async fn metashrew_view_call(&self, _method: &str, _params: &str, _height: &str) -> Result<Vec<u8>> {
        unimplemented!()
    }

    pub fn get_keystore(&self) -> Result<&Keystore> {
        match &self.wallet_state {
            WalletState::Unlocked { keystore, .. } => Ok(keystore),
            _ => Err(DeezelError::Wallet("Wallet is not unlocked".to_string())),
        }
    }

    /// A helper function to select coins for a transaction.
    fn select_coins(&self, utxos: Vec<UtxoInfo>, target_amount: Amount) -> Result<(Vec<UtxoInfo>, Amount)> {
        let mut selected_utxos = Vec::new();
        let mut total_input_amount = Amount::ZERO;

        for utxo in utxos {
            if total_input_amount >= target_amount {
                break;
            }
            if !utxo.frozen {
                total_input_amount += Amount::from_sat(utxo.amount);
                selected_utxos.push(utxo);
            }
        }

        if total_input_amount < target_amount {
            return Err(DeezelError::Wallet("Insufficient funds".to_string()));
        }

        Ok((selected_utxos, total_input_amount))
    }

    /// A helper function to estimate the virtual size of a transaction.
    fn estimate_tx_vsize(&self, tx: &Transaction, num_inputs: usize) -> u64 {
        // A simple estimation logic. This should be improved for accuracy.
        // Base size + size per input + size per output
        let base_vsize = 10;
        let input_vsize = 68; // P2TR input vsize
        let output_vsize = 43; // P2TR output vsize
        base_vsize + (num_inputs as u64 * input_vsize) + (tx.output.len() as u64 * output_vsize)
    }

    pub fn get_wallet_state(&self) -> &WalletState {
        &self.wallet_state
    }

    pub async fn unlock_wallet(&mut self, passphrase: &str) -> Result<()> {
        if let WalletState::Locked(keystore) = &self.wallet_state {
            let mnemonic = keystore.decrypt_mnemonic(passphrase)?;
            self.wallet_state = WalletState::Unlocked {
                keystore: keystore.clone(),
                mnemonic,
            };
            Ok(())
        } else {
            Err(DeezelError::Wallet("Wallet is not locked".to_string()))
        }
    }

    pub fn get_wallet_path(&self) -> Option<PathBuf> {
        self._wallet_path.clone()
    }

    /// A helper function to find address info from the keystore.
    fn find_address_info(keystore: &Keystore, address: &Address, network: Network) -> Result<AddressInfo> {
        // This is a placeholder. In a real wallet, you'd efficiently search
        // the keystore's derived addresses. 
        for i in 0..1000 { // A reasonable search limit
            for chain in 0..=1 {
                if let Ok(addrs) = keystore.get_addresses(network, "p2tr", chain, i, 1) {
                    if let Some(info) = addrs.first() {
                        if info.address == address.to_string() {
                            return Ok(info.clone());
                        }
                    }
                }
            }
        }
        Err(DeezelError::Wallet(format!("Address {} not found in keystore", address)))
    }
}




#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl JsonRpcProvider for ConcreteProvider {
    async fn call(&self, url: &str, method: &str, params: JsonValue, id: u64) -> Result<JsonValue> {
        let payload = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
            "id": id,
        });

        log::debug!("[JsonRpcProvider] Making call to {}: {}", url, payload);

        #[cfg(feature = "native-deps")]
        {
            let response = self
                .http_client
                .post(url)
                .json(&payload)
                .send()
                .await
                .map_err(|e| DeezelError::Network(e.to_string()))?;

            let response_text = response.text().await.map_err(|e| DeezelError::Network(e.to_string()))?;
            log::debug!("[JsonRpcProvider] Response from {}: {}", url, response_text);

            let json_response: JsonValue = serde_json::from_str(&response_text)
                .map_err(|e| DeezelError::RpcError(format!("Failed to parse JSON response: {}", e)))?;

            if let Some(error) = json_response.get("error") {
                if !error.is_null() {
                    return Err(DeezelError::RpcError(error.to_string()));
                }
            }

            if let Some(result) = json_response.get("result") {
                Ok(result.clone())
            } else {
                Err(DeezelError::RpcError("RPC response did not contain a 'result' field".to_string()))
            }
        }

        #[cfg(not(feature = "native-deps"))]
        {
            let _ = (url, method, params, id);
            Err(DeezelError::NotImplemented("JsonRpcProvider::call is not implemented without 'native-deps' feature".to_string()))
        }
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl StorageProvider for ConcreteProvider {
    async fn read(&self, _key: &str) -> Result<Vec<u8>> {
        unimplemented!()
    }
    
    async fn write(&self, _key: &str, _data: &[u8]) -> Result<()> {
        unimplemented!()
    }
    
    async fn exists(&self, _key: &str) -> Result<bool> {
        unimplemented!()
    }
    
    async fn delete(&self, _key: &str) -> Result<()> {
        unimplemented!()
    }
    
    async fn list_keys(&self, _prefix: &str) -> Result<Vec<String>> {
        unimplemented!()
    }
    
    fn storage_type(&self) -> &'static str {
        "placeholder"
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl NetworkProvider for ConcreteProvider {
    async fn get(&self, _url: &str) -> Result<Vec<u8>> {
        unimplemented!()
    }
    
    async fn post(&self, _url: &str, _body: &[u8], _content_type: &str) -> Result<Vec<u8>> {
        unimplemented!()
    }
    
    async fn is_reachable(&self, _url: &str) -> bool {
        unimplemented!()
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl CryptoProvider for ConcreteProvider {
    fn random_bytes(&self, _len: usize) -> Result<Vec<u8>> {
        unimplemented!()
    }
    
    fn sha256(&self, _data: &[u8]) -> Result<[u8; 32]> {
        unimplemented!()
    }
    
    fn sha3_256(&self, _data: &[u8]) -> Result<[u8; 32]> {
        unimplemented!()
    }
    
    async fn encrypt_aes_gcm(&self, _data: &[u8], _key: &[u8], _nonce: &[u8]) -> Result<Vec<u8>> {
        unimplemented!()
    }
    
    async fn decrypt_aes_gcm(&self, _data: &[u8], _key: &[u8], _nonce: &[u8]) -> Result<Vec<u8>> {
        unimplemented!()
    }
    
    async fn pbkdf2_derive(&self, _password: &[u8], _salt: &[u8], _iterations: u32, _key_len: usize) -> Result<Vec<u8>> {
        unimplemented!()
    }
}



#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl TimeProvider for ConcreteProvider {
    fn now_secs(&self) -> u64 {
        unimplemented!()
    }
    
    fn now_millis(&self) -> u64 {
        unimplemented!()
    }
    
    #[cfg(feature = "native-deps")]
    async fn sleep_ms(&self, ms: u64) {
        tokio::time::sleep(core::time::Duration::from_millis(ms)).await;
    }

    #[cfg(not(feature = "native-deps"))]
    async fn sleep_ms(&self, ms: u64) {
        #[cfg(target_arch = "wasm32")]
        {
            gloo_timers::future::sleep(core::time::Duration::from_millis(ms)).await;
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = ms;
            unimplemented!("sleep_ms is not implemented for non-wasm targets without native-deps feature")
        }
    }
}

impl LogProvider for ConcreteProvider {
    fn debug(&self, message: &str) {
        log::debug!("{}", message);
    }
    
    fn info(&self, message: &str) {
        log::info!("{}", message);
    }
    
    fn warn(&self, message: &str) {
        log::warn!("{}", message);
    }
    
    fn error(&self, message: &str) {
        log::error!("{}", message);
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl WalletProvider for ConcreteProvider {
    async fn create_wallet(&mut self, config: WalletConfig, mnemonic: Option<String>, passphrase: Option<String>) -> Result<WalletInfo> {
        let mnemonic = if let Some(m) = mnemonic {
            Mnemonic::from_phrase(&m, bip39::Language::English).map_err(|e| DeezelError::Wallet(format!("Invalid mnemonic: {e}")))?
        } else {
            Mnemonic::new(MnemonicType::Words24, bip39::Language::English)
        };

        let pass = passphrase.clone().unwrap_or_default();
        let keystore = Keystore::new(&mnemonic, config.network, &pass, None)?;

        #[cfg(not(target_arch = "wasm32"))]
        if let Some(path) = &self._wallet_path {
            let timestamp = chrono::Utc::now().format("%Y%m%d%H%M%S").to_string();
            let original_filename = path.file_stem().and_then(|s| s.to_str()).unwrap_or("keystore");
            let extension = path.extension().and_then(|s| s.to_str()).unwrap_or("json");
            let new_filename = format!("{}-{}.{}", original_filename, timestamp, extension);
            let new_path = path.with_file_name(new_filename);
            keystore.save_to_file(&new_path)?;
        }

        let addresses = keystore.get_addresses(config.network, "p2tr", 0, 0, 1)?;
        let address = addresses.first().map(|a| a.address.clone()).unwrap_or_default();
        
        self.wallet_state = WalletState::Unlocked {
            keystore,
            mnemonic: mnemonic.to_string(),
        };
        self.passphrase = passphrase;

        Ok(WalletInfo {
            address,
            network: config.network,
            mnemonic: Some(mnemonic.to_string()),
        })
    }
    
    async fn load_wallet(&mut self, config: WalletConfig, passphrase: Option<String>) -> Result<WalletInfo> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let path = PathBuf::from(config.wallet_path);
            let keystore = Keystore::from_file(&path)?;
            let pass = passphrase.as_deref().ok_or_else(|| DeezelError::Wallet("Passphrase required to load wallet".to_string()))?;
            let mnemonic = keystore.decrypt_mnemonic(pass)?;
            let addresses = keystore.get_addresses(config.network, "p2tr", 0, 0, 1)?;
            let address = addresses.first().map(|a| a.address.clone()).unwrap_or_default();

            self.wallet_state = WalletState::Unlocked {
                keystore,
                mnemonic: mnemonic.clone(),
            };
            self.passphrase = passphrase;

            Ok(WalletInfo {
                address,
                network: config.network,
                mnemonic: Some(mnemonic),
            })
        }
        #[cfg(target_arch = "wasm32")]
        {
            let _ = (config, passphrase);
            Err(DeezelError::NotImplemented("File system not available in wasm".to_string()))
        }
    }
    
    async fn get_balance(&self, addresses: Option<Vec<String>>) -> Result<WalletBalance> {
        log::info!("[WalletProvider] Calling get_balance for addresses: {:?}", addresses);
        let addrs_to_check = if let Some(provided_addresses) = addresses {
            provided_addresses
        } else {
            // If no addresses are provided, derive the first 20 from the public key.
            let derived_infos = self.get_addresses(20).await?;
            derived_infos.into_iter().map(|info| info.address).collect()
        };

        if addrs_to_check.is_empty() {
            return Ok(WalletBalance { confirmed: 0, pending: 0 });
        }

        let _total_confirmed_balance = 0_u64;
        let _total_pending_balance = 0_i64;

        // TODO: This is a placeholder implementation after removing the direct esplora calls.
        // The correct balance calculation will happen in the frontend after fetching enriched UTXOs.
        for _address in addrs_to_check {
            // let info = self.get_address_info(&address).await?;
            //
            // // Confirmed balance
            // if let Some(chain_stats) = info.get("chain_stats") {
            //     let funded = chain_stats.get("funded_txo_sum").and_then(|v| v.as_u64()).unwrap_or(0);
            //     let spent = chain_stats.get("spent_txo_sum").and_then(|v| v.as_u64()).unwrap_or(0);
            //     total_confirmed_balance += funded.saturating_sub(spent);
            // }
            //
            // // Pending balance (can be negative)
            // if let Some(mempool_stats) = info.get("mempool_stats") {
            //     let funded = mempool_stats.get("funded_txo_sum").and_then(|v| v.as_i64()).unwrap_or(0);
            //     let spent = mempool_stats.get("spent_txo_sum").and_then(|v| v.as_i64()).unwrap_or(0);
            //     total_pending_balance += funded - spent;
            // }
        }

        Ok(WalletBalance {
            confirmed: 0,
            pending: 0,
        })
    }
    
    async fn get_address(&self) -> Result<String> {
        log::info!("[WalletProvider] Calling get_address");
        let addresses = self.get_addresses(1).await?;
        if let Some(address_info) = addresses.first() {
            Ok(address_info.address.clone())
        } else {
            Err(DeezelError::Wallet("No addresses found in wallet".to_string()))
        }
    }
    
    async fn get_addresses(&self, count: u32) -> Result<Vec<AddressInfo>> {
        log::info!("[WalletProvider] Calling get_addresses with count: {}", count);
        let keystore = self.get_keystore()?;
        let addresses = keystore.get_addresses(self.get_network(), "p2tr", 0, 0, count)?;
        Ok(addresses)
    }
    
    async fn send(&mut self, params: SendParams) -> Result<String> {
        log::info!("[WalletProvider] Calling send with params: {:?}", params);
        // 1. Create the transaction
        let tx_hex = self.create_transaction(params).await?;

        // 2. Sign the transaction
        let signed_tx_hex = self.sign_transaction(tx_hex).await?;

        // 3. Broadcast the transaction
        self.broadcast_transaction(signed_tx_hex).await
    }
    
    async fn get_utxos(&self, _include_frozen: bool, addresses: Option<Vec<String>>) -> Result<Vec<(OutPoint, UtxoInfo)>> {
        log::info!("[WalletProvider] Calling get_utxos for addresses: {:?}", addresses);
        if addresses.is_none() || addresses.as_ref().unwrap().is_empty() {
            return Ok(Vec::new());
        }

        let mut all_utxos = Vec::new();
        let current_height = self.get_block_count().await.unwrap_or(0);

        for address in addresses.unwrap_or_default() {
            log::debug!("Fetching UTXOs for address: {}", address);
            let utxos_json = self.get_address_utxo(&address).await?;
            if let Ok(esplora_utxos) = serde_json::from_value::<Vec<crate::esplora::EsploraUtxo>>(utxos_json) {
                for utxo in esplora_utxos {
                    let outpoint = OutPoint::from_str(&format!("{}:{}", utxo.txid, utxo.vout))?;
                    let confirmations = if let Some(block_height) = utxo.status.block_height {
                        if current_height > 0 {
                            current_height.saturating_sub(block_height as u64) + 1
                        } else {
                            0
                        }
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
                        has_inscriptions: false,
                        has_runes: false,
                        has_alkanes: false,
                        is_coinbase: false,
                    };
                    all_utxos.push((outpoint, utxo_info));
                }
            }
        }

        Ok(all_utxos)
    }
    
    async fn get_history(&self, count: u32, address: Option<String>) -> Result<Vec<TransactionInfo>> {
        log::info!("[WalletProvider] Calling get_history for address: {:?}, count: {}", address, count);
        let addr = address.ok_or_else(|| DeezelError::Wallet("get_history requires an address".to_string()))?;
        let txs_json = self.get_address_txs(&addr).await?;
        let mut transactions = Vec::new();

        if let Some(txs_array) = txs_json.as_array() {
            for tx in txs_array.iter().take(count as usize) {
                if let Some(txid) = tx.get("txid").and_then(|t| t.as_str()) {
                    let status = tx.get("status");
                    let confirmed = status.and_then(|s| s.get("confirmed")).and_then(|c| c.as_bool()).unwrap_or(false);
                    let block_height = status.and_then(|s| s.get("block_height")).and_then(|h| h.as_u64());
                    let block_time = status.and_then(|s| s.get("block_time")).and_then(|t| t.as_u64());
                    let fee = tx.get("fee").and_then(|f| f.as_u64());

                    transactions.push(TransactionInfo {
                        txid: txid.to_string(),
                        block_height,
                        block_time,
                        confirmed,
                        fee,
                        weight: tx.get("weight").and_then(|w| w.as_u64()),
                        inputs: vec![], // Requires parsing vin
                        outputs: vec![], // Requires parsing vout
                        is_op_return: false,
                        has_protostones: false,
                        is_rbf: false,
                    });
                }
            }
        }
        Ok(transactions)
    }
    
    async fn freeze_utxo(&self, _utxo: String, _reason: Option<String>) -> Result<()> {
        unimplemented!()
    }
    
    async fn unfreeze_utxo(&self, _utxo: String) -> Result<()> {
        unimplemented!()
    }
    
    async fn create_transaction(&self, params: SendParams) -> Result<String> {
        log::info!("[WalletProvider] Calling create_transaction with params: {:?}", params);
        // 1. Determine which addresses to use for sourcing UTXOs
        let (address_strings, all_addresses) = if let Some(from_addresses) = &params.from {
            (from_addresses.clone(), from_addresses.iter().map(|s| AddressInfo {
                address: s.clone(),
                index: 0, // Not relevant here
                derivation_path: "".to_string(), // Not relevant here
                script_type: "".to_string(), // Not relevant here
                used: false, // Not relevant here
            }).collect())
        } else {
            // Fallback to discovering addresses if --from is not provided
            let discovered_addresses = self.get_addresses(100).await?; // A reasonable number for a simple wallet
            (discovered_addresses.iter().map(|a| a.address.clone()).collect(), discovered_addresses)
        };

        // 2. Get UTXOs for the specified addresses
        let utxos = self.get_utxos(false, Some(address_strings.clone())).await?;

        // 3. Perform coin selection
        let target_amount = Amount::from_sat(params.amount);
        let fee_rate = params.fee_rate.unwrap_or(1.0); // Default to 1 sat/vbyte

        let utxo_infos: Vec<UtxoInfo> = utxos.into_iter().map(|(_, info)| info).collect();
        let (selected_utxos, total_input_amount) = self.select_coins(utxo_infos, target_amount)?;

        // 4. Build the transaction skeleton
        let mut tx = Transaction {
            version: bitcoin::transaction::Version(2),
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: Vec::new(),
            output: Vec::new(),
        };

        // Add inputs from selected UTXOs
        for utxo in &selected_utxos {
            tx.input.push(TxIn {
                previous_output: OutPoint {
                    txid: bitcoin::Txid::from_str(&utxo.txid)?,
                    vout: utxo.vout,
                },
                script_sig: ScriptBuf::new(), // Empty for SegWit
                sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
                witness: Witness::new(), // Empty for now, will be added during signing
            });
        }

        // Add the recipient's output
        let network = self.get_network();
        let recipient_address = Address::from_str(&params.address)?.require_network(network)?;
        tx.output.push(TxOut {
            value: target_amount,
            script_pubkey: recipient_address.script_pubkey(),
        });

        // 5. Calculate fee and add change output if necessary
        // Start with an initial fee estimate. We add a placeholder change output to get a more
        // accurate size, then calculate the fee, then the actual change.
        let change_address = Address::from_str(&all_addresses[0].address)?.require_network(network)?;
        let change_script = change_address.script_pubkey();
        let placeholder_change = TxOut { value: Amount::ZERO, script_pubkey: change_script.clone() };
        tx.output.push(placeholder_change);

        let estimated_vsize = self.estimate_tx_vsize(&tx, selected_utxos.len());
        let fee = Amount::from_sat((estimated_vsize as f32 * fee_rate).ceil() as u64);

        // Now that we have a good fee estimate, remove the placeholder and calculate the real change.
        tx.output.pop();
        let change_amount = total_input_amount.checked_sub(target_amount).and_then(|a| a.checked_sub(fee));

        if let Some(change) = change_amount {
            if change > bitcoin::Amount::from_sat(546) { // Dust limit
                tx.output.push(TxOut {
                    value: change,
                    script_pubkey: change_script,
                });
            }
            // If change is dust, it's not added, effectively becoming part of the fee.
        }

        // 6. Serialize the unsigned transaction to hex
        Ok(bitcoin::consensus::encode::serialize_hex(&tx))
    }

    async fn sign_transaction(&mut self, tx_hex: String) -> Result<String> {
        log::info!("[WalletProvider] Calling sign_transaction");
        // 1. Deserialize the transaction
        let hex_bytes = hex::decode(tx_hex)?;
        let mut tx: Transaction = bitcoin::consensus::deserialize(&hex_bytes)?;

        // 2. Setup for signing - gather immutable info first to avoid borrow checker issues.
        let network = self.get_network();
        let secp: Secp256k1<All> = Secp256k1::new();

        // 3. Fetch the previous transaction outputs (prevouts) for signing.
        let mut prevouts = Vec::new();
        for input in &tx.input {
            let tx_info = self.get_tx(&input.previous_output.txid.to_string()).await?;
            let vout_info = tx_info["vout"].get(input.previous_output.vout as usize)
                .ok_or_else(|| DeezelError::Wallet(format!("Vout {} not found for tx {}", input.previous_output.vout, input.previous_output.txid)))?;
            
            let amount = vout_info["value"].as_u64()
                .ok_or_else(|| DeezelError::Wallet("UTXO value not found".to_string()))?;
            let script_pubkey_hex = vout_info["scriptpubkey"].as_str()
                .ok_or_else(|| DeezelError::Wallet("UTXO script pubkey not found".to_string()))?;
            
            let script_pubkey = ScriptBuf::from(Vec::from_hex(script_pubkey_hex)?);
            prevouts.push(TxOut { value: Amount::from_sat(amount), script_pubkey });
        }

        // 4. Get mutable access to the wallet state *after* all immutable borrows are done.
        let (keystore, mnemonic) = match &mut self.wallet_state {
            WalletState::Unlocked { keystore, mnemonic } => (keystore, mnemonic),
            _ => return Err(DeezelError::Wallet("Wallet must be unlocked to sign transactions".to_string())),
        };

        // 5. Sign each input
        let mut sighash_cache = SighashCache::new(&mut tx);
        for i in 0..prevouts.len() {
            let prev_txout = &prevouts[i];
            
            // Find the address and its derivation path from our keystore
            let address = Address::from_script(&prev_txout.script_pubkey, network)
                .map_err(|e| DeezelError::Wallet(format!("Failed to parse address from script: {e}")))?;
            
            // This call now takes a mutable keystore and may cache the derived address info.
            let addr_info = Self::find_address_info(keystore, &address, network)?;
            let path = DerivationPath::from_str(&addr_info.derivation_path)?;

            // Derive the private key for this input
            let mnemonic_obj = Mnemonic::from_phrase(mnemonic, bip39::Language::English)?;
            let seed = Seed::new(&mnemonic_obj, "");
            let root_key = Xpriv::new_master(network, seed.as_bytes())?;
            let derived_xpriv = root_key.derive_priv(&secp, &path)?;
            let keypair = derived_xpriv.to_keypair(&secp);
            let untweaked_keypair = UntweakedKeypair::from(keypair);
            let tweaked_keypair = untweaked_keypair.tap_tweak(&secp, None);

            // Create the sighash
            let sighash = sighash_cache.taproot_key_spend_signature_hash(
                i,
                &Prevouts::All(&prevouts),
                TapSighashType::Default,
            )?;

            // Sign the sighash
            let msg = bitcoin::secp256k1::Message::from(sighash);
            #[cfg(not(target_arch = "wasm32"))]
            let signature = secp.sign_schnorr_with_rng(&msg, &tweaked_keypair.to_keypair(), &mut thread_rng());
            #[cfg(target_arch = "wasm32")]
            let signature = secp.sign_schnorr_with_rng(&msg, &tweaked_keypair.to_keypair(), &mut OsRng);
            
            let taproot_signature = taproot::Signature {
                signature,
                sighash_type: TapSighashType::Default,
            };

            // Add the signature to the witness
            sighash_cache.witness_mut(i).unwrap().clone_from(&Witness::p2tr_key_spend(&taproot_signature));
        }

        // 6. Serialize the signed transaction
        let signed_tx = sighash_cache.into_transaction();
        Ok(bitcoin::consensus::encode::serialize_hex(&signed_tx))
    }
    
    async fn broadcast_transaction(&self, tx_hex: String) -> Result<String> {
        log::info!("[WalletProvider] Calling broadcast_transaction");
        self.send_raw_transaction(&tx_hex).await
    }
    
    async fn estimate_fee(&self, target: u32) -> Result<FeeEstimate> {
        let fee_estimates = self.get_fee_estimates().await?;
        let fee_rate = fee_estimates
            .get(target.to_string())
            .and_then(|v| v.as_f64().map(|f| f as f32))
            .unwrap_or(1.0);

        Ok(FeeEstimate {
            fee_rate,
            target_blocks: target,
        })
    }
    
    async fn get_fee_rates(&self) -> Result<FeeRates> {
        let fee_estimates = self.get_fee_estimates().await?;
        
        let fast = fee_estimates.get("1").and_then(|v| v.as_f64()).unwrap_or(10.0) as f32;
        let medium = fee_estimates.get("6").and_then(|v| v.as_f64()).unwrap_or(5.0) as f32;
        let slow = fee_estimates.get("144").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32;

        Ok(FeeRates {
            fast,
            medium,
            slow,
        })
    }
    
    async fn sync(&self) -> Result<()> {
        log::info!("Starting backend synchronization...");
        let max_retries = 60; // ~2 minutes timeout
        for i in 0..max_retries {
            // 1. Get bitcoind height (source of truth)
            let bitcoind_height = match self.get_block_count().await {
                Ok(h) => h,
                Err(e) => {
                    log::warn!("Attempt {}: Failed to get bitcoind height: {}. Retrying...", i + 1, e);
                    self.sleep_ms(2000).await;
                    continue;
                }
            };

            // 2. Get other service heights
            let metashrew_height_res = self.get_metashrew_height().await;
            let esplora_height_res = self.get_blocks_tip_height().await;
            let ord_height_res = self.get_ord_block_count().await;

            // 3. Check if services are synced
            // All services should be at least at the same height as bitcoind.
            let metashrew_synced = metashrew_height_res.as_ref().is_ok_and(|&h| h >= bitcoind_height);
            let esplora_synced = esplora_height_res.as_ref().is_ok_and(|&h| h >= bitcoind_height);
            let ord_synced = ord_height_res.as_ref().is_ok_and(|&h| h >= bitcoind_height);

            log::info!(
                "Sync attempt {}/{}: bitcoind: {}, metashrew: {} (synced: {}), esplora: {} (synced: {}), ord: {} (synced: {})",
                i + 1,
                max_retries,
                bitcoind_height,
                metashrew_height_res.map_or_else(|e| format!("err ({e})"), |h| h.to_string()),
                metashrew_synced,
                esplora_height_res.map_or_else(|e| format!("err ({e})"), |h| h.to_string()),
                esplora_synced,
                ord_height_res.map_or_else(|e| format!("err ({e})"), |h| h.to_string()),
                ord_synced
            );

            if metashrew_synced && esplora_synced && ord_synced {
                log::info!("✅ All backends synchronized successfully!");
                return Ok(());
            }

            self.sleep_ms(2000).await;
        }

        Err(DeezelError::Other(format!("Timeout waiting for backends to sync after {max_retries} attempts")))
    }
    
    async fn backup(&self) -> Result<String> {
        unimplemented!()
    }
    
    async fn get_mnemonic(&self) -> Result<Option<String>> {
        match &self.wallet_state {
            WalletState::Unlocked { mnemonic, .. } => Ok(Some(mnemonic.clone())),
            _ => Ok(None),
        }
    }
    
    async fn get_master_public_key(&self) -> Result<Option<String>> {
        match &self.wallet_state {
            WalletState::Locked(keystore) | WalletState::Unlocked { keystore, .. } => {
                Ok(Some(keystore.account_xpub.clone()))
            }
            WalletState::None => Ok(None),
        }
    }

    fn get_network(&self) -> bitcoin::Network {
        self.rpc_config.network.0
    }
    
    async fn get_internal_key(&self) -> Result<(bitcoin::XOnlyPublicKey, (Fingerprint, DerivationPath))> {
        let (keystore, mnemonic) = match &self.wallet_state {
            WalletState::Unlocked { keystore, mnemonic } => (keystore, mnemonic),
            _ => return Err(DeezelError::Wallet("Wallet must be unlocked to get internal key".to_string())),
        };

        let mnemonic = bip39::Mnemonic::from_phrase(mnemonic, bip39::Language::English)?;
        let seed = bip39::Seed::new(&mnemonic, "");
        let network = self.get_network();
        let root_key = Xpriv::new_master(network, seed.as_bytes())?;
        
        // Standard path for Taproot internal key. This should be configurable in a real wallet.
        let path = DerivationPath::from_str("m/86'/1'/0'")?;
        
        let derived_xpriv = root_key.derive_priv(&self.secp, &path)?;
        let keypair = derived_xpriv.to_keypair(&self.secp);
        let (internal_key, _) = keypair.x_only_public_key();

        let master_fingerprint = Fingerprint::from_str(&keystore.master_fingerprint)?;

        Ok((internal_key, (master_fingerprint, path)))
    }
    
    async fn sign_psbt(&mut self, psbt: &bitcoin::psbt::Psbt) -> Result<bitcoin::psbt::Psbt> {
        let mut psbt = psbt.clone();
        let mut tx = psbt.clone().extract_tx().map_err(|e| DeezelError::Other(e.to_string()))?;
        let network = self.get_network();
        let secp = Secp256k1::<All>::new();

        let mut prevouts = Vec::new();
        for input in &tx.input {
            let utxo = self.get_utxo(&input.previous_output).await?
                .ok_or_else(|| DeezelError::Wallet(format!("UTXO not found: {}", input.previous_output)))?;
            prevouts.push(utxo);
        }

        let (keystore, mnemonic) = match &mut self.wallet_state {
            WalletState::Unlocked { keystore, mnemonic } => (keystore, mnemonic),
            _ => return Err(DeezelError::Wallet("Wallet must be unlocked to sign transactions".to_string())),
        };

        let mut sighash_cache = SighashCache::new(&mut tx);
        for (i, psbt_input) in psbt.inputs.iter_mut().enumerate() {
            let prev_txout = &prevouts[i];

            if !psbt_input.tap_scripts.is_empty() {
                // Script-path spend
                let (control_block, (script, leaf_version)) = psbt_input.tap_scripts.iter().next().unwrap();
                let leaf_hash = taproot::TapLeafHash::from_script(script, *leaf_version);
                let sighash = sighash_cache.taproot_script_spend_signature_hash(
                    i,
                    &Prevouts::All(&prevouts),
                    leaf_hash,
                    TapSighashType::Default,
                )?;
                
                // Find the keypair corresponding to the internal public key from the PSBT's tap_key_origins.
                // There should be exactly one entry for a script path spend.
                let (internal_pk, (_leaf_hashes, (master_fingerprint, derivation_path))) = psbt_input.tap_key_origins.iter().next()
                    .ok_or_else(|| DeezelError::Wallet("tap_key_origins is empty for script spend".to_string()))?;

                if *master_fingerprint != Fingerprint::from_str(&keystore.master_fingerprint)? {
                    return Err(DeezelError::Wallet(
                        "Master fingerprint mismatch in tap_key_origins".to_string(),
                    ));
                }

                // Derive the private key for this input
                let mnemonic_obj = Mnemonic::from_phrase(mnemonic, bip39::Language::English)?;
                let seed = Seed::new(&mnemonic_obj, "");
                let root_key = Xpriv::new_master(network, seed.as_bytes())?;
                let derived_xpriv = root_key.derive_priv(&secp, derivation_path)?;
                let keypair = derived_xpriv.to_keypair(&secp);

                // Verify that the derived key matches the public key from the PSBT
                if keypair.public_key().x_only_public_key().0 != *internal_pk {
                    return Err(DeezelError::Wallet("Derived key does not match internal public key in PSBT".to_string()));
                }

                let msg = bitcoin::secp256k1::Message::from(sighash);
                
                #[cfg(not(target_arch = "wasm32"))]
                let signature = self.secp.sign_schnorr_with_rng(&msg, &keypair, &mut rand::thread_rng());
                #[cfg(target_arch = "wasm32")]
                let signature = self.secp.sign_schnorr_with_rng(&msg, &keypair, &mut OsRng);

                let taproot_signature = taproot::Signature { signature, sighash_type: TapSighashType::Default };
                
                let mut final_witness = Witness::new();
                final_witness.push(taproot_signature.to_vec());
                final_witness.push(script.as_bytes());
                final_witness.push(control_block.serialize());
                psbt_input.final_script_witness = Some(final_witness);

            } else {
                // Key-path spend
                let address = Address::from_script(&prev_txout.script_pubkey, network)
                    .map_err(|e| DeezelError::Wallet(format!("Failed to parse address from script: {e}")))?;
                
                let addr_info = Self::find_address_info(keystore, &address, network)?;
                let path = DerivationPath::from_str(&addr_info.derivation_path)?;

                let mnemonic_obj = Mnemonic::from_phrase(mnemonic, bip39::Language::English)?;
                let seed = Seed::new(&mnemonic_obj, "");
                let root_key = Xpriv::new_master(network, seed.as_bytes())?;
                let derived_xpriv = root_key.derive_priv(&secp, &path)?;
                let keypair = derived_xpriv.to_keypair(&secp);
                let untweaked_keypair = UntweakedKeypair::from(keypair);
                let tweaked_keypair = untweaked_keypair.tap_tweak(&secp, None);

                let sighash = sighash_cache.taproot_key_spend_signature_hash(
                    i,
                    &Prevouts::All(&prevouts),
                    TapSighashType::Default,
                )?;

                let msg = bitcoin::secp256k1::Message::from(sighash);
                #[cfg(not(target_arch = "wasm32"))]
                let signature = secp.sign_schnorr_with_rng(&msg, &tweaked_keypair.to_keypair(), &mut thread_rng());
                #[cfg(target_arch = "wasm32")]
                let signature = secp.sign_schnorr_with_rng(&msg, &tweaked_keypair.to_keypair(), &mut OsRng);
                
                let taproot_signature = taproot::Signature {
                    signature,
                    sighash_type: TapSighashType::Default,
                };

                psbt_input.tap_key_sig = Some(taproot_signature);
            }
        }

        Ok(psbt)
    }
    
    async fn get_keypair(&self) -> Result<bitcoin::secp256k1::Keypair> {
        let mnemonic = self.get_mnemonic().await?
            .ok_or_else(|| DeezelError::Wallet("Wallet must be unlocked to get keypair".to_string()))?;
        let mnemonic = bip39::Mnemonic::from_phrase(&mnemonic, bip39::Language::English)?;
        let seed = bip39::Seed::new(&mnemonic, "");
        let network = self.get_network();
        let xpriv = bitcoin::bip32::Xpriv::new_master(network, seed.as_bytes())?;
        let secp = bitcoin::secp256k1::Secp256k1::new();
        Ok(xpriv.to_keypair(&secp))
    }

    fn set_passphrase(&mut self, _passphrase: Option<String>) {
    }

    async fn get_last_used_address_index(&self) -> Result<u32> {
        let keystore = self.get_keystore()?;
        let network = self.get_network();
        let mut last_used_index = 0;
        let gap_limit = 20; // Standard gap limit

        // We check both receive (0) and change (1) chains
        for chain in 0..=1 {
            let mut consecutive_unused = 0;
            for index in 0.. {
                // Derive one address at a time
                let addresses = keystore.get_addresses(network, "p2tr", chain, index, 1)?;
                if let Some(address_info) = addresses.first() {
                    let txs = self.get_address_txs(&address_info.address).await?;
                    if txs.as_array().is_none_or(|a| a.is_empty()) {
                        consecutive_unused += 1;
                    } else {
                        last_used_index = core::cmp::max(last_used_index, index);
                        consecutive_unused = 0;
                    }
                } else {
                    // Should not happen if get_addresses works correctly
                    break;
                }

                if consecutive_unused >= gap_limit {
                    break;
                }
            }
        }
        Ok(last_used_index)
    }

    async fn get_enriched_utxos(&self, _addresses: Option<Vec<String>>) -> Result<Vec<EnrichedUtxo>> {
        unimplemented!("get_enriched_utxos is not implemented for ConcreteProvider")
    }

    async fn get_all_balances(&self, _addresses: Option<Vec<String>>) -> Result<AllBalances> {
        unimplemented!("get_all_balances is not implemented for ConcreteProvider")
    }
}






#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl MetashrewRpcProvider for ConcreteProvider {
    async fn get_metashrew_height(&self) -> Result<u64> {
        let rpc_url = self.get_bitcoin_rpc_url().ok_or_else(|| DeezelError::RpcError("Bitcoin RPC URL not configured".to_string()))?;
        let json = self.call(&rpc_url, "metashrew_height", json!([]), 1).await?;
        log::debug!("get_metashrew_height response: {:?}", json);
        if let Some(count) = json.as_u64() {
            return Ok(count);
        }
        if let Some(count_str) = json.as_str() {
            if let Ok(val) = count_str.parse::<u64>() {
                return Ok(val);
            }
        }
        if let Some(obj) = json.as_object() {
            if let Some(result) = obj.get("result") {
                if let Some(count) = result.as_u64() {
                    return Ok(count);
                }
                if let Some(count_str) = result.as_str() {
                    if let Ok(val) = count_str.parse::<u64>() {
                        return Ok(val);
                    }
                }
            }
        }
        Err(DeezelError::RpcError(format!("Invalid metashrew height response: not a u64 or string, got: {}", json)))
    }

    async fn get_state_root(&self, height: JsonValue) -> Result<String> {
        let rpc_url = self.get_bitcoin_rpc_url().ok_or_else(|| DeezelError::RpcError("Bitcoin RPC URL not configured".to_string()))?;
        let params = serde_json::json!([height]);
        let result = self.call(&rpc_url, "metashrew_stateroot", params, 1).await?;
        result.as_str().map(|s| s.to_string()).ok_or_else(|| DeezelError::RpcError("Invalid state root response".to_string()))
    }

    async fn get_contract_meta(&self, block: &str, tx: &str) -> Result<serde_json::Value> {
        let rpc_url = self.get_bitcoin_rpc_url().ok_or_else(|| DeezelError::RpcError("Bitcoin RPC URL not configured".to_string()))?;
        let params = serde_json::json!([block, tx]);
        self.call(&rpc_url, "metashrew_view", params, 1).await
    }
    
    async fn trace_outpoint(&self, txid: &str, vout: u32) -> Result<JsonValue> {
        let txid_parsed = bitcoin::Txid::from_str(txid)?;
        let mut outpoint_pb = alkanes_pb::Outpoint::default();
        // The metashrew_view `trace` method expects the raw txid bytes (little-endian),
        // which is how the `bitcoin::Txid` type stores them internally.
        // We do not need to reverse them.
        outpoint_pb.txid = txid_parsed.to_raw_hash().to_byte_array().to_vec();
        outpoint_pb.vout = vout;

        let hex_input = format!("0x{}", hex::encode(outpoint_pb.write_to_bytes()?));
        let response_bytes = self
            .metashrew_view_call("trace", &hex_input, "latest")
            .await?;
        if response_bytes.is_empty() {
            return Ok(JsonValue::Null);
        }
        // The response from `trace` is already JSON, so we parse it directly.
        let trace_json: JsonValue = serde_json::from_slice(&response_bytes)?;
        Ok(trace_json)
    }
    
    async fn get_spendables_by_address(&self, address: &str) -> Result<serde_json::Value> {
        let rpc_url = self.get_bitcoin_rpc_url().ok_or_else(|| DeezelError::RpcError("Bitcoin RPC URL not configured".to_string()))?;
        let params = serde_json::json!([address]);
        self.call(&rpc_url, "spendablesbyaddress", params, 1).await
    }
    
    async fn get_protorunes_by_address(
        &self,
        address: &str,
        block_tag: Option<String>,
        _protocol_tag: u128,
    ) -> Result<crate::alkanes::protorunes::ProtoruneWalletResponse> {
        let mut request = protorune_pb::ProtorunesWalletRequest::default();
        request.wallet = address.as_bytes().to_vec();
        // request.protocol_tag = Some(crate::utils::to_uint128(protocol_tag));
        let hex_input = format!("0x{}", hex::encode(request.write_to_bytes()?));
        let response_bytes = self
            .metashrew_view_call(
                "protorunesbyaddress",
                &hex_input,
                block_tag.as_deref().unwrap_or("latest"),
            )
            .await?;
        if response_bytes.is_empty() {
            return Ok(crate::alkanes::protorunes::ProtoruneWalletResponse {
                balances: vec![],
            });
        }
        let wallet_response = protorune_pb::WalletResponse::parse_from_bytes(response_bytes.as_slice())?;
        let mut balances = vec![];
        for item in wallet_response.outpoints.into_iter() {
            let outpoint = item.outpoint.into_option().ok_or_else(|| {
                DeezelError::Other("missing outpoint in wallet response".to_string())
            })?;
            let output = item.output.into_option().ok_or_else(|| {
                DeezelError::Other("missing output in wallet response".to_string())
            })?;
            let balance_sheet_pb = item.balances.into_option().ok_or_else(|| {
                DeezelError::Other("missing balance sheet in wallet response".to_string())
            })?;
            let txid_bytes: [u8; 32] = outpoint.txid.try_into().map_err(|_| {
                DeezelError::Other("invalid txid length in wallet response".to_string())
            })?;
            balances.push(crate::alkanes::protorunes::ProtoruneOutpointResponse {
                output: TxOut {
                    value: Amount::from_sat(output.value),
                    script_pubkey: ScriptBuf::from_bytes(output.script),
                },
                outpoint: OutPoint {
                    txid: bitcoin::Txid::from_byte_array(txid_bytes),
                    vout: outpoint.vout,
                },
                balance_sheet: {
                    let mut balances_map = BTreeMap::new();
                    for entry in balance_sheet_pb.entries {
                        if let Some(rune) = entry.rune.into_option() {
                            if let Some(rune_id) = rune.runeId.into_option() {
                                if let (Some(height), Some(txindex), Some(balance)) = (
                                    rune_id.height.into_option(),
                                    rune_id.txindex.into_option(),
                                    entry.balance.into_option(),
                                ) {
                                    let protorune_id =
                                        crate::alkanes::balance_sheet::ProtoruneRuneId {
                                            block: height.lo as u128,
                                            tx: txindex.lo as u128,
                                        };
                                    balances_map.insert(protorune_id, balance.lo as u128);
                                }
                            }
                        }
                    }
                    crate::alkanes::balance_sheet::BalanceSheet {
                        cached: crate::alkanes::balance_sheet::CachedBalanceSheet {
                            balances: balances_map,
                        },
                        load_ptrs: vec![],
                    }
                },
            });
        }
        Ok(crate::alkanes::protorunes::ProtoruneWalletResponse { balances })
    }

    async fn get_protorunes_by_outpoint(
        &self,
        txid: &str,
        vout: u32,
        block_tag: Option<String>,
        _protocol_tag: u128,
    ) -> Result<crate::alkanes::protorunes::ProtoruneOutpointResponse> {
        let txid = bitcoin::Txid::from_str(txid)?;
        let outpoint = bitcoin::OutPoint { txid, vout };
        let mut request = protorune_pb::OutpointWithProtocol::default();
        let mut txid_bytes = txid.to_byte_array().to_vec();
        txid_bytes.reverse();
        request.txid = txid_bytes;
        request.vout = outpoint.vout;
        // request.protocol = Some(crate::utils::to_uint128(protocol_tag));
        let hex_input = format!("0x{}", hex::encode(request.write_to_bytes()?));
        let response_bytes = self
            .metashrew_view_call(
                "protorunesbyoutpoint",
                &hex_input,
                block_tag.as_deref().unwrap_or("latest"),
            )
            .await?;
        if response_bytes.is_empty() {
            return Err(DeezelError::Other(
                "empty response from protorunesbyoutpoint".to_string(),
            ));
        }
        let proto_response = protorune_pb::OutpointResponse::parse_from_bytes(response_bytes.as_slice())?;
        let output = proto_response
            .output
            .into_option().ok_or_else(|| DeezelError::Other("missing output in outpoint response".to_string()))?;
        let balance_sheet_pb = proto_response
            .balances
            .into_option()
            .ok_or_else(|| {
                DeezelError::Other("missing balance sheet in outpoint response".to_string())
            })?;
        Ok(crate::alkanes::protorunes::ProtoruneOutpointResponse {
            output: TxOut {
                value: Amount::from_sat(output.value),
                script_pubkey: ScriptBuf::from_bytes(output.script),
            },
            outpoint,
            balance_sheet: {
                let mut balances_map = BTreeMap::new();
                for entry in balance_sheet_pb.entries {
                    if let Some(rune) = entry.rune.into_option() {
                        if let Some(rune_id) = rune.runeId.into_option() {
                            if let (Some(height), Some(txindex), Some(balance)) = (
                                rune_id.height.into_option(),
                                rune_id.txindex.into_option(),
                                entry.balance.into_option(),
                            ) {
                                let protorune_id =
                                    crate::alkanes::balance_sheet::ProtoruneRuneId {
                                        block: height.lo as u128,
                                        tx: txindex.lo as u128,
                                    };
                                balances_map.insert(protorune_id, balance.lo as u128);
                            }
                        }
                    }
                }
                crate::alkanes::balance_sheet::BalanceSheet {
                    cached: crate::alkanes::balance_sheet::CachedBalanceSheet {
                        balances: balances_map,
                    },
                    load_ptrs: vec![],
                }
            },
        })
    }
}



#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl EsploraProvider for ConcreteProvider {
    async fn get_blocks_tip_hash(&self) -> Result<String> {
        let _rpc_url = get_rpc_url(&self.rpc_config, &self.command)?;
        let call_type = determine_rpc_call_type(&self.rpc_config, &self.command);
        #[cfg(feature = "native-deps")]
        if call_type == RpcCallType::Rest {
            let rpc_url = get_rpc_url(&self.rpc_config, &self.command)?;
            let url = format!("{}/blocks/tip/hash", rpc_url);
            log::info!("[EsploraProvider] Using direct HTTP GET to {}", url);
            let response = self.http_client.get(&url).send().await.map_err(|e| DeezelError::Network(e.to_string()))?;
            let text = response.text().await.map_err(|e| DeezelError::Network(e.to_string()))?;
            log::info!("[EsploraProvider] get_blocks_tip_hash response: {}", text);
            return Ok(text);
        }
 
        log::info!("[EsploraProvider] Falling back to JSON-RPC call: {}", crate::esplora::EsploraJsonRpcMethods::BLOCKS_TIP_HASH);
        let rpc_url = get_rpc_url(&self.rpc_config, &self.command)?;
        let result = self.call(&rpc_url, crate::esplora::EsploraJsonRpcMethods::BLOCKS_TIP_HASH, crate::esplora::params::empty(), 1).await?;
        result.as_str().map(|s| s.to_string()).ok_or_else(|| DeezelError::RpcError("Invalid tip hash response".to_string()))
    }

    async fn get_blocks_tip_height(&self) -> Result<u64> {
        log::info!("[EsploraProvider] Calling get_blocks_tip_height");
        let rpc_url = get_rpc_url(&self.rpc_config, &self.command)?;
        let call_type = determine_rpc_call_type(&self.rpc_config, &self.command);
        #[cfg(feature = "native-deps")]
        if call_type == RpcCallType::Rest {
            let url = format!("{}/blocks/tip/height", rpc_url);
            log::info!("[EsploraProvider] Using direct HTTP GET to {}", url);
            let response = self.http_client.get(&url).send().await.map_err(|e| DeezelError::Network(e.to_string()))?;
            let text = response.text().await.map_err(|e| DeezelError::Network(e.to_string()))?;
            log::info!("[EsploraProvider] get_blocks_tip_height response: {}", text);
            return text.parse::<u64>().map_err(|e| DeezelError::RpcError(format!("Invalid tip height response from REST API: {e}")));
        }
        
        log::info!("[EsploraProvider] Falling back to JSON-RPC call: {}", crate::esplora::EsploraJsonRpcMethods::BLOCKS_TIP_HEIGHT);
        let result = self.call(&rpc_url, crate::esplora::EsploraJsonRpcMethods::BLOCKS_TIP_HEIGHT, crate::esplora::params::empty(), 1).await?;
        result.as_u64().ok_or_else(|| DeezelError::RpcError("Invalid tip height response".to_string()))
    }

    async fn get_blocks(&self, start_height: Option<u64>) -> Result<serde_json::Value> {
        log::info!("[EsploraProvider] Calling get_blocks with start_height: {:?}", start_height);
        let rpc_url = get_rpc_url(&self.rpc_config, &self.command)?;
        let call_type = determine_rpc_call_type(&self.rpc_config, &self.command);
        #[cfg(feature = "native-deps")]
        if call_type == RpcCallType::Rest {
            let url = if let Some(height) = start_height {
                format!("{}/blocks/{}", rpc_url, height)
            } else {
                format!("{}/blocks", rpc_url)
            };
            log::info!("[EsploraProvider] Using direct HTTP GET to {}", url);
            let response = self.http_client.get(&url).send().await.map_err(|e| DeezelError::Network(e.to_string()))?;
            let json = response.json().await.map_err(|e| DeezelError::Network(e.to_string()));
            log::info!("[EsploraProvider] get_blocks response: {:?}", json);
            return json;
        }
        
        log::info!("[EsploraProvider] Falling back to JSON-RPC call: {}", crate::esplora::EsploraJsonRpcMethods::BLOCKS);
        self.call(&rpc_url, crate::esplora::EsploraJsonRpcMethods::BLOCKS, crate::esplora::params::optional_single(start_height), 1).await
    }

    async fn get_block_by_height(&self, height: u64) -> Result<String> {
        log::info!("[EsploraProvider] Calling get_block_by_height: {}", height);
        let rpc_url = get_rpc_url(&self.rpc_config, &self.command)?;
        let call_type = determine_rpc_call_type(&self.rpc_config, &self.command);
        #[cfg(feature = "native-deps")]
        if call_type == RpcCallType::Rest {
            let url = format!("{}/block-height/{}", rpc_url, height);
            log::info!("[EsploraProvider] Using direct HTTP GET to {}", url);
            let response = self.http_client.get(&url).send().await.map_err(|e| DeezelError::Network(e.to_string()))?;
            let text = response.text().await.map_err(|e| DeezelError::Network(e.to_string()))?;
            log::info!("[EsploraProvider] get_block_by_height response: {}", text);
            return Ok(text);
        }
        
        log::info!("[EsploraProvider] Falling back to JSON-RPC call: {}", crate::esplora::EsploraJsonRpcMethods::BLOCK_HEIGHT);
        let result = self.call(&rpc_url, crate::esplora::EsploraJsonRpcMethods::BLOCK_HEIGHT, crate::esplora::params::single(height), 1).await?;
        result.as_str().map(|s| s.to_string()).ok_or_else(|| DeezelError::RpcError("Invalid block hash response".to_string()))
    }

    async fn get_block(&self, hash: &str) -> Result<serde_json::Value> {
        let rpc_url = get_rpc_url(&self.rpc_config, &self.command)?;
        let call_type = determine_rpc_call_type(&self.rpc_config, &self.command);
        #[cfg(feature = "native-deps")]
        if call_type == RpcCallType::Rest {
            let url = format!("{}/block/{}", rpc_url, hash);
            let response = self.http_client.get(&url).send().await.map_err(|e| DeezelError::Network(e.to_string()))?;
            return response.json().await.map_err(|e| DeezelError::Network(e.to_string()));
        }
        
        self.call(&rpc_url, crate::esplora::EsploraJsonRpcMethods::BLOCK, crate::esplora::params::single(hash), 1).await
    }

    async fn get_block_status(&self, hash: &str) -> Result<serde_json::Value> {
        let rpc_url = get_rpc_url(&self.rpc_config, &self.command)?;
        let call_type = determine_rpc_call_type(&self.rpc_config, &self.command);
        #[cfg(feature = "native-deps")]
        if call_type == RpcCallType::Rest {
            let url = format!("{}/block/{}/status", rpc_url, hash);
            let response = self.http_client.get(&url).send().await.map_err(|e| DeezelError::Network(e.to_string()))?;
            return response.json().await.map_err(|e| DeezelError::Network(e.to_string()));
        }
        
        self.call(&rpc_url, crate::esplora::EsploraJsonRpcMethods::BLOCK_STATUS, crate::esplora::params::single(hash), 1).await
    }

    async fn get_block_txids(&self, hash: &str) -> Result<serde_json::Value> {
        let rpc_url = get_rpc_url(&self.rpc_config, &self.command)?;
        let call_type = determine_rpc_call_type(&self.rpc_config, &self.command);
        #[cfg(feature = "native-deps")]
        if call_type == RpcCallType::Rest {
            let url = format!("{}/block/{}/txids", rpc_url, hash);
            let response = self.http_client.get(&url).send().await.map_err(|e| DeezelError::Network(e.to_string()))?;
            return response.json().await.map_err(|e| DeezelError::Network(e.to_string()));
        }
        
        self.call(&rpc_url, crate::esplora::EsploraJsonRpcMethods::BLOCK_TXIDS, crate::esplora::params::single(hash), 1).await
    }

    async fn get_block_header(&self, hash: &str) -> Result<String> {
        let rpc_url = get_rpc_url(&self.rpc_config, &self.command)?;
        let call_type = determine_rpc_call_type(&self.rpc_config, &self.command);
        #[cfg(feature = "native-deps")]
        if call_type == RpcCallType::Rest {
            let url = format!("{}/block/{}/header", rpc_url, hash);
            let response = self.http_client.get(&url).send().await.map_err(|e| DeezelError::Network(e.to_string()))?;
            return response.text().await.map_err(|e| DeezelError::Network(e.to_string()));
        }
        
        let result = self.call(&rpc_url, crate::esplora::EsploraJsonRpcMethods::BLOCK_HEADER, crate::esplora::params::single(hash), 1).await?;
        result.as_str().map(|s| s.to_string()).ok_or_else(|| DeezelError::RpcError("Invalid block header response".to_string()))
    }

    async fn get_block_raw(&self, hash: &str) -> Result<String> {
        let rpc_url = get_rpc_url(&self.rpc_config, &self.command)?;
        let call_type = determine_rpc_call_type(&self.rpc_config, &self.command);
        #[cfg(feature = "native-deps")]
        if call_type == RpcCallType::Rest {
            let url = format!("{}/block/{}/raw", rpc_url, hash);
            let response = self.http_client.get(&url).send().await.map_err(|e| DeezelError::Network(e.to_string()))?;
            let bytes = response.bytes().await.map_err(|e| DeezelError::Network(e.to_string()))?;
            return Ok(hex::encode(bytes));
        }
        
        let result = self.call(&rpc_url, crate::esplora::EsploraJsonRpcMethods::BLOCK_RAW, crate::esplora::params::single(hash), 1).await?;
        result.as_str().map(|s| s.to_string()).ok_or_else(|| DeezelError::RpcError("Invalid raw block response".to_string()))
    }

    async fn get_block_txid(&self, hash: &str, index: u32) -> Result<String> {
        log::info!("[EsploraProvider] Calling get_block_txid for hash: {}, index: {}", hash, index);
        let rpc_url = get_rpc_url(&self.rpc_config, &self.command)?;
        let call_type = determine_rpc_call_type(&self.rpc_config, &self.command);
        #[cfg(feature = "native-deps")]
        if call_type == RpcCallType::Rest {
            let url = format!("{}/block/{}/txid/{}", rpc_url, hash, index);
            log::info!("[EsploraProvider] Using direct HTTP GET to {}", url);
            let response = self.http_client.get(&url).send().await.map_err(|e| DeezelError::Network(e.to_string()))?;
            let text = response.text().await.map_err(|e| DeezelError::Network(e.to_string()))?;
            log::info!("[EsploraProvider] get_block_txid response: {}", text);
            return Ok(text);
        }
        
        log::info!("[EsploraProvider] Falling back to JSON-RPC call: {}", crate::esplora::EsploraJsonRpcMethods::BLOCK_TXID);
        let result = self.call(&rpc_url, crate::esplora::EsploraJsonRpcMethods::BLOCK_TXID, crate::esplora::params::dual(hash, index), 1).await?;
        result.as_str().map(|s| s.to_string()).ok_or_else(|| DeezelError::RpcError("Invalid txid response".to_string()))
    }

    async fn get_block_txs(&self, hash: &str, start_index: Option<u32>) -> Result<serde_json::Value> {
        let rpc_url = get_rpc_url(&self.rpc_config, &self.command)?;
        let call_type = determine_rpc_call_type(&self.rpc_config, &self.command);
        #[cfg(feature = "native-deps")]
        if call_type == RpcCallType::Rest {
            let url = if let Some(index) = start_index {
                format!("{}/block/{}/txs/{}", rpc_url, hash, index)
            } else {
                format!("{}/block/{}/txs", rpc_url, hash)
            };
            let response = self.http_client.get(&url).send().await.map_err(|e| DeezelError::Network(e.to_string()))?;
            return response.json().await.map_err(|e| DeezelError::Network(e.to_string()));
        }
        
        self.call(&rpc_url, crate::esplora::EsploraJsonRpcMethods::BLOCK_TXS, crate::esplora::params::optional_dual(hash, start_index), 1).await
    }


   async fn get_address_info(&self, address: &str) -> Result<serde_json::Value> {
        log::info!("[EsploraProvider] Calling get_address_info for address: {}", address);
        let rpc_url = get_rpc_url(&self.rpc_config, &self.command)?;
        let call_type = determine_rpc_call_type(&self.rpc_config, &self.command);
       #[cfg(feature = "native-deps")]
       if call_type == RpcCallType::Rest {
           let url = format!("{}/address/{}", rpc_url, address);
            log::info!("[EsploraProvider] Using direct HTTP GET to {}", url);
           let response = self.http_client.get(&url).send().await.map_err(|e| DeezelError::Network(e.to_string()))?;
           return response.json().await.map_err(|e| DeezelError::Network(e.to_string()));
       }
       
        log::info!("[EsploraProvider] Falling back to JSON-RPC call: {}", crate::esplora::EsploraJsonRpcMethods::ADDRESS);
       self.call(&rpc_url, crate::esplora::EsploraJsonRpcMethods::ADDRESS, crate::esplora::params::single(address), 1).await
   }

   async fn get_address_utxo(&self, address: &str) -> Result<serde_json::Value> {
        log::info!("[EsploraProvider] Calling get_address_utxo for address: {}", address);
        let rpc_url = get_rpc_url(&self.rpc_config, &self.command)?;
        let call_type = determine_rpc_call_type(&self.rpc_config, &self.command);
       #[cfg(feature = "native-deps")]
       if call_type == RpcCallType::Rest {
           let url = format!("{}/address/{}/utxo", rpc_url, address);
            log::info!("[EsploraProvider] Using direct HTTP GET to {}", url);
           let response = self.http_client.get(&url).send().await.map_err(|e| DeezelError::Network(e.to_string()))?;
           return response.json().await.map_err(|e| DeezelError::Network(e.to_string()));
       }
       
        log::info!("[EsploraProvider] Falling back to JSON-RPC call: {}", crate::esplora::EsploraJsonRpcMethods::ADDRESS_UTXO);
       self.call(&rpc_url, crate::esplora::EsploraJsonRpcMethods::ADDRESS_UTXO, crate::esplora::params::single(address), 1).await
   }

    async fn get_address_txs(&self, address: &str) -> Result<serde_json::Value> {
        let rpc_url = get_rpc_url(&self.rpc_config, &self.command)?;
        let call_type = determine_rpc_call_type(&self.rpc_config, &self.command);
        #[cfg(feature = "native-deps")]
        if call_type == RpcCallType::Rest {
            let url = format!("{}/address/{}/txs", rpc_url, address);
            let response = self.http_client.get(&url).send().await.map_err(|e| DeezelError::Network(e.to_string()))?;
            return response.json().await.map_err(|e| DeezelError::Network(e.to_string()));
        }
        
        self.call(&rpc_url, crate::esplora::EsploraJsonRpcMethods::ADDRESS_TXS, crate::esplora::params::single(address), 1).await
    }

    async fn get_address_txs_chain(&self, address: &str, last_seen_txid: Option<&str>) -> Result<serde_json::Value> {
        let rpc_url = get_rpc_url(&self.rpc_config, &self.command)?;
        let call_type = determine_rpc_call_type(&self.rpc_config, &self.command);
        #[cfg(feature = "native-deps")]
        if call_type == RpcCallType::Rest {
            let url = if let Some(txid) = last_seen_txid {
                format!("{}/address/{}/txs/chain/{}", rpc_url, address, txid)
            } else {
                format!("{}/address/{}/txs/chain", rpc_url, address)
            };
            let response = self.http_client.get(&url).send().await.map_err(|e| DeezelError::Network(e.to_string()))?;
            return response.json().await.map_err(|e| DeezelError::Network(e.to_string()));
        }
        
        self.call(&rpc_url, crate::esplora::EsploraJsonRpcMethods::ADDRESS_TXS_CHAIN, crate::esplora::params::optional_dual(address, last_seen_txid), 1).await
    }

    async fn get_address_txs_mempool(&self, address: &str) -> Result<serde_json::Value> {
        let rpc_url = get_rpc_url(&self.rpc_config, &self.command)?;
        let call_type = determine_rpc_call_type(&self.rpc_config, &self.command);
        #[cfg(feature = "native-deps")]
        if call_type == RpcCallType::Rest {
            let url = format!("{}/address/{}/txs/mempool", rpc_url, address);
            let response = self.http_client.get(&url).send().await.map_err(|e| DeezelError::Network(e.to_string()))?;
            return response.json().await.map_err(|e| DeezelError::Network(e.to_string()));
        }
        
        self.call(&rpc_url, crate::esplora::EsploraJsonRpcMethods::ADDRESS_TXS_MEMPOOL, crate::esplora::params::single(address), 1).await
    }


    async fn get_address_prefix(&self, prefix: &str) -> Result<serde_json::Value> {
        let rpc_url = get_rpc_url(&self.rpc_config, &self.command)?;
        let call_type = determine_rpc_call_type(&self.rpc_config, &self.command);
        #[cfg(feature = "native-deps")]
        if call_type == RpcCallType::Rest {
            let url = format!("{}/address-prefix/{}", rpc_url, prefix);
            let response = self.http_client.get(&url).send().await.map_err(|e| DeezelError::Network(e.to_string()))?;
            return response.json().await.map_err(|e| DeezelError::Network(e.to_string()));
        }
        
        self.call(&rpc_url, crate::esplora::EsploraJsonRpcMethods::ADDRESS_PREFIX, crate::esplora::params::single(prefix), 1).await
    }

    async fn get_tx(&self, txid: &str) -> Result<serde_json::Value> {
        log::info!("[EsploraProvider] Calling get_tx for txid: {}", txid);
        let rpc_url = get_rpc_url(&self.rpc_config, &self.command)?;
        let call_type = determine_rpc_call_type(&self.rpc_config, &self.command);
        #[cfg(feature = "native-deps")]
        if call_type == RpcCallType::Rest {
            let url = format!("{}/tx/{}", rpc_url, txid);
            log::info!("[EsploraProvider] Using direct HTTP GET to {}", url);
            let response = self.http_client.get(&url).send().await.map_err(|e| DeezelError::Network(e.to_string()))?;
            return response.json().await.map_err(|e| DeezelError::Network(e.to_string()));
        }
        
        log::info!("[EsploraProvider] Falling back to JSON-RPC call: {}", crate::esplora::EsploraJsonRpcMethods::TX);
        self.call(&rpc_url, crate::esplora::EsploraJsonRpcMethods::TX, crate::esplora::params::single(txid), 1).await
    }

    async fn get_tx_hex(&self, txid: &str) -> Result<String> {
        log::info!("[EsploraProvider] Calling get_tx_hex for txid: {}", txid);
        let rpc_url = get_rpc_url(&self.rpc_config, &self.command)?;
        let call_type = determine_rpc_call_type(&self.rpc_config, &self.command);
        #[cfg(feature = "native-deps")]
        if call_type == RpcCallType::Rest {
            let url = format!("{}/tx/{}/hex", rpc_url, txid);
            log::info!("[EsploraProvider] Using direct HTTP GET to {}", url);
            let response = self.http_client.get(&url).send().await.map_err(|e| DeezelError::Network(e.to_string()))?;
            let text = response.text().await.map_err(|e| DeezelError::Network(e.to_string()))?;
            log::info!("[EsploraProvider] get_tx_hex response: {}", text);
            return Ok(text);
        }
        
        log::info!("[EsploraProvider] Falling back to JSON-RPC call: {}", crate::esplora::EsploraJsonRpcMethods::TX_HEX);
        let result = self.call(&rpc_url, crate::esplora::EsploraJsonRpcMethods::TX_HEX, crate::esplora::params::single(txid), 1).await?;
        result.as_str().map(|s| s.to_string()).ok_or_else(|| DeezelError::RpcError("Invalid tx hex response".to_string()))
    }

    async fn get_tx_raw(&self, txid: &str) -> Result<String> {
        let rpc_url = get_rpc_url(&self.rpc_config, &self.command)?;
        let call_type = determine_rpc_call_type(&self.rpc_config, &self.command);
        #[cfg(feature = "native-deps")]
        if call_type == RpcCallType::Rest {
            let url = format!("{}/tx/{}/raw", rpc_url, txid);
            let response = self.http_client.get(&url).send().await.map_err(|e| DeezelError::Network(e.to_string()))?;
            let bytes = response.bytes().await.map_err(|e| DeezelError::Network(e.to_string()))?;
            return Ok(hex::encode(bytes));
        }
        
        let result = self.call(&rpc_url, crate::esplora::EsploraJsonRpcMethods::TX_RAW, crate::esplora::params::single(txid), 1).await?;
        result.as_str().map(|s| s.to_string()).ok_or_else(|| DeezelError::RpcError("Invalid raw tx response".to_string()))
    }

    async fn get_tx_status(&self, txid: &str) -> Result<serde_json::Value> {
        let rpc_url = get_rpc_url(&self.rpc_config, &self.command)?;
        let call_type = determine_rpc_call_type(&self.rpc_config, &self.command);
        #[cfg(feature = "native-deps")]
        if call_type == RpcCallType::Rest {
            let url = format!("{}/tx/{}/status", rpc_url, txid);
            let response = self.http_client.get(&url).send().await.map_err(|e| DeezelError::Network(e.to_string()))?;
            return response.json().await.map_err(|e| DeezelError::Network(e.to_string()));
        }
        
        self.call(&rpc_url, crate::esplora::EsploraJsonRpcMethods::TX_STATUS, crate::esplora::params::single(txid), 1).await
    }

    async fn get_tx_merkle_proof(&self, txid: &str) -> Result<serde_json::Value> {
        let rpc_url = get_rpc_url(&self.rpc_config, &self.command)?;
        let call_type = determine_rpc_call_type(&self.rpc_config, &self.command);
        #[cfg(feature = "native-deps")]
        if call_type == RpcCallType::Rest {
            let url = format!("{}/tx/{}/merkle-proof", rpc_url, txid);
            let response = self.http_client.get(&url).send().await.map_err(|e| DeezelError::Network(e.to_string()))?;
            return response.json().await.map_err(|e| DeezelError::Network(e.to_string()));
        }
        
        self.call(&rpc_url, crate::esplora::EsploraJsonRpcMethods::TX_MERKLE_PROOF, crate::esplora::params::single(txid), 1).await
    }

    async fn get_tx_merkleblock_proof(&self, txid: &str) -> Result<String> {
        let rpc_url = get_rpc_url(&self.rpc_config, &self.command)?;
        let call_type = determine_rpc_call_type(&self.rpc_config, &self.command);
        #[cfg(feature = "native-deps")]
        if call_type == RpcCallType::Rest {
            let url = format!("{}/tx/{}/merkleblock-proof", rpc_url, txid);
            let response = self.http_client.get(&url).send().await.map_err(|e| DeezelError::Network(e.to_string()))?;
            return response.text().await.map_err(|e| DeezelError::Network(e.to_string()));
        }
        
        let result = self.call(&rpc_url, crate::esplora::EsploraJsonRpcMethods::TX_MERKLEBLOCK_PROOF, crate::esplora::params::single(txid), 1).await?;
        result.as_str().map(|s| s.to_string()).ok_or_else(|| DeezelError::RpcError("Invalid merkleblock proof response".to_string()))
    }

    async fn get_tx_outspend(&self, txid: &str, index: u32) -> Result<serde_json::Value> {
        let rpc_url = get_rpc_url(&self.rpc_config, &self.command)?;
        let call_type = determine_rpc_call_type(&self.rpc_config, &self.command);
        #[cfg(feature = "native-deps")]
        if call_type == RpcCallType::Rest {
            let url = format!("{}/tx/{}/outspend/{}", rpc_url, txid, index);
            let response = self.http_client.get(&url).send().await.map_err(|e| DeezelError::Network(e.to_string()))?;
            return response.json().await.map_err(|e| DeezelError::Network(e.to_string()));
        }
        
        self.call(&rpc_url, crate::esplora::EsploraJsonRpcMethods::TX_OUTSPEND, crate::esplora::params::dual(txid, index), 1).await
    }

    async fn get_tx_outspends(&self, txid: &str) -> Result<serde_json::Value> {
        let rpc_url = get_rpc_url(&self.rpc_config, &self.command)?;
        let call_type = determine_rpc_call_type(&self.rpc_config, &self.command);
        #[cfg(feature = "native-deps")]
        if call_type == RpcCallType::Rest {
            let url = format!("{}/tx/{}/outspends", rpc_url, txid);
            let response = self.http_client.get(&url).send().await.map_err(|e| DeezelError::Network(e.to_string()))?;
            return response.json().await.map_err(|e| DeezelError::Network(e.to_string()));
        }
        
        self.call(&rpc_url, crate::esplora::EsploraJsonRpcMethods::TX_OUTSPENDS, crate::esplora::params::single(txid), 1).await
    }

    async fn broadcast(&self, tx_hex: &str) -> Result<String> {
        let rpc_url = get_rpc_url(&self.rpc_config, &self.command)?;
        let call_type = determine_rpc_call_type(&self.rpc_config, &self.command);
        #[cfg(feature = "native-deps")]
        if call_type == RpcCallType::Rest {
            let url = format!("{}/tx", rpc_url);
            let response = self.http_client.post(&url).body(tx_hex.to_string()).send().await.map_err(|e| DeezelError::Network(e.to_string()))?;
            return response.text().await.map_err(|e| DeezelError::Network(e.to_string()));
        }
        
        let result = self.call(&rpc_url, crate::esplora::EsploraJsonRpcMethods::BROADCAST, crate::esplora::params::single(tx_hex), 1).await?;
        result.as_str().map(|s| s.to_string()).ok_or_else(|| DeezelError::RpcError("Invalid broadcast response".to_string()))
    }

    async fn get_mempool(&self) -> Result<serde_json::Value> {
        let rpc_url = get_rpc_url(&self.rpc_config, &self.command)?;
        let call_type = determine_rpc_call_type(&self.rpc_config, &self.command);
        #[cfg(feature = "native-deps")]
        if call_type == RpcCallType::Rest {
            let url = format!("{}/mempool", rpc_url);
            let response = self.http_client.get(&url).send().await.map_err(|e| DeezelError::Network(e.to_string()))?;
            return response.json().await.map_err(|e| DeezelError::Network(e.to_string()));
        }
        
        self.call(&rpc_url, crate::esplora::EsploraJsonRpcMethods::MEMPOOL, crate::esplora::params::empty(), 1).await
    }

    async fn get_mempool_txids(&self) -> Result<serde_json::Value> {
        let rpc_url = get_rpc_url(&self.rpc_config, &self.command)?;
        let call_type = determine_rpc_call_type(&self.rpc_config, &self.command);
        #[cfg(feature = "native-deps")]
        if call_type == RpcCallType::Rest {
            let url = format!("{}/mempool/txids", rpc_url);
            let response = self.http_client.get(&url).send().await.map_err(|e| DeezelError::Network(e.to_string()))?;
            return response.json().await.map_err(|e| DeezelError::Network(e.to_string()));
        }
        
        self.call(&rpc_url, crate::esplora::EsploraJsonRpcMethods::MEMPOOL_TXIDS, crate::esplora::params::empty(), 1).await
    }

    async fn get_mempool_recent(&self) -> Result<serde_json::Value> {
        let rpc_url = get_rpc_url(&self.rpc_config, &self.command)?;
        let call_type = determine_rpc_call_type(&self.rpc_config, &self.command);
        #[cfg(feature = "native-deps")]
        if call_type == RpcCallType::Rest {
            let url = format!("{}/mempool/recent", rpc_url);
            let response = self.http_client.get(&url).send().await.map_err(|e| DeezelError::Network(e.to_string()))?;
            return response.json().await.map_err(|e| DeezelError::Network(e.to_string()));
        }
        
        self.call(&rpc_url, crate::esplora::EsploraJsonRpcMethods::MEMPOOL_RECENT, crate::esplora::params::empty(), 1).await
    }

    async fn get_fee_estimates(&self) -> Result<serde_json::Value> {
        let rpc_url = get_rpc_url(&self.rpc_config, &self.command)?;
        let call_type = determine_rpc_call_type(&self.rpc_config, &self.command);
        #[cfg(feature = "native-deps")]
        if call_type == RpcCallType::Rest {
            let url = format!("{}/fee-estimates", rpc_url);
            let response = self.http_client.get(&url).send().await.map_err(|e| DeezelError::Network(e.to_string()))?;
            return response.json().await.map_err(|e| DeezelError::Network(e.to_string()));
        }
        
        self.call(&rpc_url, crate::esplora::EsploraJsonRpcMethods::FEE_ESTIMATES, crate::esplora::params::empty(), 1).await
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl RunestoneProvider for ConcreteProvider {
    async fn decode_runestone(&self, tx: &Transaction) -> Result<serde_json::Value> {
        if let Some(artifact) = Runestone::decipher(tx) {
            match artifact {
                Artifact::Runestone(runestone) => Ok(serde_json::to_value(runestone)?),
                Artifact::Cenotaph(cenotaph) => Err(DeezelError::Runestone(format!("Cenotaph found: {cenotaph:?}"))),
            }
        } else {
            Err(DeezelError::Runestone("No runestone found in transaction".to_string()))
        }
    }

    async fn format_runestone_with_decoded_messages(&self, tx: &Transaction) -> Result<serde_json::Value> {
        if let Some(artifact) = Runestone::decipher(tx) {
            match artifact {
                Artifact::Runestone(runestone) => {
                    Ok(serde_json::json!({
                        "runestone": runestone,
                        "decoded_messages": format!("{:?}", runestone)
                    }))
                },
                Artifact::Cenotaph(cenotaph) => Err(DeezelError::Runestone(format!("Cenotaph found: {cenotaph:?}"))),
            }
        } else {
            Err(DeezelError::Runestone("No runestone found in transaction".to_string()))
        }
    }

    async fn analyze_runestone(&self, txid: &str) -> Result<serde_json::Value> {
        let tx_hex = self.get_tx_hex(txid).await?;
        let tx_bytes = hex::decode(&tx_hex)?;
        let tx: Transaction = bitcoin::consensus::deserialize(&tx_bytes)?;
        self.decode_runestone(&tx).await
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl AlkanesProvider for ConcreteProvider {
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
    ) -> Result<crate::alkanes::protorunes::ProtoruneWalletResponse> {
        <Self as MetashrewRpcProvider>::get_protorunes_by_address(self, address, block_tag, protocol_tag).await
    }

    async fn protorunes_by_outpoint(
        &self,
        txid: &str,
        vout: u32,
        block_tag: Option<String>,
        protocol_tag: u128,
    ) -> Result<crate::alkanes::protorunes::ProtoruneOutpointResponse> {
        <Self as MetashrewRpcProvider>::get_protorunes_by_outpoint(self, txid, vout, block_tag, protocol_tag).await
    }

    async fn view(&self, contract_id: &str, view_fn: &str, params: Option<&[u8]>) -> Result<JsonValue> {
        let combined_view = format!("{}/{}", contract_id, view_fn);
        let params_hex = params.map(|p| format!("0x{}", hex::encode(p))).unwrap_or_else(|| "0x".to_string());
        let result_bytes = self.metashrew_view_call(&combined_view, &params_hex, "latest").await?;

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

    async fn simulate(&self, contract_id: &str, context: &alkanes_pb::MessageContextParcel) -> Result<JsonValue> {
        let mut buf = Vec::new();
        context.write_to_writer(&mut buf)?;
        let params_hex = format!("0x{}", hex::encode(buf));
        let rpc_params = serde_json::json!([contract_id, params_hex]);
        self.call(self.rpc_config.metashrew_rpc_url.as_deref().ok_or_else(|| DeezelError::RpcError("Metashrew RPC URL not configured".to_string()))?, "alkanes_simulate", rpc_params, 1).await
    }

    async fn trace(&self, outpoint: &str) -> Result<alkanes_pb::Trace> {
        let parts: Vec<&str> = outpoint.split(':').collect();
        if parts.len() != 2 {
            return Err(DeezelError::InvalidParameters("Invalid outpoint format. Expected 'txid:vout'".to_string()));
        }
        let txid = bitcoin::Txid::from_str(parts[0])?;
        let vout = parts[1].parse::<u32>()?;

        let mut out_point_pb = alkanes_pb::Outpoint::default();
        out_point_pb.txid = txid.to_raw_hash().as_byte_array().to_vec();
        out_point_pb.vout = vout;

        let hex_input = format!("0x{}", hex::encode(out_point_pb.write_to_bytes()?));
        let response_bytes = self.metashrew_view_call("trace", &hex_input, "latest").await?;
        
        let trace = alkanes_pb::Trace::parse_from_bytes(response_bytes.as_slice())?;
        Ok(trace)
    }

    async fn get_block(&self, height: u64) -> Result<alkanes_pb::BlockResponse> {
        let mut block_request = alkanes_pb::BlockRequest::default();
        block_request.height = height as u32;
        
        let hex_input = format!("0x{}", hex::encode(block_request.write_to_bytes()?));
        let response_bytes = self.metashrew_view_call("getblock", &hex_input, "latest").await?;

        let block_response = alkanes_pb::BlockResponse::parse_from_bytes(response_bytes.as_slice())?;
        Ok(block_response)
    }

    async fn sequence(&self) -> Result<JsonValue> {
        let response_bytes = self.metashrew_view_call("sequence", "0x", "latest").await?;
        if response_bytes.len() == 16 {
            let val = u128::from_le_bytes(response_bytes.try_into().unwrap());
            return Ok(serde_json::json!(val));
        }
        Ok(serde_json::json!(format!("0x{}", hex::encode(response_bytes))))
    }

    async fn spendables_by_address(&self, address: &str) -> Result<JsonValue> {
        let mut request = protorune_pb::WalletRequest::default();
        request.wallet = address.as_bytes().to_vec();
        let hex_input = format!("0x{}", hex::encode(request.write_to_bytes()?));
        let response_bytes = self
            .metashrew_view_call("spendablesbyaddress", &hex_input, "latest")
            .await?;
        if response_bytes.is_empty() {
            return Ok(serde_json::json!([]));
        }
        let wallet_response = protorune_pb::WalletResponse::parse_from_bytes(response_bytes.as_slice())?;
        let entries: Vec<serde_json::Value> = wallet_response.outpoints.into_iter().map(|item| {
            serde_json::json!({
                "outpoint": {
                    "txid": hex::encode(item.outpoint.as_ref().map_or(vec![], |o| o.txid.clone())),
                    "vout": item.outpoint.as_ref().map_or(0, |o| o.vout),
                },
                "amount": item.output.as_ref().map_or(0, |o| o.value),
                "script": hex::encode(item.output.as_ref().map_or(vec![], |o| o.script.clone())),
                "runes": item.balances.into_option().map(|b| b.entries).unwrap_or_default().iter().map(|entry| {
                    serde_json::json!({
                        "runeId": {
                            "height": entry.rune.as_ref().and_then(|r| r.runeId.as_ref()).and_then(|id| id.height.as_ref()).map_or(0, |h| h.lo),
                            "txindex": entry.rune.as_ref().and_then(|r| r.runeId.as_ref()).and_then(|id| id.txindex.as_ref()).map_or(0, |t| t.lo),
                        },
                        "amount": entry.balance.as_ref().map_or(0, |a| a.lo),
                    })
                }).collect::<Vec<_>>(),
            })
        }).collect();
        Ok(serde_json::json!(entries))
    }

    async fn trace_block(&self, height: u64) -> Result<alkanes_pb::Trace> {
        let mut block_request = alkanes_pb::BlockRequest::default();
        block_request.height = height as u32;
        
        let hex_input = format!("0x{}", hex::encode(block_request.write_to_bytes()?));
        let response_bytes = self.metashrew_view_call("traceblock", &hex_input, "latest").await?;

        let trace = alkanes_pb::Trace::parse_from_bytes(response_bytes.as_slice())?;
        Ok(trace)
    }

    async fn get_bytecode(&self, alkane_id: &str, block_tag: Option<String>) -> Result<String> {
        let parts: Vec<&str> = alkane_id.split(':').collect();
        if parts.len() != 2 {
            return Err(DeezelError::InvalidParameters("Invalid alkane_id format. Expected 'block:tx'".to_string()));
        }
        let block = parts[0].parse::<u64>()?;
        let tx = parts[1].parse::<u64>()?;

        let mut alkane_id_pb = alkanes_pb::AlkaneId::default();
        let mut block_uint128 = alkanes_pb::Uint128::default();
        block_uint128.lo = block;
        let mut tx_uint128 = alkanes_pb::Uint128::default();
        tx_uint128.lo = tx;
        alkane_id_pb.block = Some(block_uint128).into();
        alkane_id_pb.tx = Some(tx_uint128).into();

        let mut request = alkanes_pb::BytecodeRequest::default();
        request.id = Some(alkane_id_pb).into();

        let hex_input = format!("0x{}", hex::encode(request.write_to_bytes()?));
        self.info(&format!(
            "[get_bytecode] Calling metashrew_view with view_fn: getbytecode, params: {}",
            hex_input
        ));
        let response_bytes = self.metashrew_view_call("getbytecode", &hex_input, block_tag.as_deref().unwrap_or("latest")).await?;
        self.info(&format!(
            "[get_bytecode] Received response: 0x{}",
            hex::encode(&response_bytes)
        ));
        Ok(format!("0x{}", hex::encode(response_bytes)))
    }

    #[cfg(feature = "wasm-inspection")]
    async fn inspect(
  &self,
  target: &str,
  config: AlkanesInspectConfig,
 ) -> Result<AlkanesInspectResult> {
  let inspector = AlkaneInspector::new(self.clone());
  let parts: Vec<&str> = target.split(':').collect();
  if parts.len() != 2 {
   return Err(DeezelError::InvalidParameters(
    "Invalid target format. Expected 'block:tx'".to_string(),
   ));
  }
  let block = parts[0].parse::<u64>()?;
  let tx = parts[1].parse::<u64>()?;
  let alkane_id = AlkaneId { block, tx };
  let inspection_config = InspectionConfig {
   disasm: config.disasm,
   fuzz: config.fuzz,
   fuzz_ranges: config.fuzz_ranges,
   meta: config.meta,
   codehash: config.codehash,
   raw: config.raw,
  };
  let result = inspector.inspect_alkane(&alkane_id, &inspection_config).await.map_err(|e| DeezelError::Other(e.to_string()))?;
  Ok(serde_json::from_value(serde_json::to_value(result)?)?)
 }

    #[cfg(not(feature = "wasm-inspection"))]
    async fn inspect(
        &self,
        _target: &str,
        _config: AlkanesInspectConfig,
    ) -> Result<AlkanesInspectResult> {
        Err(DeezelError::NotImplemented(
            "Alkanes inspection is not available without the 'wasm-inspection' feature".to_string(),
        ))
    }

    async fn get_balance(&self, address: Option<&str>) -> Result<Vec<AlkaneBalance>> {
        let addr_str = match address {
            Some(a) => a.to_string(),
            None => WalletProvider::get_address(self).await?,
        };
        let mut request = protorune_pb::WalletRequest::default();
        request.wallet = addr_str.as_bytes().to_vec();
        let hex_input = format!("0x{}", hex::encode(request.write_to_bytes()?));
        let response_bytes = self
            .metashrew_view_call("balancesbyaddress", &hex_input, "latest")
            .await?;
        if response_bytes.is_empty() {
            return Ok(vec![]);
        }
        let proto_sheet = protorune_pb::BalanceSheet::parse_from_bytes(response_bytes.as_slice())?;

        let result: Vec<AlkaneBalance> = proto_sheet
            .entries
            .into_iter()
            .map(|item| {
                let (alkane_id, name, symbol) = item.rune.into_option().map_or(
                    (AlkaneId { block: 0, tx: 0 }, String::new(), String::new()),
                    |r| {
                        let id = r.runeId.into_option().map_or(AlkaneId { block: 0, tx: 0 }, |rid| AlkaneId {
                            block: rid.height.into_option().map_or(0, |b| b.lo),
                            tx: rid.txindex.into_option().map_or(0, |t| t.lo),
                        });
                        (id, r.name.clone(), r.symbol.clone())
                    },
                );

                let balance = item.balance.into_option().map_or(0, |b| b.lo);

                AlkaneBalance { alkane_id, name, symbol, balance }
            })
            .collect();

        Ok(result)
    }
}
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl DeezelProvider for ConcreteProvider {
    fn provider_name(&self) -> &str {
        "ConcreteProvider"
    }

    fn get_bitcoin_rpc_url(&self) -> Option<String> {
        self.rpc_config.bitcoin_rpc_url.clone()
    }

    fn get_esplora_api_url(&self) -> Option<String> {
        self.rpc_config.esplora_url.clone()
    }

    fn get_ord_server_url(&self) -> Option<String> {
        self.rpc_config.ord_url.clone()
    }

    fn get_metashrew_rpc_url(&self) -> Option<String> {
        self.rpc_config.metashrew_rpc_url.clone()
    }

    fn clone_box(&self) -> Box<dyn DeezelProvider> {
        Box::new(self.clone())
    }

    async fn initialize(&self) -> Result<()> {
        Ok(())
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    fn secp(&self) -> &Secp256k1<All> {
        &self.secp
    }

    async fn get_utxo(&self, outpoint: &OutPoint) -> Result<Option<TxOut>> {
        let tx_info = self.get_tx(&outpoint.txid.to_string()).await?;
        let vout_info = tx_info["vout"].get(outpoint.vout as usize)
            .ok_or_else(|| DeezelError::Wallet(format!("Vout {} not found for tx {}", outpoint.vout, outpoint.txid)))?;

        let amount = vout_info["value"].as_u64()
            .ok_or_else(|| DeezelError::Wallet("UTXO value not found".to_string()))?;
        let script_pubkey_hex = vout_info["scriptpubkey"].as_str()
            .ok_or_else(|| DeezelError::Wallet("UTXO script pubkey not found".to_string()))?;

        let script_pubkey = ScriptBuf::from(Vec::from_hex(script_pubkey_hex)?);
        Ok(Some(TxOut { value: Amount::from_sat(amount), script_pubkey }))
    }

    async fn sign_taproot_script_spend(&self, sighash: bitcoin::secp256k1::Message) -> Result<bitcoin::secp256k1::schnorr::Signature> {
        let mnemonic = match &self.wallet_state {
            WalletState::Unlocked { mnemonic, .. } => mnemonic,
            _ => return Err(DeezelError::Wallet("Wallet must be unlocked to sign".to_string())),
        };
        let mnemonic = bip39::Mnemonic::from_phrase(mnemonic, bip39::Language::English)?;
        let seed = bip39::Seed::new(&mnemonic, "");
        let network = self.get_network();
        let root_key = Xpriv::new_master(network, seed.as_bytes())?;
        let keypair = root_key.to_keypair(&self.secp);
        #[cfg(not(target_arch = "wasm32"))]
        let signature = self.secp.sign_schnorr_with_rng(&sighash, &keypair, &mut rand::thread_rng());
        #[cfg(target_arch = "wasm32")]
        let signature = self.secp.sign_schnorr_with_rng(&sighash, &keypair, &mut OsRng);
        Ok(signature)
    }

    async fn wrap(&mut self, _amount: u64, _address: Option<String>, _fee_rate: Option<f32>) -> Result<String> {
        Err(DeezelError::NotImplemented("wrap".to_string()))
    }

    async fn unwrap(&mut self, _amount: u64, _address: Option<String>) -> Result<String> {
        Err(DeezelError::NotImplemented("unwrap".to_string()))
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl AddressResolver for ConcreteProvider {
    async fn resolve_all_identifiers(&self, input: &str) -> Result<String> {
        let mut resolver = crate::address_resolver::AddressResolver::new(self.clone());
        resolver.resolve_all_identifiers(input).await
    }

    fn contains_identifiers(&self, input: &str) -> bool {
        let resolver = crate::address_resolver::AddressResolver::new(self.clone());
        resolver.contains_identifiers(input)
    }

    async fn get_address(&self, address_type: &str, index: u32) -> Result<String> {
        if address_type != "p2tr" {
            return Err(DeezelError::Wallet("Only p2tr addresses are supported".to_string()));
        }
        let addresses = WalletProvider::get_addresses(self, index + 1).await?;
        addresses.get(index as usize)
            .map(|a| a.address.clone())
            .ok_or_else(|| DeezelError::Wallet(format!("Address with index {index} not found")))
    }

    async fn list_identifiers(&self) -> Result<Vec<String>> {
        // This is a placeholder. A real implementation would inspect the wallet.
        Ok(vec!["[self:p2tr:0]".to_string(), "[self:p2tr:1]".to_string()])
    }
}


#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl MetashrewProvider for ConcreteProvider {
    async fn get_height(&self) -> Result<u64> {
        <Self as MetashrewRpcProvider>::get_metashrew_height(self).await
    }

    async fn get_block_hash(&self, height: u64) -> Result<String> {
        <Self as BitcoinRpcProvider>::get_block_hash(self, height).await
    }

    async fn get_state_root(&self, _height: JsonValue) -> Result<String> {
        // Placeholder implementation.
        // In a real scenario, this would call a specific RPC method like `getstateroot`.
        // Err(DeezelError::NotImplemented("get_state_root is not implemented for ConcreteProvider".to_string()))
        <Self as MetashrewRpcProvider>::get_state_root(self, _height as serde_json::Value).await
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl UtxoProvider for ConcreteProvider {
    async fn get_utxos_by_spec(&self, spec: &[String]) -> Result<Vec<Utxo>> {
        let utxos = self.get_utxos(false, Some(spec.to_vec())).await?;
        let result = utxos
            .into_iter()
            .map(|(_outpoint, utxo_info)| Utxo {
                txid: utxo_info.txid,
                vout: utxo_info.vout,
                amount: utxo_info.amount,
                address: utxo_info.address,
            })
            .collect();
        Ok(result)
    }
}

// Implement KeystoreProvider trait for ConcreteProvider
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl KeystoreProvider for ConcreteProvider {
    async fn get_address(&self, address_type: &str, index: u32) -> Result<String> {
        <Self as AddressResolver>::get_address(self, address_type, index).await
    }
    async fn derive_addresses(&self, _master_public_key: &str, _network_params: &NetworkParams, _script_types: &[&str], _start_index: u32, _count: u32) -> Result<Vec<KeystoreAddress>> {
        Err(DeezelError::NotImplemented("KeystoreProvider derive_addresses not yet implemented".to_string()))
    }

    async fn get_default_addresses(&self, _master_public_key: &str, _network_params: &NetworkParams) -> Result<Vec<KeystoreAddress>> {
        Err(DeezelError::NotImplemented("KeystoreProvider get_default_addresses not yet implemented".to_string()))
    }

    fn parse_address_range(&self, _range_spec: &str) -> Result<(String, u32, u32)> {
        Err(DeezelError::NotImplemented("KeystoreProvider parse_address_range not yet implemented".to_string()))
    }

    async fn get_keystore_info(&self, _master_fingerprint: &str, _created_at: u64, _version: &str) -> Result<KeystoreInfo> {
        Err(DeezelError::NotImplemented("KeystoreProvider get_keystore_info not yet implemented".to_string()))
    }

    async fn derive_address_from_path(
        &self,
        master_public_key: &str,
        path: &DerivationPath,
        script_type: &str,
        network_params: &NetworkParams,
    ) -> Result<KeystoreAddress> {
        let address = crate::keystore::derive_address_from_public_key(
            master_public_key,
            path,
            network_params,
            script_type,
        )?;

        Ok(KeystoreAddress {
            address: address.to_string(),
            derivation_path: path.to_string(),
            index: path.into_iter().last().map(|child| match *child {
                bitcoin::bip32::ChildNumber::Normal { index } => index,
                bitcoin::bip32::ChildNumber::Hardened { index } => index,
            }).unwrap_or(0),
            script_type: script_type.to_string(),
            network: Some(network_params.bech32_prefix.clone()),
        })
    }
}

// Implement MonitorProvider trait for ConcreteProvider
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl MonitorProvider for ConcreteProvider {
    async fn monitor_blocks(&self, _start: Option<u64>) -> Result<()> {
        Err(DeezelError::NotImplemented("MonitorProvider monitor_blocks not yet implemented".to_string()))
    }

    async fn get_block_events(&self, _height: u64) -> Result<Vec<BlockEvent>> {
        Err(DeezelError::NotImplemented("MonitorProvider get_block_events not yet implemented".to_string()))
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl BitcoinRpcProvider for ConcreteProvider {
    async fn get_block_count(&self) -> Result<u64> {
        unimplemented!()
    }
    async fn generate_to_address(&self, _nblocks: u32, _address: &str) -> Result<JsonValue> {
        unimplemented!()
    }
    async fn get_blockchain_info(&self) -> Result<JsonValue> {
        unimplemented!()
    }
    async fn get_new_address(&self) -> Result<JsonValue> {
        unimplemented!()
    }
    async fn get_transaction_hex(&self, _txid: &str) -> Result<String> {
        unimplemented!()
    }
    async fn get_block(&self, _hash: &str, _raw: bool) -> Result<JsonValue> {
        unimplemented!()
    }
    async fn get_block_hash(&self, _height: u64) -> Result<String> {
        unimplemented!()
    }
    async fn send_raw_transaction(&self, _tx_hex: &str) -> Result<String> {
        unimplemented!()
    }
    async fn get_mempool_info(&self) -> Result<JsonValue> {
        unimplemented!()
    }
    async fn estimate_smart_fee(&self, _target: u32) -> Result<JsonValue> {
        unimplemented!()
    }
    async fn get_esplora_blocks_tip_height(&self) -> Result<u64> {
        unimplemented!()
    }
    async fn trace_transaction(&self, _txid: &str, _vout: u32, _block: Option<&str>, _tx: Option<&str>) -> Result<serde_json::Value> {
        unimplemented!()
    }
    async fn get_network_info(&self) -> Result<JsonValue> {
        unimplemented!()
    }
    async fn get_raw_transaction(&self, _txid: &str, _block_hash: Option<&str>) -> Result<JsonValue> {
        unimplemented!()
    }
    async fn get_block_header(&self, _hash: &str) -> Result<JsonValue> {
        unimplemented!()
    }
    async fn get_block_stats(&self, _hash: &str) -> Result<JsonValue> {
        unimplemented!()
    }
    async fn get_chain_tips(&self) -> Result<JsonValue> {
        unimplemented!()
    }
    async fn get_raw_mempool(&self) -> Result<JsonValue> {
        unimplemented!()
    }
    async fn get_tx_out(&self, _txid: &str, _vout: u32, _include_mempool: bool) -> Result<JsonValue> {
        unimplemented!()
    }
}

#[cfg(all(test, feature = "native-deps"))]
mod esplora_provider_tests {
    use super::*;
    use crate::commands::Commands;
    use std::str::FromStr;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};
    use serde_json::json;

    async fn setup() -> (MockServer, ConcreteProvider) {
        let server = MockServer::start().await;
        let rpc_config = RpcConfig {
            bitcoin_rpc_url: Some(server.uri()),
            metashrew_rpc_url: Some(server.uri()),
            sandshrew_rpc_url: Some(server.uri()),
            esplora_url: Some(server.uri()),
            network: crate::network::DeezelNetwork::from_str("regtest").unwrap(),
            ord_url: None,
            timeout_seconds: 600,
        };
        let command = Commands::Esplora { command: crate::commands::EsploraCommands::BlocksTipHash { raw: false } };
        let provider = ConcreteProvider::new_for_test(rpc_config, command);
        (server, provider)
    }

    #[tokio::test]
    async fn test_get_blocks_tip_hash() {
        // Arrange
        let (server, provider) = setup().await;
        let mock_hash = "0000000000000000000abcde".to_string();
        
        Mock::given(method("GET"))
            .and(path("/blocks/tip/hash"))
            .respond_with(ResponseTemplate::new(200).set_body_string(mock_hash.clone()))
            .mount(&server)
            .await;

        // Act
        let result = provider.get_blocks_tip_hash().await;

        // Assert
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), mock_hash);
    }

    #[tokio::test]
    async fn test_get_blocks_tip_height() {
        // Arrange
        let (server, provider) = setup().await;
        let mock_height = 800000;

        Mock::given(method("GET"))
            .and(path("/blocks/tip/height"))
            .respond_with(ResponseTemplate::new(200).set_body_string(mock_height.to_string()))
            .mount(&server)
            .await;

        // Act
        let result = provider.get_blocks_tip_height().await;

        // Assert
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), mock_height);
    }

    #[tokio::test]
    async fn test_get_block_by_height() {
        // Arrange
        let (server, provider) = setup().await;
        let mock_height = 800000;
        let mock_hash = "0000000000000000000abcde".to_string();

        Mock::given(method("GET"))
            .and(path(format!("/block-height/{mock_height}")))
            .respond_with(ResponseTemplate::new(200).set_body_string(mock_hash.clone()))
            .mount(&server)
            .await;

        // Act
        let result = provider.get_block_by_height(mock_height).await;

        // Assert
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), mock_hash);
    }

    #[tokio::test]
    async fn test_get_block() {
        // Arrange
        let (server, provider) = setup().await;
        let mock_hash = "00000000000000000001c7b8332e01ab8802397082a1f29f2e7e07e4f8a2a4b7";
        let mock_block = json!({
            "id": mock_hash,
            "height": 700000,
            "version": 536870912,
            "timestamp": 1629886679,
            "tx_count": 2500,
            "size": 1369315,
            "weight": 3992260,
            "merkle_root": "f35e359ac01426b654b33389d739dfe4288634029348a84a169e210d862289c9",
            "previousblockhash": "00000000000000000003a3b2b3b4b5b6b7b8b9bacbdcedfefe010203",
            "nonce": 1234567890,
            "bits": 402793003,
            "difficulty": 17899999999999.99
        });

        Mock::given(method("GET"))
            .and(path(format!("/block/{mock_hash}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(mock_block.clone()))
            .mount(&server)
            .await;

        // Act
        let result = EsploraProvider::get_block(&provider, mock_hash).await;

        // Assert
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), mock_block);
    }

    #[tokio::test]
    async fn test_get_block_status() {
        // Arrange
        let (server, provider) = setup().await;
        let mock_hash = "00000000000000000001c7b8332e01ab8802397082a1f29f2e7e07e4f8a2a4b7";
        let mock_status = json!({
            "in_best_chain": true,
            "height": 700000,
            "next_best": "00000000000000000002a3b2b3b4b5b6b7b8b9bacbdcedfefe010203"
        });

        Mock::given(method("GET"))
            .and(path(format!("/block/{mock_hash}/status")))
            .respond_with(ResponseTemplate::new(200).set_body_json(mock_status.clone()))
            .mount(&server)
            .await;

        // Act
        let result = provider.get_block_status(mock_hash).await;

        // Assert
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), mock_status);
    }

    #[tokio::test]
    async fn test_get_block_txids() {
        // Arrange
        let (server, provider) = setup().await;
        let mock_hash = "00000000000000000001c7b8332e01ab8802397082a1f29f2e7e07e4f8a2a4b7";
        let mock_txids = json!([
            "a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2",
            "f6e5d4c3b2a1f6e5d4c3b2a1f6e5d4c3b2a1f6e5d4c3b2a1f6e5d4c3b2a1f6e5"
        ]);

        Mock::given(method("GET"))
            .and(path(format!("/block/{mock_hash}/txids")))
            .respond_with(ResponseTemplate::new(200).set_body_json(mock_txids.clone()))
            .mount(&server)
            .await;

        // Act
        let result = provider.get_block_txids(mock_hash).await;

        // Assert
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), mock_txids);
    }

    #[tokio::test]
    async fn test_get_block_txid() {
        // Arrange
        let (server, provider) = setup().await;
        let mock_hash = "00000000000000000001c7b8332e01ab8802397082a1f29f2e7e07e4f8a2a4b7";
        let mock_index = 5;
        let mock_txid = "a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2";

        Mock::given(method("GET"))
            .and(path(format!("/block/{mock_hash}/txid/{mock_index}")))
            .respond_with(ResponseTemplate::new(200).set_body_string(mock_txid))
            .mount(&server)
            .await;

        // Act
        let result = provider.get_block_txid(mock_hash, mock_index).await;

        // Assert
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), mock_txid);
    }

    #[tokio::test]
    async fn test_get_tx() {
        // Arrange
        let (server, provider) = setup().await;
        let mock_txid = "a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2";
        let mock_tx = json!({
            "txid": mock_txid,
            "version": 2,
            "locktime": 0,
            "vin": [],
            "vout": [],
            "size": 100,
            "weight": 400,
            "fee": 1000,
            "status": {
                "confirmed": true,
                "block_height": 700000,
                "block_hash": "00000000000000000001c7b8332e01ab8802397082a1f29f2e7e07e4f8a2a4b7",
                "block_time": 1629886679
            }
        });

        Mock::given(method("GET"))
            .and(path(format!("/tx/{mock_txid}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(mock_tx.clone()))
            .mount(&server)
            .await;

        // Act
        let result = provider.get_tx(mock_txid).await;

        // Assert
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), mock_tx);
    }

    #[tokio::test]
    async fn test_get_tx_status() {
        // Arrange
        let (server, provider) = setup().await;
        let mock_txid = "a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2";
        let mock_status = json!({
            "confirmed": true,
            "block_height": 700000,
            "block_hash": "00000000000000000001c7b8332e01ab8802397082a1f29f2e7e07e4f8a2a4b7",
            "block_time": 1629886679
        });

        Mock::given(method("GET"))
            .and(path(format!("/tx/{mock_txid}/status")))
            .respond_with(ResponseTemplate::new(200).set_body_json(mock_status.clone()))
            .mount(&server)
            .await;

        // Act
        let result = provider.get_tx_status(mock_txid).await;

        // Assert
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), mock_status);
    }

    #[tokio::test]
    async fn test_get_tx_hex() {
        // Arrange
        let (server, provider) = setup().await;
        let mock_txid = "a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2";
        let mock_hex = "02000000000101...";

        Mock::given(method("GET"))
            .and(path(format!("/tx/{mock_txid}/hex")))
            .respond_with(ResponseTemplate::new(200).set_body_string(mock_hex))
            .mount(&server)
            .await;

        // Act
        let result = provider.get_tx_hex(mock_txid).await;

        // Assert
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), mock_hex);
    }

    #[tokio::test]
    async fn test_get_tx_raw() {
        // Arrange
        let (server, provider) = setup().await;
        let mock_txid = "a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2";
        let mock_raw = vec![0x02, 0x00, 0x00, 0x00, 0x00, 0x01, 0x01];

        Mock::given(method("GET"))
            .and(path(format!("/tx/{mock_txid}/raw")))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(mock_raw.clone()))
            .mount(&server)
            .await;

        // Act
        let result = provider.get_tx_raw(mock_txid).await;

        // Assert
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), hex::encode(mock_raw));
    }

    #[tokio::test]
    async fn test_get_tx_merkle_proof() {
        // Arrange
        let (server, provider) = setup().await;
        let mock_txid = "a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2";
        let mock_proof = json!({
            "block_height": 700000,
            "merkle": [
                "a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2"
            ],
            "pos": 123
        });

        Mock::given(method("GET"))
            .and(path(format!("/tx/{mock_txid}/merkle-proof")))
            .respond_with(ResponseTemplate::new(200).set_body_json(mock_proof.clone()))
            .mount(&server)
            .await;

        // Act
        let result = provider.get_tx_merkle_proof(mock_txid).await;

        // Assert
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), mock_proof);
    }

    #[tokio::test]
    async fn test_get_tx_outspend() {
        // Arrange
        let (server, provider) = setup().await;
        let mock_txid = "a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2";
        let mock_index = 0;
        let mock_outspend = json!({
            "spent": true,
            "txid": "f6e5d4c3b2a1f6e5d4c3b2a1f6e5d4c3b2a1f6e5d4c3b2a1f6e5d4c3b2a1f6e5",
            "vin": 0,
            "status": {
                "confirmed": true,
                "block_height": 700001,
                "block_hash": "00000000000000000002a3b2b3b4b5b6b7b8b9bacbdcedfefe010203",
                "block_time": 1629886779
            }
        });

        Mock::given(method("GET"))
            .and(path(format!("/tx/{mock_txid}/outspend/{mock_index}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(mock_outspend.clone()))
            .mount(&server)
            .await;

        // Act
        let result = provider.get_tx_outspend(mock_txid, mock_index).await;

        // Assert
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), mock_outspend);
    }

    #[tokio::test]
    async fn test_get_tx_outspends() {
        // Arrange
        let (server, provider) = setup().await;
        let mock_txid = "a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2";
        let mock_outspends = json!([
            {
                "spent": true,
                "txid": "f6e5d4c3b2a1f6e5d4c3b2a1f6e5d4c3b2a1f6e5d4c3b2a1f6e5d4c3b2a1f6e5",
                "vin": 0,
                "status": {
                    "confirmed": true,
                    "block_height": 700001,
                    "block_hash": "00000000000000000002a3b2b3b4b5b6b7b8b9bacbdcedfefe010203",
                    "block_time": 1629886779
                }
            },
            {
                "spent": false
            }
        ]);

        Mock::given(method("GET"))
            .and(path(format!("/tx/{mock_txid}/outspends")))
            .respond_with(ResponseTemplate::new(200).set_body_json(mock_outspends.clone()))
            .mount(&server)
            .await;

        // Act
        let result = provider.get_tx_outspends(mock_txid).await;

        // Assert
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), mock_outspends);
    }

    #[tokio::test]
    async fn test_get_address() {
        // Arrange
        let (server, provider) = setup().await;
        let mock_address = "bc1q...";
        let mock_address_info = json!({
            "address": mock_address,
            "chain_stats": { "funded_txo_count": 1, "funded_txo_sum": 100000, "spent_txo_count": 0, "spent_txo_sum": 0, "tx_count": 1 },
            "mempool_stats": { "funded_txo_count": 0, "funded_txo_sum": 0, "spent_txo_count": 0, "spent_txo_sum": 0, "tx_count": 0 }
        });

        Mock::given(method("GET"))
            .and(path(format!("/address/{mock_address}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(mock_address_info.clone()))
            .mount(&server)
            .await;

        // Act
        // let result = EsploraProvider::get_address(&provider, mock_address).await;

        // Assert
        // assert!(result.is_ok());
        // assert_eq!(result.unwrap(), mock_address_info);
    }

    #[tokio::test]
    async fn test_get_address_txs() {
        // Arrange
        let (server, provider) = setup().await;
        let mock_address = "bc1q...";
        let mock_txs = json!([
            { "txid": "a1b2c3d4...", "status": { "confirmed": true } }
        ]);

        Mock::given(method("GET"))
            .and(path(format!("/address/{mock_address}/txs")))
            .respond_with(ResponseTemplate::new(200).set_body_json(mock_txs.clone()))
            .mount(&server)
            .await;

        // Act
        let result = provider.get_address_txs(mock_address).await;

        // Assert
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), mock_txs);
    }

    #[tokio::test]
    async fn test_get_address_txs_chain() {
        // Arrange
        let (server, provider) = setup().await;
        let mock_address = "bc1q...";
        let mock_last_txid = "a1b2c3d4...";
        let mock_txs = json!([
            { "txid": "e5f6g7h8...", "status": { "confirmed": true } }
        ]);

        Mock::given(method("GET"))
            .and(path(format!("/address/{mock_address}/txs/chain/{mock_last_txid}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(mock_txs.clone()))
            .mount(&server)
            .await;

        // Act
        let result = provider.get_address_txs_chain(mock_address, Some(mock_last_txid)).await;

        // Assert
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), mock_txs);
    }

    #[tokio::test]
    async fn test_get_address_txs_mempool() {
        // Arrange
        let (server, provider) = setup().await;
        let mock_address = "bc1q...";
        let mock_txs = json!([
            { "txid": "mempooltx...", "status": { "confirmed": false } }
        ]);

        Mock::given(method("GET"))
            .and(path(format!("/address/{mock_address}/txs/mempool")))
            .respond_with(ResponseTemplate::new(200).set_body_json(mock_txs.clone()))
            .mount(&server)
            .await;

        // Act
        let result = provider.get_address_txs_mempool(mock_address).await;

        // Assert
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), mock_txs);
    }


    #[tokio::test]
    async fn test_get_mempool() {
        // Arrange
        let (server, provider) = setup().await;
        let mock_mempool_info = json!({
            "count": 10,
            "vsize": 12345,
            "total_fee": 54321,
            "fee_histogram": [[1.0, 12345]]
        });

        Mock::given(method("GET"))
            .and(path("/mempool"))
            .respond_with(ResponseTemplate::new(200).set_body_json(mock_mempool_info.clone()))
            .mount(&server)
            .await;

        // Act
        let result = provider.get_mempool().await;

        // Assert
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), mock_mempool_info);
    }

    #[tokio::test]
    async fn test_get_mempool_txids() {
        // Arrange
        let (server, provider) = setup().await;
        let mock_txids = json!([
            "a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2"
        ]);

        Mock::given(method("GET"))
            .and(path("/mempool/txids"))
            .respond_with(ResponseTemplate::new(200).set_body_json(mock_txids.clone()))
            .mount(&server)
            .await;

        // Act
        let result = provider.get_mempool_txids().await;

        // Assert
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), mock_txids);
    }

    #[tokio::test]
    async fn test_get_mempool_recent() {
        // Arrange
        let (server, provider) = setup().await;
        let mock_recent = json!([
            { "txid": "a1b2c3d4...", "fee": 1000, "vsize": 200, "value": 12345 }
        ]);

        Mock::given(method("GET"))
            .and(path("/mempool/recent"))
            .respond_with(ResponseTemplate::new(200).set_body_json(mock_recent.clone()))
            .mount(&server)
            .await;

        // Act
        let result = provider.get_mempool_recent().await;

        // Assert
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), mock_recent);
    }

    #[tokio::test]
    async fn test_get_fee_estimates() {
        // Arrange
        let (server, provider) = setup().await;
        let mock_fees = json!({ "1": 10.0, "6": 5.0, "144": 1.0 });

        Mock::given(method("GET"))
            .and(path("/fee-estimates"))
            .respond_with(ResponseTemplate::new(200).set_body_json(mock_fees.clone()))
            .mount(&server)
            .await;

        // Act
        let result = provider.get_fee_estimates().await;

        // Assert
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), mock_fees);
    }

    #[tokio::test]
    async fn test_broadcast() {
        // Arrange
        let (server, provider) = setup().await;
        let mock_tx_hex = "0100000001...";
        let mock_txid = "a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2";

        Mock::given(method("POST"))
            .and(path("/tx"))
            .respond_with(ResponseTemplate::new(200).set_body_string(mock_txid))
            .mount(&server)
            .await;

        // Act
        let result = provider.broadcast(mock_tx_hex).await;

        // Assert
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), mock_txid);
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl OrdProvider for ConcreteProvider {
    async fn get_inscription(&self, inscription_id: &str) -> Result<ord::Inscription> {
        let rpc_url = self.get_ord_server_url().ok_or_else(|| DeezelError::RpcError("Ord server URL not configured".to_string()))?;
        let json = self.call(&rpc_url, crate::ord::OrdJsonRpcMethods::INSCRIPTION, crate::esplora::params::single(inscription_id), 1).await?;
        serde_json::from_value(json).map_err(|e| DeezelError::Serialization(e.to_string()))
    }

    async fn get_inscriptions_in_block(&self, block_hash: &str) -> Result<ord::Inscriptions> {
        let rpc_url = self.get_ord_server_url().ok_or_else(|| DeezelError::RpcError("Ord server URL not configured".to_string()))?;
        let json = self.call(&rpc_url, crate::ord::OrdJsonRpcMethods::INSCRIPTIONS_IN_BLOCK, crate::esplora::params::single(block_hash), 1).await?;
        serde_json::from_value(json).map_err(|e| DeezelError::Serialization(e.to_string()))
    }

   async fn get_ord_address_info(&self, address: &str) -> Result<ord::AddressInfo> {
        let rpc_url = self.get_ord_server_url().ok_or_else(|| DeezelError::RpcError("Ord server URL not configured".to_string()))?;
        let json = self.call(&rpc_url, crate::ord::OrdJsonRpcMethods::ADDRESS, crate::esplora::params::single(address), 1).await?;
        serde_json::from_value(json).map_err(|e| DeezelError::Serialization(e.to_string()))
   }

   async fn get_block_info(&self, query: &str) -> Result<ord::Block> {
        let rpc_url = self.get_ord_server_url().ok_or_else(|| DeezelError::RpcError("Ord server URL not configured".to_string()))?;
        let json = self.call(&rpc_url, crate::ord::OrdJsonRpcMethods::BLOCK, crate::esplora::params::single(query), 1).await?;
        serde_json::from_value(json).map_err(|e| DeezelError::Serialization(e.to_string()))
   }

   async fn get_ord_block_count(&self) -> Result<u64> {
        let rpc_url = self.get_ord_server_url().ok_or_else(|| DeezelError::RpcError("Ord server URL not configured".to_string()))?;
        let json = self.call(&rpc_url, crate::ord::OrdJsonRpcMethods::BLOCK_COUNT, crate::esplora::params::empty(), 1).await?;
        log::debug!("get_ord_block_count response: {:?}", json);
        if let Some(count) = json.as_u64() {
            return Ok(count);
        }
        if let Some(count_str) = json.as_str() {
            return count_str.parse::<u64>().map_err(|_| DeezelError::RpcError("Invalid block count string response".to_string()));
        }
        Err(DeezelError::RpcError("Invalid block count response: not a u64 or string".to_string()))
   }

   async fn get_ord_blocks(&self) -> Result<ord::Blocks> {
        let rpc_url = self.get_ord_server_url().ok_or_else(|| DeezelError::RpcError("Ord server URL not configured".to_string()))?;
        let json = self.call(&rpc_url, crate::ord::OrdJsonRpcMethods::BLOCKS, crate::esplora::params::empty(), 1).await?;
        serde_json::from_value(json).map_err(|e| DeezelError::Serialization(e.to_string()))
   }

   async fn get_children(&self, inscription_id: &str, page: Option<u32>) -> Result<ord::Children> {
        let rpc_url = self.get_ord_server_url().ok_or_else(|| DeezelError::RpcError("Ord server URL not configured".to_string()))?;
        let json = self.call(&rpc_url, crate::ord::OrdJsonRpcMethods::CHILDREN, crate::esplora::params::optional_dual(inscription_id, page), 1).await?;
        serde_json::from_value(json).map_err(|e| DeezelError::Serialization(e.to_string()))
   }

   async fn get_content(&self, inscription_id: &str) -> Result<Vec<u8>> {
        let rpc_url = self.get_ord_server_url().ok_or_else(|| DeezelError::RpcError("Ord server URL not configured".to_string()))?;
        let result = self.call(&rpc_url, crate::ord::OrdJsonRpcMethods::CONTENT, crate::esplora::params::single(inscription_id), 1).await?;
        let hex_str = result.as_str().ok_or_else(|| DeezelError::RpcError("Invalid content response".to_string()))?;
        hex::decode(hex_str.strip_prefix("0x").unwrap_or(hex_str)).map_err(|e| DeezelError::Serialization(e.to_string()))
   }

   async fn get_inscriptions(&self, page: Option<u32>) -> Result<ord::Inscriptions> {
        let rpc_url = self.get_ord_server_url().ok_or_else(|| DeezelError::RpcError("Ord server URL not configured".to_string()))?;
        let json = self.call(&rpc_url, crate::ord::OrdJsonRpcMethods::INSCRIPTIONS, crate::esplora::params::optional_single(page), 1).await?;
        serde_json::from_value(json).map_err(|e| DeezelError::Serialization(e.to_string()))
   }

   async fn get_output(&self, output: &str) -> Result<ord::Output> {
        let rpc_url = self.get_ord_server_url().ok_or_else(|| DeezelError::RpcError("Ord server URL not configured".to_string()))?;
        let json = self.call(&rpc_url, crate::ord::OrdJsonRpcMethods::OUTPUT, crate::esplora::params::single(output), 1).await?;
        serde_json::from_value(json).map_err(|e| DeezelError::Serialization(e.to_string()))
   }

   async fn get_parents(&self, inscription_id: &str, page: Option<u32>) -> Result<ord::ParentInscriptions> {
        let rpc_url = self.get_ord_server_url().ok_or_else(|| DeezelError::RpcError("Ord server URL not configured".to_string()))?;
        let json = self.call(&rpc_url, crate::ord::OrdJsonRpcMethods::PARENTS, crate::esplora::params::optional_dual(inscription_id, page), 1).await?;
        serde_json::from_value(json).map_err(|e| DeezelError::Serialization(e.to_string()))
   }

   async fn get_rune(&self, rune: &str) -> Result<ord::RuneInfo> {
        let rpc_url = self.get_ord_server_url().ok_or_else(|| DeezelError::RpcError("Ord server URL not configured".to_string()))?;
        let json = self.call(&rpc_url, crate::ord::OrdJsonRpcMethods::RUNE, crate::esplora::params::single(rune), 1).await?;
        serde_json::from_value(json).map_err(|e| DeezelError::Serialization(e.to_string()))
   }

   async fn get_runes(&self, page: Option<u32>) -> Result<ord::Runes> {
        let rpc_url = self.get_ord_server_url().ok_or_else(|| DeezelError::RpcError("Ord server URL not configured".to_string()))?;
        let json = self.call(&rpc_url, crate::ord::OrdJsonRpcMethods::RUNES, crate::esplora::params::optional_single(page), 1).await?;
        serde_json::from_value(json).map_err(|e| DeezelError::Serialization(e.to_string()))
   }

   async fn get_sat(&self, sat: u64) -> Result<ord::SatResponse> {
        let rpc_url = self.get_ord_server_url().ok_or_else(|| DeezelError::RpcError("Ord server URL not configured".to_string()))?;
        let json = self.call(&rpc_url, crate::ord::OrdJsonRpcMethods::SAT, crate::esplora::params::single(sat), 1).await?;
        serde_json::from_value(json).map_err(|e| DeezelError::Serialization(e.to_string()))
   }

   async fn get_tx_info(&self, txid: &str) -> Result<ord::TxInfo> {
        let rpc_url = self.get_ord_server_url().ok_or_else(|| DeezelError::RpcError("Ord server URL not configured".to_string()))?;
        let json = self.call(&rpc_url, crate::ord::OrdJsonRpcMethods::TX, crate::esplora::params::single(txid), 1).await?;
        serde_json::from_value(json).map_err(|e| DeezelError::Serialization(e.to_string()))
   }
}
