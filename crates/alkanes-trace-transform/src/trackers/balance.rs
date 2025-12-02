use crate::backend::StorageBackend;
use crate::extractor::TraceExtractor;
use crate::tracker::StateTracker;
use crate::types::{AlkaneId, TraceEvent, TransactionContext, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Balance change for a specific alkane at a specific outpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceChange {
    pub outpoint: String, // txid:vout
    pub address: String,
    pub alkane_id: AlkaneId,
    pub amount: u128,
    pub block_height: i32,
    pub tx_hash: String, // Transaction hash for tracking
}

/// Aggregated balance per address per alkane
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddressBalance {
    pub address: String,
    pub alkane_id: AlkaneId,
    pub total_amount: u128,
}

/// UTXO-level balance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UtxoBalance {
    pub outpoint: String,
    pub address: String,
    pub alkane_id: AlkaneId,
    pub amount: u128,
    pub block_height: i32,
    pub spent: bool,
}

/// Extracts balance changes from value_transfer trace events
pub struct ValueTransferExtractor {
    pub context: Option<TransactionContext>,
}

impl ValueTransferExtractor {
    pub fn new() -> Self {
        Self { context: None }
    }
    
    pub fn with_context(context: TransactionContext) -> Self {
        Self { context: Some(context) }
    }
    
    /// Extract transfers from value_transfer event data
    fn extract_transfers(&self, data: &serde_json::Value, vout: i32) -> Vec<BalanceChange> {
        let mut changes = Vec::new();
        
        // Get the redirect_to field (which vout the value transfers to)
        let redirect_to = data.get("redirect_to")
            .and_then(|v| v.as_i64())
            .unwrap_or(vout as i64) as i32;
        
        // Get the address for the target vout from context
        let address = self.context.as_ref()
            .and_then(|ctx| ctx.vouts.iter().find(|v| v.index == redirect_to))
            .and_then(|v| v.address.clone());
        
        if address.is_none() {
            return changes;
        }
        
        let address = address.unwrap();
        let block_height = self.context.as_ref().map(|c| c.block_height).unwrap_or(0);
        let txid = self.context.as_ref().map(|c| c.txid.clone()).unwrap_or_default();
        let outpoint = format!("{}:{}", txid, redirect_to);
        
        // Extract transfers array
        let transfers = data.get("transfers")
            .and_then(|v| v.as_array())
            .map(|a| a.as_slice())
            .unwrap_or(&[]);
        
        for transfer in transfers {
            // Parse alkane ID
            let alkane_id = transfer.get("id")
                .or_else(|| transfer.get("alkaneId"));

            if let Some(id_obj) = alkane_id {
                // block and tx can be strings or numbers - handle both
                let block = id_obj.get("block")
                    .and_then(|v| {
                        v.as_str().and_then(|s| s.parse::<i32>().ok())
                            .or_else(|| v.as_i64().map(|n| n as i32))
                    })
                    .unwrap_or(0);
                let tx = id_obj.get("tx")
                    .and_then(|v| {
                        v.as_str().and_then(|s| s.parse::<i64>().ok())
                            .or_else(|| v.as_i64())
                    })
                    .unwrap_or(0);

                // Parse amount - value can be string or number
                let amount = transfer.get("value")
                    .or_else(|| transfer.get("amount"))
                    .and_then(|v| {
                        v.as_str().and_then(|s| s.parse::<u128>().ok())
                            .or_else(|| v.as_u64().map(|n| n as u128))
                            .or_else(|| v.as_i64().map(|n| n as u128))
                    })
                    .unwrap_or(0);

                if block > 0 && amount > 0 {
                    changes.push(BalanceChange {
                        outpoint: outpoint.clone(),
                        address: address.clone(),
                        alkane_id: AlkaneId::new(block, tx),
                        amount,
                        block_height,
                        tx_hash: txid.clone(),
                    });
                }
            }
        }
        
        changes
    }
    
    /// Extract balance changes from receive_intent event with transfers array
    /// NOTE: receive_intent events show what's INCOMING to a protostone (shadow vout),
    /// but we should NOT create balance entries from them directly because:
    /// 1. The vout is a virtual protostone index, not a physical output
    /// 2. The actual destination is determined by value_transfer events
    /// We keep this for backward compatibility with tests that use incoming_alkanes format
    fn extract_from_receive_intent(&self, data: &serde_json::Value, vout: i32) -> Vec<BalanceChange> {
        let mut changes = Vec::new();

        // Get the address for this vout from context
        // Note: For receive_intent, vout is typically a shadow vout (tx.output.len() + 1 + i)
        // which won't have an address. This is expected behavior for protocol messages.
        let address = self.context.as_ref()
            .and_then(|ctx| ctx.vouts.iter().find(|v| v.index == vout))
            .and_then(|v| v.address.clone());

        if address.is_none() {
            // This is expected for receive_intent events on shadow vouts
            // The actual balance tracking happens via value_transfer events
            return changes;
        }

        let address = address.unwrap();
        let block_height = self.context.as_ref().map(|c| c.block_height).unwrap_or(0);
        let txid = self.context.as_ref().map(|c| c.txid.clone()).unwrap_or_default();
        let outpoint = format!("{}:{}", txid, vout);

        // The field name varies depending on the source:
        // - From protostone.rs convert_trace_to_events: "transfers"
        // - From test data: "incoming_alkanes" or "incomingAlkanes"
        let incoming_alkanes = data.get("transfers")
            .or_else(|| data.get("incoming_alkanes"))
            .or_else(|| data.get("incomingAlkanes"))
            .and_then(|v| v.as_array())
            .map(|a| a.as_slice())
            .unwrap_or(&[]);

        for alkane_entry in incoming_alkanes {
            // Parse alkane ID
            let alkane_id = alkane_entry.get("id");

            if let Some(id_obj) = alkane_id {
                // block and tx can be strings or numbers - handle both
                let block = id_obj.get("block")
                    .and_then(|v| {
                        v.as_str().and_then(|s| s.parse::<i32>().ok())
                            .or_else(|| v.as_i64().map(|n| n as i32))
                    })
                    .unwrap_or(0);
                let tx = id_obj.get("tx")
                    .and_then(|v| {
                        v.as_str().and_then(|s| s.parse::<i64>().ok())
                            .or_else(|| v.as_i64())
                    })
                    .unwrap_or(0);

                // Parse amount from value field - can be:
                // - U128 format {lo, hi}
                // - String representation
                // - Direct number
                let amount = alkane_entry.get("value")
                    .and_then(|v| {
                        // Check for U128 format {lo, hi}
                        if let Some(lo) = v.get("lo") {
                            lo.as_i64().or_else(|| lo.as_u64().map(|n| n as i64))
                                .map(|n| n as u128)
                        } else {
                            // Fallback to direct value (string or number)
                            v.as_str().and_then(|s| s.parse::<u128>().ok())
                                .or_else(|| v.as_u64().map(|n| n as u128))
                                .or_else(|| v.as_i64().map(|n| n as u128))
                        }
                    })
                    .or_else(|| {
                        // Fallback to amount field
                        alkane_entry.get("amount")
                            .and_then(|v| {
                                v.as_str().and_then(|s| s.parse::<u128>().ok())
                                    .or_else(|| v.as_u64().map(|n| n as u128))
                                    .or_else(|| v.as_i64().map(|n| n as u128))
                            })
                    })
                    .unwrap_or(0);

                if block > 0 && amount > 0 {
                    changes.push(BalanceChange {
                        outpoint: outpoint.clone(),
                        address: address.clone(),
                        alkane_id: AlkaneId::new(block, tx),
                        amount,
                        block_height,
                        tx_hash: txid.clone(),
                    });
                }
            }
        }

        changes
    }
}

impl Default for ValueTransferExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl TraceExtractor for ValueTransferExtractor {
    type Output = Vec<BalanceChange>;
    
    fn extract(&self, trace: &TraceEvent) -> Result<Option<Vec<BalanceChange>>> {
        let changes = match trace.event_type.as_str() {
            "value_transfer" => self.extract_transfers(&trace.data, trace.vout),
            "receive_intent" => self.extract_from_receive_intent(&trace.data, trace.vout),
            _ => return Ok(None),
        };
        
        if changes.is_empty() {
            Ok(None)
        } else {
            Ok(Some(changes))
        }
    }
    
    fn name(&self) -> &'static str {
        "value_transfer_extractor"
    }
}

/// Tracks aggregate balances and UTXO-level balances
pub struct BalanceTracker;

impl BalanceTracker {
    pub fn new() -> Self {
        Self
    }
    
    /// Encode key for aggregate balance: "balance:{address}:{alkane_id}"
    fn balance_key(address: &str, alkane_id: &AlkaneId) -> Vec<u8> {
        format!("balance:{}:{}", address, alkane_id.to_string()).into_bytes()
    }
    
    /// Encode key for UTXO balance: "utxo:{outpoint}:{alkane_id}"
    fn utxo_key(outpoint: &str, alkane_id: &AlkaneId) -> Vec<u8> {
        format!("utxo:{}:{}", outpoint, alkane_id.to_string()).into_bytes()
    }
    
    /// Encode key for holder enumeration: "holder:{alkane_id}:{address}"
    fn holder_key(alkane_id: &AlkaneId, address: &str) -> Vec<u8> {
        format!("holder:{}:{}", alkane_id.to_string(), address).into_bytes()
    }
}

impl Default for BalanceTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl StateTracker for BalanceTracker {
    type Input = Vec<BalanceChange>;
    
    fn name(&self) -> &'static str {
        "value_transfer_extractor"
    }
    
    fn update<B: StorageBackend>(&mut self, backend: &mut B, changes: Vec<BalanceChange>) -> Result<()> {
        for change in changes {
            let balance_key = Self::balance_key(&change.address, &change.alkane_id);
            let utxo_key = Self::utxo_key(&change.outpoint, &change.alkane_id);
            let holder_key = Self::holder_key(&change.alkane_id, &change.address);
            
            // Update UTXO-level balance
            let utxo_balance = UtxoBalance {
                outpoint: change.outpoint.clone(),
                address: change.address.clone(),
                alkane_id: change.alkane_id.clone(),
                amount: change.amount,
                block_height: change.block_height,
                spent: false,
            };
            
            let utxo_bytes = serde_json::to_vec(&utxo_balance)?;
            backend.set("utxo_balances", &utxo_key, &utxo_bytes)?;
            
            // Update aggregate balance
            let current_balance = backend.get("address_balances", &balance_key)?
                .and_then(|bytes| serde_json::from_slice::<AddressBalance>(&bytes).ok())
                .map(|b| b.total_amount)
                .unwrap_or(0);
            
            let new_balance = AddressBalance {
                address: change.address.clone(),
                alkane_id: change.alkane_id.clone(),
                total_amount: current_balance + change.amount,
            };
            
            let balance_bytes = serde_json::to_vec(&new_balance)?;
            backend.set("address_balances", &balance_key, &balance_bytes)?;
            
            // Update holder enumeration
            backend.set("holders", &holder_key, &balance_bytes)?;
        }
        
        Ok(())
    }
    
    fn reset<B: StorageBackend>(&mut self, backend: &mut B) -> Result<()> {
        // Clear all tables
        for key in ["address_balances", "utxo_balances", "holders"] {
            for (k, _) in backend.scan(key)? {
                backend.delete(key, &k)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::InMemoryBackend;
    use crate::types::VoutInfo;
    use serde_json::json;
    
    fn create_test_context() -> TransactionContext {
        TransactionContext {
            txid: "abc123".to_string(),
            block_height: 100,
            timestamp: chrono::Utc::now(),
            vouts: vec![
                VoutInfo {
                    index: 0,
                    address: Some("bc1qtest".to_string()),
                    script_pubkey: "".to_string(),
                    value: 1000,
                },
                VoutInfo {
                    index: 1,
                    address: Some("bc1qtest2".to_string()),
                    script_pubkey: "".to_string(),
                    value: 2000,
                },
            ],
        }
    }
    
    #[test]
    fn test_value_transfer_extraction() {
        let context = create_test_context();
        let extractor = ValueTransferExtractor::with_context(context);
        
        let trace = TraceEvent {
            event_type: "value_transfer".to_string(),
            vout: 0,
            alkane_address_block: "4".to_string(),
            alkane_address_tx: "0".to_string(),
            data: json!({
                "redirect_to": 1,
                "transfers": [
                    {
                        "id": {"block": 4, "tx": 10},
                        "amount": "1000"
                    },
                    {
                        "id": {"block": 4, "tx": 20},
                        "amount": "2000"
                    }
                ]
            }),
        };
        
        let result = extractor.extract(&trace).unwrap();
        assert!(result.is_some());
        
        let changes = result.unwrap();
        assert_eq!(changes.len(), 2);
        assert_eq!(changes[0].address, "bc1qtest2");
        assert_eq!(changes[0].alkane_id.block, 4);
        assert_eq!(changes[0].alkane_id.tx, 10);
        assert_eq!(changes[0].amount, 1000);
        assert_eq!(changes[1].amount, 2000);
    }
    
    #[test]
    fn test_balance_tracking() {
        let mut backend = InMemoryBackend::new();
        let mut tracker = BalanceTracker::new();
        
        let changes = vec![
            BalanceChange {
                outpoint: "abc123:0".to_string(),
                address: "bc1qtest".to_string(),
                alkane_id: AlkaneId::new(4, 10),
                amount: 1000,
                block_height: 100,
                tx_hash: "abc123".to_string(),
            },
            BalanceChange {
                outpoint: "abc123:1".to_string(),
                address: "bc1qtest".to_string(),
                alkane_id: AlkaneId::new(4, 10),
                amount: 500,
                block_height: 100,
                tx_hash: "abc123".to_string(),
            },
        ];
        
        tracker.update(&mut backend, changes).unwrap();
        
        // Check aggregate balance
        let balance_key = BalanceTracker::balance_key("bc1qtest", &AlkaneId::new(4, 10));
        let balance_bytes = backend.get("address_balances", &balance_key).unwrap().unwrap();
        let balance: AddressBalance = serde_json::from_slice(&balance_bytes).unwrap();
        
        assert_eq!(balance.total_amount, 1500);
        assert_eq!(balance.address, "bc1qtest");
        
        // Check UTXO balances
        let utxo_key = BalanceTracker::utxo_key("abc123:0", &AlkaneId::new(4, 10));
        let utxo_bytes = backend.get("utxo_balances", &utxo_key).unwrap().unwrap();
        let utxo: UtxoBalance = serde_json::from_slice(&utxo_bytes).unwrap();
        
        assert_eq!(utxo.amount, 1000);
        assert!(!utxo.spent);
    }
    
    #[test]
    fn test_balance_accumulation() {
        let mut backend = InMemoryBackend::new();
        let mut tracker = BalanceTracker::new();
        
        // First deposit
        tracker.update(&mut backend, vec![
            BalanceChange {
                outpoint: "tx1:0".to_string(),
                address: "bc1qtest".to_string(),
                alkane_id: AlkaneId::new(4, 10),
                amount: 1000,
                block_height: 100,
                tx_hash: "tx1".to_string(),
            },
        ]).unwrap();
        
        // Second deposit
        tracker.update(&mut backend, vec![
            BalanceChange {
                outpoint: "tx2:0".to_string(),
                address: "bc1qtest".to_string(),
                alkane_id: AlkaneId::new(4, 10),
                amount: 2000,
                block_height: 101,
                tx_hash: "tx2".to_string(),
            },
        ]).unwrap();
        
        // Check accumulated balance
        let balance_key = BalanceTracker::balance_key("bc1qtest", &AlkaneId::new(4, 10));
        let balance_bytes = backend.get("address_balances", &balance_key).unwrap().unwrap();
        let balance: AddressBalance = serde_json::from_slice(&balance_bytes).unwrap();
        
        assert_eq!(balance.total_amount, 3000);
    }
}
