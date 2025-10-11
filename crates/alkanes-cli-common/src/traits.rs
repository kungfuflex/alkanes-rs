use serde::{Serialize, Deserialize};
use crate::{
    AlkanesError, Result,
    types::*,
};
use async_trait::async_trait;
use bitcoin::{OutPoint, psbt::Psbt, secp256k1::Keypair, XOnlyPublicKey, Address, Transaction};
use serde_json::Value;

#[async_trait]
pub trait AlkanesProvider: Send + Sync {
    async fn get_block_count(&self) -> Result<u64>;
}

#[async_trait]
pub trait WalletProvider: Send + Sync {
    async fn create_wallet(&mut self, config: WalletConfig, mnemonic: Option<String>, passphrase: Option<String>) -> Result<WalletInfo>;
    async fn load_wallet(&mut self, config: WalletConfig, passphrase: Option<String>) -> Result<WalletInfo>;
    async fn get_balance(&self, addresses: Option<Vec<String>>) -> Result<WalletBalance>;
    async fn get_address(&self) -> Result<String>;
    async fn get_addresses(&self, count: u32) -> Result<Vec<AddressInfo>>;
    async fn send(&mut self, params: SendParams) -> Result<String>;
    async fn get_utxos(&self, include_frozen: bool, addresses: Option<Vec<String>>) -> Result<Vec<(OutPoint, UtxoInfo)>>;
    async fn get_history(&self, count: u32, address: Option<String>) -> Result<Vec<TransactionInfo>>;
    async fn freeze_utxo(&self, utxo: String, reason: Option<String>) -> Result<()>;
    async fn unfreeze_utxo(&self, utxo: String) -> Result<()>;
    async fn create_transaction(&self, params: SendParams) -> Result<String>;
    async fn sign_transaction(&mut self, tx_hex: String) -> Result<String>;
    async fn broadcast_transaction(&self, tx_hex: String) -> Result<String>;
    async fn estimate_fee(&self, target: u32) -> Result<FeeEstimate>;
    async fn get_fee_rates(&self) -> Result<FeeRates>;
    async fn sync(&self) -> Result<()>;
    async fn backup(&self) -> Result<String>;
    async fn get_mnemonic(&self) -> Result<Option<String>>;
    fn get_network(&self) -> bitcoin::Network;
    async fn get_internal_key(&self) -> Result<(XOnlyPublicKey, bitcoin::secp256k1::Keypair)>;
    async fn sign_psbt(&mut self, psbt: &Psbt) -> Result<Psbt>;
    async fn get_keypair(&self) -> Result<Keypair>;
    async fn get_enriched_utxos(&self, addresses: Option<Vec<String>>) -> Result<Vec<EnrichedUtxo>>;
    async fn get_all_balances(&self, addresses: Option<Vec<String>>) -> Result<AllBalances>;
}

#[async_trait]
pub trait JsonRpcProvider: Send + Sync {
    async fn call(&self, url: &str, method: &str, params: Value, id: u64) -> Result<Value>;
}

#[async_trait]
pub trait BitcoinRpcProvider: Send + Sync {
    async fn send_raw_transaction(&self, tx_hex: &str) -> Result<String>;
    async fn get_block_count(&self) -> Result<u64>;
    async fn generate_to_address(&self, nblocks: u32, address: &str) -> Result<Value>;
    async fn get_blockchain_info(&self) -> Result<Value>;
    async fn get_transaction_hex(&self, txid: &str) -> Result<String>;
    async fn get_block(&self, hash: &str, raw: bool) -> Result<Value>;
    async fn get_block_hash(&self, height: u64) -> Result<String>;
    async fn get_mempool_info(&self) -> Result<Value>;
    async fn estimate_smart_fee(&self, target: u32) -> Result<Value>;
    async fn get_esplora_blocks_tip_height(&self) -> Result<u64>;
    async fn trace_transaction(&self, txid: &str, vout: u32, block: Option<&str>, tx: Option<&str>) -> Result<Value>;
    async fn get_new_address(&self) -> Result<Value>;
    async fn get_network_info(&self) -> Result<Value>;
    async fn get_raw_transaction(&self, txid: &str, block_hash: Option<&str>) -> Result<Value>;
    async fn get_block_header(&self, hash: &str) -> Result<Value>;
    async fn get_block_stats(&self, hash: &str) -> Result<Value>;
    async fn get_chain_tips(&self) -> Result<Value>;
    async fn get_raw_mempool(&self) -> Result<Value>;
    async fn get_tx_out(&self, txid: &str, vout: u32, include_mempool: bool) -> Result<Value>;
}

#[async_trait]
pub trait MetashrewRpcProvider: Send + Sync {
    async fn get_block(&self, height: u64) -> Result<Value>;
    async fn get_height(&self) -> Result<u64>;
    async fn get_state_root(&self, height: Value) -> Result<String>;
    async fn get_contract_meta(&self, block: &str, tx: &str) -> Result<Value>;
    async fn trace_outpoint(&self, txid: &str, vout: u32) -> Result<Value>;
    async fn get_spendables_by_address(&self, address: &str) -> Result<Value>;
    async fn get_protorunes_by_address(&self, address: &str, block_tag: Option<String>, protocol_tag: u128) -> Result<crate::alkanes::protorunes::ProtoruneWalletResponse>;
    async fn get_protorunes_by_outpoint(&self, txid: &str, vout: u32, block_tag: Option<String>, protocol_tag: u128) -> Result<crate::alkanes::protorunes::ProtoruneOutpointResponse>;
}

#[async_trait]
pub trait EsploraProvider: Send + Sync {
    async fn get_transactions(&self, address: &Address) -> Result<Vec<Transaction>>;
}

#[async_trait]
pub trait RunestoneProvider: Send + Sync {
    async fn get_runestones(&self) -> Result<Value>;
}

#[async_trait]
pub trait OrdProvider: Send + Sync {
    async fn get_ordinals(&self, address: &Address) -> Result<Value>;
}

#[async_trait]
pub trait MonitorProvider: Send + Sync {
    async fn get_block_events(&self, height: u64) -> Result<Vec<BlockEvent>>;
}



#[async_trait]
pub trait KeystoreProvider: Send + Sync {
    async fn get_addresses(&self, master_fingerprint: &str, start_index: u32, count: u32) -> Result<Vec<KeystoreAddress>>;
    async fn get_default_addresses(&self, master_public_key: &str, network_params: &crate::network::NetworkParams) -> Result<Vec<KeystoreAddress>>;
    async fn get_keystore_info(&self, master_fingerprint: &str, created_at: u64, version: &str) -> Result<KeystoreInfo>;
    async fn derive_address(&self, master_fingerprint: &str, path: &str, network_params: &crate::network::NetworkParams) -> Result<KeystoreAddress>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeystoreAddress {
    pub address: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeystoreInfo {
    pub master_fingerprint: String,
    pub created_at: u64,
    pub version: String,
}

#[async_trait]
pub trait UtxoProvider: Send + Sync {
    async fn get_utxos_by_spec(&self, spec: &[String]) -> Result<Vec<UtxoInfo>>;
}

#[async_trait]
pub trait AddressResolverProvider: Send + Sync {
    async fn get_address(&self, address_type: &str, index: u32) -> Result<String>;
    async fn resolve_all_identifiers(&self, input: &str) -> Result<String>;
    fn contains_identifiers(&self, input: &str) -> bool;
    async fn list_identifiers(&self) -> Result<Vec<String>>;
}