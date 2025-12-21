// BRC20-Prog executor for contract deployment and interaction
// This module handles the commit-reveal transaction pattern for BRC20-prog inscriptions

use crate::{AlkanesError, DeezelProvider, Result};
use crate::traits::{WalletProvider, UtxoInfo};
use bitcoin::{Transaction, ScriptBuf, OutPoint, TxOut, Address, XOnlyPublicKey, psbt::Psbt, Txid};
use bitcoin::blockdata::script::Builder as ScriptBuilder;
use bitcoin_hashes::Hash;
use core::str::FromStr;
#[cfg(not(feature = "std"))]
use alloc::{vec, vec::Vec, string::{String, ToString}, format};
#[cfg(feature = "std")]
use std::{vec, vec::Vec, string::{String, ToString}, format};

use super::envelope::Brc20ProgEnvelope;
use super::types::{Brc20ProgExecuteParams, Brc20ProgExecuteResult};

const MAX_FEE_SATS: u64 = 100_000;
const DUST_LIMIT: u64 = 546;

/// Rebar payment information
struct RebarPaymentInfo {
    payment_address: Address,
    payment_amount: u64,
}

/// BRC20-Prog executor for contract operations
pub struct Brc20ProgExecutor<'a> {
    pub provider: &'a mut dyn DeezelProvider,
}

impl<'a> Brc20ProgExecutor<'a> {
    /// Create a new BRC20-prog executor
    pub fn new(provider: &'a mut dyn DeezelProvider) -> Self {
        Self { provider }
    }

    /// Execute BRC20-prog with Presign+RBF hybrid strategy
    /// This strategy:
    /// 1. Pre-builds and signs all transactions (commit, reveal, activation) with RBF-enabled sequences
    /// 2. Broadcasts all transactions simultaneously to minimize timing gaps
    /// 3. Monitors mempool for frontrunning attempts
    /// 4. If frontrunning detected, RBF-bumps commit and rebuilds/rebroadcasts reveal+activation
    /// 5. Repeats monitoring and RBF-bumping up to 3 times to outpace attackers
    async fn execute_with_presign_rbf(&mut self, params: Brc20ProgExecuteParams) -> Result<Brc20ProgExecuteResult> {
        log::info!("🔐 Presign+RBF Hybrid Strategy: Building and signing all transactions upfront...");
        log::info!("Inscription content: {}", params.inscription_content);

        // Create the envelope
        let envelope = Brc20ProgEnvelope::new(params.inscription_content.as_bytes().to_vec());

        // Step 1: Build commit transaction (unsigned)
        log::info!("📝 Step 1/6: Building commit transaction...");
        let (commit_psbt, commit_fee, commit_internal_key) = self.build_commit_psbt_for_presign(&params, &envelope).await?;

        // Calculate commit txid from unsigned transaction
        let commit_tx = commit_psbt.unsigned_tx.clone();
        let commit_txid = commit_tx.txid();
        let commit_outpoint = OutPoint { txid: commit_txid, vout: 0 };
        let commit_output = commit_tx.output[0].clone();

        log::info!("   Commit txid (pre-calculated): {}", commit_txid);

        // Step 2: Build reveal transaction (unsigned, spending future commit output)
        log::info!("📝 Step 2/6: Building reveal transaction...");
        let (reveal_psbt, reveal_fee, reveal_inscription_outpoint) = self.build_reveal_psbt_for_presign(
            &params,
            &envelope,
            commit_outpoint,
            commit_output.clone(),
            commit_internal_key,
        ).await?;

        let reveal_tx = reveal_psbt.unsigned_tx.clone();
        let reveal_txid = reveal_tx.txid();
        let reveal_inscription_output = reveal_tx.output.iter()
            .find(|o| o.value.to_sat() == 546 && !o.script_pubkey.is_op_return())
            .ok_or_else(|| AlkanesError::Wallet("No inscription output found in reveal".to_string()))?
            .clone();

        log::info!("   Reveal txid (pre-calculated): {}", reveal_txid);

        // Step 3: Build activation transaction (if needed)
        let (activation_psbt_opt, activation_fee_opt) = if params.use_activation {
            log::info!("📝 Step 3/6: Building activation transaction...");
            let (act_psbt, act_fee) = self.build_activation_psbt_for_presign(
                &params,
                reveal_inscription_outpoint,
                reveal_inscription_output,
            ).await?;

            let act_txid = act_psbt.unsigned_tx.txid();
            log::info!("   Activation txid (pre-calculated): {}", act_txid);
            (Some(act_psbt), Some(act_fee))
        } else {
            log::info!("📝 Step 3/6: Skipping activation (2-tx pattern)");
            (None, None)
        };

        // Step 4: Sign all transactions
        log::info!("✍️  Step 4/6: Signing all transactions...");
        let signed_commit = self.sign_and_finalize_psbt(commit_psbt).await?;
        let signed_reveal = self.sign_and_finalize_reveal_psbt_simple(reveal_psbt, &envelope, commit_internal_key).await?;
        let signed_activation = if let Some(act_psbt) = activation_psbt_opt {
            Some(self.sign_and_finalize_psbt(act_psbt).await?)
        } else {
            None
        };

        log::info!("   ✅ All transactions signed");

        // Step 5: Broadcast all transactions at once
        log::info!("📡 Step 5/6: Broadcasting all transactions simultaneously...");

        let commit_hex = bitcoin::consensus::encode::serialize_hex(&signed_commit);
        let reveal_hex = bitcoin::consensus::encode::serialize_hex(&signed_reveal);

        let final_commit_txid = self.broadcast_with_options(&commit_hex, &params).await?;
        log::info!("   ✅ Commit broadcast: {}", final_commit_txid);

        let final_reveal_txid = self.broadcast_with_options(&reveal_hex, &params).await?;
        log::info!("   ✅ Reveal broadcast: {}", final_reveal_txid);

        let final_activation_txid = if let Some(ref act_tx) = signed_activation {
            let act_hex = bitcoin::consensus::encode::serialize_hex(act_tx);
            let txid = self.broadcast_with_options(&act_hex, &params).await?;
            log::info!("   ✅ Activation broadcast: {}", txid);
            Some(txid)
        } else {
            None
        };

        // Step 6: Monitor for frontrunning and RBF if needed
        log::info!("🔍 Step 6/6: Monitoring for frontrunning attacks...");
        self.monitor_and_bump_presigned_txs(
            &final_commit_txid,
            &final_reveal_txid,
            final_activation_txid.as_deref(),
            &envelope,
            &params,
        ).await?;

        log::info!("✅ Presign+RBF strategy completed successfully!");

        Ok(Brc20ProgExecuteResult {
            commit_txid: final_commit_txid,
            reveal_txid: final_reveal_txid,
            activation_txid: final_activation_txid,
            commit_fee,
            reveal_fee,
            activation_fee: activation_fee_opt,
            inputs_used: vec![],
            outputs_created: vec![],
            traces: None,
        })
    }

    /// Execute a BRC20-prog operation (deploy or call) using commit-reveal pattern
    pub async fn execute(&mut self, params: Brc20ProgExecuteParams) -> Result<Brc20ProgExecuteResult> {
        log::info!("Starting BRC20-prog execution");
        log::info!("Inscription content: {}", params.inscription_content);

        // ANTI-FRONTRUNNING: Use presign strategy by default to prevent frontrunning
        // This builds and signs all transactions BEFORE broadcasting any of them,
        // eliminating the window where attackers can see the commit and race to build their own reveal.
        //
        // The presign approach is the ONLY reliable way to prevent frontrunning because:
        // - Commit and reveal are broadcast in immediate succession (no delay)
        // - Reveal is already signed when commit hits the mempool
        // - Attackers have no time to build and broadcast a competing reveal

        // Always use presign strategy (ignore the strategy parameter - it doesn't work)
        log::info!("🔐 Using presign strategy to prevent frontrunning");
        return self.execute_with_presign_rbf(params).await;


        // Create the envelope with the JSON payload
        let envelope = Brc20ProgEnvelope::new(params.inscription_content.as_bytes().to_vec());

        // Check if resuming from existing commit
        let (commit_txid, commit_fee, commit_outpoint, commit_output, internal_key) = if let Some(ref resume_txid) = params.resume_from_commit {
            log::info!("🔄 Resuming from transaction: {}", resume_txid);

            // Auto-detect if this is a commit or reveal transaction
            let actual_commit_txid = self.detect_and_get_commit_txid(resume_txid).await?;

            self.resume_from_commit(&actual_commit_txid, &params, &envelope).await?
        } else {
            // Build and execute commit transaction
            let result = self.build_and_broadcast_commit(&params, &envelope).await?;
            log::info!("✅ Commit transaction broadcast: {}", result.0);
            result
        };

        // STRATEGIES DISABLED: They don't work - frontrunners can still extract and use the inscription data
        // The only real protection is making frontrunning cost money via small commit outputs

        // // Apply CPFP strategy if enabled (but not when resuming - the CPFP tx may already be broadcast)
        // if params.resume_from_commit.is_none() {
        //     if let Some(super::types::AntiFrontrunningStrategy::Cpfp) = params.strategy {
        //         log::info!("🚀 Applying CPFP strategy: broadcasting high-fee child transaction...");
        //         self.apply_cpfp_strategy(&commit_txid.to_string(), &params).await?;
        //     }
        //
        //     // Apply RBF monitoring strategy if enabled
        //     if let Some(super::types::AntiFrontrunningStrategy::Rbf) = params.strategy {
        //         log::info!("🔍 RBF: Starting mempool monitoring for frontrunning attempts...");
        //         self.monitor_and_bump_if_frontrun(&commit_txid.to_string(), &envelope, &params).await?;
        //     }
        // } else {
        //     log::info!("   Skipping anti-frontrunning strategies (resuming from existing commit)");
        // }

        // For slipstream/rebar, we must wait for the transaction to be mined before continuing
        if params.use_slipstream || params.use_rebar {
            log::info!("⏳ Waiting for commit transaction to be mined (required for slipstream/rebar)...");
            self.wait_for_confirmation(&commit_txid.to_string(), &params).await?;
        } else {
            // NO WAIT - broadcast reveal immediately to prevent frontrunning
            // We already have commit_output and commit_outpoint in memory, no need to wait for esplora
            log::info!("⚡ Broadcasting reveal immediately (anti-frontrunning)");

            // Mine a block if on regtest
            if params.mine_enabled {
                self.mine_blocks_if_regtest(&params).await?;
                self.provider.sync().await?;
            }
        }

        // Build and execute reveal transaction
        let (reveal_txid, reveal_fee, reveal_inscription_outpoint, reveal_inscription_output) = self
            .build_and_broadcast_reveal(
                &params,
                &envelope,
                commit_outpoint,
                commit_output,
                internal_key,
            )
            .await?;

        log::info!("✅ Reveal transaction broadcast: {reveal_txid}");

        // For slipstream/rebar, we must wait for the transaction to be mined before continuing
        if params.use_slipstream || params.use_rebar {
            log::info!("⏳ Waiting for reveal transaction to be mined (required for slipstream/rebar)...");
            self.wait_for_confirmation(&reveal_txid.to_string(), &params).await?;
        } else {
            // NO WAIT - broadcast activation immediately to prevent frontrunning
            // We already have reveal_inscription_outpoint and reveal_inscription_output in memory
            log::info!("⚡ Broadcasting activation immediately (anti-frontrunning)");

            if params.mine_enabled {
                self.mine_blocks_if_regtest(&params).await?;
                self.provider.sync().await?;
            }
        }

        // For deploy operations, optionally execute activation transaction
        let (activation_txid, activation_fee) = if params.use_activation {
            log::info!("Executing activation transaction (3-tx pattern)");

            let (act_txid, act_fee) = self
                .build_and_broadcast_activation(&params, reveal_inscription_outpoint, reveal_inscription_output)
                .await?;

            log::info!("✅ Activation transaction broadcast: {act_txid}");

            // For slipstream/rebar, we must wait for the transaction to be mined
            if params.use_slipstream || params.use_rebar {
                log::info!("⏳ Waiting for activation transaction to be mined (required for slipstream/rebar)...");
                self.wait_for_confirmation(&act_txid.to_string(), &params).await?;
            } else if params.mine_enabled {
                self.mine_blocks_if_regtest(&params).await?;
                self.provider.sync().await?;
            }

            (Some(act_txid.to_string()), Some(act_fee))
        } else {
            log::info!("Skipping activation transaction (using 2-tx pattern with OP_RETURN in reveal)");
            (None, None)
        };

        Ok(Brc20ProgExecuteResult {
            commit_txid: commit_txid.to_string(),
            reveal_txid: reveal_txid.to_string(),
            activation_txid,
            commit_fee,
            reveal_fee,
            activation_fee,
            inputs_used: vec![],
            outputs_created: vec![],
            traces: None,
        })
    }

    /// Smart resume: Auto-detect if txid is commit or reveal, extract commit txid if needed
    async fn detect_and_get_commit_txid(
        &mut self,
        txid: &str,
    ) -> Result<String> {
        log::info!("🔍 Auto-detecting transaction type for: {}", txid);

        // Fetch the transaction
        let tx_hex = self.provider.get_tx_hex(txid).await?;
        let tx_bytes = hex::decode(&tx_hex)?;
        let tx: Transaction = bitcoin::consensus::deserialize(&tx_bytes)?;

        // Check if this is a reveal transaction (has large witness on input 0)
        if !tx.input.is_empty() && !tx.input[0].witness.is_empty() {
            let witness_size: usize = tx.input[0].witness.iter().map(|w| w.len()).sum();

            if witness_size > 1000 {
                // This looks like a reveal transaction (large inscription witness)
                let commit_txid = tx.input[0].previous_output.txid;
                log::info!("   ✓ Detected REVEAL transaction");
                log::info!("   → Extracting commit txid: {}", commit_txid);
                return Ok(commit_txid.to_string());
            }
        }

        // Otherwise, assume it's the commit transaction
        log::info!("   ✓ Detected COMMIT transaction");
        Ok(txid.to_string())
    }

    /// Resume execution from an existing commit transaction
    async fn resume_from_commit(
        &mut self,
        commit_txid: &str,
        params: &Brc20ProgExecuteParams,
        envelope: &Brc20ProgEnvelope,
    ) -> Result<(Txid, u64, OutPoint, TxOut, XOnlyPublicKey)> {
        log::info!("   Fetching commit transaction...");

        // Get the commit transaction
        let commit_tx_hex = self.provider.get_tx_hex(commit_txid).await?;
        let commit_tx_bytes = hex::decode(&commit_tx_hex)?;
        let commit_tx: Transaction = bitcoin::consensus::deserialize(&commit_tx_bytes)?;

        // Get the commit output (should be at index 0)
        let commit_output = commit_tx.output.get(0)
            .ok_or_else(|| AlkanesError::Wallet("Commit transaction has no outputs".to_string()))?
            .clone();

        let commit_txid_parsed = Txid::from_str(commit_txid)?;
        let commit_outpoint = OutPoint {
            txid: commit_txid_parsed,
            vout: 0,
        };

        log::info!("   Commit output value: {} sats", commit_output.value.to_sat());
        log::info!("   Commit script pubkey: {}", commit_output.script_pubkey);

        // Get the internal key from the wallet (we need this to spend the taproot output)
        let (internal_key, _) = self.provider.get_internal_key().await?;

        // Verify that the commit address matches what we expect
        let expected_commit_address = self.create_commit_address_for_envelope(envelope, internal_key).await?;
        if commit_output.script_pubkey != expected_commit_address.script_pubkey() {
            log::warn!("   ⚠️  Warning: Commit output script doesn't match expected taproot script");
            log::warn!("   This might indicate the inscription content doesn't match the commit");
        }

        log::info!("   ✅ Successfully loaded commit transaction");

        // Estimate commit fee (we don't have the exact fee, but we can estimate)
        let commit_fee = 0u64; // We don't know the exact fee when resuming

        Ok((commit_txid_parsed, commit_fee, commit_outpoint, commit_output, internal_key))
    }

    /// Build and broadcast the commit transaction
    async fn build_and_broadcast_commit(
        &mut self,
        params: &Brc20ProgExecuteParams,
        envelope: &Brc20ProgEnvelope,
    ) -> Result<(Txid, u64, OutPoint, TxOut, XOnlyPublicKey)> {
        log::info!("Building commit transaction");

        let (internal_key, _) = self.provider.get_internal_key().await?;
        let commit_address = self.create_commit_address_for_envelope(envelope, internal_key).await?;
        log::info!("Commit address: {commit_address}");

        // Calculate EXACT commit output using reference implementation approach:
        // Build a dummy reveal transaction with REAL script and control block to get exact vsize
        let fee_rate = params.fee_rate.unwrap_or(600.0);

        // Build the reveal script and taproot structures NOW (before commit)
        let reveal_script = envelope.build_reveal_script();
        use bitcoin::taproot::{TaprootBuilder, LeafVersion, TapLeafHash};
        let taproot_builder = TaprootBuilder::new()
            .add_leaf(0, reveal_script.clone())
            .map_err(|e| AlkanesError::Other(format!("{e:?}")))?;
        let taproot_spend_info = taproot_builder
            .finalize(self.provider.secp(), internal_key)
            .map_err(|e| AlkanesError::Other(format!("{e:?}")))?;
        let control_block = taproot_spend_info
            .control_block(&(reveal_script.clone(), LeafVersion::TapScript))
            .ok_or_else(|| AlkanesError::Other("Failed to create control block".to_string()))?;

        // Create dummy reveal transaction to calculate exact size (like reference implementation)
        use bitcoin::transaction::Version;
        let dummy_outpoint = OutPoint::null();
        let reveal_output_value = if params.use_activation { 546 } else { 1 };

        let mut dummy_reveal_tx = Transaction {
            version: Version::TWO,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: vec![bitcoin::TxIn {
                previous_output: dummy_outpoint,
                script_sig: ScriptBuf::new(),
                sequence: bitcoin::Sequence::ENABLE_RBF_NO_LOCKTIME,
                witness: bitcoin::Witness::new(),
            }],
            output: vec![TxOut {
                value: bitcoin::Amount::from_sat(reveal_output_value),
                script_pubkey: ScriptBuf::new_op_return(&[]), // Dummy OP_RETURN
            }],
        };

        // Add dummy witness (64-byte signature + script + control block) - like reference
        dummy_reveal_tx.input[0].witness.push(vec![0u8; 64]); // Dummy 64-byte signature
        dummy_reveal_tx.input[0].witness.push(reveal_script.as_bytes());
        dummy_reveal_tx.input[0].witness.push(control_block.serialize());

        // Get EXACT vsize
        let reveal_vsize = dummy_reveal_tx.vsize();
        let reveal_fee = (fee_rate * reveal_vsize as f32).ceil() as u64;

        // Commit output = reveal output value + reveal fee (like reference: postage + reveal_fee)
        let commit_output_amount = reveal_output_value + reveal_fee;

        log::info!("💰 Calculated EXACT commit output: {} sats (reference implementation method)", commit_output_amount);
        log::info!("   Fee rate: {} sat/vB, Reveal vsize: {} vB, Reveal fee: {} sats",
                   fee_rate, reveal_vsize, reveal_fee);
        log::info!("   Reveal output: {} sats, No change (avoids dust)", reveal_output_value);

        // Select UTXOs for funding the commit
        let funding_utxos = self.select_utxos_for_amount(
            commit_output_amount,
            &params.from_addresses,
        ).await?;

        let commit_output = TxOut {
            value: bitcoin::Amount::from_sat(commit_output_amount),
            script_pubkey: commit_address.script_pubkey(),
        };

        // Calculate Rebar payment if needed (estimate tx size first)
        let estimated_commit_vsize = 10 + (funding_utxos.len() * 107) + (2 * 43); // base + inputs + 2 outputs
        let rebar_payment = self.calculate_rebar_payment(estimated_commit_vsize, params).await?;

        let (commit_psbt, commit_fee) = self
            .build_commit_psbt(
                funding_utxos,
                commit_output.clone(),
                params.fee_rate,
                &params.change_address,
                rebar_payment,
            )
            .await?;

        // Sign and broadcast commit transaction
        let commit_tx = self.sign_and_finalize_psbt(commit_psbt).await?;
        let commit_tx_hex = bitcoin::consensus::encode::serialize_hex(&commit_tx);
        let commit_txid_string = self.broadcast_with_options(&commit_tx_hex, params).await?;

        let commit_outpoint = OutPoint {
            txid: commit_tx.compute_txid(),
            vout: 0,
        };

        Ok((
            commit_tx.compute_txid(),
            commit_fee,
            commit_outpoint,
            commit_output,
            internal_key,
        ))
    }

    /// Build and broadcast the reveal transaction
    async fn build_and_broadcast_reveal(
        &mut self,
        params: &Brc20ProgExecuteParams,
        envelope: &Brc20ProgEnvelope,
        commit_outpoint: OutPoint,
        commit_output: TxOut,
        commit_internal_key: XOnlyPublicKey,
    ) -> Result<(Txid, u64, OutPoint, TxOut)> {
        log::info!("Building reveal transaction");

        // TEMPORARY: No additional UTXOs needed since commit output covers reveal fee
        // TODO: Re-enable this once we fix signing complexity
        let additional_utxos: Vec<OutPoint> = vec![];

        log::info!("   Selected {} additional UTXOs for reveal funding", additional_utxos.len());

        // Get change address
        let change_address_str = if let Some(ref addr) = params.change_address {
            addr.clone()
        } else {
            WalletProvider::get_address(self.provider).await?
        };
        let change_address = Address::from_str(&change_address_str)?
            .require_network(self.provider.get_network())?;

        // Create outputs based on activation mode
        let outputs = if params.use_activation {
            // 3-tx pattern: Create 546-sat inscription UTXO for later activation, NO change
            log::info!("Creating 546-sat inscription output (will be spent to OP_RETURN in activation tx, no change)");

            let inscription_output = TxOut {
                value: bitcoin::Amount::from_sat(546),
                script_pubkey: change_address.script_pubkey(),
            };

            // NO change output - commit output is sized exactly to cover inscription + fee
            vec![inscription_output]
        } else {
            // 2-tx pattern: Output directly to OP_RETURN with 1 sat, NO change output
            log::info!("Creating OP_RETURN output directly in reveal tx (2-tx pattern, no change)");

            let op_return_script = self.create_brc20prog_op_return();
            let op_return_output = TxOut {
                value: bitcoin::Amount::from_sat(1),
                script_pubkey: op_return_script,
            };

            // NO change output - commit output is sized exactly to cover fee
            vec![op_return_output]
        };

        // Fetch the TxOut data for the additional UTXOs
        let mut all_inputs_with_txouts = vec![(commit_outpoint, commit_output.clone())];

        for utxo_outpoint in additional_utxos {
            let tx_hex = self.provider.get_tx_hex(&utxo_outpoint.txid.to_string()).await?;
            let tx_bytes = hex::decode(&tx_hex)?;
            let tx: Transaction = bitcoin::consensus::deserialize(&tx_bytes)?;
            let txout = tx.output.get(utxo_outpoint.vout as usize)
                .ok_or_else(|| AlkanesError::Wallet(format!("UTXO vout {} not found in transaction", utxo_outpoint.vout)))?
                .clone();
            all_inputs_with_txouts.push((utxo_outpoint, txout));
        }

        log::info!("   Total reveal transaction inputs: {}", all_inputs_with_txouts.len());

        // Calculate Rebar payment if needed (estimate tx size first)
        let estimated_reveal_vsize = 10 + 200 + (all_inputs_with_txouts.len() * 107) + (2 * 43); // base + script-path input + standard inputs + 2 outputs
        let rebar_payment = self.calculate_rebar_payment(estimated_reveal_vsize, params).await?;

        // Build reveal PSBT with script-path spending (NO MORE CLTV STRATEGY - it doesn't work)
        let (mut reveal_psbt, reveal_fee) = self
            .build_reveal_psbt(
                all_inputs_with_txouts,
                outputs,
                params.fee_rate,
                Some(envelope),
                commit_internal_key,
                rebar_payment,
                None, // No locktime - CLTV strategy removed
            )
            .await?;

        // Sign the reveal transaction with script-path signature
        let reveal_tx = self.sign_and_finalize_reveal_psbt(
            &mut reveal_psbt,
            envelope,
            commit_internal_key,
        ).await?;

        let reveal_tx_hex = bitcoin::consensus::encode::serialize_hex(&reveal_tx);
        let reveal_txid_string = self.broadcast_with_options(&reveal_tx_hex, params).await?;

        let reveal_txid = reveal_tx.compute_txid();

        // Inscription outpoint depends on the pattern
        let inscription_outpoint = OutPoint {
            txid: reveal_txid,
            vout: 0, // First output in both patterns
        };

        Ok((reveal_txid, reveal_fee, inscription_outpoint, reveal_tx.output[0].clone()))
    }

    /// Build and broadcast the activation transaction (for deploy operations)
    /// This sends the 1-sat inscription to OP_RETURN to activate the deployment
    async fn build_and_broadcast_activation(
        &mut self,
        params: &Brc20ProgExecuteParams,
        inscription_outpoint: OutPoint,
        inscription_utxo: TxOut,
    ) -> Result<(Txid, u64)> {
        log::info!("Building activation transaction");
        log::info!("Inscription outpoint: {}:{}", inscription_outpoint.txid, inscription_outpoint.vout);

        // Get the 1-sat inscription UTXO
        /*let inscription_utxo = self
            .provider
            .get_utxo(&inscription_outpoint)
            .await?
            .ok_or_else(|| AlkanesError::Wallet(format!("Inscription UTXO not found: {inscription_outpoint}")))?;*/

        if inscription_utxo.value.to_sat() != 546 {
            return Err(AlkanesError::Wallet(format!(
                "Expected 546-sat inscription, but found {} sats at {}",
                inscription_utxo.value.to_sat(),
                inscription_outpoint
            )));
        }

        // Create OP_RETURN output with 1 sat to capture the inscription
        let op_return_script = self.create_brc20prog_op_return();
        let op_return_output = TxOut {
            value: bitcoin::Amount::from_sat(1),
            script_pubkey: op_return_script,
        };

        // Get change address
        let change_address_str = if let Some(ref addr) = params.change_address {
            addr.clone()
        } else {
            WalletProvider::get_address(self.provider).await?
        };
        let change_address = Address::from_str(&change_address_str)?
            .require_network(self.provider.get_network())?;

        // We need additional UTXOs to pay for the transaction fee
        let estimated_activation_fee = 10_000u64; // Generous estimate
        // Exclude the reveal transaction outputs since they might not be indexed yet
        let exclude_txid = inscription_outpoint.txid;
        let funding_utxos = self.select_utxos_for_amount_excluding(
            estimated_activation_fee,
            &params.from_addresses,
            &[exclude_txid],
        ).await?;

        // Build the activation transaction
        let mut total_input_value = inscription_utxo.value.to_sat(); // 1 sat from inscription
        let mut all_inputs = vec![inscription_outpoint];
        let mut input_txouts = vec![inscription_utxo];

        for outpoint in &funding_utxos {
            let utxo = self
                .provider
                .get_utxo(outpoint)
                .await?
                .ok_or_else(|| AlkanesError::Wallet(format!("UTXO not found: {outpoint}")))?;
            total_input_value += utxo.value.to_sat();
            all_inputs.push(*outpoint);
            input_txouts.push(utxo);
        }

        // Calculate Rebar payment if needed (estimate tx size first)
        let estimated_activation_inputs = all_inputs.len();
        let estimated_activation_vsize = 10 + (estimated_activation_inputs * 107) + (2 * 43);
        let rebar_payment = self.calculate_rebar_payment(estimated_activation_vsize, params).await?;

        // Build a temporary transaction to estimate size
        let mut temp_outputs = vec![op_return_output.clone()];
        if let Some(ref rebar) = rebar_payment {
            temp_outputs.push(TxOut {
                value: bitcoin::Amount::from_sat(rebar.payment_amount),
                script_pubkey: rebar.payment_address.script_pubkey(),
            });
        }
        temp_outputs.push(TxOut {
            value: bitcoin::Amount::from_sat(0),
            script_pubkey: change_address.script_pubkey(),
        });

        let mut temp_tx = Transaction {
            version: bitcoin::transaction::Version::TWO,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: all_inputs
                .iter()
                .map(|outpoint| bitcoin::TxIn {
                    previous_output: *outpoint,
                    script_sig: ScriptBuf::new(),
                    sequence: bitcoin::Sequence::ENABLE_RBF_NO_LOCKTIME,
                    witness: bitcoin::Witness::new(),
                })
                .collect(),
            output: temp_outputs,
        };

        // Add dummy witness for size estimation
        for input in &mut temp_tx.input {
            input.witness.push([0u8; 65]);
        }

        // For Rebar, fee to miners is 0 (payment goes to Rebar output)
        let fee = if rebar_payment.is_some() {
            0
        } else {
            let fee_rate_sat_vb = params.fee_rate.unwrap_or(600.0);
            (fee_rate_sat_vb * temp_tx.vsize() as f32).ceil() as u64
        };

        let rebar_payment_amount = rebar_payment.as_ref().map(|r| r.payment_amount).unwrap_or(0);

        // Calculate change
        let change_value = total_input_value
            .saturating_sub(1)  // OP_RETURN output
            .saturating_sub(rebar_payment_amount)
            .saturating_sub(fee);

        if change_value < 546 {
            return Err(AlkanesError::Wallet(
                "Not enough funds for activation transaction".to_string(),
            ));
        }

        let change_output = TxOut {
            value: bitcoin::Amount::from_sat(change_value),
            script_pubkey: change_address.script_pubkey(),
        };

        // Build final outputs list including Rebar payment if needed
        let mut final_outputs = vec![op_return_output];
        if let Some(rebar) = rebar_payment {
            final_outputs.push(TxOut {
                value: bitcoin::Amount::from_sat(rebar.payment_amount),
                script_pubkey: rebar.payment_address.script_pubkey(),
            });
        }
        final_outputs.push(change_output);

        // Build the final PSBT
        let unsigned_tx = Transaction {
            version: bitcoin::transaction::Version::TWO,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: all_inputs
                .iter()
                .map(|outpoint| bitcoin::TxIn {
                    previous_output: *outpoint,
                    script_sig: ScriptBuf::new(),
                    sequence: bitcoin::Sequence::ENABLE_RBF_NO_LOCKTIME,
                    witness: bitcoin::Witness::new(),
                })
                .collect(),
            output: final_outputs,
        };

        let mut psbt = Psbt::from_unsigned_tx(unsigned_tx)?;

        // Add witness UTXOs and tap info
        for (i, utxo) in input_txouts.iter().enumerate() {
            psbt.inputs[i].witness_utxo = Some(utxo.clone());
            if utxo.script_pubkey.is_p2tr() {
                let (internal_key, (fingerprint, path)) = self.provider.get_internal_key().await?;
                psbt.inputs[i].tap_internal_key = Some(internal_key);
                psbt.inputs[i]
                    .tap_key_origins
                    .insert(internal_key, (vec![], (fingerprint, path)));
            }
        }

        // Sign and finalize
        let activation_tx = self.sign_and_finalize_psbt(psbt).await?;

        // Broadcast
        let activation_tx_hex = bitcoin::consensus::encode::serialize_hex(&activation_tx);
        let activation_txid_string = self.broadcast_with_options(&activation_tx_hex, params).await?;

        Ok((activation_tx.compute_txid(), fee))
    }

    /// Create a taproot address for the commit transaction
    async fn create_commit_address_for_envelope(
        &self,
        envelope: &Brc20ProgEnvelope,
        internal_key: XOnlyPublicKey,
    ) -> Result<Address> {
        use bitcoin::taproot::TaprootBuilder;
        let network = self.provider.get_network();

        let reveal_script = envelope.build_reveal_script();

        let taproot_builder = TaprootBuilder::new()
            .add_leaf(0, reveal_script.clone())
            .map_err(|e| AlkanesError::Other(format!("{e:?}")))?;

        let taproot_spend_info = taproot_builder
            .finalize(self.provider.secp(), internal_key)
            .map_err(|e| AlkanesError::Other(format!("{e:?}")))?;

        let commit_address = Address::p2tr_tweaked(taproot_spend_info.output_key(), network);

        Ok(commit_address)
    }

    /// Create OP_RETURN script for BRC20PROG
    fn create_brc20prog_op_return(&self) -> ScriptBuf {
        ScriptBuilder::new()
            .push_opcode(bitcoin::blockdata::opcodes::all::OP_RETURN)
            .push_slice(b"BRC20PROG")
            .into_script()
    }

    /// Build commit PSBT for presign strategy (uses final sequences for deterministic txid)
    async fn build_commit_psbt_for_presign(
        &mut self,
        params: &Brc20ProgExecuteParams,
        envelope: &Brc20ProgEnvelope,
    ) -> Result<(Psbt, u64, XOnlyPublicKey)> {
        let (internal_key, _) = self.provider.get_internal_key().await?;
        let commit_address = self.create_commit_address_for_envelope(envelope, internal_key).await?;

        let required_reveal_amount = 10_000u64;
        let funding_utxos = self.select_utxos_for_amount(
            required_reveal_amount,
            &params.from_addresses,
        ).await?;

        let commit_output = TxOut {
            value: bitcoin::Amount::from_sat(required_reveal_amount),
            script_pubkey: commit_address.script_pubkey(),
        };

        // Build commit with FINAL sequences (no RBF) for deterministic txid
        let (commit_psbt, commit_fee) = self.build_commit_psbt_final_seq(
            funding_utxos,
            commit_output,
            params.fee_rate,
            &params.change_address,
        ).await?;

        Ok((commit_psbt, commit_fee, internal_key))
    }

    /// Build commit PSBT with final sequences (for presign)
    async fn build_commit_psbt_final_seq(
        &mut self,
        funding_utxos: Vec<OutPoint>,
        commit_output: TxOut,
        fee_rate: Option<f32>,
        change_address: &Option<String>,
    ) -> Result<(Psbt, u64)> {
        use bitcoin::psbt::Input as PsbtInput;

        let mut total_input_value = 0u64;
        let mut input_txouts = Vec::new();
        for outpoint in &funding_utxos {
            let utxo = self.provider.get_utxo(outpoint).await?
                .ok_or_else(|| AlkanesError::Wallet(format!("UTXO not found: {}", outpoint)))?;
            total_input_value += utxo.value.to_sat();
            input_txouts.push(TxOut {
                value: utxo.value,
                script_pubkey: utxo.script_pubkey.clone(),
            });
        }

        let change_address_str = if let Some(ref addr) = change_address {
            addr.clone()
        } else {
            WalletProvider::get_address(self.provider).await?
        };
        let change_addr = Address::from_str(&change_address_str)?
            .require_network(self.provider.get_network())?;

        // Estimate fee
        let temp_outputs = vec![commit_output.clone(), TxOut {
            value: bitcoin::Amount::from_sat(0),
            script_pubkey: change_addr.script_pubkey(),
        }];

        let temp_tx = Transaction {
            version: bitcoin::transaction::Version::TWO,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: funding_utxos.iter().map(|outpoint| bitcoin::TxIn {
                previous_output: *outpoint,
                script_sig: ScriptBuf::new(),
                sequence: bitcoin::Sequence::ENABLE_RBF_NO_LOCKTIME, // RBF-enabled for presign+RBF hybrid
                witness: bitcoin::Witness::from_slice(&[vec![0u8; 65]]),
            }).collect(),
            output: temp_outputs,
        };

        let fee_rate_sat_vb = fee_rate.unwrap_or(600.0);
        let fee = (fee_rate_sat_vb * temp_tx.vsize() as f32).ceil() as u64;
        let change_value = total_input_value.saturating_sub(commit_output.value.to_sat()).saturating_sub(fee);

        if change_value < 546 {
            return Err(AlkanesError::Wallet("Not enough funds for commit and change".to_string()));
        }

        let final_outputs = vec![
            commit_output,
            TxOut {
                value: bitcoin::Amount::from_sat(change_value),
                script_pubkey: change_addr.script_pubkey(),
            },
        ];

        let unsigned_tx = Transaction {
            version: bitcoin::transaction::Version::TWO,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: funding_utxos.iter().map(|outpoint| bitcoin::TxIn {
                previous_output: *outpoint,
                script_sig: ScriptBuf::new(),
                sequence: bitcoin::Sequence::ENABLE_RBF_NO_LOCKTIME, // RBF-enabled
                witness: bitcoin::Witness::new(),
            }).collect(),
            output: final_outputs,
        };

        let mut psbt = Psbt::from_unsigned_tx(unsigned_tx)?;
        for (i, utxo) in input_txouts.iter().enumerate() {
            psbt.inputs[i] = PsbtInput {
                witness_utxo: Some(utxo.clone()),
                ..Default::default()
            };
        }

        Ok((psbt, fee))
    }

    /// Build reveal PSBT for presign strategy
    async fn build_reveal_psbt_for_presign(
        &mut self,
        params: &Brc20ProgExecuteParams,
        envelope: &Brc20ProgEnvelope,
        commit_outpoint: OutPoint,
        commit_output: TxOut,
        commit_internal_key: XOnlyPublicKey,
    ) -> Result<(Psbt, u64, OutPoint)> {
        let change_address_str = if let Some(ref addr) = params.change_address {
            addr.clone()
        } else {
            WalletProvider::get_address(self.provider).await?
        };
        let change_address = Address::from_str(&change_address_str)?
            .require_network(self.provider.get_network())?;

        let op_return_output = TxOut {
            value: bitcoin::Amount::from_sat(0),
            script_pubkey: self.create_brc20prog_op_return(),
        };

        let change_output = TxOut {
            value: bitcoin::Amount::from_sat(1), // Placeholder
            script_pubkey: change_address.script_pubkey(),
        };

        let outputs = if params.use_activation {
            // 3-tx pattern: inscription output + change
            vec![
                TxOut {
                    value: bitcoin::Amount::from_sat(546),
                    script_pubkey: change_address.script_pubkey(),
                },
                change_output,
            ]
        } else {
            vec![op_return_output, change_output]
        };

        let (reveal_psbt, reveal_fee) = self.build_reveal_psbt(
            vec![(commit_outpoint, commit_output)],
            outputs,
            params.fee_rate,
            Some(envelope),
            commit_internal_key,
            None, // No rebar for presign
            None, // No locktime for presign
        ).await?;

        let reveal_outpoint = OutPoint {
            txid: reveal_psbt.unsigned_tx.txid(),
            vout: 0,
        };

        Ok((reveal_psbt, reveal_fee, reveal_outpoint))
    }

    /// Build activation PSBT for presign strategy
    async fn build_activation_psbt_for_presign(
        &mut self,
        params: &Brc20ProgExecuteParams,
        reveal_inscription_outpoint: OutPoint,
        reveal_inscription_output: TxOut,
    ) -> Result<(Psbt, u64)> {
        use bitcoin::psbt::Input as PsbtInput;

        let change_address_str = if let Some(ref addr) = params.change_address {
            addr.clone()
        } else {
            WalletProvider::get_address(self.provider).await?
        };
        let change_address = Address::from_str(&change_address_str)?
            .require_network(self.provider.get_network())?;

        let op_return_output = TxOut {
            value: bitcoin::Amount::from_sat(0),
            script_pubkey: self.create_brc20prog_op_return(),
        };

        // Estimate fee
        let temp_tx = Transaction {
            version: bitcoin::transaction::Version::TWO,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: vec![bitcoin::TxIn {
                previous_output: reveal_inscription_outpoint,
                script_sig: ScriptBuf::new(),
                sequence: bitcoin::Sequence::ENABLE_RBF_NO_LOCKTIME, // RBF-enabled
                witness: bitcoin::Witness::from_slice(&[vec![0u8; 65]]),
            }],
            output: vec![
                op_return_output.clone(),
                TxOut {
                    value: bitcoin::Amount::from_sat(0),
                    script_pubkey: change_address.script_pubkey(),
                },
            ],
        };

        let fee_rate_sat_vb = params.fee_rate.unwrap_or(600.0);
        let fee = (fee_rate_sat_vb * temp_tx.vsize() as f32).ceil() as u64;
        let change_value = reveal_inscription_output.value.to_sat().saturating_sub(1).saturating_sub(fee);

        if change_value < 546 {
            return Err(AlkanesError::Wallet("Not enough funds for activation".to_string()));
        }

        let final_outputs = vec![
            op_return_output,
            TxOut {
                value: bitcoin::Amount::from_sat(change_value),
                script_pubkey: change_address.script_pubkey(),
            },
        ];

        let unsigned_tx = Transaction {
            version: bitcoin::transaction::Version::TWO,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: vec![bitcoin::TxIn {
                previous_output: reveal_inscription_outpoint,
                script_sig: ScriptBuf::new(),
                sequence: bitcoin::Sequence::ENABLE_RBF_NO_LOCKTIME, // RBF-enabled
                witness: bitcoin::Witness::new(),
            }],
            output: final_outputs,
        };

        let mut psbt = Psbt::from_unsigned_tx(unsigned_tx)?;
        psbt.inputs[0] = PsbtInput {
            witness_utxo: Some(reveal_inscription_output),
            ..Default::default()
        };

        Ok((psbt, fee))
    }

    /// Simplified reveal PSBT signing (for presign)
    async fn sign_and_finalize_reveal_psbt_simple(
        &mut self,
        mut psbt: Psbt,
        envelope: &Brc20ProgEnvelope,
        commit_internal_key: XOnlyPublicKey,
    ) -> Result<Transaction> {
        self.sign_and_finalize_reveal_psbt(&mut psbt, envelope, commit_internal_key).await
    }

    /// Monitor mempool and bump fee if frontrunning detected (RBF strategy)
    async fn monitor_and_bump_if_frontrun(
        &mut self,
        commit_txid: &str,
        envelope: &Brc20ProgEnvelope,
        params: &Brc20ProgExecuteParams,
    ) -> Result<()> {
        use crate::traits::EsploraProvider;

        // Get the commit transaction to extract its tapscript
        let commit_tx_hex = self.provider.get_tx_hex(commit_txid).await?;
        let commit_tx_bytes = hex::decode(&commit_tx_hex)?;
        let commit_tx: Transaction = bitcoin::consensus::deserialize(&commit_tx_bytes)?;

        // Extract the taproot output script for comparison
        let commit_script = &commit_tx.output[0].script_pubkey;

        log::info!("   Monitoring for transactions with similar taproot scripts...");
        log::info!("   Will check mempool every 3 seconds for up to 30 seconds");

        let max_monitoring_attempts = 10; // 30 seconds total
        let max_rbf_bumps = 3; // Maximum number of fee bumps
        let mut rbf_bump_count = 0;
        let mut current_txid = commit_txid.to_string();
        let mut current_fee_rate = params.fee_rate.unwrap_or(600.0);

        for attempt in 1..=max_monitoring_attempts {
            tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

            // Check if our transaction is confirmed yet
            match self.provider.get_tx_status(&current_txid).await {
                Ok(status) => {
                    let confirmed = status.get("confirmed")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);

                    if confirmed {
                        log::info!("   ✅ Transaction confirmed - stopping RBF monitoring");
                        return Ok(());
                    }
                }
                Err(_) => {
                    // Transaction not found yet, continue monitoring
                }
            }

            // Get mempool transactions
            let mempool_txids = match self.provider.get_mempool_txids().await {
                Ok(txids) => txids,
                Err(e) => {
                    log::warn!("   ⚠️  Failed to fetch mempool: {}", e);
                    continue;
                }
            };

            let mempool_count = mempool_txids.as_array().map(|a| a.len()).unwrap_or(0);
            log::debug!("   Checking {} mempool transactions...", mempool_count);

            // Check each mempool transaction for similar scripts
            for mempool_txid_value in mempool_txids.as_array().unwrap_or(&vec![]) {
                let mempool_txid = mempool_txid_value.as_str().unwrap_or("");
                if mempool_txid.is_empty() || mempool_txid == current_txid {
                    continue;
                }

                // Get the transaction
                match self.provider.get_tx_hex(mempool_txid).await {
                    Ok(tx_hex) => {
                        if let Ok(tx_bytes) = hex::decode(&tx_hex) {
                            if let Ok(mempool_tx) = bitcoin::consensus::deserialize::<Transaction>(&tx_bytes) {
                                // Check if any output has a similar taproot script
                                for output in &mempool_tx.output {
                                    if output.script_pubkey == *commit_script && mempool_txid != current_txid {
                                        log::warn!("   ⚠️  FRONTRUNNING DETECTED!");
                                        log::warn!("   Competing transaction: {}", mempool_txid);
                                        log::warn!("   Same taproot script detected");

                                        if rbf_bump_count >= max_rbf_bumps {
                                            log::error!("   ❌ Maximum RBF bumps ({}) reached - cannot bump further", max_rbf_bumps);
                                            return Err(AlkanesError::Network(format!(
                                                "Frontrunning detected but max RBF bumps exceeded. Competing tx: {}",
                                                mempool_txid
                                            )));
                                        }

                                        // Bump the fee by 50%
                                        current_fee_rate *= 1.5;
                                        rbf_bump_count += 1;

                                        log::warn!("   🚀 Bumping fee to {} sat/vB (bump #{}/{})",
                                                   current_fee_rate, rbf_bump_count, max_rbf_bumps);

                                        // Rebuild and rebroadcast with higher fee
                                        match self.rbf_bump_commit(&current_txid, current_fee_rate, params, envelope).await {
                                            Ok(new_txid) => {
                                                log::info!("   ✅ RBF replacement broadcast: {}", new_txid);
                                                current_txid = new_txid;
                                            }
                                            Err(e) => {
                                                log::error!("   ❌ Failed to broadcast RBF replacement: {}", e);
                                                return Err(e);
                                            }
                                        }

                                        // Break to restart monitoring loop with new txid
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    Err(_) => continue,
                }
            }

            if attempt % 3 == 0 {
                log::info!("   Still monitoring... ({}/{})", attempt, max_monitoring_attempts);
            }
        }

        log::info!("   Monitoring period complete");
        Ok(())
    }

    /// Monitor mempool for frontrunning and RBF-bump presigned transactions if needed
    async fn monitor_and_bump_presigned_txs(
        &mut self,
        commit_txid: &str,
        reveal_txid: &str,
        activation_txid: Option<&str>,
        envelope: &Brc20ProgEnvelope,
        params: &Brc20ProgExecuteParams,
    ) -> Result<()> {
        use crate::traits::EsploraProvider;

        // If using slipstream/rebar, just wait for confirmations (no monitoring needed)
        if params.use_slipstream || params.use_rebar {
            log::info!("   Using private mempool service - waiting for confirmations...");
            self.wait_for_confirmation(commit_txid, params).await?;
            self.wait_for_confirmation(reveal_txid, params).await?;
            if let Some(act_txid) = activation_txid {
                self.wait_for_confirmation(act_txid, params).await?;
            }
            return Ok(());
        }

        // Get the commit transaction to extract its tapscript
        let commit_tx_hex = self.provider.get_tx_hex(commit_txid).await?;
        let commit_tx_bytes = hex::decode(&commit_tx_hex)?;
        let commit_tx: Transaction = bitcoin::consensus::deserialize(&commit_tx_bytes)?;
        let commit_script = &commit_tx.output[0].script_pubkey;

        log::info!("   Monitoring mempool for competing transactions...");
        log::info!("   Will check every 3 seconds for up to 30 seconds");

        let max_monitoring_attempts = 10; // 30 seconds total
        let max_rbf_bumps = 3;
        let mut rbf_bump_count = 0;
        let mut current_commit_txid = commit_txid.to_string();
        let mut current_fee_rate = params.fee_rate.unwrap_or(600.0);

        for attempt in 1..=max_monitoring_attempts {
            tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

            // Check if our commit is confirmed
            match self.provider.get_tx_status(&current_commit_txid).await {
                Ok(status) => {
                    let confirmed = status.get("confirmed")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);

                    if confirmed {
                        log::info!("   ✅ Commit transaction confirmed!");
                        return Ok(());
                    }

                    // Check mempool for frontrunning
                    if let Ok(mempool_json) = self.provider.get_mempool_txids().await {
                        if let Some(mempool_txids) = mempool_json.as_array() {
                            log::debug!("   Checking {} mempool transactions...", mempool_txids.len());

                            for mempool_txid_val in mempool_txids {
                                if let Some(mempool_txid) = mempool_txid_val.as_str() {
                                    // Skip our own transaction
                                    if mempool_txid == current_commit_txid {
                                        continue;
                                    }

                                    // Check if this transaction has the same taproot script
                                    if let Ok(competitor_hex) = self.provider.get_tx_hex(mempool_txid).await {
                                        if let Ok(competitor_bytes) = hex::decode(&competitor_hex) {
                                            if let Ok(competitor_tx) = bitcoin::consensus::deserialize::<Transaction>(&competitor_bytes) {
                                                // Check if any output matches our commit script (frontrunning!)
                                                if competitor_tx.output.iter().any(|out| &out.script_pubkey == commit_script) {
                                                    log::warn!("   ⚠️  FRONTRUNNING DETECTED!");
                                                    log::warn!("   Competing transaction: {}", mempool_txid);

                                                    if rbf_bump_count >= max_rbf_bumps {
                                                        log::warn!("   Maximum RBF bumps ({}) reached", max_rbf_bumps);
                                                        return Err(AlkanesError::Wallet(
                                                            "Frontrunning detected and max RBF attempts exhausted".to_string()
                                                        ));
                                                    }

                                                    rbf_bump_count += 1;
                                                    current_fee_rate *= 1.5; // Increase fee by 50%
                                                    log::info!("   🔄 RBF Bump #{}: Increasing fee to {:.1} sat/vB",
                                                        rbf_bump_count, current_fee_rate);

                                                    // RBF the commit transaction
                                                    match self.rbf_bump_commit(&current_commit_txid, current_fee_rate, params, envelope).await {
                                                        Ok(new_commit_txid) => {
                                                            log::info!("   ✅ New commit broadcast: {}", new_commit_txid);
                                                            current_commit_txid = new_commit_txid.clone();

                                                            // Get the new commit transaction details
                                                            let new_commit_tx_hex = self.provider.get_tx_hex(&new_commit_txid).await?;
                                                            let new_commit_tx_bytes = hex::decode(&new_commit_tx_hex)?;
                                                            let new_commit_tx: Transaction = bitcoin::consensus::deserialize(&new_commit_tx_bytes)?;

                                                            let new_commit_outpoint = OutPoint {
                                                                txid: new_commit_tx.txid(),
                                                                vout: 0,
                                                            };
                                                            let new_commit_output = new_commit_tx.output[0].clone();

                                                            // Extract internal key from the commit transaction's taproot output
                                                            let new_commit_internal_key = self.extract_internal_key_from_commit(&new_commit_tx, envelope).await?;

                                                            // Rebuild and broadcast reveal transaction
                                                            log::info!("   🔄 Rebuilding reveal transaction...");
                                                            let (new_reveal_psbt, _reveal_fee, new_reveal_inscription_outpoint) =
                                                                self.build_reveal_psbt_for_presign(
                                                                    params,
                                                                    envelope,
                                                                    new_commit_outpoint,
                                                                    new_commit_output,
                                                                    new_commit_internal_key,
                                                                ).await?;

                                                            let new_signed_reveal = self.sign_and_finalize_reveal_psbt_simple(
                                                                new_reveal_psbt,
                                                                envelope,
                                                                new_commit_internal_key
                                                            ).await?;

                                                            let new_reveal_hex = bitcoin::consensus::encode::serialize_hex(&new_signed_reveal);
                                                            let new_reveal_txid = self.broadcast_with_options(&new_reveal_hex, params).await?;
                                                            log::info!("   ✅ New reveal broadcast: {}", new_reveal_txid);

                                                            // Rebuild and broadcast activation if needed
                                                            if params.use_activation {
                                                                log::info!("   🔄 Rebuilding activation transaction...");
                                                                let new_reveal_inscription_output = new_signed_reveal.output[0].clone();

                                                                let (new_activation_psbt, _activation_fee) =
                                                                    self.build_activation_psbt_for_presign(
                                                                        params,
                                                                        new_reveal_inscription_outpoint,
                                                                        new_reveal_inscription_output,
                                                                    ).await?;

                                                                let new_signed_activation = self.sign_and_finalize_psbt(new_activation_psbt).await?;
                                                                let new_activation_hex = bitcoin::consensus::encode::serialize_hex(&new_signed_activation);
                                                                let new_activation_txid = self.broadcast_with_options(&new_activation_hex, params).await?;
                                                                log::info!("   ✅ New activation broadcast: {}", new_activation_txid);
                                                            }

                                                            // Break to restart monitoring with new transactions
                                                            break;
                                                        }
                                                        Err(e) => {
                                                            log::error!("   ❌ Failed to RBF commit: {}", e);
                                                            return Err(e);
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Err(_) => continue,
            }

            if attempt % 3 == 0 {
                log::info!("   Still monitoring... ({}/{})", attempt, max_monitoring_attempts);
            }
        }

        log::info!("   ✅ Monitoring complete - no frontrunning detected");
        Ok(())
    }

    /// Extract internal key from commit transaction's taproot output
    async fn extract_internal_key_from_commit(
        &self,
        commit_tx: &Transaction,
        envelope: &Brc20ProgEnvelope,
    ) -> Result<XOnlyPublicKey> {
        use crate::traits::WalletProvider;

        // The internal key should be the wallet's key
        // We need to reconstruct it from the taproot address
        let taproot_output = &commit_tx.output[0];

        // For now, we'll use the wallet's key directly
        // In a more robust implementation, we'd extract it from the taproot address
        let secp = bitcoin::secp256k1::Secp256k1::new();
        let keypair = WalletProvider::get_keypair(self.provider).await?;
        Ok(XOnlyPublicKey::from_keypair(&keypair).0)
    }

    /// Create RBF replacement transaction with higher fee
    async fn rbf_bump_commit(
        &mut self,
        old_txid: &str,
        new_fee_rate: f32,
        params: &Brc20ProgExecuteParams,
        envelope: &Brc20ProgEnvelope,
    ) -> Result<String> {
        // Get the old transaction
        let old_tx_hex = self.provider.get_tx_hex(old_txid).await?;
        let old_tx_bytes = hex::decode(&old_tx_hex)?;
        let old_tx: Transaction = bitcoin::consensus::deserialize(&old_tx_bytes)?;

        // Rebuild commit transaction with same inputs but higher fee
        let funding_utxos: Vec<OutPoint> = old_tx.input.iter().map(|i| i.previous_output).collect();
        let commit_output = old_tx.output[0].clone();

        let (new_psbt, _new_fee) = self.build_commit_psbt(
            funding_utxos,
            commit_output,
            Some(new_fee_rate),
            &params.change_address,
            None, // No rebar for RBF
        ).await?;

        // Sign and broadcast
        let signed_tx = self.sign_and_finalize_psbt(new_psbt).await?;
        let tx_hex = bitcoin::consensus::encode::serialize_hex(&signed_tx);

        let new_txid = self.broadcast_with_options(&tx_hex, params).await?;
        Ok(new_txid)
    }

    /// Apply CPFP (Child-Pays-For-Parent) strategy to accelerate commit transaction
    async fn apply_cpfp_strategy(&mut self, commit_txid: &str, params: &Brc20ProgExecuteParams) -> Result<()> {
        use bitcoin::psbt::Input as PsbtInput;
        use crate::traits::BitcoinRpcProvider;

        // Wait for the commit transaction to propagate to esplora
        log::info!("   Waiting for commit transaction to propagate...");

        // Retry fetching the transaction up to 3 times with delays
        let commit_tx = loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

            match self.provider.get_tx_hex(commit_txid).await {
                Ok(hex_str) => {
                    match hex::decode(&hex_str) {
                        Ok(bytes) => {
                            match bitcoin::consensus::deserialize::<Transaction>(&bytes) {
                                Ok(tx) => break tx,
                                Err(e) => {
                                    log::warn!("   Failed to deserialize transaction, retrying: {}", e);
                                    continue;
                                }
                            }
                        }
                        Err(_) => {
                            log::warn!("   Transaction not yet available in esplora, retrying...");
                            continue;
                        }
                    }
                }
                Err(e) => {
                    log::warn!("   Failed to fetch transaction, retrying: {}", e);
                    continue;
                }
            }
        };

        // Find the change output (the largest output that's not the commit output at index 0)
        let change_output_index = if commit_tx.output.len() > 1 {
            // Assume the last output is the change (after commit and possibly rebar payment)
            commit_tx.output.len() - 1
        } else {
            return Err(AlkanesError::Wallet("No change output found for CPFP".to_string()));
        };

        let change_output = &commit_tx.output[change_output_index];
        let change_value = change_output.value.to_sat();

        log::info!("   Found change output: {} sats at index {}", change_value, change_output_index);

        // Create a transaction that spends the change output with high fee
        // Use 1000 sat/vB to ensure fast confirmation
        let cpfp_fee_rate = 1000.0;
        let estimated_cpfp_vsize = 150; // Rough estimate for 1-input, 1-output tx
        let cpfp_fee = (cpfp_fee_rate * estimated_cpfp_vsize as f32).ceil() as u64;

        if change_value <= cpfp_fee {
            return Err(AlkanesError::Wallet(format!(
                "Change output ({} sats) is too small for CPFP fee ({} sats)",
                change_value, cpfp_fee
            )));
        }

        let cpfp_output_value = change_value.saturating_sub(cpfp_fee);

        log::info!("   CPFP fee: {} sats @ {} sat/vB", cpfp_fee, cpfp_fee_rate);
        log::info!("   Effective package fee rate: {:.1} sat/vB",
            (cpfp_fee as f64) / ((commit_tx.vsize() + estimated_cpfp_vsize) as f64));

        // Get destination address (use change address or wallet address)
        let dest_address_str = if let Some(ref change_addr) = params.change_address {
            change_addr.clone()
        } else {
            WalletProvider::get_address(self.provider).await?
        };
        let dest_address = Address::from_str(&dest_address_str)?
            .require_network(self.provider.get_network())?;

        // Build the CPFP transaction
        let cpfp_outpoint = OutPoint {
            txid: commit_tx.txid(),
            vout: change_output_index as u32,
        };

        let cpfp_tx = Transaction {
            version: bitcoin::transaction::Version::TWO,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: vec![bitcoin::TxIn {
                previous_output: cpfp_outpoint,
                script_sig: ScriptBuf::new(),
                sequence: bitcoin::Sequence::ENABLE_RBF_NO_LOCKTIME,
                witness: bitcoin::Witness::new(),
            }],
            output: vec![TxOut {
                value: bitcoin::Amount::from_sat(cpfp_output_value),
                script_pubkey: dest_address.script_pubkey(),
            }],
        };

        let mut cpfp_psbt = Psbt::from_unsigned_tx(cpfp_tx)?;
        cpfp_psbt.inputs[0] = PsbtInput {
            witness_utxo: Some(change_output.clone()),
            ..Default::default()
        };

        // Sign and broadcast the CPFP transaction
        let signed_cpfp_tx = self.sign_and_finalize_psbt(cpfp_psbt).await?;
        let cpfp_tx_hex = bitcoin::consensus::encode::serialize_hex(&signed_cpfp_tx);

        let cpfp_txid = self.broadcast_with_options(&cpfp_tx_hex, params).await?;

        log::info!("   ✅ CPFP transaction broadcast: {}", cpfp_txid);
        log::info!("   This accelerates both transactions as a package");

        Ok(())
    }

    /// Calculate Rebar payment information for a transaction
    async fn calculate_rebar_payment(&self, tx_vsize: usize, params: &Brc20ProgExecuteParams) -> Result<Option<RebarPaymentInfo>> {
        if !params.use_rebar {
            return Ok(None);
        }

        log::info!("🔒 Querying Rebar Shield for payment info...");
        use crate::provider::rebar;

        let rebar_info = rebar::query_info().await
            .map_err(|e| AlkanesError::Network(format!("Failed to query Rebar info: {}", e)))?;

        let tier_index = params.rebar_tier.unwrap_or(1);
        let tier = rebar::get_tier(&rebar_info, tier_index)
            .map_err(|e| AlkanesError::Network(format!("Failed to get Rebar tier: {}", e)))?;

        let payment_amount = rebar::calculate_payment(tx_vsize, tier);

        log::info!("   Rebar tier {}: {} sat/vB @ {:.0}% hashrate",
                   tier_index, tier.feerate, tier.estimated_hashrate * 100.0);
        log::info!("   Payment amount: {} sats", payment_amount);
        log::info!("   Payment address: {}", rebar_info.payment.p2wpkh);

        let payment_address = Address::from_str(&rebar_info.payment.p2wpkh)?
            .require_network(self.provider.get_network())?;

        Ok(Some(RebarPaymentInfo {
            payment_address,
            payment_amount,
        }))
    }

    /// Select UTXOs for a specific Bitcoin amount
    async fn select_utxos_for_amount(
        &self,
        amount: u64,
        from_addresses: &Option<Vec<String>>,
    ) -> Result<Vec<OutPoint>> {
        self.select_utxos_for_amount_excluding(amount, from_addresses, &[]).await
    }

    /// Select UTXOs for a specific Bitcoin amount, excluding specific transactions
    async fn select_utxos_for_amount_excluding(
        &self,
        amount: u64,
        from_addresses: &Option<Vec<String>>,
        exclude_txids: &[Txid],
    ) -> Result<Vec<OutPoint>> {
        log::info!("Selecting UTXOs for {} sats", amount);

        let utxos = self.provider.get_utxos(true, from_addresses.clone()).await?;
        let spendable_utxos: Vec<(OutPoint, UtxoInfo)> = utxos
            .into_iter()
            .filter(|(outpoint, info)| {
                !info.frozen && !exclude_txids.contains(&outpoint.txid)
            })
            .collect();

        log::info!("Found {} spendable UTXOs", spendable_utxos.len());

        let mut selected_outpoints = Vec::new();
        let mut bitcoin_collected = 0u64;

        for (outpoint, utxo) in spendable_utxos {
            if bitcoin_collected < amount {
                bitcoin_collected += utxo.amount;
                selected_outpoints.push(outpoint);
            } else {
                break;
            }
        }

        if bitcoin_collected < amount {
            return Err(AlkanesError::Wallet(format!(
                "Insufficient funds: need {} sats, have {}",
                amount, bitcoin_collected
            )));
        }

        log::info!(
            "Selected {} UTXOs with {} sats",
            selected_outpoints.len(),
            bitcoin_collected
        );
        Ok(selected_outpoints)
    }

    /// Build commit PSBT
    async fn build_commit_psbt(
        &mut self,
        funding_utxos: Vec<OutPoint>,
        commit_output: TxOut,
        fee_rate: Option<f32>,
        change_address: &Option<String>,
        rebar_payment: Option<RebarPaymentInfo>,
    ) -> Result<(Psbt, u64)> {
        let mut total_input_value = 0;
        let mut input_txouts = Vec::new();
        for outpoint in &funding_utxos {
            let utxo = self
                .provider
                .get_utxo(outpoint)
                .await?
                .ok_or_else(|| AlkanesError::Wallet(format!("UTXO not found: {outpoint}")))?;
            total_input_value += utxo.value.to_sat();
            input_txouts.push(utxo);
        }

        let change_address_str = if let Some(ref addr) = change_address {
            addr.clone()
        } else {
            WalletProvider::get_address(self.provider).await?
        };
        let change_addr = Address::from_str(&change_address_str)?
            .require_network(self.provider.get_network())?;

        let temp_change_output = TxOut {
            value: bitcoin::Amount::from_sat(0),
            script_pubkey: change_addr.script_pubkey(),
        };

        // Build temp outputs list including Rebar payment if needed
        let mut temp_outputs = vec![commit_output.clone()];
        if let Some(ref rebar) = rebar_payment {
            temp_outputs.push(TxOut {
                value: bitcoin::Amount::from_sat(rebar.payment_amount),
                script_pubkey: rebar.payment_address.script_pubkey(),
            });
        }
        temp_outputs.push(temp_change_output);

        let mut temp_tx_for_size = Transaction {
            version: bitcoin::transaction::Version::TWO,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: funding_utxos
                .iter()
                .map(|outpoint| bitcoin::TxIn {
                    previous_output: *outpoint,
                    script_sig: ScriptBuf::new(),
                    sequence: bitcoin::Sequence::ENABLE_RBF_NO_LOCKTIME,
                    witness: bitcoin::Witness::new(),
                })
                .collect(),
            output: temp_outputs.clone(),
        };
        for input in &mut temp_tx_for_size.input {
            input.witness.push([0u8; 65]);
        }

        // For Rebar, fee to miners is 0 (payment goes to Rebar output)
        let fee = if rebar_payment.is_some() {
            0
        } else {
            let fee_rate_sat_vb = fee_rate.unwrap_or(600.0);
            (fee_rate_sat_vb * temp_tx_for_size.vsize() as f32).ceil() as u64
        };

        let rebar_payment_amount = rebar_payment.as_ref().map(|r| r.payment_amount).unwrap_or(0);

        let change_value = total_input_value
            .saturating_sub(commit_output.value.to_sat())
            .saturating_sub(rebar_payment_amount)
            .saturating_sub(fee);
        if change_value < 546 {
            return Err(AlkanesError::Wallet(
                "Not enough funds for commit and change".to_string(),
            ));
        }

        let final_change_output = TxOut {
            value: bitcoin::Amount::from_sat(change_value),
            script_pubkey: change_addr.script_pubkey(),
        };

        // Build final outputs list including Rebar payment if needed
        let mut final_outputs = vec![commit_output];
        if let Some(rebar) = rebar_payment {
            final_outputs.push(TxOut {
                value: bitcoin::Amount::from_sat(rebar.payment_amount),
                script_pubkey: rebar.payment_address.script_pubkey(),
            });
        }
        final_outputs.push(final_change_output);

        let unsigned_tx = Transaction {
            version: bitcoin::transaction::Version::TWO,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: funding_utxos
                .iter()
                .map(|outpoint| bitcoin::TxIn {
                    previous_output: *outpoint,
                    script_sig: ScriptBuf::new(),
                    sequence: bitcoin::Sequence::ENABLE_RBF_NO_LOCKTIME,
                    witness: bitcoin::Witness::new(),
                })
                .collect(),
            output: final_outputs,
        };
        let mut psbt = Psbt::from_unsigned_tx(unsigned_tx)?;

        for (i, utxo) in input_txouts.iter().enumerate() {
            psbt.inputs[i].witness_utxo = Some(utxo.clone());
            if utxo.script_pubkey.is_p2tr() {
                let (internal_key, (fingerprint, path)) = self.provider.get_internal_key().await?;
                psbt.inputs[i].tap_internal_key = Some(internal_key);
                psbt.inputs[i]
                    .tap_key_origins
                    .insert(internal_key, (vec![], (fingerprint, path)));
            }
        }

        Ok((psbt, fee))
    }

    /// Build reveal PSBT
    async fn build_reveal_psbt(
        &mut self,
        utxos_with_txouts: Vec<(OutPoint, TxOut)>,
        mut outputs: Vec<TxOut>,
        fee_rate: Option<f32>,
        envelope: Option<&Brc20ProgEnvelope>,
        commit_internal_key: XOnlyPublicKey,
        rebar_payment: Option<RebarPaymentInfo>,
        locktime: Option<u32>,
    ) -> Result<(Psbt, u64)> {
        use bitcoin::transaction::Version;

        let mut total_input_value = 0;
        let mut input_txouts = Vec::new();
        let utxos: Vec<OutPoint> = utxos_with_txouts.iter().map(|(op, _)| *op).collect();
        
        for (_outpoint, txout) in &utxos_with_txouts {
            total_input_value += txout.value.to_sat();
            input_txouts.push(txout.clone());
        }

        // Use locktime if provided (for CLTV strategy)
        let tx_locktime = if let Some(height) = locktime {
            bitcoin::absolute::LockTime::from_height(height)
                .map_err(|e| AlkanesError::Wallet(format!("Invalid locktime height: {}", e)))?
        } else {
            bitcoin::absolute::LockTime::ZERO
        };

        let mut temp_tx = Transaction {
            version: Version::TWO,
            lock_time: tx_locktime,
            input: utxos
                .iter()
                .map(|outpoint| bitcoin::TxIn {
                    previous_output: *outpoint,
                    script_sig: ScriptBuf::new(),
                    sequence: bitcoin::Sequence::ENABLE_RBF_NO_LOCKTIME,
                    witness: bitcoin::Witness::new(),
                })
                .collect(),
            output: outputs.clone(),
        };

        for (i, input) in temp_tx.input.iter_mut().enumerate() {
            if let Some(env) = envelope {
                if i == 0 {
                    // Script-path spend - calculate actual witness size
                    // Witness structure: [signature (65), script, control_block (33)]
                    let reveal_script = env.build_reveal_script();
                    let script_size = reveal_script.len();
                    let control_block_size = 33; // Fixed size for control block
                    let signature_size = 65; // Schnorr signature + sighash type
                    
                    // Create realistic witness placeholder
                    input.witness.push(vec![0u8; signature_size]);
                    input.witness.push(vec![0u8; script_size]);
                    input.witness.push(vec![0u8; control_block_size]);
                    continue;
                }
            }
            // Key-path spend
            input.witness.push([0u8; 65]);
        }

        // For Rebar, fee to miners is 0 (payment goes to Rebar output)
        let capped_fee = if rebar_payment.is_some() {
            0
        } else {
            let fee_rate_sat_vb = fee_rate.unwrap_or(600.0);
            let estimated_fee = (fee_rate_sat_vb * temp_tx.vsize() as f32).ceil() as u64;
            estimated_fee.min(MAX_FEE_SATS)
        };

        // Add Rebar payment output if needed
        if let Some(ref rebar) = rebar_payment {
            outputs.insert(outputs.len() - 1, TxOut {
                value: bitcoin::Amount::from_sat(rebar.payment_amount),
                script_pubkey: rebar.payment_address.script_pubkey(),
            });
        }

        let total_output_value_sans_change: u64 = outputs
            .iter()
            .filter(|o| o.value.to_sat() > 0 && !o.script_pubkey.is_op_return())
            .map(|o| o.value.to_sat())
            .sum();

        let change_value = total_input_value
            .saturating_sub(total_output_value_sans_change)
            .saturating_sub(capped_fee);

        if let Some(change_output) = outputs
            .iter_mut()
            .find(|o| o.value.to_sat() == 1 && !o.script_pubkey.is_op_return())
        {
            change_output.value = bitcoin::Amount::from_sat(change_value);
        }

        let mut psbt = Psbt::from_unsigned_tx(Transaction {
            version: Version::TWO,
            lock_time: tx_locktime,
            input: utxos
                .iter()
                .map(|outpoint| bitcoin::TxIn {
                    previous_output: *outpoint,
                    script_sig: ScriptBuf::new(),
                    sequence: bitcoin::Sequence::ENABLE_RBF_NO_LOCKTIME,
                    witness: bitcoin::Witness::new(),
                })
                .collect(),
            output: outputs,
        })?;

        for (i, utxo) in input_txouts.iter().enumerate() {
            psbt.inputs[i].witness_utxo = Some(utxo.clone());
            if i == 0 && envelope.is_some() {
                // First input is the commit UTXO with script-path spending
                psbt.inputs[i].tap_internal_key = Some(commit_internal_key);
            } else if utxo.script_pubkey.is_p2tr() {
                let (internal_key, (fingerprint, path)) = self.provider.get_internal_key().await?;
                psbt.inputs[i].tap_internal_key = Some(internal_key);
                psbt.inputs[i]
                    .tap_key_origins
                    .insert(internal_key, (vec![], (fingerprint, path)));
            }
        }

        Ok((psbt, capped_fee))
    }

    /// Sign and finalize a PSBT
    async fn sign_and_finalize_psbt(&mut self, mut psbt: Psbt) -> Result<Transaction> {
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

    /// Sign and finalize the reveal PSBT with script-path spending
    async fn sign_and_finalize_reveal_psbt(
        &mut self,
        psbt: &mut Psbt,
        envelope: &Brc20ProgEnvelope,
        commit_internal_key: XOnlyPublicKey,
    ) -> Result<Transaction> {
        use bitcoin::sighash::{Prevouts, SighashCache, TapSighashType};
        use bitcoin::taproot::{LeafVersion, TapLeafHash, TaprootBuilder};

        // No wallet inputs to sign - commit output covers reveal fee
        // Input 0 will be signed with script-path spending below
        if psbt.inputs.len() > 1 {
            log::warn!("   ⚠️  Unexpected: reveal has {} inputs (expected only commit input)", psbt.inputs.len());
        }

        // Get the unsigned transaction (needed for script-path signing of input 0)
        let unsigned_tx = psbt.unsigned_tx.clone();

        // Build taproot spend info for the commit input (input 0)
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
        let prevouts: Vec<TxOut> = psbt
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

        // Calculate sighash for script-path spending (input 0 - the commit)
        let mut sighash_cache = SighashCache::new(&unsigned_tx);
        let leaf_hash = TapLeafHash::from_script(&reveal_script, LeafVersion::TapScript);
        let sighash = sighash_cache
            .taproot_script_spend_signature_hash(0, &prevouts_all, leaf_hash, TapSighashType::Default)
            .map_err(|e| AlkanesError::Transaction(e.to_string()))?;

        // Sign the sighash for the commit input
        let signature = self
            .provider
            .sign_taproot_script_spend(sighash.into())
            .await?;
        let taproot_signature = bitcoin::taproot::Signature {
            signature,
            sighash_type: TapSighashType::Default,
        };
        let signature_bytes = taproot_signature.to_vec();

        // Create the complete witness for the commit input
        let witness = envelope.create_complete_witness(&signature_bytes, control_block)?;

        // Create the final transaction
        let mut tx = unsigned_tx.clone();
        tx.input[0].witness = witness;

        // Add witnesses for other inputs (already signed by provider)
        for i in 1..tx.input.len() {
            if let Some(tap_key_sig) = &psbt.inputs[i].tap_key_sig {
                tx.input[i].witness = bitcoin::Witness::p2tr_key_spend(tap_key_sig);
            } else {
                log::warn!("   ⚠️  Warning: Input {} has no signature after signing", i);
            }
        }

        Ok(tx)
    }

    /// Mine blocks on regtest if needed
    async fn mine_blocks_if_regtest(&self, params: &Brc20ProgExecuteParams) -> Result<()> {
        if self.provider.get_network() == bitcoin::Network::Regtest {
            log::info!("Mining block on regtest network...");
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
            let address = if let Some(ref change_address) = params.change_address {
                change_address.clone()
            } else {
                WalletProvider::get_address(self.provider).await?
            };
            self.provider.generate_to_address(1, &address).await?;
        }
        Ok(())
    }

    /// Wait for blockchain to reach a specific height
    async fn wait_for_block_height(&self, target_height: u64, params: &Brc20ProgExecuteParams) -> Result<()> {
        use crate::traits::BitcoinRpcProvider;

        // On regtest, mine blocks to reach target height
        if self.provider.get_network() == bitcoin::Network::Regtest && params.mine_enabled {
            let current_height = self.provider.get_block_count().await?;
            let blocks_to_mine = target_height.saturating_sub(current_height);

            if blocks_to_mine > 0 {
                log::info!("   Mining {} blocks to reach target height {}", blocks_to_mine, target_height);
                for _ in 0..blocks_to_mine {
                    self.mine_blocks_if_regtest(params).await?;
                }
                self.provider.sync().await?;
            }
            return Ok(());
        }

        // On mainnet/testnet, poll until we reach the target height
        log::info!("   Polling for block height {}...", target_height);

        loop {
            let current_height = self.provider.get_block_count().await?;

            if current_height >= target_height {
                log::info!("   ✅ Block height {} reached", current_height);
                return Ok(());
            }

            let blocks_remaining = target_height.saturating_sub(current_height);
            log::info!("   Current height: {}, waiting for {} more blocks", current_height, blocks_remaining);

            // Wait 30 seconds before checking again (average block time ~10 min, but check more frequently)
            tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
        }
    }

    /// Wait for a transaction to be confirmed (mined in a block)
    /// This is required when using slipstream or rebar since they don't propagate to mempool
    /// Note: For slipstream/rebar, there is NO timeout - it will wait indefinitely for confirmation
    async fn wait_for_confirmation(&self, txid: &str, params: &Brc20ProgExecuteParams) -> Result<()> {
        // On regtest, mine a block immediately
        if self.provider.get_network() == bitcoin::Network::Regtest && params.mine_enabled {
            self.mine_blocks_if_regtest(params).await?;
            self.provider.sync().await?;
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            return Ok(());
        }

        log::info!("⏳ Polling for confirmation every 5 seconds (no timeout for slipstream/rebar)...");
        log::info!("   Transaction may take hours or days to confirm depending on network conditions");

        let mut attempts = 0;

        loop {
            attempts += 1;

            // Log progress every 60 attempts (5 minutes)
            if attempts % 60 == 0 {
                log::info!("   Still waiting... ({} checks, ~{} minutes elapsed)", attempts, attempts * 5 / 60);
            }

            // Try to get the transaction status
            match self.provider.get_tx_status(txid).await {
                Ok(status) => {
                    // Parse the status JSON - esplora returns {"confirmed": bool, "block_height": number, ...}
                    let confirmed = status.get("confirmed")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);

                    if confirmed {
                        let block_height = status.get("block_height")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0);
                        log::info!("✅ Transaction confirmed in block {} after {} checks", block_height, attempts);
                        // Give the indexer a moment to process the block
                        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                        self.provider.sync().await?;
                        return Ok(());
                    } else {
                        log::debug!("Transaction still in mempool, waiting...");
                    }
                }
                Err(e) => {
                    log::debug!("Transaction not found yet: {}", e);
                }
            }

            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        }
    }

    /// Broadcast transaction with optional slipstream or rebar
    async fn broadcast_with_options(&self, tx_hex: &str, params: &Brc20ProgExecuteParams) -> Result<String> {
        if params.use_rebar {
            log::info!("🔒 Using Rebar Shield for private transaction broadcast");
            use crate::provider::rebar;
            rebar::submit_transaction(tx_hex).await
                .map_err(|e| AlkanesError::Network(format!("Rebar Shield error: {}", e)))
        } else if params.use_slipstream {
            log::info!("🚀 Using MARA Slipstream for transaction broadcast");

            let client = reqwest::Client::new();
            let payload = serde_json::json!({
                "tx_hex": tx_hex
            });

            let response = client
                .post("https://slipstream.mara.com/rest-api/submit-tx")
                .header("Content-Type", "application/json")
                .json(&payload)
                .send()
                .await
                .map_err(|e| AlkanesError::Network(format!("Slipstream request failed: {}", e)))?;

            let status = response.status();
            let response_text = response.text().await
                .map_err(|e| AlkanesError::Network(format!("Failed to read Slipstream response: {}", e)))?;

            if !status.is_success() {
                return Err(AlkanesError::Network(format!("Slipstream error ({}): {}", status, response_text)));
            }

            let response_json: serde_json::Value = serde_json::from_str(&response_text)
                .map_err(|e| AlkanesError::Network(format!("Failed to parse Slipstream response: {}", e)))?;

            // Extract txid from response (it's in the "message" field)
            if let Some(txid) = response_json.get("message").and_then(|v| v.as_str()) {
                log::info!("✅ Transaction submitted to MARA Slipstream successfully!");
                Ok(txid.to_string())
            } else if let Some(txid) = response_json.get("txid").and_then(|v| v.as_str()) {
                // Fallback to "txid" field in case API changes
                log::info!("✅ Transaction submitted to MARA Slipstream successfully!");
                Ok(txid.to_string())
            } else {
                Err(AlkanesError::Network(format!("Slipstream response missing txid: {}", response_text)))
            }
        } else {
            // Standard broadcast
            self.provider.broadcast_transaction(tx_hex.to_string()).await
        }
    }
}
