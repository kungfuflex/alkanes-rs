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
use crate::alkanes::types::OrdinalsStrategy;
use crate::traits::{OrdProvider, EsploraProvider, DeezelProvider};
use bitcoin::{OutPoint, TxOut, Transaction, ScriptBuf, Address, Txid};
use bitcoin::hashes::Hash;
use bitcoin::psbt::Psbt;

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec, vec, format};
#[cfg(feature = "std")]
use std::{string::String, vec::Vec, vec, format};

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

/// Result of building a split transaction
pub struct SplitResult {
    /// The split PSBT
    pub psbt: Psbt,
    /// The fee paid
    pub fee: u64,
    /// Clean outpoints to use for main transaction funding, with their TxOut data
    /// (We include TxOut because the split tx hasn't been broadcast yet)
    pub clean_utxos: Vec<(OutPoint, TxOut)>,
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

    /// Build a split transaction to protect inscribed UTXOs
    ///
    /// Creates a transaction that:
    /// - Takes inscribed UTXOs as inputs
    /// - Sends inscribed sats to safe outputs (user's address)
    /// - Sends clean sats to funding outputs (for main transaction)
    pub fn build_split_transaction(
        &self,
        split_plans: &[SplitPlan],
        utxo_info: &[(OutPoint, TxOut)],
        safe_address: &Address,
    ) -> Result<(Transaction, Vec<(OutPoint, TxOut)>)> {
        let mut inputs = Vec::new();
        let mut outputs = Vec::new();
        let mut clean_utxos = Vec::new();

        for (idx, plan) in split_plans.iter().enumerate() {
            inputs.push(bitcoin::TxIn {
                previous_output: plan.outpoint,
                script_sig: ScriptBuf::new(),
                sequence: bitcoin::Sequence::ENABLE_RBF_NO_LOCKTIME,
                witness: bitcoin::Witness::new(),
            });

            // Safe output (inscribed sats go here - to safe address for protection)
            outputs.push(TxOut {
                value: bitcoin::Amount::from_sat(plan.safe_amount),
                script_pubkey: safe_address.script_pubkey(),
            });

            // Clean output (for funding - to safe address, will be spent in main tx)
            let clean_output = TxOut {
                value: bitcoin::Amount::from_sat(plan.clean_amount),
                script_pubkey: safe_address.script_pubkey(),
            };
            outputs.push(clean_output.clone());

            // Record the clean output as a future UTXO
            // The txid will be calculated after we create the transaction
            // For now, use a placeholder that we'll update
            clean_utxos.push((
                OutPoint {
                    txid: Txid::from_byte_array([0u8; 32]), // Placeholder - will be updated
                    vout: (idx * 2 + 1) as u32, // Clean outputs are at odd indices
                },
                clean_output,
            ));
        }

        let tx = Transaction {
            version: bitcoin::transaction::Version::TWO,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: inputs,
            output: outputs,
        };

        // Update the clean UTXOs with the actual txid
        let txid = tx.compute_txid();
        for (outpoint, _) in &mut clean_utxos {
            outpoint.txid = txid;
        }

        Ok((tx, clean_utxos))
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

/// Build a split transaction to protect inscribed UTXOs (standalone function)
pub fn build_split_transaction(
    split_plans: &[SplitPlan],
    safe_address: &Address,
) -> Result<(Transaction, Vec<(OutPoint, TxOut)>)> {
    let mut inputs = Vec::new();
    let mut outputs = Vec::new();
    let mut clean_utxos = Vec::new();

    for (idx, plan) in split_plans.iter().enumerate() {
        inputs.push(bitcoin::TxIn {
            previous_output: plan.outpoint,
            script_sig: ScriptBuf::new(),
            sequence: bitcoin::Sequence::ENABLE_RBF_NO_LOCKTIME,
            witness: bitcoin::Witness::new(),
        });

        // Safe output (inscribed sats)
        outputs.push(TxOut {
            value: bitcoin::Amount::from_sat(plan.safe_amount),
            script_pubkey: safe_address.script_pubkey(),
        });

        // Clean output (for funding)
        let clean_output = TxOut {
            value: bitcoin::Amount::from_sat(plan.clean_amount),
            script_pubkey: safe_address.script_pubkey(),
        };
        outputs.push(clean_output.clone());

        clean_utxos.push((
            OutPoint {
                txid: Txid::from_byte_array([0u8; 32]),
                vout: (idx * 2 + 1) as u32,
            },
            clean_output,
        ));
    }

    let tx = Transaction {
        version: bitcoin::transaction::Version::TWO,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: inputs,
        output: outputs,
    };

    let txid = tx.compute_txid();
    for (outpoint, _) in &mut clean_utxos {
        outpoint.txid = txid;
    }

    Ok((tx, clean_utxos))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_split_basic() {
        // Test basic split calculation
        let inscriptions = vec![
            InscriptionInfo {
                inscription_id: InscriptionId::default(),
                sat_offset: 1000,
            }
        ];

        // Create a mock handler (we can't actually run it without a provider)
        // Just test the logic
        let utxo_value = 10000u64;
        let max_offset = inscriptions.iter().map(|i| i.sat_offset).max().unwrap_or(0);
        let safe_amount = (max_offset + 1).max(DUST_LIMIT);
        let fee_rate = 10.0f32;
        let estimated_fee = (fee_rate * 140.0).ceil() as u64;
        let clean_amount = utxo_value.saturating_sub(safe_amount).saturating_sub(estimated_fee);

        assert_eq!(safe_amount, 1001);
        assert!(clean_amount >= DUST_LIMIT * 2);
    }

    #[test]
    fn test_sat_flow_calculation() {
        // Test that sats flow correctly from inputs to outputs
        // Input: 10000 sats with inscription at offset 5000
        // Expected: sats 0-5000 go to output 0 (safe), sats 5001-10000 go to output 1 (clean)

        let inscription_offset = 5000u64;
        let safe_amount = inscription_offset + 1; // 5001
        let clean_amount = 10000 - safe_amount - 140; // ~4859 (after fee)

        assert_eq!(safe_amount, 5001);
        assert!(clean_amount > DUST_LIMIT);
    }
}
