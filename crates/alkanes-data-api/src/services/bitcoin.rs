use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;

use super::alkanes::AlkanesService;
use super::alkanes_rpc::AlkanesRpcClient;
use crate::services::alkanes::FormattedUtxo;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddressBalance {
    pub balance: u64,
    #[serde(rename = "unconfirmedBalance")]
    pub unconfirmed_balance: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub txid: String,
    pub block_height: Option<i32>,
    pub timestamp: Option<i64>,
    pub fee: Option<u64>,
}

pub struct BitcoinService {
    rpc: AlkanesRpcClient,
}

impl BitcoinService {
    pub fn new(rpc: AlkanesRpcClient) -> Self {
        Self { rpc }
    }

    /// Get address balance
    pub async fn get_address_balance(&self, address: &str) -> Result<AddressBalance> {
        let utxos = self.rpc.get_address_utxos(address).await?;
        
        let mut balance = 0u64;
        let mut unconfirmed_balance = 0u64;

        if let Some(utxos_array) = utxos.as_array() {
            for utxo in utxos_array {
                if let Some(value) = utxo.get("value").and_then(|v| v.as_u64()) {
                    let status_confirmed = utxo
                        .get("status")
                        .and_then(|s| s.get("confirmed"))
                        .and_then(|c| c.as_bool())
                        .unwrap_or(false);

                    if status_confirmed {
                        balance += value;
                    } else {
                        unconfirmed_balance += value;
                    }
                }
            }
        }

        Ok(AddressBalance {
            balance,
            unconfirmed_balance,
        })
    }

    /// Get taproot balance (same as address balance for now)
    pub async fn get_taproot_balance(&self, address: &str) -> Result<AddressBalance> {
        self.get_address_balance(address).await
    }

    /// Get address UTXOs
    pub async fn get_address_utxos(
        &self,
        address: &str,
        _spend_strategy: Option<String>,
    ) -> Result<Vec<FormattedUtxo>> {
        let utxos_value = self.rpc.get_address_utxos(address).await?;
        
        let mut utxos = Vec::new();

        if let Some(utxos_array) = utxos_value.as_array() {
            for utxo in utxos_array {
                if let (Some(txid), Some(vout), Some(value)) = (
                    utxo.get("txid").and_then(|v| v.as_str()),
                    utxo.get("vout").and_then(|v| v.as_u64()),
                    utxo.get("value").and_then(|v| v.as_u64()),
                ) {
                    utxos.push(FormattedUtxo {
                        tx_id: txid.to_string(),
                        script_pk: String::new(), // TODO: Get from transaction
                        output_index: vout as u32,
                        satoshis: value,
                        address: address.to_string(),
                        indexed: true,
                        inscriptions: vec![],
                        runes: Default::default(),
                        confirmations: 1,
                        alkanes: Default::default(),
                    });
                }
            }
        }

        Ok(utxos)
    }

    /// Get AMM-spendable UTXOs (excluding alkanes, runes, inscriptions)
    pub async fn get_amm_utxos(
        &self,
        address: &str,
        alkanes_service: &AlkanesService,
    ) -> Result<Vec<FormattedUtxo>> {
        // Get all UTXOs
        let all_utxos_value = self.rpc.get_address_utxos(address).await?;
        
        // Get alkanes UTXOs
        let alkanes_utxos = alkanes_service.get_alkanes_utxos(address).await?;
        
        let alkane_outpoints: HashSet<String> = alkanes_utxos
            .iter()
            .map(|u| format!("{}:{}", u.tx_id, u.output_index))
            .collect();

        let mut spendable_utxos = Vec::new();

        if let Some(utxos_array) = all_utxos_value.as_array() {
            for utxo in utxos_array {
                let has_runes = utxo
                    .get("runes")
                    .and_then(|r| r.as_array())
                    .map(|a| !a.is_empty())
                    .unwrap_or(false);

                let has_inscriptions = utxo
                    .get("inscriptions")
                    .and_then(|i| i.as_array())
                    .map(|a| !a.is_empty())
                    .unwrap_or(false);

                if let (Some(txid), Some(vout), Some(value)) = (
                    utxo.get("txid").and_then(|v| v.as_str()),
                    utxo.get("vout").and_then(|v| v.as_u64()),
                    utxo.get("value").and_then(|v| v.as_u64()),
                ) {
                    let outpoint = format!("{}:{}", txid, vout);

                    if !has_runes && !has_inscriptions && !alkane_outpoints.contains(&outpoint) {
                        spendable_utxos.push(FormattedUtxo {
                            tx_id: txid.to_string(),
                            script_pk: String::new(),
                            output_index: vout as u32,
                            satoshis: value,
                            address: address.to_string(),
                            indexed: true,
                            inscriptions: vec![],
                            runes: Default::default(),
                            confirmations: 1,
                            alkanes: Default::default(),
                        });
                    }
                }
            }
        }

        // Add alkanes UTXOs to the list
        spendable_utxos.extend(alkanes_utxos);

        Ok(spendable_utxos)
    }

    /// Get account UTXOs (not implemented - requires account management)
    pub async fn get_account_utxos(&self, _account: &str) -> Result<Vec<FormattedUtxo>> {
        // This would require account/address mapping
        Ok(vec![])
    }

    /// Get account balance (not implemented - requires account management)
    pub async fn get_account_balance(&self, _account: &str) -> Result<AddressBalance> {
        // This would require account/address mapping
        Ok(AddressBalance {
            balance: 0,
            unconfirmed_balance: 0,
        })
    }

    /// Get taproot transaction history
    pub async fn get_taproot_history(
        &self,
        address: &str,
        total_txs: i32,
    ) -> Result<Vec<Transaction>> {
        let txs_value = self.rpc.get_address_txs(address).await?;
        
        let mut transactions = Vec::new();

        if let Some(txs_array) = txs_value.as_array() {
            for (i, tx) in txs_array.iter().enumerate() {
                if i >= total_txs as usize {
                    break;
                }

                if let Some(txid) = tx.get("txid").and_then(|v| v.as_str()) {
                    transactions.push(Transaction {
                        txid: txid.to_string(),
                        block_height: tx
                            .get("status")
                            .and_then(|s| s.get("block_height"))
                            .and_then(|h| h.as_i64())
                            .map(|h| h as i32),
                        timestamp: tx
                            .get("status")
                            .and_then(|s| s.get("block_time"))
                            .and_then(|t| t.as_i64()),
                        fee: tx.get("fee").and_then(|f| f.as_u64()),
                    });
                }
            }
        }

        Ok(transactions)
    }

    /// Get intent history (for wallet integration)
    pub async fn get_intent_history(
        &self,
        address: &str,
        _total_txs: Option<i32>,
    ) -> Result<Vec<Value>> {
        // This is a simplified version - full implementation would parse
        // transactions and create intent objects for wallet
        let txs = self.rpc.get_address_txs(address).await?;
        
        if let Some(txs_array) = txs.as_array() {
            Ok(txs_array.clone())
        } else {
            Ok(vec![])
        }
    }
}
