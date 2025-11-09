// BRC20-Prog executor for contract deployment and interaction
// This module handles the commit-reveal transaction pattern for BRC20-prog inscriptions

use crate::{AlkanesError, DeezelProvider, Result};
use crate::traits::{WalletProvider, UtxoInfo};
use bitcoin::{Transaction, ScriptBuf, OutPoint, TxOut, Address, XOnlyPublicKey, psbt::Psbt, Txid};
use bitcoin::blockdata::script::Builder as ScriptBuilder;
use core::str::FromStr;
#[cfg(not(feature = "std"))]
use alloc::{vec, vec::Vec, string::{String, ToString}, format};
#[cfg(feature = "std")]
use std::{vec, vec::Vec, string::{String, ToString}, format};

use super::envelope::Brc20ProgEnvelope;
use super::types::{Brc20ProgExecuteParams, Brc20ProgExecuteResult};

const MAX_FEE_SATS: u64 = 100_000;
const DUST_LIMIT: u64 = 546;

/// BRC20-Prog executor for contract operations
pub struct Brc20ProgExecutor<'a> {
    pub provider: &'a mut dyn DeezelProvider,
}

impl<'a> Brc20ProgExecutor<'a> {
    /// Create a new BRC20-prog executor
    pub fn new(provider: &'a mut dyn DeezelProvider) -> Self {
        Self { provider }
    }

    /// Execute a BRC20-prog operation (deploy or call) using commit-reveal pattern
    pub async fn execute(&mut self, params: Brc20ProgExecuteParams) -> Result<Brc20ProgExecuteResult> {
        log::info!("Starting BRC20-prog execution");
        log::info!("Inscription content: {}", params.inscription_content);

        // Create the envelope with the JSON payload
        let envelope = Brc20ProgEnvelope::new(params.inscription_content.as_bytes().to_vec());

        // Build and execute commit transaction
        let (commit_txid, commit_fee, commit_outpoint, required_reveal_amount, internal_key) =
            self.build_and_broadcast_commit(&params, &envelope).await?;

        log::info!("✅ Commit transaction broadcast: {commit_txid}");

        // Mine a block if on regtest
        if params.mine_enabled {
            self.mine_blocks_if_regtest(&params).await?;
            self.provider.sync().await?;
        }

        // Build and execute reveal transaction
        let (reveal_txid, reveal_fee) = self
            .build_and_broadcast_reveal(
                &params,
                &envelope,
                commit_outpoint,
                required_reveal_amount,
                internal_key,
            )
            .await?;

        log::info!("✅ Reveal transaction broadcast: {reveal_txid}");

        if params.mine_enabled {
            self.mine_blocks_if_regtest(&params).await?;
            self.provider.sync().await?;
        }

        Ok(Brc20ProgExecuteResult {
            commit_txid: commit_txid.to_string(),
            reveal_txid: reveal_txid.to_string(),
            commit_fee,
            reveal_fee,
            inputs_used: vec![],
            outputs_created: vec![],
            traces: None,
        })
    }

    /// Build and broadcast the commit transaction
    async fn build_and_broadcast_commit(
        &mut self,
        params: &Brc20ProgExecuteParams,
        envelope: &Brc20ProgEnvelope,
    ) -> Result<(Txid, u64, OutPoint, u64, XOnlyPublicKey)> {
        log::info!("Building commit transaction");

        let (internal_key, _) = self.provider.get_internal_key().await?;
        let commit_address = self.create_commit_address_for_envelope(envelope, internal_key).await?;
        log::info!("Commit address: {commit_address}");

        // Calculate required amount for reveal transaction
        // We need enough for: recipient output + fees + padding
        let mut required_reveal_amount = DUST_LIMIT; // For the OP_RETURN output
        let estimated_reveal_fee = 50_000u64; // Generous estimate for reveal tx
        required_reveal_amount += estimated_reveal_fee;

        // Select UTXOs for funding the commit
        let funding_utxos = self.select_utxos_for_amount(
            required_reveal_amount,
            &params.from_addresses,
        ).await?;

        let commit_output = TxOut {
            value: bitcoin::Amount::from_sat(required_reveal_amount),
            script_pubkey: commit_address.script_pubkey(),
        };

        let (commit_psbt, commit_fee) = self
            .build_commit_psbt(
                funding_utxos,
                commit_output,
                params.fee_rate,
                &params.change_address,
            )
            .await?;

        // Sign and broadcast commit transaction
        let commit_tx = self.sign_and_finalize_psbt(commit_psbt).await?;
        let commit_txid_string = self
            .provider
            .broadcast_transaction(bitcoin::consensus::encode::serialize_hex(&commit_tx))
            .await?;

        let commit_outpoint = OutPoint {
            txid: commit_tx.compute_txid(),
            vout: 0,
        };

        Ok((
            commit_tx.compute_txid(),
            commit_fee,
            commit_outpoint,
            required_reveal_amount,
            internal_key,
        ))
    }

    /// Build and broadcast the reveal transaction
    async fn build_and_broadcast_reveal(
        &mut self,
        params: &Brc20ProgExecuteParams,
        envelope: &Brc20ProgEnvelope,
        commit_outpoint: OutPoint,
        commit_output_value: u64,
        commit_internal_key: XOnlyPublicKey,
    ) -> Result<(Txid, u64)> {
        log::info!("Building reveal transaction");

        // Create reveal transaction with OP_RETURN output for BRC20PROG
        let op_return_script = self.create_brc20prog_op_return();
        let reveal_output = TxOut {
            value: bitcoin::Amount::ZERO,
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

        // Calculate change amount (commit output value - reveal fee)
        let estimated_reveal_fee = 50_000u64;
        let change_amount = commit_output_value.saturating_sub(estimated_reveal_fee);

        let change_output = TxOut {
            value: bitcoin::Amount::from_sat(change_amount),
            script_pubkey: change_address.script_pubkey(),
        };

        let outputs = vec![reveal_output, change_output];

        // Build reveal PSBT with script-path spending
        let (mut reveal_psbt, reveal_fee) = self
            .build_reveal_psbt(
                vec![commit_outpoint],
                outputs,
                params.fee_rate,
                Some(envelope),
                commit_internal_key,
            )
            .await?;

        // Sign the reveal transaction with script-path signature
        let reveal_tx = self.sign_and_finalize_reveal_psbt(
            &mut reveal_psbt,
            envelope,
            commit_internal_key,
        ).await?;

        let reveal_txid_string = self
            .provider
            .broadcast_transaction(bitcoin::consensus::encode::serialize_hex(&reveal_tx))
            .await?;

        Ok((reveal_tx.compute_txid(), reveal_fee))
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

    /// Select UTXOs for a specific Bitcoin amount
    async fn select_utxos_for_amount(
        &self,
        amount: u64,
        from_addresses: &Option<Vec<String>>,
    ) -> Result<Vec<OutPoint>> {
        log::info!("Selecting UTXOs for {} sats", amount);

        let utxos = self.provider.get_utxos(true, from_addresses.clone()).await?;
        let spendable_utxos: Vec<(OutPoint, UtxoInfo)> = utxos
            .into_iter()
            .filter(|(_, info)| !info.frozen)
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
        let temp_outputs = vec![commit_output.clone(), temp_change_output];

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
            output: temp_outputs,
        };
        for input in &mut temp_tx_for_size.input {
            input.witness.push([0u8; 65]);
        }

        let fee_rate_sat_vb = fee_rate.unwrap_or(600.0);
        let fee = (fee_rate_sat_vb * temp_tx_for_size.vsize() as f32).ceil() as u64;

        let change_value = total_input_value
            .saturating_sub(commit_output.value.to_sat())
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
        let final_outputs = vec![commit_output, final_change_output];

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
        utxos: Vec<OutPoint>,
        mut outputs: Vec<TxOut>,
        fee_rate: Option<f32>,
        envelope: Option<&Brc20ProgEnvelope>,
        commit_internal_key: XOnlyPublicKey,
    ) -> Result<(Psbt, u64)> {
        use bitcoin::transaction::Version;

        let mut total_input_value = 0;
        let mut input_txouts = Vec::new();
        for outpoint in &utxos {
            let utxo = self
                .provider
                .get_utxo(outpoint)
                .await?
                .ok_or_else(|| AlkanesError::Wallet(format!("UTXO not found: {outpoint}")))?;
            total_input_value += utxo.value.to_sat();
            input_txouts.push(utxo);
        }

        let mut temp_tx = Transaction {
            version: Version::TWO,
            lock_time: bitcoin::absolute::LockTime::ZERO,
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
            if envelope.is_some() && i == 0 {
                // Script-path spend - use larger placeholder
                input.witness.push([0u8; 400]);
            } else {
                // Key-path spend
                input.witness.push([0u8; 65]);
            }
        }

        let fee_rate_sat_vb = fee_rate.unwrap_or(600.0);
        let estimated_fee = (fee_rate_sat_vb * temp_tx.vsize() as f32).ceil() as u64;
        let capped_fee = estimated_fee.min(MAX_FEE_SATS);

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
            .find(|o| o.value.to_sat() > 0 && !o.script_pubkey.is_op_return())
        {
            change_output.value = bitcoin::Amount::from_sat(change_value);
        }

        let mut psbt = Psbt::from_unsigned_tx(Transaction {
            version: Version::TWO,
            lock_time: bitcoin::absolute::LockTime::ZERO,
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

        // Get the unsigned transaction
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

        // Calculate sighash for script-path spending
        let mut sighash_cache = SighashCache::new(&unsigned_tx);
        let leaf_hash = TapLeafHash::from_script(&reveal_script, LeafVersion::TapScript);
        let sighash = sighash_cache
            .taproot_script_spend_signature_hash(0, &prevouts_all, leaf_hash, TapSighashType::Default)
            .map_err(|e| AlkanesError::Transaction(e.to_string()))?;

        // Sign the sighash
        let signature = self
            .provider
            .sign_taproot_script_spend(sighash.into())
            .await?;
        let taproot_signature = bitcoin::taproot::Signature {
            signature,
            sighash_type: TapSighashType::Default,
        };
        let signature_bytes = taproot_signature.to_vec();

        // Create the complete witness
        let witness = envelope.create_complete_witness(&signature_bytes, control_block)?;

        // Create the final transaction
        let mut tx = unsigned_tx.clone();
        tx.input[0].witness = witness;

        // Sign other inputs if needed
        for i in 1..tx.input.len() {
            if let Some(tap_key_sig) = &psbt.inputs[i].tap_key_sig {
                tx.input[i].witness = bitcoin::Witness::p2tr_key_spend(tap_key_sig);
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
}
