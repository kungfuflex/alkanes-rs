//! # Bitcoind Provider Integration Tests
//!
//! This module contains tests for the `BitcoindProvider` implementation.
//! It uses a mock `JsonRpcProvider` to simulate responses from a Bitcoin Core
//! node and verify that the `BitcoindProvider` methods correctly parse
//! the responses into the expected `bitcoincore-rpc-json` types.

use deezel_common::traits::{BitcoindProvider, JsonRpcProvider};
use deezel_common::Result;
use async_trait::async_trait;
use serde_json::{json, Value as JsonValue};
use std::sync::{Arc, Mutex};

/// A mock `JsonRpcProvider` for testing purposes.
#[derive(Clone)]
struct MockRpcProvider {
    expected_method: Arc<Mutex<String>>,
    expected_params: Arc<Mutex<JsonValue>>,
    response: Arc<Mutex<JsonValue>>,
}

impl MockRpcProvider {
    fn new(expected_method: &str, expected_params: JsonValue, response: JsonValue) -> Self {
        Self {
            expected_method: Arc::new(Mutex::new(expected_method.to_string())),
            expected_params: Arc::new(Mutex::new(expected_params)),
            response: Arc::new(Mutex::new(response)),
        }
    }
}

#[async_trait(?Send)]
impl JsonRpcProvider for MockRpcProvider {
    async fn call(
        &self,
        _url: &str,
        method: &str,
        params: JsonValue,
        _id: u64,
    ) -> Result<JsonValue> {
        let expected_method = self.expected_method.lock().unwrap();
        let expected_params = self.expected_params.lock().unwrap();
        assert_eq!(method, *expected_method);
        assert_eq!(params, *expected_params);
        let response = self.response.lock().unwrap();
        Ok(response.clone())
    }

    async fn get_bytecode(&self, _block: &str, _tx: &str) -> Result<String> {
        unimplemented!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoincore_rpc_json::{
        GetBlockResult, GetBlockchainInfoResult, GetNetworkInfoResult, GetRawTransactionResult,
    };
    use bitcoin::{consensus::deserialize, Block, BlockHash, Txid};
    use std::str::FromStr;

    #[tokio::test]
    async fn test_get_blockchain_info() {
        let expected_response = json!({
            "chain": "regtest",
            "blocks": 101,
            "headers": 101,
            "bestblockhash": "0f9188f13cb7b2c71f2a335e3a4fc328bf5beb436012afca590b1a11466e2206",
            "difficulty": 4.656542373906925e-10,
            "mediantime": 1296688602,
            "verificationprogress": 1.0,
            "initialblockdownload": false,
            "chainwork": "000000000000000000000000000000000000000000000000000000ca00ca00ca",
            "size_on_disk": 6812,
            "pruned": false,
            "softforks": {},
            "warnings": ""
        });

        let mock_provider = MockRpcProvider::new(
            "getblockchaininfo",
            json!([]),
            expected_response.clone(),
        );

        let result = mock_provider.get_blockchain_info().await.unwrap();
        let expected: GetBlockchainInfoResult = serde_json::from_value(expected_response).unwrap();

        assert_eq!(result.chain, expected.chain);
        assert_eq!(result.blocks, expected.blocks);
        assert_eq!(result.best_block_hash, expected.best_block_hash);
    }

    #[tokio::test]
    async fn test_get_network_info() {
        let expected_response = json!({
            "version": 230000,
            "subversion": "/Satoshi:0.21.0/",
            "protocolversion": 70016,
            "localservices": "0000000000000409",
            "localrelay": true,
            "timeoffset": 0,
            "connections": 0,
            "connections_in": 0,
            "connections_out": 0,
            "networkactive": true,
            "networks": [],
            "relayfee": 0.00001000,
            "incrementalfee": 0.00001000,
            "localaddresses": [],
            "warnings": ""
        });

        let mock_provider = MockRpcProvider::new(
            "getnetworkinfo",
            json!([]),
            expected_response.clone(),
        );

        let result = mock_provider.get_network_info().await.unwrap();
        let expected: GetNetworkInfoResult = serde_json::from_value(expected_response).unwrap();

        assert_eq!(result.version, expected.version);
        assert_eq!(result.subversion, expected.subversion);
    }

    #[tokio::test]
    async fn test_get_raw_transaction() {
        let txid =
            Txid::from_str("0e3e2357e806b6cdb1f70b54c3a3a17b6714ee1f0e68bebb44a74b1efd512098")
                .unwrap();
        let block_hash = BlockHash::from_str(
            "0f9188f13cb7b2c71f2a335e3a4fc328bf5beb436012afca590b1a11466e2206",
        )
        .unwrap();
        let tx_hex = "01000000010000000000000000000000000000000000000000000000000000000000000000ffffffff0704ffff001d0104ffffffff0100f2052a0100000043410496b538e853519c726a2c91e61ec11600ae1390813a627c66fb8be7947be63c52da7589379515d4e0a604f8141781e62294721166bf621e73a82cbf2342c858eeac00000000";

        let mock_provider = MockRpcProvider::new(
            "getrawtransaction",
            json!([txid, false, block_hash]),
            json!(tx_hex),
        );

        let result = mock_provider
            .get_raw_transaction(&txid, Some(&block_hash))
            .await
            .unwrap();
        let expected: bitcoin::Transaction = deserialize(&hex::decode(tx_hex).unwrap()).unwrap();

        assert_eq!(result.compute_txid(), expected.compute_txid());
    }

    #[tokio::test]
    async fn test_get_raw_transaction_info() {
        let txid =
            Txid::from_str("0e3e2357e806b6cdb1f70b54c3a3a17b6714ee1f0e68bebb44a74b1efd512098")
                .unwrap();
        let block_hash_str = "0f9188f13cb7b2c71f2a335e3a4fc328bf5beb436012afca590b1a11466e2206";
        let block_hash = BlockHash::from_str(block_hash_str).unwrap();

        let expected_response = json!({
            "in_active_chain": true,
            "hex": "01000000010000000000000000000000000000000000000000000000000000000000000000ffffffff0704ffff001d0104ffffffff0100f2052a0100000043410496b538e853519c726a2c91e61ec11600ae1390813a627c66fb8be7947be63c52da7589379515d4e0a604f8141781e62294721166bf621e73a82cbf2342c858eeac00000000",
            "txid": "0e3e2357e806b6cdb1f70b54c3a3a17b6714ee1f0e68bebb44a74b1efd512098",
            "hash": "0e3e2357e806b6cdb1f70b54c3a3a17b6714ee1f0e68bebb44a74b1efd512098",
            "size": 204,
            "vsize": 204,
            "version": 1,
            "locktime": 0,
            "vin": [
                {
                    "coinbase": "04ffff001d0104",
                    "sequence": 4294967295u32
                }
            ],
            "vout": [
                {
                    "value": 50.0,
                    "n": 0,
                    "scriptPubKey": {
                        "asm": "0496b538e853519c726a2c91e61ec11600ae1390813a627c66fb8be7947be63c52da7589379515d4e0a604f8141781e62294721166bf621e73a82cbf2342c858eeac OP_CHECKSIG",
                        "hex": "410496b538e853519c726a2c91e61ec11600ae1390813a627c66fb8be7947be63c52da7589379515d4e0a604f8141781e62294721166bf621e73a82cbf2342c858eeac",
                        "reqSigs": 1,
                        "type": "pubkey",
                        "addresses": [
                            "1EHNa6Q4Jz2uvNExL497mE43ikXhwF6kZm"
                        ]
                    }
                }
            ],
            "blockhash": block_hash_str,
            "confirmations": 101,
            "time": 1296688602,
            "blocktime": 1296688602
        });

        let mock_provider = MockRpcProvider::new(
            "getrawtransaction",
            json!([txid, true, block_hash]),
            expected_response.clone(),
        );

        let result = mock_provider
            .get_raw_transaction_info(&txid, Some(&block_hash))
            .await
            .unwrap();
        let expected: GetRawTransactionResult =
            serde_json::from_value(expected_response).unwrap();

        assert_eq!(result.txid, expected.txid);
        assert_eq!(result.hex, expected.hex);
    }

    #[tokio::test]
    async fn test_get_block_hash() {
        let height = 101;
        let block_hash_str = "0f9188f13cb7b2c71f2a335e3a4fc328bf5beb436012afca590b1a11466e2206";
        let block_hash = BlockHash::from_str(block_hash_str).unwrap();

        let mock_provider =
            MockRpcProvider::new("getblockhash", json!([height]), json!(block_hash_str));

        let result = mock_provider.get_block_hash(height).await.unwrap();
        assert_eq!(result, block_hash);
    }

    #[tokio::test]
    async fn test_get_block() {
        let block_hash_str = "0f9188f13cb7b2c71f2a335e3a4fc328bf5beb436012afca590b1a11466e2206";
        let block_hash = BlockHash::from_str(block_hash_str).unwrap();
        let block_hex = "0100000000000000000000000000000000000000000000000000000000000000000000003ba3edfd7a7b12b27ac72c3e67768f617fc81bc3888a51323a9fb8aa4b1e5e4a29ab5f49ffff001d1dac2b7c0101000000010000000000000000000000000000000000000000000000000000000000000000ffffffff0704ffff001d0104ffffffff0100f2052a0100000043410496b538e853519c726a2c91e61ec11600ae1390813a627c66fb8be7947be63c52da7589379515d4e0a604f8141781e62294721166bf621e73a82cbf2342c858eeac00000000";

        let mock_provider =
            MockRpcProvider::new("getblock", json!([block_hash, 0]), json!(block_hex));

        let result = mock_provider.get_block(&block_hash).await.unwrap();
        let expected: Block = deserialize(&hex::decode(block_hex).unwrap()).unwrap();

        assert_eq!(result.header.version, expected.header.version);
        assert_eq!(result.header.prev_blockhash, expected.header.prev_blockhash);
    }

    #[tokio::test]
    async fn test_get_block_info() {
        let block_hash_str = "0f9188f13cb7b2c71f2a335e3a4fc328bf5beb436012afca590b1a11466e2206";
        let block_hash = BlockHash::from_str(block_hash_str).unwrap();
        let expected_response = json!({
            "hash": block_hash_str,
            "confirmations": 1,
            "size": 285,
            "strippedsize": 285,
            "weight": 1140,
            "height": 101,
            "version": 1,
            "versionHex": "00000001",
            "merkleroot": "0e3e2357e806b6cdb1f70b54c3a3a17b6714ee1f0e68bebb44a74b1efd512098",
            "tx": [
                "0e3e2357e806b6cdb1f70b54c3a3a17b6714ee1f0e68bebb44a74b1efd512098"
            ],
            "time": 1296688602,
            "mediantime": 1296688602,
            "nonce": 2,
            "bits": "207fffff",
            "difficulty": 4.656542373906925e-10,
            "chainwork": "000000000000000000000000000000000000000000000000000000ca00ca00ca",
            "nTx": 1,
            "previousblockhash": "0000000000000000000000000000000000000000000000000000000000000000"
        });

        let mock_provider = MockRpcProvider::new(
            "getblock",
            json!([block_hash, 1]),
            expected_response.clone(),
        );

        let result = mock_provider.get_block_info(&block_hash).await.unwrap();
        let expected: GetBlockResult = serde_json::from_value(expected_response).unwrap();

        assert_eq!(result.hash, expected.hash);
        assert_eq!(result.height, expected.height);
    }
}