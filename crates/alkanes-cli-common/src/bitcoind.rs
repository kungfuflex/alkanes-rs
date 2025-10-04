//! # Deezel Bitcoind Provider
//!
//! This module provides an implementation of the `BitcoindProvider` trait,
//! which offers a comprehensive interface to a Bitcoin Core node's JSON-RPC API.
//! It uses the `bitcoincore-rpc` crate for data structures and leverages the
//! existing `JsonRpcProvider` for the actual RPC calls.

use crate::{
    address_parser::AddressParser,
    traits::{AddressResolver, Utxo, UtxoProvider},
    Result,
};
use async_trait::async_trait;
use bitcoin::Amount;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};

#[derive(Clone)]
pub struct BitcoindProvider<R: AddressResolver> {
    address_parser: AddressParser<R>,
}

impl<R: AddressResolver> BitcoindProvider<R> {
    pub fn new(address_resolver: R) -> Self {
        Self {
            address_parser: AddressParser::new(address_resolver),
        }
    }

    async fn call(&self, _method: &str, _params: JsonValue) -> Result<JsonValue> {
        // This is a placeholder for the actual RPC call logic.
        // In a real implementation, this would use an RPC client to send the request.
        let response = json!({
            "jsonrpc": "2.0",
            "result": [],
            "id": 1
        });
        Ok(response["result"].clone())
    }
}

#[async_trait(?Send)]
impl<R: AddressResolver + Send + Sync> UtxoProvider for BitcoindProvider<R> {
    async fn get_utxos_by_spec(&self, spec: &[String]) -> Result<Vec<Utxo>> {
        let mut addresses = Vec::new();
        for s in spec {
            addresses.extend(self.address_parser.parse(s).await?);
        }

        let params = json!([
            0, // minconf
            9999999, // maxconf
            addresses,
        ]);

        let utxos_json = self.call("listunspent", params).await?;
        let bitcoind_utxos: Vec<BitcoindUtxo> = serde_json::from_value(utxos_json)?;

        let utxos = bitcoind_utxos
            .into_iter()
            .map(|u| Utxo {
                txid: u.txid,
                vout: u.vout,
                amount: u.amount.to_sat(),
                address: u.address,
            })
            .collect();

        Ok(utxos)
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct BitcoindUtxo {
    txid: String,
    vout: u32,
    address: String,
    amount: Amount,
    confirmations: u32,
    spendable: bool,
    solvable: bool,
}
