//! Ordinals inscription handling for UTXO safety
//!
//! This module provides functionality for detecting and protecting ordinal inscriptions
//! when spending UTXOs. It supports three strategies:
//!
//! - **Exclude**: Fail if we must spend inscribed UTXOs (default, safest)
//! - **Preserve**: Split UTXOs to protect inscriptions before spending
//! - **Burn**: Allow spending inscribed UTXOs without protection
//!
//! When using the Preserve strategy, inscribed UTXOs are split into two outputs:
//! - Safe output: Contains the inscribed sats (sent to user's address)
//! - Clean output: Contains remaining sats (used for funding)
//!
//! The split transaction is broadcast atomically with the main transaction using
//! `sendrawtransactions` to prevent race conditions.

use crate::{AlkanesError, Result};
use crate::alkanes::types::{AlkaneId, OrdinalsStrategy};
use crate::traits::{OrdProvider, EsploraProvider, DeezelProvider};
use bitcoin::{OutPoint, TxOut, Transaction, ScriptBuf, Address, Txid};
use bitcoin::hashes::Hash;
use bitcoin::psbt::Psbt;
use ordinals::{Runestone, RuneId, Edict};
use protorune_support::protostone::{Protostones, Protostone, ProtostoneEdict as ProtoruneEdict};
use protorune_support::balance_sheet::ProtoruneRuneId;

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec, vec, format, collections::BTreeMap};
#[cfg(feature = "std")]
use std::{string::String, vec::Vec, vec, format, collections::BTreeMap};

use crate::vendored_ord::InscriptionId;

/// Minimum dust limit for outputs (546 sats for P2TR)
pub const DUST_LIMIT: u64 = 546;

/// Information about an inscription on a UTXO
#[derive(Debug, Clone)]
pub struct InscriptionInfo {
    /// Inscription ID
    pub inscription_id: InscriptionId,
    /// Offset of the inscribed sat within the UTXO (0-indexed)
    pub sat_offset: u64,
}

/// Traced inscription info for a pending UTXO
/// When a UTXO is unconfirmed, we trace back through parent transactions
/// to determine inscription state from settled UTXOs
#[derive(Debug, Clone)]
pub struct TracedInscriptionInfo {
    /// Original inscription ID (from the settled UTXO)
    pub inscription_id: InscriptionId,
    /// Current offset within this UTXO after sat flow through pending txs
    pub sat_offset: u64,
    /// Chain of txids from settled UTXO to this pending UTXO
    pub trace_path: Vec<Txid>,
}

/// Plan for splitting a UTXO to protect inscriptions
#[derive(Debug, Clone)]
pub struct SplitPlan {
    /// The outpoint being split
    pub outpoint: OutPoint,
    /// Amount to send to safe output (contains inscribed sats)
    pub safe_amount: u64,
    /// Amount to send to clean output (for funding)
    pub clean_amount: u64,
}

/// Result of building a split transaction.
pub struct SplitResult {
    /// The split PSBT. Every input already carries `witness_utxo` (and
    /// `tap_internal_key` where taproot), so it is ready to sign.
    pub psbt: Psbt,
    /// Estimated fee the split transaction pays, in sats.
    pub fee: u64,
    /// Clean outpoints the main transaction may fund from — one per plan,
    /// plus an optional residual change output when extras over-funded.
    ///
    /// The `TxOut` is carried deliberately: the split transaction has NOT
    /// been broadcast when the main transaction is signed, so no node can
    /// answer a prevout lookup for these. The caller must copy them into
    /// the main PSBT's `witness_utxo`, or signing silently produces an
    /// invalid taproot signature (the prevout value is committed in the
    /// sighash).
    pub clean_utxos: Vec<(OutPoint, TxOut)>,
    /// Clean outpoints carrying alkanes routed off the inscribed UTXOs,
    /// with the balances that landed on each.
    pub alkane_outpoints: Vec<(OutPoint, Vec<(AlkaneId, u128)>)>,
    /// Where rune balances were routed, if any rune-bearing inputs were
    /// consumed. Deliberately NOT part of `clean_utxos`: funding the main
    /// transaction from it would spend the runes straight back out again.
    pub rune_outpoint: Option<(OutPoint, Vec<(RuneId, u128)>)>,
    /// Extras consumed as additional inputs. The caller MUST remove these
    /// from the main transaction's inputs — they are spent by the split, so
    /// leaving them in place builds a transaction that double-spends them.
    pub consumed_extras: Vec<OutPoint>,
}

/// Addresses and knobs the split builder needs, resolved by the caller.
///
/// Resolution stays with the caller so the builder works for both the
/// alkanes execute path (which resolves `p2tr:0`-style identifiers out of
/// `EnhancedExecuteParams`) and plain wallet send (which already has a
/// concrete change address).
pub struct SplitConfig<'a> {
    /// Where inscribed sats and clean change are sent.
    pub safe_address: &'a Address,
    /// Where routed alkanes are sent. Often the same as `safe_address`.
    pub alkane_change_address: &'a Address,
    /// sat/vB used to size the split transaction's fee.
    pub fee_rate: f32,
    /// Skip appending the default DIESEL mint protostone.
    pub skip_diesel_mint: bool,
}

/// A rune-bearing UTXO to be spent by the split, with its balances routed to
/// an output of their own.
///
/// Runes are not protected the way inscriptions are. An inscription rides a
/// specific sat, so it is saved by cutting the UTXO at an offset. A rune
/// balance has no sat position at all — it moves by runestone edict — so the
/// only way to preserve it is to spend the UTXO and edict the balance onto a
/// dedicated output. `calculate_split`'s offset arithmetic is meaningless
/// here, which is why these arrive separately from [`SplitPlan`].
pub struct RuneInput {
    /// The outpoint being spent.
    pub outpoint: OutPoint,
    /// Its `TxOut`, for the input's `witness_utxo` and the funding maths.
    pub txout: TxOut,
    /// Rune balances it carries, as `(id, amount)`.
    pub balances: Vec<(RuneId, u128)>,
}

/// Assemble the single runestone OP_RETURN a split transaction may carry.
///
/// A transaction can hold only ONE runestone, so alkane routing (which
/// travels in `protocol` as encoded protostones) and rune routing (which
/// travels in `edicts`) have to share it rather than each emitting their own.
///
/// `pointer` is the RUNE pointer: it directs any rune balance the edicts did
/// not allocate. So when runes are being routed it must name the rune output
/// — leaving it on the alkane output would hand an unallocated rune remainder
/// to the wrong place. With no runes in play it keeps naming the alkane
/// output, which is what the alkanes execute path already relies on.
///
/// Returns `None` when there is nothing to encode, in which case the caller
/// must not emit an OP_RETURN at all.
pub(crate) fn assemble_runestone(
    alkane: Option<(u32, Vec<u128>)>,
    runes: Option<(u32, Vec<Edict>)>,
) -> Option<Runestone> {
    if alkane.is_none() && runes.is_none() {
        return None;
    }
    let pointer = runes
        .as_ref()
        .map(|(vout, _)| *vout)
        .or_else(|| alkane.as_ref().map(|(vout, _)| *vout));
    Some(Runestone {
        protocol: alkane.map(|(_, values)| values),
        edicts: runes.map(|(_, edicts)| edicts).unwrap_or_default(),
        pointer,
        ..Default::default()
    })
}

/// Handler for ordinal inscriptions on UTXOs
pub struct OrdinalsHandler<'a, P: OrdProvider + EsploraProvider> {
    provider: &'a P,
}

impl<'a, P: OrdProvider + EsploraProvider> OrdinalsHandler<'a, P> {
    /// Create a new ordinals handler
    pub fn new(provider: &'a P) -> Self {
        Self { provider }
    }

    /// Query ord for inscriptions on a specific UTXO
    /// Returns a list of inscription IDs and their sat offsets within the UTXO
    ///
    /// If ord is unavailable, logs a warning and returns empty list (fail-open)
    /// If mempool_indexer is enabled and the UTXO is pending, traces back through
    /// parent transactions to determine inscription state from settled UTXOs.
    pub async fn get_utxo_inscriptions(
        &self,
        outpoint: &OutPoint,
        mempool_indexer: bool,
    ) -> Result<Vec<InscriptionInfo>> {
        let output_str = format!("{}:{}", outpoint.txid, outpoint.vout);

        // Try to query ord for the output
        match self.provider.get_output(&output_str).await {
            Ok(output) => {
                // Check if output has inscriptions
                let inscription_ids = match output.inscriptions {
                    Some(ids) if !ids.is_empty() => ids,
                    _ => return Ok(vec![]), // No inscriptions
                };

                let mut inscriptions = Vec::new();

                // For each inscription, query its satpoint to get the offset
                for inscription_id in inscription_ids {
                    let inscription_id_str = inscription_id.to_string();
                    match self.provider.get_inscription(&inscription_id_str).await {
                        Ok(inscription) => {
                            // SatPoint contains outpoint and offset
                            // The offset tells us which sat within the UTXO is inscribed
                            inscriptions.push(InscriptionInfo {
                                inscription_id: inscription.id,
                                sat_offset: inscription.satpoint.offset,
                            });
                        }
                        Err(e) => {
                            log::warn!(
                                "Could not query inscription {}: {} - skipping",
                                inscription_id_str, e
                            );
                            // Continue with other inscriptions
                        }
                    }
                }

                if !inscriptions.is_empty() {
                    log::info!(
                        "Found {} inscription(s) on {}: {:?}",
                        inscriptions.len(),
                        output_str,
                        inscriptions.iter().map(|i| format!("{}@{}", i.inscription_id, i.sat_offset)).collect::<Vec<_>>()
                    );
                }

                Ok(inscriptions)
            }
            Err(e) => {
                // Ord can't find this output - it might be pending (unconfirmed)
                if mempool_indexer {
                    log::info!(
                        "🔍 Ord can't find {} - attempting mempool trace for pending UTXO",
                        output_str
                    );
                    // Try to trace back through parent transactions
                    match self.trace_pending_utxo_inscriptions(outpoint).await {
                        Ok(traced) => {
                            if !traced.is_empty() {
                                log::info!(
                                    "🔍 Traced {} inscription(s) on pending UTXO {}: {:?}",
                                    traced.len(),
                                    output_str,
                                    traced.iter().map(|i| format!("{}@{}", i.inscription_id, i.sat_offset)).collect::<Vec<_>>()
                                );
                            }
                            // Convert TracedInscriptionInfo to InscriptionInfo
                            Ok(traced.into_iter().map(|t| InscriptionInfo {
                                inscription_id: t.inscription_id,
                                sat_offset: t.sat_offset,
                            }).collect())
                        }
                        Err(trace_err) => {
                            log::warn!(
                                "⚠️ Could not trace pending UTXO {} - proceeding without inscription check: {}",
                                output_str, trace_err
                            );
                            Ok(vec![])
                        }
                    }
                } else {
                    // mempool_indexer disabled - fail-open with warning
                    log::warn!(
                        "⚠️ Could not query ord for {} - proceeding without inscription check: {}",
                        output_str, e
                    );
                    log::warn!(
                        "   Hint: Enable --mempool-indexer to trace inscription state of pending UTXOs"
                    );
                    Ok(vec![])
                }
            }
        }
    }

    /// Trace inscription state of a pending UTXO by backtracing through parent transactions
    ///
    /// When a UTXO is unconfirmed, ord can't tell us about its inscriptions.
    /// We trace back through the transaction chain until we find settled UTXOs,
    /// then calculate how inscriptions flow forward to determine the pending UTXO's state.
    pub async fn trace_pending_utxo_inscriptions(
        &self,
        outpoint: &OutPoint,
    ) -> Result<Vec<TracedInscriptionInfo>> {
        log::info!("🔍 Tracing pending UTXO: {}:{}", outpoint.txid, outpoint.vout);

        // Fetch the pending transaction
        let tx_hex = self.provider.get_tx_hex(&outpoint.txid.to_string()).await?;
        let tx_bytes = hex::decode(&tx_hex)?;
        let tx: Transaction = bitcoin::consensus::deserialize(&tx_bytes)?;

        // Get the output we care about
        let target_output = tx.output.get(outpoint.vout as usize)
            .ok_or_else(|| AlkanesError::Wallet(format!(
                "Output {} not found in tx {}", outpoint.vout, outpoint.txid
            )))?;
        let target_value = target_output.value.to_sat();

        // Calculate sat ranges for each output (ordinal-style sat flow)
        // Sats flow from inputs to outputs in order
        let mut output_sat_ranges: Vec<(u64, u64)> = Vec::new();
        let mut sat_cursor = 0u64;

        for output in &tx.output {
            let start = sat_cursor;
            let end = sat_cursor + output.value.to_sat();
            output_sat_ranges.push((start, end));
            sat_cursor = end;
        }

        let (target_start, target_end) = output_sat_ranges[outpoint.vout as usize];
        log::debug!("   Target output sat range: {}..{}", target_start, target_end);

        // Trace each input to find inscriptions
        let mut traced_inscriptions: Vec<TracedInscriptionInfo> = Vec::new();
        let mut input_sat_cursor = 0u64;

        for (input_idx, input) in tx.input.iter().enumerate() {
            let input_outpoint = &input.previous_output;

            // Try to get inscription info for this input
            // First check if it's settled (ord can find it)
            let input_output_str = format!("{}:{}", input_outpoint.txid, input_outpoint.vout);

            let (input_inscriptions, input_value) = match self.provider.get_output(&input_output_str).await {
                Ok(output) => {
                    // Settled UTXO - get inscriptions from ord
                    let mut inscriptions = Vec::new();
                    if let Some(ids) = output.inscriptions {
                        for inscription_id in ids {
                            let inscription_id_str = inscription_id.to_string();
                            if let Ok(inscription) = self.provider.get_inscription(&inscription_id_str).await {
                                inscriptions.push((inscription.id, inscription.satpoint.offset));
                            }
                        }
                    }
                    (inscriptions, output.value)
                }
                Err(_) => {
                    // This input is also pending - recursively trace it
                    log::debug!("   Input {} is also pending, recursively tracing...", input_idx);
                    let recursive_traced = Box::pin(self.trace_pending_utxo_inscriptions(input_outpoint)).await?;

                    // Get the input value from the parent transaction
                    let parent_tx_hex = self.provider.get_tx_hex(&input_outpoint.txid.to_string()).await?;
                    let parent_tx_bytes = hex::decode(&parent_tx_hex)?;
                    let parent_tx: Transaction = bitcoin::consensus::deserialize(&parent_tx_bytes)?;
                    let parent_output = parent_tx.output.get(input_outpoint.vout as usize)
                        .ok_or_else(|| AlkanesError::Wallet(format!(
                            "Output {} not found in parent tx {}", input_outpoint.vout, input_outpoint.txid
                        )))?;

                    let inscriptions: Vec<(InscriptionId, u64)> = recursive_traced.iter()
                        .map(|t| (t.inscription_id.clone(), t.sat_offset))
                        .collect();
                    (inscriptions, parent_output.value.to_sat())
                }
            };

            // Calculate which sats from this input flow to our target output
            let input_start = input_sat_cursor;
            let input_end = input_sat_cursor + input_value;
            input_sat_cursor = input_end;

            // Check if any inscription sats from this input land in our target output
            for (inscription_id, sat_offset_in_input) in input_inscriptions {
                // Calculate the absolute position of this inscribed sat
                let absolute_sat_pos = input_start + sat_offset_in_input;

                // Check if this sat lands in our target output
                if absolute_sat_pos >= target_start && absolute_sat_pos < target_end {
                    let new_offset = absolute_sat_pos - target_start;
                    log::debug!(
                        "   Inscription {} flows from input {} offset {} to output {} offset {}",
                        inscription_id, input_idx, sat_offset_in_input, outpoint.vout, new_offset
                    );
                    traced_inscriptions.push(TracedInscriptionInfo {
                        inscription_id,
                        sat_offset: new_offset,
                        trace_path: vec![outpoint.txid],
                    });
                }
            }
        }

        Ok(traced_inscriptions)
    }

    /// Calculate how to split a UTXO to protect inscriptions
    ///
    /// Given a UTXO with inscriptions at various offsets, calculates the split amounts:
    /// - Safe output: receives all sats up to and including the highest inscribed sat
    /// - Clean output: receives remaining sats (safe for funding)
    ///
    /// Returns None if no split is needed (all inscriptions are in the last sat which would
    /// go to change anyway, or not enough clean sats remain after split)
    pub fn calculate_split(
        &self,
        outpoint: OutPoint,
        utxo_value: u64,
        inscriptions: &[InscriptionInfo],
        _fee_rate: f32,
    ) -> Option<SplitPlan> {
        if inscriptions.is_empty() {
            return None;
        }

        let max_offset = inscriptions.iter().map(|i| i.sat_offset).max().unwrap_or(0);
        let safe_amount = (max_offset + 1).max(DUST_LIMIT);

        // Hard requirement: at least one sat past the inscription offset.
        // Fee + dust top-up are handled by the split-tx builder via extra
        // clean inputs from elsewhere in the wallet.
        if utxo_value <= safe_amount {
            log::warn!(
                "UTXO has {} sats but inscription at offset {} requires {} sats for safe output - cannot split",
                utxo_value, max_offset, safe_amount
            );
            return None;
        }

        let clean_amount = utxo_value - safe_amount;

        log::info!(
            "Split plan: {} sats → safe({}) + clean({}) (extra inputs may be pulled to cover fee/dust)",
            utxo_value, safe_amount, clean_amount
        );

        Some(SplitPlan {
            outpoint,
            safe_amount,
            clean_amount,
        })
    }

    /// Check selected UTXOs for inscriptions based on the ordinals strategy
    ///
    /// Returns:
    /// - Ok(None) if no inscriptions found or strategy is Burn
    /// - Ok(Some(plans)) if strategy is Preserve and inscribed UTXOs need splitting
    /// - Err if strategy is Exclude and inscribed UTXOs were found
    pub async fn check_utxos_for_inscriptions(
        &self,
        funding_utxos: &[(OutPoint, TxOut)],
        strategy: OrdinalsStrategy,
        fee_rate: f32,
        mempool_indexer: bool,
    ) -> Result<Option<Vec<SplitPlan>>> {
        match strategy {
            OrdinalsStrategy::Burn => {
                // Just proceed without checking
                log::debug!("Ordinals strategy: burn - skipping inscription check");
                Ok(None)
            }
            OrdinalsStrategy::Exclude | OrdinalsStrategy::Preserve => {
                let mut split_plans: Vec<SplitPlan> = Vec::new();
                let mut inscribed_utxos: Vec<String> = Vec::new();

                // Check each UTXO for inscriptions
                for (outpoint, txout) in funding_utxos {
                    let inscriptions = self.get_utxo_inscriptions(outpoint, mempool_indexer).await?;

                    if !inscriptions.is_empty() {
                        let utxo_value = txout.value.to_sat();

                        match strategy {
                            OrdinalsStrategy::Exclude => {
                                // Record this for error message
                                inscribed_utxos.push(format!("{} ({} inscriptions)", outpoint, inscriptions.len()));
                            }
                            OrdinalsStrategy::Preserve => {
                                // Calculate split plan
                                if let Some(plan) = self.calculate_split(*outpoint, utxo_value, &inscriptions, fee_rate) {
                                    split_plans.push(plan);
                                } else {
                                    // Cannot split this UTXO - return error
                                    return Err(AlkanesError::Wallet(format!(
                                        "UTXO {} contains inscriptions but cannot be safely split. \
                                        Please use a different UTXO without inscriptions or use --ordinals-strategy burn.",
                                        outpoint
                                    )));
                                }
                            }
                            _ => unreachable!(),
                        }
                    }
                }

                match strategy {
                    OrdinalsStrategy::Exclude if !inscribed_utxos.is_empty() => {
                        Err(AlkanesError::Wallet(format!(
                            "Cannot proceed: the following UTXOs contain inscriptions and ordinals_strategy is 'exclude':\n  {}\n\
                            Use --ordinals-strategy preserve to protect inscriptions, or --ordinals-strategy burn to allow spending them.",
                            inscribed_utxos.join("\n  ")
                        )))
                    }
                    OrdinalsStrategy::Preserve if !split_plans.is_empty() => {
                        log::info!("🔀 Found {} inscribed UTXO(s) requiring split transaction", split_plans.len());
                        Ok(Some(split_plans))
                    }
                    _ => Ok(None),
                }
            }
        }
    }

}

/// Helper functions for use with DeezelProvider (trait object compatible)
/// These are standalone functions that can be used in the execute workflow

/// Check UTXOs for inscriptions based on ordinals strategy (DeezelProvider compatible)
///
/// Returns:
/// - Ok(None) if no inscriptions found or strategy is Burn
/// - Ok(Some(plans)) if strategy is Preserve and inscribed UTXOs need splitting
/// - Err if strategy is Exclude and inscribed UTXOs were found
pub async fn check_utxos_for_inscriptions_with_provider(
    provider: &dyn DeezelProvider,
    funding_utxos: &[(OutPoint, TxOut)],
    strategy: OrdinalsStrategy,
    fee_rate: f32,
    mempool_indexer: bool,
) -> Result<Option<Vec<SplitPlan>>> {
    match strategy {
        OrdinalsStrategy::Burn => {
            log::debug!("Ordinals strategy: burn - skipping inscription check");
            Ok(None)
        }
        OrdinalsStrategy::Exclude | OrdinalsStrategy::Preserve => {
            let mut split_plans: Vec<SplitPlan> = Vec::new();
            let mut inscribed_utxos: Vec<String> = Vec::new();

            for (outpoint, txout) in funding_utxos {
                let inscriptions = get_utxo_inscriptions_with_provider(
                    provider,
                    outpoint,
                    mempool_indexer,
                ).await?;

                if !inscriptions.is_empty() {
                    let utxo_value = txout.value.to_sat();

                    match strategy {
                        OrdinalsStrategy::Exclude => {
                            inscribed_utxos.push(format!("{} ({} inscriptions)", outpoint, inscriptions.len()));
                        }
                        OrdinalsStrategy::Preserve => {
                            if let Some(plan) = calculate_split(*outpoint, utxo_value, &inscriptions, fee_rate) {
                                split_plans.push(plan);
                            } else {
                                return Err(AlkanesError::Wallet(format!(
                                    "UTXO {} contains inscriptions but cannot be safely split. \
                                    Please use a different UTXO or use --ordinals-strategy burn.",
                                    outpoint
                                )));
                            }
                        }
                        _ => unreachable!(),
                    }
                }
            }

            match strategy {
                OrdinalsStrategy::Exclude if !inscribed_utxos.is_empty() => {
                    Err(AlkanesError::Wallet(format!(
                        "Cannot proceed: the following UTXOs contain inscriptions and ordinals_strategy is 'exclude':\n  {}\n\
                        Use --ordinals-strategy preserve to protect inscriptions, or --ordinals-strategy burn to allow spending them.",
                        inscribed_utxos.join("\n  ")
                    )))
                }
                OrdinalsStrategy::Preserve if !split_plans.is_empty() => {
                    log::info!("🔀 Found {} inscribed UTXO(s) requiring split transaction", split_plans.len());
                    Ok(Some(split_plans))
                }
                _ => Ok(None),
            }
        }
    }
}

/// Query ord for inscriptions on a specific UTXO (DeezelProvider compatible)
pub async fn get_utxo_inscriptions_with_provider(
    provider: &dyn DeezelProvider,
    outpoint: &OutPoint,
    mempool_indexer: bool,
) -> Result<Vec<InscriptionInfo>> {
    let output_str = format!("{}:{}", outpoint.txid, outpoint.vout);

    match provider.get_output(&output_str).await {
        Ok(output) => {
            let inscription_ids = match output.inscriptions {
                Some(ids) if !ids.is_empty() => ids,
                _ => return Ok(vec![]),
            };

            let mut inscriptions = Vec::new();

            for inscription_id in inscription_ids {
                let inscription_id_str = inscription_id.to_string();
                match provider.get_inscription(&inscription_id_str).await {
                    Ok(inscription) => {
                        inscriptions.push(InscriptionInfo {
                            inscription_id: inscription.id,
                            sat_offset: inscription.satpoint.offset,
                        });
                    }
                    Err(e) => {
                        log::warn!(
                            "Could not query inscription {}: {} - skipping",
                            inscription_id_str, e
                        );
                    }
                }
            }

            if !inscriptions.is_empty() {
                log::info!(
                    "Found {} inscription(s) on {}: {:?}",
                    inscriptions.len(),
                    output_str,
                    inscriptions.iter().map(|i| format!("{}@{}", i.inscription_id, i.sat_offset)).collect::<Vec<_>>()
                );
            }

            Ok(inscriptions)
        }
        Err(e) => {
            if mempool_indexer {
                log::info!(
                    "🔍 Ord can't find {} - attempting mempool trace for pending UTXO",
                    output_str
                );
                match trace_pending_utxo_inscriptions_with_provider(provider, outpoint).await {
                    Ok(traced) => {
                        if !traced.is_empty() {
                            log::info!(
                                "🔍 Traced {} inscription(s) on pending UTXO {}: {:?}",
                                traced.len(),
                                output_str,
                                traced.iter().map(|i| format!("{}@{}", i.inscription_id, i.sat_offset)).collect::<Vec<_>>()
                            );
                        }
                        Ok(traced.into_iter().map(|t| InscriptionInfo {
                            inscription_id: t.inscription_id,
                            sat_offset: t.sat_offset,
                        }).collect())
                    }
                    Err(trace_err) => {
                        log::warn!(
                            "⚠️ Could not trace pending UTXO {} - proceeding without inscription check: {}",
                            output_str, trace_err
                        );
                        Ok(vec![])
                    }
                }
            } else {
                log::warn!(
                    "⚠️ Could not query ord for {} - proceeding without inscription check: {}",
                    output_str, e
                );
                log::warn!(
                    "   Hint: Enable --mempool-indexer to trace inscription state of pending UTXOs"
                );
                Ok(vec![])
            }
        }
    }
}

/// Trace inscription state of a pending UTXO (DeezelProvider compatible)
pub async fn trace_pending_utxo_inscriptions_with_provider(
    provider: &dyn DeezelProvider,
    outpoint: &OutPoint,
) -> Result<Vec<TracedInscriptionInfo>> {
    log::info!("🔍 Tracing pending UTXO: {}:{}", outpoint.txid, outpoint.vout);

    let tx_hex = provider.get_tx_hex(&outpoint.txid.to_string()).await?;
    let tx_bytes = hex::decode(&tx_hex)?;
    let tx: Transaction = bitcoin::consensus::deserialize(&tx_bytes)?;

    let target_output = tx.output.get(outpoint.vout as usize)
        .ok_or_else(|| AlkanesError::Wallet(format!(
            "Output {} not found in tx {}", outpoint.vout, outpoint.txid
        )))?;
    let _target_value = target_output.value.to_sat();

    // Calculate sat ranges for each output
    let mut output_sat_ranges: Vec<(u64, u64)> = Vec::new();
    let mut sat_cursor = 0u64;

    for output in &tx.output {
        let start = sat_cursor;
        let end = sat_cursor + output.value.to_sat();
        output_sat_ranges.push((start, end));
        sat_cursor = end;
    }

    let (target_start, target_end) = output_sat_ranges[outpoint.vout as usize];
    log::debug!("   Target output sat range: {}..{}", target_start, target_end);

    let mut traced_inscriptions: Vec<TracedInscriptionInfo> = Vec::new();
    let mut input_sat_cursor = 0u64;

    for (input_idx, input) in tx.input.iter().enumerate() {
        let input_outpoint = &input.previous_output;
        let input_output_str = format!("{}:{}", input_outpoint.txid, input_outpoint.vout);

        let (input_inscriptions, input_value) = match provider.get_output(&input_output_str).await {
            Ok(output) => {
                let mut inscriptions = Vec::new();
                if let Some(ids) = output.inscriptions {
                    for inscription_id in ids {
                        let inscription_id_str = inscription_id.to_string();
                        if let Ok(inscription) = provider.get_inscription(&inscription_id_str).await {
                            inscriptions.push((inscription.id, inscription.satpoint.offset));
                        }
                    }
                }
                (inscriptions, output.value)
            }
            Err(_) => {
                log::debug!("   Input {} is also pending, recursively tracing...", input_idx);
                let recursive_traced = Box::pin(trace_pending_utxo_inscriptions_with_provider(provider, input_outpoint)).await?;

                let parent_tx_hex = provider.get_tx_hex(&input_outpoint.txid.to_string()).await?;
                let parent_tx_bytes = hex::decode(&parent_tx_hex)?;
                let parent_tx: Transaction = bitcoin::consensus::deserialize(&parent_tx_bytes)?;
                let parent_output = parent_tx.output.get(input_outpoint.vout as usize)
                    .ok_or_else(|| AlkanesError::Wallet(format!(
                        "Output {} not found in parent tx {}", input_outpoint.vout, input_outpoint.txid
                    )))?;

                let inscriptions: Vec<(InscriptionId, u64)> = recursive_traced.iter()
                    .map(|t| (t.inscription_id.clone(), t.sat_offset))
                    .collect();
                (inscriptions, parent_output.value.to_sat())
            }
        };

        let input_start = input_sat_cursor;
        let input_end = input_sat_cursor + input_value;
        input_sat_cursor = input_end;

        for (inscription_id, sat_offset_in_input) in input_inscriptions {
            let absolute_sat_pos = input_start + sat_offset_in_input;

            if absolute_sat_pos >= target_start && absolute_sat_pos < target_end {
                let new_offset = absolute_sat_pos - target_start;
                log::debug!(
                    "   Inscription {} flows from input {} offset {} to output {} offset {}",
                    inscription_id, input_idx, sat_offset_in_input, outpoint.vout, new_offset
                );
                traced_inscriptions.push(TracedInscriptionInfo {
                    inscription_id,
                    sat_offset: new_offset,
                    trace_path: vec![outpoint.txid],
                });
            }
        }
    }

    Ok(traced_inscriptions)
}

/// Calculate how to split a UTXO to protect inscriptions (standalone function).
///
/// Returns the inscribed-UTXO breakdown only: safe (inscription) and clean
/// (remainder). The split-tx builder is responsible for pulling additional
/// clean inputs from elsewhere in the wallet to cover fees and dust thresholds —
/// this function does NOT require the inscribed UTXO to self-fund the split.
/// That requirement was overly strict: most ordinal mints land on small UTXOs
/// (~546-1500 sats) precisely because that minimizes inscriber cost, leaving
/// no headroom for both a safe output AND a usable clean output AND the
/// split-tx fee. With external funding, those small inscribed UTXOs split
/// fine.
///
/// The only hard requirement is `utxo_value > safe_amount` — there must be
/// at least one sat past the inscription offset for the clean output to
/// exist. If `clean_amount` ends up below dust, the builder will top it up
/// from extra funding (and pay the fee from extras as well).
pub fn calculate_split(
    outpoint: OutPoint,
    utxo_value: u64,
    inscriptions: &[InscriptionInfo],
    _fee_rate: f32,
) -> Option<SplitPlan> {
    if inscriptions.is_empty() {
        return None;
    }

    let max_offset = inscriptions.iter().map(|i| i.sat_offset).max().unwrap_or(0);
    let safe_amount = (max_offset + 1).max(DUST_LIMIT);

    if utxo_value <= safe_amount {
        log::warn!(
            "UTXO has {} sats but inscription at offset {} requires {} sats for safe output - cannot split",
            utxo_value, max_offset, safe_amount
        );
        return None;
    }

    let clean_amount = utxo_value - safe_amount;

    log::info!(
        "Split plan: {} sats → safe({}) + clean({}) (extra inputs may be pulled to cover fee/dust)",
        utxo_value, safe_amount, clean_amount
    );

    Some(SplitPlan {
        outpoint,
        safe_amount,
        clean_amount,
    })
}

/// The default DIESEL mint protostone, pointed at `pointer`.
///
/// Lives here rather than on the alkanes executor so the split builder — used
/// by both the alkanes execute path and plain wallet send — can append it
/// without reaching into that module.
pub fn diesel_mint_protostone(pointer: u32) -> Protostone {
    Protostone {
        protocol_tag: 1,
        message: vec![2u8, 0u8, 77u8],
        pointer: Some(pointer),
        refund: Some(pointer),
        burn: None,
        from: None,
        edicts: vec![],
    }
}

/// Build a split PSBT that lifts inscribed sats onto their own outputs.
///
/// This is the real builder, extracted from the alkanes execute path so the
/// plain wallet-send path can use it too. (Two naive `build_split_transaction`
/// helpers used to live here; both had zero callers and produced zero-fee
/// transactions, because their outputs summed to exactly their inputs.)
///
/// Layout: for each plan, a `safe` output holding every sat up to and
/// including the highest inscribed offset, then a `clean` output with the
/// remainder — clean outputs land on odd indices, which is what
/// [`SplitResult::clean_utxos`] reports. Alkanes riding on the inscribed
/// UTXOs are routed to a dedicated output via a protostone OP_RETURN, so
/// splitting for an inscription never burns tokens that shared the UTXO.
///
/// `extra_funding_utxos` is what makes this usable in practice. Most ordinal
/// mints land on small (~546-1500 sat) UTXOs because that minimises inscriber
/// cost, and such a UTXO cannot pay for its own split (safe + clean + fee
/// exceeds its value). The builder pulls clean inputs from elsewhere in the
/// wallet to cover the fee and to top up clean outputs that would otherwise
/// fall below dust; any surplus comes back as a residual change output rather
/// than being burned as fee. Callers must pre-filter this set: anything in it
/// is spent unconditionally, so it must carry no inscriptions and no alkanes.
pub async fn build_split_psbt(
    provider: &dyn DeezelProvider,
    plans: &[SplitPlan],
    funding_utxos: &[(OutPoint, TxOut)],
    extra_funding_utxos: &[(OutPoint, TxOut)],
    cfg: &SplitConfig<'_>,
    split_utxo_alkanes: &BTreeMap<OutPoint, Vec<(AlkaneId, u128)>>,
    rune_inputs: &[RuneInput],
) -> Result<SplitResult> {
    use bitcoin::transaction::Version;

    let fee_rate = cfg.fee_rate;
    let safe_script = cfg.safe_address.script_pubkey();

    let mut inputs = Vec::new();
    let mut outputs = Vec::new();
    let mut input_txouts = Vec::new();
    let mut clean_utxos: Vec<(OutPoint, TxOut)> = Vec::new();
    let mut total_input_value = 0u64;
    // How much each plan's clean output was inflated above its natural
    // `clean_amount` because that amount was below dust. Owed to extras.
    let mut clean_topup_owed: u64 = 0;

    for (idx, plan) in plans.iter().enumerate() {
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

        // Safe output: the inscribed sats.
        outputs.push(TxOut {
            value: bitcoin::Amount::from_sat(plan.safe_amount),
            script_pubkey: safe_script.clone(),
        });

        // Clean output: the funding remainder. Below-dust remainders are
        // topped up to DUST_LIMIT from extras — including a zero remainder,
        // so the caller always gets a usable funding UTXO at the canonical
        // odd index.
        let clean_value = if plan.clean_amount < DUST_LIMIT {
            clean_topup_owed += DUST_LIMIT - plan.clean_amount;
            DUST_LIMIT
        } else {
            plan.clean_amount
        };
        let clean_txout = TxOut {
            value: bitcoin::Amount::from_sat(clean_value),
            script_pubkey: safe_script.clone(),
        };
        outputs.push(clean_txout.clone());

        clean_utxos.push((
            OutPoint {
                txid: Txid::from_byte_array([0u8; 32]), // placeholder, set once the txid is known
                vout: (idx * 2 + 1) as u32,
            },
            clean_txout,
        ));
    }

    // Rune-bearing inputs. Unlike an inscription there is nothing to cut:
    // a rune balance has no sat position, so the UTXO is spent whole and the
    // balance edicted onto an output of its own. Appended AFTER the plan loop
    // so the `idx * 2 + 1` indexing of clean outputs stays valid.
    for rune_input in rune_inputs {
        total_input_value += rune_input.txout.value.to_sat();
        inputs.push(bitcoin::TxIn {
            previous_output: rune_input.outpoint,
            script_sig: ScriptBuf::new(),
            sequence: bitcoin::Sequence::ENABLE_RBF_NO_LOCKTIME,
            witness: bitcoin::Witness::new(),
        });
        input_txouts.push(rune_input.txout.clone());
    }

    // Alkane-aware split: route any alkanes on the inscribed UTXOs to a
    // dedicated clean output, so splitting for an inscription doesn't burn
    // tokens that happened to share the UTXO.
    let has_alkanes = !split_utxo_alkanes.is_empty();
    let mut alkane_outpoints: Vec<(OutPoint, Vec<(AlkaneId, u128)>)> = Vec::new();
    let mut alkane_routing: Option<(u32, Vec<u128>)> = None;

    if has_alkanes {
        let alkane_output_index = outputs.len() as u32;
        outputs.push(TxOut {
            value: bitcoin::Amount::from_sat(DUST_LIMIT),
            script_pubkey: cfg.alkane_change_address.script_pubkey(),
        });

        let mut aggregated_alkanes: BTreeMap<AlkaneId, u128> = BTreeMap::new();
        for (_outpoint, alkanes) in split_utxo_alkanes {
            for (alkane_id, amount) in alkanes {
                *aggregated_alkanes.entry(alkane_id.clone()).or_insert(0) += amount;
            }
        }

        let mut protostone_edicts = Vec::new();
        let mut alkane_output_balances = Vec::new();
        for (alkane_id, amount) in &aggregated_alkanes {
            protostone_edicts.push(ProtoruneEdict {
                id: ProtoruneRuneId {
                    block: alkane_id.block as u128,
                    tx: alkane_id.tx as u128,
                },
                amount: *amount,
                output: alkane_output_index as u128,
            });
            alkane_output_balances.push((alkane_id.clone(), *amount));
            log::info!("  Split alkane edict: {}:{} × {} → v{}",
                alkane_id.block, alkane_id.tx, amount, alkane_output_index);
        }

        let split_protostone = Protostone {
            protocol_tag: 1u128,
            message: vec![],
            pointer: Some(alkane_output_index),
            refund: Some(alkane_output_index),
            edicts: protostone_edicts,
            from: None,
            burn: None,
        };

        let mut split_protostones = vec![split_protostone];
        if !cfg.skip_diesel_mint {
            split_protostones.push(diesel_mint_protostone(alkane_output_index));
        }
        alkane_routing = Some((alkane_output_index, split_protostones.encipher()?));

        alkane_outpoints.push((
            OutPoint {
                txid: Txid::from_byte_array([0u8; 32]), // placeholder, set below
                vout: alkane_output_index,
            },
            alkane_output_balances,
        ));

        log::info!("🔗 Added alkane routing: {} alkane type(s) → clean output v{}",
            aggregated_alkanes.len(), alkane_output_index);
    }

    // Rune routing: one dedicated output, one edict per rune id. This output
    // is deliberately kept out of `clean_utxos` — funding the main
    // transaction from it would spend the runes straight back out.
    let mut rune_outpoint: Option<(OutPoint, Vec<(RuneId, u128)>)> = None;
    let mut rune_routing: Option<(u32, Vec<Edict>)> = None;

    if !rune_inputs.is_empty() {
        let rune_output_index = outputs.len() as u32;
        outputs.push(TxOut {
            value: bitcoin::Amount::from_sat(DUST_LIMIT),
            script_pubkey: safe_script.clone(),
        });

        let mut aggregated_runes: BTreeMap<RuneId, u128> = BTreeMap::new();
        for rune_input in rune_inputs {
            for (id, amount) in &rune_input.balances {
                *aggregated_runes.entry(*id).or_insert(0) += *amount;
            }
        }

        let mut edicts = Vec::new();
        let mut rune_balances = Vec::new();
        for (id, amount) in &aggregated_runes {
            edicts.push(Edict { id: *id, amount: *amount, output: rune_output_index });
            rune_balances.push((*id, *amount));
            log::info!("  Split rune edict: {} × {} → v{}", id, amount, rune_output_index);
        }

        rune_routing = Some((rune_output_index, edicts));
        rune_outpoint = Some((
            OutPoint {
                txid: Txid::from_byte_array([0u8; 32]), // placeholder, set below
                vout: rune_output_index,
            },
            rune_balances,
        ));

        log::info!("🪙 Added rune routing: {} rune(s) → output v{}",
            aggregated_runes.len(), rune_output_index);
    }

    // ONE runestone carries both, or none is emitted at all. See
    // `assemble_runestone` for why the pointer follows the runes.
    if let Some(runestone) = assemble_runestone(alkane_routing, rune_routing) {
        outputs.push(TxOut {
            value: bitcoin::Amount::ZERO,
            script_pubkey: runestone.encipher(),
        });
    }

    let total_output_value: u64 = outputs.iter().map(|o| o.value.to_sat()).sum();

    // Pull extra clean inputs to cover (a) dust top-ups on clean outputs,
    // (b) the split fee. Each extra input costs ~68 vbytes, which raises the
    // fee, so add one at a time and re-estimate until the shortfall clears.
    let mut consumed_extras: Vec<OutPoint> = Vec::new();
    let mut extras_iter = extra_funding_utxos.iter();

    // P2TR input ~68 vbytes, P2TR output ~43 vbytes, tx overhead ~10 vbytes.
    let recompute_fee = |inputs_len: usize, outputs_len: usize| -> u64 {
        let vsize = 10 + inputs_len * 68 + outputs_len * 43;
        (fee_rate * vsize as f32).ceil() as u64
    };

    let mut estimated_fee = recompute_fee(inputs.len(), outputs.len());

    loop {
        let needed = total_output_value
            .saturating_add(estimated_fee)
            .saturating_sub(total_input_value);
        if needed == 0 {
            break;
        }

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

        estimated_fee = recompute_fee(inputs.len(), outputs.len());
    }

    // Return any surplus as a residual change output rather than burning it
    // as fee — but only once it clears dust after paying for its own ~43
    // vbytes.
    let surplus = total_input_value - total_output_value - estimated_fee;
    if surplus >= DUST_LIMIT + (fee_rate * 43.0).ceil() as u64 {
        let residual_fee_delta = (fee_rate * 43.0).ceil() as u64;
        let residual_txout = TxOut {
            value: bitcoin::Amount::from_sat(surplus - residual_fee_delta),
            script_pubkey: safe_script.clone(),
        };
        outputs.push(residual_txout.clone());
        estimated_fee += residual_fee_delta;
        clean_utxos.push((
            OutPoint {
                txid: Txid::from_byte_array([0u8; 32]), // placeholder, set below
                vout: (outputs.len() - 1) as u32,
            },
            residual_txout,
        ));
    }

    let unsigned_tx = Transaction {
        version: Version::TWO,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: inputs,
        output: outputs,
    };

    let mut psbt = Psbt::from_unsigned_tx(unsigned_tx)?;

    // Every input gets its witness_utxo, so this PSBT is signable without a
    // node round-trip — which matters because the main transaction spends
    // these outputs before this one is broadcast.
    for (i, txout) in input_txouts.iter().enumerate() {
        psbt.inputs[i].witness_utxo = Some(txout.clone());
        if txout.script_pubkey.is_p2tr() {
            let (internal_key, (fingerprint, path)) = provider.get_internal_key().await?;
            psbt.inputs[i].tap_internal_key = Some(internal_key);
            psbt.inputs[i].tap_key_origins.insert(internal_key, (vec![], (fingerprint, path)));
        }
    }

    // Now that the transaction is final, stamp the real txid over every
    // placeholder.
    let txid = psbt.unsigned_tx.compute_txid();
    for (outpoint, _) in &mut clean_utxos {
        outpoint.txid = txid;
    }
    for (outpoint, _) in &mut alkane_outpoints {
        outpoint.txid = txid;
    }
    if let Some((outpoint, _)) = &mut rune_outpoint {
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

    Ok(SplitResult {
        psbt,
        fee: estimated_fee,
        clean_utxos,
        alkane_outpoints,
        rune_outpoint,
        consumed_extras,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn op() -> OutPoint {
        OutPoint { txid: Txid::from_byte_array([7u8; 32]), vout: 0 }
    }

    fn inscribed_at(offset: u64) -> InscriptionInfo {
        InscriptionInfo { inscription_id: InscriptionId::default(), sat_offset: offset }
    }

    // NOTE: the two tests these replaced asserted on arithmetic they
    // performed inline and never called `calculate_split` at all — they
    // would have passed with the function deleted. These call it.

    #[test]
    fn the_safe_output_covers_the_highest_inscribed_offset_not_the_first() {
        // Two inscriptions on one UTXO: sizing the safe output off the first
        // one seen would leave the higher inscribed sat in the clean output,
        // where it gets spent as ordinary funding.
        let plan = calculate_split(op(), 10_000, &[inscribed_at(250), inscribed_at(1000)], 10.0)
            .expect("splittable");

        assert_eq!(plan.safe_amount, 1001);
        assert_eq!(plan.clean_amount, 10_000 - 1001);
    }

    #[test]
    fn the_split_neither_invents_nor_loses_sats() {
        let plan = calculate_split(op(), 10_000, &[inscribed_at(1000)], 10.0).expect("splittable");
        assert_eq!(plan.safe_amount + plan.clean_amount, 10_000);
    }

    #[test]
    fn a_low_offset_inscription_still_reserves_a_spendable_safe_output() {
        // An inscription on sat 0 needs only 1 sat to be "covered", but a
        // 1-sat output is unspendable dust, so the floor applies.
        let plan = calculate_split(op(), 10_000, &[inscribed_at(0)], 1.0).expect("splittable");
        assert_eq!(plan.safe_amount, DUST_LIMIT);
    }

    #[test]
    fn a_utxo_with_no_headroom_past_the_inscription_cannot_split() {
        // The inscribed sat is the last one: there is no clean remainder to
        // fund anything with, so there is no safe way to split.
        assert!(calculate_split(op(), 10_000, &[inscribed_at(9_999)], 1.0).is_none());
    }

    #[test]
    fn no_inscriptions_means_no_split_is_needed() {
        assert!(calculate_split(op(), 10_000, &[], 1.0).is_none());
    }

    fn rid(block: u64, tx: u32) -> RuneId {
        RuneId { block, tx }
    }

    #[test]
    fn nothing_to_route_emits_no_runestone() {
        // The caller must not add an OP_RETURN in this case.
        assert!(assemble_runestone(None, None).is_none());
    }

    #[test]
    fn alkanes_alone_keep_the_pointer_on_the_alkane_output() {
        let rs = assemble_runestone(Some((3, vec![7u128, 8u128])), None).expect("runestone");
        assert_eq!(rs.pointer, Some(3));
        assert_eq!(rs.protocol, Some(vec![7u128, 8u128]));
        assert!(rs.edicts.is_empty());
    }

    #[test]
    fn runes_take_the_pointer_even_when_alkanes_are_present() {
        // `pointer` is the RUNE pointer: it directs unallocated rune
        // balance. Leaving it on the alkane output would hand a rune
        // remainder to the wrong place.
        let edicts = vec![Edict { id: rid(840_000, 1), amount: 5, output: 4 }];
        let rs = assemble_runestone(Some((3, vec![7u128])), Some((4, edicts.clone())))
            .expect("runestone");
        assert_eq!(rs.pointer, Some(4), "the rune output must win");
        assert_eq!(rs.protocol, Some(vec![7u128]), "alkane protostones still ride along");
        assert_eq!(rs.edicts, edicts);
    }

    #[test]
    fn runes_alone_point_at_the_rune_output() {
        let edicts = vec![Edict { id: rid(840_000, 9), amount: 1, output: 2 }];
        let rs = assemble_runestone(None, Some((2, edicts.clone()))).expect("runestone");
        assert_eq!(rs.pointer, Some(2));
        assert!(rs.protocol.is_none());
        assert_eq!(rs.edicts, edicts);
    }
}
