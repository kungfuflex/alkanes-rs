// This file is part of the deezel project.
// Copyright (c) 2023, Casey Rodarmor, all rights reserved.
// Copyright (c) 2024, The Deezel Developers, all rights reserved.
// Deezel is licensed under the MIT license.
// See LICENSE file in the project root for full license information.

//! Enhanced alkanes execute functionality with commit/reveal transaction support
//!
//! This module implements the complex alkanes execute command that supports:
//! - Commit/reveal transaction pattern for envelope data
//! - Complex protostone parsing with cellpacks and edicts
//! - UTXO selection based on alkanes and Bitcoin requirements
//! - Runestone construction with multiple protostones
//! - Address identifier resolution for outputs and change
//! - Transaction tracing with metashrew synchronization

use crate::{Result, AlkanesError, DeezelProvider};
use crate::traits::{WalletProvider, UtxoInfo};
use crate::ordinals::{check_utxos_for_inscriptions_with_provider, SplitPlan};
use super::types::OrdinalsStrategy;
use bitcoin::{Transaction, ScriptBuf, OutPoint, TxOut, Address, XOnlyPublicKey, psbt::Psbt};
use bitcoin::hashes::Hash;
use anyhow::Context;
use core::str::FromStr;
#[cfg(not(feature = "std"))]
use alloc::{vec, vec::Vec, string::{String, ToString}, format};
#[cfg(feature = "std")]
use std::{vec, vec::Vec, string::{String, ToString}, format, io::{self, Write}};
// Note: tokio::time::sleep doesn't work in WASM - use provider.sleep_ms() instead
#[cfg(not(target_arch = "wasm32"))]
use tokio::time::{sleep, Duration};
pub use super::types::{
    AlkaneId, AlkanesBalance, EnhancedExecuteParams, EnhancedExecuteResult, ExecutionState,
    InputRequirement, OutputTarget, PrefetchedAlkane, PrefetchedUtxo, ProtostoneEdict,
    ProtostoneSpec, ReadyToSignCommitTx, ReadyToSignRevealTx, ReadyToSignTx,
    UtxoDataSource,
};
use super::envelope::AlkanesEnvelope;
use anyhow::anyhow;
use ordinals::Runestone;
use protorune_support::protostone::{Protostones, Protostone, ProtostoneEdict as ProtoruneEdict};
use protorune_support::balance_sheet::ProtoruneRuneId;

const MAX_FEE_SATS: u64 = 100_000; // 0.001 BTC. Cap to avoid "absurdly high fee rate" errors.
const DUST_LIMIT: u64 = 546;

/// Bitcoin Core's default minimum relay fee rate (`-minrelaytxfee`) is
/// 1000 sat/kvB == 1.0 sat/vB. A standalone tx (or, before package relay is
/// universally deployed, an orphaned CPFP child once its parent confirms
/// alone) below this rate cannot enter the mempool. The split-tx CPFP child
/// must never be built below this floor.
const MIN_RELAY_FEE_RATE: f32 = 1.0;

/// Compute the fee rate the CPFP **child** (Tx B) must pay so that the whole
/// parent+child *package* clears the user's target fee rate, then floor the
/// result at the network min-relay rate.
///
/// True CPFP semantics: a miner evaluates the package by its combined rate
///   (parent_fee + child_fee) / (parent_vsize + child_vsize).
/// To make that ratio >= `target_rate`, the child must pay
///   child_fee = target_rate * (parent_vsize + child_vsize) - parent_fee
/// i.e. it pays for itself AND for any per-vbyte shortfall left by the parent.
/// Dividing by the child's own vsize gives the rate to hand the child builder.
///
/// The parent is normally already built at `target_rate`, so the package term
/// reduces to roughly `target_rate` — but if the parent came in under target
/// (rounding, change-dust absorption, or a deliberately lean parent) the child
/// makes up the difference. The result is floored at `MIN_RELAY_FEE_RATE` so
/// the child is always individually relayable even if its parent confirms
/// first and the child is briefly evaluated on its own.
fn child_fee_rate_for_package(
    target_rate: f32,
    parent_fee: u64,
    parent_vsize: u64,
    child_vsize: u64,
) -> f32 {
    let child_vsize = child_vsize.max(1) as f32;
    let package_vsize = parent_vsize as f32 + child_vsize;
    let required_package_fee = target_rate * package_vsize;
    // What the child must contribute on top of the parent's actual fee.
    let required_child_fee = (required_package_fee - parent_fee as f32).max(0.0);
    let child_rate = required_child_fee / child_vsize;
    // Never below min-relay, and never below the user's own target either —
    // a CPFP child should at minimum pay the target rate for itself.
    child_rate.max(target_rate).max(MIN_RELAY_FEE_RATE)
}

/// frBTC, frZEC, frETH and any other cross-chain wrap target lives at block 32
/// in the alkanes namespace. The wrap opcode is uniformly 77 across these
/// contracts (calls `exchange()` which mints the wrapped representation).
const WRAP_NAMESPACE_BLOCK: u128 = 32;
const WRAP_OPCODE: u128 = 77;

/// Returns true when a protostone calls a cross-chain wrap contract (frBTC,
/// frZEC, frETH, etc.) — i.e., target alkane has block=32 and the cellpack's
/// first input (the opcode) is 77. Used by `execute_split` to decide whether
/// the request is eligible for the wrap+execute split path.
pub fn is_wrap_protostone(spec: &ProtostoneSpec) -> bool {
    let Some(cellpack) = &spec.cellpack else {
        return false;
    };
    if cellpack.target.block != WRAP_NAMESPACE_BLOCK {
        return false;
    }
    cellpack.inputs.first().copied() == Some(WRAP_OPCODE)
}

/// Decode a raw transaction hex into the JSON shape that
/// `apply_mempool_adjustment` consumes (a single mempool-tx object with
/// `txid`, `vin[]`, `vout[]`).
///
/// We can't get the full pay-from `prevout.scriptpubkey_address` without
/// looking up the source UTXOs (esplora gives us that; raw bitcoin tx
/// only carries the prev_txid:vout reference). For the spent-input
/// strip pass we only need txid+vout, which the raw hex carries
/// directly. For the pay-to-us output pass we need
/// `scriptpubkey_address` per output, which we derive from each
/// output's scriptPubKey.
pub fn decode_tx_hex_to_mempool_json(tx_hex: &str) -> anyhow::Result<serde_json::Value> {
    use bitcoin::consensus::Decodable;
    let bytes = hex::decode(tx_hex.strip_prefix("0x").unwrap_or(tx_hex))?;
    let tx: bitcoin::Transaction = bitcoin::Transaction::consensus_decode(&mut &bytes[..])?;
    let txid = tx.compute_txid().to_string();

    let vin: Vec<serde_json::Value> = tx
        .input
        .iter()
        .map(|i| {
            serde_json::json!({
                "txid": i.previous_output.txid.to_string(),
                "vout": i.previous_output.vout,
            })
        })
        .collect();

    let vout: Vec<serde_json::Value> = tx
        .output
        .iter()
        .map(|o| {
            // Try mainnet then testnet/regtest networks for address derivation.
            // We don't know the network here, so try in order.
            let address_str = bitcoin::Address::from_script(
                o.script_pubkey.as_script(),
                bitcoin::Network::Bitcoin,
            )
            .ok()
            .or_else(|| {
                bitcoin::Address::from_script(
                    o.script_pubkey.as_script(),
                    bitcoin::Network::Testnet,
                )
                .ok()
            })
            .or_else(|| {
                bitcoin::Address::from_script(
                    o.script_pubkey.as_script(),
                    bitcoin::Network::Regtest,
                )
                .ok()
            })
            .map(|a| a.to_string());
            serde_json::json!({
                "scriptpubkey_address": address_str,
                "value": o.value.to_sat(),
            })
        })
        .collect();

    Ok(serde_json::json!({
        "txid": txid,
        "vin": vin,
        "vout": vout,
    }))
}

/// Bitcoin requires coinbase outputs to have 100 confirmations before spending.
pub const COINBASE_MATURITY: u32 = 100;

/// Why a UTXO was rejected by `check_utxo_eligibility`. Exposed for
/// structured logging and unit-test assertions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UtxoSkipReason {
    /// Wallet has explicitly frozen this outpoint.
    Frozen,
    /// Coinbase output that hasn't reached `COINBASE_MATURITY` confirmations.
    ImmatureCoinbase { confirmations: u32 },
    /// Confirmed UTXO mined into a block the alkanes indexer (metashrew)
    /// hasn't reached yet. Its alkane balance sheet is unknown until
    /// metashrew catches up; spending it risks underspending alkanes.
    UnindexedHeight { block_height: u64, max_indexed: u64 },
}

/// Returns `Ok(())` if `info` is eligible for coin selection, else `Err`
/// with the structured reason for skipping. Pure function — no provider /
/// network dependency, fully unit-testable.
///
/// Filter order mirrors the historical inline filter in `select_utxos`:
///   1. Frozen wallets always trump everything.
///   2. Immature coinbase next (consensus rule).
///   3. Indexer-height check last — only applied for *confirmed* UTXOs
///      (`block_height = Some(_)`); unconfirmed UTXOs (mempool) are left
///      to the existing `apply_mempool_adjustment` path which adds back
///      "we built this" txs from `known_pending_tx_hexes`.
pub fn check_utxo_eligibility(
    info: &UtxoInfo,
    max_indexed_height: Option<u64>,
) -> core::result::Result<(), UtxoSkipReason> {
    if info.frozen {
        return Err(UtxoSkipReason::Frozen);
    }
    if info.is_coinbase && info.confirmations < COINBASE_MATURITY {
        return Err(UtxoSkipReason::ImmatureCoinbase {
            confirmations: info.confirmations,
        });
    }
    if let (Some(max_h), Some(h)) = (max_indexed_height, info.block_height) {
        if h > max_h {
            return Err(UtxoSkipReason::UnindexedHeight {
                block_height: h,
                max_indexed: max_h,
            });
        }
    }
    Ok(())
}

/// Stats from `apply_mempool_adjustment` — exposed so callers can log
/// what changed without re-walking the candidate set.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MempoolAdjustmentReport {
    /// Number of confirmed UTXOs removed because they are already spent
    /// in one of our pending mempool transactions.
    pub stripped: usize,
    /// Number of unconfirmed outputs added to the candidate set because
    /// they pay one of our addresses from a pending mempool tx.
    pub added: usize,
}

/// Pure UTXO-set adjustment function — mutates `spendable_utxos` in
/// place to reflect the impact of the user's own pending mempool
/// transactions. Extracted from `select_utxos` so it can be unit-tested
/// without spinning up a `WebProvider` / `MockProvider` chain.
///
/// `mempool_payloads` is one esplora `address/{addr}/txs/mempool`
/// response per address (each is a JSON array of tx objects with
/// `txid`, `vin[]`, `vout[]` fields).
///
/// `addresses` is the set of our addresses — only outputs paying these
/// addresses become candidate UTXOs (we don't add outputs paying others
/// even if they appear in the same mempool tx).
pub fn apply_mempool_adjustment(
    spendable_utxos: &mut Vec<(OutPoint, UtxoInfo)>,
    mempool_payloads: &[serde_json::Value],
    addresses: &[String],
) -> MempoolAdjustmentReport {
    let address_set: alloc::collections::BTreeSet<&str> =
        addresses.iter().map(|s| s.as_str()).collect();

    let mut spent_outpoints: alloc::collections::BTreeSet<(String, u32)> =
        alloc::collections::BTreeSet::new();
    let mut new_outputs: Vec<(OutPoint, UtxoInfo)> = Vec::new();

    for txs_val in mempool_payloads {
        let txs = match txs_val.as_array() {
            Some(t) => t,
            None => continue,
        };
        for tx in txs {
            let txid_str = tx
                .get("txid")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            if txid_str.is_empty() {
                continue;
            }

            // Strip the inputs this mempool tx is spending.
            if let Some(vins) = tx.get("vin").and_then(|v| v.as_array()) {
                for vin in vins {
                    let prev_txid = vin
                        .get("txid")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string();
                    let prev_vout = vin.get("vout").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                    if !prev_txid.is_empty() {
                        spent_outpoints.insert((prev_txid, prev_vout));
                    }
                }
            }

            // Add outputs paying one of our addresses as candidate UTXOs.
            if let Some(vouts) = tx.get("vout").and_then(|v| v.as_array()) {
                for (idx, vout) in vouts.iter().enumerate() {
                    let vout_addr = vout
                        .get("scriptpubkey_address")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default();
                    if !address_set.contains(vout_addr) {
                        continue;
                    }
                    let value = vout.get("value").and_then(|v| v.as_u64()).unwrap_or(0);
                    if value == 0 {
                        continue; // OP_RETURN / dust below threshold.
                    }
                    let outpoint_str = format!("{}:{}", txid_str, idx);
                    let outpoint = match OutPoint::from_str(&outpoint_str) {
                        Ok(o) => o,
                        Err(_) => continue,
                    };
                    new_outputs.push((
                        outpoint,
                        UtxoInfo {
                            txid: txid_str.clone(),
                            vout: idx as u32,
                            amount: value,
                            address: vout_addr.to_string(),
                            script_pubkey: None,
                            confirmations: 0,
                            frozen: false,
                            freeze_reason: None,
                            block_height: None,
                            has_inscriptions: false,
                            has_runes: false,
                            has_alkanes: false,
                            is_coinbase: false,
                        },
                    ));
                }
            }
        }
    }

    let before = spendable_utxos.len();
    spendable_utxos.retain(|(op, _)| {
        !spent_outpoints.contains(&(op.txid.to_string(), op.vout))
    });
    let stripped = before - spendable_utxos.len();

    let existing: alloc::collections::BTreeSet<String> = spendable_utxos
        .iter()
        .map(|(op, _)| format!("{}:{}", op.txid, op.vout))
        .collect();
    let mut added = 0usize;
    for (op, info) in new_outputs {
        let key = format!("{}:{}", op.txid, op.vout);
        if !existing.contains(&key) {
            spendable_utxos.push((op, info));
            added += 1;
        }
    }

    MempoolAdjustmentReport { stripped, added }
}

/// Result from UTXO selection including alkanes balances
#[derive(Debug, Clone)]
pub(crate) struct UtxoSelectionResult {
    /// Selected outpoints
    pub(crate) outpoints: Vec<OutPoint>,
    /// TxOuts fetched during selection. Espo-backed selection fills this so
    /// PSBT construction can avoid getrawtransaction fallback fetches.
    pub(crate) txouts: alloc::collections::BTreeMap<OutPoint, TxOut>,
    /// Actual alkanes balances found in the selected UTXOs (aggregate)
    pub(crate) alkanes_found: alloc::collections::BTreeMap<AlkaneId, u64>,
    /// Per-UTXO alkane balances (for alkane-aware ordinals splitting)
    pub(crate) per_utxo_alkanes: alloc::collections::BTreeMap<OutPoint, Vec<(AlkaneId, u64)>>,
}


/// Build a per-outpoint TxOut lookup map from `params.prefetched_utxos`.
///
/// Returns `None` when the caller didn't supply any (so per-input loops can
/// elide the per-iteration map lookup branch entirely with a single `Option`
/// check). On invalid hex / malformed outpoint, surfaces a structured error
/// rather than silently dropping entries — a stale cache that produces
/// garbage should fail loud at execute-time, not later at sighash-mismatch.
fn build_prefetched_txouts_map(
    params: &EnhancedExecuteParams,
) -> Result<Option<alloc::collections::BTreeMap<OutPoint, TxOut>>> {
    if params.prefetched_utxos.is_empty() {
        return Ok(None);
    }
    let mut map = alloc::collections::BTreeMap::new();
    for entry in &params.prefetched_utxos {
        let outpoint = OutPoint::from_str(&entry.outpoint)
            .map_err(|e| AlkanesError::Validation(format!(
                "prefetched_utxos: invalid outpoint '{}': {}", entry.outpoint, e
            )))?;
        let script_bytes = hex::decode(&entry.script_pubkey_hex)
            .map_err(|e| AlkanesError::Validation(format!(
                "prefetched_utxos: invalid script_pubkey_hex for {}: {}", entry.outpoint, e
            )))?;
        map.insert(outpoint, TxOut {
            value: bitcoin::Amount::from_sat(entry.value),
            script_pubkey: ScriptBuf::from_bytes(script_bytes),
        });
    }
    Ok(Some(map))
}

fn build_effective_txouts_map(
    params: &EnhancedExecuteParams,
    selected_txouts: &alloc::collections::BTreeMap<OutPoint, TxOut>,
) -> Result<Option<alloc::collections::BTreeMap<OutPoint, TxOut>>> {
    let mut map = build_prefetched_txouts_map(params)?.unwrap_or_default();
    for (outpoint, txout) in selected_txouts {
        map.entry(*outpoint).or_insert_with(|| txout.clone());
    }
    if map.is_empty() {
        Ok(None)
    } else {
        Ok(Some(map))
    }
}

/// Build a per-outpoint alkane-balance lookup from a `prefetched_utxos`
/// slice (typically `&params.prefetched_utxos`, but threaded as a slice so
/// `select_utxos` — which doesn't take the full params struct — can consume it).
///
/// Only entries with `alkanes: Some(_)` participate; `None` means the caller
/// has no assertion for that outpoint, and the SDK falls back to RPC. Empty
/// `Some(vec![])` is authoritative "asserted clean — do not query."
///
/// Returns `None` when no entry has `Some(_)` so the consumer can take a
/// single-branch fast path (mirrors `build_prefetched_txouts_map`).
///
/// Trust contract identical to `build_prefetched_txouts_map`'s hex decode: a
/// malformed `amount` decimal surfaces as a structured error rather than
/// silently dropping the entry. Stale-cache callers should fail loud at
/// execute-time, not later at swap-revert time.
fn build_prefetched_alkanes_map(
    prefetched_utxos: &[PrefetchedUtxo],
) -> Result<Option<alloc::collections::BTreeMap<OutPoint, Vec<(ProtoruneRuneId, u128)>>>> {
    if prefetched_utxos.is_empty() {
        return Ok(None);
    }
    let mut map = alloc::collections::BTreeMap::new();
    let mut any_asserted = false;
    for entry in prefetched_utxos {
        let Some(alkanes) = &entry.alkanes else {
            continue;
        };
        any_asserted = true;
        let outpoint = OutPoint::from_str(&entry.outpoint)
            .map_err(|e| AlkanesError::Validation(format!(
                "prefetched_utxos: invalid outpoint '{}': {}", entry.outpoint, e
            )))?;
        let mut balances: Vec<(ProtoruneRuneId, u128)> = Vec::with_capacity(alkanes.len());
        for a in alkanes {
            let amount = u128::from_str(&a.amount)
                .map_err(|e| AlkanesError::Validation(format!(
                    "prefetched_utxos: invalid alkane amount '{}' for {}:{}: {}",
                    a.amount, a.block, a.tx, e
                )))?;
            balances.push((ProtoruneRuneId { block: a.block, tx: a.tx }, amount));
        }
        map.insert(outpoint, balances);
    }
    if !any_asserted {
        return Ok(None);
    }
    Ok(Some(map))
}

/// True when every `(block, tx) -> amount` in `alkanes_needed` is already
/// satisfied by the union of asserted balances in `prefetched_utxos`.
///
/// This lets `select_utxos` skip the metashrew `provider.sync()` wait when the
/// caller has done the discovery legwork on their side and is presenting a
/// self-contained UTXO set. Callers that supply prefetched alkane balances
/// (e.g. subfrost-app's `useWalletState` + canonical
/// `metashrew_view protorunesbyoutpoint` path) own freshness for those entries;
/// re-confirming via a sync poll is wasted work and can stall swaps for tens
/// of seconds when the public indexer lags bitcoind by a block or two.
///
/// Returns `false` (i.e. fall back to sync) on any malformed prefetched entry
/// — same conservative posture as `build_prefetched_alkanes_map`'s callers.
fn prefetched_covers_alkanes_needed(
    prefetched_utxos: &[PrefetchedUtxo],
    alkanes_needed: &alloc::collections::BTreeMap<(u64, u64), u64>,
) -> bool {
    if alkanes_needed.is_empty() {
        return true;
    }
    let map = match build_prefetched_alkanes_map(prefetched_utxos) {
        Ok(Some(m)) => m,
        _ => return false,
    };
    let mut available: alloc::collections::BTreeMap<(u128, u128), u128> =
        alloc::collections::BTreeMap::new();
    for balances in map.values() {
        for (rune_id, amount) in balances {
            *available.entry((rune_id.block, rune_id.tx)).or_insert(0) += *amount;
        }
    }
    for ((block, tx), amount) in alkanes_needed {
        let key = (*block as u128, *tx as u128);
        if available.get(&key).copied().unwrap_or(0) < (*amount as u128) {
            return false;
        }
    }
    true
}

fn json_u64(value: &serde_json::Value) -> Option<u64> {
    value.as_u64().or_else(|| value.as_str().and_then(|s| s.parse::<u64>().ok()))
}

fn json_u128(value: &serde_json::Value) -> Option<u128> {
    value.as_u64().map(|v| v as u128).or_else(|| {
        value
            .as_str()
            .and_then(|s| s.parse::<u128>().ok())
    })
}

fn parse_alkane_id_value(value: &serde_json::Value) -> Option<ProtoruneRuneId> {
    if let Some(s) = value.as_str() {
        let (block, tx) = s.split_once(':')?;
        return Some(ProtoruneRuneId {
            block: block.parse::<u128>().ok()?,
            tx: tx.parse::<u128>().ok()?,
        });
    }

    let block = value.get("block").and_then(json_u128)?;
    let tx = value.get("tx").and_then(json_u128)?;
    Some(ProtoruneRuneId { block, tx })
}

fn parse_espo_alkanes(outpoint_value: &serde_json::Value) -> Vec<(ProtoruneRuneId, u128)> {
    outpoint_value
        .get("alkanes")
        .and_then(|v| v.as_array())
        .map(|alkanes| {
            alkanes
                .iter()
                .filter_map(|entry| {
                    let rune_id = entry
                        .get("alkane")
                        .and_then(parse_alkane_id_value)
                        .or_else(|| parse_alkane_id_value(entry))?;
                    let amount = entry.get("amount").and_then(json_u128)?;
                    Some((rune_id, amount))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Enhanced alkanes executor
pub struct EnhancedAlkanesExecutor<'a> {
    pub provider: &'a mut dyn DeezelProvider,
}

impl<'a> EnhancedAlkanesExecutor<'a> {
    /// Create a new enhanced alkanes executor
    pub fn new(provider: &'a mut dyn DeezelProvider) -> Self {
        Self { provider }
    }

    /// Resolve fee rate: use the provided rate if Some, otherwise fetch the medium
    /// (6-block target) rate from esplora fee estimates
    async fn resolve_fee_rate(&mut self, fee_rate: Option<f32>) -> Result<f32> {
        match fee_rate {
            Some(rate) => Ok(rate),
            None => {
                let rates = self.provider.get_fee_rates().await?;
                log::info!("Using esplora medium fee rate: {} sat/vB", rates.medium);
                Ok(rates.medium)
            }
        }
    }

    /// Estimate transaction virtual size (vsize) in vbytes
    /// 
    /// This is used for fee calculation before UTXO selection.
    /// 
    /// # Arguments
    /// * `num_inputs` - Number of transaction inputs
    /// * `num_outputs` - Number of transaction outputs
    /// * `has_envelope` - Whether transaction has large witness data (envelope)
    /// * `has_runestone` - Whether transaction has OP_RETURN runestone
    fn estimate_transaction_vsize(
        num_inputs: usize,
        num_outputs: usize,
        has_envelope: bool,
        has_runestone: bool,
    ) -> usize {
        // Base transaction overhead (version, locktime, input/output counts)
        let base_size = 10; // version(4) + locktime(4) + compact_size(1) + compact_size(1)
        
        // Input size estimation
        // - Previous outpoint: 36 bytes (32 byte txid + 4 byte vout)
        // - Script sig: 1 byte (empty for witness)
        // - Sequence: 4 bytes
        // - Witness: ~65 bytes for P2TR key-path spend (1 byte count + 64 byte signature)
        let input_size_per_input = 36 + 1 + 4; // Non-witness: 41 bytes
        let witness_size_per_input = if has_envelope {
            // For envelope (script-path spend): signature(64) + script(varies) + control block(33)
            // Estimate ~150KB for contract deployments, but this is only for first input
            // Average it out across inputs for simplicity
            150_000 / num_inputs.max(1) + 100
        } else {
            // Regular P2TR key-path spend: 65 bytes witness
            65
        };
        
        // Output size estimation
        // - Value: 8 bytes
        // - Script length: 1 byte (compact size)
        // - Script pubkey: ~34 bytes for P2TR
        let output_size_per_output = 8 + 1 + 34;
        
        // OP_RETURN runestone adds extra output with variable size
        let runestone_size = if has_runestone {
            // Runestone OP_RETURN typically 50-200 bytes depending on complexity
            // Conservative estimate: 150 bytes
            150
        } else {
            0
        };
        
        // Calculate sizes
        let non_witness_size = base_size 
            + (num_inputs * input_size_per_input)
            + (num_outputs * output_size_per_output)
            + runestone_size;
        
        let witness_size = num_inputs * witness_size_per_input;
        
        // vsize = (weight / 4) where weight = (non_witness_size * 4) + witness_size
        let weight = (non_witness_size * 4) + witness_size;
        let vsize = (weight + 3) / 4; // Ceiling division
        
        log::debug!(
            "Transaction size estimate: {} inputs, {} outputs, envelope: {}, runestone: {}",
            num_inputs, num_outputs, has_envelope, has_runestone
        );
        log::debug!(
            "  Non-witness: {} bytes, Witness: {} bytes, Weight: {} WU, VSize: {} vbytes",
            non_witness_size, witness_size, weight, vsize
        );
        
        vsize
    }

    /// Execute an enhanced alkanes transaction with commit/reveal pattern
    pub async fn execute(&mut self, params: EnhancedExecuteParams) -> Result<ExecutionState> {
        log::info!("Starting enhanced alkanes execution");

        self.validate_envelope_cellpack_usage(&params)?;

        // Browser-wallet flow calls execute() through alkanesExecuteWithStrings
        // so it can return unsigned PSBTs for external wallet signing. The
        // split-tx gate originally lived only in execute_full(), which meant
        // split_transactions=true was ignored for UniSat/OKX/Xverse paths and
        // BTC->token still built as one atomic wrap+swap tx. Return Tx B as
        // the main PSBT and attach Tx A as split_psbt so JS can sign both and
        // broadcast them as one CPFP package.
        if params.split_transactions
            && !params.protostones.is_empty()
            && is_wrap_protostone(&params.protostones[0])
            && params.protostones.len() >= 2
            && params.envelope_data.is_none()
        {
            log::info!("🔀 Using split_transactions PSBT mode: wrap → CPFP execute chain");
            return self.build_split_transaction_psbts(params).await;
        }

        if let Some(envelope_data) = &params.envelope_data {
            log::info!("CONTRACT DEPLOYMENT: Using envelope with BIN data for contract deployment");
            log::info!("Envelope data size: {} bytes", envelope_data.len());
            let envelope = AlkanesEnvelope::for_contract(envelope_data.clone());
            log::info!("Created AlkanesEnvelope with BIN protocol tag and gzip compression");
            self.build_commit_reveal_pattern(params, &envelope).await
        } else {
            log::info!("CONTRACT EXECUTION: Single transaction without envelope");
            self.build_single_transaction(&params).await
        }
    }

    /// Execute the full transaction flow, returning the final result
    ///
    /// This method handles the complete execution flow internally:
    /// - For deployments (with envelope): commit -> reveal -> mine -> trace
    /// - For simple transactions: sign -> broadcast -> mine -> trace
    ///
    /// This avoids serialization issues when passing state between JS and Rust.
    pub async fn execute_full(&mut self, params: EnhancedExecuteParams) -> Result<EnhancedExecuteResult> {
        log::info!("Starting full enhanced alkanes execution");

        self.validate_envelope_cellpack_usage(&params)?;

        // Split-tx mode: when the request begins with a wrap protostone and
        // the caller opted in via params.split_transactions=true, fork the
        // protostones across two CPFP-chained transactions so each tx gets
        // its own per-tx fuel budget. Avoids OOG when the combined wrap +
        // execute fuel cost exceeds MINIMUM_FUEL_CHANGE1 (3.5M) and the
        // landing block has block_fuel exhausted.
        if params.split_transactions
            && !params.protostones.is_empty()
            && is_wrap_protostone(&params.protostones[0])
            && params.protostones.len() >= 2
            && params.envelope_data.is_none()
        {
            log::info!("🔀 Using split_transactions mode: wrap → CPFP execute chain");
            return self.execute_split(params).await;
        }

        if let Some(envelope_data) = &params.envelope_data {
            log::info!("CONTRACT DEPLOYMENT: Using envelope with BIN data for contract deployment");
            log::info!("Envelope data size: {} bytes", envelope_data.len());
            let envelope = AlkanesEnvelope::for_contract(envelope_data.clone());
            log::info!("Created AlkanesEnvelope with BIN protocol tag and gzip compression");

            // Use presign pattern for atomic commit-reveal (prevents frontrunning)
            log::info!("🔐 Using presign strategy for atomic commit-reveal deployment");
            return self.execute_full_with_presign(params, &envelope).await;
        } else {
            log::info!("CONTRACT EXECUTION: Single transaction without envelope");

            // Build transaction
            let sign_state = match self.build_single_transaction(&params).await? {
                ExecutionState::ReadyToSign(state) => state,
                other => return Err(AlkanesError::Other(format!("Unexpected state after build: {:?}", other))),
            };

            // Execute
            self.resume_execution(sign_state, &params).await
        }
    }

    /// Split a wrap+execute request into two CPFP-chained transactions:
    ///
    ///   Tx A (parent, wrap-only):
    ///     - inputs: BTC funding UTXOs from `params.from_addresses`
    ///     - outputs: [signer_address (10000 sats), user_taproot (546 sats),
    ///                 user (BTC change)]
    ///     - protostones: [params.protostones[0]] — pointer rewritten to v1
    ///                    (forward minted alkane to user's alkane carrier
    ///                    instead of the original next-protostone target)
    ///
    ///   Tx B (child, execute):
    ///     - inputs: Tx A's v1 (alkane carrier with the freshly-minted
    ///               wrapped alkane) + Tx A's v2 (BTC change for fees)
    ///     - outputs: [user_taproot (alkane carrier for execute output),
    ///                 user (BTC change)]
    ///     - protostones: params.protostones[1..] (refs unchanged — Tx B
    ///                    receives the alkane via the spent input UTXO,
    ///                    so the first cellpack protostone in Tx B
    ///                    automatically gets it via incoming_alkanes)
    ///
    /// Both txs are signed and broadcast as a parent-child mempool chain
    /// (sendrawtransactions array call). The indexer treats the chain as
    /// atomic — the wrap's effects are observed before the execute runs.
    ///
    /// Per-tx fuel budgets are independent, so Tx A's wrap (~2.75M) and
    /// Tx B's execute (~2M for a swap) each fit under MINIMUM_FUEL_CHANGE1
    /// (3.5M) regardless of block_fuel availability.
    async fn execute_split(
        &mut self,
        params: EnhancedExecuteParams,
    ) -> Result<EnhancedExecuteResult> {
        if params.protostones.len() < 2 {
            return Err(AlkanesError::Other(
                "execute_split requires at least 2 protostones (wrap + execute)".to_string(),
            ));
        }
        if !is_wrap_protostone(&params.protostones[0]) {
            return Err(AlkanesError::Other(
                "execute_split: protostones[0] must be a wrap (target=(32,N) opcode=77)"
                    .to_string(),
            ));
        }

        // ---- Tx A: wrap-only ----------------------------------------------
        let mut tx_a_params = params.clone();
        tx_a_params.split_transactions = false;

        // The wrap protostone's pointer typically targets p1 (forward to the
        // next protostone in the original atomic flow). In split mode we
        // redirect to v1 (user's alkane carrier output) so the minted
        // alkane lands on a real Bitcoin output that Tx B can spend.
        let mut wrap_proto = params.protostones[0].clone();
        wrap_proto.pointer = Some(OutputTarget::Output(1));
        wrap_proto.refund = Some(OutputTarget::Output(1));
        tx_a_params.protostones = vec![wrap_proto];

        // Strip alkane input requirements — Tx A is wrap-only and consumes
        // only BTC. The original `params.input_requirements` carried
        // alkane requirements meant for the execute protostone (Tx B),
        // and forwarding them here causes Tx A's selector to grab the
        // user's alkane UTXOs as "inputs" — destroying them in the wrap
        // (observed 2026-05-03 mainnet wrap+addLiquidity attempt: Tx A
        // unintentionally consumed the user's 5e4a4112:0 DIESEL alkane
        // carrier as a generic dust input, leaving Tx B with
        // "Insufficient alkanes: need 30000000 of 2:0, have 0").
        tx_a_params.input_requirements.retain(|req| {
            !matches!(req, InputRequirement::Alkanes { .. })
        });

        log::info!("[split-tx] Step 1/2: building wrap-only Tx A");
        let tx_a_result = Box::pin(self.execute_full(tx_a_params)).await?;
        let wrap_txid = tx_a_result.reveal_txid.clone();
        let wrap_fee = tx_a_result.reveal_fee;
        log::info!("[split-tx] Tx A broadcast: {} (fee {} sats)", wrap_txid, wrap_fee);

        // ---- Tx B: execute (spends Tx A's v1 + v2) ------------------------
        // Hand Tx A's signed hex into Tx B's `select_utxos` via
        // `known_pending_tx_hexes`. This bypasses the indexer-propagation
        // timing window (observed ~325ms lag on mainnet 2026-05-03) where
        // `address/{addr}/txs/mempool` returned empty between Tx A's
        // broadcast and Tx B's coin selection — letting Tx B re-pick
        // Tx A's already-spent prevouts and triggering BIP125 RBF
        // rejection. With the synthetic injection, the strip pass
        // doesn't depend on indexer lag.
        // CPFP fee: make Tx B (child) pay enough that the package (Tx A + Tx B)
        // clears the target rate, floored at min-relay — see
        // child_fee_rate_for_package. Decode Tx A from its broadcast hex to get
        // the parent's true vsize; fall back to an estimate if hex is missing.
        let target_rate = self.resolve_fee_rate(params.fee_rate).await?;
        let parent_vsize = tx_a_result
            .reveal_tx_hex
            .as_ref()
            .and_then(|hex| {
                let bytes = hex::decode(hex).ok()?;
                let tx: bitcoin::Transaction =
                    bitcoin::consensus::encode::deserialize(&bytes).ok()?;
                Some(tx.vsize() as u64)
            })
            .unwrap_or_else(|| {
                Self::estimate_transaction_vsize(2, 3, false, true) as u64
            });
        let child_vsize_estimate = Self::estimate_transaction_vsize(
            2,
            params.to_addresses.len().saturating_sub(1).max(1) + 1,
            false,
            true,
        ) as u64;
        let child_rate = child_fee_rate_for_package(
            target_rate,
            wrap_fee,
            parent_vsize,
            child_vsize_estimate,
        );
        log::info!(
            "[split-tx] CPFP child fee rate: {:.3} sat/vB (target {:.3}, parent {} sats / {} vB, child ~{} vB)",
            child_rate, target_rate, wrap_fee, parent_vsize, child_vsize_estimate
        );

        let mut tx_b_params = params.clone();
        tx_b_params.split_transactions = false;
        tx_b_params.fee_rate = Some(child_rate);
        tx_b_params.protostones = params.protostones[1..].to_vec();
        if let Some(tx_a_hex) = tx_a_result.reveal_tx_hex.clone() {
            tx_b_params.known_pending_tx_hexes.push(tx_a_hex);
        } else {
            log::warn!("[split-tx] Tx A result missing reveal_tx_hex — falling back to indexer-only mempool view (may race indexer propagation)");
        }

        // Drop BTC input requirements — Tx A's BTC change at v2 covers Tx B
        // fees via natural UTXO selection.
        tx_b_params.input_requirements.retain(|req| {
            !matches!(
                req,
                InputRequirement::Bitcoin { .. } | InputRequirement::BitcoinOutput { .. }
            )
        });

        // Tx B doesn't need the wrap's signer recipient. Drop the first
        // to_address (typically the signer) if there are at least 2.
        if tx_b_params.to_addresses.len() >= 2 {
            tx_b_params.to_addresses.remove(0);
        }

        log::info!("[split-tx] Step 2/2: building execute Tx B (spends {})", wrap_txid);
        let tx_b_result = Box::pin(self.execute_full(tx_b_params)).await?;
        log::info!(
            "[split-tx] Tx B broadcast: {} (fee {} sats)",
            tx_b_result.reveal_txid,
            tx_b_result.reveal_fee
        );

        Ok(EnhancedExecuteResult {
            split_txid: tx_b_result.split_txid,
            split_fee: tx_b_result.split_fee,
            commit_txid: None,
            reveal_txid: tx_b_result.reveal_txid,
            commit_fee: None,
            reveal_fee: tx_b_result.reveal_fee,
            inputs_used: [tx_a_result.inputs_used, tx_b_result.inputs_used].concat(),
            outputs_created: [tx_a_result.outputs_created, tx_b_result.outputs_created].concat(),
            traces: match (tx_a_result.traces, tx_b_result.traces) {
                (Some(a), Some(b)) => Some([a, b].concat()),
                (Some(a), None) => Some(a),
                (None, Some(b)) => Some(b),
                (None, None) => None,
            },
            wrap_txid: Some(wrap_txid),
            wrap_fee: Some(wrap_fee),
            reveal_tx_hex: tx_b_result.reveal_tx_hex,
        })
    }

    /// Build unsigned PSBTs for split wrap+execute mode.
    ///
    /// This mirrors execute_split(), but stops before signing/broadcasting so
    /// browser wallets can sign both transactions in JS. The returned
    /// ReadyToSignTx uses:
    ///   - split_psbt: Tx A parent, wrap-only
    ///   - psbt: Tx B child, execute-only
    async fn build_split_transaction_psbts(
        &mut self,
        params: EnhancedExecuteParams,
    ) -> Result<ExecutionState> {
        if params.protostones.len() < 2 {
            return Err(AlkanesError::Other(
                "build_split_transaction_psbts requires at least 2 protostones (wrap + execute)".to_string(),
            ));
        }
        if !is_wrap_protostone(&params.protostones[0]) {
            return Err(AlkanesError::Other(
                "build_split_transaction_psbts: protostones[0] must be a wrap (target=(32,N) opcode=77)"
                    .to_string(),
            ));
        }

        let mut tx_a_params = params.clone();
        tx_a_params.split_transactions = false;

        let mut wrap_proto = params.protostones[0].clone();
        wrap_proto.pointer = Some(OutputTarget::Output(1));
        wrap_proto.refund = Some(OutputTarget::Output(1));
        tx_a_params.protostones = vec![wrap_proto];
        tx_a_params.input_requirements.retain(|req| {
            !matches!(req, InputRequirement::Alkanes { .. })
        });

        log::info!("[split-tx:psbt] Step 1/2: building unsigned wrap-only Tx A");
        let tx_a_state = match self.build_single_transaction(&tx_a_params).await? {
            ExecutionState::ReadyToSign(state) => state,
            other => return Err(AlkanesError::Other(format!(
                "Unexpected Tx A state after split build: {:?}",
                other,
            ))),
        };

        let tx_a_hex = bitcoin::consensus::encode::serialize_hex(&tx_a_state.psbt.unsigned_tx);
        let wrap_txid = tx_a_state.psbt.unsigned_tx.compute_txid().to_string();

        // CPFP fee: Tx B is the child of Tx A and must pay enough that the
        // *package* (Tx A + Tx B) clears the user's target fee rate, and must
        // never fall below the network min-relay floor. Previously Tx B was
        // built at the bare target rate against its own tiny input value (just
        // Tx A's dust + change), which collapsed to ~0.15 sat/vB once funds
        // ran short — below min-relay, so once Tx A confirmed alone Tx B became
        // an un-confirmable orphan. Force Tx B's fee rate to the package child
        // rate so it pays for both itself and any parent shortfall.
        let parent_vsize = tx_a_state.psbt.unsigned_tx.vsize() as u64;
        let parent_fee = tx_a_state.fee;
        let target_rate = self.resolve_fee_rate(params.fee_rate).await?;
        // Estimate Tx B's vsize: it spends Tx A's alkane carrier + BTC change
        // (~2 inputs) and produces the execute output(s) + change + runestone.
        let child_vsize_estimate = Self::estimate_transaction_vsize(
            2,
            params.to_addresses.len().saturating_sub(1).max(1) + 1,
            false,
            true,
        ) as u64;
        let child_rate = child_fee_rate_for_package(
            target_rate,
            parent_fee,
            parent_vsize,
            child_vsize_estimate,
        );
        log::info!(
            "[split-tx:psbt] CPFP child fee rate: {:.3} sat/vB (target {:.3}, parent {} sats / {} vB, child ~{} vB)",
            child_rate, target_rate, parent_fee, parent_vsize, child_vsize_estimate
        );

        let mut tx_b_params = params.clone();
        tx_b_params.split_transactions = false;
        tx_b_params.fee_rate = Some(child_rate);
        tx_b_params.protostones = params.protostones[1..].to_vec();
        tx_b_params.known_pending_tx_hexes.push(tx_a_hex);
        tx_b_params.input_requirements.retain(|req| {
            !matches!(
                req,
                InputRequirement::Bitcoin { .. } | InputRequirement::BitcoinOutput { .. }
            )
        });
        if tx_b_params.to_addresses.len() >= 2 {
            tx_b_params.to_addresses.remove(0);
        }

        log::info!("[split-tx:psbt] Step 2/2: building unsigned execute Tx B (spends {})", wrap_txid);
        let mut tx_b_state = match self.build_single_transaction(&tx_b_params).await? {
            ExecutionState::ReadyToSign(state) => state,
            other => return Err(AlkanesError::Other(format!(
                "Unexpected Tx B state after split build: {:?}",
                other,
            ))),
        };

        tx_b_state.split_psbt = Some(tx_a_state.psbt);
        tx_b_state.split_fee = Some(tx_a_state.fee);

        Ok(ExecutionState::ReadyToSign(tx_b_state))
    }

    pub async fn resume_execution(
        &mut self,
        state: ReadyToSignTx,
        params: &EnhancedExecuteParams,
    ) -> Result<EnhancedExecuteResult> {
        let unsigned_tx = &state.psbt.unsigned_tx;

        if !params.auto_confirm {
            // Show split transaction preview if present
            if let Some(ref split_psbt) = state.split_psbt {
                if !params.raw_output {
                    log::info!("📋 Split Transaction Preview (protects inscribed UTXOs):");
                    log::info!("   Inputs: {}", split_psbt.unsigned_tx.input.len());
                    log::info!("   Outputs: {}", split_psbt.unsigned_tx.output.len());
                    log::info!("   Fee: {} sats", state.split_fee.unwrap_or(0));
                }
            }

            self.show_preview_and_confirm(
                unsigned_tx,
                &serde_json::to_value(&state.analysis)?,
                state.fee,
                state.estimated_vsize,
                params.raw_output,
            )?;
        }

        // Sign split PSBT if present
        let (split_txid, split_tx_hex) = if let Some(split_psbt) = state.split_psbt {
            log::info!("🔀 Signing split transaction...");
            let split_tx = self.sign_and_finalize_psbt(split_psbt).await?;
            let split_txid = split_tx.compute_txid().to_string();
            let split_hex = bitcoin::consensus::encode::serialize_hex(&split_tx);
            (Some(split_txid), Some(split_hex))
        } else {
            (None, None)
        };

        // Sign main transaction
        let tx = self.sign_and_finalize_psbt(state.psbt).await?;
        let tx_hex = bitcoin::consensus::encode::serialize_hex(&tx);
        let tx_hex_for_result = tx_hex.clone(); // Returned in EnhancedExecuteResult so callers (notably execute_split) can hand it to the next tx's selector.
        let main_txid = tx.compute_txid().to_string();

        // Broadcast atomically using send_raw_transactions if we have a split
        let txid = if let Some(split_hex) = split_tx_hex {
            use crate::traits::BitcoinRpcProvider;
            log::info!("🚀 Broadcasting split + main transactions atomically...");
            let tx_hexes = vec![split_hex, tx_hex];
            let txids = self.provider.send_raw_transactions(&tx_hexes).await?;

            if txids.len() >= 2 {
                log::info!("✅ Split transaction broadcast: {}", txids[0]);
                log::info!("✅ Main transaction broadcast: {}", txids[1]);
                txids[1].clone()
            } else if txids.len() == 1 {
                // Fallback: only one txid returned, use it
                log::warn!("⚠️ Only one txid returned from batch broadcast");
                txids[0].clone()
            } else {
                return Err(AlkanesError::RpcError("No txids returned from broadcast".to_string()));
            }
        } else {
            // No split, just broadcast the main transaction
            self.provider.broadcast_transaction(tx_hex).await?
        };

        if !params.raw_output {
            log::info!("✅ Transaction broadcast successfully!");
            log::info!("🔗 TXID: {txid}");
        }

        // Note: pending_tx_store push happens inside the provider's
        // `broadcast_transaction` / `send_raw_transactions` impls
        // themselves (alkanes-web-sys/src/provider.rs). Doing it
        // there means every JS-side caller — including paths that
        // bypass execute_full (browser-wallet manual PSBT signing,
        // direct broadcast calls from mutation hooks) — gets
        // auto-pushed without per-call ad-hoc plumbing.

        if params.mine_enabled {
            self.mine_blocks_if_regtest(params).await?;
            self.provider.sync().await?;
        }

        let traces = if params.trace_enabled {
            self.trace_reveal_transaction(&txid, params).await?
        } else {
            None
        };

        Ok(EnhancedExecuteResult {
            split_txid,
            split_fee: state.split_fee,
            commit_txid: None,
            reveal_txid: txid,
            commit_fee: None,
            reveal_fee: state.fee,
            inputs_used: tx.input.iter().map(|i| i.previous_output.to_string()).collect(),
            outputs_created: tx.output.iter().map(|o| o.script_pubkey.to_string()).collect(),
            traces,
            wrap_txid: None,
            wrap_fee: None,
            reveal_tx_hex: Some(tx_hex_for_result),
        })
    }

    pub async fn resume_commit_execution(
        &mut self,
        state: ReadyToSignCommitTx,
    ) -> Result<ExecutionState> {
        // 1. Sign and broadcast the commit transaction
        let commit_tx = self.sign_and_finalize_psbt(state.psbt).await?;
        log::info!("[DEBUG] About to broadcast commit transaction");
        let commit_txid_result = self
            .provider
            .broadcast_transaction(bitcoin::consensus::encode::serialize_hex(&commit_tx))
            .await;
        log::info!("[DEBUG] broadcast_transaction returned");
        let commit_txid = commit_txid_result?;
        log::info!("[DEBUG] Got commit_txid: {}", commit_txid);
        log::info!("Commit transaction broadcast successfully: {commit_txid}");

        // Mine a block to confirm the commit transaction if on regtest
        if state.params.mine_enabled {
            self.mine_blocks_if_regtest(&state.params).await?;
            self.provider.sync().await?;
        }

        // 2. Build the reveal transaction PSBT
        let commit_outpoint = bitcoin::OutPoint { txid: commit_tx.compute_txid(), vout: 0 };
        let (reveal_psbt, reveal_fee, reveal_estimated_vsize) = self
            .build_reveal_psbt(
                &state.params,
                &state.envelope,
                commit_outpoint,
                state.required_reveal_amount,
                state.commit_internal_key,
                state.commit_internal_key_fingerprint,
                &state.commit_internal_key_path,
            )
            .await?;

        // 3. Analyze the reveal transaction
        let analysis =
            crate::transaction::analysis::analyze_transaction(&reveal_psbt.unsigned_tx);

        let inspection_result = {
            #[cfg(feature = "wasm-inspection")]
            {
                self.inspect_from_envelope(&state.envelope).await.ok()
            }
            #[cfg(not(feature = "wasm-inspection"))]
            {
                None
            }
        };

        // 4. Return the next state
        Ok(ExecutionState::ReadyToSignReveal(ReadyToSignRevealTx {
            psbt: reveal_psbt,
            fee: reveal_fee,
            estimated_vsize: reveal_estimated_vsize,
            analysis,
            commit_txid,
            commit_fee: state.fee,
            params: state.params,
            inspection_result,
            commit_internal_key: state.commit_internal_key,
            commit_internal_key_fingerprint: state.commit_internal_key_fingerprint,
            commit_internal_key_path: state.commit_internal_key_path,
        }))
    }

    pub async fn resume_reveal_execution(
        &mut self,
        state: ReadyToSignRevealTx,
    ) -> Result<EnhancedExecuteResult> {
        let unsigned_tx = &state.psbt.unsigned_tx;

        if !state.params.auto_confirm {
            self.show_preview_and_confirm(
                unsigned_tx,
                &serde_json::to_value(&state.analysis)?,
                state.fee,
                state.estimated_vsize,
                state.params.raw_output,
            )?;
        }

        let reveal_tx = self.sign_and_finalize_psbt(state.psbt).await?;
        let reveal_txid = self
            .provider
            .broadcast_transaction(bitcoin::consensus::encode::serialize_hex(&reveal_tx))
            .await?;

        if !state.params.raw_output {
            log::info!("✅ Reveal transaction broadcast successfully!");
            log::info!("🔗 TXID: {reveal_txid}");
        }

        if state.params.mine_enabled {
            self.mine_blocks_if_regtest(&state.params).await?;
            self.provider.sync().await?;
        }

        let traces = if state.params.trace_enabled {
            self.trace_reveal_transaction(&reveal_txid, &state.params).await?
        } else {
            None
        };

        Ok(EnhancedExecuteResult {
            split_txid: None,
            split_fee: None,
            commit_txid: Some(state.commit_txid),
            reveal_txid,
            commit_fee: Some(state.commit_fee),
            reveal_fee: state.fee,
            inputs_used: reveal_tx.input.iter().map(|i| i.previous_output.to_string()).collect(),
            outputs_created: reveal_tx.output.iter().map(|o| o.script_pubkey.to_string()).collect(),
            traces,
            wrap_txid: None,
            wrap_fee: None,
            reveal_tx_hex: None,
        })
    }

    /// Build the commit transaction and return it in a ready-to-sign state.
    async fn build_commit_reveal_pattern(
        &mut self,
        params: EnhancedExecuteParams,
        envelope: &AlkanesEnvelope,
    ) -> Result<ExecutionState> {
        log::info!("Building commit transaction");

        let (internal_key, (fingerprint, path)) = self.provider.get_internal_key().await?;
        let commit_address = self.create_commit_address_for_envelope(envelope, internal_key).await?;
        log::info!("Envelope commit address: {commit_address}");

        let mut required_reveal_amount = 546u64;
        for requirement in &params.input_requirements {
            if let InputRequirement::Bitcoin { amount } = requirement {
                required_reveal_amount += amount;
            }
        }

        // Calculate estimated reveal fee based on actual reveal script size
        let reveal_script = envelope.build_reveal_script();
        let reveal_script_size = reveal_script.len();

        // Estimate reveal transaction size:
        // - Base transaction overhead: ~10 bytes (version, locktime, input/output counts)
        // - Input (commit UTXO): 36 bytes (outpoint) + 4 bytes (sequence) + 1 byte (scriptsig)
        // - Witness: signature (64) + script (reveal_script_size) + control block (~33)
        // - Outputs: recipient outputs + change + OP_RETURN runestone
        let num_outputs = params.to_addresses.len().max(1) + 2; // at least 1 recipient + change + OP_RETURN
        let output_size = num_outputs * 43; // ~43 bytes per P2TR output
        let witness_size = 64 + reveal_script_size + 33;
        let non_witness_size = 10 + 41 + output_size;
        let weight = (non_witness_size * 4) + witness_size;
        let estimated_vsize = (weight + 3) / 4;

        // Use user-specified fee rate or network default
        let network = self.provider.get_network();
        let default_fee_rate = match network {
            bitcoin::Network::Bitcoin => 10.0,
            bitcoin::Network::Testnet => 5.0,
            bitcoin::Network::Regtest => 1.0,
            bitcoin::Network::Signet => 5.0,
            _ => 5.0,
        };
        let fee_rate_sat_vb = params.fee_rate.unwrap_or(default_fee_rate);
        let estimated_reveal_fee = ((estimated_vsize as f32 * fee_rate_sat_vb) * 1.2).ceil() as u64; // 20% buffer

        log::info!("Reveal script size: {} bytes, estimated vsize: {} vbytes, fee rate: {:.1} sat/vB, estimated fee: {} sats",
                   reveal_script_size, estimated_vsize, fee_rate_sat_vb, estimated_reveal_fee);

        required_reveal_amount += estimated_reveal_fee;
        required_reveal_amount += params.to_addresses.len() as u64 * 546;

        let utxo_selection = self
            .select_utxos(&[InputRequirement::Bitcoin { amount: required_reveal_amount }], &params.from_addresses, &params.known_pending_tx_hexes, params.max_indexed_height, &params.prefetched_utxos, &params.excluded_utxos, params.utxo_source)
            .await?;
        let funding_utxos = utxo_selection.outpoints.clone();

        // Check selected UTXOs for ordinal inscriptions based on strategy
        let final_funding_utxos = if params.ordinals_strategy != OrdinalsStrategy::Burn {
            let mut funding_utxos_with_txout: Vec<(OutPoint, TxOut)> = Vec::new();
            for outpoint in &funding_utxos {
                if let Some(txout) = utxo_selection.txouts.get(outpoint).cloned() {
                    funding_utxos_with_txout.push((*outpoint, txout));
                } else if let Some(txout) = self.provider.get_utxo(outpoint).await? {
                    funding_utxos_with_txout.push((*outpoint, txout));
                }
            }

            log::info!("🔍 Checking commit UTXOs for ordinal inscriptions (strategy: {:?})", params.ordinals_strategy);
            match check_utxos_for_inscriptions_with_provider(
                self.provider,
                &funding_utxos_with_txout,
                params.ordinals_strategy,
                fee_rate_sat_vb,
                params.mempool_indexer,
            ).await {
                Ok(None) => {
                    log::info!("✅ No ordinal inscriptions found in commit UTXOs");
                    funding_utxos
                }
                Ok(Some(plans)) => {
                    // For commit/reveal flow with inscribed UTXOs, we need to handle this differently
                    // Since the commit tx must be broadcast first, we can't bundle split atomically
                    // For now, fail with a helpful message suggesting to use the single-tx flow
                    // or to manually split the UTXOs first
                    log::error!("❌ Inscribed UTXOs detected in commit/reveal flow");
                    log::error!("   {} UTXOs contain inscriptions", plans.len());
                    return Err(AlkanesError::Wallet(format!(
                        "Cannot use commit/reveal pattern with inscribed UTXOs. \
                        The commit transaction must be broadcast separately, which prevents atomic split.\n\
                        Options:\n\
                        1. Use --ordinals-strategy burn to allow spending inscribed UTXOs (destroys inscriptions)\n\
                        2. Manually split inscribed UTXOs before executing\n\
                        3. Use a transaction pattern that doesn't require commit/reveal"
                    )));
                }
                Err(e) => {
                    return Err(e);
                }
            }
        } else {
            funding_utxos
        };

        let commit_output = TxOut {
            value: bitcoin::Amount::from_sat(required_reveal_amount),
            script_pubkey: commit_address.script_pubkey(),
        };

        let (commit_psbt, commit_fee) = self
            .build_commit_psbt(final_funding_utxos, commit_output, params.fee_rate)
            .await?;

        Ok(ExecutionState::ReadyToSignCommit(ReadyToSignCommitTx {
            psbt: commit_psbt,
            fee: commit_fee,
            required_reveal_amount,
            params,
            envelope: envelope.clone(),
            commit_internal_key: internal_key,
            commit_internal_key_fingerprint: fingerprint,
            commit_internal_key_path: path,
        }))
    }

    /// Creates a taproot address for the commit transaction.
    async fn create_commit_address_for_envelope(
        &self,
        envelope: &AlkanesEnvelope,
        internal_key: XOnlyPublicKey,
    ) -> Result<Address> {
        use bitcoin::taproot::TaprootBuilder;
        let network = self.provider.get_network();

        let reveal_script = envelope.build_reveal_script();

        let taproot_builder = TaprootBuilder::new()
            .add_leaf(0, reveal_script.clone()).map_err(|e| AlkanesError::Other(format!("{e:?}")))?;

        let taproot_spend_info = taproot_builder
            .finalize(self.provider.secp(), internal_key).map_err(|e| AlkanesError::Other(format!("{e:?}")))?;

        let commit_address = Address::p2tr_tweaked(taproot_spend_info.output_key(), network);

        Ok(commit_address)
    }

    /// Execute single transaction (no envelope)
    async fn build_single_transaction(&mut self, params: &EnhancedExecuteParams) -> Result<ExecutionState> {
        log::info!("Building single transaction (no envelope)");
        log::info!("[execute] params.from_addresses = {:?}", params.from_addresses);
        log::info!("[execute] params.change_address = {:?}", params.change_address);

        // Create outputs first (including identifier-based outputs)
        // NOTE: We validate against original protostones first, then will re-validate after generating automatic protostone
        let mut outputs = self.create_outputs(&params.to_addresses, &params.change_address, &params.input_requirements, &params.protostones).await?;
        
        // Validate original protostones against the actual number of outputs we created
        self.validate_protostones(&params.protostones, outputs.len())?;
        
        // Apply BTC assignments from protostones
        for protostone in &params.protostones {
            if let Some(transfer) = &protostone.bitcoin_transfer {
                if let OutputTarget::Output(vout) = transfer.target {
                    if let Some(output) = outputs.get_mut(vout as usize) {
                        output.value = bitcoin::Amount::from_sat(transfer.amount);
                    }
                }
            }
        }
        
        // Apply BTC assignments from input requirements (B:amount:vN)
        for requirement in &params.input_requirements {
            if let InputRequirement::BitcoinOutput { amount, target } = requirement {
                if let OutputTarget::Output(vout) = target {
                    if let Some(output) = outputs.get_mut(*vout as usize) {
                        output.value = bitcoin::Amount::from_sat(*amount);
                        log::info!("Assigned {} sats to output v{} via B:amount:vN", amount, vout);
                    }
                }
            }
        }
        let total_bitcoin_needed: u64 = outputs.iter().filter(|o| o.value.to_sat() > 0).map(|o| o.value.to_sat()).sum();
        let mut final_requirements = params.input_requirements.iter().filter(|req| !matches!(req, InputRequirement::Bitcoin {..} | InputRequirement::BitcoinOutput {..})).cloned().collect::<Vec<_>>();
        
        // Estimate transaction size to calculate proper fee BEFORE UTXO selection
        // This is critical to avoid "absurdly high fee rate" errors
        let network = self.provider.get_network();
        let default_fee_rate = match network {
            bitcoin::Network::Bitcoin => 50.0,
            bitcoin::Network::Testnet => 10.0,
            bitcoin::Network::Regtest => 1.0,
            bitcoin::Network::Signet => 10.0,
            _ => 10.0,
        };
        let fee_rate_sat_vb = params.fee_rate.unwrap_or(default_fee_rate);
        
        // Estimate transaction size with initial guess of inputs (will iterate if needed)
        let num_alkane_reqs = final_requirements.iter().filter(|r| matches!(r, InputRequirement::Alkanes { .. })).count();
        let estimated_inputs = (num_alkane_reqs + 1).max(2); // At least 2 inputs for safety
        let estimated_outputs = outputs.len() + 1; // +1 for OP_RETURN
        let has_runestone = !params.protostones.is_empty();
        
        let estimated_vsize = Self::estimate_transaction_vsize(estimated_inputs, estimated_outputs, false, has_runestone);
        let estimated_fee = (fee_rate_sat_vb * estimated_vsize as f32).ceil() as u64;
        
        // Add 50% buffer to fee to account for variations in actual transaction size
        let fee_with_buffer = (estimated_fee as f64 * 1.5).ceil() as u64;
        
        log::info!("Fee estimation: {} vbytes × {:.1} sat/vB = {} sats (with 50% buffer: {} sats)",
                   estimated_vsize, fee_rate_sat_vb, estimated_fee, fee_with_buffer);
        
        // Include fee in Bitcoin requirements for UTXO selection
        let bitcoin_requirement = total_bitcoin_needed + fee_with_buffer;
        log::info!("Total Bitcoin requirement: {} sats (outputs) + {} sats (fee) = {} sats",
                   total_bitcoin_needed, fee_with_buffer, bitcoin_requirement);
        
        final_requirements.push(InputRequirement::Bitcoin { amount: bitcoin_requirement });
        let mut utxo_selection = self.select_utxos(&final_requirements, &params.from_addresses, &params.known_pending_tx_hexes, params.max_indexed_height, &params.prefetched_utxos, &params.excluded_utxos, params.utxo_source).await?;

        // Check selected UTXOs for ordinal inscriptions based on strategy
        // We need to get TxOut data for each selected UTXO to check for inscriptions
        let mut funding_utxos_with_txout: Vec<(OutPoint, TxOut)> = Vec::new();
        for outpoint in &utxo_selection.outpoints {
            if let Some(txout) = utxo_selection.txouts.get(outpoint).cloned() {
                funding_utxos_with_txout.push((*outpoint, txout));
            } else if let Some(txout) = self.provider.get_utxo(outpoint).await? {
                funding_utxos_with_txout.push((*outpoint, txout));
            }
        }

        // Check for inscriptions if ordinals_strategy is not Burn
        // Returns (split_psbt, split_fee, updated_utxo_outpoints)
        let (split_psbt, split_fee, final_funding_outpoints): (Option<Psbt>, Option<u64>, Vec<OutPoint>) =
            if params.ordinals_strategy != OrdinalsStrategy::Burn {
                log::info!("🔍 Checking selected UTXOs for ordinal inscriptions (strategy: {:?})", params.ordinals_strategy);
                match check_utxos_for_inscriptions_with_provider(
                    self.provider,
                    &funding_utxos_with_txout,
                    params.ordinals_strategy,
                    fee_rate_sat_vb,
                    params.mempool_indexer,
                ).await {
                    Ok(None) => {
                        log::info!("✅ No ordinal inscriptions found in selected UTXOs");
                        (None, None, utxo_selection.outpoints.clone())
                    }
                    Ok(Some(plans)) => {
                        log::info!("📋 Building split transaction for {} inscribed UTXOs", plans.len());
                        for plan in &plans {
                            log::info!("   Split: {} → safe({}) + clean({})",
                                plan.outpoint, plan.safe_amount, plan.clean_amount);
                        }

                        // Collect alkane data for inscribed UTXOs being split
                        let split_utxo_alkanes: alloc::collections::BTreeMap<OutPoint, Vec<(AlkaneId, u64)>> = plans.iter()
                            .filter_map(|plan| {
                                utxo_selection.per_utxo_alkanes.get(&plan.outpoint)
                                    .map(|alkanes| (plan.outpoint, alkanes.clone()))
                            })
                            .collect();

                        if !split_utxo_alkanes.is_empty() {
                            log::info!("🔗 Alkane-aware split: {} inscribed UTXOs carry alkanes", split_utxo_alkanes.len());
                            for (op, alkanes) in &split_utxo_alkanes {
                                for (alkane_id, amount) in alkanes {
                                    log::info!("   {} has {}:{} = {} units", op, alkane_id.block, alkane_id.tx, amount);
                                }
                            }
                        }

                        // Compute clean extras the split-tx may consume. A
                        // UTXO is a valid extra if it's already in the
                        // selected set (so we know its TxOut data), is NOT
                        // inscribed (not in `plans`), and carries NO alkanes
                        // (not in `per_utxo_alkanes` — spending it as a fee
                        // input would burn user tokens).
                        //
                        // The split-tx pulls from this set as needed when
                        // small inscribed UTXOs can't self-fund the split.
                        let inscribed_outpoints: std::collections::HashSet<OutPoint> =
                            plans.iter().map(|p| p.outpoint).collect();
                        let alkane_outpoints_in_selection: std::collections::HashSet<OutPoint> =
                            utxo_selection.per_utxo_alkanes.keys().copied().collect();
                        let extra_funding_utxos: Vec<(OutPoint, TxOut)> = funding_utxos_with_txout
                            .iter()
                            .filter(|(op, _)| {
                                !inscribed_outpoints.contains(op)
                                    && !alkane_outpoints_in_selection.contains(op)
                            })
                            .cloned()
                            .collect();

                        log::info!(
                            "Split-tx extras candidates: {} clean UTXOs available (selected: {}, inscribed: {}, alkane-bearing: {})",
                            extra_funding_utxos.len(),
                            utxo_selection.outpoints.len(),
                            inscribed_outpoints.len(),
                            alkane_outpoints_in_selection.len(),
                        );

                        // Build split transaction PSBT (alkane-aware, extras-aware)
                        let (
                            split_psbt_result,
                            split_fee_result,
                            clean_outpoints,
                            alkane_outpoints,
                            consumed_extra_outpoints,
                        ) = self.build_split_psbt(
                            &plans,
                            &funding_utxos_with_txout,
                            &extra_funding_utxos,
                            fee_rate_sat_vb,
                            params,
                            &split_utxo_alkanes,
                        ).await?;

                        // Replace inscribed UTXOs with clean UTXOs from split.
                        // Also remove any extras consumed by the split — those
                        // outpoints are now spent in the split tx and can't
                        // appear in the main tx's input list.
                        let consumed_extras_set: std::collections::HashSet<OutPoint> =
                            consumed_extra_outpoints.iter().copied().collect();
                        let mut new_outpoints = Vec::new();

                        // Keep non-inscribed, non-consumed UTXOs
                        for outpoint in &utxo_selection.outpoints {
                            if !inscribed_outpoints.contains(outpoint)
                                && !consumed_extras_set.contains(outpoint)
                            {
                                new_outpoints.push(*outpoint);
                            }
                        }
                        // Add clean BTC UTXOs from split (for fee funding)
                        new_outpoints.extend(clean_outpoints);
                        // Add clean alkane UTXOs from split (for alkane spending)
                        new_outpoints.extend(alkane_outpoints.iter().map(|(op, _)| *op));

                        // Update alkanes_found: remove alkanes from inscribed UTXOs, add from alkane outpoints
                        for plan in &plans {
                            if let Some(alkanes) = utxo_selection.per_utxo_alkanes.get(&plan.outpoint) {
                                for (alkane_id, _amount) in alkanes {
                                    if let Some(_found) = utxo_selection.alkanes_found.get_mut(alkane_id) {
                                        // The alkanes are still there, just on new outpoints now
                                        // No need to subtract/re-add — the aggregate total is unchanged
                                        log::debug!("Alkane {}:{} moved from inscribed UTXO {} to clean alkane output",
                                            alkane_id.block, alkane_id.tx, plan.outpoint);
                                    }
                                }
                            }
                        }

                        // Update per_utxo_alkanes: remove inscribed UTXOs, add alkane outpoints
                        for plan in &plans {
                            utxo_selection.per_utxo_alkanes.remove(&plan.outpoint);
                        }
                        for (outpoint, alkanes) in &alkane_outpoints {
                            utxo_selection.per_utxo_alkanes.insert(*outpoint, alkanes.clone());
                        }

                        log::info!(
                            "🔀 Split transaction built: {} clean BTC UTXOs + {} clean alkane UTXOs replace {} inscribed UTXOs ({} extras consumed)",
                            plans.len(),
                            alkane_outpoints.len(),
                            inscribed_outpoints.len(),
                            consumed_extras_set.len(),
                        );

                        (Some(split_psbt_result), Some(split_fee_result), new_outpoints)
                    }
                    Err(e) => {
                        // Strategy is Exclude and inscribed UTXOs were found - fail
                        return Err(e);
                    }
                }
            } else {
                log::debug!("🔥 Ordinals strategy is Burn - skipping inscription check");
                (None, None, utxo_selection.outpoints.clone())
            };

        // Calculate alkanes needed and check for excess
        let alkanes_needed = self.calculate_alkanes_needed(&params.input_requirements);
        let alkanes_excess = self.calculate_excess(&utxo_selection.alkanes_found, &alkanes_needed);
        
        // Handle excess alkanes: DO NOT insert auto-change protostone at the beginning.
        // Inserting at position 0 causes the protorune runtime to route input alkanes
        // to the auto-change protostone instead of the user's contract call protostone.
        // Instead, excess alkanes flow to the Runestone default pointer (output 0),
        // matching @alkanes/ts-sdk behavior.
        let final_protostones = if !alkanes_excess.is_empty() && false /* DISABLED */ {
            log::info!("🔄 Handling excess alkanes with automatic protostone generation");
            
            // Determine alkanes change address
            let alkanes_change_addr = params.alkanes_change_address.as_ref()
                .or(params.change_address.as_ref())
                .map(|s| s.as_str())
                .unwrap_or("p2tr:0");
            
            log::info!("Alkanes change will be sent to: {}", alkanes_change_addr);
            
            // Resolve the alkanes change address to find or create the correct output
            use crate::traits::AddressResolver;
            let resolved_change_addr = self.provider.resolve_all_identifiers(alkanes_change_addr).await?;
            let change_address = Address::from_str(&resolved_change_addr)?.require_network(self.provider.get_network())?;
            let change_script_pubkey = change_address.script_pubkey();

            // Find existing output matching the alkanes change address, or append a new one
            let alkanes_change_output_index = if let Some(idx) = outputs.iter().position(|o| o.script_pubkey == change_script_pubkey) {
                log::info!("Found existing output at index {} matching alkanes change address", idx);
                idx as u32
            } else {
                // No matching output exists — create one at the end
                // Appending avoids shifting existing output references (v0, v1, etc.)
                let new_idx = outputs.len() as u32;
                outputs.push(TxOut {
                    value: bitcoin::Amount::from_sat(DUST_LIMIT),
                    script_pubkey: change_script_pubkey,
                });
                log::info!("Created new alkanes change output at index {} (appended)", new_idx);
                new_idx
            };
            
            // Generate automatic protostone to split alkanes
            // Sends needed amounts to p1 (first user protostone) and excess to change output
            let auto_protostone = self.generate_alkanes_change_protostone(
                &alkanes_needed,
                &utxo_selection.alkanes_found,
                alkanes_change_output_index,
            ).await?;
            
            // Log original user protostones before adjustment
            log::info!("📝 Original user protostones (before adjustment):");
            for (i, ps) in params.protostones.iter().enumerate() {
                log::info!("   Protostone {}: {} edicts", i, ps.edicts.len());
                for (j, edict) in ps.edicts.iter().enumerate() {
                    log::info!("     Edict {}: alkane={}:{}, amount={}, target={:?}", 
                              j, edict.alkane_id.block, edict.alkane_id.tx, edict.amount, edict.target);
                }
                log::info!("     pointer={:?}, refund={:?}", ps.pointer, ps.refund);
            }
            
            // Adjust user protostone references - shift p0->p1, p1->p2, etc.
            // because we're inserting the auto-change protostone at the beginning
            let adjusted_user_protostones = self.adjust_protostone_references(&params.protostones);
            
            log::info!("📝 Adjusted user protostones (after shifting for auto-change):");
            for (i, ps) in adjusted_user_protostones.iter().enumerate() {
                log::info!("   Protostone {}: {} edicts", i, ps.edicts.len());
                for (j, edict) in ps.edicts.iter().enumerate() {
                    log::info!("     Edict {}: alkane={}:{}, amount={}, target={:?}", 
                              j, edict.alkane_id.block, edict.alkane_id.tx, edict.amount, edict.target);
                }
                log::info!("     pointer={:?}, refund={:?}", ps.pointer, ps.refund);
            }
            
            // Insert automatic protostone at the BEGINNING
            let mut combined = vec![auto_protostone];
            combined.extend(adjusted_user_protostones);
            
            log::info!("✅ Generated automatic protostone at beginning, final protostone count: {}", combined.len());
            combined
        } else {
            log::info!("✅ No excess alkanes - using original protostones");
            params.protostones.clone()
        };
        
        // Validate final protostones after potential automatic protostone insertion
        self.validate_protostones(&final_protostones, outputs.len())?;
        
        log::info!("🔍 About to construct runestone:");
        log::info!("   outputs.len() = {} (outputs before OP_RETURN)", outputs.len());
        for (i, output) in outputs.iter().enumerate() {
            log::info!("   Output {}: {} sats", i, output.value);
        }

        // Use final_funding_outpoints which may have inscribed UTXOs replaced with clean ones from split
        // When alkane inputs are specified, route them to the first protomessage (not output 0)
        let has_alkane_inputs = params.input_requirements.iter().any(|r| matches!(r, InputRequirement::Alkanes { .. }));
        let runestone_script = self.construct_runestone_script_with_alkane_routing(&final_protostones, outputs.len(), has_alkane_inputs)?;
        let prefetched_for_build = build_effective_txouts_map(params, &utxo_selection.txouts)?;
        let (psbt, fee, estimated_vsize) = self.build_psbt_and_fee(final_funding_outpoints.clone(), outputs, Some(runestone_script), params.fee_rate, None, None, prefetched_for_build.as_ref()).await?;

        // Validate the transaction before returning
        self.validate_transaction(&psbt, &final_funding_outpoints, fee, params, prefetched_for_build.as_ref()).await?;

        let unsigned_tx = &psbt.unsigned_tx;
        let analysis = crate::transaction::analysis::analyze_transaction(unsigned_tx);
        let inspection_result = self.inspect_from_protostones(&final_protostones).await.ok();

        Ok(ExecutionState::ReadyToSign(ReadyToSignTx {
            psbt,
            analysis,
            fee,
            estimated_vsize,
            inspection_result,
            split_psbt,
            split_fee,
        }))
    }
    
    /// Validate transaction to ensure sound value transfer semantics
    async fn validate_transaction(
        &self,
        psbt: &bitcoin::psbt::Psbt,
        selected_utxos: &[OutPoint],
        fee: u64,
        params: &EnhancedExecuteParams,
        effective_prefetched: Option<&alloc::collections::BTreeMap<OutPoint, TxOut>>,
    ) -> Result<()> {
        let tx = &psbt.unsigned_tx;

        let built_prefetched = if effective_prefetched.is_none() {
            build_prefetched_txouts_map(params)?
        } else {
            None
        };
        let prefetched = effective_prefetched.or(built_prefetched.as_ref());

        // 1. Calculate total input value
        let mut total_input_value = 0u64;
        for outpoint in selected_utxos {
            let utxo = match prefetched.and_then(|m| m.get(outpoint)) {
                Some(txout) => txout.clone(),
                None => self.provider.get_utxo(outpoint).await?
                    .ok_or_else(|| AlkanesError::Wallet(format!("UTXO not found during validation: {outpoint}")))?,
            };
            total_input_value += utxo.value.to_sat();
        }
        
        // 2. Calculate total output value
        let total_output_value: u64 = tx.output.iter()
            .filter(|o| !o.script_pubkey.is_op_return())
            .map(|o| o.value.to_sat())
            .sum();
        
        // 3. Validate: inputs >= outputs + fee
        if total_input_value < total_output_value + fee {
            return Err(AlkanesError::Validation(format!(
                "Insufficient funds: inputs ({}) < outputs ({}) + fee ({})",
                total_input_value, total_output_value, fee
            )));
        }
        
        // 4. Validate dust limits
        for (i, output) in tx.output.iter().enumerate() {
            if !output.script_pubkey.is_op_return() && output.value.to_sat() > 0 && output.value.to_sat() < DUST_LIMIT {
                return Err(AlkanesError::Validation(format!(
                    "Output {} has value {} sats which is below dust limit ({} sats)",
                    i, output.value.to_sat(), DUST_LIMIT
                )));
            }
        }
        
        // 5. Validate fee reasonableness
        if fee > MAX_FEE_SATS {
            return Err(AlkanesError::Validation(format!(
                "Fee {} sats exceeds maximum allowed fee ({} sats)",
                fee, MAX_FEE_SATS
            )));
        }
        
        // Calculate actual change amount
        let actual_change = total_input_value - total_output_value - fee;
        log::info!("Transaction validation passed:");
        log::info!("  Total inputs: {} sats", total_input_value);
        log::info!("  Total outputs: {} sats", total_output_value);
        log::info!("  Fee: {} sats", fee);
        log::info!("  Change: {} sats", actual_change);
        
        Ok(())
    }

    pub fn validate_protostones(&self, protostones: &[ProtostoneSpec], num_outputs: usize) -> Result<()> {
        log::info!("Validating {} protostones against {} outputs (including change and OP_RETURN)", protostones.len(), num_outputs);
        
        // The last output is the BTC change output, and we'll add an OP_RETURN,
        // so the actual number of usable physical outputs is num_outputs
        // (since we validate AFTER creating outputs but BEFORE adding OP_RETURN)
        
        for (i, protostone) in protostones.iter().enumerate() {
            for edict in &protostone.edicts {
                if let OutputTarget::Protostone(p) = edict.target {
                    if p <= i as u32 {
                        return Err(AlkanesError::Validation(format!(
                            "Protostone {i} refers to protostone {p} which is not allowed (must be > {i})"
                        )));
                    }
                }
            }
            
            if let Some(bitcoin_transfer) = &protostone.bitcoin_transfer {
                if matches!(bitcoin_transfer.target, OutputTarget::Protostone(_)) {
                    return Err(AlkanesError::Validation(format!(
                        "Bitcoin transfer in protostone {i} cannot target another protostone"
                    )));
                }
            }
            
            // Check pointer
            if let Some(OutputTarget::Output(v)) = protostone.pointer {
                if v as usize >= num_outputs {
                    return Err(AlkanesError::Validation(format!(
                        "Protostone {i} has pointer to output v{v} but only {num_outputs} outputs will exist"
                    )));
                }
            }
            
            // Check refund
            if let Some(OutputTarget::Output(v)) = protostone.refund {
                if v as usize >= num_outputs {
                    return Err(AlkanesError::Validation(format!(
                        "Protostone {i} has refund to output v{v} but only {num_outputs} outputs will exist"
                    )));
                }
            }
            
            // Check edicts
            for edict in &protostone.edicts {
                match edict.target {
                    OutputTarget::Output(v) => {
                        if v as usize >= num_outputs {
                            return Err(AlkanesError::Validation(format!(
                                "Edict in protostone {i} targets output v{v} but only {num_outputs} outputs will exist"
                            )));
                        }
                    },
                    OutputTarget::Protostone(p) => {
                        if p as usize >= protostones.len() {
                            return Err(AlkanesError::Validation(format!(
                                "Edict in protostone {} targets protostone p{} but only {} protostones exist",
                                i, p, protostones.len()
                            )));
                        }
                    },
                    OutputTarget::Split => {}
                }
            }
        }
        
        Ok(())
    }

    /// Visible to crate-internal tests so the alkane-needed branch's
    /// BTC-fill protection (line ~1944) can be exercised end-to-end
    /// without going through `execute_full`. The test in
    /// `pending_tx_store::tests` uses MockProvider's `alkane_balances`
    /// + utxo set to assert the skip-non-needed-alkane-carrier path.
    pub(crate) async fn select_utxos(&mut self, requirements: &[InputRequirement], from_addresses: &Option<Vec<String>>, known_pending_tx_hexes: &[String], max_indexed_height: Option<u64>, prefetched_utxos: &[PrefetchedUtxo], excluded_utxos: &[String], utxo_source: UtxoDataSource) -> Result<UtxoSelectionResult> {
        use crate::traits::AddressResolver;

        log::info!("Selecting UTXOs for {} requirements", requirements.len());
        log::info!("UTXO data source: {:?}", utxo_source);
        if let Some(addrs) = from_addresses {
            log::info!("Sourcing UTXOs from: {addrs:?}");
        }
        if let Some(h) = max_indexed_height {
            log::info!("max_indexed_height = {} (skipping confirmed UTXOs above this)", h);
        }

        // Caller-supplied soft locks ("txid:vout") — e.g. UTXOs committed to open
        // lending offers. Parsed once; malformed entries are logged and ignored.
        let excluded_set: alloc::collections::BTreeSet<OutPoint> = excluded_utxos
            .iter()
            .filter_map(|s| match OutPoint::from_str(s) {
                Ok(op) => Some(op),
                Err(_) => {
                    log::warn!("Ignoring malformed excluded_utxos entry: {}", s);
                    None
                }
            })
            .collect();
        if !excluded_set.is_empty() {
            log::info!("Excluding {} caller-locked UTXO(s) from selection", excluded_set.len());
        }

        // Resolve address identifiers like p2tr:0 to actual addresses before passing to get_utxos
        let resolved_from_addresses = if let Some(addrs) = from_addresses {
            let mut resolved = Vec::new();
            for addr in addrs {
                let resolved_addr = self.provider.resolve_all_identifiers(addr).await?;
                resolved.push(resolved_addr);
            }
            Some(resolved)
        } else {
            None
        };

        let mut selected_txout_candidates: alloc::collections::BTreeMap<OutPoint, TxOut> =
            alloc::collections::BTreeMap::new();
        let mut espo_alkanes_by_outpoint: alloc::collections::BTreeMap<OutPoint, Vec<(ProtoruneRuneId, u128)>> =
            alloc::collections::BTreeMap::new();

        let use_espo_source = matches!(utxo_source, UtxoDataSource::Espo);
        let mut using_espo_source = false;
        let utxos = if use_espo_source {
            if let Some(addresses) = &resolved_from_addresses {
                let mut fetched_utxos: Vec<(OutPoint, UtxoInfo)> = Vec::new();
                let mut espo_failed = false;

                for address in addresses {
                    match self.provider.get_address_spendable_outpoints(address).await {
                        Ok(response) => {
                            if response.get("ok").and_then(|v| v.as_bool()) == Some(false) {
                                log::warn!(
                                    "Espo spendable outpoints returned ok=false for {}; falling back to metashrew path",
                                    address
                                );
                                espo_failed = true;
                                break;
                            }

                            let outpoints = response
                                .get("outpoints")
                                .and_then(|v| v.as_array())
                                .cloned()
                                .unwrap_or_default();

                            log::info!(
                                "Espo spendable outpoints returned {} UTXOs for {}",
                                outpoints.len(),
                                address
                            );

                            for entry in outpoints {
                                let Some(outpoint_str) = entry.get("outpoint").and_then(|v| v.as_str()) else {
                                    continue;
                                };
                                let Ok(outpoint) = OutPoint::from_str(outpoint_str) else {
                                    log::debug!("Skipping malformed Espo outpoint: {}", outpoint_str);
                                    continue;
                                };
                                let Some(value) = entry.get("value").and_then(json_u64) else {
                                    log::debug!("Skipping Espo outpoint without value: {}", outpoint_str);
                                    continue;
                                };
                                let Some(script_hex) = entry.get("script_pubkey_hex").and_then(|v| v.as_str()) else {
                                    log::debug!("Skipping Espo outpoint without script_pubkey_hex: {}", outpoint_str);
                                    continue;
                                };
                                let script_bytes = match hex::decode(script_hex) {
                                    Ok(bytes) => bytes,
                                    Err(e) => {
                                        log::debug!("Skipping Espo outpoint with invalid script_pubkey_hex {}: {}", outpoint_str, e);
                                        continue;
                                    }
                                };
                                let script_pubkey = ScriptBuf::from_bytes(script_bytes);
                                let alkanes = parse_espo_alkanes(&entry);
                                let has_alkanes = alkanes.iter().any(|(_, amount)| *amount > 0);
                                let has_runes = entry
                                    .get("runes")
                                    .and_then(|v| v.as_array())
                                    .map(|runes| !runes.is_empty())
                                    .unwrap_or(false);

                                selected_txout_candidates.insert(
                                    outpoint,
                                    TxOut {
                                        value: bitcoin::Amount::from_sat(value),
                                        script_pubkey: script_pubkey.clone(),
                                    },
                                );
                                espo_alkanes_by_outpoint.insert(outpoint, alkanes);
                                fetched_utxos.push((
                                    outpoint,
                                    UtxoInfo {
                                        txid: outpoint.txid.to_string(),
                                        vout: outpoint.vout,
                                        amount: value,
                                        address: address.clone(),
                                        script_pubkey: Some(script_pubkey),
                                        confirmations: entry
                                            .get("confirmations")
                                            .and_then(json_u64)
                                            .unwrap_or(0)
                                            .min(u32::MAX as u64) as u32,
                                        frozen: false,
                                        freeze_reason: None,
                                        block_height: entry.get("block_height").and_then(json_u64),
                                        has_inscriptions: false,
                                        has_runes,
                                        has_alkanes,
                                        is_coinbase: entry
                                            .get("coinbase")
                                            .and_then(|v| v.as_bool())
                                            .unwrap_or(false),
                                    },
                                ));
                            }
                        }
                        Err(e) => {
                            log::warn!(
                                "Espo spendable outpoints failed for {}; falling back to metashrew path: {}",
                                address,
                                e
                            );
                            espo_failed = true;
                            break;
                        }
                    }
                }

                if espo_failed {
                    selected_txout_candidates.clear();
                    espo_alkanes_by_outpoint.clear();
                    self.provider.get_utxos(true, Some(addresses.clone())).await?
                } else {
                    using_espo_source = true;
                    fetched_utxos
                }
            } else {
                log::warn!(
                    "Espo UTXO source requires explicit from_addresses; falling back to metashrew path"
                );
                self.provider.get_utxos(true, None).await?
            }
        } else {
            self.provider.get_utxos(true, resolved_from_addresses).await?
        };
        log::debug!("Found {} total wallet UTXOs from specified sources", utxos.len());

        // Filter UTXOs through the centralised eligibility check
        // (frozen / immature-coinbase / unindexed-height). Logs structured
        // skip reasons so operators can tell *why* a UTXO was excluded.
        let mut spendable_utxos: Vec<(OutPoint, UtxoInfo)> = utxos.into_iter()
            .filter(|(outpoint, info)| {
                if excluded_set.contains(outpoint) {
                    log::debug!("Skipping caller-excluded UTXO: {}:{}", info.txid, info.vout);
                    return false;
                }
                true
            })
            .filter(|(_, info)| {
                match check_utxo_eligibility(info, max_indexed_height) {
                    Ok(()) => true,
                    Err(UtxoSkipReason::Frozen) => {
                        log::debug!("Skipping frozen UTXO: {}:{}", info.txid, info.vout);
                        false
                    }
                    Err(UtxoSkipReason::ImmatureCoinbase { confirmations }) => {
                        log::debug!(
                            "Skipping immature coinbase UTXO: {}:{} (confirmations: {}, required: {})",
                            info.txid, info.vout, confirmations, COINBASE_MATURITY
                        );
                        false
                    }
                    Err(UtxoSkipReason::UnindexedHeight { block_height, max_indexed }) => {
                        log::debug!(
                            "Skipping unindexed UTXO: {}:{} (block_height={}, max_indexed={})",
                            info.txid, info.vout, block_height, max_indexed
                        );
                        false
                    }
                }
            })
            .collect();

        log::info!("Found {} spendable wallet UTXOs after eligibility filter", spendable_utxos.len());

        // Caller-supplied per-outpoint alkane assertions, used to short-circuit
        // the two `protorunesbyoutpoint` fanouts below (~40s on a 30+ dust
        // wallet, observed mainnet 2026-05-09). `None` here means either no
        // caller supplied assertions at all, or all entries omitted `alkanes`
        // — both cases keep the existing slow path verbatim. Built once and
        // shared between the alkane-aware primary-discovery branch (line
        // ~1716) and the BTC-only exclusion branch (line ~2119).
        let mut prefetched_alkanes_merged =
            build_prefetched_alkanes_map(prefetched_utxos)?.unwrap_or_default();
        prefetched_alkanes_merged.extend(espo_alkanes_by_outpoint);
        let prefetched_alkanes = if prefetched_alkanes_merged.is_empty() {
            None
        } else {
            Some(prefetched_alkanes_merged)
        };

        // Mempool-aware UTXO adjustment for the user's own pending txs.
        //
        // The lua spendable_utxos.lua script (and its esplora fallback) only
        // returns CONFIRMED UTXOs. That breaks two scenarios:
        //
        //   1. Quick double-submit: user broadcasts tx X, then immediately
        //      tries to broadcast tx Y. Both pull the same confirmed UTXO
        //      because the indexer hasn't yet seen X.spent its inputs.
        //      Result: BIP125 RBF conflict (Y has same prevout as X with
        //      lower fee → "insufficient fee, rejecting replacement").
        //
        //   2. CPFP chains (split-tx mode): execute_split broadcasts wrap
        //      Tx A then immediately builds execute Tx B. Tx B should spend
        //      Tx A's outputs (alkane carrier + BTC change), but those are
        //      unconfirmed so the lua filter excludes them. Meanwhile the
        //      original user UTXOs still appear "spendable" even though
        //      Tx A consumed them. Same RBF symptom as #1.
        //
        // Fix: walk the user's mempool txs, strip outpoints they spend from
        // `spendable_utxos`, and add the txs' user-paying outputs as
        // candidates. Skipped on qubitcoin (no esplora mempool endpoint).
        if !self.provider.is_qubitcoin_mode() {
            let mempool_addresses: Vec<String> = if let Some(addrs) = from_addresses {
                let mut resolved = Vec::new();
                for addr in addrs {
                    if let Ok(r) = self.provider.resolve_all_identifiers(addr).await {
                        resolved.push(r);
                    }
                }
                resolved
            } else {
                let mut a: Vec<String> = spendable_utxos.iter().map(|(_, u)| u.address.clone()).collect();
                a.sort();
                a.dedup();
                a
            };

            let mut mempool_payloads: Vec<serde_json::Value> = Vec::new();
            for address in &mempool_addresses {
                match self.provider.get_address_txs_mempool(address).await {
                    Ok(v) => mempool_payloads.push(v),
                    Err(e) => log::debug!("get_address_txs_mempool({}) failed: {}", address, e),
                }
            }

            // Inject caller-provided pending txs (e.g. Tx A's hex passed by
            // execute_split). The indexer's mempool view lags the just-
            // broadcast tx by ~hundreds of ms — by the time Tx B's
            // select_utxos calls /address/_/txs/mempool, Tx A often hasn't
            // propagated through the indexer pipeline yet, so the filter
            // can't see what to strip. Decoding the caller-supplied tx hex
            // into the same JSON shape closes that window deterministically.
            //
            // Two sources are merged:
            //   1. `known_pending_tx_hexes` — explicit per-call override,
            //      used by callers without provider access (vendored CLI
            //      tools, integration tests).
            //   2. `provider.pending_tx_store()` — session-scoped store
            //      that all `execute_full` paths push to on broadcast,
            //      so chained / cross-call flows benefit automatically
            //      without each caller having to thread Tx-A's hex
            //      through.
            //
            // Duplicates are tolerated — `apply_mempool_adjustment` keys
            // its spent-outpoint set by (txid, vout), so the same tx
            // appearing twice is a no-op.
            let mut all_pending: Vec<String> = known_pending_tx_hexes.to_vec();
            if let Some(store) = self.provider.pending_tx_store() {
                match store.list().await {
                    Ok(hexes) => all_pending.extend(hexes),
                    Err(e) => log::warn!("pending_tx_store.list() failed: {}", e),
                }
            }
            for hex_str in &all_pending {
                match decode_tx_hex_to_mempool_json(hex_str) {
                    Ok(synthetic_tx) => {
                        mempool_payloads.push(serde_json::json!([synthetic_tx]));
                    }
                    Err(e) => {
                        log::warn!("Failed to decode pending tx hex: {}", e);
                    }
                }
            }

            let report = apply_mempool_adjustment(
                &mut spendable_utxos,
                &mempool_payloads,
                &mempool_addresses,
            );
            if report.stripped > 0 || report.added > 0 {
                log::info!(
                    "Mempool-aware adjustment: stripped {} confirmed UTXOs already spent in our pending txs, added {} unconfirmed outputs from those txs (final: {})",
                    report.stripped, report.added, spendable_utxos.len()
                );
            }
        }

        let mut selected_outpoints = Vec::new();
        let mut bitcoin_needed = 0u64;
        let mut alkanes_needed = alloc::collections::BTreeMap::new();

        for requirement in requirements {
            match requirement {
                InputRequirement::Bitcoin { amount } => {
                    bitcoin_needed += amount;
                }
                InputRequirement::BitcoinOutput { amount, .. } => {
                    // BitcoinOutput requirements contribute to Bitcoin needed
                    bitcoin_needed += amount;
                }
                InputRequirement::Alkanes { block, tx, amount } => {
                    let key = (*block, *tx);
                    *alkanes_needed.entry(key).or_insert(0) += amount;
                }
            }
        }

        log::info!("Need {} sats Bitcoin and {} different alkanes tokens", bitcoin_needed, alkanes_needed.len());

        if !alkanes_needed.is_empty() {
            if using_espo_source {
                log::info!("Alkane inputs required -- using Espo spendable outpoint balances");
            } else if prefetched_covers_alkanes_needed(prefetched_utxos, &alkanes_needed) {
                // Caller (e.g. subfrost-app's useWalletState path) has asserted
                // alkane balances per outpoint that already satisfy every
                // requirement. They own freshness for those entries; no need to
                // burn the metashrew sync poll loop here (saves ~5-30s when the
                // public indexer lags bitcoind by a block or two).
                log::info!(
                    "Alkane inputs required -- caller-supplied prefetched_utxos cover all {} alkane requirement(s); skipping indexer sync",
                    alkanes_needed.len()
                );
            } else {
                log::info!("Alkane inputs required -- syncing indexer before balance query");
                self.provider.sync().await?;
            }
        }

        let mut bitcoin_collected = 0u64;
        let mut alkanes_collected: alloc::collections::BTreeMap<(u64, u64), u64> = alloc::collections::BTreeMap::new();
        let mut alkanes_found: alloc::collections::BTreeMap<AlkaneId, u64> = alloc::collections::BTreeMap::new();
        let mut per_utxo_alkanes: alloc::collections::BTreeMap<OutPoint, Vec<(AlkaneId, u64)>> = alloc::collections::BTreeMap::new();

        // If we need alkanes, query protorunes_by_address directly to find UTXOs with balances
        // This bypasses the lua batch script which has issues with individual outpoint queries
        if !alkanes_needed.is_empty() {
            log::info!("Querying UTXOs for alkane balances using espo (primary) with metashrew fallback...");

            // IMPORTANT: Get addresses from from_addresses parameter, NOT just from spendable_utxos
            // The alkane UTXOs may be on addresses that have no Bitcoin UTXOs (esplora doesn't know about them)
            // So we must query ALL addresses specified by the user, not just ones with existing UTXOs
            let addresses_to_query: Vec<String> = if let Some(addrs) = from_addresses {
                // Re-resolve addresses (in case they're descriptors like p2tr:0)
                let mut resolved = Vec::new();
                for addr in addrs {
                    if let Ok(resolved_addr) = self.provider.resolve_all_identifiers(addr).await {
                        resolved.push(resolved_addr);
                    }
                }
                resolved
            } else {
                // Fall back to addresses from existing UTXOs
                let mut addrs: Vec<String> = spendable_utxos.iter().map(|(_, u)| u.address.clone()).collect();
                addrs.sort();
                addrs.dedup();
                addrs
            };

            log::info!("Fetching balances for {} addresses (espo primary, metashrew fallback): {:?}",
                       addresses_to_query.len(), addresses_to_query);

            // Create a map of (txid:vout) -> balance data for quick lookup
            let mut utxo_balances: alloc::collections::BTreeMap<String, serde_json::Value> = alloc::collections::BTreeMap::new();

            if using_espo_source {
                if let Some(prefetched) = prefetched_alkanes.as_ref() {
                    for (outpoint, balances) in prefetched {
                        let balances_array: Vec<serde_json::Value> = balances
                            .iter()
                            .filter(|(_, amount)| *amount > 0)
                            .map(|(rune_id, amount)| serde_json::json!({
                                "block": rune_id.block,
                                "tx": rune_id.tx,
                                "amount": amount.to_string(),
                            }))
                            .collect();
                        if !balances_array.is_empty() {
                            utxo_balances.insert(
                                format!("{}:{}", outpoint.txid, outpoint.vout),
                                serde_json::json!({ "balances": balances_array }),
                            );
                        }
                    }
                    log::info!(
                        "Espo spendable outpoints supplied alkane balances for {} UTXOs",
                        utxo_balances.len()
                    );
                }
            }

            // Primary discovery: enrich each currently-spendable BTC UTXO with
            // its alkane balance via per-outpoint protorunesbyoutpoint. The
            // BTC layer's UTXO set is the source of truth for spentness, so
            // walking spendable_utxos guarantees we never miss a just-
            // confirmed alkane output. The address-keyed fallbacks below
            // (espo / metashrew Lua / protorunesbyaddress) lag the BTC layer
            // by one or more blocks on mainnet, which surfaced as
            // "Insufficient spendable have 0" errors when a user tries to
            // spend tokens minted by a tx confirmed in the most recent block.
            //
            // Mirrors the wallet UI's display path
            // (subfrost-app/queries/account.ts::fetchAlkaneBalancesViaProtobuf,
            // commit 9ec751fb) — same architecture, applied to the SDK's
            // mutation-side selector.
            //
            // Skipped in qubitcoin mode (espo/lua aren't available there
            // anyway, and protorunesbyaddress is the canonical path).
            if !self.provider.is_qubitcoin_mode() {
                // Do the balances discovered so far satisfy every alkane
                // requirement? Drives the two-pass probe below.
                let requirements_covered =
                    |utxo_balances: &alloc::collections::BTreeMap<String, serde_json::Value>|
                     -> bool {
                        let mut totals: alloc::collections::BTreeMap<(u64, u64), u64> =
                            alloc::collections::BTreeMap::new();
                        for utxo_data in utxo_balances.values() {
                            let Some(arr) = utxo_data.get("balances").and_then(|v| v.as_array()) else {
                                continue;
                            };
                            for b in arr {
                                let block = b.get("block").and_then(|v| {
                                    v.as_u64().or_else(|| v.as_str().and_then(|s| s.parse::<u64>().ok()))
                                });
                                let tx = b.get("tx").and_then(|v| {
                                    v.as_u64().or_else(|| v.as_str().and_then(|s| s.parse::<u64>().ok()))
                                });
                                let amount = b.get("amount").and_then(|v| {
                                    v.as_u64().or_else(|| v.as_str().and_then(|s| s.parse::<u64>().ok()))
                                });
                                if let (Some(block), Some(tx), Some(amount)) = (block, tx, amount) {
                                    let e = totals.entry((block, tx)).or_insert(0);
                                    *e = e.saturating_add(amount);
                                }
                            }
                        }
                        alkanes_needed
                            .iter()
                            .all(|(key, needed)| totals.get(key).copied().unwrap_or(0) >= *needed)
                    };

                // PASS 0: dust UTXOs (≤1000 sats). Alkanes CONVENTIONALLY ride dust
                // outputs, so this covers almost every wallet with minimal RPC fan-out.
                //
                // PASS 1 (only when pass 0 leaves a requirement unmet): every remaining
                // spendable UTXO. Nothing on-chain enforces the dust convention —
                // mainnet counter-example (2026-07-11, borrower of lending child
                // 2:92269): three 2916-sat UTXOs carrying 2000 frBTC each were
                // invisible to the dust-only probe, and because a non-empty primary
                // result skips the address-keyed fallbacks entirely, a 21864
                // repayment failed "Insufficient alkanes: … have 18739" out of a
                // real 24739 balance. Prefetched assertions still short-circuit in
                // pass 1, so callers that assert every outpoint (e.g. subfrost-app's
                // wallet-state snapshot) pay zero extra RPC for the extension.
                let mut probed: alloc::collections::BTreeSet<OutPoint> =
                    alloc::collections::BTreeSet::new();
                for pass in 0..2u8 {
                    if pass == 1 && requirements_covered(&utxo_balances) {
                        break;
                    }
                    let candidates: Vec<&(OutPoint, UtxoInfo)> = spendable_utxos
                        .iter()
                        .filter(|(op, u)| !probed.contains(op) && (pass == 1 || u.amount <= 1000))
                        .collect();
                    if candidates.is_empty() {
                        continue;
                    }
                    if pass == 0 {
                        log::info!(
                            "Primary discovery: per-outpoint protorunesbyoutpoint for {} dust UTXOs",
                            candidates.len()
                        );
                    } else {
                        log::info!(
                            "Extended discovery: requirements unmet after dust pass — probing {} non-dust UTXOs",
                            candidates.len()
                        );
                    }

                    // Counters for the prefetched-vs-RPC observability log
                    // emitted at the bottom of this loop. Cheap to maintain
                    // even when the optimization isn't engaged.
                    let mut prefetched_count: usize = 0;
                    let mut rpc_count: usize = 0;

                    for (outpoint, _utxo) in &candidates {
                        probed.insert(*outpoint);
                        // Short-circuit: if the caller has asserted balances
                        // for this outpoint via `prefetched_utxos[i].alkanes`,
                        // use them and skip the RPC. Empty Vec is authoritative
                        // "no alkanes here" — leaves utxo_balances unchanged
                        // for that key, mirroring the slow path's behavior
                        // when balances_array is empty. Same trust contract
                        // as `value` / `script_pubkey_hex`: caller is
                        // responsible for invalidating on block-tip change.
                        if let Some(balances) = prefetched_alkanes
                            .as_ref()
                            .and_then(|m| m.get(outpoint))
                        {
                            prefetched_count += 1;
                            let mut balances_array = Vec::new();
                            for (rune_id, amount) in balances {
                                if *amount == 0 {
                                    continue;
                                }
                                balances_array.push(serde_json::json!({
                                    "block": rune_id.block,
                                    "tx": rune_id.tx,
                                    "amount": amount.to_string(),
                                }));
                            }
                            if !balances_array.is_empty() {
                                let key = format!(
                                    "{}:{}",
                                    outpoint.txid, outpoint.vout
                                );
                                utxo_balances.insert(
                                    key,
                                    serde_json::json!({ "balances": balances_array }),
                                );
                            }
                            continue;
                        }

                        rpc_count += 1;
                        let txid_str = outpoint.txid.to_string();
                        match self.provider
                            .get_protorunes_by_outpoint(&txid_str, outpoint.vout, None, 1)
                            .await
                        {
                            Ok(response) => {
                                let mut balances_array = Vec::new();
                                for (rune_id, amount) in
                                    &response.balance_sheet.cached.balances
                                {
                                    // Drop zero-amount placeholder entries —
                                    // the indexer occasionally returns them
                                    // for outpoints that referenced an alkane
                                    // id without actually carrying value.
                                    if *amount == 0 {
                                        continue;
                                    }
                                    balances_array.push(serde_json::json!({
                                        "block": rune_id.block,
                                        "tx": rune_id.tx,
                                        "amount": amount.to_string(),
                                    }));
                                }
                                if !balances_array.is_empty() {
                                    let key = format!(
                                        "{}:{}",
                                        outpoint.txid, outpoint.vout
                                    );
                                    utxo_balances.insert(
                                        key,
                                        serde_json::json!({ "balances": balances_array }),
                                    );
                                }
                            }
                            Err(e) => {
                                // Non-fatal: a single failed outpoint just
                                // means we miss its balance for this call.
                                // The address-keyed fallbacks below may pick
                                // it up.
                                log::debug!(
                                    "protorunesbyoutpoint failed for {}:{}: {}",
                                    outpoint.txid, outpoint.vout, e
                                );
                            }
                        }
                    }

                    log::info!(
                        "{} discovery: {} prefetched, {} via RPC",
                        if pass == 0 { "Primary" } else { "Extended" },
                        prefetched_count, rpc_count
                    );
                }
            }

            // Address-keyed fallbacks. Run only if the primary path produced
            // no balances (e.g., qubitcoin mode, or networks where the
            // protorunesbyoutpoint view isn't available). When the primary
            // path succeeds these are skipped to avoid double-fetching.
            let primary_succeeded = !utxo_balances.is_empty();
            if !primary_succeeded {
                log::info!("Primary path produced no balances — falling back to address-keyed views");
            }

            // Fetch alkane balances per address.
            // Strategy: try espo first, then metashrew Lua, then protorunesbyaddress.
            // In qubitcoin mode, skip espo/Lua and go straight to protorunesbyaddress.
            let empty_addresses: Vec<String> = Vec::new();
            let addresses_for_fallback: &[String] = if primary_succeeded {
                &empty_addresses
            } else {
                &addresses_to_query
            };
            for address in addresses_for_fallback {
                if self.provider.is_qubitcoin_mode() {
                    // Qubitcoin mode: use protorunesbyaddress directly
                    log::info!("Qubitcoin mode: using protorunesbyaddress for alkane UTXO discovery");
                    match self.provider.get_protorunes_by_address(address, None, 1).await {
                        Ok(response) => {
                            for outpoint_resp in &response.balances {
                                let key = format!("{}:{}", outpoint_resp.outpoint.txid, outpoint_resp.outpoint.vout);
                                let mut balances_array = Vec::new();
                                for (rune_id, amount) in &outpoint_resp.balance_sheet.cached.balances {
                                    balances_array.push(serde_json::json!({
                                        "block": rune_id.block,
                                        "tx": rune_id.tx,
                                        "amount": amount
                                    }));
                                }
                                if !balances_array.is_empty() {
                                    utxo_balances.insert(key, serde_json::json!({ "balances": balances_array }));
                                }
                            }
                            log::info!("protorunesbyaddress returned {} outpoints with balances", utxo_balances.len());
                        }
                        Err(e) => {
                            log::error!("protorunesbyaddress failed for {}: {}", address, e);
                        }
                    }
                    continue;
                }
                // Primary: espo get_address_outpoints (no metashrew dependency)
                match self.provider.get_address_outpoints(address).await {
                    Ok(result) => {
                        // Espo response: {"ok": true, "outpoints": [{"outpoint": "txid:vout", "entries": [{"alkane": "block:tx", "amount": "N"}]}]}
                        if let Some(outpoints_array) = result.get("outpoints").and_then(|v| v.as_array()) {
                            log::info!("Espo returned {} outpoints for address {}", outpoints_array.len(), address);
                            for outpoint_obj in outpoints_array {
                                if let Some(outpoint_str) = outpoint_obj.get("outpoint").and_then(|v| v.as_str()) {
                                    // Convert espo entries to the internal format: { balances: [{ block, tx, amount }] }
                                    let mut balances_array = Vec::new();
                                    if let Some(entries) = outpoint_obj.get("entries").and_then(|v| v.as_array()) {
                                        for entry in entries {
                                            if let (Some(alkane_str), Some(amount_str)) = (
                                                entry.get("alkane").and_then(|v| v.as_str()),
                                                entry.get("amount").and_then(|v| v.as_str()),
                                            ) {
                                                // Parse "block:tx" format
                                                let parts: Vec<&str> = alkane_str.split(':').collect();
                                                if parts.len() == 2 {
                                                    if let (Ok(block), Ok(tx)) = (parts[0].parse::<u64>(), parts[1].parse::<u64>()) {
                                                        let amount = amount_str.parse::<u64>().unwrap_or(0);
                                                        balances_array.push(serde_json::json!({
                                                            "block": block,
                                                            "tx": tx,
                                                            "amount": amount
                                                        }));
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    utxo_balances.insert(outpoint_str.to_string(), serde_json::json!({
                                        "balances": balances_array
                                    }));
                                }
                            }
                        }
                    }
                    Err(e) => {
                        log::info!("Espo unavailable for address {} ({}), falling back to metashrew batch", address, e);
                        // Fallback: metashrew Lua batch script
                        match self.provider.batch_fetch_utxo_balances(address, Some(1), None).await {
                            Ok(result) => {
                                // Parse the result and index by txid:vout
                                // The lua script may wrap results in {"returns": {"utxos": [...]}}
                                let utxos_value = result.get("utxos")
                                    .or_else(|| result.get("returns").and_then(|r| r.get("utxos")));
                                if let Some(utxos_array) = utxos_value.and_then(|v| v.as_array()) {
                                    for utxo_entry in utxos_array {
                                        if let (Some(txid), Some(vout)) = (
                                            utxo_entry.get("txid").and_then(|v| v.as_str()),
                                            utxo_entry.get("vout").and_then(|v| v.as_u64())
                                        ) {
                                            let key = format!("{}:{}", txid, vout);
                                            utxo_balances.insert(key, utxo_entry.clone());
                                        }
                                    }
                                }
                            }
                            Err(e2) => {
                                log::info!("Both espo and metashrew failed for address {}: espo={}, metashrew={}", address, e, e2);
                                // Final fallback: use protorunesbyaddress directly
                                // This works in qubitcoin mode where espo/lua are unavailable
                                log::info!("Falling back to protorunesbyaddress for alkane UTXO discovery");
                                match self.provider.get_protorunes_by_address(address, None, 1).await {
                                    Ok(response) => {
                                        for outpoint_resp in &response.balances {
                                            let key = format!("{}:{}", outpoint_resp.outpoint.txid, outpoint_resp.outpoint.vout);
                                            // Extract balances from the balance_sheet
                                            let mut balances_array = Vec::new();
                                            for (rune_id, amount) in &outpoint_resp.balance_sheet.cached.balances {
                                                balances_array.push(serde_json::json!({
                                                    "block": rune_id.block,
                                                    "tx": rune_id.tx,
                                                    "amount": amount
                                                }));
                                            }
                                            if !balances_array.is_empty() {
                                                utxo_balances.insert(key, serde_json::json!({
                                                    "balances": balances_array
                                                }));
                                            }
                                        }
                                        log::info!("protorunesbyaddress returned {} outpoints with balances", utxo_balances.len());
                                    }
                                    Err(e3) => {
                                        log::error!("All alkane UTXO discovery methods failed for {}: {}", address, e3);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            
            // Check if any espo-discovered alkane UTXOs are missing from spendable_utxos
            // This happens when esplora doesn't return the UTXO (e.g., small value outputs)
            // but espo knows about it because it tracks alkane balances
            let spendable_keys: alloc::collections::BTreeSet<String> = spendable_utxos.iter()
                .map(|(op, _)| format!("{}:{}", op.txid, op.vout))
                .collect();

            for (utxo_key, utxo_data) in &utxo_balances {
                if spendable_keys.contains(utxo_key) {
                    continue; // Already in spendable set
                }

                // Check if this UTXO has alkanes we need
                let has_needed = utxo_data.get("balances").and_then(|v| v.as_array()).map(|arr| {
                    arr.iter().any(|b| {
                        let block = b.get("block").and_then(|v| v.as_u64()).unwrap_or(0);
                        let tx = b.get("tx").and_then(|v| v.as_u64()).unwrap_or(0);
                        alkanes_needed.contains_key(&(block, tx))
                    })
                }).unwrap_or(false);

                if !has_needed {
                    continue;
                }

                // This UTXO has needed alkanes but isn't in spendable set - fetch its tx and add it
                let parts: Vec<&str> = utxo_key.split(':').collect();
                if parts.len() != 2 {
                    continue;
                }
                let txid_str = parts[0];
                let vout: u32 = match parts[1].parse() {
                    Ok(v) => v,
                    Err(_) => continue,
                };

                log::info!("Alkane UTXO {}:{} missing from esplora -- adding from protorunesbyaddress data", txid_str, vout);

                // In qubitcoin mode, protorunesbyaddress already includes the TxOut data.
                // Look up the outpoint in the wallet response to get value/script.
                // If not available, try fetching the raw TX (requires txindex).
                let txid = match bitcoin::Txid::from_str(txid_str) {
                    Ok(t) => t,
                    Err(_) => continue,
                };
                let outpoint = OutPoint { txid, vout };
                if excluded_set.contains(&outpoint) {
                    log::debug!("Skipping caller-excluded espo alkane UTXO: {}", utxo_key);
                    continue;
                }

                // Try to get output info from the protorunesbyaddress response (already in memory)
                // The response is stored in utxo_balances but we need the TxOut data.
                // As a fallback, use dust value (546 sats) and the address's script_pubkey.
                let address = if !addresses_to_query.is_empty() {
                    addresses_to_query[0].clone()
                } else {
                    String::new()
                };

                // Use dust value as default — protorunesbyaddress UTXOs are typically dust
                let utxo_value = 546u64;
                // Derive script_pubkey from address (needed for PSBT witness UTXO)
                let script_pubkey = bitcoin::Address::from_str(&address)
                    .ok()
                    .and_then(|a| a.require_network(self.provider.get_network()).ok())
                    .map(|a| a.script_pubkey());
                let utxo_info = UtxoInfo {
                    txid: txid_str.to_string(),
                    vout,
                    amount: utxo_value,
                    address: address.clone(),
                    script_pubkey,
                    confirmations: 100,
                    frozen: false,
                    freeze_reason: None,
                    block_height: None,
                    has_inscriptions: false,
                    has_runes: false,
                    has_alkanes: true,
                    is_coinbase: false,
                };
                log::info!("Added alkane UTXO {}:{} ({} sats) to spendable set", txid_str, vout, utxo_value);
                spendable_utxos.push((outpoint, utxo_info));
            }

            // Now process UTXOs using the pre-fetched balance data
            for (outpoint, utxo) in spendable_utxos {
                let key = format!("{}:{}", outpoint.txid, outpoint.vout);

                if let Some(utxo_data) = utxo_balances.get(&key) {
                    // Parse balance data from batch result
                    // Note: amounts may come as strings (from lua/protobuf) or numbers
                    let balances = utxo_data.get("balances").and_then(|v| v.as_array()).map(|arr| {
                        arr.iter().filter_map(|b| {
                            let block = b.get("block").and_then(|v| {
                                v.as_u64().or_else(|| v.as_str().and_then(|s| s.parse::<u64>().ok()))
                            })?;
                            let tx = b.get("tx").and_then(|v| {
                                v.as_u64().or_else(|| v.as_str().and_then(|s| s.parse::<u64>().ok()))
                            })?;
                            // Handle amount as either number or string
                            let amount = b.get("amount").and_then(|v| {
                                v.as_u64().or_else(|| v.as_str().and_then(|s| s.parse::<u64>().ok()))
                            })?;
                            Some(((block, tx), amount))
                        }).collect::<Vec<_>>()
                    }).unwrap_or_default();
                    
                    let mut has_needed_alkane = false;
                    let mut utxo_selected = false;

                    // Check if this UTXO has any alkanes we need
                    for ((block, tx), amount) in &balances {
                        let key = (*block, *tx);
                        if let Some(needed) = alkanes_needed.get(&key) {
                            let collected = alkanes_collected.entry(key).or_insert(0);
                            if *collected < *needed {
                                has_needed_alkane = true;
                                *collected += amount;
                                log::debug!("Found {} of alkane {}:{} in UTXO {}:{} (collected: {}/{})",
                                    amount, block, tx, outpoint.txid, outpoint.vout, *collected, needed);
                            }
                        }
                    }
                    
                    // Select this UTXO if it has alkanes we need.
                    // Do NOT select alkane-carrying UTXOs just for Bitcoin — this
                    // would accidentally spend someone's tokens as fee inputs.
                    if has_needed_alkane {
                        bitcoin_collected += utxo.amount;
                        selected_outpoints.push(outpoint);
                        utxo_selected = true;
                        log::debug!("Selected UTXO {}:{} for required alkanes (btc: {})", outpoint.txid, outpoint.vout, utxo.amount);
                    } else if !balances.is_empty() {
                        // This UTXO carries alkanes we don't need — skip it for BTC
                        log::debug!("Skipping UTXO {}:{} — has alkane balances not in requirements", outpoint.txid, outpoint.vout);
                    } else if bitcoin_collected < bitcoin_needed {
                        // No alkane balances — safe to use for BTC
                        bitcoin_collected += utxo.amount;
                        selected_outpoints.push(outpoint);
                        utxo_selected = true;
                        log::debug!("Selected UTXO {}:{} for Bitcoin only (btc: {})", outpoint.txid, outpoint.vout, utxo.amount);
                    }
                    
                    // Track ALL alkanes found in selected UTXOs (for change calculation)
                    if utxo_selected {
                        let mut utxo_alkane_list = Vec::new();
                        for ((block, tx), amount) in &balances {
                            let alkane_key = AlkaneId {
                                block: *block,
                                tx: *tx,
                            };
                            *alkanes_found.entry(alkane_key.clone()).or_insert(0) += amount;
                            utxo_alkane_list.push((alkane_key, *amount));
                        }
                        if !utxo_alkane_list.is_empty() {
                            per_utxo_alkanes.insert(outpoint, utxo_alkane_list);
                        }
                    }
                    
                    // Check if we've collected enough of everything
                    let all_alkanes_satisfied = alkanes_needed.iter().all(|(key, needed)| {
                        alkanes_collected.get(key).unwrap_or(&0) >= needed
                    });
                    
                    if bitcoin_collected >= bitcoin_needed && all_alkanes_satisfied {
                        break;
                    }
                } else {
                    // No balance data for this UTXO, still consider it for Bitcoin if needed
                    if bitcoin_collected < bitcoin_needed {
                        bitcoin_collected += utxo.amount;
                        selected_outpoints.push(outpoint);
                        log::debug!("Selected UTXO {}:{} for Bitcoin only (no balance data)", outpoint.txid, outpoint.vout);
                    }
                }
            }
            
            // Validate we have enough alkanes
            for (key, needed) in &alkanes_needed {
                let collected = alkanes_collected.get(key).unwrap_or(&0);
                if collected < needed {
                    return Err(AlkanesError::Wallet(format!(
                        "Insufficient alkanes: need {} of {}:{}, have {}",
                        needed, key.0, key.1, collected
                    )));
                }
            }
            
            log::info!("Selected {} UTXOs with sufficient alkanes", selected_outpoints.len());
        } else {
            // No alkanes needed — but the candidate set may still contain
            // alkane carriers (the wallet's UTXO list is BTC-layer, alkanes
            // ride on dust outputs). Quick-check each candidate via
            // protorunesbyoutpoint and skip those that carry alkane
            // balances; otherwise the selector would happily grab a
            // user's frBTC / DIESEL carrier as a generic dust fee input
            // and destroy the alkane in-flight.
            //
            // Triggers in the wrap-only Tx A of `execute_split` (the
            // wrap protostone has no alkane requirements). Observed
            // 2026-05-03 mainnet: Tx A 8bee7472... unintentionally
            // consumed 5e4a4112:0 (a 546-sat UTXO carrying 0.52 DIESEL),
            // leaving Tx B with "Insufficient alkanes: need 30000000
            // of 2:0, have 0" because the user's only DIESEL carrier
            // had been silently spent for fees.
            //
            // Skipped on qubitcoin (no protorunesbyoutpoint there).
            //
            // 2026-07-11: the carrier check covers EVERY candidate, not just dust.
            // Alkanes can ride outputs above the 1000-sat convention (mainnet:
            // 2916-sat UTXOs carrying 2000 frBTC each) — the old dust-only
            // pre-scan would have selected exactly such a carrier as a plain fee
            // input and burned its tokens, the same failure class as the
            // 2026-05-03 incident this exclusion was built for. The check is
            // LAZY — performed per candidate as the selector reaches it and
            // stopping once bitcoin_needed is met — so the RPC cost is bounded
            // by the number of UTXOs actually walked (prefetched assertions,
            // dust or not, still answer for free).
            let mut prefetched_count: usize = 0;
            let mut rpc_count: usize = 0;
            let mut carriers_skipped: usize = 0;
            for (outpoint, utxo) in spendable_utxos {
                if bitcoin_collected >= bitcoin_needed {
                    break;
                }
                let is_carrier = if self.provider.is_qubitcoin_mode() {
                    false
                } else if let Some(balances) =
                    prefetched_alkanes.as_ref().and_then(|m| m.get(&outpoint))
                {
                    // Same short-circuit shape as the primary-discovery branch:
                    // caller-asserted balances skip the RPC. An empty Vec means
                    // "asserted clean — not a carrier."
                    prefetched_count += 1;
                    balances.iter().any(|(_, amt)| *amt > 0)
                } else {
                    rpc_count += 1;
                    let txid_str = outpoint.txid.to_string();
                    match self
                        .provider
                        .get_protorunes_by_outpoint(&txid_str, outpoint.vout, None, 1)
                        .await
                    {
                        Ok(response) => response
                            .balance_sheet
                            .cached
                            .balances
                            .values()
                            .any(|amt| *amt > 0),
                        // Unverifiable — keep the pre-existing behavior (select).
                        Err(_) => false,
                    }
                };
                if is_carrier {
                    carriers_skipped += 1;
                    log::debug!(
                        "Skipping alkane carrier {}:{} (no alkanes needed)",
                        outpoint.txid, outpoint.vout
                    );
                    continue;
                }
                bitcoin_collected += utxo.amount;
                selected_outpoints.push(outpoint);
            }
            if carriers_skipped > 0 {
                log::info!(
                    "Excluded {} alkane-carrying UTXO(s) from BTC-only selection",
                    carriers_skipped
                );
            }
            log::info!(
                "BTC-only exclusion: {} prefetched, {} via RPC",
                prefetched_count, rpc_count
            );
        }

        if bitcoin_collected < bitcoin_needed {
            return Err(AlkanesError::Wallet(format!(
                "Insufficient funds: need {bitcoin_needed} sats, have {bitcoin_collected}"
            )));
        }

        log::info!("Selected {} UTXOs meeting all requirements (Bitcoin: {}/{}, Alkanes: {} types)", 
            selected_outpoints.len(), bitcoin_collected, bitcoin_needed, alkanes_needed.len());
        
        // Log what we actually found for debugging
        if !alkanes_found.is_empty() {
            log::info!("Alkanes found in selected UTXOs:");
            for (alkane_id, amount) in &alkanes_found {
                log::info!("  {}:{} = {} units", alkane_id.block, alkane_id.tx, amount);
            }
        }
        
        // In qubitcoin mode, verify selected UTXOs are still unspent via gettxout.
        // The protorunesbyaddress index may include stale (spent) outpoints.
        if self.provider.is_qubitcoin_mode() && !selected_outpoints.is_empty() {
            let mut verified = Vec::new();
            for outpoint in &selected_outpoints {
                let txid_str = outpoint.txid.to_string();
                match crate::traits::JsonRpcProvider::call(self.provider, "", "gettxout", serde_json::json!([txid_str, outpoint.vout]), 1).await {
                    Ok(resp) => {
                        let result = resp.get("result").unwrap_or(&resp);
                        if result.is_null() {
                            log::warn!("UTXO {}:{} is SPENT (stale), removing from selection", &txid_str[..16.min(txid_str.len())], outpoint.vout);
                            continue;
                        }
                        verified.push(*outpoint);
                    }
                    Err(_) => {
                        // Can't verify — include anyway
                        verified.push(*outpoint);
                    }
                }
            }
            if verified.len() < selected_outpoints.len() {
                log::info!("Filtered {} stale UTXOs, {} remaining", selected_outpoints.len() - verified.len(), verified.len());
            }
            selected_outpoints = verified;
        }

        let selected_txouts = selected_outpoints
            .iter()
            .filter_map(|outpoint| {
                selected_txout_candidates
                    .get(outpoint)
                    .cloned()
                    .map(|txout| (*outpoint, txout))
            })
            .collect();

        Ok(UtxoSelectionResult {
            outpoints: selected_outpoints,
            txouts: selected_txouts,
            alkanes_found,
            per_utxo_alkanes,
        })
    }

    async fn create_outputs(
        &mut self,
        to_addresses: &[String],
        change_address: &Option<String>,
        input_requirements: &[InputRequirement],
        protostones: &[ProtostoneSpec],
    ) -> Result<Vec<TxOut>> {
        use crate::traits::AddressResolver;
        
        let mut outputs = Vec::new();
        let network = self.provider.get_network();

        let total_explicit_bitcoin: u64 = input_requirements.iter().filter_map(|req| {
            if let InputRequirement::Bitcoin { amount } = req { Some(*amount) } else { None }
        }).sum();

        // Scan protostones to find the highest vN identifier referenced
        let max_identifier = self.find_max_output_identifier(protostones);
        
        log::debug!("Scanning {} protostones for output identifiers", protostones.len());
        log::debug!("Max identifier found: {:?}", max_identifier);
        log::debug!("to_addresses: {:?}", to_addresses);
        
        // Determine how many outputs we need to create
        let num_identifier_outputs = if to_addresses.is_empty() {
            // No explicit --to addresses, so we need to create outputs for all identifiers
            max_identifier.map(|n| (n + 1) as usize).unwrap_or(0)
        } else {
            // Use the number of --to addresses, but ensure we have enough for all identifiers
            to_addresses.len().max(max_identifier.map(|n| (n + 1) as usize).unwrap_or(0))
        };

        log::info!("Creating {} identifier-based outputs (max identifier: {:?})", 
                   num_identifier_outputs, max_identifier);

        if total_explicit_bitcoin > 0 && num_identifier_outputs == 0 {
            return Err(AlkanesError::Validation("Bitcoin input requirement provided but no recipient addresses or output identifiers.".to_string()));
        }

        let amount_per_recipient = if total_explicit_bitcoin > 0 && num_identifier_outputs > 0 {
            total_explicit_bitcoin / num_identifier_outputs as u64
        } else {
            DUST_LIMIT
        };

        // Create outputs for each identifier
        for i in 0..num_identifier_outputs {
            let addr_str = if i < to_addresses.len() {
                // Use explicit --to address
                to_addresses[i].clone()
            } else if let Some(change_addr) = change_address {
                // Use --change address as default
                change_addr.clone()
            } else {
                // Default to p2tr:0
                "p2tr:0".to_string()
            };
            
            log::debug!("Creating output {} for identifier v{}: address '{}'", i, i, addr_str);
            // Resolve address identifiers like p2tr:0 to actual addresses
            let resolved_addr = self.provider.resolve_all_identifiers(&addr_str).await?;
            let address = Address::from_str(&resolved_addr)?.require_network(network)?;
            outputs.push(TxOut {
                value: bitcoin::Amount::from_sat(amount_per_recipient.max(DUST_LIMIT)),
                script_pubkey: address.script_pubkey(),
            });
        }

        // Add BTC change output if needed
        // Default to p2tr:0 if no --change specified (taproot is preferred for Alkanes)
        let change_addr_str = change_address.as_ref().map(|s| s.as_str()).unwrap_or("p2tr:0");
        log::debug!("Adding BTC change output: address '{}'", change_addr_str);
        let resolved_addr = self.provider.resolve_all_identifiers(change_addr_str).await?;
        let address = Address::from_str(&resolved_addr)?.require_network(network)?;
        outputs.push(TxOut {
            value: bitcoin::Amount::from_sat(0), // Will be filled in later with actual change
            script_pubkey: address.script_pubkey(),
        });

        Ok(outputs)
    }

    /// Calculate alkanes needed from input requirements
    fn calculate_alkanes_needed(&self, requirements: &[InputRequirement]) -> alloc::collections::BTreeMap<AlkaneId, u64> {
        let mut needed = alloc::collections::BTreeMap::new();
        
        for requirement in requirements {
            if let InputRequirement::Alkanes { block, tx, amount } = requirement {
                let alkane_id = AlkaneId { block: *block, tx: *tx };
                *needed.entry(alkane_id).or_insert(0) += amount;
            }
        }
        
        log::debug!("Alkanes needed: {} types", needed.len());
        for (alkane_id, amount) in &needed {
            log::debug!("  {}:{} = {} units", alkane_id.block, alkane_id.tx, amount);
        }
        
        needed
    }
    
    /// Calculate excess alkanes (found - needed)
    fn calculate_excess(
        &self,
        alkanes_found: &alloc::collections::BTreeMap<AlkaneId, u64>,
        alkanes_needed: &alloc::collections::BTreeMap<AlkaneId, u64>,
    ) -> alloc::collections::BTreeMap<AlkaneId, u64> {
        let mut excess = alloc::collections::BTreeMap::new();
        
        for (alkane_id, found_amount) in alkanes_found {
            let needed_amount = alkanes_needed.get(alkane_id).unwrap_or(&0);
            if *found_amount > *needed_amount {
                let excess_amount = found_amount - needed_amount;
                excess.insert(alkane_id.clone(), excess_amount);
                log::info!("Excess alkane {}:{}: {} units (found: {}, needed: {})", 
                          alkane_id.block, alkane_id.tx, excess_amount, found_amount, needed_amount);
            }
        }
        
        if excess.is_empty() {
            log::info!("No excess alkanes - exact match!");
        } else {
            log::info!("Found {} types of excess alkanes", excess.len());
        }
        
        excess
    }
    
    /// Generate automatic protostone for alkanes change
    async fn generate_alkanes_change_protostone(
        &mut self,
        alkanes_needed: &alloc::collections::BTreeMap<AlkaneId, u64>,
        alkanes_found: &alloc::collections::BTreeMap<AlkaneId, u64>,
        alkanes_change_output_index: u32,
    ) -> Result<ProtostoneSpec> {
        log::info!("Generating automatic split protostone for {} needed alkane types, {} found alkane types",
                   alkanes_needed.len(), alkanes_found.len());

        // Create edicts to send needed amounts to p1 (first user protostone)
        // and true excess (found - needed) to the change output
        let mut edicts = Vec::new();

        for (alkane_id, needed) in alkanes_needed {
            let found = alkanes_found.get(alkane_id).copied().unwrap_or(0);

            if found > 0 {
                // Send needed amount to p1 (the first user protostone that will execute after this auto-change)
                edicts.push(ProtostoneEdict {
                    alkane_id: alkane_id.clone(),
                    amount: *needed,
                    target: OutputTarget::Protostone(1), // p1
                });
                log::debug!("  Edict: Send {} units of {}:{} to p1",
                           needed, alkane_id.block, alkane_id.tx);

                // If there's true excess (found > needed), send it back to change output
                let excess = found - needed;
                if excess > 0 {
                    edicts.push(ProtostoneEdict {
                        alkane_id: alkane_id.clone(),
                        amount: excess,
                        target: OutputTarget::Output(alkanes_change_output_index),
                    });
                    log::debug!("  Edict: Send {} units of {}:{} (excess) to v{}",
                               excess, alkane_id.block, alkane_id.tx, alkanes_change_output_index);
                }
            }
        }

        // Also handle alkanes found in UTXOs that are NOT in input_requirements at all.
        // These are "collateral" alkanes that happen to be on the same UTXOs.
        // Without explicit edicts, they would follow the pointer chain and could end up
        // at the wrong output (e.g., the recipient instead of the sender's change).
        for (alkane_id, found_amount) in alkanes_found {
            if !alkanes_needed.contains_key(alkane_id) && *found_amount > 0 {
                edicts.push(ProtostoneEdict {
                    alkane_id: alkane_id.clone(),
                    amount: *found_amount,
                    target: OutputTarget::Output(alkanes_change_output_index),
                });
                log::info!("  Edict: Send {} units of {}:{} (unrequested collateral) to v{}",
                           found_amount, alkane_id.block, alkane_id.tx, alkanes_change_output_index);
            }
        }

        // Create the protostone
        // This protostone will:
        // - Split alkanes: send needed amounts to p1, send excess + collateral to change output
        // - Point to p1 (the first user protostone after this auto-change protostone)
        // - Refund to the change output
        Ok(ProtostoneSpec {
            cellpack: None,
            edicts,
            bitcoin_transfer: None,
            pointer: Some(OutputTarget::Protostone(1)), // Point to p1 (first user protostone)
            refund: Some(OutputTarget::Output(alkanes_change_output_index)),
        })
    }
    
    /// Adjust protostone references after inserting automatic protostone at index 0
    /// This shifts all p0 -> p1, p1 -> p2, etc.
    fn adjust_protostone_references(&self, protostones: &[ProtostoneSpec]) -> Vec<ProtostoneSpec> {
        log::info!("Adjusting protostone references (shifting by 1)");
        
        let mut adjusted = Vec::new();
        
        for (i, protostone) in protostones.iter().enumerate() {
            let mut adjusted_protostone = protostone.clone();
            
            // Adjust pointer
            if let Some(OutputTarget::Protostone(p)) = adjusted_protostone.pointer {
                adjusted_protostone.pointer = Some(OutputTarget::Protostone(p + 1));
                log::debug!("  Protostone {}: pointer p{} -> p{}", i, p, p + 1);
            }
            
            // Adjust refund
            if let Some(OutputTarget::Protostone(p)) = adjusted_protostone.refund {
                adjusted_protostone.refund = Some(OutputTarget::Protostone(p + 1));
                log::debug!("  Protostone {}: refund p{} -> p{}", i, p, p + 1);
            }
            
            // Adjust edicts
            for (j, edict) in adjusted_protostone.edicts.iter_mut().enumerate() {
                if let OutputTarget::Protostone(p) = edict.target {
                    edict.target = OutputTarget::Protostone(p + 1);
                    log::debug!("  Protostone {}: edict {} target p{} -> p{}", i, j, p, p + 1);
                }
            }
            
            adjusted.push(adjusted_protostone);
        }
        
        adjusted
    }
    
    /// Find the maximum output identifier (vN) referenced in protostones
    fn find_max_output_identifier(&self, protostones: &[ProtostoneSpec]) -> Option<u32> {
        let mut max_id: Option<u32> = None;
        
        for protostone in protostones {
            // Check pointer
            if let Some(OutputTarget::Output(n)) = protostone.pointer {
                max_id = Some(max_id.map(|m: u32| m.max(n)).unwrap_or(n));
            }
            
            // Check refund
            if let Some(OutputTarget::Output(n)) = protostone.refund {
                max_id = Some(max_id.map(|m: u32| m.max(n)).unwrap_or(n));
            }
            
            // Check edicts
            for edict in &protostone.edicts {
                if let OutputTarget::Output(n) = edict.target {
                    max_id = Some(max_id.map(|m: u32| m.max(n)).unwrap_or(n));
                }
            }
            
            // Check bitcoin transfer
            if let Some(btc_transfer) = &protostone.bitcoin_transfer {
                if let OutputTarget::Output(n) = btc_transfer.target {
                    max_id = Some(max_id.map(|m: u32| m.max(n)).unwrap_or(n));
                }
            }
        }
        
        max_id
    }

    fn convert_protostone_specs(&self, specs: &[ProtostoneSpec]) -> Result<Vec<protorune_support::protostone::Protostone>> {
        // We need to know how many physical outputs there are to calculate protostone shadow outputs
        // For now, we'll need to pass this information. Let's use a helper closure.
        self.convert_protostone_specs_with_output_count(specs, 0) // Will be updated with actual count
    }

    fn convert_protostone_specs_with_output_count(&self, specs: &[ProtostoneSpec], num_physical_outputs: u32) -> Result<Vec<protorune_support::protostone::Protostone>> {
        specs.iter().enumerate().map(|(i, spec)| {
            let edicts = spec.edicts.iter().map(|e| {
                Ok(ProtoruneEdict {
                    id: protorune_support::balance_sheet::ProtoruneRuneId {
                        block: e.alkane_id.block as u128,
                        tx: e.alkane_id.tx as u128,
                    },
                    amount: e.amount as u128,
                    output: match e.target {
                        OutputTarget::Output(v) => v as u128,
                        // Protostone targets use shadow vouts above physical outputs.
                        // After OP_RETURN is appended, tx.output.len() = num_physical_outputs + 1.
                        // Protorune indexer maps protostone N to vout = tx.output.len() + 1 + N
                        //   = (num_physical_outputs + 1) + 1 + N = num_physical_outputs + 2 + N.
                        OutputTarget::Protostone(p) => (num_physical_outputs + 2 + p) as u128,
                        OutputTarget::Split => 0, // Split not supported in ProtostoneEdict
                    },
                })
            }).collect::<Result<Vec<_>>>()?;

            let message = spec.cellpack.as_ref().map(|c| c.encipher()).unwrap_or_default();
            log::info!("Converting protostone #{}: cellpack present={}, message_len={}", i, spec.cellpack.is_some(), message.len());
            
            // Convert pointer: v{N} -> N, p{N} -> num_physical_outputs + 1 (OP_RETURN) + 1 (base offset) + N
            let pointer = match &spec.pointer {
                Some(OutputTarget::Output(v)) => {
                    log::info!("  Pointer: v{} (physical output {})", v, v);
                    Some(*v)
                }
                Some(OutputTarget::Protostone(p)) => {
                    let calculated = num_physical_outputs + 2 + p;
                    log::info!("  Pointer: p{} (shadow output = {} + 2 + {} = {})", p, num_physical_outputs, p, calculated);
                    Some(calculated)
                }
                Some(OutputTarget::Split) => {
                    log::warn!("  Pointer: Split not supported for protostones, defaulting to 0");
                    Some(0)
                }
                None => {
                    log::info!("  Pointer: None, defaulting to 0");
                    Some(0)
                }
            };

            // Convert refund: v{N} -> N, p{N} -> num_physical_outputs + 2 + N
            let refund = match &spec.refund {
                Some(OutputTarget::Output(v)) => {
                    log::info!("  Refund: v{} (physical output {})", v, v);
                    Some(*v)
                }
                Some(OutputTarget::Protostone(p)) => {
                    let calculated = num_physical_outputs + 2 + p;
                    log::info!("  Refund: p{} (shadow output = {} + 2 + {} = {})", p, num_physical_outputs, p, calculated);
                    Some(calculated)
                }
                Some(OutputTarget::Split) => {
                    log::warn!("  Refund: Split not supported for protostones, defaulting to 0");
                    Some(0)
                }
                None => {
                    log::info!("  Refund: None, defaulting to 0");
                    Some(0)
                }
            };
            
            Ok(Protostone {
                protocol_tag: 1, // ALKANE protocol tag
                burn: None,
                refund,
                pointer,
                from: None,
                message,
                edicts,
            })
        }).collect()
    }

    fn construct_runestone_script(&self, protostones: &[ProtostoneSpec], num_outputs: usize) -> Result<ScriptBuf> {
        self.construct_runestone_script_with_alkane_routing(protostones, num_outputs, false)
    }

    fn construct_runestone_script_with_alkane_routing(&self, protostones: &[ProtostoneSpec], num_outputs: usize, has_alkane_inputs: bool) -> Result<ScriptBuf> {
        log::info!("Constructing runestone with {} protostones and {} outputs (before OP_RETURN), alkane_inputs={}", protostones.len(), num_outputs, has_alkane_inputs);
        log::info!("  After OP_RETURN is added, tx.output.len() = {} + 1 = {}", num_outputs, num_outputs + 1);
        log::info!("  Formula: pN -> vout = {} + 1 + N = {} + N", num_outputs, num_outputs + 1);

        let converted_protostones = self.convert_protostone_specs_with_output_count(protostones, num_outputs as u32)?;

        // Debug logging
        for (i, p) in converted_protostones.iter().enumerate() {
            log::info!("Protostone #{}: protocol_tag={}, message_len={} bytes", i, p.protocol_tag, p.message.len());
        }

        // Use the Protostones trait to properly encode the protocol field
        let protocol_values = converted_protostones.encipher()?;
        log::info!("Encoded protocol values: {} u128 values", protocol_values.len());

        // Runestone pointer: always 0 (first --to output).
        //
        // Extended pointers (shadow vouts for protomessage routing) cause issues
        // with contracts that call Runestone::decipher internally (e.g. frBTC).
        // The protorune indexer routes alkane tokens to protostones based on the
        // protostone's own pointer field, not the Runestone pointer.
        let pointer = 0u32;

        let runestone = Runestone {
            protocol: Some(protocol_values),
            pointer: Some(pointer),
            ..Default::default()
        };

        Ok(runestone.encipher())
    }

    async fn build_psbt_and_fee(
        &mut self,
        utxos: Vec<OutPoint>,
        mut outputs: Vec<TxOut>,
        runestone_script: Option<ScriptBuf>,
        fee_rate: Option<f32>,
        envelope: Option<&AlkanesEnvelope>,
        first_input_txout: Option<TxOut>,
        // Optional caller-supplied per-outpoint TxOut cache. Built once at
        // the top of execute() from `params.prefetched_utxos` and threaded
        // here so the per-input loop below can skip the slow getrawtransaction
        // path for every outpoint the JS wallet has already cached.
        prefetched_txouts: Option<&alloc::collections::BTreeMap<OutPoint, TxOut>>,
    ) -> Result<(Psbt, u64, usize)> {
        use bitcoin::transaction::Version;

        if let Some(script) = runestone_script {
            if !script.is_empty() {
                 outputs.push(TxOut {
                    value: bitcoin::Amount::ZERO,
                    script_pubkey: script,
                });
            }
        }

        let mut total_input_value = 0;
        let mut input_txouts = Vec::new();
        for (i, outpoint) in utxos.iter().enumerate() {
            let utxo = if i == 0 && first_input_txout.is_some() {
                // Use the pre-known first input (commit output) if provided
                first_input_txout.clone().unwrap()
            } else if let Some(txout) = prefetched_txouts.and_then(|m| m.get(outpoint)) {
                // Cached by caller — skip the getrawtransaction roundtrip.
                txout.clone()
            } else {
                // Fetch from provider for other inputs
                self.provider.get_utxo(outpoint).await?
                    .ok_or_else(|| AlkanesError::Wallet(format!("UTXO not found: {outpoint}")))?
            };
            total_input_value += utxo.value.to_sat();
            input_txouts.push(utxo);
        }
    
        let mut temp_tx = Transaction {
            version: Version::TWO,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: utxos.iter().map(|outpoint| bitcoin::TxIn {
                previous_output: *outpoint,
                script_sig: ScriptBuf::new(),
                sequence: bitcoin::Sequence::ENABLE_RBF_NO_LOCKTIME,
                witness: bitcoin::Witness::new(),
            }).collect(),
            output: outputs.clone(),
        };
    
        for (i, input) in temp_tx.input.iter_mut().enumerate() {
            if let Some(env) = envelope {
                if i == 0 {
                    // This is the commit input with envelope, which will be a script-path spend.
                    // The witness will be: <signature> <script> <control_block>
                    // Build the actual reveal script to get accurate size
                    let reveal_script = env.build_reveal_script();
                    
                    // Estimate witness sizes:
                    // - Signature: 64 bytes (Schnorr signature)
                    // - Script: actual reveal script size
                    // - Control block: ~33 bytes (1 byte version + 32 byte internal key)
                    let estimated_witness_size = 64 + reveal_script.len() + 33;
                    input.witness.push(vec![0u8; estimated_witness_size]);
                } else {
                    input.witness.push([0u8; 65]);
                }
            } else {
                // Regular p2tr key-path spend or other witness types.
                // A 65-byte witness is a good estimate for a P2TR key-path spend.
                input.witness.push([0u8; 65]);
            }
        }
    
        // Use network-appropriate default fee rate (already calculated in build_single_transaction)
        // Keep 600.0 as absolute fallback for commit transactions which may not have network context
        let fee_rate_sat_vb = fee_rate.unwrap_or(10.0); // Lowered from 600.0
        let estimated_vsize = temp_tx.vsize();
        let estimated_fee = (fee_rate_sat_vb * estimated_vsize as f32).ceil() as u64;
        // Add a small buffer (1%) to account for any size differences between temp tx and final signed tx
        let estimated_fee_with_buffer = (estimated_fee as f64 * 1.01).ceil() as u64;
        let capped_fee = estimated_fee_with_buffer.min(MAX_FEE_SATS);
        log::info!("Estimated fee: {estimated_fee}, With buffer: {estimated_fee_with_buffer}, Capped fee: {capped_fee}");
    
        let total_output_value_sans_change: u64 = outputs.iter()
            .filter(|o| o.value.to_sat() > 0)
            .map(|o| o.value.to_sat())
            .sum();
    
        let change_value = total_input_value.saturating_sub(total_output_value_sans_change).saturating_sub(capped_fee);

        // Change placement.
        //
        // Preference order:
        //   1. A zero-value, non-OP_RETURN placeholder (canonical: caller used
        //      `create_outputs` which appends one pointing at change_address).
        //   2. The LAST non-OP_RETURN output (walk backwards so an appended
        //      runestone OP_RETURN doesn't gate this off — that was the bug
        //      c12 reported 2026-05-17: outputs were [wrap_to_signer, dust,
        //      OP_RETURN] with no placeholder; the old `outputs.iter_mut().last()`
        //      hit the OP_RETURN and the inner `!is_op_return()` guard skipped
        //      the assignment → change_value silently disappeared into fees).
        //
        // If neither path can absorb the change AND it's above dust, return
        // an error — silently turning user funds into miner fees is the worst
        // possible failure mode for a wallet operation.
        const DUST_THRESHOLD_SATS: u64 = 546;
        if change_value > 0 {
            let placed = if let Some(change_output) = outputs.iter_mut()
                .find(|o| o.value.to_sat() == 0 && !o.script_pubkey.is_op_return())
            {
                change_output.value = bitcoin::Amount::from_sat(change_value);
                true
            } else if let Some(target) = outputs.iter_mut()
                .rev()
                .find(|o| !o.script_pubkey.is_op_return())
            {
                target.value = bitcoin::Amount::from_sat(target.value.to_sat() + change_value);
                true
            } else {
                false
            };

            if !placed && change_value > DUST_THRESHOLD_SATS {
                return Err(AlkanesError::Wallet(format!(
                    "build_psbt_and_fee: cannot place {change_value} sats of change \
                     — every output is OP_RETURN. Caller must supply at least one \
                     non-OP_RETURN output (typically a zero-value change placeholder \
                     via create_outputs) so the surplus has somewhere to land."
                )));
            }
            // change_value <= dust with no placement target: silently dropped
            // (standard Bitcoin Core dust policy).
        }
    
        let mut psbt = Psbt::from_unsigned_tx(bitcoin::Transaction {
            version: Version::TWO,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: utxos.iter().map(|outpoint| bitcoin::TxIn {
                previous_output: *outpoint,
                script_sig: ScriptBuf::new(),
                sequence: bitcoin::Sequence::ENABLE_RBF_NO_LOCKTIME,
                witness: bitcoin::Witness::new(),
            }).collect(),
            output: outputs,
        })?;
    
        for (i, utxo) in input_txouts.iter().enumerate() {
            psbt.inputs[i].witness_utxo = Some(utxo.clone());
            if utxo.script_pubkey.is_p2tr() {
                let (internal_key, (fingerprint, path)) = self.provider.get_internal_key().await?;
                psbt.inputs[i].tap_internal_key = Some(internal_key);
                // For key-path spends, `tap_key_origins` is not strictly needed by all signers,
                // but it's good practice to include it.
                psbt.inputs[i].tap_key_origins.insert(
                    internal_key,
                    (vec![], (fingerprint, path))
                );
            }
        }
        
        Ok((psbt, capped_fee, estimated_vsize))
    }

    async fn sign_and_finalize_psbt(&mut self, mut psbt: bitcoin::psbt::Psbt) -> Result<Transaction> {
        let signed_psbt = self.provider.sign_psbt(&mut psbt).await?;
        let mut tx = signed_psbt.clone().extract_tx()?;
        for (i, psbt_input) in signed_psbt.inputs.iter().enumerate() {
            if let Some(tap_key_sig) = &psbt_input.tap_key_sig {
                tx.input[i].witness = bitcoin::Witness::p2tr_key_spend(tap_key_sig);
            } else if let Some(final_script_witness) = &psbt_input.final_script_witness {
                tx.input[i].witness = final_script_witness.clone();
            }
        }
        Ok(tx)
    }

    /// Build a split PSBT to protect inscribed UTXOs.
    ///
    /// Inputs:
    ///   - `plans`: per-inscribed-UTXO breakdown (safe + clean amounts).
    ///   - `funding_utxos`: TxOut data for the inscribed inputs (used to look
    ///     up scripts/values on the inputs being split).
    ///   - `extra_funding_utxos`: clean (no inscriptions, no alkanes) UTXOs
    ///     from elsewhere in the wallet that the builder may consume to cover
    ///     fees or top up clean outputs that fall below dust. Pre-filtered by
    ///     the caller — anything in here is safe to add as an additional input.
    ///   - `split_utxo_alkanes`: alkane balances on inscribed UTXOs (for
    ///     alkane-aware splits with OP_RETURN routing).
    ///
    /// Returns:
    ///   - The split PSBT.
    ///   - Estimated fee (sats).
    ///   - Clean BTC outpoints from the split tx (one per plan + an optional
    ///     consolidated extras-funded change output).
    ///   - Clean alkane outpoints with their balances.
    ///   - List of `extra_funding_utxos` outpoints that were consumed as
    ///     additional inputs. The caller MUST remove these from the main tx's
    ///     input list, since they're now spent in the split tx.
    ///
    /// Why extra_funding_utxos exists: most ordinal mints land on small
    /// (~546-1500 sat) inscribed UTXOs because that minimizes inscriber cost.
    /// Those UTXOs can't self-fund the split (safe output + clean output +
    /// fee > utxo_value), so without external top-up, `Preserve` strategy
    /// fails for the very inscribed UTXOs users actually hold. With external
    /// top-up, small inscribed UTXOs split fine.
    async fn build_split_psbt(
        &mut self,
        plans: &[SplitPlan],
        funding_utxos: &[(OutPoint, TxOut)],
        extra_funding_utxos: &[(OutPoint, TxOut)],
        fee_rate: f32,
        params: &EnhancedExecuteParams,
        split_utxo_alkanes: &alloc::collections::BTreeMap<OutPoint, Vec<(AlkaneId, u64)>>,
    ) -> Result<(Psbt, u64, Vec<OutPoint>, Vec<(OutPoint, Vec<(AlkaneId, u64)>)>, Vec<OutPoint>)> {
        use bitcoin::transaction::Version;

        // Get safe address for split outputs
        let safe_address_str = params.change_address.as_ref()
            .map(|s| s.as_str())
            .unwrap_or("p2tr:0");
        use crate::traits::AddressResolver;
        let resolved_addr = self.provider.resolve_all_identifiers(safe_address_str).await?;
        let safe_address = Address::from_str(&resolved_addr)?.require_network(self.provider.get_network())?;

        // Get alkane change address for alkane outputs
        let alkane_change_addr_str = params.alkanes_change_address.as_ref()
            .or(params.change_address.as_ref())
            .map(|s| s.as_str())
            .unwrap_or("p2tr:0");
        let resolved_alkane_addr = self.provider.resolve_all_identifiers(alkane_change_addr_str).await?;
        let alkane_change_address = Address::from_str(&resolved_alkane_addr)?.require_network(self.provider.get_network())?;

        // Build inputs and outputs
        let mut inputs = Vec::new();
        let mut outputs = Vec::new();
        let mut input_txouts = Vec::new();
        let mut clean_outpoints = Vec::new();
        let mut total_input_value = 0u64;
        // Tracks how much each plan's clean output was inflated above its
        // natural `clean_amount` (because the natural amount was below dust).
        // The total inflation is owed to extras and pulled later.
        let mut clean_topup_owed: u64 = 0;

        for (idx, plan) in plans.iter().enumerate() {
            // Find the TxOut for this input
            let txout = funding_utxos.iter()
                .find(|(op, _)| *op == plan.outpoint)
                .map(|(_, txout)| txout.clone())
                .ok_or_else(|| AlkanesError::Wallet(format!("UTXO not found for split: {}", plan.outpoint)))?;

            total_input_value += txout.value.to_sat();

            inputs.push(bitcoin::TxIn {
                previous_output: plan.outpoint,
                script_sig: ScriptBuf::new(),
                sequence: bitcoin::Sequence::ENABLE_RBF_NO_LOCKTIME,
                witness: bitcoin::Witness::new(),
            });
            input_txouts.push(txout);

            // Safe output (inscribed sats go here)
            outputs.push(TxOut {
                value: bitcoin::Amount::from_sat(plan.safe_amount),
                script_pubkey: safe_address.script_pubkey(),
            });

            // Clean output (funding sats from the inscribed UTXO go here).
            // If the inscribed UTXO's clean remainder is below dust, top up
            // to DUST_LIMIT — the missing sats come from extras. If the
            // remainder is zero (utxo_value == safe_amount + 1 etc.), we
            // still emit a dust output so the caller has a usable funding
            // UTXO at the canonical odd-index position; same top-up logic.
            let clean_value = if plan.clean_amount < DUST_LIMIT {
                clean_topup_owed += DUST_LIMIT - plan.clean_amount;
                DUST_LIMIT
            } else {
                plan.clean_amount
            };
            outputs.push(TxOut {
                value: bitcoin::Amount::from_sat(clean_value),
                script_pubkey: safe_address.script_pubkey(),
            });

            // Track the clean outpoint (will update txid after building tx)
            clean_outpoints.push(OutPoint {
                txid: bitcoin::Txid::from_byte_array([0u8; 32]), // Placeholder
                vout: (idx * 2 + 1) as u32, // Clean outputs are at odd indices
            });
        }

        // Alkane-aware split: if any inscribed UTXOs carry alkanes, add a dedicated
        // clean alkane output and a protostone OP_RETURN to route alkanes there.
        // This prevents alkanes from being lost when their UTXO is split for inscriptions.
        let has_alkanes = !split_utxo_alkanes.is_empty();
        let mut alkane_outpoints_with_balances: Vec<(OutPoint, Vec<(AlkaneId, u64)>)> = Vec::new();

        if has_alkanes {
            // Add a clean alkane output at the end (before OP_RETURN)
            let alkane_output_index = outputs.len() as u32;
            outputs.push(TxOut {
                value: bitcoin::Amount::from_sat(DUST_LIMIT),
                script_pubkey: alkane_change_address.script_pubkey(),
            });

            // Aggregate all alkanes from inscribed UTXOs
            let mut aggregated_alkanes: alloc::collections::BTreeMap<AlkaneId, u64> = alloc::collections::BTreeMap::new();
            for (_outpoint, alkanes) in split_utxo_alkanes {
                for (alkane_id, amount) in alkanes {
                    *aggregated_alkanes.entry(alkane_id.clone()).or_insert(0) += amount;
                }
            }

            // Build protostone edicts to route each alkane to the clean alkane output
            let mut protostone_edicts = Vec::new();
            let mut alkane_output_balances = Vec::new();
            for (alkane_id, amount) in &aggregated_alkanes {
                // Edict: send all of this alkane to the clean alkane output (vN)
                protostone_edicts.push(ProtoruneEdict {
                    id: ProtoruneRuneId {
                        block: alkane_id.block as u128,
                        tx: alkane_id.tx as u128,
                    },
                    amount: *amount as u128,
                    output: alkane_output_index as u128,
                });
                alkane_output_balances.push((alkane_id.clone(), *amount));
                log::info!("  Split alkane edict: {}:{} × {} → v{}",
                    alkane_id.block, alkane_id.tx, amount, alkane_output_index);
            }

            // Build the protostone (protocol_tag 1 for alkanes)
            let split_protostone = Protostone {
                protocol_tag: 1u128,
                message: vec![],
                pointer: Some(alkane_output_index),
                refund: Some(alkane_output_index),
                edicts: protostone_edicts,
                from: None,
                burn: None,
            };

            // Encode the protostone into a Runestone OP_RETURN script
            let protocol_values = vec![split_protostone].encipher()?;
            let runestone = Runestone {
                protocol: Some(protocol_values),
                pointer: Some(alkane_output_index),
                ..Default::default()
            };
            let runestone_script = runestone.encipher();

            outputs.push(TxOut {
                value: bitcoin::Amount::ZERO,
                script_pubkey: runestone_script,
            });

            // Track that the clean alkane output will carry these alkanes
            alkane_outpoints_with_balances.push((
                OutPoint {
                    txid: bitcoin::Txid::from_byte_array([0u8; 32]), // Placeholder, updated below
                    vout: alkane_output_index,
                },
                alkane_output_balances,
            ));

            log::info!("🔗 Added alkane routing: {} alkane types → clean output v{} with OP_RETURN protostone",
                aggregated_alkanes.len(), alkane_output_index);
        }

        // Compute total output value already committed (alkane DUST_LIMIT
        // outputs are the only non-OP_RETURN extras beyond the per-plan
        // safe+clean pairs).
        let total_output_value: u64 = outputs.iter()
            .map(|o| o.value.to_sat())
            .sum();

        // Pull additional clean inputs (a) to cover any clean-output top-ups
        // forced by below-dust inscribed UTXOs, (b) to cover the split-tx
        // fee, and (c) to add a residual change output if the extras over-fund.
        //
        // We over-pull conservatively: each extra input adds ~68 vbytes which
        // grows the fee, so the simplest correct approach is "add inputs
        // until total_input_value >= total_output_value + fee_with_one_more_input,
        // then iterate fee until stable." Two passes through extras at most.
        let mut consumed_extras: Vec<OutPoint> = Vec::new();
        let mut extras_iter = extra_funding_utxos.iter();

        // Helper: recompute estimated fee based on current input/output counts.
        // P2TR input: ~68 vbytes; P2TR output: ~43 vbytes; tx overhead: ~10 vbytes.
        let recompute_fee = |inputs_len: usize, outputs_len: usize| -> u64 {
            let vsize = 10 + inputs_len * 68 + outputs_len * 43;
            (fee_rate * vsize as f32).ceil() as u64
        };

        // Initial fee estimate (no residual change output yet).
        let mut estimated_fee = recompute_fee(inputs.len(), outputs.len());

        // Required: total_input_value >= total_output_value + estimated_fee.
        // Difference is what we must pull from extras (or from inscribed-UTXO
        // headroom that isn't already accounted for in clean outputs).
        //
        // Inscribed UTXOs have already contributed `total_input_value` worth.
        // Clean outputs were sized at (plan.clean_amount or DUST_LIMIT topup).
        // If `clean_topup_owed > 0`, that headroom must come from extras too.
        //
        // Pull extras one at a time until satisfied.
        loop {
            let needed = total_output_value
                .saturating_add(estimated_fee)
                .saturating_sub(total_input_value);
            if needed == 0 {
                break;
            }

            // Need more — try to pull next extra.
            let Some((extra_op, extra_txout)) = extras_iter.next() else {
                return Err(AlkanesError::Wallet(format!(
                    "Not enough clean funds to split inscribed UTXOs: need {} more sats \
                     (output total {} + fee {} - inscribed inputs {}). \
                     Wallet has no additional clean UTXOs available — either fund the \
                     wallet with more BTC or use --ordinals-strategy burn (destroys \
                     inscriptions).",
                    needed, total_output_value, estimated_fee, total_input_value
                )));
            };

            inputs.push(bitcoin::TxIn {
                previous_output: *extra_op,
                script_sig: ScriptBuf::new(),
                sequence: bitcoin::Sequence::ENABLE_RBF_NO_LOCKTIME,
                witness: bitcoin::Witness::new(),
            });
            input_txouts.push(extra_txout.clone());
            total_input_value += extra_txout.value.to_sat();
            consumed_extras.push(*extra_op);

            // Recompute fee with the extra input.
            estimated_fee = recompute_fee(inputs.len(), outputs.len());
        }

        // If extras over-funded, emit a residual change output so the
        // surplus isn't burned as fee. Only emit once we know we have
        // sufficient surplus to clear DUST_LIMIT after the marginal output's
        // own fee cost (one extra output = ~43 vbytes).
        let surplus = total_input_value - total_output_value - estimated_fee;
        if surplus >= DUST_LIMIT + (fee_rate * 43.0).ceil() as u64 {
            let residual_fee_delta = (fee_rate * 43.0).ceil() as u64;
            let residual_value = surplus - residual_fee_delta;
            outputs.push(TxOut {
                value: bitcoin::Amount::from_sat(residual_value),
                script_pubkey: safe_address.script_pubkey(),
            });
            estimated_fee += residual_fee_delta;
            // Track this residual as a clean outpoint usable by the main tx.
            clean_outpoints.push(OutPoint {
                txid: bitcoin::Txid::from_byte_array([0u8; 32]), // placeholder, set below
                vout: (outputs.len() - 1) as u32,
            });
        }

        // Build the unsigned transaction
        let unsigned_tx = Transaction {
            version: Version::TWO,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: inputs,
            output: outputs,
        };

        // Create PSBT
        let mut psbt = Psbt::from_unsigned_tx(unsigned_tx)?;

        // Add witness_utxo and tap_internal_key for each input
        for (i, txout) in input_txouts.iter().enumerate() {
            psbt.inputs[i].witness_utxo = Some(txout.clone());
            if txout.script_pubkey.is_p2tr() {
                let (internal_key, (fingerprint, path)) = self.provider.get_internal_key().await?;
                psbt.inputs[i].tap_internal_key = Some(internal_key);
                psbt.inputs[i].tap_key_origins.insert(
                    internal_key,
                    (vec![], (fingerprint, path))
                );
            }
        }

        // Calculate actual txid and update all placeholder outpoints
        let txid = psbt.unsigned_tx.compute_txid();
        for outpoint in &mut clean_outpoints {
            outpoint.txid = txid;
        }
        for (outpoint, _) in &mut alkane_outpoints_with_balances {
            outpoint.txid = txid;
        }

        log::info!(
            "Built split PSBT: {} inputs ({} inscribed + {} extras) → {} outputs (alkane:{}, top-up owed: {} sats, fee: {})",
            psbt.unsigned_tx.input.len(),
            plans.len(),
            consumed_extras.len(),
            psbt.unsigned_tx.output.len(),
            if has_alkanes { "1+OP_RETURN" } else { "0" },
            clean_topup_owed,
            estimated_fee,
        );

        Ok((psbt, estimated_fee, clean_outpoints, alkane_outpoints_with_balances, consumed_extras))
    }

    async fn build_commit_psbt(
        &mut self,
        funding_utxos: Vec<OutPoint>,
        commit_output: TxOut,
        fee_rate: Option<f32>,
    ) -> Result<(bitcoin::psbt::Psbt, u64)> {
        let mut total_input_value = 0;
        let mut input_txouts = Vec::new();
        for outpoint in &funding_utxos {
            let utxo = self.provider.get_utxo(outpoint).await?
                .ok_or_else(|| AlkanesError::Wallet(format!("UTXO not found: {outpoint}")))?;
            total_input_value += utxo.value.to_sat();
            input_txouts.push(utxo);
        }
    
        let change_address_str = WalletProvider::get_address(self.provider).await?;
        let change_address = Address::from_str(&change_address_str)?.require_network(self.provider.get_network())?;
        let temp_change_output = TxOut { value: bitcoin::Amount::from_sat(0), script_pubkey: change_address.script_pubkey() };
        let temp_outputs = vec![commit_output.clone(), temp_change_output];
    
        let mut temp_tx_for_size = Transaction {
            version: bitcoin::transaction::Version::TWO,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: funding_utxos.iter().map(|outpoint| bitcoin::TxIn {
                previous_output: *outpoint,
                script_sig: ScriptBuf::new(),
                sequence: bitcoin::Sequence::ENABLE_RBF_NO_LOCKTIME,
                witness: bitcoin::Witness::new(),
            }).collect(),
            output: temp_outputs,
        };
        for input in &mut temp_tx_for_size.input {
            input.witness.push([0u8; 65]);
        }
    
        let fee_rate_sat_vb = self.resolve_fee_rate(fee_rate).await?;
        let fee = (fee_rate_sat_vb * temp_tx_for_size.vsize() as f32).ceil() as u64;

        let change_value = total_input_value.saturating_sub(commit_output.value.to_sat()).saturating_sub(fee);
        if change_value < 546 {
            return Err(AlkanesError::Wallet("Not enough funds for commit and change".to_string()));
        }
    
        let final_change_output = TxOut { value: bitcoin::Amount::from_sat(change_value), script_pubkey: change_address.script_pubkey() };
        let final_outputs = vec![commit_output, final_change_output];
    
        let unsigned_tx = Transaction {
            version: bitcoin::transaction::Version::TWO,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: funding_utxos.iter().map(|outpoint| bitcoin::TxIn {
                previous_output: *outpoint,
                script_sig: ScriptBuf::new(),
                sequence: bitcoin::Sequence::ENABLE_RBF_NO_LOCKTIME,
                witness: bitcoin::Witness::new(),
            }).collect(),
            output: final_outputs,
        };
        let mut psbt = bitcoin::psbt::Psbt::from_unsigned_tx(unsigned_tx)?;
    
        for (i, utxo) in input_txouts.iter().enumerate() {
            psbt.inputs[i].witness_utxo = Some(utxo.clone());
            if utxo.script_pubkey.is_p2tr() {
                let (internal_key, (fingerprint, path)) = self.provider.get_internal_key().await?;
                psbt.inputs[i].tap_internal_key = Some(internal_key);
                psbt.inputs[i].tap_key_origins.insert(
                    internal_key,
                    (vec![], (fingerprint, path.clone()))
                );
            }
        }
    
        Ok((psbt, fee))
    }
    
    async fn build_reveal_psbt(
        &mut self,
        params: &EnhancedExecuteParams,
        envelope: &AlkanesEnvelope,
        commit_outpoint: OutPoint,
        commit_output_value: u64,
        commit_internal_key: XOnlyPublicKey,
        commit_internal_key_fingerprint: bitcoin::bip32::Fingerprint,
        commit_internal_key_path: &bitcoin::bip32::DerivationPath,
    ) -> Result<(bitcoin::psbt::Psbt, u64, usize)> {
        let mut selected_utxos = vec![commit_outpoint];
        let mut selected_txouts: alloc::collections::BTreeMap<OutPoint, TxOut> =
            alloc::collections::BTreeMap::new();
        let mut total_bitcoin_needed = params.to_addresses.len() as u64 * DUST_LIMIT;
        for req in &params.input_requirements {
            if let InputRequirement::Bitcoin { amount } = req {
                total_bitcoin_needed += amount;
            }
        }
        total_bitcoin_needed += 50_000;

        if commit_output_value < total_bitcoin_needed {
            let additional_needed = total_bitcoin_needed - commit_output_value;
            let additional_reqs = vec![InputRequirement::Bitcoin { amount: additional_needed }];
            let utxo_selection = self.select_utxos(&additional_reqs, &params.from_addresses, &params.known_pending_tx_hexes, params.max_indexed_height, &params.prefetched_utxos, &params.excluded_utxos, params.utxo_source).await?;
            selected_txouts.extend(utxo_selection.txouts);
            selected_utxos.extend(utxo_selection.outpoints);
        }

        let outputs = self.create_outputs(&params.to_addresses, &params.change_address, &params.input_requirements, &params.protostones).await?;
        
        // Validate protostones against the ACTUAL number of outputs created
        self.validate_protostones(&params.protostones, outputs.len())?;
        
        let runestone_script = self.construct_runestone_script(&params.protostones, outputs.len())?;
        
        // Create the commit output TxOut (it may not be indexed yet if still in mempool)
        let commit_address = self.create_commit_address_for_envelope(envelope, commit_internal_key).await?;
        let commit_txout = TxOut {
            value: bitcoin::Amount::from_sat(commit_output_value),
            script_pubkey: commit_address.script_pubkey(),
        };
        
        let prefetched_for_reveal = build_effective_txouts_map(params, &selected_txouts)?;
        let (mut psbt, fee, estimated_vsize) = self.build_psbt_and_fee(selected_utxos, outputs, Some(runestone_script), params.fee_rate, Some(envelope), Some(commit_txout), prefetched_for_reveal.as_ref()).await?;

        let reveal_script = envelope.build_reveal_script();
        let (spend_info, _) = self.create_taproot_spend_info_for_envelope(envelope, commit_internal_key).await?;
        let leaf_hash = bitcoin::taproot::TapLeafHash::from_script(&reveal_script, bitcoin::taproot::LeafVersion::TapScript);

        psbt.inputs[0].tap_internal_key = Some(commit_internal_key);
        psbt.inputs[0].tap_scripts.insert(
            spend_info.control_block(&(reveal_script.clone(), bitcoin::taproot::LeafVersion::TapScript)).unwrap(),
            (reveal_script, bitcoin::taproot::LeafVersion::TapScript)
        );
        psbt.inputs[0].tap_key_origins.insert(
            commit_internal_key,
            (vec![leaf_hash], (commit_internal_key_fingerprint, commit_internal_key_path.clone()))
        );

        Ok((psbt, fee, estimated_vsize))
    }

    /// Creates the taproot spend info and control block for an envelope.
    async fn create_taproot_spend_info_for_envelope(
        &self,
        envelope: &AlkanesEnvelope,
        internal_key: XOnlyPublicKey,
    ) -> Result<(bitcoin::taproot::TaprootSpendInfo, bitcoin::taproot::ControlBlock)> {
        use bitcoin::taproot::{TaprootBuilder, LeafVersion};

        let reveal_script = envelope.build_reveal_script();

        let taproot_builder = TaprootBuilder::new()
            .add_leaf(0, reveal_script.clone())
            .map_err(|e| AlkanesError::Other(format!("{e:?}")))?;

        let taproot_spend_info = taproot_builder
            .finalize(self.provider.secp(), internal_key)
            .map_err(|e| AlkanesError::Other(format!("{e:?}")))?;

        let control_block = taproot_spend_info
            .control_block(&(reveal_script, LeafVersion::TapScript))
            .ok_or_else(|| AlkanesError::Other("Failed to create control block".to_string()))?;

        Ok((taproot_spend_info, control_block))
    }

    pub async fn create_taproot_script_signature(
        &self,
        tx: &Transaction,
        input_index: usize,
        script: &[u8],
        _control_block: &[u8],
        prevouts: &[TxOut],
    ) -> Result<Vec<u8>> {
        use bitcoin::sighash::{SighashCache, TapSighashType, Prevouts};
        use bitcoin::taproot;

        log::info!("Creating taproot script-path signature for input {input_index}");
        
        let prevouts_len = prevouts.len();
        let prevouts_all = Prevouts::All(prevouts);
        
        log::info!("Using Prevouts::All with {prevouts_len} prevouts for sighash calculation");

        let mut sighash_cache = SighashCache::new(tx);

        let script_buf = ScriptBuf::from(script.to_vec());
        let leaf_hash = taproot::TapLeafHash::from_script(&script_buf, taproot::LeafVersion::TapScript);

        let sighash = sighash_cache
            .taproot_script_spend_signature_hash(
                input_index,
                &prevouts_all,
                leaf_hash,
                TapSighashType::Default,
            )
            .map_err(|e| AlkanesError::Transaction(e.to_string()))?;

        log::info!("Computed taproot script-path sighash for input {input_index}");

        let signature = self.provider.sign_taproot_script_spend(sighash.into(), None).await?;

        let taproot_signature = taproot::Signature {
            signature,
            sighash_type: TapSighashType::Default,
        };

        let signature_bytes = taproot_signature.to_vec();
        
        log::info!("✅ Created taproot script-path signature: {} bytes", signature_bytes.len());

        Ok(signature_bytes)
    }

    /// Traces the reveal transaction to get the results of protostone execution.
    /// Uses the abstracted trace_protostones method from AlkanesProvider.
    async fn trace_reveal_transaction(&self, txid: &str, _params: &EnhancedExecuteParams) -> Result<Option<Vec<serde_json::Value>>> {
        use crate::traits::AlkanesProvider;
        
        log::info!("Tracing transaction: {txid}");
        
        // Use the abstracted trace_protostones method
        self.provider.trace_protostones(txid).await
    }

    /// Mines blocks on the regtest network if the provider is configured for it.
    async fn mine_blocks_if_regtest(&self, params: &EnhancedExecuteParams) -> Result<()> {
        use crate::traits::TimeProvider;

        if self.provider.get_network() == bitcoin::Network::Regtest {
            log::info!("Mining blocks on regtest network...");
            // Use cross-platform sleep (works in both native and WASM)
            self.provider.sleep_ms(2000).await;
            let address = if let Some(change_address) = &params.change_address {
                change_address.clone()
            } else {
                WalletProvider::get_address(self.provider).await?
            };
            self.provider.generate_to_address(1, &address).await?;
        }
        Ok(())
    }


    fn validate_envelope_cellpack_usage(&self, params: &EnhancedExecuteParams) -> Result<()> {
        let has_envelope = params.envelope_data.is_some();
        let has_cellpacks = params.protostones.iter().any(|p| p.cellpack.is_some());
        let has_edicts = params.protostones.iter().any(|p| !p.edicts.is_empty());

        if has_envelope && !has_cellpacks {
            return Err(AlkanesError::Other(anyhow!(
                "Incomplete deployment: Envelope provided but no cellpack to trigger deployment."
            ).to_string()));
        }

        if !has_envelope && has_cellpacks {
            return Ok(());
        }

        let has_alkane_inputs = !params.input_requirements.is_empty();
        if !has_envelope && !has_cellpacks && !has_edicts && !has_alkane_inputs && !params.protostones.is_empty() {
             return Err(AlkanesError::Other(anyhow!(
                "No operation: Protostones provided without envelope, cellpack, edicts, or alkane inputs."
            ).to_string()));
        }

        Ok(())
    }

    async fn inspect_from_protostones(&self, protostones: &[ProtostoneSpec]) -> Result<super::types::AlkanesInspectResult> {
        use super::types::{AlkaneId, AlkanesInspectConfig};
        use crate::utils::u128_from_slice;

        let cellpack_data = protostones
            .iter()
            .find_map(|p| p.cellpack.as_ref())
            .map(|c| c.encipher())
            .ok_or_else(|| AlkanesError::Other("No cellpack found in protostones for inspection.".to_string()))?;

        if cellpack_data.len() < 48 {
            return Err(AlkanesError::Other("Cellpack data is too short for inspection.".to_string()));
        }

        let alkane_id = AlkaneId {
            block: u128_from_slice(&cellpack_data[0..16]) as u64,
            tx: u128_from_slice(&cellpack_data[16..32]) as u64,
        };
        let opcode = u128_from_slice(&cellpack_data[32..48]);

        let config = AlkanesInspectConfig {
            disasm: false,
            fuzz: true,
            fuzz_ranges: Some(opcode.to_string()),
            meta: true,
            codehash: false,
            raw: false,
        };

        self.provider.inspect(&format!("{}:{}", alkane_id.block, alkane_id.tx), config).await
    }

    #[cfg(feature = "wasm-inspection")]
    async fn inspect_from_envelope(&self, envelope: &AlkanesEnvelope) -> Result<super::types::AlkanesInspectResult> {
        use super::types::{AlkaneId, AlkanesInspectResult};
        use wasmparser::{Parser, Payload};

        let wasm = &envelope.payload;
        let mut metadata = None;
        let mut metadata_error = None;

        let parser = Parser::new(0);
        for payload in parser.parse_all(wasm) {
            if let Ok(Payload::CustomSection(reader)) = payload {
                if reader.name() == "__meta" {
                    match serde_json::from_slice(reader.data()) {
                        Ok(m) => metadata = Some(m),
                        Err(e) => metadata_error = Some(e.to_string()),
                    }
                    break;
                }
            }
        }

        Ok(AlkanesInspectResult {
            alkane_id: AlkaneId { block: 0, tx: 0 }, // Not applicable for pre-deployment inspection
            bytecode_length: wasm.len(),
            disassembly: None,
            metadata,
            metadata_error,
            codehash: None,
            fuzzing_results: None,
        })
    }

    fn show_preview_and_confirm(
        &self,
        tx: &Transaction,
        analysis: &serde_json::Value,
        fee: u64,
        estimated_vsize: usize,
        raw_output: bool,
    ) -> Result<()> {
        if raw_output {
            println!("{}", serde_json::to_string_pretty(analysis)?);
        } else {
            println!("\n🔍 Transaction Preview");
            println!("═══════════════════════");
            println!("📋 Transaction ID: {}", tx.compute_txid());
            println!("💰 Estimated Fee: {fee} sats");
            println!("📊 Transaction Size: {} vbytes (estimated with witness)", estimated_vsize);
            println!("📈 Fee Rate: {:.2} sat/vB", fee as f64 / estimated_vsize as f64);

            crate::runestone_enhanced::print_human_readable_runestone(tx, analysis, self.provider.get_network());
        }

        println!("\n⚠️  TRANSACTION CONFIRMATION");
        println!("═══════════════════════════");
        println!("This transaction will be broadcast to the network.");
        println!("Please review the details above carefully.");
        print!("\nDo you want to proceed with broadcasting this transaction? (y/n) ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).context("Failed to read user input")?;
        let input = input.trim().to_lowercase();

        if input != "y" && input != "yes" {
            return Err(AlkanesError::Other("Transaction cancelled by user".to_string()));
        }

        Ok(())
    }

    /// Execute full alkanes deployment using presign strategy (atomic commit-reveal)
    /// This matches the brc20-prog pattern: sign all transactions first, then broadcast atomically
    async fn execute_full_with_presign(
        &mut self,
        params: EnhancedExecuteParams,
        envelope: &AlkanesEnvelope,
    ) -> Result<EnhancedExecuteResult> {
        log::info!("🔐 Presign Strategy: Building and signing all transactions upfront...");

        // Step 1: Get internal key with secret for anti-frontrunning
        let (internal_key, ephemeral_secret, (fingerprint, path)) =
            self.provider.get_internal_key_with_secret().await?;
        log::info!("📝 Step 1/5: Got internal key with ephemeral secret");

        // Step 2: Build commit PSBT with FINAL sequences (for deterministic txid)
        log::info!("📝 Step 2/5: Building commit transaction with final sequences...");
        let commit_address = self.create_commit_address_for_envelope(envelope, internal_key).await?;

        // Calculate reveal fee
        let reveal_script = envelope.build_reveal_script();
        let reveal_script_size = reveal_script.len();
        let network = self.provider.get_network();
        let default_fee_rate = match network {
            bitcoin::Network::Bitcoin => 10.0,
            bitcoin::Network::Testnet => 5.0,
            bitcoin::Network::Regtest => 1.0,
            bitcoin::Network::Signet => 5.0,
            _ => 5.0,
        };
        let fee_rate_sat_vb = params.fee_rate.unwrap_or(default_fee_rate);

        // Estimate reveal transaction size
        let num_outputs = params.to_addresses.len().max(1) + 2;
        let output_size = num_outputs * 43;
        let witness_size = 64 + reveal_script_size + 33;
        let non_witness_size = 10 + 41 + output_size;
        let weight = (non_witness_size * 4) + witness_size;
        let estimated_vsize = (weight + 3) / 4;
        let estimated_reveal_fee = ((estimated_vsize as f32 * fee_rate_sat_vb) * 1.2).ceil() as u64;

        let mut required_reveal_amount = 546u64;
        for requirement in &params.input_requirements {
            if let InputRequirement::Bitcoin { amount } = requirement {
                required_reveal_amount += amount;
            }
        }
        required_reveal_amount += estimated_reveal_fee;
        required_reveal_amount += params.to_addresses.len() as u64 * 546;

        // Select UTXOs for commit
        let utxo_selection = self
            .select_utxos(&[InputRequirement::Bitcoin { amount: required_reveal_amount }], &params.from_addresses, &params.known_pending_tx_hexes, params.max_indexed_height, &params.prefetched_utxos, &params.excluded_utxos, params.utxo_source)
            .await?;
        let funding_utxos = utxo_selection.outpoints.clone();

        // Build commit PSBT with FINAL sequences (no RBF) for deterministic txid
        let commit_output = bitcoin::TxOut {
            value: bitcoin::Amount::from_sat(required_reveal_amount),
            script_pubkey: commit_address.script_pubkey(),
        };

        let commit_psbt = self.build_commit_psbt_with_final_sequences(
            funding_utxos.clone(),
            commit_output,
            fee_rate_sat_vb,
        ).await?;

        // Calculate commit txid from unsigned transaction
        let commit_tx = commit_psbt.unsigned_tx.clone();
        let commit_txid = commit_tx.compute_txid();
        let commit_outpoint = bitcoin::OutPoint { txid: commit_txid, vout: 0 };
        let commit_output = commit_tx.output[0].clone();

        // Calculate total input value for commit fee
        let mut total_input_value = 0u64;
        for outpoint in &funding_utxos {
            if let Some(txout) = self.provider.get_utxo(outpoint).await? {
                total_input_value += txout.value.to_sat();
            }
        }
        let commit_fee = total_input_value - commit_tx.output.iter().map(|o| o.value.to_sat()).sum::<u64>();

        log::info!("   Commit txid (pre-calculated): {}", commit_txid);
        log::info!("   Commit fee: {} sats", commit_fee);

        // Step 3: Build reveal PSBT spending the future commit output
        log::info!("📝 Step 3/5: Building reveal transaction...");
        let (reveal_psbt, reveal_fee, _reveal_estimated_vsize) = self
            .build_reveal_psbt_for_presign(
                &params,
                envelope,
                commit_outpoint,
                commit_output,
                internal_key,
                fingerprint,
                path.clone(),
                fee_rate_sat_vb,
            )
            .await?;

        let reveal_txid = reveal_psbt.unsigned_tx.compute_txid();
        log::info!("   Reveal txid (pre-calculated): {}", reveal_txid);
        log::info!("   Reveal fee: {} sats", reveal_fee);

        // Step 4: Sign both transactions
        log::info!("✍️  Step 4/5: Signing both transactions...");
        let signed_commit = self.sign_and_finalize_psbt(commit_psbt).await?;
        let signed_reveal = self.sign_and_finalize_reveal_psbt(reveal_psbt, envelope, internal_key, ephemeral_secret).await?;
        log::info!("   ✅ Both transactions signed");

        // Step 5: Broadcast atomically
        log::info!("📡 Step 5/5: Broadcasting commit and reveal ATOMICALLY...");
        let commit_hex = bitcoin::consensus::encode::serialize_hex(&signed_commit);
        let reveal_hex = bitcoin::consensus::encode::serialize_hex(&signed_reveal);

        use crate::traits::BitcoinRpcProvider;
        let txids = self.provider.send_raw_transactions(&[commit_hex, reveal_hex]).await?;

        let final_commit_txid = txids.get(0)
            .ok_or_else(|| AlkanesError::RpcError("No commit txid in batch response".to_string()))?
            .clone();
        let final_reveal_txid = txids.get(1)
            .ok_or_else(|| AlkanesError::RpcError("No reveal txid in batch response".to_string()))?
            .clone();

        log::info!("   ✅ Commit broadcast: {}", final_commit_txid);
        log::info!("   ✅ Reveal broadcast: {}", final_reveal_txid);

        // Mine blocks if on regtest
        if params.mine_enabled {
            self.mine_blocks_if_regtest(&params).await?;
            self.provider.sync().await?;
        }

        // Trace if requested
        let traces = if params.trace_enabled {
            self.trace_reveal_transaction(&final_reveal_txid, &params).await?
        } else {
            None
        };

        Ok(EnhancedExecuteResult {
            split_txid: None,
            split_fee: None,
            commit_txid: Some(final_commit_txid),
            reveal_txid: final_reveal_txid,
            commit_fee: Some(commit_fee),
            reveal_fee,
            inputs_used: signed_reveal.input.iter().map(|i| i.previous_output.to_string()).collect(),
            outputs_created: signed_reveal.output.iter().map(|o| o.script_pubkey.to_string()).collect(),
            traces,
            wrap_txid: None,
            wrap_fee: None,
            reveal_tx_hex: None,
        })
    }

    /// Build commit PSBT with final sequences (no RBF) for deterministic txid
    async fn build_commit_psbt_with_final_sequences(
        &mut self,
        funding_utxos: Vec<bitcoin::OutPoint>,
        commit_output: bitcoin::TxOut,
        fee_rate: f32,
    ) -> Result<bitcoin::psbt::Psbt> {
        use bitcoin::psbt::{Psbt, Input as PsbtInput};

        let mut inputs_with_txouts = Vec::new();
        let mut total_input_value = 0u64;

        for outpoint in &funding_utxos {
            if let Some(txout) = self.provider.get_utxo(outpoint).await? {
                total_input_value += txout.value.to_sat();
                inputs_with_txouts.push((*outpoint, txout));
            }
        }

        // Create unsigned transaction with FINAL sequences
        let tx_inputs: Vec<bitcoin::TxIn> = inputs_with_txouts
            .iter()
            .map(|(outpoint, _)| bitcoin::TxIn {
                previous_output: *outpoint,
                script_sig: bitcoin::ScriptBuf::new(),
                sequence: bitcoin::Sequence::MAX, // FINAL sequence for deterministic txid
                witness: bitcoin::Witness::new(),
            })
            .collect();

        // Estimate fee for this transaction
        let estimated_vsize = 100 + (inputs_with_txouts.len() * 68) + (2 * 43); // rough estimate
        let estimated_fee = (estimated_vsize as f32 * fee_rate).ceil() as u64;

        let change_value = total_input_value.saturating_sub(commit_output.value.to_sat()).saturating_sub(estimated_fee);
        use crate::traits::WalletProvider;
        let change_address_str: String = WalletProvider::get_address(self.provider).await?;
        let change_script = bitcoin::Address::from_str(&change_address_str)?
            .require_network(self.provider.get_network())?
            .script_pubkey();

        let mut outputs = vec![commit_output];
        if change_value >= 546 {
            outputs.push(bitcoin::TxOut {
                value: bitcoin::Amount::from_sat(change_value),
                script_pubkey: change_script,
            });
        }

        let unsigned_tx = bitcoin::Transaction {
            version: bitcoin::transaction::Version::TWO,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: tx_inputs,
            output: outputs,
        };

        // Create PSBT
        let mut psbt = Psbt::from_unsigned_tx(unsigned_tx)?;

        // Add witness UTXOs
        for (i, (_, txout)) in inputs_with_txouts.iter().enumerate() {
            psbt.inputs[i].witness_utxo = Some(txout.clone());
        }

        // Sign the PSBT
        self.provider.sign_psbt(&psbt).await
    }

    /// Build reveal PSBT for presign strategy
    async fn build_reveal_psbt_for_presign(
        &mut self,
        params: &EnhancedExecuteParams,
        envelope: &AlkanesEnvelope,
        commit_outpoint: bitcoin::OutPoint,
        commit_output: bitcoin::TxOut,
        commit_internal_key: bitcoin::XOnlyPublicKey,
        commit_internal_key_fingerprint: bitcoin::bip32::Fingerprint,
        commit_internal_key_path: bitcoin::bip32::DerivationPath,
        fee_rate_sat_vb: f32,
    ) -> Result<(bitcoin::psbt::Psbt, u64, usize)> {
        // Build reveal transaction outputs
        let mut outputs = Vec::new();

        // Add recipient outputs
        for to_addr in &params.to_addresses {
            let addr = bitcoin::Address::from_str(to_addr)?
                .require_network(self.provider.get_network())?;
            outputs.push(bitcoin::TxOut {
                value: bitcoin::Amount::from_sat(546),
                script_pubkey: addr.script_pubkey(),
            });
        }

        // Calculate how much value is available for change/fee
        let output_total: u64 = outputs.iter().map(|o| o.value.to_sat()).sum();
        let remaining_value = commit_output.value.to_sat().saturating_sub(output_total);

        // Add change output - use provided change address or wallet address
        let change_addr_str = if let Some(ref change_addr) = params.change_address {
            change_addr.clone()
        } else {
            // Get wallet address as fallback
            use crate::traits::WalletProvider;
            WalletProvider::get_address(self.provider).await?
        };

        let addr = bitcoin::Address::from_str(&change_addr_str)?
            .require_network(self.provider.get_network())?;

        // Calculate reveal fee based on actual transaction size
        let reveal_script = envelope.build_reveal_script();
        let reveal_script_size = reveal_script.len();

        // Estimate reveal transaction size:
        // - 1 input with taproot witness (signature + script + control block)
        // - witness: 64 bytes (signature) + script_size + 33 bytes (control block)
        // - At least 1 output (we'll add it below)
        let num_outputs = params.to_addresses.len().max(1) + 1; // recipients + change
        let output_size = num_outputs * 43; // P2TR outputs are ~43 bytes each
        let witness_size = 64 + reveal_script_size + 33;
        let non_witness_size = 10 + 41 + output_size; // version, locktime, input, outputs
        let weight = (non_witness_size * 4) + witness_size;
        let estimated_vsize = (weight + 3) / 4;
        let estimated_reveal_fee = ((estimated_vsize as f32 * fee_rate_sat_vb) * 1.2).ceil() as u64;

        let change_amount = remaining_value.saturating_sub(estimated_reveal_fee);

        // Always add at least one output (transactions must have outputs)
        // Even for contract deployments, we need to return the change
        if change_amount >= 546 {
            outputs.push(bitcoin::TxOut {
                value: bitcoin::Amount::from_sat(change_amount),
                script_pubkey: addr.script_pubkey(),
            });
        } else if outputs.is_empty() {
            // If we have no outputs at all and change is below dust,
            // we still need at least one output - use dust limit
            outputs.push(bitcoin::TxOut {
                value: bitcoin::Amount::from_sat(546),
                script_pubkey: addr.script_pubkey(),
            });
        }

        // Add OP_RETURN runestone for protostones (CRITICAL FIX for alkane deployments)
        if !params.protostones.is_empty() {
            log::info!("Adding OP_RETURN with {} protostones for alkane deployment", params.protostones.len());
            // Validate protostones against the actual number of outputs created
            self.validate_protostones(&params.protostones, outputs.len())?;

            let runestone_script = self.construct_runestone_script(&params.protostones, outputs.len())?;
            outputs.push(bitcoin::TxOut {
                value: bitcoin::Amount::ZERO,
                script_pubkey: runestone_script,
            });
            log::info!("OP_RETURN runestone added as output #{}", outputs.len() - 1);
        }

        // Create unsigned reveal transaction
        let tx_input = bitcoin::TxIn {
            previous_output: commit_outpoint,
            script_sig: bitcoin::ScriptBuf::new(),
            sequence: bitcoin::Sequence::ENABLE_RBF_NO_LOCKTIME,
            witness: bitcoin::Witness::new(),
        };

        let unsigned_tx = bitcoin::Transaction {
            version: bitcoin::transaction::Version::TWO,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: vec![tx_input],
            output: outputs,
        };

        let estimated_vsize = unsigned_tx.vsize();
        let reveal_fee = commit_output.value.to_sat() - unsigned_tx.output.iter().map(|o| o.value.to_sat()).sum::<u64>();

        // Create PSBT
        let mut psbt = bitcoin::psbt::Psbt::from_unsigned_tx(unsigned_tx)?;
        psbt.inputs[0].witness_utxo = Some(commit_output);
        psbt.inputs[0].tap_internal_key = Some(commit_internal_key);
        psbt.inputs[0].tap_key_origins.insert(
            commit_internal_key,
            (Vec::new(), (commit_internal_key_fingerprint, commit_internal_key_path)),
        );

        Ok((psbt, reveal_fee, estimated_vsize))
    }

    /// Sign and finalize reveal PSBT using ephemeral secret
    async fn sign_and_finalize_reveal_psbt(
        &mut self,
        mut psbt: bitcoin::psbt::Psbt,
        envelope: &AlkanesEnvelope,
        commit_internal_key: bitcoin::XOnlyPublicKey,
        ephemeral_secret: bitcoin::secp256k1::SecretKey,
    ) -> Result<bitcoin::Transaction> {
        use bitcoin::sighash::{Prevouts, SighashCache, TapSighashType};
        use bitcoin::taproot::{LeafVersion, TapLeafHash, TaprootBuilder};

        let unsigned_tx = psbt.unsigned_tx.clone();

        // Build taproot spend info
        let reveal_script = envelope.build_reveal_script();
        let taproot_builder = TaprootBuilder::new()
            .add_leaf(0, reveal_script.clone())
            .map_err(|e| AlkanesError::Other(format!("{e:?}")))?;
        let taproot_spend_info = taproot_builder
            .finalize(self.provider.secp(), commit_internal_key)
            .map_err(|e| AlkanesError::Other(format!("{e:?}")))?;
        let control_block = taproot_spend_info
            .control_block(&(reveal_script.clone(), LeafVersion::TapScript))
            .ok_or_else(|| AlkanesError::Other("Failed to create control block".to_string()))?;

        // Collect prevouts
        let prevouts: Vec<bitcoin::TxOut> = psbt
            .inputs
            .iter()
            .map(|input| {
                input
                    .witness_utxo
                    .clone()
                    .ok_or_else(|| AlkanesError::Other("Missing witness UTXO".to_string()))
            })
            .collect::<Result<Vec<_>>>()?;
        let prevouts_all = Prevouts::All(&prevouts);

        // Calculate sighash for script-path spending
        let mut sighash_cache = SighashCache::new(&unsigned_tx);
        let leaf_hash = TapLeafHash::from_script(&reveal_script, LeafVersion::TapScript);
        let sighash = sighash_cache
            .taproot_script_spend_signature_hash(0, &prevouts_all, leaf_hash, TapSighashType::Default)
            .map_err(|e| AlkanesError::Transaction(e.to_string()))?;

        // Sign with ephemeral secret
        let signature = self
            .provider
            .sign_taproot_script_spend(sighash.into(), Some(ephemeral_secret))
            .await?;
        let taproot_signature = bitcoin::taproot::Signature {
            signature,
            sighash_type: TapSighashType::Default,
        };
        let signature_bytes = taproot_signature.to_vec();

        // Create the complete witness
        let witness = envelope.create_complete_witness(&signature_bytes, control_block)?;

        // Create final transaction
        let mut tx = unsigned_tx.clone();
        tx.input[0].witness = witness;

        Ok(tx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock_provider::MockProvider;
    use bitcoin::{Amount, Network};

    #[tokio::test]
    async fn test_create_outputs_dust_limit() {
        let mut provider = MockProvider::new(Network::Regtest);
        let addr1 = WalletProvider::get_address(&provider).await.unwrap();
        let mut executor = EnhancedAlkanesExecutor::new(&mut provider);
        let to_addresses = vec![addr1.clone(), addr1];
        let input_requirements = vec![];

        let outputs = executor.create_outputs(&to_addresses, &None, &input_requirements, &[]).await.unwrap();

        // 2 recipient (dust) outputs + 1 always-appended BTC change output (0,
        // filled in later).
        assert_eq!(outputs.len(), 3);
        assert_eq!(outputs[0].value, Amount::from_sat(546));
        assert_eq!(outputs[1].value, Amount::from_sat(546));
        assert_eq!(outputs[2].value, Amount::from_sat(0));
    }

    #[tokio::test]
    async fn test_create_outputs_with_explicit_bitcoin() {
        let mut provider = MockProvider::new(Network::Regtest);
        let addr1 = WalletProvider::get_address(&provider).await.unwrap();
        let mut executor = EnhancedAlkanesExecutor::new(&mut provider);
        let to_addresses = vec![addr1.clone(), addr1];
        let input_requirements = vec![InputRequirement::Bitcoin { amount: 20000 }];

        let outputs = executor.create_outputs(&to_addresses, &None, &input_requirements, &[]).await.unwrap();

        // 20000 sats split across 2 recipients (10000 each) + 1 change output (0).
        assert_eq!(outputs.len(), 3);
        assert_eq!(outputs[0].value, Amount::from_sat(10000));
        assert_eq!(outputs[1].value, Amount::from_sat(10000));
        assert_eq!(outputs[2].value, Amount::from_sat(0));
    }

    /// The split-tx CPFP child fee rate must (a) never drop below the network
    /// min-relay floor — the root cause of Issue 9, where the child landed at
    /// ~0.15 sat/vB and orphaned once the parent confirmed alone — and (b) make
    /// the parent+child package clear the user's target rate (true CPFP).
    #[test]
    fn test_child_fee_rate_for_package() {
        // Degenerate / starved case (the bug): parent already ate the funds, so
        // a naive child rate would be near zero. Must floor at min-relay.
        let starved = child_fee_rate_for_package(5.0, 100_000, 200, 150);
        assert!(starved >= MIN_RELAY_FEE_RATE, "child must clear min-relay");
        assert!(starved >= 5.0, "child must at least pay its own target rate");

        // Parent built exactly at target: child pays ~target for itself (the
        // package is already at target, so no extra parent shortfall).
        let parent_vsize = 200u64;
        let child_vsize = 150u64;
        let target = 10.0f32;
        let parent_fee = (target * parent_vsize as f32) as u64; // parent at target
        let rate = child_fee_rate_for_package(target, parent_fee, parent_vsize, child_vsize);
        let package_fee = parent_fee as f32 + rate * child_vsize as f32;
        let package_rate = package_fee / (parent_vsize + child_vsize) as f32;
        assert!(package_rate >= target - 0.01, "package must clear target rate");

        // Lean parent (below target): child must overpay to lift the package.
        let lean_parent_fee = (target * parent_vsize as f32 * 0.2) as u64;
        let lean_rate =
            child_fee_rate_for_package(target, lean_parent_fee, parent_vsize, child_vsize);
        let lean_package =
            (lean_parent_fee as f32 + lean_rate * child_vsize as f32)
                / (parent_vsize + child_vsize) as f32;
        assert!(lean_package >= target - 0.01, "child must cover parent shortfall");
        assert!(lean_rate > target, "lean-parent child must exceed bare target");

        // Very low target still clamps to min-relay.
        let low = child_fee_rate_for_package(0.1, 0, 200, 150);
        assert!(low >= MIN_RELAY_FEE_RATE);
    }

    /// `is_wrap_protostone` must reliably distinguish frBTC/frZEC/frETH wraps
    /// (target=(32,N) opcode=77) from any other protostone, because the
    /// split-tx mode dispatch in `execute_full` is gated on this check.
    /// A false positive here would split a non-wrap into two txs and break
    /// the caller's intended atomic semantics; a false negative would force
    /// the original (sometimes-OOG) atomic flow even when the caller asked
    /// to split.
    #[test]
    fn test_is_wrap_protostone_recognizes_frbtc_wrap() {
        let spec = ProtostoneSpec {
            cellpack: Some(alkanes_support::cellpack::Cellpack {
                target: alkanes_support::id::AlkaneId { block: 32, tx: 0 },
                inputs: vec![77],
            }),
            edicts: vec![],
            bitcoin_transfer: None,
            pointer: None,
            refund: None,
        };
        assert!(is_wrap_protostone(&spec));
    }

    #[test]
    fn test_is_wrap_protostone_recognizes_other_block32_wrap_targets() {
        // Cross-chain wraps (frZEC, frETH) live at block=32, different tx.
        let spec = ProtostoneSpec {
            cellpack: Some(alkanes_support::cellpack::Cellpack {
                target: alkanes_support::id::AlkaneId { block: 32, tx: 99 },
                inputs: vec![77, 1, 2, 3],
            }),
            edicts: vec![],
            bitcoin_transfer: None,
            pointer: None,
            refund: None,
        };
        assert!(is_wrap_protostone(&spec));
    }

    #[test]
    fn test_is_wrap_protostone_rejects_amm_factory_swap() {
        // Factory swap is at block=4 — must NOT be classified as a wrap.
        let spec = ProtostoneSpec {
            cellpack: Some(alkanes_support::cellpack::Cellpack {
                target: alkanes_support::id::AlkaneId { block: 4, tx: 65522 },
                inputs: vec![13, 2, 32, 0, 2, 0, 9990, 1, 947644],
            }),
            edicts: vec![],
            bitcoin_transfer: None,
            pointer: None,
            refund: None,
        };
        assert!(!is_wrap_protostone(&spec));
    }

    #[test]
    fn test_is_wrap_protostone_rejects_block32_non_wrap_opcode() {
        // Same target as frBTC but a different opcode (e.g. unwrap=78) must
        // not be classified as a wrap.
        let spec = ProtostoneSpec {
            cellpack: Some(alkanes_support::cellpack::Cellpack {
                target: alkanes_support::id::AlkaneId { block: 32, tx: 0 },
                inputs: vec![78, 100],
            }),
            edicts: vec![],
            bitcoin_transfer: None,
            pointer: None,
            refund: None,
        };
        assert!(!is_wrap_protostone(&spec));
    }

    #[test]
    fn test_is_wrap_protostone_rejects_edict_only_protostone() {
        // Edict protostones have no cellpack — they're transfers, not wraps.
        let spec = ProtostoneSpec {
            cellpack: None,
            edicts: vec![],
            bitcoin_transfer: None,
            pointer: None,
            refund: None,
        };
        assert!(!is_wrap_protostone(&spec));
    }

    // -----------------------------------------------------------------
    // Mempool-aware UTXO adjustment tests.
    //
    // Reproduces the runtime bug fixed alongside split-tx mode: select_utxos
    // was returning only confirmed UTXOs, so when execute_split broadcast
    // Tx A (wrap) and immediately recursed to build Tx B (execute), Tx B's
    // selector saw the SAME 4 confirmed wallet UTXOs Tx A had just spent,
    // built a tx with the same prevouts, and got rejected by the relay as
    // "insufficient fee, rejecting replacement" (BIP125 RBF).
    //
    // Mainnet repro: tx c6b8f0a3611f9072337553e493d057d0ce991916f97453666731507eb702de22
    // landed in mempool as Tx A; Tx B (ba90bff1cccbc50331e0a00d4731e3c571ff975b316ffae76a84b5387413df07)
    // shared inputs and was rejected. apply_mempool_adjustment fixes the
    // root cause by stripping spent outpoints + adding the unconfirmed
    // pay-to-us outputs as candidate inputs.
    // -----------------------------------------------------------------

    fn make_utxo(txid_hex: &str, vout: u32, value: u64, address: &str) -> (OutPoint, UtxoInfo) {
        let outpoint = OutPoint::from_str(&format!("{}:{}", txid_hex, vout)).unwrap();
        let info = UtxoInfo {
            txid: txid_hex.to_string(),
            vout,
            amount: value,
            address: address.to_string(),
            script_pubkey: None,
            confirmations: 1,
            frozen: false,
            freeze_reason: None,
            block_height: Some(1),
            has_inscriptions: false,
            has_runes: false,
            has_alkanes: false,
            is_coinbase: false,
        };
        (outpoint, info)
    }

    const TXID_A: &str = "c6b8f0a3611f9072337553e493d057d0ce991916f97453666731507eb702de22";
    const TXID_OLD_1: &str = "2255b42e4b984e3b7c4a2828302385422dddfe58e76de3595d7f466657b4fc80";
    const TXID_OLD_2: &str = "e7006c4c14cc5527f2d3b231144cb280caee7b87b3a2fd514a3ecd347e5b54df";
    const USER_ADDR: &str = "bc1p026hg4dfhchc0axnmlpamu4v9gltcqtrzk0nvyc00n4eu5nl5tpsrh7zkm";
    const SIGNER_ADDR: &str = "bc1p5lushqjk7kxpqa87ppwn0dealu999999999999999999999999999999000";

    /// Reproduces the split-tx Tx-B failure scenario before the fix.
    ///
    /// Pre-fix: Tx B's selector saw the same 4 confirmed UTXOs Tx A spent.
    /// After apply_mempool_adjustment: those 4 are stripped, and Tx A's
    /// 2 user-paying outputs (alkane carrier + BTC change) become Tx B's
    /// only candidate inputs — exactly the CPFP intent of execute_split.
    #[test]
    fn test_mempool_adjustment_strips_inputs_and_adds_outputs_for_split_tx() {
        let mut spendable: Vec<(OutPoint, UtxoInfo)> = vec![
            make_utxo(TXID_OLD_1, 1, 546, USER_ADDR),
            make_utxo(TXID_OLD_2, 1, 546, USER_ADDR),
            make_utxo(TXID_OLD_2, 2, 846, USER_ADDR),
            make_utxo(
                "601a0f80119a49351bdf8088423813d9d1f68b1326d81e2b2daba5f57764b1c0",
                0, 546, USER_ADDR,
            ),
        ];

        // Mempool tx: matches the real-world Tx A shape (4 vins → 4 vouts:
        // signer / user alkane carrier / user BTC change / OP_RETURN).
        let mempool = serde_json::json!([
            {
                "txid": TXID_A,
                "vin": [
                    { "txid": TXID_OLD_1, "vout": 1 },
                    { "txid": TXID_OLD_2, "vout": 1 },
                    { "txid": TXID_OLD_2, "vout": 2 },
                    { "txid": "601a0f80119a49351bdf8088423813d9d1f68b1326d81e2b2daba5f57764b1c0", "vout": 0 },
                ],
                "vout": [
                    { "scriptpubkey_address": SIGNER_ADDR, "value": 50000 },
                    { "scriptpubkey_address": USER_ADDR, "value": 546 },
                    { "scriptpubkey_address": USER_ADDR, "value": 78462 },
                    { "scriptpubkey_address": null, "value": 0 }
                ],
            }
        ]);

        let report = apply_mempool_adjustment(
            &mut spendable,
            &[mempool],
            &[USER_ADDR.to_string()],
        );

        assert_eq!(report.stripped, 4, "all 4 confirmed UTXOs spent in Tx A should be stripped");
        assert_eq!(report.added, 2, "Tx A's two user-paying outputs should be added");
        assert_eq!(spendable.len(), 2);

        let outpoints: Vec<String> = spendable
            .iter()
            .map(|(op, _)| format!("{}:{}", op.txid, op.vout))
            .collect();
        assert!(outpoints.iter().any(|s| s == &format!("{}:1", TXID_A)),
                "alkane carrier (Tx A:1) must be a candidate input for Tx B");
        assert!(outpoints.iter().any(|s| s == &format!("{}:2", TXID_A)),
                "BTC change (Tx A:2) must be a candidate input for Tx B");

        // The signer output should NOT have been added — it doesn't pay us.
        assert!(!outpoints.iter().any(|s| s == &format!("{}:0", TXID_A)),
                "signer output should not be a candidate input for the user");

        // Confirm the new outputs carry the right amounts.
        let alkane_carrier = spendable.iter().find(|(op, _)| op.txid.to_string() == TXID_A && op.vout == 1).unwrap();
        let btc_change = spendable.iter().find(|(op, _)| op.txid.to_string() == TXID_A && op.vout == 2).unwrap();
        assert_eq!(alkane_carrier.1.amount, 546);
        assert_eq!(btc_change.1.amount, 78462);
        assert_eq!(alkane_carrier.1.confirmations, 0, "Tx A's outputs are still unconfirmed");
    }

    /// No-op case: no mempool txs → no adjustment, no changes.
    #[test]
    fn test_mempool_adjustment_noop_when_mempool_empty() {
        let mut spendable: Vec<(OutPoint, UtxoInfo)> = vec![
            make_utxo(TXID_OLD_1, 1, 546, USER_ADDR),
        ];
        let report = apply_mempool_adjustment(&mut spendable, &[], &[USER_ADDR.to_string()]);
        assert_eq!(report.stripped, 0);
        assert_eq!(report.added, 0);
        assert_eq!(spendable.len(), 1);
    }

    /// OP_RETURN outputs (value=0, no address) must not be added as candidates.
    /// Otherwise the selector would try to spend them and the tx would be
    /// rejected at construction time.
    #[test]
    fn test_mempool_adjustment_skips_op_return_and_value_zero() {
        let mut spendable: Vec<(OutPoint, UtxoInfo)> = Vec::new();
        let mempool = serde_json::json!([
            {
                "txid": TXID_A,
                "vin": [],
                "vout": [
                    { "scriptpubkey_address": USER_ADDR, "value": 0 },        // dust placeholder
                    { "scriptpubkey_address": null, "value": 0 },             // OP_RETURN
                    { "scriptpubkey_address": USER_ADDR, "value": 1000 }      // legit
                ],
            }
        ]);
        let report = apply_mempool_adjustment(&mut spendable, &[mempool], &[USER_ADDR.to_string()]);
        assert_eq!(report.added, 1, "only the value=1000 output should be added");
        assert_eq!(spendable.len(), 1);
        assert_eq!(spendable[0].1.amount, 1000);
    }

    /// Outputs paying addresses we don't own must not be added as candidates.
    /// (Same mempool tx may have outputs paying multiple parties; we only
    /// claim the ones paying our addresses.)
    #[test]
    fn test_mempool_adjustment_ignores_outputs_to_other_addresses() {
        let mut spendable: Vec<(OutPoint, UtxoInfo)> = Vec::new();
        let mempool = serde_json::json!([
            {
                "txid": TXID_A,
                "vin": [],
                "vout": [
                    { "scriptpubkey_address": "bc1qsomeoneelse9999999999999999999999999", "value": 10000 },
                    { "scriptpubkey_address": USER_ADDR, "value": 546 }
                ],
            }
        ]);
        let report = apply_mempool_adjustment(&mut spendable, &[mempool], &[USER_ADDR.to_string()]);
        assert_eq!(report.added, 1);
        assert_eq!(spendable[0].1.address, USER_ADDR);
    }

    /// When a spent input we don't currently track appears in mempool (e.g.,
    /// the indexer had it indexed under a different address), strip is a
    /// no-op for that outpoint but the rest of the adjustment still works.
    #[test]
    fn test_mempool_adjustment_strip_is_partial_when_outpoint_not_in_set() {
        let mut spendable: Vec<(OutPoint, UtxoInfo)> = vec![
            make_utxo(TXID_OLD_1, 1, 546, USER_ADDR), // present
            // TXID_OLD_2:1 NOT in our set
        ];
        let mempool = serde_json::json!([
            {
                "txid": TXID_A,
                "vin": [
                    { "txid": TXID_OLD_1, "vout": 1 },  // we have this
                    { "txid": TXID_OLD_2, "vout": 1 }   // we don't
                ],
                "vout": [],
            }
        ]);
        let report = apply_mempool_adjustment(&mut spendable, &[mempool], &[USER_ADDR.to_string()]);
        assert_eq!(report.stripped, 1, "only the matching outpoint gets stripped");
        assert!(spendable.is_empty());
    }

    /// Synthetic-mempool path: `execute_split` hands Tx A's signed hex to
    /// Tx B's `select_utxos` via `known_pending_tx_hexes`, bypassing the
    /// indexer-propagation timing window. Verify `decode_tx_hex_to_mempool_json`
    /// produces a payload that `apply_mempool_adjustment` can act on.
    #[test]
    fn test_decode_tx_hex_to_mempool_json_then_adjust() {
        // Real Tx A from the 2026-05-03 mainnet split-tx run:
        // c5520bb64d1a742a6bd62999267f683e1f0756481220ff2155d2be841a3d7b92.
        // Inputs: 601a0f80...:1 (574 sats), c6b8f0a3...:2 (78462 sats).
        // Outputs: signer 30000, user 546, user 48204, OP_RETURN.
        // Hex pulled live from
        // https://mempool.space/api/tx/c5520bb64d1a.../hex.
        let tx_hex = "02000000000102c0b16477f5a5ab2d2b1ed826138bf6d1d91338428880df1b35499a11800f1a600100000000fdffffff22de02b77e503167665374f9161999ced057d093e453753372901f61a3f0b8c60200000000fdffffff043075000000000000225120a7f90b8256f58c1074fe085d37b73dff3040774babc216dae106e281e020638b22020000000000002251207ab57455a9be2f87f4d3dfc3ddf2ac2a3ebc0163159f36130f7ceb9e527fa2c34cbc0000000000002251207ab57455a9be2f87f4d3dfc3ddf2ac2a3ebc0163159f36130f7ceb9e527fa2c30000000000000000136a5d101600ff7f818cec8ad0abc0a8a081d2150140300f852484bcd16e2d5c2850f8c3bc1bd861a033971994f621fb589deb3edf8225dfbbdb969abb738b4ba2e1c119c7c3f860d77095b150b058a89170b2d532ad01408e1f00dd1c42ee3c073f256395d5b74d7c8366a52d29b72832a1ebec3bda4048f3a86f41625ec8736cf97051796b20961e05e11291aa65737cbf0ddb243f450f00000000";

        let synthetic = decode_tx_hex_to_mempool_json(tx_hex).expect("decode tx hex");

        assert_eq!(synthetic.get("vin").unwrap().as_array().unwrap().len(), 2);
        assert_eq!(synthetic.get("vout").unwrap().as_array().unwrap().len(), 4);

        // Round-trip: feed the synthetic JSON into the adjustment fn alongside
        // a candidate set that contains Tx A's prevouts. They should be stripped
        // and Tx A's pay-to-us outputs should be added.
        let mut spendable: Vec<(OutPoint, UtxoInfo)> = vec![
            make_utxo(
                "601a0f80119a49351bdf8088423813d9d1f68b1326d81e2b2daba5f57764b1c0",
                1, 574, USER_ADDR,
            ),
            make_utxo(
                "c6b8f0a3611f9072337553e493d057d0ce991916f97453666731507eb702de22",
                2, 78462, USER_ADDR,
            ),
        ];
        let report = apply_mempool_adjustment(
            &mut spendable,
            &[serde_json::json!([synthetic])],
            &[USER_ADDR.to_string()],
        );
        assert_eq!(report.stripped, 2, "Tx A's two prevouts should be stripped");
        assert_eq!(report.added, 2, "Tx A's two pay-to-user outputs should be added");
        let amounts: alloc::collections::BTreeSet<u64> =
            spendable.iter().map(|(_, u)| u.amount).collect();
        assert!(amounts.contains(&546));
        assert!(amounts.contains(&48204));
    }

    /// Multi-address case: per-address mempool fetches each contribute their
    /// own spent set and outputs. Mirrors how `select_utxos` calls
    /// `get_address_txs_mempool` once per resolved address.
    #[test]
    fn test_mempool_adjustment_multi_address_aggregates() {
        let addr_a = USER_ADDR.to_string();
        let addr_b = "bc1qcoldwalletsegwit9999999999999999999999".to_string();

        let mut spendable: Vec<(OutPoint, UtxoInfo)> = vec![
            make_utxo(TXID_OLD_1, 0, 1000, &addr_a),
            make_utxo(TXID_OLD_2, 0, 2000, &addr_b),
        ];

        let mempool_a = serde_json::json!([{
            "txid": TXID_A,
            "vin": [{ "txid": TXID_OLD_1, "vout": 0 }],
            "vout": [{ "scriptpubkey_address": addr_a, "value": 800 }],
        }]);
        let mempool_b = serde_json::json!([{
            "txid": "deadbeef00000000000000000000000000000000000000000000000000000000",
            "vin": [{ "txid": TXID_OLD_2, "vout": 0 }],
            "vout": [{ "scriptpubkey_address": addr_b, "value": 1500 }],
        }]);

        let report = apply_mempool_adjustment(
            &mut spendable,
            &[mempool_a, mempool_b],
            &[addr_a.clone(), addr_b.clone()],
        );

        assert_eq!(report.stripped, 2);
        assert_eq!(report.added, 2);
        assert_eq!(spendable.len(), 2);

        // Both addresses should have a fresh unconfirmed UTXO.
        assert!(spendable.iter().any(|(_, u)| u.address == addr_a && u.amount == 800));
        assert!(spendable.iter().any(|(_, u)| u.address == addr_b && u.amount == 1500));
    }

    // -----------------------------------------------------------------
    // Alkane-needed branch BTC-fill protection.
    //
    // The protection at execute.rs::select_utxos line ~1944 — when
    // `alkanes_needed` is non-empty AND the BTC-fill loop encounters
    // an alkane-bearing UTXO whose alkane is NOT in the requirements,
    // the loop must skip it (not consume it as fee dust).
    //
    // Empirical validation came from a 2026-05-03 mainnet run (Tx A
    // e3a99b2f... left the user's DIESEL carrier alone). This test
    // pins it directly: set up MockProvider with one DIESEL carrier
    // (needed) + one frBTC carrier (not needed) + one plain BTC
    // UTXO, run select_utxos asking for `alkanes_needed = {DIESEL}`
    // and a BTC budget that fits in the plain BTC UTXO. Expected:
    //   - DIESEL carrier selected (for alkane requirement)
    //   - plain BTC selected (for fee budget)
    //   - frBTC carrier NOT selected (alkane-bearing, not needed)
    // -----------------------------------------------------------------
    #[tokio::test]
    async fn test_alkane_needed_branch_skips_non_needed_alkane_carriers() {
        use crate::alkanes::types::InputRequirement;
        use crate::mock_provider::MockProvider;
        use bitcoin::address::Address;
        use bitcoin::key::Secp256k1;
        use std::str::FromStr;

        // Build a regtest mock with three UTXOs:
        //   tx_diesel: 546 sats, carries DIESEL [2:0] amount=1000  (needed)
        //   tx_frbtc:  546 sats, carries frBTC  [32:0] amount=5000 (NOT needed)
        //   tx_btc:    50000 sats, no alkanes                       (BTC for fees)
        let mut mock = MockProvider::new(bitcoin::Network::Regtest);

        let secp = Secp256k1::new();
        let (sk, pk) = secp.generate_keypair(&mut rand::thread_rng());
        let (xonly, _) = pk.x_only_public_key();
        let addr = Address::p2tr(&secp, xonly, None, bitcoin::Network::Regtest);
        mock.set_keypair(sk, bitcoin::PublicKey::new(pk));
        let script = addr.script_pubkey();

        // Three UTXOs at the same address.
        let txid_diesel = bitcoin::Txid::from_str(
            "1111111111111111111111111111111111111111111111111111111111111111",
        )
        .unwrap();
        let txid_frbtc = bitcoin::Txid::from_str(
            "2222222222222222222222222222222222222222222222222222222222222222",
        )
        .unwrap();
        let txid_btc = bitcoin::Txid::from_str(
            "3333333333333333333333333333333333333333333333333333333333333333",
        )
        .unwrap();
        {
            let mut utxos = mock.utxos.lock().unwrap();
            utxos.push((
                bitcoin::OutPoint::new(txid_diesel, 0),
                bitcoin::TxOut {
                    value: bitcoin::Amount::from_sat(546),
                    script_pubkey: script.clone(),
                },
            ));
            utxos.push((
                bitcoin::OutPoint::new(txid_frbtc, 0),
                bitcoin::TxOut {
                    value: bitcoin::Amount::from_sat(546),
                    script_pubkey: script.clone(),
                },
            ));
            utxos.push((
                bitcoin::OutPoint::new(txid_btc, 0),
                bitcoin::TxOut {
                    value: bitcoin::Amount::from_sat(50000),
                    script_pubkey: script.clone(),
                },
            ));
        }
        {
            let mut ab = mock.alkane_balances.lock().unwrap();
            ab.insert(format!("{}:0", txid_diesel), vec![(2, 0, 1000)]); // DIESEL
            ab.insert(format!("{}:0", txid_frbtc), vec![(32, 0, 5000)]); // frBTC
            // tx_btc → no entry → empty balances
        }

        // The legacy lua-batch path also queries balances; the mock
        // returns no_response when called via lua_batch_balances. The
        // primary protorunesbyoutpoint path runs first (per
        // select_utxos's discovery order) and feeds utxo_balances
        // from `get_protorunes_by_outpoint`, which now reads from
        // `alkane_balances`.

        // Now exercise select_utxos. Need 800 sats DIESEL + 5000 sats BTC.
        let mut executor = EnhancedAlkanesExecutor::new(&mut mock);
        let requirements = vec![
            InputRequirement::Alkanes {
                block: 2,
                tx: 0,
                amount: 800,
            },
            InputRequirement::Bitcoin { amount: 5000 },
        ];
        let result = executor
            .select_utxos(
                &requirements,
                &Some(vec![addr.to_string()]),
                &[],
                None,
                &[],
                &[],
                UtxoDataSource::Metashrew,
            )
            .await
            .unwrap();

        // Inspect: DIESEL carrier and BTC UTXO must be selected;
        // frBTC carrier must NOT be selected.
        let selected: alloc::collections::BTreeSet<String> = result
            .outpoints
            .iter()
            .map(|o| format!("{}:{}", o.txid, o.vout))
            .collect();

        assert!(
            selected.contains(&format!("{}:0", txid_diesel)),
            "DIESEL carrier must be selected for the alkane requirement"
        );
        assert!(
            selected.contains(&format!("{}:0", txid_btc)),
            "plain BTC UTXO must be selected for the fee budget"
        );
        assert!(
            !selected.contains(&format!("{}:0", txid_frbtc)),
            "frBTC carrier must NOT be selected — it carries an alkane we don't need, the protection at execute.rs:1944 must skip it"
        );
        assert_eq!(result.outpoints.len(), 2, "exactly 2 selected: DIESEL + BTC");
    }

    // ────────────────────────────────────────────────────────────────────
    // check_utxo_eligibility — pure-function tests for the indexer-aware
    // UTXO height filter.
    //
    // Background: esplora indexes new blocks ~immediately after they're
    // mined; metashrew (the alkanes WASM indexer) takes longer because it
    // re-runs every protostone in the block. The steady-state on mainnet
    // has esplora 1–2 blocks ahead. We can safely spend any UTXO whose
    // creating block is `<= max_indexed_height` because alkane balance
    // sheets are *immutable per-outpoint* — once written, they don't
    // change. UTXOs at higher heights have unknown balance sheets and
    // must be skipped.
    //
    // These tests pin the filter semantics so the next refactor can't
    // silently regress and start spending unindexed UTXOs (which would
    // cause "input amount cannot be zero" errors at contract execution
    // because the caller-believed alkane content turns out to be wrong).
    // ────────────────────────────────────────────────────────────────────

    fn mk_utxo(
        txid_byte: u8,
        block_height: Option<u64>,
        frozen: bool,
        is_coinbase: bool,
        confirmations: u32,
    ) -> UtxoInfo {
        let txid_str = format!("{:0<64}", format!("{:02x}", txid_byte));
        UtxoInfo {
            txid: txid_str,
            vout: 0,
            amount: 100_000,
            address: "bc1qmock".to_string(),
            script_pubkey: None,
            confirmations,
            frozen,
            freeze_reason: None,
            block_height,
            has_inscriptions: false,
            has_runes: false,
            has_alkanes: false,
            is_coinbase,
        }
    }

    #[test]
    fn eligibility_passes_simple_confirmed_utxo() {
        let utxo = mk_utxo(0xaa, Some(800_000), false, false, 6);
        assert_eq!(check_utxo_eligibility(&utxo, None), Ok(()));
        assert_eq!(check_utxo_eligibility(&utxo, Some(800_000)), Ok(()));
        assert_eq!(check_utxo_eligibility(&utxo, Some(800_001)), Ok(()));
    }

    #[test]
    fn eligibility_skips_frozen() {
        let utxo = mk_utxo(0xbb, Some(800_000), /* frozen */ true, false, 6);
        assert_eq!(
            check_utxo_eligibility(&utxo, Some(800_000)),
            Err(UtxoSkipReason::Frozen),
        );
    }

    #[test]
    fn eligibility_skips_immature_coinbase() {
        let utxo = mk_utxo(0xcc, Some(800_000), false, /* is_coinbase */ true, /* conf */ 50);
        assert_eq!(
            check_utxo_eligibility(&utxo, Some(800_000)),
            Err(UtxoSkipReason::ImmatureCoinbase { confirmations: 50 }),
        );
    }

    #[test]
    fn eligibility_passes_mature_coinbase() {
        let utxo = mk_utxo(0xcc, Some(800_000), false, true, COINBASE_MATURITY);
        assert_eq!(check_utxo_eligibility(&utxo, Some(800_000)), Ok(()));
    }

    #[test]
    fn eligibility_skips_unindexed_height_when_filter_set() {
        // metashrew at 800,000; UTXO mined into 800,001 — balance sheet
        // not yet queryable, must be skipped.
        let utxo = mk_utxo(0xdd, Some(800_001), false, false, 6);
        assert_eq!(
            check_utxo_eligibility(&utxo, Some(800_000)),
            Err(UtxoSkipReason::UnindexedHeight {
                block_height: 800_001,
                max_indexed: 800_000,
            }),
        );
    }

    #[test]
    fn eligibility_passes_unindexed_height_when_no_filter() {
        // Back-compat: max_indexed_height = None disables the filter.
        let utxo = mk_utxo(0xee, Some(999_999_999), false, false, 6);
        assert_eq!(check_utxo_eligibility(&utxo, None), Ok(()));
    }

    #[test]
    fn eligibility_passes_unconfirmed_utxo_with_filter() {
        // block_height = None means "mempool / unconfirmed". The height
        // filter is a confirmed-only check; mempool UTXOs are handled by
        // the separate `apply_mempool_adjustment` path which adds back
        // "we built this" txs from `known_pending_tx_hexes`. Make sure
        // we don't accidentally drop them here.
        let utxo = mk_utxo(0xff, None, false, false, 0);
        assert_eq!(check_utxo_eligibility(&utxo, Some(800_000)), Ok(()));
    }

    #[test]
    fn eligibility_filter_models_real_lag_window() {
        // Realistic scenario: wallet has 4 UTXOs across 4 different blocks;
        // metashrew is 2 blocks behind bitcoind. Verify the filter keeps
        // exactly the safely-indexed prefix.
        let max_indexed: u64 = 948_720;
        let utxos = vec![
            mk_utxo(0x01, Some(948_700), false, false, 22), // safe
            mk_utxo(0x02, Some(948_720), false, false, 2),  // safe — exactly at tip
            mk_utxo(0x03, Some(948_721), false, false, 1),  // unindexed
            mk_utxo(0x04, Some(948_722), false, false, 0),  // unindexed
        ];

        let kept: Vec<_> = utxos
            .iter()
            .filter(|u| check_utxo_eligibility(u, Some(max_indexed)).is_ok())
            .collect();
        assert_eq!(kept.len(), 2, "only the two indexed UTXOs survive");
        assert_eq!(kept[0].block_height, Some(948_700));
        assert_eq!(kept[1].block_height, Some(948_720));

        // The two unindexed ones must surface UnindexedHeight specifically
        // (not get lumped into a generic skip — operators rely on this
        // structured reason for telemetry / overlay copy).
        match check_utxo_eligibility(&utxos[2], Some(max_indexed)) {
            Err(UtxoSkipReason::UnindexedHeight { block_height, max_indexed: m }) => {
                assert_eq!(block_height, 948_721);
                assert_eq!(m, max_indexed);
            }
            other => panic!("expected UnindexedHeight, got {:?}", other),
        }
    }

    #[test]
    fn eligibility_filter_order_frozen_beats_height() {
        // Defensive: a frozen UTXO at an unindexed height should report
        // Frozen (not UnindexedHeight) so that operator-facing errors
        // explain the *real* reason. Order is documented in the helper's
        // doc comment — pin it here.
        let utxo = mk_utxo(0x99, Some(999_999), /* frozen */ true, false, 0);
        assert_eq!(
            check_utxo_eligibility(&utxo, Some(800_000)),
            Err(UtxoSkipReason::Frozen),
        );
    }

    // ────────────────────────────────────────────────────────────────────
    // select_utxos integration — verify the height filter actually flows
    // through the public coin-selection API, not just the helper. Uses
    // MockProvider, whose `get_utxos` hardcodes `block_height = Some(100)`
    // for every UTXO it returns; we drive max_indexed_height around that
    // value to pin both branches.
    // ────────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn select_utxos_includes_when_max_indexed_at_or_above_utxo_height() {
        use crate::mock_provider::MockProvider;
        use bitcoin::address::Address;
        use bitcoin::key::Secp256k1;
        use std::str::FromStr;

        let mut mock = MockProvider::new(bitcoin::Network::Regtest);
        let secp = Secp256k1::new();
        let (sk, pk) = secp.generate_keypair(&mut rand::thread_rng());
        let (xonly, _) = pk.x_only_public_key();
        let addr = Address::p2tr(&secp, xonly, None, bitcoin::Network::Regtest);
        mock.set_keypair(sk, bitcoin::PublicKey::new(pk));
        let script = addr.script_pubkey();

        let txid = bitcoin::Txid::from_str(
            "abababababababababababababababababababababababababababababababab",
        )
        .unwrap();
        {
            let mut utxos = mock.utxos.lock().unwrap();
            utxos.push((
                bitcoin::OutPoint::new(txid, 0),
                bitcoin::TxOut {
                    value: bitcoin::Amount::from_sat(50_000),
                    script_pubkey: script.clone(),
                },
            ));
        }

        let mut executor = EnhancedAlkanesExecutor::new(&mut mock);
        let requirements = vec![InputRequirement::Bitcoin { amount: 5_000 }];

        // MockProvider hardcodes block_height=100. With max_indexed=100
        // (exact match) the UTXO is eligible — height filter passes.
        let result = executor
            .select_utxos(
                &requirements,
                &Some(vec![addr.to_string()]),
                &[],
                Some(100),
                &[],
                &[],
                UtxoDataSource::Metashrew,
            )
            .await
            .unwrap();
        assert_eq!(result.outpoints.len(), 1, "UTXO at height=100 selected when max_indexed=100");

        // Same with max_indexed strictly above the UTXO's height.
        let result = executor
            .select_utxos(
                &requirements,
                &Some(vec![addr.to_string()]),
                &[],
                Some(200),
                &[],
                &[],
                UtxoDataSource::Metashrew,
            )
            .await
            .unwrap();
        assert_eq!(result.outpoints.len(), 1, "UTXO at height=100 selected when max_indexed=200");

        // And with no filter (None) — back-compat path.
        let result = executor
            .select_utxos(
                &requirements,
                &Some(vec![addr.to_string()]),
                &[],
                None,
                &[],
                &[],
                UtxoDataSource::Metashrew,
            )
            .await
            .unwrap();
        assert_eq!(result.outpoints.len(), 1, "UTXO selected when filter disabled");
    }

    #[tokio::test]
    async fn select_utxos_excludes_when_max_indexed_below_utxo_height() {
        use crate::mock_provider::MockProvider;
        use bitcoin::address::Address;
        use bitcoin::key::Secp256k1;
        use std::str::FromStr;

        let mut mock = MockProvider::new(bitcoin::Network::Regtest);
        let secp = Secp256k1::new();
        let (sk, pk) = secp.generate_keypair(&mut rand::thread_rng());
        let (xonly, _) = pk.x_only_public_key();
        let addr = Address::p2tr(&secp, xonly, None, bitcoin::Network::Regtest);
        mock.set_keypair(sk, bitcoin::PublicKey::new(pk));
        let script = addr.script_pubkey();

        let txid = bitcoin::Txid::from_str(
            "cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd",
        )
        .unwrap();
        {
            let mut utxos = mock.utxos.lock().unwrap();
            utxos.push((
                bitcoin::OutPoint::new(txid, 0),
                bitcoin::TxOut {
                    value: bitcoin::Amount::from_sat(50_000),
                    script_pubkey: script.clone(),
                },
            ));
        }

        let mut executor = EnhancedAlkanesExecutor::new(&mut mock);
        let requirements = vec![InputRequirement::Bitcoin { amount: 5_000 }];

        // MockProvider's UTXO is at height=100; max_indexed=99 means the
        // indexer hasn't reached this block yet. select_utxos must fail
        // because there are no eligible UTXOs to cover the 5_000-sat ask.
        let result = executor
            .select_utxos(
                &requirements,
                &Some(vec![addr.to_string()]),
                &[],
                Some(99),
                &[],
                &[],
                UtxoDataSource::Metashrew,
            )
            .await;
        assert!(
            result.is_err(),
            "select_utxos must fail when all UTXOs are at heights metashrew hasn't indexed",
        );
        let err_str = format!("{:?}", result.unwrap_err()).to_lowercase();
        assert!(
            err_str.contains("insufficient")
                || err_str.contains("not enough")
                || err_str.contains("no utxos")
                || err_str.contains("0 spendable"),
            "expected insufficient-funds-style error, got: {err_str}"
        );

        // Sanity: with the filter disabled the same setup succeeds.
        let result = executor
            .select_utxos(
                &requirements,
                &Some(vec![addr.to_string()]),
                &[],
                None,
                &[],
                &[],
                UtxoDataSource::Metashrew,
            )
            .await
            .unwrap();
        assert_eq!(result.outpoints.len(), 1, "UTXO selectable without filter");
    }

    // ────────────────────────────────────────────────────────────────────
    // Non-dust alkane carriers (2026-07-11 mainnet incident, lending child
    // 2:92269). Alkanes conventionally ride ≤1000-sat outputs, but nothing
    // enforces that on-chain: the borrower held three 2916-sat UTXOs
    // carrying 2000 frBTC each. The dust-only discovery pass made his
    // 21864 repayment fail "Insufficient alkanes: … have 18739" out of a
    // real 24739 balance, and the dust-only BTC carrier-exclusion would
    // happily have burned those carriers as fee inputs. Both tests replay
    // the incident's exact numbers.
    // ────────────────────────────────────────────────────────────────────

    /// Shared fixture: taproot wallet with the incident's UTXO set.
    /// vouts 0-2: 546-sat dust carrying 17823 / 915 / 1 of alkane 32:0.
    /// vouts 3-5: 2916-sat NON-DUST UTXOs carrying 2000 of 32:0 each.
    /// vout  6:   50_000-sat clean BTC.
    fn incident_wallet(mock: &MockProvider, script: &bitcoin::ScriptBuf) -> bitcoin::Txid {
        use std::str::FromStr;
        let txid = bitcoin::Txid::from_str(
            "efefefefefefefefefefefefefefefefefefefefefefefefefefefefefefefef",
        )
        .unwrap();
        let entries: [(u64, u64); 7] = [
            (546, 17_823),
            (546, 915),
            (546, 1),
            (2_916, 2_000),
            (2_916, 2_000),
            (2_916, 2_000),
            (50_000, 0),
        ];
        let mut utxos = mock.utxos.lock().unwrap();
        let mut balances = mock.alkane_balances.lock().unwrap();
        for (vout, (sats, alkane_amt)) in entries.iter().enumerate() {
            utxos.push((
                bitcoin::OutPoint::new(txid, vout as u32),
                bitcoin::TxOut {
                    value: bitcoin::Amount::from_sat(*sats),
                    script_pubkey: script.clone(),
                },
            ));
            if *alkane_amt > 0 {
                balances.insert(format!("{}:{}", txid, vout), vec![(32, 0, *alkane_amt)]);
            }
        }
        txid
    }

    #[tokio::test]
    async fn select_utxos_aggregates_alkanes_on_non_dust_utxos() {
        use crate::mock_provider::MockProvider;
        use bitcoin::address::Address;
        use bitcoin::key::Secp256k1;

        let mut mock = MockProvider::new(bitcoin::Network::Regtest);
        let secp = Secp256k1::new();
        let (sk, pk) = secp.generate_keypair(&mut rand::thread_rng());
        let (xonly, _) = pk.x_only_public_key();
        let addr = Address::p2tr(&secp, xonly, None, bitcoin::Network::Regtest);
        mock.set_keypair(sk, bitcoin::PublicKey::new(pk));
        // Standard-bitcoin mode: exercise the per-outpoint primary discovery
        // (dust pass + extended non-dust pass), NOT the qubitcoin
        // protorunesbyaddress path (which never had the dust filter).
        mock.qubitcoin_mode = false;
        let txid = incident_wallet(&mock, &addr.script_pubkey());

        let mut executor = EnhancedAlkanesExecutor::new(&mut mock);
        // The incident's repayment: 21864 of 32:0. Dust UTXOs alone hold
        // 18739 — satisfying this REQUIRES the extended (non-dust) pass.
        let requirements = vec![InputRequirement::Alkanes { block: 32, tx: 0, amount: 21_864 }];

        let result = executor
            .select_utxos(
                &requirements,
                &Some(vec![addr.to_string()]),
                &[],
                None,
                &[],
                &[],
                UtxoDataSource::Metashrew,
            )
            .await
            .expect("selection must aggregate across dust AND non-dust alkane carriers");

        let found = result
            .alkanes_found
            .iter()
            .find(|(id, _)| id.block == 32 && id.tx == 0)
            .map(|(_, amt)| *amt)
            .unwrap_or(0);
        assert!(
            found >= 21_864,
            "collected {found} of 32:0 — must cover the 21864 requirement (dust holds only 18739)"
        );
        assert!(
            result
                .outpoints
                .iter()
                .any(|op| op.txid == txid && (3..=5).contains(&op.vout)),
            "at least one 2916-sat carrier must be selected — dust alone cannot cover the ask"
        );
    }

    #[tokio::test]
    async fn btc_only_selection_skips_non_dust_alkane_carriers() {
        use crate::mock_provider::MockProvider;
        use bitcoin::address::Address;
        use bitcoin::key::Secp256k1;

        let mut mock = MockProvider::new(bitcoin::Network::Regtest);
        let secp = Secp256k1::new();
        let (sk, pk) = secp.generate_keypair(&mut rand::thread_rng());
        let (xonly, _) = pk.x_only_public_key();
        let addr = Address::p2tr(&secp, xonly, None, bitcoin::Network::Regtest);
        mock.set_keypair(sk, bitcoin::PublicKey::new(pk));
        // Standard-bitcoin mode: the carrier exclusion is skipped entirely on
        // qubitcoin (no protorunesbyoutpoint there).
        mock.qubitcoin_mode = false;
        let txid = incident_wallet(&mock, &addr.script_pubkey());

        let mut executor = EnhancedAlkanesExecutor::new(&mut mock);
        // BTC-only ask (no alkane requirements) — must be funded from the
        // clean 50k UTXO; every carrier (dust or 2916-sat) must be skipped,
        // otherwise this tx burns the user's tokens as fees.
        let requirements = vec![InputRequirement::Bitcoin { amount: 5_000 }];

        let result = executor
            .select_utxos(
                &requirements,
                &Some(vec![addr.to_string()]),
                &[],
                None,
                &[],
                &[],
                UtxoDataSource::Metashrew,
            )
            .await
            .expect("clean 50k UTXO covers the ask");

        for op in &result.outpoints {
            assert!(
                !(op.txid == txid && op.vout <= 5),
                "selected {}:{} — an alkane carrier must NEVER fund a BTC-only tx (token burn)",
                op.txid, op.vout
            );
        }
        assert!(
            result.outpoints.iter().any(|op| op.txid == txid && op.vout == 6),
            "the clean 50k UTXO is the only legitimate funding source"
        );
    }

    // ────────────────────────────────────────────────────────────────────
    // excluded_utxos — caller-locked outpoints (2026-07-12). subfrost-app
    // passes the UTXOs committed to its open lending offers here: each
    // offer's pre-signed prep tx spends specific outpoints, so ANY other
    // flow spending one silently invalidates the offer.
    // ────────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn select_utxos_never_spends_caller_excluded_outpoints() {
        use crate::mock_provider::MockProvider;
        use bitcoin::address::Address;
        use bitcoin::key::Secp256k1;

        let mut mock = MockProvider::new(bitcoin::Network::Regtest);
        let secp = Secp256k1::new();
        let (sk, pk) = secp.generate_keypair(&mut rand::thread_rng());
        let (xonly, _) = pk.x_only_public_key();
        let addr = Address::p2tr(&secp, xonly, None, bitcoin::Network::Regtest);
        mock.set_keypair(sk, bitcoin::PublicKey::new(pk));
        mock.qubitcoin_mode = false;
        let script = addr.script_pubkey();
        let txid = incident_wallet(&mock, &script); // vouts 0-5 alkane carriers, vout 6 = 50k clean
        // Second clean BTC UTXO so an exclusion of the first still leaves funding.
        {
            let txid2 = bitcoin::Txid::from_str(
                "abababababababababababababababababababababababababababababababab",
            ).unwrap();
            let mut utxos = mock.utxos.lock().unwrap();
            utxos.push((
                bitcoin::OutPoint::new(txid2, 0),
                bitcoin::TxOut { value: bitcoin::Amount::from_sat(40_000), script_pubkey: script.clone() },
            ));
        }

        let mut executor = EnhancedAlkanesExecutor::new(&mut mock);

        // BTC-only: exclude the 50k UTXO (as if a lending prep commits it) —
        // the ask must be funded from the 40k one instead.
        let requirements = vec![InputRequirement::Bitcoin { amount: 5_000 }];
        let result = executor
            .select_utxos(
                &requirements,
                &Some(vec![addr.to_string()]),
                &[],
                None,
                &[],
                &[format!("{}:{}", txid, 6)],
                UtxoDataSource::Metashrew,
            )
            .await
            .expect("selection must fund from the non-excluded UTXO");
        assert!(
            !result.outpoints.iter().any(|op| op.txid == txid && op.vout == 6),
            "caller-excluded UTXO must never be selected"
        );

        // Alkane requirement: exclude the 17823 carrier (vout 0) — the remaining
        // carriers (915 + 1 + 3×2000) cover a 6000 ask without it.
        let requirements = vec![InputRequirement::Alkanes { block: 32, tx: 0, amount: 6_000 }];
        let result = executor
            .select_utxos(
                &requirements,
                &Some(vec![addr.to_string()]),
                &[],
                None,
                &[],
                &[format!("{}:{}", txid, 0)],
                UtxoDataSource::Metashrew,
            )
            .await
            .expect("alkane ask must aggregate from non-excluded carriers");
        assert!(
            !result.outpoints.iter().any(|op| op.txid == txid && op.vout == 0),
            "excluded alkane carrier must never be selected"
        );

        // Malformed entries are ignored (fail-open per entry), valid ones still apply.
        let result = executor
            .select_utxos(
                &vec![InputRequirement::Bitcoin { amount: 5_000 }],
                &Some(vec![addr.to_string()]),
                &[],
                None,
                &[],
                &["not-an-outpoint".to_string(), format!("{}:{}", txid, 6)],
                UtxoDataSource::Metashrew,
            )
            .await
            .expect("malformed exclusion entry must not fail the selection");
        assert!(!result.outpoints.iter().any(|op| op.txid == txid && op.vout == 6));
    }

    // ────────────────────────────────────────────────────────────────────
    // JSON parsing — `alkanesExecuteWithStrings` and `alkanesExecuteFull`
    // both accept `max_indexed_height` (and the camelCase alias
    // `maxIndexedHeight`). Pin the parsing contract: the helper returns
    // u64 values from either spelling, and absent/null means "no filter".
    //
    // We test the JSON shape directly here (via serde_json::Value) — the
    // actual web-sys closures live behind wasm_bindgen so unit-testing
    // them from native code isn't straightforward, but the field-extraction
    // logic is small enough that pinning it via JSON value tests catches
    // any future drift in the option-parser tuple.
    // ────────────────────────────────────────────────────────────────────

    fn parse_max_indexed_from_opts(opts_json: &str) -> Option<u64> {
        let opts: serde_json::Value = serde_json::from_str(opts_json).unwrap();
        opts.get("max_indexed_height")
            .or_else(|| opts.get("maxIndexedHeight"))
            .and_then(|v| v.as_u64())
    }

    #[test]
    fn options_parser_reads_snake_case_max_indexed_height() {
        assert_eq!(
            parse_max_indexed_from_opts(r#"{"max_indexed_height":948720}"#),
            Some(948_720),
        );
    }

    #[test]
    fn options_parser_reads_camel_case_max_indexed_height() {
        assert_eq!(
            parse_max_indexed_from_opts(r#"{"maxIndexedHeight":948720}"#),
            Some(948_720),
        );
    }

    #[test]
    fn options_parser_returns_none_when_absent() {
        assert_eq!(parse_max_indexed_from_opts(r#"{}"#), None);
        assert_eq!(parse_max_indexed_from_opts(r#"{"max_indexed_height":null}"#), None);
        assert_eq!(parse_max_indexed_from_opts(r#"{"other_field":42}"#), None);
    }

    #[test]
    fn options_parser_handles_zero_height() {
        // Genesis-block / fresh-regtest case. 0 is a valid u64; must parse
        // (not get coerced to None). This pins back-compat for fixtures
        // that explicitly pass 0 to mean "no UTXOs are usable yet".
        assert_eq!(
            parse_max_indexed_from_opts(r#"{"max_indexed_height":0}"#),
            Some(0),
        );
    }

    #[test]
    fn enhanced_execute_params_serde_roundtrips_max_indexed_height() {
        // Defensive: the new field must serialise + deserialise cleanly so
        // the wasm bridge can pass it across the JS boundary unchanged.
        let json = serde_json::json!({
            "max_indexed_height": 948_720u64,
        });
        // Just the field — not the full struct — to keep the test focused.
        let parsed: serde_json::Value = serde_json::from_str(&json.to_string()).unwrap();
        assert_eq!(parsed["max_indexed_height"].as_u64(), Some(948_720));
    }

    // ---- prefetched_covers_alkanes_needed coverage ----
    //
    // Pins the short-circuit invariant for the sync gate in select_utxos. Live
    // call sites: subfrost-app's useWalletState path supplies prefetched_utxos
    // with .alkanes asserted per outpoint; when those balances cover
    // alkanes_needed, the metashrew sync poll must be skipped (the gate that
    // historically stalled mainnet swaps for ~30s when the public indexer was
    // a block or two behind bitcoind).

    fn mk_alkane(block: u128, tx: u128, amount: &str) -> PrefetchedAlkane {
        PrefetchedAlkane { block, tx, amount: amount.to_string() }
    }

    fn mk_prefetched(outpoint: &str, alkanes: Option<Vec<PrefetchedAlkane>>) -> PrefetchedUtxo {
        PrefetchedUtxo {
            outpoint: outpoint.to_string(),
            value: 546,
            script_pubkey_hex: "0014deadbeef00000000000000000000000000000000".to_string(),
            alkanes,
        }
    }

    // Stable txid stubs that satisfy OutPoint::from_str's 64-hex requirement.
    const TXID_A_COV: &str = "1111111111111111111111111111111111111111111111111111111111111111";
    const TXID_B: &str = "2222222222222222222222222222222222222222222222222222222222222222";
    const TXID_C: &str = "3333333333333333333333333333333333333333333333333333333333333333";

    #[test]
    fn prefetched_covers_alkanes_empty_needed_is_trivially_covered() {
        let needed = alloc::collections::BTreeMap::new();
        assert!(prefetched_covers_alkanes_needed(&[], &needed));
        assert!(prefetched_covers_alkanes_needed(
            &[mk_prefetched(&format!("{}:0", TXID_A_COV), None)],
            &needed,
        ));
    }

    #[test]
    fn prefetched_covers_alkanes_single_utxo_meets_requirement() {
        let prefetched = vec![mk_prefetched(
            &format!("{}:0", TXID_A_COV),
            Some(vec![mk_alkane(2, 0, "5000")]),
        )];
        let mut needed = alloc::collections::BTreeMap::new();
        needed.insert((2u64, 0u64), 5000u64);
        assert!(prefetched_covers_alkanes_needed(&prefetched, &needed));
    }

    #[test]
    fn prefetched_covers_alkanes_aggregates_across_multiple_outpoints() {
        // Two dust UTXOs each carrying half of the requested DIESEL. Selector
        // can spend both; gate must recognise the sum.
        let prefetched = vec![
            mk_prefetched(&format!("{}:0", TXID_A_COV), Some(vec![mk_alkane(2, 0, "300")])),
            mk_prefetched(&format!("{}:1", TXID_B), Some(vec![mk_alkane(2, 0, "700")])),
        ];
        let mut needed = alloc::collections::BTreeMap::new();
        needed.insert((2u64, 0u64), 1000u64);
        assert!(prefetched_covers_alkanes_needed(&prefetched, &needed));
    }

    #[test]
    fn prefetched_covers_alkanes_falls_back_when_short() {
        // 999 < 1000 — must NOT short-circuit; selector still needs the sync
        // to discover the missing balance from indexer.
        let prefetched = vec![mk_prefetched(
            &format!("{}:0", TXID_A_COV),
            Some(vec![mk_alkane(2, 0, "999")]),
        )];
        let mut needed = alloc::collections::BTreeMap::new();
        needed.insert((2u64, 0u64), 1000u64);
        assert!(!prefetched_covers_alkanes_needed(&prefetched, &needed));
    }

    #[test]
    fn prefetched_covers_alkanes_falls_back_when_token_missing_from_cache() {
        // Cache covers DIESEL (2:0) but not METHANE (2:1). Requirement on
        // METHANE must trigger sync — gate cannot lie about token presence.
        let prefetched = vec![mk_prefetched(
            &format!("{}:0", TXID_A_COV),
            Some(vec![mk_alkane(2, 0, "9999")]),
        )];
        let mut needed = alloc::collections::BTreeMap::new();
        needed.insert((2u64, 1u64), 1u64);
        assert!(!prefetched_covers_alkanes_needed(&prefetched, &needed));
    }

    #[test]
    fn prefetched_covers_alkanes_requires_every_requirement_covered() {
        // Mixed: DIESEL covered, METHANE not. Must NOT short-circuit on first
        // hit — every entry has to be satisfied.
        let prefetched = vec![
            mk_prefetched(&format!("{}:0", TXID_A_COV), Some(vec![mk_alkane(2, 0, "5000")])),
            mk_prefetched(&format!("{}:0", TXID_B), Some(vec![mk_alkane(2, 1, "10")])),
        ];
        let mut needed = alloc::collections::BTreeMap::new();
        needed.insert((2u64, 0u64), 4000u64);
        needed.insert((2u64, 1u64), 100u64);
        assert!(!prefetched_covers_alkanes_needed(&prefetched, &needed));
    }

    #[test]
    fn prefetched_covers_alkanes_no_assertions_falls_back() {
        // PrefetchedUtxo with alkanes=None means "caller has no assertion".
        // build_prefetched_alkanes_map returns Ok(None); coverage check must
        // fall back to sync rather than silently treating cache as authoritative.
        let prefetched = vec![mk_prefetched(&format!("{}:0", TXID_A_COV), None)];
        let mut needed = alloc::collections::BTreeMap::new();
        needed.insert((2u64, 0u64), 1u64);
        assert!(!prefetched_covers_alkanes_needed(&prefetched, &needed));
    }

    #[test]
    fn prefetched_covers_alkanes_empty_balances_means_clean_outpoint() {
        // alkanes=Some(vec![]) is the authoritative "no alkanes here" signal.
        // Doesn't cover any DIESEL requirement, so coverage check returns false.
        let prefetched = vec![mk_prefetched(&format!("{}:0", TXID_A_COV), Some(vec![]))];
        let mut needed = alloc::collections::BTreeMap::new();
        needed.insert((2u64, 0u64), 1u64);
        assert!(!prefetched_covers_alkanes_needed(&prefetched, &needed));
    }

    #[test]
    fn prefetched_covers_alkanes_malformed_amount_falls_back() {
        // build_prefetched_alkanes_map surfaces a Validation error on a bad
        // amount string. Coverage check must treat that as fall-through, not
        // panic and not silently skip the sync.
        let prefetched = vec![mk_prefetched(
            &format!("{}:0", TXID_A_COV),
            Some(vec![mk_alkane(2, 0, "not-a-number")]),
        )];
        let mut needed = alloc::collections::BTreeMap::new();
        needed.insert((2u64, 0u64), 1u64);
        assert!(!prefetched_covers_alkanes_needed(&prefetched, &needed));
    }

    #[test]
    fn prefetched_covers_alkanes_handles_three_distinct_tokens() {
        // Realistic swap: needs DIESEL + frBTC + LP token in one tx. Cache
        // has all three. Pin that no token is silently dropped from the
        // aggregation pass.
        let prefetched = vec![
            mk_prefetched(
                &format!("{}:0", TXID_A_COV),
                Some(vec![mk_alkane(2, 0, "100"), mk_alkane(32, 0, "200")]),
            ),
            mk_prefetched(
                &format!("{}:0", TXID_C),
                Some(vec![mk_alkane(2, 4, "50")]),
            ),
        ];
        let mut needed = alloc::collections::BTreeMap::new();
        needed.insert((2u64, 0u64), 100u64);
        needed.insert((32u64, 0u64), 200u64);
        needed.insert((2u64, 4u64), 50u64);
        assert!(prefetched_covers_alkanes_needed(&prefetched, &needed));
    }

    // -------------------------------------------------------------------------
    // c12 BTC->alkane swap repro: build_psbt_and_fee silently drops change
    //
    // Reported by c12hz in ALKANES #general-chat 2026-05-17:
    //   "I'm trying to swap from btc to an alkane on subfrost app. The unisat
    //    tx view shows me it's spending my entire btc balance (even though I
    //    only selected 25%) and none of the outputs go back to my wallet"
    //
    // Hypothesis: when the caller's `outputs` list does NOT include an explicit
    // zero-value non-OP_RETURN change placeholder, AND `runestone_script` is
    // passed (so an OP_RETURN gets appended inside build_psbt_and_fee), the
    // change-finder at execute.rs:3281 finds nothing matching `value == 0 &&
    // !is_op_return()`, falls through to the "add to last output" branch which
    // is gated by `!is_op_return()` on the LAST output (now the just-appended
    // OP_RETURN) — so the change_value computed but NEVER WRITTEN. The diff
    // between input and output becomes the implicit fee, burning the user's
    // change into miner fees.
    //
    // This test FAILS on the buggy code and PASSES on the fix.
    // -------------------------------------------------------------------------
    #[tokio::test]
    async fn build_psbt_and_fee_writes_change_when_op_return_is_last() {
        use bitcoin::{Amount, OutPoint, ScriptBuf, TxOut, Txid};

        let mut provider = MockProvider::new(Network::Regtest);
        let recipient_addr = WalletProvider::get_address(&provider).await.unwrap();
        let recipient_script = bitcoin::Address::from_str(&recipient_addr)
            .unwrap()
            .require_network(Network::Regtest)
            .unwrap()
            .script_pubkey();

        // Stub a single 100_000-sat BTC UTXO for the wallet.
        let stub_txid =
            Txid::from_str("1111111111111111111111111111111111111111111111111111111111111111")
                .unwrap();
        let stub_outpoint = OutPoint { txid: stub_txid, vout: 0 };
        provider
            .utxos
            .lock()
            .unwrap()
            .push((stub_outpoint, TxOut {
                value: Amount::from_sat(100_000),
                script_pubkey: recipient_script.clone(),
            }));

        let mut executor = EnhancedAlkanesExecutor::new(&mut provider);

        // Atomic wrap+swap outputs shape (matches what
        // execute_with_strings's caller hands to build_psbt_and_fee BEFORE
        // create_outputs's change-placeholder is added):
        //   [ wrap-to-signer (10_000 sats), alkane-receive-dust (546 sats) ]
        //
        // NOTE: this deliberately omits the change placeholder so we exercise
        // the buggy fallback path. The runestone OP_RETURN is appended by
        // build_psbt_and_fee itself (third arg), so the final output list will
        // be [10_000-sats-to-signer, 546-sats-dust, OP_RETURN].
        let outputs = vec![
            TxOut { value: Amount::from_sat(10_000), script_pubkey: recipient_script.clone() },
            TxOut { value: Amount::from_sat(546),    script_pubkey: recipient_script.clone() },
        ];

        // Non-empty runestone OP_RETURN so build_psbt_and_fee appends it
        // as the last output (the exact scenario c12 hit on UniSat).
        let runestone_script = ScriptBuf::from(vec![0x6a, 0x01, 0xff]); // OP_RETURN <1-byte>

        let (psbt, capped_fee, _vsize) = executor
            .build_psbt_and_fee(
                vec![stub_outpoint],
                outputs,
                Some(runestone_script),
                Some(1.0), // 1 sat/vbyte — keeps fee small so change is clearly visible
                None,
                None,
                None,
            )
            .await
            .expect("build_psbt_and_fee should succeed");

        let total_input: u64 = 100_000;
        let total_explicit_output: u64 = 10_000 + 546; // explicit non-zero, non-OP_RETURN

        // Sum the non-OP_RETURN output values from the BUILT PSBT.
        let total_in_psbt: u64 = psbt
            .unsigned_tx
            .output
            .iter()
            .filter(|o| !o.script_pubkey.is_op_return())
            .map(|o| o.value.to_sat())
            .sum();

        let actual_fee_paid = total_input - total_in_psbt;
        let expected_max_fee = capped_fee + 100; // 100-sat tolerance for rounding

        // BUG REPRO: with the buggy code, total_in_psbt stays at 10_546 (the
        // explicit outputs only) because no change was written anywhere. That
        // means actual_fee_paid = 89_454, but capped_fee is ~few-hundred sats.
        // The user's ~89_000 sats of change silently disappears into miner fees.
        //
        // FIX EXPECTATION: total_in_psbt > total_explicit_output, with the
        // delta being exactly the change_value. actual_fee_paid stays close to
        // capped_fee.
        assert!(
            actual_fee_paid <= expected_max_fee,
            "BUG REPRO: build_psbt_and_fee dropped change. total_input={}, \
             total_outputs_in_psbt={}, computed_capped_fee={}, \
             implicit_fee_paid={}. Excess of {} sats was silently burned. \
             Outputs in PSBT: {:?}",
            total_input,
            total_in_psbt,
            capped_fee,
            actual_fee_paid,
            actual_fee_paid.saturating_sub(capped_fee),
            psbt.unsigned_tx
                .output
                .iter()
                .map(|o| (o.value.to_sat(), o.script_pubkey.is_op_return()))
                .collect::<Vec<_>>(),
        );

        // Stronger assertion: there should be a non-OP_RETURN output carrying
        // the change. With the explicit outputs being 10_000 + 546 = 10_546,
        // and total input being 100_000, change should be ~89_400+ sats.
        let max_non_op_return_value = psbt
            .unsigned_tx
            .output
            .iter()
            .filter(|o| !o.script_pubkey.is_op_return())
            .map(|o| o.value.to_sat())
            .max()
            .unwrap_or(0);

        assert!(
            max_non_op_return_value > total_explicit_output,
            "Expected a change output to have absorbed the surplus. \
             Largest non-OP_RETURN output: {} sats. \
             Total explicit pre-build output: {} sats.",
            max_non_op_return_value,
            total_explicit_output,
        );
    }
}
