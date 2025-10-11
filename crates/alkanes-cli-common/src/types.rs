use serde::{Deserialize, Serialize};
use bitcoin::{Network, OutPoint, psbt::Psbt, secp256k1::Keypair, XOnlyPublicKey, Address, Transaction};
use crate::network::NetworkParams;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WalletConfig {
    pub wallet_path: String,
    pub bitcoin_rpc_url: String,
    pub metashrew_rpc_url: String,
    pub network: Network,
    pub network_params: Option<NetworkParams>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletInfo {
    pub address: String,
    pub balance: WalletBalance,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletBalance {
    pub confirmed: u64,
    pub pending: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddressInfo {
    pub address: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendParams {
    pub address: String,
    pub amount: u64,
    pub fee_rate: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UtxoInfo {
    pub outpoint: OutPoint,
    pub amount: u64,
    pub address: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionInfo {
    pub txid: String,
    pub timestamp: u64,
    pub fee: u64,
    pub inputs: Vec<TransactionInput>,
    pub outputs: Vec<TransactionOutput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeEstimate {
    pub feerate: f64,
    pub blocks: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeRates {
    pub fastest_fee: u64,
    pub half_hour_fee: u64,
    pub hour_fee: u64,
    pub economy_fee: u64,
    pub minimum_fee: u64,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetBalance {
    pub name: String,
    pub symbol: String,
    pub balance: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionInput {
    pub address: String,
    pub amount: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionOutput {
    pub address: String,
    pub amount: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockEvent {
    pub block_height: u64,
    pub block_hash: String,
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