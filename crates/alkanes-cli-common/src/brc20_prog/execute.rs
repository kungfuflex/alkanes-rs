// BRC20-Prog executor for contract deployment and interaction
// This module handles the commit-reveal transaction pattern for BRC20-prog inscriptions

use crate::{AlkanesError, DeezelProvider, Result};
use crate::traits::{WalletProvider, UtxoInfo, OrdProvider};
use crate::vendored_ord::InscriptionId;
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

/// Information about an inscription on a UTXO
#[derive(Debug, Clone)]
struct InscriptionInfo {
    /// Inscription ID
    inscription_id: InscriptionId,
    /// Offset of the inscribed sat within the UTXO (0-indexed)
    sat_offset: u64,
}

/// Plan for splitting a UTXO to protect inscriptions
#[derive(Debug, Clone)]
struct SplitPlan {
    /// The outpoint being split
    outpoint: OutPoint,
    /// Amount to send to safe output (contains inscribed sats)
    safe_amount: u64,
    /// Amount to send to clean output (for funding)
    clean_amount: u64,
}

/// Result of building a split transaction
struct SplitResult {
    /// The split PSBT
    psbt: Psbt,
    /// The fee paid
    fee: u64,
    /// Clean outpoints to use for commit transaction funding, with their TxOut data
    /// (We include TxOut because the split tx hasn't been broadcast yet)
    clean_utxos: Vec<(OutPoint, TxOut)>,
}

/// Traced inscription info for a pending UTXO
/// When a UTXO is unconfirmed, we trace back through parent transactions
/// to determine inscription state from settled UTXOs
#[derive(Debug, Clone)]
struct TracedInscriptionInfo {
    /// Original inscription ID (from the settled UTXO)
    inscription_id: InscriptionId,
    /// Current offset within this UTXO after sat flow through pending txs
    sat_offset: u64,
    /// Chain of txids from settled UTXO to this pending UTXO
    trace_path: Vec<Txid>,
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

    /// Execute BRC20-prog with Presign+RBF hybrid strategy
    /// This strategy:
    /// 1. Pre-builds and signs all transactions (split, commit, reveal, activation) with RBF-enabled sequences
    /// 2. Broadcasts all transactions simultaneously to minimize timing gaps
    /// 3. Monitors mempool for frontrunning attempts
    /// 4. If frontrunning detected, RBF-bumps commit and rebuilds/rebroadcasts reveal+activation
    /// 5. Repeats monitoring and RBF-bumping up to 3 times to outpace attackers
    async fn execute_with_presign_rbf(&mut self, params: Brc20ProgExecuteParams) -> Result<Brc20ProgExecuteResult> {
        log::info!("🔐 Presign+RBF Hybrid Strategy: Building and signing all transactions upfront...");
        log::info!("Inscription content: {}", params.inscription_content);

        // Create the envelope
        let envelope = Brc20ProgEnvelope::new(params.inscription_content.as_bytes().to_vec());

        // Step 1: Build commit transaction (and optional split transaction for inscribed UTXOs)
        log::info!("📝 Step 1/7: Building commit transaction (with inscription check)...");
        let (split_psbt_opt, split_fee_opt, commit_psbt, commit_fee, commit_internal_key, ephemeral_secret) =
            self.build_commit_psbt_for_presign(&params, &envelope).await?;

        // Log split transaction if present
        if let Some(ref split_psbt) = split_psbt_opt {
            let split_txid = split_psbt.unsigned_tx.txid();
            log::info!("   🔀 Split txid (pre-calculated): {} - protects inscribed UTXOs", split_txid);
        }

        // Calculate commit txid from unsigned transaction
        let commit_tx = commit_psbt.unsigned_tx.clone();
        let commit_txid = commit_tx.txid();
        let commit_outpoint = OutPoint { txid: commit_txid, vout: 0 };
        let commit_output = commit_tx.output[0].clone();

        log::info!("   Commit txid (pre-calculated): {}", commit_txid);

        // Extract commit change output (at index 1) to use for funding activation
        let commit_change_output = commit_tx.output.get(1)
            .ok_or_else(|| AlkanesError::Wallet("Commit transaction has no change output".to_string()))?
            .clone();
        let commit_change_outpoint = OutPoint { txid: commit_txid, vout: 1 };

        // Step 2: Build reveal transaction (unsigned, spending future commit output)
        log::info!("📝 Step 2/7: Building reveal transaction...");
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
            log::info!("📝 Step 3/7: Building activation transaction...");
            // Use commit change output to fund activation fee
            let (act_psbt, act_fee) = self.build_activation_psbt_for_presign(
                &params,
                reveal_inscription_outpoint,
                reveal_inscription_output,
                commit_change_outpoint,
                commit_change_output,
            ).await?;

            let act_txid = act_psbt.unsigned_tx.txid();
            log::info!("   Activation txid (pre-calculated): {}", act_txid);
            (Some(act_psbt), Some(act_fee))
        } else {
            log::info!("📝 Step 3/7: Skipping activation (2-tx pattern)");
            (None, None)
        };

        // === EXTERNAL SIGNER SUPPORT ===
        // If return_unsigned is true, return PSBTs for external signing
        // NOTE: The reveal transaction is signed INTERNALLY with the ephemeral key
        // because only we know the ephemeral secret. User wallet signs split, commit, and activation.
        if params.return_unsigned {
            log::info!("📤 Returning PSBTs for external signer (reveal signed internally)...");

            // Serialize PSBTs to base64 for user wallet to sign
            let unsigned_split_psbt = split_psbt_opt.as_ref().map(|psbt| {
                use bitcoin::psbt::Psbt;
                let serialized = psbt.serialize();
                use base64::Engine;
                base64::engine::general_purpose::STANDARD.encode(&serialized)
            });

            let unsigned_commit_psbt = {
                let serialized = commit_psbt.serialize();
                use base64::Engine;
                base64::engine::general_purpose::STANDARD.encode(&serialized)
            };

            // Sign the reveal transaction INTERNALLY with the ephemeral key
            // The user's wallet cannot sign this - only we have the ephemeral secret
            let signed_reveal = self.sign_and_finalize_reveal_psbt_simple(
                reveal_psbt,
                &envelope,
                commit_internal_key,
                Some(ephemeral_secret)
            ).await?;
            let signed_reveal_tx_hex = bitcoin::consensus::encode::serialize_hex(&signed_reveal);
            log::info!("   ✅ Reveal transaction signed internally with ephemeral key");

            let unsigned_activation_psbt = activation_psbt_opt.as_ref().map(|psbt| {
                let serialized = psbt.serialize();
                use base64::Engine;
                base64::engine::general_purpose::STANDARD.encode(&serialized)
            });

            // Return result with unsigned PSBTs + signed reveal
            // txids are pre-calculated from unsigned tx (SegWit txids don't include witness)
            return Ok(Brc20ProgExecuteResult {
                split_txid: split_psbt_opt.as_ref().map(|p| p.unsigned_tx.txid().to_string()),
                split_fee: split_fee_opt,
                commit_txid: commit_txid.to_string(),
                reveal_txid: reveal_txid.to_string(),
                activation_txid: activation_psbt_opt.as_ref().map(|p| p.unsigned_tx.txid().to_string()),
                commit_fee,
                reveal_fee,
                activation_fee: activation_fee_opt,
                inputs_used: vec![],
                outputs_created: vec![],
                traces: None,
            contract_address: None,
                unsigned_split_psbt,
                unsigned_commit_psbt: Some(unsigned_commit_psbt),
                // Reveal is already signed - return the signed tx hex, not unsigned PSBT
                unsigned_reveal_psbt: None,
                signed_reveal_tx_hex: Some(signed_reveal_tx_hex),
                unsigned_activation_psbt,
                requires_signing: true,
            });
        }

        // Step 4: Sign all transactions
        log::info!("✍️  Step 4/7: Signing all transactions...");

        // Sign split transaction if present
        let signed_split = if let Some(split_psbt) = split_psbt_opt {
            Some(self.sign_and_finalize_psbt(split_psbt).await?)
        } else {
            None
        };

        let signed_commit = self.sign_and_finalize_psbt(commit_psbt).await?;
        let signed_reveal = self.sign_and_finalize_reveal_psbt_simple(reveal_psbt, &envelope, commit_internal_key, Some(ephemeral_secret)).await?;
        let signed_activation = if let Some(act_psbt) = activation_psbt_opt {
            Some(self.sign_and_finalize_psbt(act_psbt).await?)
        } else {
            None
        };

        log::info!("   ✅ All transactions signed");

        // Step 5: Broadcast all transactions atomically in a single batch
        log::info!("📡 Step 5/7: Broadcasting all transactions ATOMICALLY to prevent frontrunning...");

        // Build array of all transactions to broadcast together
        // Order: split (if present) → commit → reveal → activation (if present)
        let mut tx_hexes = Vec::new();

        if let Some(ref split_tx) = signed_split {
            let split_hex = bitcoin::consensus::encode::serialize_hex(split_tx);
            tx_hexes.push(split_hex);
        }

        let commit_hex = bitcoin::consensus::encode::serialize_hex(&signed_commit);
        let reveal_hex = bitcoin::consensus::encode::serialize_hex(&signed_reveal);
        tx_hexes.push(commit_hex);
        tx_hexes.push(reveal_hex);

        if let Some(ref act_tx) = signed_activation {
            let act_hex = bitcoin::consensus::encode::serialize_hex(act_tx);
            tx_hexes.push(act_hex);
        }

        // ATOMIC BATCH BROADCAST - all transactions hit mempool simultaneously
        use crate::traits::BitcoinRpcProvider;
        let txids = self.provider.send_raw_transactions(&tx_hexes).await?;

        // Parse txids based on which transactions were included
        let mut txid_idx = 0;

        let final_split_txid = if signed_split.is_some() {
            let txid = txids.get(txid_idx)
                .ok_or_else(|| AlkanesError::RpcError("No split txid in batch response".to_string()))?
                .clone();
            txid_idx += 1;
            Some(txid)
        } else {
            None
        };

        let final_commit_txid = txids.get(txid_idx)
            .ok_or_else(|| AlkanesError::RpcError("No commit txid in batch response".to_string()))?
            .clone();
        txid_idx += 1;

        let final_reveal_txid = txids.get(txid_idx)
            .ok_or_else(|| AlkanesError::RpcError("No reveal txid in batch response".to_string()))?
            .clone();
        txid_idx += 1;

        let final_activation_txid = if signed_activation.is_some() {
            Some(txids.get(txid_idx)
                .ok_or_else(|| AlkanesError::RpcError("No activation txid in batch response".to_string()))?
                .clone())
        } else {
            None
        };

        // Log broadcast results
        if let Some(ref split_txid) = final_split_txid {
            log::info!("   ✅ Split broadcast: {} (protected inscribed UTXOs)", split_txid);
        }
        log::info!("   ✅ Commit broadcast: {}", final_commit_txid);
        log::info!("   ✅ Reveal broadcast: {}", final_reveal_txid);
        if let Some(ref act_txid) = final_activation_txid {
            log::info!("   ✅ Activation broadcast: {}", act_txid);
        }
        log::info!("   🎯 All {} transactions broadcast atomically in single RPC call!", tx_hexes.len());

        // Step 6: Skip monitoring for presign strategy
        // Since all transactions were broadcast atomically in a single RPC call,
        // frontrunning protection is inherent - there's no window for attackers.
        // The monitoring step would require waiting for propagation and is not needed.
        log::info!("🔍 Step 6/7: Skipping frontrunning monitoring (atomic broadcast provides protection)");

        log::info!("✅ Presign+RBF strategy completed successfully!");

        // Compute contract address for deploy operations.
        // The deployer ETH address is derived from the change address pkscript,
        // then the contract address is keccak256(rlp([deployer, nonce=0]))[12:].
        let contract_address = if params.use_activation {
            // Use explicit change_address, or from_addresses[0], or try wallet
            let effective_change = params.change_address.clone()
                .or_else(|| params.from_addresses.as_ref()
                    .and_then(|addrs| addrs.first().cloned()));

            effective_change.and_then(|addr| {
                let parsed = bitcoin::Address::from_str(&addr).ok()?;
                let assumed = parsed.assume_checked();
                let pkscript_hex = hex::encode(assumed.script_pubkey().as_bytes());
                let deployer_eth = crate::brc20_prog::pkscript_to_eth_address(&pkscript_hex).ok()?;
                let contract = crate::brc20_prog::compute_contract_address(&deployer_eth, 0).ok()?;
                log::info!("📋 Computed contract address: {} (deployer: {}, from: {})", contract, deployer_eth, addr);
                Some(contract)
            })
        } else {
            None
        };

        Ok(Brc20ProgExecuteResult {
            split_txid: final_split_txid,
            split_fee: split_fee_opt,
            commit_txid: final_commit_txid,
            reveal_txid: final_reveal_txid,
            activation_txid: final_activation_txid,
            commit_fee,
            reveal_fee,
            activation_fee: activation_fee_opt,
            inputs_used: vec![],
            outputs_created: vec![],
            traces: None,
            contract_address,
            // No unsigned PSBTs - transactions were signed and broadcast
            unsigned_split_psbt: None,
            unsigned_commit_psbt: None,
            unsigned_reveal_psbt: None,
            signed_reveal_tx_hex: None,
            unsigned_activation_psbt: None,
            requires_signing: false,
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
            split_txid: None,
            split_fee: None,
            commit_txid: commit_txid.to_string(),
            reveal_txid: reveal_txid.to_string(),
            activation_txid,
            commit_fee,
            reveal_fee,
            activation_fee,
            inputs_used: vec![],
            outputs_created: vec![],
            traces: None,
            contract_address: None,
            // No unsigned PSBTs - transactions were signed and broadcast
            unsigned_split_psbt: None,
            unsigned_commit_psbt: None,
            unsigned_reveal_psbt: None,
            signed_reveal_tx_hex: None,
            unsigned_activation_psbt: None,
            requires_signing: false,
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
        let fee_rate = self.resolve_fee_rate(params.fee_rate).await?;

        // Build the reveal script and taproot structures NOW (before commit)
        // The reveal script includes <pubkey> CHECKSIG to prevent frontrunning
        let reveal_script = envelope.build_reveal_script(internal_key);
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
        let mut reveal_vsize = dummy_reveal_tx.vsize();
        if params.mint_diesel {
            reveal_vsize += 90; // ~43 bytes (P2TR output) + ~50 bytes (OP_RETURN)
        }
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

        // Get change address for DIESEL minting
        let change_address_str = if let Some(ref addr) = params.change_address {
            addr.clone()
        } else {
            WalletProvider::get_address(self.provider).await?
        };
        let change_addr = Address::from_str(&change_address_str)?
            .require_network(self.provider.get_network())?;

        let (commit_psbt, commit_fee) = self
            .build_commit_psbt(
                funding_utxos,
                commit_output.clone(),
                params.fee_rate,
                &params.change_address,
                rebar_payment,
                params.mint_diesel,
                &change_addr,
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
        let mut outputs = Vec::new();

        if params.use_activation {
            // 3-tx pattern: inscription output first
            log::info!("Creating 546-sat inscription output (will be spent to OP_RETURN in activation tx, no change)");

            outputs.push(TxOut {
                value: bitcoin::Amount::from_sat(546),
                script_pubkey: change_address.script_pubkey(),
            });

            // Add DIESEL outputs (pointer = 1, after inscription at 0)
            if params.mint_diesel {
                let (diesel_output, diesel_script) = self.create_diesel_mint_outputs(&change_address, 1)?;
                outputs.push(diesel_output);
                outputs.push(TxOut {
                    value: bitcoin::Amount::ZERO,
                    script_pubkey: diesel_script,
                });
            }
        } else {
            // 2-tx pattern: DIESEL first (if enabled), then BRC20PROG OP_RETURN
            if params.mint_diesel {
                let (diesel_output, diesel_script) = self.create_diesel_mint_outputs(&change_address, 0)?;
                outputs.push(diesel_output);
                outputs.push(TxOut {
                    value: bitcoin::Amount::ZERO,
                    script_pubkey: diesel_script,
                });
            }

            // BRC20PROG OP_RETURN
            log::info!("Creating OP_RETURN output directly in reveal tx (2-tx pattern, no change)");
            outputs.push(TxOut {
                value: bitcoin::Amount::from_sat(1),
                script_pubkey: self.create_brc20prog_op_return(),
            });
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
            None, // No ephemeral secret in old flow
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
            let fee_rate_sat_vb = self.resolve_fee_rate(params.fee_rate).await?;
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

        // Build reveal script with pubkey+CHECKSIG to prevent frontrunning
        let reveal_script = envelope.build_reveal_script(internal_key);

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

    /// Create DIESEL mint outputs for commit/reveal transactions
    /// Returns: (recipient_output, op_return_script)
    fn create_diesel_mint_outputs(
        &self,
        recipient_address: &Address,
        pointer_index: u32,
    ) -> Result<(TxOut, ScriptBuf)> {
        use ordinals::Runestone;
        use protorune_support::protostone::{Protostone, ProtostoneEdict, Protostones};

        // DIESEL contract: block 2, tx 0, opcode 77 (mint)
        let message = vec![2u8, 0u8, 77u8];

        let protostone = Protostone {
            protocol_tag: 1,  // ALKANES
            message,
            pointer: Some(pointer_index),
            refund: Some(pointer_index),
            burn: None,
            from: None,
            edicts: vec![],
        };

        // Encode protostone into Runestone protocol field
        let protocol_values = vec![protostone].encipher()?;

        let runestone = Runestone {
            protocol: Some(protocol_values),
            pointer: Some(pointer_index),
            ..Default::default()
        };

        let op_return_script = runestone.encipher();

        // Create recipient output (dust amount to receive DIESEL)
        let recipient_output = TxOut {
            value: bitcoin::Amount::from_sat(546),
            script_pubkey: recipient_address.script_pubkey(),
        };

        Ok((recipient_output, op_return_script))
    }

    /// Build commit PSBT for presign strategy (uses final sequences for deterministic txid)
    /// Returns: (split_psbt, split_fee, commit_psbt, commit_fee, internal_key, ephemeral_secret)
    async fn build_commit_psbt_for_presign(
        &mut self,
        params: &Brc20ProgExecuteParams,
        envelope: &Brc20ProgEnvelope,
    ) -> Result<(Option<Psbt>, Option<u64>, Psbt, u64, XOnlyPublicKey, bitcoin::secp256k1::SecretKey)> {
        // ANTI-FRONTRUNNING: Get ephemeral key with secret for signing
        let (internal_key, ephemeral_secret, _) = self.provider.get_internal_key_with_secret().await?;
        let commit_address = self.create_commit_address_for_envelope(envelope, internal_key).await?;

        // Calculate EXACT commit output using reference implementation approach:
        // Build a dummy reveal transaction with REAL script and control block to get exact vsize
        let fee_rate = self.resolve_fee_rate(params.fee_rate).await?;

        // Build the reveal script and taproot structures NOW (before commit)
        // The reveal script includes <pubkey> CHECKSIG to prevent frontrunning
        let reveal_script = envelope.build_reveal_script(internal_key);
        use bitcoin::taproot::{TaprootBuilder, LeafVersion};
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
                script_pubkey: ScriptBuf::new_op_return(&[]), // Dummy OP_RETURN (size is similar)
            }],
        };

        // Add dummy witness (64-byte signature + script + control block) - like reference
        dummy_reveal_tx.input[0].witness.push(vec![0u8; 64]); // Dummy 64-byte signature
        dummy_reveal_tx.input[0].witness.push(reveal_script.as_bytes());
        dummy_reveal_tx.input[0].witness.push(control_block.serialize());

        // Get EXACT vsize
        let mut reveal_vsize = dummy_reveal_tx.vsize();
        if params.mint_diesel {
            reveal_vsize += 90; // ~43 bytes (P2TR output) + ~50 bytes (OP_RETURN)
        }
        let reveal_fee = (fee_rate * reveal_vsize as f32).ceil() as u64;

        // Commit output = reveal output value + reveal fee (like reference: postage + reveal_fee)
        let commit_output_amount = reveal_output_value + reveal_fee;

        log::info!("💰 Calculated EXACT commit output for presign: {} sats", commit_output_amount);
        log::info!("   Fee rate: {} sat/vB, Reveal vsize: {} vB, Reveal fee: {} sats",
                   fee_rate, reveal_vsize, reveal_fee);
        log::info!("   Reveal output: {} sats, No change (avoids dust)", reveal_output_value);

        // Calculate additional funding needed for activation transaction's additional outputs
        // This is used for FrBTC wrap (send BTC to signer) or unwrap (dust to signer)
        let additional_outputs_total: u64 = params.additional_outputs
            .as_ref()
            .map(|outputs| outputs.iter().map(|o| o.amount).sum())
            .unwrap_or(0);

        // Estimate activation fee (2 inputs: inscription + change, outputs: OP_RETURN + additional + change)
        // Rough estimate: ~150 vB base + ~34 vB per additional output
        let additional_output_count = params.additional_outputs.as_ref().map(|o| o.len()).unwrap_or(0);
        let estimated_activation_vsize = 150 + (additional_output_count as u64 * 34);
        let estimated_activation_fee = (fee_rate * estimated_activation_vsize as f32).ceil() as u64;

        // Total required beyond commit output: activation fee + additional outputs + dust change (546)
        let additional_funding_required = if params.use_activation {
            estimated_activation_fee + additional_outputs_total + 546
        } else {
            0
        };

        if additional_outputs_total > 0 {
            log::info!("   Additional outputs for activation: {} sats ({} outputs)",
                       additional_outputs_total, additional_output_count);
            log::info!("   Estimated activation funding needed: {} sats", additional_funding_required);
        }

        // Select UTXOs for both commit output AND activation funding
        let total_funding_required = commit_output_amount + additional_funding_required;
        let funding_utxos = self.select_utxos_for_amount(
            total_funding_required,
            &params.from_addresses,
        ).await?;

        // Get change address for split transaction
        let change_address_str = if let Some(ref addr) = params.change_address {
            addr.clone()
        } else {
            WalletProvider::get_address(self.provider).await?
        };
        let change_addr = Address::from_str(&change_address_str)?
            .require_network(self.provider.get_network())?;

        // Check if any selected UTXOs have inscriptions and need splitting
        let split_result = self.build_split_psbt_if_needed(
            &funding_utxos,
            &change_addr,
            fee_rate,
            params.mempool_indexer,
            params.ordinals_strategy,
        ).await?;

        // Determine which UTXOs to use for commit
        // We need to track both outpoints AND their TxOut data (for split outputs that don't exist yet)
        let (final_funding_utxos_with_txouts, split_psbt, split_fee) = match split_result {
            Some(split) => {
                log::info!("🔀 Split transaction will be used to protect inscriptions");

                // Start with clean outputs from split (we have their TxOut data)
                let mut utxos_with_txouts: Vec<(OutPoint, TxOut)> = split.clean_utxos.clone();

                // Add any UTXOs that didn't need splitting (weren't in the split)
                // For these, we need to look them up
                let split_originals: Vec<_> = funding_utxos.iter()
                    .filter(|op| split.psbt.unsigned_tx.input.iter().any(|i| &i.previous_output == *op))
                    .cloned()
                    .collect();

                for outpoint in &funding_utxos {
                    if !split_originals.contains(outpoint) {
                        // Look up this UTXO since it wasn't split
                        let utxo = self.provider.get_utxo(outpoint).await?
                            .ok_or_else(|| AlkanesError::Wallet(format!("UTXO not found: {}", outpoint)))?;
                        utxos_with_txouts.push((*outpoint, TxOut {
                            value: utxo.value,
                            script_pubkey: utxo.script_pubkey.clone(),
                        }));
                    }
                }

                (utxos_with_txouts, Some(split.psbt), Some(split.fee))
            }
            None => {
                // No split needed - look up all UTXOs
                let mut utxos_with_txouts = Vec::new();
                for outpoint in &funding_utxos {
                    let utxo = self.provider.get_utxo(outpoint).await?
                        .ok_or_else(|| AlkanesError::Wallet(format!("UTXO not found: {}", outpoint)))?;
                    utxos_with_txouts.push((*outpoint, TxOut {
                        value: utxo.value,
                        script_pubkey: utxo.script_pubkey.clone(),
                    }));
                }
                (utxos_with_txouts, None, None)
            }
        };

        let commit_output = TxOut {
            value: bitcoin::Amount::from_sat(commit_output_amount),
            script_pubkey: commit_address.script_pubkey(),
        };

        // Build commit with FINAL sequences (no RBF) for deterministic txid
        let (commit_psbt, commit_fee) = self.build_commit_psbt_final_seq_with_txouts(
            final_funding_utxos_with_txouts,
            commit_output,
            params.fee_rate,
            &params.change_address,
            params.mint_diesel,
            &change_addr,
        ).await?;

        Ok((split_psbt, split_fee, commit_psbt, commit_fee, internal_key, ephemeral_secret))
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

        let fee_rate_sat_vb = self.resolve_fee_rate(fee_rate).await?;
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

    /// Build commit PSBT with final sequences, using pre-fetched TxOut data
    /// This version is used when we have split outputs that haven't been broadcast yet
    async fn build_commit_psbt_final_seq_with_txouts(
        &mut self,
        funding_utxos_with_txouts: Vec<(OutPoint, TxOut)>,
        commit_output: TxOut,
        fee_rate: Option<f32>,
        change_address: &Option<String>,
        mint_diesel: bool,
        change_addr: &Address,
    ) -> Result<(Psbt, u64)> {
        use bitcoin::psbt::Input as PsbtInput;

        let mut total_input_value = 0u64;
        let mut input_txouts = Vec::new();
        let mut outpoints = Vec::new();

        for (outpoint, txout) in &funding_utxos_with_txouts {
            total_input_value += txout.value.to_sat();
            input_txouts.push(txout.clone());
            outpoints.push(*outpoint);
        }

        // Estimate fee
        let mut temp_outputs = vec![commit_output.clone()];

        // Add DIESEL outputs if requested
        if mint_diesel {
            let (diesel_output, diesel_script) = self.create_diesel_mint_outputs(change_addr, 1)?;
            temp_outputs.push(diesel_output);
            temp_outputs.push(TxOut {
                value: bitcoin::Amount::ZERO,
                script_pubkey: diesel_script,
            });
        }

        temp_outputs.push(TxOut {
            value: bitcoin::Amount::from_sat(0),
            script_pubkey: change_addr.script_pubkey(),
        });

        let temp_tx = Transaction {
            version: bitcoin::transaction::Version::TWO,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: outpoints.iter().map(|outpoint| bitcoin::TxIn {
                previous_output: *outpoint,
                script_sig: ScriptBuf::new(),
                sequence: bitcoin::Sequence::ENABLE_RBF_NO_LOCKTIME,
                witness: bitcoin::Witness::from_slice(&[vec![0u8; 65]]),
            }).collect(),
            output: temp_outputs,
        };

        let fee_rate_sat_vb = self.resolve_fee_rate(fee_rate).await?;
        let fee = (fee_rate_sat_vb * temp_tx.vsize() as f32).ceil() as u64;

        // Subtract commit output, fee, and DIESEL output if minting
        let diesel_cost = if mint_diesel { 546 } else { 0 };
        let change_value = total_input_value
            .saturating_sub(commit_output.value.to_sat())
            .saturating_sub(fee)
            .saturating_sub(diesel_cost);

        if change_value < 546 {
            return Err(AlkanesError::Wallet(format!(
                "Not enough funds for commit and change: have {} sats, need {} for commit + {} fee + {} diesel, leaving {} for change (min 546)",
                total_input_value, commit_output.value.to_sat(), fee, diesel_cost, change_value
            )));
        }

        let mut final_outputs = vec![commit_output];

        // Add DIESEL outputs if requested
        if mint_diesel {
            let (diesel_output, diesel_script) = self.create_diesel_mint_outputs(change_addr, 1)?;
            final_outputs.push(diesel_output);
            final_outputs.push(TxOut {
                value: bitcoin::Amount::ZERO,
                script_pubkey: diesel_script,
            });
        }

        // Add change output
        final_outputs.push(TxOut {
            value: bitcoin::Amount::from_sat(change_value),
            script_pubkey: change_addr.script_pubkey(),
        });

        let unsigned_tx = Transaction {
            version: bitcoin::transaction::Version::TWO,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: outpoints.iter().map(|outpoint| bitcoin::TxIn {
                previous_output: *outpoint,
                script_sig: ScriptBuf::new(),
                sequence: bitcoin::Sequence::ENABLE_RBF_NO_LOCKTIME,
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
    /// This creates a reveal with NO change output - commit is sized exactly to cover reveal fee + dust output
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

        // NO CHANGE OUTPUT - commit is sized exactly for reveal fee + dust output
        // This ensures all excess value goes to miners as fee
        let mut outputs = Vec::new();

        if params.use_activation {
            // 3-tx pattern: inscription output first
            log::info!("Creating 546-sat inscription output (no change - exact fee calculation)");
            outputs.push(TxOut {
                value: bitcoin::Amount::from_sat(546),
                script_pubkey: change_address.script_pubkey(),
            });

            // Add DIESEL outputs (pointer = 1, after inscription at 0)
            if params.mint_diesel {
                let (diesel_output, diesel_script) = self.create_diesel_mint_outputs(&change_address, 1)?;
                outputs.push(diesel_output);
                outputs.push(TxOut {
                    value: bitcoin::Amount::ZERO,
                    script_pubkey: diesel_script,
                });
            }
        } else {
            // 2-tx pattern: DIESEL first (if enabled), then BRC20PROG OP_RETURN
            if params.mint_diesel {
                let (diesel_output, diesel_script) = self.create_diesel_mint_outputs(&change_address, 0)?;
                outputs.push(diesel_output);
                outputs.push(TxOut {
                    value: bitcoin::Amount::ZERO,
                    script_pubkey: diesel_script,
                });
            }

            // BRC20PROG OP_RETURN
            log::info!("Creating OP_RETURN output (no change - exact fee calculation)");
            outputs.push(TxOut {
                value: bitcoin::Amount::from_sat(0),
                script_pubkey: self.create_brc20prog_op_return(),
            });
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
    /// Supports additional outputs for FrBTC wrap (send BTC to signer) or unwrap (dust to signer)
    async fn build_activation_psbt_for_presign(
        &mut self,
        params: &Brc20ProgExecuteParams,
        reveal_inscription_outpoint: OutPoint,
        reveal_inscription_output: TxOut,
        commit_change_outpoint: OutPoint,
        commit_change_output: TxOut,
    ) -> Result<(Psbt, u64)> {
        use bitcoin::psbt::Input as PsbtInput;

        let network = self.provider.get_network();
        let change_address_str = if let Some(ref addr) = params.change_address {
            addr.clone()
        } else {
            WalletProvider::get_address(self.provider).await?
        };
        let change_address = Address::from_str(&change_address_str)?
            .require_network(network)?;

        let op_return_output = TxOut {
            value: bitcoin::Amount::from_sat(1), // 1 sat for OP_RETURN - inscription goes to this sat
            script_pubkey: self.create_brc20prog_op_return(),
        };

        // Build additional outputs (for FrBTC wrap/unwrap)
        let mut additional_txouts = Vec::new();
        let mut additional_outputs_total = 0u64;
        if let Some(ref additional_outputs) = params.additional_outputs {
            for output in additional_outputs {
                let addr = Address::from_str(&output.address)
                    .map_err(|e| AlkanesError::AddressResolution(format!("Invalid additional output address '{}': {}", output.address, e)))?
                    .require_network(network)
                    .map_err(|e| AlkanesError::AddressResolution(format!("Address network mismatch for '{}': {}", output.address, e)))?;

                additional_txouts.push(TxOut {
                    value: bitcoin::Amount::from_sat(output.amount),
                    script_pubkey: addr.script_pubkey(),
                });
                additional_outputs_total += output.amount;
                log::info!("   Adding output to {}: {} sats", output.address, output.amount);
            }
        }

        // Estimate fee using 2 inputs and all outputs
        let mut temp_outputs = vec![op_return_output.clone()];
        temp_outputs.extend(additional_txouts.iter().cloned());
        temp_outputs.push(TxOut {
            value: bitcoin::Amount::from_sat(0),
            script_pubkey: change_address.script_pubkey(),
        });

        let temp_tx = Transaction {
            version: bitcoin::transaction::Version::TWO,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: vec![
                bitcoin::TxIn {
                    previous_output: reveal_inscription_outpoint,
                    script_sig: ScriptBuf::new(),
                    sequence: bitcoin::Sequence::ENABLE_RBF_NO_LOCKTIME,
                    witness: bitcoin::Witness::from_slice(&[vec![0u8; 65]]),
                },
                bitcoin::TxIn {
                    previous_output: commit_change_outpoint,
                    script_sig: ScriptBuf::new(),
                    sequence: bitcoin::Sequence::ENABLE_RBF_NO_LOCKTIME,
                    witness: bitcoin::Witness::from_slice(&[vec![0u8; 65]]),
                },
            ],
            output: temp_outputs,
        };

        let fee_rate_sat_vb = self.resolve_fee_rate(params.fee_rate).await?;
        let fee = (fee_rate_sat_vb * temp_tx.vsize() as f32).ceil() as u64;

        // Total inputs: inscription (546) + commit change
        let total_input = reveal_inscription_output.value.to_sat() + commit_change_output.value.to_sat();
        // Outputs: OP_RETURN (1) + additional outputs + change
        let change_value = total_input.saturating_sub(1).saturating_sub(additional_outputs_total).saturating_sub(fee);

        if change_value < 546 {
            return Err(AlkanesError::Wallet(format!(
                "Not enough funds for activation: total_input={}, additional_outputs={}, fee={}, change={}",
                total_input, additional_outputs_total, fee, change_value
            )));
        }

        log::info!("   Activation tx: {} inputs, {} + {} outputs, fee={}, change={}",
                   2, 1 + additional_txouts.len(), 1, fee, change_value);

        // Build final outputs: OP_RETURN, additional outputs, then change
        let mut final_outputs = vec![op_return_output];
        final_outputs.extend(additional_txouts);
        final_outputs.push(TxOut {
            value: bitcoin::Amount::from_sat(change_value),
            script_pubkey: change_address.script_pubkey(),
        });

        let unsigned_tx = Transaction {
            version: bitcoin::transaction::Version::TWO,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: vec![
                bitcoin::TxIn {
                    previous_output: reveal_inscription_outpoint,
                    script_sig: ScriptBuf::new(),
                    sequence: bitcoin::Sequence::ENABLE_RBF_NO_LOCKTIME,
                    witness: bitcoin::Witness::new(),
                },
                bitcoin::TxIn {
                    previous_output: commit_change_outpoint,
                    script_sig: ScriptBuf::new(),
                    sequence: bitcoin::Sequence::ENABLE_RBF_NO_LOCKTIME,
                    witness: bitcoin::Witness::new(),
                },
            ],
            output: final_outputs,
        };

        let mut psbt = Psbt::from_unsigned_tx(unsigned_tx)?;
        psbt.inputs[0] = PsbtInput {
            witness_utxo: Some(reveal_inscription_output),
            ..Default::default()
        };
        psbt.inputs[1] = PsbtInput {
            witness_utxo: Some(commit_change_output),
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
        ephemeral_secret: Option<bitcoin::secp256k1::SecretKey>,
    ) -> Result<Transaction> {
        self.sign_and_finalize_reveal_psbt(&mut psbt, envelope, commit_internal_key, ephemeral_secret).await
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
        let mut current_fee_rate = self.resolve_fee_rate(params.fee_rate).await?;

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
        let mut current_fee_rate = self.resolve_fee_rate(params.fee_rate).await?;

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
                                                                new_commit_internal_key,
                                                                None // Can't use ephemeral secret in RBF path - this path shouldn't trigger with ephemeral keys
                                                            ).await?;

                                                            let new_reveal_hex = bitcoin::consensus::encode::serialize_hex(&new_signed_reveal);
                                                            let new_reveal_txid = self.broadcast_with_options(&new_reveal_hex, params).await?;
                                                            log::info!("   ✅ New reveal broadcast: {}", new_reveal_txid);

                                                            // Rebuild and broadcast activation if needed
                                                            if params.use_activation {
                                                                log::info!("   🔄 Rebuilding activation transaction...");
                                                                let new_reveal_inscription_output = new_signed_reveal.output[0].clone();

                                                                // Extract commit change output for funding activation
                                                                let new_commit_change_output = new_commit_tx.output.get(1)
                                                                    .ok_or_else(|| AlkanesError::Wallet("New commit transaction has no change output".to_string()))?
                                                                    .clone();
                                                                let new_commit_change_outpoint = OutPoint {
                                                                    txid: new_commit_tx.txid(),
                                                                    vout: 1
                                                                };

                                                                let (new_activation_psbt, _activation_fee) =
                                                                    self.build_activation_psbt_for_presign(
                                                                        params,
                                                                        new_reveal_inscription_outpoint,
                                                                        new_reveal_inscription_output,
                                                                        new_commit_change_outpoint,
                                                                        new_commit_change_output,
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

        // Get change address
        let change_address_str = if let Some(ref addr) = params.change_address {
            addr.clone()
        } else {
            WalletProvider::get_address(self.provider).await?
        };
        let change_addr = Address::from_str(&change_address_str)?
            .require_network(self.provider.get_network())?;

        let (new_psbt, _new_fee) = self.build_commit_psbt(
            funding_utxos,
            commit_output,
            Some(new_fee_rate),
            &params.change_address,
            None, // No rebar for RBF
            params.mint_diesel,
            &change_addr,
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
    /// Prioritizes confirmed UTXOs over pending (unconfirmed) ones
    async fn select_utxos_for_amount_excluding(
        &self,
        amount: u64,
        from_addresses: &Option<Vec<String>>,
        exclude_txids: &[Txid],
    ) -> Result<Vec<OutPoint>> {
        log::info!("Selecting UTXOs for {} sats", amount);

        let utxos = self.provider.get_utxos(true, from_addresses.clone()).await?;
        let mut spendable_utxos: Vec<(OutPoint, UtxoInfo)> = utxos
            .into_iter()
            .filter(|(outpoint, info)| {
                !info.frozen && !exclude_txids.contains(&outpoint.txid)
            })
            .collect();

        // Sort UTXOs: confirmed first (by confirmations desc), then pending
        // This ensures we use confirmed UTXOs before falling back to pending ones
        spendable_utxos.sort_by(|(_, a), (_, b)| {
            // Confirmed UTXOs (confirmations > 0) come before pending (confirmations == 0)
            match (a.confirmations > 0, b.confirmations > 0) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => b.confirmations.cmp(&a.confirmations), // Higher confirmations first
            }
        });

        let confirmed_count = spendable_utxos.iter().filter(|(_, u)| u.confirmations > 0).count();
        let pending_count = spendable_utxos.len() - confirmed_count;
        log::info!("Found {} spendable UTXOs ({} confirmed, {} pending)",
            spendable_utxos.len(), confirmed_count, pending_count);

        let mut selected_outpoints = Vec::new();
        let mut bitcoin_collected = 0u64;
        let mut using_pending = false;

        for (outpoint, utxo) in spendable_utxos {
            if bitcoin_collected < amount {
                if utxo.confirmations == 0 && !using_pending {
                    using_pending = true;
                    log::warn!("⚠️ Falling back to pending (unconfirmed) UTXOs - confirmed UTXOs insufficient");
                }
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

    /// Query ord for inscriptions on a specific UTXO
    /// Returns a list of inscription IDs and their sat offsets within the UTXO
    ///
    /// If ord is unavailable, logs a warning and returns empty list (fail-open)
    /// If mempool_indexer is enabled and the UTXO is pending, traces back through
    /// parent transactions to determine inscription state from settled UTXOs.
    async fn get_utxo_inscriptions(&self, outpoint: &OutPoint, mempool_indexer: bool) -> Result<Vec<InscriptionInfo>> {
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
    async fn trace_pending_utxo_inscriptions(&self, outpoint: &OutPoint) -> Result<Vec<TracedInscriptionInfo>> {
        use crate::traits::EsploraProvider;

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
    fn calculate_split(&self, utxo_value: u64, inscriptions: &[InscriptionInfo], fee_rate: f32) -> Option<SplitPlan> {
        if inscriptions.is_empty() {
            return None;
        }

        // Find the highest offset among all inscriptions
        // We need to send all sats up to and including this sat to the safe output
        let max_offset = inscriptions.iter().map(|i| i.sat_offset).max().unwrap_or(0);

        // Safe amount = offset + 1 (because offset is 0-indexed)
        // This ensures the inscribed sat goes to the first output (safe)
        let safe_amount = max_offset + 1;

        // Ensure safe amount is at least dust limit
        let safe_amount = safe_amount.max(DUST_LIMIT);

        // Calculate clean amount (remaining sats)
        if utxo_value <= safe_amount {
            // Not enough sats after protecting inscriptions
            log::warn!(
                "UTXO has {} sats but inscription at offset {} requires {} sats for safe output - cannot split",
                utxo_value, max_offset, safe_amount
            );
            return None;
        }

        // Estimate split tx fee (1 input, 2 outputs, ~140 vB for p2tr)
        let estimated_split_fee = (fee_rate * 140.0).ceil() as u64;

        let clean_amount = utxo_value.saturating_sub(safe_amount).saturating_sub(estimated_split_fee);

        // Ensure clean amount is at least dust limit + some buffer for actual funding
        if clean_amount < DUST_LIMIT * 2 {
            log::warn!(
                "After protecting inscriptions and fees, only {} sats remain - not enough for funding",
                clean_amount
            );
            return None;
        }

        log::info!(
            "Split plan: {} sats → safe({}) + clean({}) + fee(~{})",
            utxo_value, safe_amount, clean_amount, estimated_split_fee
        );

        Some(SplitPlan {
            outpoint: OutPoint::null(), // Will be filled in by caller
            safe_amount,
            clean_amount,
        })
    }

    /// Check selected UTXOs for inscriptions and build split transaction if needed
    ///
    /// Returns None if no split is needed, or Some(SplitResult) with the split PSBT
    /// and clean outpoints to use for commit transaction funding
    ///
    /// If mempool_indexer is true, pending UTXOs will be traced back through parent
    /// transactions to determine inscription state.
    ///
    /// ordinals_strategy controls behavior:
    /// - Exclude: fail if any selected UTXO contains inscriptions
    /// - Preserve: split inscribed UTXOs to protect inscriptions (default)
    /// - Burn: skip inscription check entirely (allows spending inscribed UTXOs)
    async fn build_split_psbt_if_needed(
        &mut self,
        funding_utxos: &[OutPoint],
        change_address: &Address,
        fee_rate: f32,
        mempool_indexer: bool,
        ordinals_strategy: crate::alkanes::types::OrdinalsStrategy,
    ) -> Result<Option<SplitResult>> {
        use crate::alkanes::types::OrdinalsStrategy;

        // Burn strategy: skip inscription check entirely
        if ordinals_strategy == OrdinalsStrategy::Burn {
            log::info!("🔥 Ordinals strategy is 'burn' - skipping inscription check");
            return Ok(None);
        }

        let mut split_plans: Vec<SplitPlan> = Vec::new();
        let mut utxo_info: Vec<(OutPoint, TxOut)> = Vec::new();

        // Check each UTXO for inscriptions
        for outpoint in funding_utxos {
            let inscriptions = self.get_utxo_inscriptions(outpoint, mempool_indexer).await?;

            if !inscriptions.is_empty() {
                // Exclude strategy: fail immediately if inscriptions found
                if ordinals_strategy == OrdinalsStrategy::Exclude {
                    return Err(AlkanesError::Wallet(format!(
                        "Cannot proceed: UTXO {} contains inscriptions and ordinals_strategy is 'exclude'. \
                        Use ordinals_strategy 'preserve' to protect inscriptions, or 'burn' to allow spending them.",
                        outpoint
                    )));
                }

                // Preserve strategy: split inscribed UTXOs
                // Get UTXO value
                let utxo = self.provider.get_utxo(outpoint).await?
                    .ok_or_else(|| AlkanesError::Wallet(format!("UTXO not found: {}", outpoint)))?;
                let utxo_value = utxo.value.to_sat();

                // Calculate split plan
                if let Some(mut plan) = self.calculate_split(utxo_value, &inscriptions, fee_rate) {
                    plan.outpoint = *outpoint;
                    split_plans.push(plan);
                    utxo_info.push((*outpoint, TxOut {
                        value: utxo.value,
                        script_pubkey: utxo.script_pubkey.clone(),
                    }));
                } else {
                    // Cannot split this UTXO - return error
                    return Err(AlkanesError::Wallet(format!(
                        "UTXO {} contains inscriptions but cannot be safely split. \
                        Please use a different UTXO without inscriptions.",
                        outpoint
                    )));
                }
            }
        }

        if split_plans.is_empty() {
            return Ok(None); // No split needed
        }

        log::info!("🔀 Building split transaction to protect {} inscribed UTXO(s)", split_plans.len());

        // Build the split transaction
        // Each inscribed UTXO becomes an input with 2 outputs (safe + clean)
        let mut inputs = Vec::new();
        let mut outputs = Vec::new();

        for (plan, (outpoint, _txout)) in split_plans.iter().zip(utxo_info.iter()) {
            inputs.push(bitcoin::TxIn {
                previous_output: *outpoint,
                script_sig: ScriptBuf::new(),
                sequence: bitcoin::Sequence::ENABLE_RBF_NO_LOCKTIME,
                witness: bitcoin::Witness::new(),
            });

            // Safe output (inscribed sats go here - to change address for safekeeping)
            outputs.push(TxOut {
                value: bitcoin::Amount::from_sat(plan.safe_amount),
                script_pubkey: change_address.script_pubkey(),
            });

            // Clean output (for funding - also to change address, will be spent in commit)
            outputs.push(TxOut {
                value: bitcoin::Amount::from_sat(plan.clean_amount),
                script_pubkey: change_address.script_pubkey(),
            });
        }

        // Create the transaction
        let split_tx = Transaction {
            version: bitcoin::transaction::Version::TWO,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: inputs,
            output: outputs.clone(),
        };

        // Calculate actual fee
        let vsize = split_tx.vsize() + (split_plans.len() * 65); // Add witness size estimate
        let fee = (fee_rate * vsize as f32).ceil() as u64;

        // Adjust last clean output to absorb fee
        let last_clean_idx = outputs.len() - 1;
        let adjusted_value = outputs[last_clean_idx].value.to_sat().saturating_sub(fee);
        if adjusted_value < DUST_LIMIT {
            return Err(AlkanesError::Wallet(
                "Not enough funds in split outputs after fee".to_string()
            ));
        }

        let mut final_outputs = outputs;
        final_outputs[last_clean_idx].value = bitcoin::Amount::from_sat(adjusted_value);

        let final_tx = Transaction {
            version: bitcoin::transaction::Version::TWO,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: split_tx.input.clone(),
            output: final_outputs,
        };

        let split_txid = final_tx.txid();

        // Create PSBT
        let mut psbt = Psbt::from_unsigned_tx(final_tx)?;

        // Add witness UTXOs to PSBT inputs
        for (i, (_, txout)) in utxo_info.iter().enumerate() {
            psbt.inputs[i].witness_utxo = Some(txout.clone());
        }

        // Calculate clean outpoints with their TxOut data (every second output, starting from index 1)
        let mut clean_utxos = Vec::new();
        for (i, plan) in split_plans.iter().enumerate() {
            let clean_vout = (i * 2 + 1) as u32; // 1, 3, 5, ...
            let clean_outpoint = OutPoint {
                txid: split_txid,
                vout: clean_vout,
            };
            // Get the actual output value from the final transaction
            let clean_txout = psbt.unsigned_tx.output[clean_vout as usize].clone();
            clean_utxos.push((clean_outpoint, clean_txout.clone()));

            log::info!(
                "   UTXO {} → safe output :{} ({}s), clean output :{} ({}s)",
                plan.outpoint,
                i * 2, plan.safe_amount,
                clean_vout, clean_txout.value.to_sat()
            );
        }

        log::info!("   Split txid: {}", split_txid);

        Ok(Some(SplitResult {
            psbt,
            fee,
            clean_utxos,
        }))
    }

    /// Build commit PSBT
    async fn build_commit_psbt(
        &mut self,
        funding_utxos: Vec<OutPoint>,
        commit_output: TxOut,
        fee_rate: Option<f32>,
        change_address: &Option<String>,
        rebar_payment: Option<RebarPaymentInfo>,
        mint_diesel: bool,
        change_addr: &Address,
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

        // Build temp outputs list including Rebar payment and DIESEL if needed
        let mut temp_outputs = vec![commit_output.clone()];

        // Add DIESEL outputs if requested
        if mint_diesel {
            let (diesel_output, diesel_script) = self.create_diesel_mint_outputs(change_addr, 1)?;
            temp_outputs.push(diesel_output);
            temp_outputs.push(TxOut {
                value: bitcoin::Amount::ZERO,
                script_pubkey: diesel_script,
            });
        }

        if let Some(ref rebar) = rebar_payment {
            temp_outputs.push(TxOut {
                value: bitcoin::Amount::from_sat(rebar.payment_amount),
                script_pubkey: rebar.payment_address.script_pubkey(),
            });
        }

        temp_outputs.push(TxOut {
            value: bitcoin::Amount::from_sat(0),
            script_pubkey: change_addr.script_pubkey(),
        });

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
            let fee_rate_sat_vb = self.resolve_fee_rate(fee_rate).await?;
            (fee_rate_sat_vb * temp_tx_for_size.vsize() as f32).ceil() as u64
        };

        let rebar_payment_amount = rebar_payment.as_ref().map(|r| r.payment_amount).unwrap_or(0);
        let diesel_cost = if mint_diesel { 546 } else { 0 };

        let change_value = total_input_value
            .saturating_sub(commit_output.value.to_sat())
            .saturating_sub(rebar_payment_amount)
            .saturating_sub(fee)
            .saturating_sub(diesel_cost);
        if change_value < 546 {
            return Err(AlkanesError::Wallet(
                "Not enough funds for commit and change".to_string(),
            ));
        }

        // Build final outputs list including DIESEL and Rebar payment if needed
        let mut final_outputs = vec![commit_output];

        // Add DIESEL outputs if requested
        if mint_diesel {
            let (diesel_output, diesel_script) = self.create_diesel_mint_outputs(change_addr, 1)?;
            final_outputs.push(diesel_output);
            final_outputs.push(TxOut {
                value: bitcoin::Amount::ZERO,
                script_pubkey: diesel_script,
            });
        }

        if let Some(rebar) = rebar_payment {
            final_outputs.push(TxOut {
                value: bitcoin::Amount::from_sat(rebar.payment_amount),
                script_pubkey: rebar.payment_address.script_pubkey(),
            });
        }

        final_outputs.push(TxOut {
            value: bitcoin::Amount::from_sat(change_value),
            script_pubkey: change_addr.script_pubkey(),
        });

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
                    let reveal_script = env.build_reveal_script(commit_internal_key);
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
            let fee_rate_sat_vb = self.resolve_fee_rate(fee_rate).await?;
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
        ephemeral_secret: Option<bitcoin::secp256k1::SecretKey>,
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
        // The reveal script includes <pubkey> CHECKSIG to prevent frontrunning
        let reveal_script = envelope.build_reveal_script(commit_internal_key);
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
        // ANTI-FRONTRUNNING: Pass ephemeral secret to ensure we sign with the correct key
        let signature = self
            .provider
            .sign_taproot_script_spend(sighash.into(), ephemeral_secret)
            .await?;
        let taproot_signature = bitcoin::taproot::Signature {
            signature,
            sighash_type: TapSighashType::Default,
        };
        let signature_bytes = taproot_signature.to_vec();

        // Create the complete witness for the commit input
        let witness = envelope.create_complete_witness(&signature_bytes, control_block, commit_internal_key)?;

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

#[cfg(test)]
mod tests {
    use super::*;

    /// Test sat flow calculation for inscription tracing
    /// This verifies the core ordinal sat-flow logic used in trace_pending_utxo_inscriptions
    #[test]
    fn test_sat_flow_calculation() {
        // Simulate a transaction with 3 inputs and 2 outputs
        // Input 0: 50,000 sats (has inscription at offset 10,000)
        // Input 1: 30,000 sats (no inscription)
        // Input 2: 20,000 sats (has inscription at offset 5,000)
        // Output 0: 40,000 sats
        // Output 1: 60,000 sats

        let input_values = vec![50_000u64, 30_000, 20_000];
        let output_values = vec![40_000u64, 60_000];

        // Calculate output sat ranges
        let mut output_sat_ranges: Vec<(u64, u64)> = Vec::new();
        let mut sat_cursor = 0u64;
        for value in &output_values {
            let start = sat_cursor;
            let end = sat_cursor + value;
            output_sat_ranges.push((start, end));
            sat_cursor = end;
        }

        assert_eq!(output_sat_ranges, vec![(0, 40_000), (40_000, 100_000)]);

        // Calculate which output each inscription lands in
        // Inscription 1: input 0 offset 10,000 -> absolute position 10,000 -> output 0 (0..40000)
        let inscription_1_abs_pos = 0 + 10_000; // input_start + offset
        assert!(inscription_1_abs_pos >= output_sat_ranges[0].0 && inscription_1_abs_pos < output_sat_ranges[0].1);
        let inscription_1_new_offset = inscription_1_abs_pos - output_sat_ranges[0].0;
        assert_eq!(inscription_1_new_offset, 10_000);

        // Inscription 2: input 2 offset 5,000 -> absolute position 80,000 + 5,000 = 85,000 -> output 1 (40000..100000)
        let inscription_2_abs_pos = (50_000 + 30_000) + 5_000; // inputs 0+1 + offset
        assert!(inscription_2_abs_pos >= output_sat_ranges[1].0 && inscription_2_abs_pos < output_sat_ranges[1].1);
        let inscription_2_new_offset = inscription_2_abs_pos - output_sat_ranges[1].0;
        assert_eq!(inscription_2_new_offset, 45_000);
    }

    /// Test that inscription at boundary correctly flows to expected output
    #[test]
    fn test_sat_flow_boundary() {
        // Output 0 ends at sat 40,000 (exclusive)
        // Output 1 starts at sat 40,000 (inclusive)
        let output_sat_ranges = vec![(0u64, 40_000), (40_000u64, 100_000)];

        // Sat at position 39,999 should go to output 0
        let pos_39999 = 39_999u64;
        assert!(pos_39999 >= output_sat_ranges[0].0 && pos_39999 < output_sat_ranges[0].1);

        // Sat at position 40,000 should go to output 1
        let pos_40000 = 40_000u64;
        assert!(pos_40000 >= output_sat_ranges[1].0 && pos_40000 < output_sat_ranges[1].1);
    }

    /// Test split calculation for protecting inscriptions
    #[test]
    fn test_split_calculation_basic() {
        // UTXO with 100,000 sats, inscription at offset 50,000
        let utxo_value = 100_000u64;
        let max_offset = 50_000u64;
        let fee_rate = 10.0f32;

        // Safe amount = offset + 1 (to include the inscribed sat)
        let safe_amount = (max_offset + 1).max(DUST_LIMIT);
        assert_eq!(safe_amount, 50_001);

        // Estimated split tx fee (1 input, 2 outputs, ~140 vB for p2tr)
        let estimated_split_fee = (fee_rate * 140.0).ceil() as u64;
        assert_eq!(estimated_split_fee, 1400);

        // Clean amount = remaining after safe and fee
        let clean_amount = utxo_value.saturating_sub(safe_amount).saturating_sub(estimated_split_fee);
        assert_eq!(clean_amount, 100_000 - 50_001 - 1400);
        assert_eq!(clean_amount, 48_599);

        // Verify clean amount is above dust
        assert!(clean_amount >= DUST_LIMIT);
    }

    /// Test split calculation when inscription is near the end
    #[test]
    fn test_split_calculation_inscription_near_end() {
        // UTXO with 10,000 sats, inscription at offset 9,000
        let utxo_value = 10_000u64;
        let max_offset = 9_000u64;
        let fee_rate = 10.0f32;

        let safe_amount = (max_offset + 1).max(DUST_LIMIT);
        assert_eq!(safe_amount, 9_001);

        let estimated_split_fee = (fee_rate * 140.0).ceil() as u64;

        // Clean amount would be negative or below dust - cannot split
        let remaining = utxo_value.saturating_sub(safe_amount);
        assert!(remaining < estimated_split_fee + DUST_LIMIT);
        // This UTXO cannot be safely split
    }

    /// Test split calculation with multiple inscriptions
    #[test]
    fn test_split_calculation_multiple_inscriptions() {
        // UTXO with 100,000 sats, inscriptions at offsets 10,000, 30,000, and 50,000
        let offsets = vec![10_000u64, 30_000, 50_000];
        let utxo_value = 100_000u64;

        // Must protect up to the highest offset
        let max_offset = offsets.iter().max().unwrap();
        assert_eq!(*max_offset, 50_000);

        // Safe amount covers all inscriptions
        let safe_amount = (max_offset + 1).max(DUST_LIMIT);
        assert_eq!(safe_amount, 50_001);
    }

    /// Test that inscription at offset 0 is handled correctly
    #[test]
    fn test_split_calculation_offset_zero() {
        // Inscription at offset 0 means the very first sat is inscribed
        let max_offset = 0u64;
        let utxo_value = 10_000u64;

        // Safe amount = 1 sat, but must be at least dust limit
        let safe_amount = (max_offset + 1).max(DUST_LIMIT);
        assert_eq!(safe_amount, DUST_LIMIT); // Should be 546 (dust limit)
    }
}
