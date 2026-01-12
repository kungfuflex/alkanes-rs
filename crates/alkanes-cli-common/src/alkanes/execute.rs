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
    InputRequirement, OutputTarget, ProtostoneEdict, ProtostoneSpec, ReadyToSignCommitTx,
    ReadyToSignRevealTx, ReadyToSignTx,
};
use super::envelope::AlkanesEnvelope;
use anyhow::anyhow;
use ordinals::Runestone;
use protorune_support::protostone::{Protostones, Protostone, ProtostoneEdict as ProtoruneEdict};

const MAX_FEE_SATS: u64 = 100_000; // 0.001 BTC. Cap to avoid "absurdly high fee rate" errors.
const DUST_LIMIT: u64 = 546;

/// Result from UTXO selection including alkanes balances
#[derive(Debug, Clone)]
struct UtxoSelectionResult {
    /// Selected outpoints
    outpoints: Vec<OutPoint>,
    /// Actual alkanes balances found in the selected UTXOs
    alkanes_found: alloc::collections::BTreeMap<AlkaneId, u64>,
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
            .select_utxos(&[InputRequirement::Bitcoin { amount: required_reveal_amount }], &params.from_addresses)
            .await?;
        let funding_utxos = utxo_selection.outpoints.clone();

        // Check selected UTXOs for ordinal inscriptions based on strategy
        let final_funding_utxos = if params.ordinals_strategy != OrdinalsStrategy::Burn {
            let mut funding_utxos_with_txout: Vec<(OutPoint, TxOut)> = Vec::new();
            for outpoint in &funding_utxos {
                if let Some(txout) = self.provider.get_utxo(outpoint).await? {
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
        let utxo_selection = self.select_utxos(&final_requirements, &params.from_addresses).await?;

        // Check selected UTXOs for ordinal inscriptions based on strategy
        // We need to get TxOut data for each selected UTXO to check for inscriptions
        let mut funding_utxos_with_txout: Vec<(OutPoint, TxOut)> = Vec::new();
        for outpoint in &utxo_selection.outpoints {
            if let Some(txout) = self.provider.get_utxo(outpoint).await? {
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

                        // Build split transaction PSBT
                        let (split_psbt_result, split_fee_result, clean_outpoints) =
                            self.build_split_psbt(&plans, &funding_utxos_with_txout, fee_rate_sat_vb, params).await?;

                        // Replace inscribed UTXOs with clean UTXOs from split
                        let mut new_outpoints = Vec::new();
                        let inscribed_outpoints: std::collections::HashSet<OutPoint> =
                            plans.iter().map(|p| p.outpoint).collect();

                        // Keep non-inscribed UTXOs
                        for outpoint in &utxo_selection.outpoints {
                            if !inscribed_outpoints.contains(outpoint) {
                                new_outpoints.push(*outpoint);
                            }
                        }
                        // Add clean UTXOs from split
                        new_outpoints.extend(clean_outpoints);

                        log::info!("🔀 Split transaction built: {} clean UTXOs will replace {} inscribed UTXOs",
                            plans.len(), inscribed_outpoints.len());

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
        
        // Handle excess alkanes by generating automatic protostone
        let final_protostones = if !alkanes_excess.is_empty() {
            log::info!("🔄 Handling excess alkanes with automatic protostone generation");
            
            // Determine alkanes change address
            let alkanes_change_addr = params.alkanes_change_address.as_ref()
                .or(params.change_address.as_ref())
                .map(|s| s.as_str())
                .unwrap_or("p2tr:0");
            
            log::info!("Alkanes change will be sent to: {}", alkanes_change_addr);
            
            // Create alkanes change output
            // This will be the FIRST identifier output (v0) if we're generating automatic protostone
            // Or it could be a separate output - we need to determine the index
            let alkanes_change_output_index = if outputs.is_empty() {
                // No outputs yet, alkanes change will be v0
                0
            } else {
                // Alkanes change goes to first identifier output (v0)
                0
            };
            
            // Ensure we have an output at the alkanes change index
            // If outputs is empty or we need a specific output, create it
            if outputs.len() <= alkanes_change_output_index as usize {
                use crate::traits::AddressResolver;
                let resolved_addr = self.provider.resolve_all_identifiers(alkanes_change_addr).await?;
                let address = Address::from_str(&resolved_addr)?.require_network(self.provider.get_network())?;
                outputs.insert(alkanes_change_output_index as usize, TxOut {
                    value: bitcoin::Amount::from_sat(DUST_LIMIT),
                    script_pubkey: address.script_pubkey(),
                });
                log::info!("Created alkanes change output at index {}", alkanes_change_output_index);
            }
            
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
        let runestone_script = self.construct_runestone_script(&final_protostones, outputs.len())?;
        let (psbt, fee, estimated_vsize) = self.build_psbt_and_fee(final_funding_outpoints.clone(), outputs, Some(runestone_script), params.fee_rate, None, None).await?;

        // Validate the transaction before returning
        self.validate_transaction(&psbt, &final_funding_outpoints, fee, params).await?;

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
    ) -> Result<()> {
        let tx = &psbt.unsigned_tx;
        
        // 1. Calculate total input value
        let mut total_input_value = 0u64;
        for outpoint in selected_utxos {
            let utxo = self.provider.get_utxo(outpoint).await?
                .ok_or_else(|| AlkanesError::Wallet(format!("UTXO not found during validation: {outpoint}")))?;
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

    async fn select_utxos(&mut self, requirements: &[InputRequirement], from_addresses: &Option<Vec<String>>) -> Result<UtxoSelectionResult> {
        use crate::traits::AddressResolver;
        
        log::info!("Selecting UTXOs for {} requirements", requirements.len());
        if let Some(addrs) = from_addresses {
            log::info!("Sourcing UTXOs from: {addrs:?}");
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

        let utxos = self.provider.get_utxos(true, resolved_from_addresses).await?;
        log::debug!("Found {} total wallet UTXOs from specified sources", utxos.len());

        // Bitcoin requires coinbase outputs to have 100 confirmations before spending
        const COINBASE_MATURITY: u32 = 100;

        let spendable_utxos: Vec<(OutPoint, UtxoInfo)> = utxos.into_iter()
            .filter(|(_, info)| {
                // Filter out frozen UTXOs
                if info.frozen {
                    log::debug!("Skipping frozen UTXO: {}:{}", info.txid, info.vout);
                    return false;
                }
                
                // Filter out immature coinbase outputs
                if info.is_coinbase && info.confirmations < COINBASE_MATURITY {
                    log::debug!(
                        "Skipping immature coinbase UTXO: {}:{} (confirmations: {}, required: {})",
                        info.txid, info.vout, info.confirmations, COINBASE_MATURITY
                    );
                    return false;
                }
                
                true
            })
            .collect();
        
        log::info!("Found {} spendable (non-frozen) wallet UTXOs", spendable_utxos.len());

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

        let mut bitcoin_collected = 0u64;
        let mut alkanes_collected: alloc::collections::BTreeMap<(u64, u64), u64> = alloc::collections::BTreeMap::new();
        let mut alkanes_found: alloc::collections::BTreeMap<AlkaneId, u64> = alloc::collections::BTreeMap::new();

        // If we need alkanes, we must query each UTXO to find ones that contain the required alkanes
        if !alkanes_needed.is_empty() {
            log::info!("Querying UTXOs for alkane balances using batched approach...");
            
            // Group UTXOs by address for batch fetching
            let mut utxos_by_address: alloc::collections::BTreeMap<String, Vec<(OutPoint, UtxoInfo)>> = alloc::collections::BTreeMap::new();
            for (outpoint, utxo) in spendable_utxos.clone() {
                utxos_by_address.entry(utxo.address.clone()).or_insert_with(Vec::new).push((outpoint, utxo));
            }
            
            log::info!("Fetching balances for {} addresses (batch mode - 1 RPC call per address instead of {} calls)", 
                       utxos_by_address.len(), spendable_utxos.len());
            
            // Create a map of (txid:vout) -> balance data for quick lookup
            let mut utxo_balances: alloc::collections::BTreeMap<String, serde_json::Value> = alloc::collections::BTreeMap::new();
            
            // Batch fetch for each address
            for (address, _utxos) in &utxos_by_address {
                match self.provider.batch_fetch_utxo_balances(address, Some(1), None).await {
                    Ok(result) => {
                        // Parse the result and index by txid:vout
                        if let Some(utxos_array) = result.get("utxos").and_then(|v| v.as_array()) {
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
                    Err(e) => {
                        log::warn!("Failed to batch fetch UTXOs for address {}: {}", address, e);
                        // Fall back to individual queries for this address
                        for (outpoint, _) in _utxos {
                            match self.provider.protorunes_by_outpoint(
                                &outpoint.txid.to_string(),
                                outpoint.vout,
                                None, // block_tag
                                1,    // protocol_tag for alkanes
                            ).await {
                                Ok(response) => {
                                    // Convert to same format as batch result
                                    let mut balances_array = Vec::new();
                                    for (alkane_id, amount) in &response.balance_sheet.cached.balances {
                                        balances_array.push(serde_json::json!({
                                            "block": alkane_id.block,
                                            "tx": alkane_id.tx,
                                            "amount": amount
                                        }));
                                    }
                                    let key = format!("{}:{}", outpoint.txid, outpoint.vout);
                                    utxo_balances.insert(key, serde_json::json!({
                                        "txid": outpoint.txid.to_string(),
                                        "vout": outpoint.vout,
                                        "balances": balances_array
                                    }));
                                }
                                Err(e) => {
                                    log::warn!("Failed to query alkanes for UTXO {}:{}: {}", outpoint.txid, outpoint.vout, e);
                                }
                            }
                        }
                    }
                }
            }
            
            // Now process UTXOs using the pre-fetched balance data
            for (outpoint, utxo) in spendable_utxos {
                let key = format!("{}:{}", outpoint.txid, outpoint.vout);
                
                if let Some(utxo_data) = utxo_balances.get(&key) {
                    // Parse balance data from batch result
                    let balances = utxo_data.get("balances").and_then(|v| v.as_array()).map(|arr| {
                        arr.iter().filter_map(|b| {
                            let block = b.get("block").and_then(|v| v.as_u64())?;
                            let tx = b.get("tx").and_then(|v| v.as_u64())?;
                            let amount = b.get("amount").and_then(|v| v.as_u64())?;
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
                    
                    // Select this UTXO if it has alkanes we need OR if we still need Bitcoin
                    if has_needed_alkane || bitcoin_collected < bitcoin_needed {
                        bitcoin_collected += utxo.amount;
                        selected_outpoints.push(outpoint);
                        utxo_selected = true;
                        log::debug!("Selected UTXO {}:{} (has_alkanes: {}, btc: {})", outpoint.txid, outpoint.vout, has_needed_alkane, utxo.amount);
                    }
                    
                    // Track ALL alkanes found in selected UTXOs (for change calculation)
                    if utxo_selected {
                        for ((block, tx), amount) in &balances {
                            let alkane_key = AlkaneId {
                                block: *block,
                                tx: *tx,
                            };
                            *alkanes_found.entry(alkane_key).or_insert(0) += amount;
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
            // No alkanes needed, just select UTXOs for Bitcoin
            for (outpoint, utxo) in spendable_utxos {
                if bitcoin_collected < bitcoin_needed {
                    bitcoin_collected += utxo.amount;
                    selected_outpoints.push(outpoint);
                } else {
                    break;
                }
            }
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
        
        Ok(UtxoSelectionResult {
            outpoints: selected_outpoints,
            alkanes_found,
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
        log::info!("Generating automatic split protostone for {} alkane types", alkanes_needed.len());
        
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
        
        // Create the protostone
        // This protostone will:
        // - Split alkanes: send needed amounts to p1, send excess to change output
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
                        // Protostone targets: physical_outputs + 1 (OP_RETURN) + 1 (base offset) + protostone_index
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
                    log::info!("  Pointer: p{} (shadow output = {} + 1 + 1 + {} = {})", p, num_physical_outputs, p, calculated);
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

            // Convert refund: v{N} -> N, p{N} -> num_physical_outputs + 1 (OP_RETURN) + 1 (base offset) + N
            let refund = match &spec.refund {
                Some(OutputTarget::Output(v)) => {
                    log::info!("  Refund: v{} (physical output {})", v, v);
                    Some(*v)
                }
                Some(OutputTarget::Protostone(p)) => {
                    let calculated = num_physical_outputs + 2 + p;
                    log::info!("  Refund: p{} (shadow output = {} + 1 + 1 + {} = {})", p, num_physical_outputs, p, calculated);
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
        log::info!("Constructing runestone with {} protostones and {} outputs (before OP_RETURN)", protostones.len(), num_outputs);
        log::info!("  After OP_RETURN is added, protostone vouts will start at: {} + 1 = {}", num_outputs, num_outputs + 1);
        log::info!("  Formula: pN -> vout = {} + N (OP_RETURN gets added later)", num_outputs);
        
        let converted_protostones = self.convert_protostone_specs_with_output_count(protostones, num_outputs as u32)?;

        // Debug logging
        for (i, p) in converted_protostones.iter().enumerate() {
            log::info!("Protostone #{}: protocol_tag={}, message_len={} bytes", i, p.protocol_tag, p.message.len());
        }

        // Use the Protostones trait to properly encode the protocol field
        let protocol_values = converted_protostones.encipher()?;
        log::info!("Encoded protocol values: {} u128 values", protocol_values.len());

        let runestone = Runestone {
            protocol: Some(protocol_values),
            pointer: Some(0),  // Point to output 0 (the alkanes target output)
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
    
        if let Some(change_output) = outputs.iter_mut().find(|o| o.value.to_sat() == 0 && !o.script_pubkey.is_op_return()) {
            change_output.value = bitcoin::Amount::from_sat(change_value);
        } else if let Some(last_output) = outputs.iter_mut().last() {
             if !last_output.script_pubkey.is_op_return() {
                last_output.value = bitcoin::Amount::from_sat(last_output.value.to_sat() + change_value);
             }
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

    /// Build a split PSBT to protect inscribed UTXOs
    ///
    /// Returns (split_psbt, split_fee, clean_outpoints)
    /// The clean_outpoints are the UTXOs that can be used for funding after the split
    async fn build_split_psbt(
        &mut self,
        plans: &[SplitPlan],
        funding_utxos: &[(OutPoint, TxOut)],
        fee_rate: f32,
        params: &EnhancedExecuteParams,
    ) -> Result<(Psbt, u64, Vec<OutPoint>)> {
        use bitcoin::transaction::Version;

        // Get safe address for split outputs
        let safe_address_str = params.change_address.as_ref()
            .map(|s| s.as_str())
            .unwrap_or("p2tr:0");
        use crate::traits::AddressResolver;
        let resolved_addr = self.provider.resolve_all_identifiers(safe_address_str).await?;
        let safe_address = Address::from_str(&resolved_addr)?.require_network(self.provider.get_network())?;

        // Build inputs and outputs
        let mut inputs = Vec::new();
        let mut outputs = Vec::new();
        let mut input_txouts = Vec::new();
        let mut clean_outpoints = Vec::new();
        let mut total_input_value = 0u64;
        let mut total_output_value = 0u64;

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
            total_output_value += plan.safe_amount;

            // Clean output (funding sats go here)
            let clean_output = TxOut {
                value: bitcoin::Amount::from_sat(plan.clean_amount),
                script_pubkey: safe_address.script_pubkey(),
            };
            outputs.push(clean_output);
            total_output_value += plan.clean_amount;

            // Track the clean outpoint (will update txid after building tx)
            clean_outpoints.push(OutPoint {
                txid: bitcoin::Txid::from_byte_array([0u8; 32]), // Placeholder
                vout: (idx * 2 + 1) as u32, // Clean outputs are at odd indices
            });
        }

        // Estimate fee
        let estimated_vsize = 10 + (inputs.len() * 68) + (outputs.len() * 43);
        let estimated_fee = (fee_rate * estimated_vsize as f32).ceil() as u64;

        // Adjust the last clean output to account for fee
        if let Some(last_clean_output) = outputs.last_mut() {
            if last_clean_output.value.to_sat() > estimated_fee + DUST_LIMIT {
                last_clean_output.value = bitcoin::Amount::from_sat(
                    last_clean_output.value.to_sat() - estimated_fee
                );
            } else {
                return Err(AlkanesError::Wallet(
                    "Not enough funds in split to cover fee".to_string()
                ));
            }
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

        // Calculate actual txid and update clean outpoints
        let txid = psbt.unsigned_tx.compute_txid();
        for outpoint in &mut clean_outpoints {
            outpoint.txid = txid;
        }

        log::info!("Built split PSBT: {} inputs → {} outputs, fee: {} sats",
            plans.len(), psbt.unsigned_tx.output.len(), estimated_fee);

        Ok((psbt, estimated_fee, clean_outpoints))
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
    
        let fee_rate_sat_vb = fee_rate.unwrap_or(600.0);
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
            let utxo_selection = self.select_utxos(&additional_reqs, &params.from_addresses).await?;
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
        
        let (mut psbt, fee, estimated_vsize) = self.build_psbt_and_fee(selected_utxos, outputs, Some(runestone_script), params.fee_rate, Some(envelope), Some(commit_txout)).await?;

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

        if has_envelope && !has_cellpacks {
            return Err(AlkanesError::Other(anyhow!(
                "Incomplete deployment: Envelope provided but no cellpack to trigger deployment."
            ).to_string()));
        }

        if !has_envelope && has_cellpacks {
            return Ok(());
        }
        
        if !has_envelope && !has_cellpacks && !params.protostones.is_empty() {
             return Err(AlkanesError::Other(anyhow!(
                "No operation: Protostones provided without envelope or cellpack."
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
            .select_utxos(&[InputRequirement::Bitcoin { amount: required_reveal_amount }], &params.from_addresses)
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

        // Add OP_RETURN runestone if needed
        // Note: Runestone handling can be added here if protostones are needed

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

        assert_eq!(outputs.len(), 2);
        for output in outputs {
            assert_eq!(output.value, Amount::from_sat(546));
        }
    }

    #[tokio::test]
    async fn test_create_outputs_with_explicit_bitcoin() {
        let mut provider = MockProvider::new(Network::Regtest);
        let addr1 = WalletProvider::get_address(&provider).await.unwrap();
        let mut executor = EnhancedAlkanesExecutor::new(&mut provider);
        let to_addresses = vec![addr1.clone(), addr1];
        let input_requirements = vec![InputRequirement::Bitcoin { amount: 20000 }];

        let outputs = executor.create_outputs(&to_addresses, &None, &input_requirements, &[]).await.unwrap();

        assert_eq!(outputs.len(), 2);
        for output in outputs {
            assert_eq!(output.value, Amount::from_sat(10000));
        }
    }
}
