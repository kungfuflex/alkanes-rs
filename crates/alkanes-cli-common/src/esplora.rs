//! Esplora API response structures and JSON-RPC implementation
//!
//! This module provides comprehensive response structures for all Esplora API endpoints
//! and implements them as JSON-RPC calls following the alkanes pattern.
//! 
//! The JSON-RPC method mapping follows this pattern:
//! - REST: `/address/{address}/utxo` → JSON-RPC: `esplora_address::utxo` params: [address]
//! - REST: `/blocks/tip/hash` → JSON-RPC: `esplora_blocks:tip:hash` params: []
//! - REST: `/block/{hash}` → JSON-RPC: `esplora_block` params: [hash]
//! - REST: `/tx/{txid}/outspend/{index}` → JSON-RPC: `esplora_tx::outspend` params: [txid, index]

use serde::{Deserialize, Serialize};
use deezel_pretty_print_macro::PrettyPrint;
use alloc::string::String;
use alloc::vec::Vec;
use alloc::collections::BTreeMap as HashMap;
use serde_json::json;

use crate::{
    address_parser::AddressParser,
    traits::{AddressResolver, Utxo, UtxoProvider},
    Result,
};
use async_trait::async_trait;
use serde_json::Value as JsonValue;

#[derive(Clone)]
pub struct EsploraProvider<R: AddressResolver> {
    address_parser: AddressParser<R>,
}

impl<R: AddressResolver> EsploraProvider<R> {
    pub fn new(address_resolver: R) -> Self {
        Self {
            address_parser: AddressParser::new(address_resolver),
        }
    }

    async fn get(&self, _path: &str) -> Result<JsonValue> {
        // This is a placeholder for the actual HTTP GET logic.
        // In a real implementation, this would use an HTTP client to send the request.
        Ok(json!([]))
    }
}

#[async_trait(?Send)]
impl<R: AddressResolver + Send + Sync> UtxoProvider for EsploraProvider<R> {
    async fn get_utxos_by_spec(&self, spec: &[String]) -> Result<Vec<Utxo>> {
        let mut addresses = Vec::new();
        for s in spec {
            addresses.extend(self.address_parser.parse(s).await?);
        }

        let mut utxos = Vec::new();
        for address in addresses {
            let path = format!("/address/{}/utxo", address);
            let esplora_utxos_json = self.get(&path).await?;
            let esplora_utxos: Vec<EsploraUtxo> = serde_json::from_value(esplora_utxos_json)?;
            for u in esplora_utxos {
                utxos.push(Utxo {
                    txid: u.txid,
                    vout: u.vout,
                    amount: u.value,
                    address: address.clone(),
                });
            }
        }

        Ok(utxos)
    }
}


/// Block information response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EsploraBlock {
    pub id: String,
    pub height: u32,
    pub version: u32,
    pub timestamp: u32,
    pub tx_count: u32,
    pub size: u32,
    pub weight: u64,
    pub merkle_root: String,
    pub previousblockhash: Option<String>,
    pub mediantime: u32,
    pub nonce: u32,
    pub bits: u32,
    pub difficulty: f64,
}

/// Block status information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EsploraBlockStatus {
    pub in_best_chain: bool,
    pub height: Option<u32>,
    pub next_best: Option<String>,
}

/// Transaction input information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EsploraTxIn {
    pub txid: String,
    pub vout: u32,
    pub prevout: Option<EsploraTxOut>,
    pub scriptsig: String,
    pub scriptsig_asm: String,
    pub witness: Option<Vec<String>>,
    pub is_coinbase: bool,
    pub sequence: u32,
    pub inner_redeemscript_asm: Option<String>,
    pub inner_witnessscript_asm: Option<String>,
}

/// Transaction output information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EsploraTxOut {
    pub scriptpubkey: String,
    pub scriptpubkey_asm: String,
    pub scriptpubkey_type: String,
    pub scriptpubkey_address: Option<String>,
    pub value: u64,
}

/// Complete transaction information
#[derive(Debug, Clone, Serialize, Deserialize, PrettyPrint)]
pub struct EsploraTransaction {
    pub txid: String,
    pub version: u32,
    pub locktime: u32,
    pub vin: Vec<EsploraTxIn>,
    pub vout: Vec<EsploraTxOut>,
    pub size: u32,
    pub weight: u64,
    pub fee: u64,
    pub status: Option<EsploraTransactionStatus>,
}

/// Transaction status information
#[derive(Debug, Clone, Serialize, Deserialize, PrettyPrint)]
pub struct EsploraTransactionStatus {
    pub confirmed: bool,
    pub block_height: Option<u32>,
    pub block_hash: Option<String>,
    pub block_time: Option<u64>,
}

/// UTXO information
#[derive(Debug, Clone, Serialize, Deserialize, PrettyPrint)]
pub struct EsploraUtxo {
    pub txid: String,
    pub vout: u32,
    pub status: EsploraTransactionStatus,
    pub value: u64,
}

/// Address information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EsploraAddress {
    pub address: String,
    pub chain_stats: EsploraAddressStats,
    pub mempool_stats: EsploraAddressStats,
}

/// Address statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EsploraAddressStats {
    pub funded_txo_count: u32,
    pub funded_txo_sum: u64,
    pub spent_txo_count: u32,
    pub spent_txo_sum: u64,
    pub tx_count: u32,
}

/// Spending information for transaction output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EsploraSpending {
    pub spent: bool,
    pub txid: Option<String>,
    pub vin: Option<u32>,
    pub status: Option<EsploraTransactionStatus>,
}

/// Merkle proof information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EsploraMerkleProof {
    pub block_height: u32,
    pub merkle: Vec<String>,
    pub pos: u32,
}

/// Mempool information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EsploraMempool {
    pub count: u32,
    pub vsize: u64,
    pub total_fee: u64,
    pub fee_histogram: Vec<(f64, u64)>,
}

/// Fee estimates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EsploraFeeEstimates {
    #[serde(flatten)]
    pub estimates: HashMap<String, f64>,
}

/// JSON-RPC method names for Esplora endpoints
pub struct EsploraJsonRpcMethods;

impl EsploraJsonRpcMethods {
    // Block endpoints
    pub const BLOCKS_TIP_HASH: &'static str = "esplora_blocks:tip:hash";
    pub const BLOCKS_TIP_HEIGHT: &'static str = "esplora_blocks:tip:height";
    pub const BLOCKS: &'static str = "esplora_blocks";
    pub const BLOCK_HEIGHT: &'static str = "esplora_block:height";
    pub const BLOCK: &'static str = "esplora_block";
    pub const BLOCK_STATUS: &'static str = "esplora_block:status";
    pub const BLOCK_TXIDS: &'static str = "esplora_block:txids";
    pub const BLOCK_HEADER: &'static str = "esplora_block:header";
    pub const BLOCK_RAW: &'static str = "esplora_block:raw";
    pub const BLOCK_TXID: &'static str = "esplora_block:txid";
    pub const BLOCK_TXS: &'static str = "esplora_block:txs";

    // Address endpoints
    pub const ADDRESS: &'static str = "esplora_address";
    pub const ADDRESS_TXS: &'static str = "esplora_address::txs";
    pub const ADDRESS_TXS_CHAIN: &'static str = "esplora_address::txs:chain";
    pub const ADDRESS_TXS_MEMPOOL: &'static str = "esplora_address::txs:mempool";
    pub const ADDRESS_UTXO: &'static str = "esplora_address::utxo";
    pub const ADDRESS_PREFIX: &'static str = "esplora_address-prefix";

    // Transaction endpoints
    pub const TX: &'static str = "esplora_tx";
    pub const TX_HEX: &'static str = "esplora_tx::hex";
    pub const TX_RAW: &'static str = "esplora_tx::raw";
    pub const TX_STATUS: &'static str = "esplora_tx::status";
    pub const TX_MERKLE_PROOF: &'static str = "esplora_tx::merkle-proof";
    pub const TX_MERKLEBLOCK_PROOF: &'static str = "esplora_tx::merkleblock-proof";
    pub const TX_OUTSPEND: &'static str = "esplora_tx::outspend";
    pub const TX_OUTSPENDS: &'static str = "esplora_tx::outspends";

    // Mempool endpoints
    pub const MEMPOOL: &'static str = "esplora_mempool";
    pub const MEMPOOL_TXIDS: &'static str = "esplora_mempool:txids";
    pub const MEMPOOL_RECENT: &'static str = "esplora_mempool:recent";

    // Other endpoints
    pub const FEE_ESTIMATES: &'static str = "esplora_fee-estimates";
    pub const BROADCAST: &'static str = "esplora_broadcast";
}

/// Helper functions for parameter formatting
pub mod params {
    use serde_json::{json, Value};

    /// Create parameters for single value endpoints
    pub fn single<T: serde::Serialize>(value: T) -> Value {
        json!([value])
    }

    /// Create parameters for dual value endpoints
    pub fn dual<T: serde::Serialize, U: serde::Serialize>(first: T, second: U) -> Value {
        json!([first, second])
    }

    /// Create parameters for optional single value endpoints
    pub fn optional_single<T: serde::Serialize>(value: Option<T>) -> Value {
        match value {
            Some(v) => json!([v]),
            None => json!([]),
        }
    }

    /// Create parameters for an optional second value
    pub fn optional_dual<T: serde::Serialize, U: serde::Serialize>(first: T, second: Option<U>) -> Value {
        match second {
            Some(s) => json!([first, s]),
            None => json!([first]),
        }
    }

    /// Create empty parameters
    pub fn empty() -> Value {
        json!([])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_method_names() {
        assert_eq!(EsploraJsonRpcMethods::BLOCKS_TIP_HASH, "esplora_blocks:tip:hash");
        assert_eq!(EsploraJsonRpcMethods::ADDRESS_UTXO, "esplora_address::utxo");
        assert_eq!(EsploraJsonRpcMethods::TX_OUTSPEND, "esplora_tx::outspend");
    }

    #[test]
    fn test_params_helpers() {
        assert_eq!(params::single("test"), json!(["test"]));
        assert_eq!(params::dual("first", "second"), json!(["first", "second"]));
        assert_eq!(params::empty(), json!([]));
        assert_eq!(params::optional_single(Some("value")), json!(["value"]));
        assert_eq!(params::optional_single::<String>(None), json!([]));
        assert_eq!(params::optional_dual("a", Some(1)), json!(["a", 1]));
        assert_eq!(params::optional_dual("a", None::<i32>), json!(["a"]));
    }

    #[test]
    fn test_block_deserialization() {
        let block_json = json!({
            "id": "000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f",
            "height": 0,
            "version": 1,
            "timestamp": 1231006505,
            "tx_count": 1,
            "size": 285,
            "weight": 1140,
            "merkle_root": "4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b",
            "previousblockhash": null,
            "mediantime": 1231006505,
            "nonce": 2083236893,
            "bits": 486604799,
            "difficulty": 1.0
        });

        let block: EsploraBlock = serde_json::from_value(block_json).unwrap();
        assert_eq!(block.height, 0);
        assert_eq!(block.tx_count, 1);
        assert!(block.previousblockhash.is_none());
    }

    #[test]
    fn test_transaction_deserialization() {
        let tx_json = json!({
            "txid": "f4184fc596403b9d638783cf57adfe4c75c605f6356fbc91338530e9831e9e16",
            "version": 1,
            "locktime": 0,
            "vin": [{
                "txid": "0000000000000000000000000000000000000000000000000000000000000000",
                "vout": serde_json::Number::from(4294967295u32),
                "prevout": null,
                "scriptsig": "04ffff001d0104455468652054696d65732030332f4a616e2f32303039204368616e63656c6c6f72206f6e206272696e6b206f66207365636f6e64206261696c6f757420666f722062616e6b73",
                "scriptsig_asm": "OP_PUSHBYTES_4 ffff001d OP_PUSHBYTES_1 04 OP_PUSHBYTES_69 5468652054696d65732030332f4a616e2f32303039204368616e63656c6c6f72206f6e206272696e6b206f66207365636f6e64206261696c6f757420666f722062616e6b73",
                "is_coinbase": true,
                "sequence": serde_json::Number::from(4294967295u32)
            }],
            "vout": [{
                "scriptpubkey": "04678afdb0fe5548271967f1a67130b7105cd6a828e03909a67962e0ea1f61deb649f6bc3f4cef38c4f35504e51ec112de5c384df7ba0b8d578a4c702b6bf11d5f",
                "scriptpubkey_asm": "OP_PUSHBYTES_65 04678afdb0fe5548271967f1a67130b7105cd6a828e03909a67962e0ea1f61deb649f6bc3f4cef38c4f35504e51ec112de5c384df7ba0b8d578a4c702b6bf11d5f OP_CHECKSIG",
                "scriptpubkey_type": "p2pk",
                "scriptpubkey_address": "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa",
                "value": serde_json::Number::from(5000000000u64)
            }],
            "size": 204,
            "weight": 816,
            "fee": 0,
            "status": {
                "confirmed": true,
                "block_height": 0,
                "block_hash": "000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f",
                "block_time": 1231006505
            }
        });
        let tx: EsploraTransaction = serde_json::from_value(tx_json).unwrap();
        assert_eq!(tx.vin.len(), 1);
        assert_eq!(tx.vout.len(), 1);
        assert!(tx.vin[0].is_coinbase);
        assert_eq!(tx.vout[0].value, 5000000000);
    }

    #[test]
    fn test_utxo_deserialization() {
        let utxo_json = json!({
            "txid": "f4184fc596403b9d638783cf57adfe4c75c605f6356fbc91338530e9831e9e16",
            "vout": 0,
            "status": {
                "confirmed": true,
                "block_height": 0,
                "block_hash": "000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f",
                "block_time": 1231006505
            },
            "value": serde_json::Number::from(5000000000u64)
        });
        let utxo: EsploraUtxo = serde_json::from_value(utxo_json).unwrap();
        assert_eq!(utxo.vout, 0);
        assert_eq!(utxo.value, 5000000000);
        assert!(utxo.status.confirmed);
    }

    #[test]
    fn test_fee_estimates_deserialization() {
        let fee_json = json!({
            "1": 10.0,
            "6": 5.0,
            "144": 1.0
        });
        let fees: EsploraFeeEstimates = serde_json::from_value(fee_json).unwrap();
        assert_eq!(fees.estimates.get("1"), Some(&10.0));
        assert_eq!(fees.estimates.get("6"), Some(&5.0));
        assert_eq!(fees.estimates.get("144"), Some(&1.0));
    }
}
