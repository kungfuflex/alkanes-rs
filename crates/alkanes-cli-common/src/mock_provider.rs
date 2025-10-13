use crate::traits::{
    AlkanesNetwork,
    AlkanesProvider,
    BitcoinRpcProvider,
    ChainProvider,
    JsonRpcProvider,
    KeystoreProvider,
    MetashrewProvider,
    MetashrewRpcProvider,
    MonitorProvider,
    NetworkProvider,
    OrdProvider,
    RunestoneProvider,
    StorageProvider,
    UtxoProvider,
    WalletProvider,
};
use crate::{
    alkanes::protorunes::{ProtoruneOutpointResponse, ProtoruneWalletResponse},
    network::NetworkParams,
    AlkanesError, Result,
};

use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use bitcoin::secp256k1::{Secp256k1, All, schnorr, SecretKey};
use bitcoin::key::{Keypair, PrivateKey};
use crate::alkanes::{EnhancedExecuteParams, EnhancedExecuteResult, execute::EnhancedAlkanesExecutor};
use crate::alkanes::types::{ExecutionState, ReadyToSignCommitTx, ReadyToSignRevealTx, ReadyToSignTx};
use bitcoin::{Address, Network, OutPoint, Transaction, TxOut, XOnlyPublicKey, bip32::{DerivationPath, Fingerprint}};
use core::str::FromStr;

/// Mock provider for testing
#[derive(Clone)]
pub struct MockProvider {
    pub responses: HashMap<String, JsonValue>,
    pub network: Network,
    pub utxos: Arc<Mutex<Vec<(OutPoint, TxOut)>>>,
    pub broadcasted_txs: Arc<Mutex<HashMap<String, String>>>,
    pub secp: Secp256k1<All>,
    pub secret_key: SecretKey,
    pub internal_key: XOnlyPublicKey,
}

impl Default for MockProvider {
    fn default() -> Self {
        Self::new(Network::Regtest)
    }
}

impl MockProvider {
    pub fn new(network: Network) -> Self {
        let secp = Secp256k1::new();
        let (secret_key, public_key) = secp.generate_keypair(&mut rand::thread_rng());
        let (internal_key, _) = public_key.x_only_public_key();
        Self {
            responses: HashMap::new(),
            network,
            utxos: Arc::new(Mutex::new(vec![])),
            broadcasted_txs: Arc::new(Mutex::new(HashMap::new())),
            secp,
            secret_key,
            internal_key,
        }
    }
    
    pub fn set_keypair(&mut self, secret_key: SecretKey, public_key: bitcoin::PublicKey) {
        self.secret_key = secret_key;
        self.internal_key = public_key.inner.x_only_public_key().0;
    }
}

impl JsonRpcProvider for MockProvider {
    fn call<'a>(&'a self, _url: &'a str, method: &'a str, _params: JsonValue, _id: u64) -> Pin<Box<dyn Future<Output = Result<JsonValue>> + Send + 'a>> {
        Box::pin(async move {
            self.responses.get(method)
                .cloned()
                .ok_or_else(|| AlkanesError::JsonRpc(format!("No mock response for method: {method}")))
        })
    }
    

}

impl StorageProvider for MockProvider {
    fn read<'a>(&'a self, _key: &'a str) -> Pin<Box<dyn Future<Output = Result<Vec<u8>>> + Send + 'a>> {
        Box::pin(async move {
            Ok(b"mock_data".to_vec())
        })
    }
    
    fn write<'a>(&'a self, _key: &'a str, _data: &'a [u8]) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move {
            Ok(())
        })
    }
    
    fn exists<'a>(&'a self, _key: &'a str) -> Pin<Box<dyn Future<Output = Result<bool>> + Send + 'a>> {
        Box::pin(async move {
            Ok(true)
        })
    }
    
    fn delete<'a>(&'a self, _key: &'a str) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move {
            Ok(())
        })
    }
    
    fn list_keys<'a>(&'a self, _prefix: &'a str) -> Pin<Box<dyn Future<Output = Result<Vec<String>>> + Send + 'a>> {
        Box::pin(async move {
            Ok(vec!["mock_key".to_string()])
        })
    }
    
    fn storage_type(&self) -> &'static str {
        "mock"
    }
}

impl NetworkProvider for MockProvider {
    fn get<'a>(&'a self, _url: &'a str) -> Pin<Box<dyn Future<Output = Result<Vec<u8>>> + Send + 'a>> {
        Box::pin(async move {
            Ok(b"mock_response".to_vec())
        })
    }
    
    fn post<'a>(&'a self, _url: &'a str, _body: &'a [u8], _content_type: &'a str) -> Pin<Box<dyn Future<Output = Result<Vec<u8>>> + Send + 'a>> {
        Box::pin(async move {
            Ok(b"mock_response".to_vec())
        })
    }
    
    fn is_reachable<'a>(&'a self, _url: &'a str) -> Pin<Box<dyn Future<Output = bool> + Send + 'a>> {
        Box::pin(async move {
            true
        })
    }
}

impl CryptoProvider for MockProvider {
    fn random_bytes(&self, len: usize) -> Result<Vec<u8>> {
        Ok(vec![0u8; len])
    }
    
    fn sha256(&self, _data: &[u8]) -> Result<[u8; 32]> {
        Ok([0u8; 32])
    }
    
    fn sha3_256(&self, _data: &[u8]) -> Result<[u8; 32]> {
        Ok([0u8; 32])
    }
    
    fn encrypt_aes_gcm<'a>(&'a self, data: &'a [u8], _key: &'a [u8], _nonce: &'a [u8]) -> Pin<Box<dyn Future<Output = Result<Vec<u8>>> + Send + 'a>> {
        Box::pin(async move {
            Ok(data.to_vec())
        })
    }
    
    fn decrypt_aes_gcm<'a>(&'a self, data: &'a [u8], _key: &'a [u8], _nonce: &'a [u8]) -> Pin<Box<dyn Future<Output = Result<Vec<u8>>> + Send + 'a>> {
        Box::pin(async move {
            Ok(data.to_vec())
        })
    }
    
    fn pbkdf2_derive<'a>(&'a self, _password: &'a [u8], _salt: &'a [u8], _iterations: u32, key_len: usize) -> Pin<Box<dyn Future<Output = Result<Vec<u8>>> + Send + 'a>> {
        Box::pin(async move {
            Ok(vec![0u8; key_len])
        })
    }
}

impl TimeProvider for MockProvider {
    fn now_secs(&self) -> u64 {
        1640995200 // 2022-01-01 00:00:00 UTC
    }
    
    fn now_millis(&self) -> u64 {
        1640995200000
    }
    
    fn sleep_ms<'a>(&'a self, _ms: u64) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            // No-op for mock
        })
    }
}

impl LogProvider for MockProvider {
    fn debug(&self, _message: &str) {}
    fn info(&self, _message: &str) {}
    fn warn(&self, _message: &str) {}
    fn error(&self, _message: &str) {}
}

impl WalletProvider for MockProvider {
    fn create_wallet<'a>(&'a mut self, _config: WalletConfig, _mnemonic: Option<String>, _passphrase: Option<String>) -> Pin<Box<dyn Future<Output = Result<WalletInfo>> + Send + 'a>> {
        Box::pin(async move {
            Ok(WalletInfo {
                address: "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4".to_string(),
                network: self.network,
                mnemonic: Some("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about".to_string()),
            })
        })
    }
    
    fn load_wallet<'a>(&'a mut self, _config: WalletConfig, _passphrase: Option<String>) -> Pin<Box<dyn Future<Output = Result<WalletInfo>> + Send + 'a>> {
        Box::pin(async move {
            self.create_wallet(WalletConfig {
                wallet_path: "test".to_string(),
                network: self.network,
                bitcoin_rpc_url: "http://localhost:8332".to_string(),
                metashrew_rpc_url: "http://localhost:8080".to_string(),
                network_params: None,
            }, None, None).await
        })
    }
    
    fn get_balance<'a>(&'a self, _addresses: Option<Vec<String>>) -> Pin<Box<dyn Future<Output = Result<WalletBalance>> + Send + 'a>> {
        Box::pin(async move {
            Ok(WalletBalance {
                confirmed: 100000000,
                pending: 0,
            })
        })
    }
    
    fn get_address<'a>(&'a self) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>> {
        Box::pin(async move {
            let address = Address::p2tr(&self.secp, self.internal_key, None, self.network);
            Ok(address.to_string())
        })
    }
    
    fn get_addresses<'a>(&'a self, count: u32) -> Pin<Box<dyn Future<Output = Result<Vec<AddressInfo>>> + Send + 'a>> {
        Box::pin(async move {
            let mut addresses = Vec::new();
            for i in 0..count {
                addresses.push(AddressInfo {
                    address: format!("bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t{i}"),
                    script_type: "p2wpkh".to_string(),
                    derivation_path: format!("m/84'/0'/0'/0/{i}"),
                    index: i,
                    used: false,
                });
            }
            Ok(addresses)
        })
    }
    
    fn send<'a>(&'a mut self, _params: SendParams) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>> {
        Box::pin(async move {
            Ok("mock_txid".to_string())
        })
    }
    
    fn get_utxos<'a>(&'a self, _include_frozen: bool, addresses: Option<Vec<String>>) -> Pin<Box<dyn Future<Output = Result<Vec<(OutPoint, UtxoInfo)>>> + Send + 'a>> {
        Box::pin(async move {
            let utxos = self.utxos.lock().unwrap();
            let mut utxo_infos: Vec<(OutPoint, UtxoInfo)> = utxos.iter().map(|(outpoint, tx_out)| {
                let address = Address::from_script(&tx_out.script_pubkey, self.network)
                    .map(|addr| addr.to_string())
                    .unwrap_or_else(|_| "unknown_script".to_string()); // Handle unrecognized scripts

                let info = UtxoInfo {
                    txid: outpoint.txid.to_string(),
                    vout: outpoint.vout,
                    amount: tx_out.value.to_sat(),
                    address,
                    script_pubkey: Some(tx_out.script_pubkey.clone()),
                    confirmations: 10,
                    frozen: false,
                    freeze_reason: None,
                    block_height: Some(100),
                    has_inscriptions: false,
                    has_runes: false,
                    has_alkanes: false,
                    is_coinbase: false,
                };
                (*outpoint, info)
            }).collect();

            if let Some(addresses) = addresses {
                if !addresses.is_empty() {
                    utxo_infos.retain(|(_, info)| addresses.contains(&info.address));
                }
            }

            Ok(utxo_infos)
        })
    }
    
    fn get_history<'a>(&'a self, _count: u32, _address: Option<String>) -> Pin<Box<dyn Future<Output = Result<Vec<TransactionInfo>>> + Send + 'a>> {
        Box::pin(async move {
            Ok(vec![TransactionInfo {
                txid: "mock_txid".to_string(),
                block_height: Some(800000),
                block_time: Some(1640995200),
                confirmed: true,
                fee: Some(1000),
                weight: Some(0),
                inputs: vec![],
                outputs: vec![],
                is_op_return: false,
                has_protostones: false,
                is_rbf: false,
            }])
        })
    }
    
    fn freeze_utxo<'a>(&'a self, _utxo: String, _reason: Option<String>) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move {
            Ok(())
        })
    }
    
    fn unfreeze_utxo<'a>(&'a self, _utxo: String) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move {
            Ok(())
        })
    }
    
    fn create_transaction<'a>(&'a self, _params: SendParams) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>> {
        Box::pin(async move {
            Ok("mock_tx_hex".to_string())
        })
    }
    
    fn sign_transaction<'a>(&'a mut self, _tx_hex: String) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>> {
        Box::pin(async move {
            Ok("mock_signed_tx_hex".to_string())
        })
    }
    
    fn broadcast_transaction<'a>(&'a self, tx_hex: String) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>> {
        Box::pin(async move {
            let tx_bytes = hex::decode(&tx_hex).map_err(|e| AlkanesError::Hex(e.to_string()))?;
            let tx: Transaction = bitcoin::consensus::deserialize(&tx_bytes).map_err(|e| AlkanesError::Serialization(e.to_string()))?;
            let txid = tx.compute_txid();
            
            // Add the new outputs of this transaction to the UTXO set
            let mut utxos = self.utxos.lock().unwrap();
            for (i, tx_out) in tx.output.iter().enumerate() {
                utxos.push((OutPoint::new(txid, i as u32), tx_out.clone()));
            }

            self.broadcasted_txs.lock().unwrap().insert(txid.to_string(), tx_hex);
            Ok(txid.to_string())
        })
    }
    
    fn estimate_fee<'a>(&'a self, _target: u32) -> Pin<Box<dyn Future<Output = Result<FeeEstimate>> + Send + 'a>> {
        Box::pin(async move {
            Ok(FeeEstimate {
                fee_rate: 10.0,
                target_blocks: 6,
            })
        })
    }
    
    fn get_fee_rates<'a>(&'a self) -> Pin<Box<dyn Future<Output = Result<FeeRates>> + Send + 'a>> {
        Box::pin(async move {
            Ok(FeeRates {
                fast: 20.0,
                medium: 10.0,
                slow: 5.0,
            })
        })
    }
    
    fn sync<'a>(&'a self) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move {
            Ok(())
        })
    }
    
    fn backup<'a>(&'a self) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>> {
        Box::pin(async move {
            Ok("mock_backup_data".to_string())
        })
    }
    
    fn get_mnemonic<'a>(&'a self) -> Pin<Box<dyn Future<Output = Result<Option<String>>> + Send + 'a>> {
        Box::pin(async move {
            Ok(Some("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about".to_string()))
        })
    }
    
    fn get_network(&self) -> Network {
        self.network
    }
    
    fn get_internal_key<'a>(&'a self) -> Pin<Box<dyn Future<Output = Result<(XOnlyPublicKey, (Fingerprint, DerivationPath))>> + Send + 'a>> {
        Box::pin(async move {
            let fingerprint = Fingerprint::from_str("00000000").unwrap();
            let path = DerivationPath::from_str("m/86'/1'/0'").unwrap();
            Ok((self.internal_key, (fingerprint, path)))
        })
    }
    
    fn sign_psbt<'a>(&'a mut self, psbt: &'a bitcoin::psbt::Psbt) -> Pin<Box<dyn Future<Output = Result<bitcoin::psbt::Psbt>> + Send + 'a>> {
        Box::pin(async move {
            let secp = self.secp();
            let mut psbt = psbt.clone();
            let mut keys = HashMap::new();
            let private_key = PrivateKey::new(self.secret_key, self.network);
            keys.insert(self.internal_key, private_key);
            psbt.sign(&keys, secp).map_err(|e| AlkanesError::Other(format!("{e:?}")))?;
            Ok(psbt)
        })
    }
    
    fn get_keypair<'a>(&'a self) -> Pin<Box<dyn Future<Output = Result<Keypair>> + Send + 'a>> {
        Box::pin(async move {
            Ok(Keypair::from_secret_key(&self.secp, &self.secret_key))
        })
    }

    fn set_passphrase(&mut self, _passphrase: Option<String>) {}

    fn get_last_used_address_index<'a>(&'a self) -> Pin<Box<dyn Future<Output = Result<u32>> + Send + 'a>> {
        Box::pin(async move {
            Ok(0)
        })
    }

    fn get_enriched_utxos<'a>(&'a self, _addresses: Option<Vec<String>>) -> Pin<Box<dyn Future<Output = Result<Vec<crate::provider::EnrichedUtxo>>> + Send + 'a>> {
        Box::pin(async move {
            unimplemented!("get_enriched_utxos is not implemented for MockProvider")
        })
    }

    fn get_all_balances<'a>(&'a self, _addresses: Option<Vec<String>>) -> Pin<Box<dyn Future<Output = Result<crate::provider::AllBalances>> + Send + 'a>> {
        Box::pin(async move {
            unimplemented!("get_all_balances is not implemented for MockProvider")
        })
    }
    fn get_master_public_key<'a>(&'a self) -> Pin<Box<dyn Future<Output = Result<Option<String>>> + Send + 'a>> {
        Box::pin(async move {
            Ok(None)
        })
    }
}

impl AddressResolver for MockProvider {
    fn resolve_all_identifiers<'a>(&'a self, input: &'a str) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>> {
        Box::pin(async move {
            // Replace identifiers with actual addresses
            let result = input.replace("p2tr:0", "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4");
            Ok(result)
        })
    }
    
    fn contains_identifiers(&self, input: &str) -> bool {
        input.contains("p2tr:") || input.contains("p2wpkh:")
    }
    
    fn get_address<'a>(&'a self, _address_type: &'a str, _index: u32) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>> {
        Box::pin(async move {
            Ok("bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4".to_string())
        })
    }
    
    fn list_identifiers<'a>(&'a self) -> Pin<Box<dyn Future<Output = Result<Vec<String>>> + Send + 'a>> {
        Box::pin(async move {
            Ok(vec!["p2tr:0".to_string(), "p2wpkh:0".to_string()])
        })
    }
}

impl BitcoinRpcProvider for MockProvider {
    fn get_block_count<'a>(&'a self) -> Pin<Box<dyn Future<Output = Result<u64>> + Send + 'a>> {
        Box::pin(async move {
            Ok(800000)
        })
    }
    
    fn generate_to_address<'a>(&'a self, _nblocks: u32, _address: &'a str) -> Pin<Box<dyn Future<Output = Result<JsonValue>> + Send + 'a>> {
        Box::pin(async move {
            Ok(serde_json::json!(["mock_block_hash"]))
        })
    }
    
    fn get_new_address<'a>(&'a self) -> Pin<Box<dyn Future<Output = Result<JsonValue>> + Send + 'a>> {
        Box::pin(async move {
            Ok(serde_json::json!("bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4"))
        })
    }
    
    fn get_transaction_hex<'a>(&'a self, txid: &'a str) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>> {
        Box::pin(async move {
            self.broadcasted_txs
                .lock()
                .unwrap()
                .get(txid)
                .cloned()
                .ok_or_else(|| AlkanesError::JsonRpc(format!("No mock tx hex for txid: {txid}")))
        })
    }
    
    fn get_block<'a>(&'a self, _hash: &'a str, _raw: bool) -> Pin<Box<dyn Future<Output = Result<JsonValue>> + Send + 'a>> {
        Box::pin(async move {
            Ok(serde_json::json!({"height": 800000}))
        })
    }
    
    async fn get_block_hash(&self, _height: u64) -> Result<String> {
        Ok("mock_block_hash".to_string())
    }
    
    async fn send_raw_transaction(&self, _tx_hex: &str) -> Result<String> {
        Ok("mock_txid".to_string())
    }

    async fn get_blockchain_info(&self) -> Result<JsonValue> {
        Ok(JsonValue::Null)
    }

    async fn get_network_info(&self) -> Result<JsonValue> {
        Ok(JsonValue::Null)
    }

    async fn get_raw_transaction(&self, _txid: &str, _block_hash: Option<&str>) -> Result<JsonValue> {
        Ok(JsonValue::Null)
    }

    async fn get_block_header(&self, _hash: &str) -> Result<JsonValue> {
        Ok(JsonValue::Null)
    }

    async fn get_block_stats(&self, _hash: &str) -> Result<JsonValue> {
        Ok(JsonValue::Null)
    }

    async fn get_chain_tips(&self) -> Result<JsonValue> {
        Ok(JsonValue::Null)
    }

    async fn get_raw_mempool(&self) -> Result<JsonValue> {
        Ok(JsonValue::Null)
    }

    async fn get_tx_out(&self, _txid: &str, _vout: u32, _include_mempool: bool) -> Result<JsonValue> {
        Ok(JsonValue::Null)
    }

    async fn get_mempool_info(&self) -> Result<JsonValue> {
        Ok(serde_json::json!({"size": 1000}))
    }
    
    async fn estimate_smart_fee(&self, _target: u32) -> Result<JsonValue> {
        Ok(serde_json::json!({"feerate": 0.00010000}))
    }
    
    async fn get_esplora_blocks_tip_height(&self) -> Result<u64> {
        Ok(800000)
    }
    
    async fn trace_transaction(&self, _txid: &str, _vout: u32, _block: Option<&str>, _tx: Option<&str>) -> Result<JsonValue> {
        Ok(serde_json::json!({"trace": "mock_trace"}))
    }
}

impl MetashrewRpcProvider for MockProvider {
    async fn get_metashrew_height(&self) -> Result<u64> {
        Ok(800001)
    }

    async fn get_state_root(&self, _height: JsonValue) -> Result<String> {
        Ok(String::new())
    }
    
    async fn get_contract_meta(&self, _block: &str, _tx: &str) -> Result<JsonValue> {
        Ok(serde_json::json!({"name": "test_contract"}))
    }
    
    async fn trace_outpoint(&self, _txid: &str, _vout: u32) -> Result<JsonValue> {
        Ok(serde_json::json!({ "events": [] }))
    }
    
    async fn get_spendables_by_address(&self, _address: &str) -> Result<JsonValue> {
        Ok(serde_json::json!([]))
    }
    
    async fn get_protorunes_by_address(&self, _address: &str, _block_tag: Option<String>, _protocol_tag: u128) -> Result<ProtoruneWalletResponse> {
        Ok(ProtoruneWalletResponse::default())
    }
    
    async fn get_protorunes_by_outpoint(&self, _txid: &str, _vout: u32, _block_tag: Option<String>, _protocol_tag: u128) -> Result<ProtoruneOutpointResponse> {
        Ok(ProtoruneOutpointResponse::default())
    }
}

impl MetashrewProvider for MockProvider {
    async fn get_height(&self) -> Result<u64> {
        Ok(800000)
    }
    async fn get_block_hash(&self, _height: u64) -> Result<String> {
        Ok("mock_block_hash".to_string())
    }
    async fn get_state_root(&self, _height: JsonValue) -> Result<String> {
        Ok("mock_state_root".to_string())
    }
}

impl EsploraProvider for MockProvider {
    async fn get_blocks_tip_hash(&self) -> Result<String> {
        Ok("mock_tip_hash".to_string())
    }
    
    async fn get_blocks_tip_height(&self) -> Result<u64> {
        Ok(800000)
    }
    
    async fn get_blocks(&self, _start_height: Option<u64>) -> Result<JsonValue> {
        Ok(serde_json::json!([]))
    }
    
    async fn get_block_by_height(&self, _height: u64) -> Result<String> {
        Ok("mock_block_hash".to_string())
    }
    
    async fn get_block(&self, _hash: &str) -> Result<JsonValue> {
        Ok(serde_json::json!({"height": 800000}))
    }
    
    async fn get_block_status(&self, _hash: &str) -> Result<JsonValue> {
        Ok(serde_json::json!({"confirmed": true}))
    }
    
    async fn get_block_txids(&self, _hash: &str) -> Result<JsonValue> {
        Ok(serde_json::json!(["mock_txid"]))
    }
    
    async fn get_block_header(&self, _hash: &str) -> Result<String> {
        Ok("mock_header".to_string())
    }
    
    async fn get_block_raw(&self, _hash: &str) -> Result<String> {
        Ok("mock_raw_block".to_string())
    }
    
    async fn get_block_txid(&self, _hash: &str, _index: u32) -> Result<String> {
        Ok("mock_txid".to_string())
    }
    
    async fn get_block_txs(&self, _hash: &str, _start_index: Option<u32>) -> Result<JsonValue> {
        Ok(serde_json::json!([]))
    }
    
    
    async fn get_address_info(&self, address: &str) -> Result<JsonValue> {
        Ok(serde_json::json!({
            "address": address,
            "chain_stats": { "funded_txo_count": 0, "funded_txo_sum": 0, "spent_txo_count": 0, "spent_txo_sum": 0, "tx_count": 0 },
            "mempool_stats": { "funded_txo_count": 0, "funded_txo_sum": 0, "spent_txo_count": 0, "spent_txo_sum": 0, "tx_count": 0 }
        }))
    }

    async fn get_address_utxo(&self, _address: &str) -> Result<JsonValue> {
        Ok(serde_json::json!([]))
    }
    
    async fn get_address_txs(&self, _address: &str) -> Result<JsonValue> {
        Ok(serde_json::json!([]))
    }
    
    async fn get_address_txs_chain(&self, _address: &str, _last_seen_txid: Option<&str>) -> Result<JsonValue> {
        Ok(serde_json::json!([]))
    }
    
    async fn get_address_txs_mempool(&self, _address: &str) -> Result<JsonValue> {
        Ok(serde_json::json!([]))
    }
    
    
    async fn get_address_prefix(&self, _prefix: &str) -> Result<JsonValue> {
        Ok(serde_json::json!([]))
    }
    
    async fn get_tx(&self, _txid: &str) -> Result<JsonValue> {
        Ok(serde_json::json!({"txid": "mock_txid"}))
    }
    
    async fn get_tx_hex(&self, _txid: &str) -> Result<String> {
        Ok("mock_tx_hex".to_string())
    }
    
    async fn get_tx_raw(&self, _txid: &str) -> Result<String> {
        Ok("mock_raw_tx".to_string())
    }
    
    async fn get_tx_status(&self, _txid: &str) -> Result<JsonValue> {
        Ok(serde_json::json!({"confirmed": true}))
    }
    
    async fn get_tx_merkle_proof(&self, _txid: &str) -> Result<JsonValue> {
        Ok(serde_json::json!({"proof": "mock_proof"}))
    }
    
    async fn get_tx_merkleblock_proof(&self, _txid: &str) -> Result<String> {
        Ok("mock_merkleblock_proof".to_string())
    }
    
    async fn get_tx_outspend(&self, _txid: &str, _index: u32) -> Result<JsonValue> {
        Ok(serde_json::json!({"spent": false}))
    }
    
    async fn get_tx_outspends(&self, _txid: &str) -> Result<JsonValue> {
        Ok(serde_json::json!([]))
    }
    
    async fn broadcast(&self, _tx_hex: &str) -> Result<String> {
        Ok("mock_txid".to_string())
    }
    
    async fn get_mempool(&self) -> Result<JsonValue> {
        Ok(serde_json::json!({"count": 1000}))
    }
    
    async fn get_mempool_txids(&self) -> Result<JsonValue> {
        Ok(serde_json::json!(["mock_txid"]))
    }
    
    async fn get_mempool_recent(&self) -> Result<JsonValue> {
        Ok(serde_json::json!([]))
    }
    
    async fn get_fee_estimates(&self) -> Result<JsonValue> {
        Ok(serde_json::json!({"1": 20.0, "6": 10.0, "144": 5.0}))
    }
}

impl RunestoneProvider for MockProvider {
    async fn decode_runestone(&self, _tx: &Transaction) -> Result<JsonValue> {
        Ok(serde_json::json!({"etching": {"rune": "BITCOIN"}}))
    }
    
    async fn format_runestone_with_decoded_messages(&self, _tx: &Transaction) -> Result<JsonValue> {
        Ok(serde_json::json!({"formatted": "mock_formatted_runestone"}))
    }
    
    async fn analyze_runestone(&self, _txid: &str) -> Result<JsonValue> {
        Ok(serde_json::json!({"analysis": "mock_analysis"}))
    }
}

impl AlkanesProvider for MockProvider {
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
        _address: &str,
        _block_tag: Option<String>,
        _protocol_tag: u128,
    ) -> Result<crate::alkanes::protorunes::ProtoruneWalletResponse> {
        Err(AlkanesError::NotImplemented(
            "protorunes_by_address".to_string(),
        ))
    }
    async fn protorunes_by_outpoint(
        &self,
        _txid: &str,
        _vout: u32,
        _block_tag: Option<String>,
        _protocol_tag: u128,
    ) -> Result<crate::alkanes::protorunes::ProtoruneOutpointResponse> {
        Err(AlkanesError::NotImplemented(
            "protorunes_by_outpoint".to_string(),
        ))
    }
    async fn simulate(&self, _contract_id: &str, _context: &crate::proto::alkanes::MessageContextParcel) -> Result<JsonValue> {
        todo!()
    }
    async fn view(&self, _contract_id: &str, _view_fn: &str, _params: Option<&[u8]>) -> Result<JsonValue> {
        todo!()
    }
    async fn trace(&self, _outpoint: &str) -> Result<crate::proto::alkanes::Trace> {
        Err(AlkanesError::NotImplemented("trace".to_string()))
    }
    async fn get_block(&self, _height: u64) -> Result<crate::proto::alkanes::BlockResponse> {
        Err(AlkanesError::NotImplemented("get_block".to_string()))
    }
    async fn sequence(&self) -> Result<JsonValue> {
        todo!()
    }
    async fn spendables_by_address(&self, _address: &str) -> Result<JsonValue> {
        todo!()
    }
    async fn trace_block(&self, _height: u64) -> Result<crate::proto::alkanes::Trace> {
        Err(AlkanesError::NotImplemented("trace_block".to_string()))
    }
    async fn get_bytecode(&self, _alkane_id: &str, _block_tag: Option<String>) -> Result<String> {
        todo!()
    }
    async fn inspect(&self, _target: &str, _config: crate::alkanes::AlkanesInspectConfig) -> Result<crate::alkanes::AlkanesInspectResult> {
        todo!()
    }
    async fn get_balance(&self, _address: Option<&str>) -> Result<Vec<crate::alkanes::AlkaneBalance>> {
        todo!()
    }
}

impl MonitorProvider for MockProvider {
    async fn monitor_blocks(&self, _start: Option<u64>) -> Result<()> {
        Ok(())
    }
    
    async fn get_block_events(&self, _height: u64) -> Result<Vec<BlockEvent>> {
        Ok(vec![BlockEvent {
            event_type: "transaction".to_string(),
            block_height: 800000,
            txid: "mock_txid".to_string(),
            data: serde_json::json!({"amount": 100000}),
        }])
    }
}

impl KeystoreProvider for MockProvider {
    async fn get_address(&self, _address_type: &str, _index: u32) -> Result<String> {
        Ok("mock_address".to_string())
    }

    async fn derive_addresses(
        &self,
        _master_public_key: &str,
        _network_params: &NetworkParams,
        _script_types: &[&str],
        _start_index: u32,
        _count: u32,
    ) -> Result<Vec<KeystoreAddress>> {
        Ok(vec![])
    }
    async fn get_default_addresses(
        &self,
        _master_public_key: &str,
        _network_params: &NetworkParams,
    ) -> Result<Vec<KeystoreAddress>> {
        Ok(vec![])
    }
    fn parse_address_range(&self, _range_spec: &str) -> Result<(String, u32, u32)> {
        unimplemented!()
    }
    async fn get_keystore_info(
        &self,
        _master_fingerprint: &str,
        _created_at: u64,
        _version: &str,
    ) -> Result<KeystoreInfo> {
        unimplemented!()
    }
    async fn derive_address_from_path(
        &self,
        _master_public_key: &str,
        _path: &DerivationPath,
        _script_type: &str,
        _network_params: &NetworkParams,
    ) -> Result<KeystoreAddress> {

        Ok(KeystoreAddress {
            address: "mock_address".to_string(),
            derivation_path: "m/0/0".to_string(),
            index: 0,
            script_type: "p2wpkh".to_string(),
            network: Some("regtest".to_string()),
        })
    }
}

use crate::ord::{
    AddressInfo as OrdAddressInfo, Block as OrdBlock, Blocks as OrdBlocks, Children as OrdChildren,
    Inscription as OrdInscription, Inscriptions as OrdInscriptions, Output as OrdOutput,
    ParentInscriptions as OrdParents, SatResponse as OrdSat, RuneInfo as OrdRuneInfo,
    Runes as OrdRunes, TxInfo as OrdTxInfo,
};

impl OrdProvider for MockProvider {
    async fn get_inscription(&self, _inscription_id: &str) -> Result<OrdInscription> {
        todo!()
    }
    async fn get_inscriptions_in_block(&self, _block_hash: &str) -> Result<OrdInscriptions> {
        todo!()
    }
    async fn get_ord_address_info(&self, _address: &str) -> Result<OrdAddressInfo> {
        todo!()
    }
    async fn get_block_info(&self, _query: &str) -> Result<OrdBlock> {
        todo!()
    }
    async fn get_ord_block_count(&self) -> Result<u64> {
        todo!()
    }
    async fn get_ord_blocks(&self) -> Result<OrdBlocks> {
        todo!()
    }
    async fn get_children(&self, _inscription_id: &str, _page: Option<u32>) -> Result<OrdChildren> {
        todo!()
    }
    async fn get_content(&self, _inscription_id: &str) -> Result<Vec<u8>> {
        todo!()
    }
    async fn get_inscriptions(&self, _page: Option<u32>) -> Result<OrdInscriptions> {
        todo!()
    }
    async fn get_output(&self, _output: &str) -> Result<OrdOutput> {
        todo!()
    }
    async fn get_parents(&self, _inscription_id: &str, _page: Option<u32>) -> Result<OrdParents> {
        todo!()
    }
    async fn get_rune(&self, _rune: &str) -> Result<OrdRuneInfo> {
        todo!()
    }
    async fn get_runes(&self, _page: Option<u32>) -> Result<OrdRunes> {
        todo!()
    }
    async fn get_sat(&self, _sat: u64) -> Result<OrdSat> {
        todo!()
    }
    async fn get_tx_info(&self, _txid: &str) -> Result<OrdTxInfo> {
        todo!()
    }
}


impl AlkanesProvider for MockProvider {
    fn provider_name(&self) -> &str {
        "mock"
    }
    
    async fn initialize(&self) -> Result<()> {
        Ok(())
    }
    
    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    fn clone_box(&self) -> Box<dyn AlkanesProvider> {
        Box::new(self.clone())
    }

    fn get_bitcoin_rpc_url(&self) -> Option<String> {
        None
    }

    fn get_esplora_api_url(&self) -> Option<String> {
        None
    }

    fn get_ord_server_url(&self) -> Option<String> {
        None
    }

    fn get_metashrew_rpc_url(&self) -> Option<String> {
        None
    }

    fn secp(&self) -> &Secp256k1<All> {
        &self.secp
    }

    async fn get_utxo(&self, outpoint: &OutPoint) -> Result<Option<TxOut>> {
        let utxos = self.utxos.lock().unwrap();
        Ok(utxos.iter().find(|(op, _)| op == outpoint).map(|(_, tx_out)| tx_out.clone()))
    }

    async fn sign_taproot_script_spend(
        &self,
        sighash: bitcoin::secp256k1::Message,
    ) -> Result<schnorr::Signature> {
        let keypair = Keypair::from_secret_key(&self.secp, &self.secret_key);
        Ok(self.secp.sign_schnorr_with_rng(&sighash, &keypair, &mut rand::thread_rng()))
    }

    async fn wrap(&mut self, _amount: u64, _address: Option<String>, _fee_rate: Option<f32>) -> Result<String> {
        unimplemented!("wrap is not implemented for MockProvider")
    }

    async fn unwrap(&mut self, _amount: u64, _address: Option<String>) -> Result<String> {
        unimplemented!("unwrap is not implemented for MockProvider")
    }
}