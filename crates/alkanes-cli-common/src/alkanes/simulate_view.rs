//! Native mirrors + JSON-RPC helpers for the RC8 view functions
//! `simulatetransaction`, `simulateprotostones`, and `simulateblock`
//! exposed by alkanes-rs v2.2.0-rc.8.
//!
//! The prost structs ([`alkanes_support::proto::alkanes::SimulateTransactionRequest`]
//! et al.) are the wire format; the structs in this module are ergonomic
//! Rust mirrors with [`From`] conversions in both directions, so client
//! code never has to peek inside `Option<Uint128>` wrappers.
//!
//! Three async helpers ([`simulate_transaction`], [`simulate_protostones`],
//! [`simulate_block`]) take a `&P: MetashrewRpcProvider`, build the prost
//! request, hex-encode it, dispatch through
//! [`metashrew_view_call`](crate::traits::MetashrewRpcProvider::metashrew_view_call),
//! decode the response, and hand back the native form.

use crate::traits::MetashrewRpcProvider;
use crate::{AlkanesError, Result};
use alkanes_support::proto::alkanes as pb;
use prost::Message;
use serde::{Deserialize, Serialize};

#[cfg(not(feature = "std"))]
use alloc::{format, string::{String, ToString}, vec::Vec};
#[cfg(feature = "std")]
use std::vec::Vec;

/// View-function name used in `metashrew_view`.
pub const VIEW_SIMULATE_TRANSACTION: &str = "simulatetransaction";
/// View-function name used in `metashrew_view`.
pub const VIEW_SIMULATE_PROTOSTONES: &str = "simulateprotostones";
/// View-function name used in `metashrew_view`.
pub const VIEW_SIMULATE_BLOCK: &str = "simulateblock";

// ---------------------------------------------------------------------------
// Native mirrors
// ---------------------------------------------------------------------------

/// Compact `(block, tx)` identifier; mirrors `pb::AlkaneId`'s two u128 fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AlkaneId {
    pub block: u128,
    pub tx: u128,
}

/// One transfer of an alkane amount, mirroring `pb::AlkaneTransfer`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlkaneTransfer {
    pub id: AlkaneId,
    pub value: u128,
}

/// One `key → value` pair from an alkane's storage slot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyValue {
    #[serde(with = "hex_bytes")]
    pub key: Vec<u8>,
    #[serde(with = "hex_bytes")]
    pub value: Vec<u8>,
}

/// Pre-execution storage override, applied to the sandbox atomic before
/// the protostones run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorageOverride {
    pub alkane: AlkaneId,
    pub entries: Vec<KeyValue>,
}

/// Post-execution final state of every storage slot touched during a
/// protostone, keyed by the alkane that owns the slot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TouchedStorage {
    pub alkane: AlkaneId,
    pub entries: Vec<KeyValue>,
}

/// Bitcoin outpoint `(txid, vout)`; mirrors `pb::Outpoint`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Outpoint {
    #[serde(with = "hex_bytes")]
    pub txid: Vec<u8>,
    pub vout: u32,
}

/// All alkane balances routed to one tx output after simulation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VoutBalances {
    pub vout: u32,
    pub balances: Vec<AlkaneTransfer>,
}

/// One protostone execution from a simulation; `trace` is the raw
/// prost-encoded `AlkanesTrace` bytes (callers convert via
/// `alkanes_support::trace::Trace::try_from(bytes)` to render).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtostoneExecution {
    pub index: u32,
    pub outpoint: Outpoint,
    /// Hex-encoded prost bytes of `pb::AlkanesTrace`. Kept as bytes here
    /// to avoid pulling in the full trace type; convert client-side.
    #[serde(with = "hex_bytes")]
    pub trace: Vec<u8>,
    pub fuel_used: u64,
    pub touched_storage: Vec<TouchedStorage>,
}

/// Native form of `pb::SimulateTransactionResponse`. Returned by all
/// three helper fns (`simulateblock` returns a `Vec` of these).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SimulateTransactionResponse {
    pub txid: String,
    pub height: u64,
    pub protostones: Vec<ProtostoneExecution>,
    pub final_balances_by_vout: Vec<VoutBalances>,
    pub total_fuel_used: u64,
    #[serde(with = "hex_bytes")]
    pub used_transaction: Vec<u8>,
    #[serde(with = "hex_bytes")]
    pub used_block: Vec<u8>,
    pub error: String,
}

/// Native form of `pb::SimulateBlockResponse` — wraps a per-tx slice.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SimulateBlockResponse {
    pub block_hash: String,
    pub height: u64,
    pub txs: Vec<SimulateTransactionResponse>,
    pub total_fuel_used: u64,
    #[serde(with = "hex_bytes")]
    pub used_block: Vec<u8>,
    pub error: String,
}

/// Input for [`simulate_transaction`] — a height + raw tx (or PSBT) +
/// optional storage overrides.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SimulateTransactionInput {
    pub height: u64,
    #[serde(with = "hex_bytes")]
    pub transaction: Vec<u8>,
    #[serde(default)]
    pub storage_overrides: Vec<StorageOverride>,
}

/// Input for [`simulate_protostones`] — height + alkane inputs +
/// enciphered protostones bytes + optional tx/block + storage overrides.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SimulateProtostonesInput {
    pub height: u64,
    pub alkane_inputs: Vec<AlkaneTransfer>,
    #[serde(with = "hex_bytes")]
    pub protostones: Vec<u8>,
    #[serde(default, with = "hex_bytes")]
    pub transaction: Vec<u8>,
    #[serde(default, with = "hex_bytes")]
    pub block: Vec<u8>,
    #[serde(default)]
    pub storage_overrides: Vec<StorageOverride>,
}

/// Input for [`simulate_block`] — height + consensus-encoded block +
/// optional shared storage overrides.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SimulateBlockInput {
    pub height: u64,
    #[serde(with = "hex_bytes")]
    pub block: Vec<u8>,
    #[serde(default)]
    pub storage_overrides: Vec<StorageOverride>,
}

// ---------------------------------------------------------------------------
// From <-> proto impls (native ↔ wire)
// ---------------------------------------------------------------------------

fn pb_u128(v: u128) -> pb::Uint128 {
    pb::Uint128 {
        lo: (v & 0xFFFF_FFFF_FFFF_FFFF) as u64,
        hi: (v >> 64) as u64,
    }
}

fn from_pb_u128(v: &pb::Uint128) -> u128 {
    ((v.hi as u128) << 64) | (v.lo as u128)
}

impl From<&AlkaneId> for pb::AlkaneId {
    fn from(id: &AlkaneId) -> Self {
        pb::AlkaneId {
            block: Some(pb_u128(id.block)),
            tx: Some(pb_u128(id.tx)),
        }
    }
}
impl From<pb::AlkaneId> for AlkaneId {
    fn from(id: pb::AlkaneId) -> Self {
        AlkaneId {
            block: id.block.as_ref().map(from_pb_u128).unwrap_or_default(),
            tx: id.tx.as_ref().map(from_pb_u128).unwrap_or_default(),
        }
    }
}

impl From<&AlkaneTransfer> for pb::AlkaneTransfer {
    fn from(t: &AlkaneTransfer) -> Self {
        pb::AlkaneTransfer {
            id: Some((&t.id).into()),
            value: Some(pb_u128(t.value)),
        }
    }
}
impl From<pb::AlkaneTransfer> for AlkaneTransfer {
    fn from(t: pb::AlkaneTransfer) -> Self {
        AlkaneTransfer {
            id: t.id.map(Into::into).unwrap_or_default(),
            value: t.value.as_ref().map(from_pb_u128).unwrap_or_default(),
        }
    }
}

impl From<&KeyValue> for pb::KeyValuePair {
    fn from(kv: &KeyValue) -> Self {
        pb::KeyValuePair {
            key: kv.key.clone(),
            value: kv.value.clone(),
        }
    }
}
impl From<pb::KeyValuePair> for KeyValue {
    fn from(kv: pb::KeyValuePair) -> Self {
        KeyValue {
            key: kv.key,
            value: kv.value,
        }
    }
}

impl From<&StorageOverride> for pb::StorageOverride {
    fn from(o: &StorageOverride) -> Self {
        pb::StorageOverride {
            alkane: Some((&o.alkane).into()),
            entries: o.entries.iter().map(Into::into).collect(),
        }
    }
}
impl From<pb::StorageOverride> for StorageOverride {
    fn from(o: pb::StorageOverride) -> Self {
        StorageOverride {
            alkane: o.alkane.map(Into::into).unwrap_or_default(),
            entries: o.entries.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<pb::TouchedStorage> for TouchedStorage {
    fn from(o: pb::TouchedStorage) -> Self {
        TouchedStorage {
            alkane: o.alkane.map(Into::into).unwrap_or_default(),
            entries: o.entries.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<pb::Outpoint> for Outpoint {
    fn from(o: pb::Outpoint) -> Self {
        Outpoint {
            txid: o.txid,
            vout: o.vout,
        }
    }
}

impl From<pb::VoutBalances> for VoutBalances {
    fn from(v: pb::VoutBalances) -> Self {
        VoutBalances {
            vout: v.vout,
            balances: v.balances.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<pb::ProtostoneExecution> for ProtostoneExecution {
    fn from(p: pb::ProtostoneExecution) -> Self {
        let trace_bytes = p
            .trace
            .as_ref()
            .map(|t| t.encode_to_vec())
            .unwrap_or_default();
        ProtostoneExecution {
            index: p.index,
            outpoint: p.outpoint.map(Into::into).unwrap_or_default(),
            trace: trace_bytes,
            fuel_used: p.fuel_used,
            touched_storage: p.touched_storage.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<pb::SimulateTransactionResponse> for SimulateTransactionResponse {
    fn from(r: pb::SimulateTransactionResponse) -> Self {
        SimulateTransactionResponse {
            txid: r.txid,
            height: r.height,
            protostones: r.protostones.into_iter().map(Into::into).collect(),
            final_balances_by_vout: r
                .final_balances_by_vout
                .into_iter()
                .map(Into::into)
                .collect(),
            total_fuel_used: r.total_fuel_used,
            used_transaction: r.used_transaction,
            used_block: r.used_block,
            error: r.error,
        }
    }
}

impl From<pb::SimulateBlockResponse> for SimulateBlockResponse {
    fn from(r: pb::SimulateBlockResponse) -> Self {
        SimulateBlockResponse {
            block_hash: r.block_hash,
            height: r.height,
            txs: r.txs.into_iter().map(Into::into).collect(),
            total_fuel_used: r.total_fuel_used,
            used_block: r.used_block,
            error: r.error,
        }
    }
}

impl From<&SimulateTransactionInput> for pb::SimulateTransactionRequest {
    fn from(i: &SimulateTransactionInput) -> Self {
        pb::SimulateTransactionRequest {
            height: i.height,
            transaction: i.transaction.clone(),
            storage_overrides: i.storage_overrides.iter().map(Into::into).collect(),
        }
    }
}

impl From<&SimulateProtostonesInput> for pb::SimulateProtostonesRequest {
    fn from(i: &SimulateProtostonesInput) -> Self {
        pb::SimulateProtostonesRequest {
            height: i.height,
            alkane_inputs: i.alkane_inputs.iter().map(Into::into).collect(),
            protostones: i.protostones.clone(),
            transaction: i.transaction.clone(),
            block: i.block.clone(),
            storage_overrides: i.storage_overrides.iter().map(Into::into).collect(),
        }
    }
}

impl From<&SimulateBlockInput> for pb::SimulateBlockRequest {
    fn from(i: &SimulateBlockInput) -> Self {
        pb::SimulateBlockRequest {
            height: i.height,
            block: i.block.clone(),
            storage_overrides: i.storage_overrides.iter().map(Into::into).collect(),
        }
    }
}

// ---------------------------------------------------------------------------
// JSON-RPC helpers
// ---------------------------------------------------------------------------

fn block_tag_str(tag: Option<&str>) -> &str {
    tag.unwrap_or("latest")
}

fn encode_proto<M: Message>(m: &M) -> String {
    format!("0x{}", hex::encode(m.encode_to_vec()))
}

/// Call `metashrew_view "simulatetransaction"` and decode the response.
///
/// Server side this drives the full per-tx replay (the same code path
/// the indexer itself uses), so per-protostone traces + per-vout balances
/// come back in one round-trip.
pub async fn simulate_transaction<P: MetashrewRpcProvider + ?Sized>(
    provider: &P,
    input: &SimulateTransactionInput,
    block_tag: Option<&str>,
) -> Result<SimulateTransactionResponse> {
    let req: pb::SimulateTransactionRequest = input.into();
    let params_hex = encode_proto(&req);
    let bytes = provider
        .metashrew_view_call(VIEW_SIMULATE_TRANSACTION, &params_hex, block_tag_str(block_tag))
        .await?;
    let resp = pb::SimulateTransactionResponse::decode(bytes.as_slice()).map_err(|e| {
        AlkanesError::Other(format!(
            "failed to decode SimulateTransactionResponse: {} ({} bytes)",
            e,
            bytes.len()
        ))
    })?;
    Ok(resp.into())
}

/// Call `metashrew_view "simulateprotostones"` and decode the response.
///
/// Use this when you have a list of protostones (and the alkane inputs
/// flowing into the first one) but no fully-constructed tx context.
pub async fn simulate_protostones<P: MetashrewRpcProvider + ?Sized>(
    provider: &P,
    input: &SimulateProtostonesInput,
    block_tag: Option<&str>,
) -> Result<SimulateTransactionResponse> {
    let req: pb::SimulateProtostonesRequest = input.into();
    let params_hex = encode_proto(&req);
    let bytes = provider
        .metashrew_view_call(VIEW_SIMULATE_PROTOSTONES, &params_hex, block_tag_str(block_tag))
        .await?;
    let resp = pb::SimulateTransactionResponse::decode(bytes.as_slice()).map_err(|e| {
        AlkanesError::Other(format!(
            "failed to decode SimulateTransactionResponse: {} ({} bytes)",
            e,
            bytes.len()
        ))
    })?;
    Ok(resp.into())
}

/// Call `metashrew_view "simulateblock"` and decode the response.
///
/// Replays every tx in `input.block` through the shared sandbox,
/// preserving intra-block atomicity (tx[1] sees tx[0]'s writes).
/// Coinbase / no-runestone txs surface as empty-shape entries so the
/// `txs` slice aligns 1:1 with `block.txdata`.
pub async fn simulate_block<P: MetashrewRpcProvider + ?Sized>(
    provider: &P,
    input: &SimulateBlockInput,
    block_tag: Option<&str>,
) -> Result<SimulateBlockResponse> {
    let req: pb::SimulateBlockRequest = input.into();
    let params_hex = encode_proto(&req);
    let bytes = provider
        .metashrew_view_call(VIEW_SIMULATE_BLOCK, &params_hex, block_tag_str(block_tag))
        .await?;
    let resp = pb::SimulateBlockResponse::decode(bytes.as_slice()).map_err(|e| {
        AlkanesError::Other(format!(
            "failed to decode SimulateBlockResponse: {} ({} bytes)",
            e,
            bytes.len()
        ))
    })?;
    Ok(resp.into())
}

// ---------------------------------------------------------------------------
// Hex-bytes serde helper
// ---------------------------------------------------------------------------

mod hex_bytes {
    use serde::{Deserialize, Deserializer, Serializer};
    #[cfg(not(feature = "std"))]
    use alloc::{string::String, vec::Vec};
    #[cfg(feature = "std")]
    use std::vec::Vec;

    pub fn serialize<S: Serializer>(bytes: &[u8], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&hex::encode(bytes))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        let s = String::deserialize(d)?;
        let s = s.strip_prefix("0x").unwrap_or(&s);
        hex::decode(s).map_err(serde::de::Error::custom)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn u128_round_trip_through_proto() {
        let v: u128 = 0xDEADBEEF_CAFEBABE_0102030405060708u128;
        let pb = pb_u128(v);
        assert_eq!(from_pb_u128(&pb), v);
    }

    #[test]
    fn alkane_id_round_trip() {
        let id = AlkaneId { block: 2, tx: 77087 };
        let pb: pb::AlkaneId = (&id).into();
        let back: AlkaneId = pb.into();
        assert_eq!(id, back);
    }

    #[test]
    fn alkane_transfer_round_trip() {
        let t = AlkaneTransfer {
            id: AlkaneId { block: 2, tx: 0 },
            value: 1_000_000_000_000u128,
        };
        let pb: pb::AlkaneTransfer = (&t).into();
        let back: AlkaneTransfer = pb.into();
        assert_eq!(t, back);
    }

    #[test]
    fn storage_override_round_trip() {
        let so = StorageOverride {
            alkane: AlkaneId { block: 4, tx: 70002 },
            entries: vec![
                KeyValue { key: vec![1, 2, 3], value: vec![0xff] },
                KeyValue { key: b"/balance".to_vec(), value: b"\x10\x00".to_vec() },
            ],
        };
        let pb: pb::StorageOverride = (&so).into();
        let back: StorageOverride = pb.into();
        assert_eq!(so, back);
    }

    #[test]
    fn simulate_transaction_input_encodes() {
        let i = SimulateTransactionInput {
            height: 955_828,
            transaction: vec![0xde, 0xad, 0xbe, 0xef],
            storage_overrides: vec![],
        };
        let req: pb::SimulateTransactionRequest = (&i).into();
        assert_eq!(req.height, 955_828);
        assert_eq!(req.transaction, vec![0xde, 0xad, 0xbe, 0xef]);
        let hex_params = encode_proto(&req);
        assert!(hex_params.starts_with("0x"));
        // Round-trip the wire format
        let bytes = hex::decode(&hex_params[2..]).unwrap();
        let decoded = pb::SimulateTransactionRequest::decode(bytes.as_slice()).unwrap();
        assert_eq!(decoded.height, 955_828);
    }

    #[test]
    fn simulate_protostones_input_encodes_optional_tx_block() {
        let i = SimulateProtostonesInput {
            height: 100,
            alkane_inputs: vec![AlkaneTransfer {
                id: AlkaneId { block: 2, tx: 0 },
                value: 1_000,
            }],
            protostones: vec![0xab, 0xcd],
            transaction: vec![],
            block: vec![],
            storage_overrides: vec![],
        };
        let req: pb::SimulateProtostonesRequest = (&i).into();
        assert_eq!(req.height, 100);
        assert_eq!(req.alkane_inputs.len(), 1);
        assert_eq!(req.protostones, vec![0xab, 0xcd]);
        assert!(req.transaction.is_empty());
        assert!(req.block.is_empty());
    }

    #[test]
    fn simulate_transaction_response_round_trip() {
        let pb_resp = pb::SimulateTransactionResponse {
            txid: "deadbeef".into(),
            height: 200,
            protostones: vec![pb::ProtostoneExecution {
                index: 0,
                outpoint: Some(pb::Outpoint { txid: vec![1; 32], vout: 4 }),
                trace: None,
                fuel_used: 42,
                touched_storage: vec![pb::TouchedStorage {
                    alkane: Some(pb::AlkaneId {
                        block: Some(pb_u128(2)),
                        tx: Some(pb_u128(77087)),
                    }),
                    entries: vec![pb::KeyValuePair {
                        key: b"/foo".to_vec(),
                        value: b"\x01".to_vec(),
                    }],
                }],
            }],
            final_balances_by_vout: vec![pb::VoutBalances {
                vout: 2,
                balances: vec![pb::AlkaneTransfer {
                    id: Some(pb::AlkaneId {
                        block: Some(pb_u128(2)),
                        tx: Some(pb_u128(0)),
                    }),
                    value: Some(pb_u128(99)),
                }],
            }],
            total_fuel_used: 42,
            used_transaction: vec![0xaa, 0xbb],
            used_block: vec![0xcc],
            error: String::new(),
        };
        let native: SimulateTransactionResponse = pb_resp.into();
        assert_eq!(native.txid, "deadbeef");
        assert_eq!(native.height, 200);
        assert_eq!(native.protostones.len(), 1);
        assert_eq!(native.protostones[0].index, 0);
        assert_eq!(native.protostones[0].outpoint.vout, 4);
        assert_eq!(native.protostones[0].fuel_used, 42);
        assert_eq!(native.protostones[0].touched_storage.len(), 1);
        assert_eq!(
            native.protostones[0].touched_storage[0].alkane,
            AlkaneId { block: 2, tx: 77087 }
        );
        assert_eq!(native.final_balances_by_vout[0].vout, 2);
        assert_eq!(native.final_balances_by_vout[0].balances[0].value, 99);
        assert_eq!(native.total_fuel_used, 42);
    }

    #[test]
    fn view_fn_names_match_metashrew_export() {
        assert_eq!(VIEW_SIMULATE_TRANSACTION, "simulatetransaction");
        assert_eq!(VIEW_SIMULATE_PROTOSTONES, "simulateprotostones");
        assert_eq!(VIEW_SIMULATE_BLOCK, "simulateblock");
    }
}
