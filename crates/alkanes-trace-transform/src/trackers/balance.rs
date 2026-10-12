use crate::backend::StorageBackend;
use crate::extractor::TraceExtractor;
use crate::tracker::StateTracker;
use crate::types::{AlkaneId, Result, TraceEvent, TransactionContext};
use anyhow::{bail, Context};
use serde::{Deserialize, Serialize};

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
        Self {
            context: Some(context),
        }
    }

    /// Extract transfers from value_transfer event data
    fn extract_transfers(&self, data: &serde_json::Value, vout: i32) -> Result<Vec<BalanceChange>> {
        let mut changes = Vec::new();

        let context = self
            .context
            .as_ref()
            .context("value_transfer extraction requires transaction context")?;

        // Get the redirect_to field (which vout the value transfers to)
        let redirect_to = match data.get("redirect_to") {
            Some(value) => parse_i32(value, "value_transfer.redirect_to")?,
            None => vout,
        };
        if redirect_to < 0 {
            bail!("value_transfer.redirect_to cannot be negative");
        }

        // Get the address for the target vout from context
        let address = context
            .vouts
            .iter()
            .find(|v| v.index == redirect_to)
            .and_then(|v| v.address.clone());

        let block_height = context.block_height;
        let txid = context.txid.clone();
        let outpoint = format!("{}:{}", txid, redirect_to);

        // Extract transfers array
        let transfers = data
            .get("transfers")
            .and_then(|v| v.as_array())
            .context("value_transfer.transfers must be an array")?;

        for transfer in transfers {
            // Parse alkane ID
            let alkane_id = transfer
                .get("id")
                .or_else(|| transfer.get("alkaneId"))
                .context("value_transfer entry is missing id")?;
            let block = parse_i32(
                alkane_id
                    .get("block")
                    .context("value_transfer id is missing block")?,
                "value_transfer.id.block",
            )?;
            let tx = parse_i64(
                alkane_id
                    .get("tx")
                    .context("value_transfer id is missing tx")?,
                "value_transfer.id.tx",
            )?;
            if block <= 0 || tx < 0 {
                bail!("value_transfer alkane id must have block > 0 and tx >= 0");
            }

            let amount = parse_u128(
                transfer
                    .get("value")
                    .or_else(|| transfer.get("amount"))
                    .context("value_transfer entry is missing value/amount")?,
                "value_transfer amount",
            )?;

            if amount > 0 {
                // Addressless protocol outputs are not spendable Marketplace inventory, but
                // their payloads are still fully validated above so malformed events fail closed.
                if let Some(address) = address.as_ref() {
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

        Ok(changes)
    }

    /// Extract balance changes from receive_intent event with transfers array
    /// NOTE: receive_intent events show what's INCOMING to a protostone (shadow vout),
    /// but we should NOT create balance entries from them directly because:
    /// 1. The vout is a virtual protostone index, not a physical output
    /// 2. The actual destination is determined by value_transfer events
    /// We keep this for backward compatibility with tests that use incoming_alkanes format
    fn extract_from_receive_intent(
        &self,
        data: &serde_json::Value,
        vout: i32,
    ) -> Result<Vec<BalanceChange>> {
        let mut changes = Vec::new();

        let context = self
            .context
            .as_ref()
            .context("receive_intent extraction requires transaction context")?;

        // Get the address for this vout from context
        // Note: For receive_intent, vout is typically a shadow vout (tx.output.len() + 1 + i)
        // which won't have an address. This is expected behavior for protocol messages.
        let address = context
            .vouts
            .iter()
            .find(|v| v.index == vout)
            .and_then(|v| v.address.clone());

        let block_height = context.block_height;
        let txid = context.txid.clone();
        let outpoint = format!("{}:{}", txid, vout);

        // The field name varies depending on the source:
        // - From protostone.rs convert_trace_to_events: "transfers"
        // - From test data: "incoming_alkanes" or "incomingAlkanes"
        let incoming_alkanes = data
            .get("transfers")
            .or_else(|| data.get("incoming_alkanes"))
            .or_else(|| data.get("incomingAlkanes"))
            .and_then(|v| v.as_array())
            .context("receive_intent transfers/incoming_alkanes must be an array")?;

        for alkane_entry in incoming_alkanes {
            // Parse alkane ID
            let alkane_id = alkane_entry
                .get("id")
                .context("receive_intent entry is missing id")?;
            let block = parse_i32(
                alkane_id
                    .get("block")
                    .context("receive_intent id is missing block")?,
                "receive_intent.id.block",
            )?;
            let tx = parse_i64(
                alkane_id
                    .get("tx")
                    .context("receive_intent id is missing tx")?,
                "receive_intent.id.tx",
            )?;
            if block <= 0 || tx < 0 {
                bail!("receive_intent alkane id must have block > 0 and tx >= 0");
            }
            let amount = parse_u128(
                alkane_entry
                    .get("value")
                    .or_else(|| alkane_entry.get("amount"))
                    .context("receive_intent entry is missing value/amount")?,
                "receive_intent amount",
            )?;

            if amount > 0 {
                // Shadow vouts normally have no address, but their payloads must still be valid.
                if let Some(address) = address.as_ref() {
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

        Ok(changes)
    }
}

fn parse_i32(value: &serde_json::Value, label: &str) -> Result<i32> {
    if let Some(text) = value.as_str() {
        return text
            .parse::<i32>()
            .with_context(|| format!("{label} is not canonical i32 decimal text"));
    }
    if let Some(number) = value.as_i64() {
        return i32::try_from(number).with_context(|| format!("{label} exceeds i32"));
    }
    bail!("{label} must be an integer or decimal string")
}

fn parse_i64(value: &serde_json::Value, label: &str) -> Result<i64> {
    if let Some(text) = value.as_str() {
        return text
            .parse::<i64>()
            .with_context(|| format!("{label} is not canonical i64 decimal text"));
    }
    value
        .as_i64()
        .with_context(|| format!("{label} must be an integer or decimal string"))
}

fn parse_u64_word(value: &serde_json::Value, label: &str) -> Result<u64> {
    if let Some(text) = value.as_str() {
        return text
            .parse::<u64>()
            .with_context(|| format!("{label} is not canonical u64 decimal text"));
    }
    value
        .as_u64()
        .with_context(|| format!("{label} must be a non-negative integer or decimal string"))
}

fn parse_u128(value: &serde_json::Value, label: &str) -> Result<u128> {
    if let Some(text) = value.as_str() {
        return text
            .parse::<u128>()
            .with_context(|| format!("{label} is not canonical u128 decimal text"));
    }
    if let Some(number) = value.as_u64() {
        return Ok(number as u128);
    }
    if value.as_i64().is_some() {
        bail!("{label} cannot be negative");
    }
    if let Some(object) = value.as_object() {
        let lo = parse_u64_word(
            object.get("lo").context("u128 object is missing lo")?,
            "u128.lo",
        )?;
        let hi = match object.get("hi") {
            Some(hi) => parse_u64_word(hi, "u128.hi")?,
            None => 0,
        };
        return Ok(((hi as u128) << 64) | lo as u128);
    }
    bail!("{label} must be a non-negative integer, decimal string, or {{lo, hi}} object")
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
            "value_transfer" => self.extract_transfers(&trace.data, trace.vout)?,
            "receive_intent" => self.extract_from_receive_intent(&trace.data, trace.vout)?,
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

    fn update<B: StorageBackend>(
        &mut self,
        backend: &mut B,
        changes: Vec<BalanceChange>,
    ) -> Result<()> {
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
            let current_balance = backend
                .get("address_balances", &balance_key)?
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
    fn malformed_value_transfer_fails_closed() {
        let extractor = ValueTransferExtractor::with_context(create_test_context());
        let trace = TraceEvent {
            event_type: "value_transfer".to_string(),
            vout: 0,
            alkane_address_block: "4".to_string(),
            alkane_address_tx: "0".to_string(),
            data: json!({"redirect_to": 1}),
        };

        assert!(extractor.extract(&trace).is_err());
    }

    #[test]
    fn negative_amount_never_wraps_to_u128() {
        let extractor = ValueTransferExtractor::with_context(create_test_context());
        let trace = TraceEvent {
            event_type: "value_transfer".to_string(),
            vout: 0,
            alkane_address_block: "4".to_string(),
            alkane_address_tx: "0".to_string(),
            data: json!({
                "transfers": [{"id": {"block": 4, "tx": 10}, "amount": -1}]
            }),
        };

        assert!(extractor.extract(&trace).is_err());
    }

    #[test]
    fn malformed_alkane_id_fails_closed() {
        let extractor = ValueTransferExtractor::with_context(create_test_context());
        let trace = TraceEvent {
            event_type: "value_transfer".to_string(),
            vout: 0,
            alkane_address_block: "4".to_string(),
            alkane_address_tx: "0".to_string(),
            data: json!({
                "transfers": [{"id": {"block": "not-a-number", "tx": 10}, "amount": 1}]
            }),
        };

        assert!(extractor.extract(&trace).is_err());
    }

    #[test]
    fn full_u128_word_encoding_is_preserved() {
        let extractor = ValueTransferExtractor::with_context(create_test_context());
        let trace = TraceEvent {
            event_type: "value_transfer".to_string(),
            vout: 0,
            alkane_address_block: "4".to_string(),
            alkane_address_tx: "0".to_string(),
            data: json!({
                "transfers": [{
                    "id": {"block": 4, "tx": 10},
                    "amount": {"lo": 7, "hi": 1}
                }]
            }),
        };

        let changes = extractor.extract(&trace).unwrap().unwrap();
        assert_eq!(changes[0].amount, (1_u128 << 64) + 7);
    }

    #[test]
    fn addressless_events_are_still_validated() {
        let mut context = create_test_context();
        context.vouts[0].address = None;
        let extractor = ValueTransferExtractor::with_context(context);
        let trace = TraceEvent {
            event_type: "value_transfer".to_string(),
            vout: 0,
            alkane_address_block: "4".to_string(),
            alkane_address_tx: "0".to_string(),
            data: json!({
                "transfers": [{"id": {"block": 4, "tx": 10}, "amount": -1}]
            }),
        };

        assert!(extractor.extract(&trace).is_err());
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
        let balance_bytes = backend
            .get("address_balances", &balance_key)
            .unwrap()
            .unwrap();
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
        tracker
            .update(
                &mut backend,
                vec![BalanceChange {
                    outpoint: "tx1:0".to_string(),
                    address: "bc1qtest".to_string(),
                    alkane_id: AlkaneId::new(4, 10),
                    amount: 1000,
                    block_height: 100,
                    tx_hash: "tx1".to_string(),
                }],
            )
            .unwrap();

        // Second deposit
        tracker
            .update(
                &mut backend,
                vec![BalanceChange {
                    outpoint: "tx2:0".to_string(),
                    address: "bc1qtest".to_string(),
                    alkane_id: AlkaneId::new(4, 10),
                    amount: 2000,
                    block_height: 101,
                    tx_hash: "tx2".to_string(),
                }],
            )
            .unwrap();

        // Check accumulated balance
        let balance_key = BalanceTracker::balance_key("bc1qtest", &AlkaneId::new(4, 10));
        let balance_bytes = backend
            .get("address_balances", &balance_key)
            .unwrap()
            .unwrap();
        let balance: AddressBalance = serde_json::from_slice(&balance_bytes).unwrap();

        assert_eq!(balance.total_amount, 3000);
    }
}
