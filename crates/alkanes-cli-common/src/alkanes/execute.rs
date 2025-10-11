use crate::{Result, AlkanesError};
use crate::traits::{AlkanesProvider, WalletProvider};
use crate::types::UtxoInfo;
use bitcoin::{Transaction, ScriptBuf, OutPoint, TxOut, Address, XOnlyPublicKey, psbt::Psbt};
use anyhow::Context;
use core::str::FromStr;
#[cfg(not(feature = "std"))]
use alloc::{vec, vec::Vec, string::{String, ToString}, format};
#[cfg(feature = "std")]
use std::{vec, vec::Vec, string::{String, ToString}, format, io::{self, Write}};
use tokio::time::{sleep, Duration};
pub use super::types::{
    EnhancedExecuteParams, EnhancedExecuteResult, ExecutionState, InputRequirement, OutputTarget,
    ProtostoneSpec, ReadyToSignCommitTx, ReadyToSignRevealTx, ReadyToSignTx,
};
use super::envelope::AlkanesEnvelope;
use anyhow::anyhow;
use ordinals::Runestone;
use crate::alkanes::protostone::{Protostone, ProtostoneEdict};

const MAX_FEE_SATS: u64 = 100_000; // 0.001 BTC. Cap to avoid "absurdly high fee rate" errors.
const DUST_LIMIT: u64 = 546;


/// Enhanced alkanes executor
pub struct EnhancedAlkanesExecutor<'a> {
    pub provider: &'a mut dyn AlkanesProvider,
}

impl<'a> EnhancedAlkanesExecutor<'a> {
    /// Create a new enhanced alkanes executor
    pub fn new(provider: &'a mut dyn AlkanesProvider) -> Self {
        Self { provider }
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

    pub async fn resume_execution(
        &mut self,
        state: ReadyToSignTx,
        params: &EnhancedExecuteParams,
    ) -> Result<EnhancedExecuteResult> {
        let unsigned_tx = &state.psbt.unsigned_tx;

        if !params.auto_confirm {
            self.show_preview_and_confirm(
                unsigned_tx,
                &serde_json::to_value(&state.analysis)?,
                state.fee,
                params.raw_output,
            )?;
        }

        let tx = self.sign_and_finalize_psbt(state.psbt).await?;
        let tx_hex = bitcoin::consensus::encode::serialize_hex(&tx);
        let txid = self.provider.broadcast_transaction(tx_hex).await?;

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
        let commit_txid = self
            .provider
            .broadcast_transaction(bitcoin::consensus::encode::serialize_hex(&commit_tx))
            .await?;
        log::info!("✅ Commit transaction broadcast successfully: {commit_txid}");

        // Mine a block to confirm the commit transaction if on regtest
        if state.params.mine_enabled {
            self.mine_blocks_if_regtest(&state.params).await?;
            self.provider.sync().await?;
        }

        // 2. Build the reveal transaction PSBT
        let commit_outpoint = bitcoin::OutPoint { txid: commit_tx.compute_txid(), vout: 0 };
        let (reveal_psbt, reveal_fee) = self
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
        let estimated_reveal_fee = 50_000u64;
        required_reveal_amount += estimated_reveal_fee;
        required_reveal_amount += params.to_addresses.len() as u64 * 546;

        let funding_utxos =
            self
                .select_utxos(&[InputRequirement::Bitcoin { amount: required_reveal_amount }], &params.from_addresses)
                .await?;

        let commit_output = TxOut {
            value: bitcoin::Amount::from_sat(required_reveal_amount),
            script_pubkey: commit_address.script_pubkey(),
        };

        let (commit_psbt, commit_fee) = self
            .build_commit_psbt(funding_utxos, commit_output, params.fee_rate)
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

        self.validate_protostones(&params.protostones, params.to_addresses.len())?;
        let mut outputs = self.create_outputs(&params.to_addresses, &params.change_address, &params.input_requirements).await?;
        for protostone in &params.protostones {
            if let Some(transfer) = &protostone.bitcoin_transfer {
                if let OutputTarget::Output(vout) = transfer.target {
                    if let Some(output) = outputs.get_mut(vout as usize) {
                        output.value = bitcoin::Amount::from_sat(transfer.amount);
                    }
                }
            }
        }
        let total_bitcoin_needed: u64 = outputs.iter().filter(|o| o.value.to_sat() > 0).map(|o| o.value.to_sat()).sum();
        let mut final_requirements = params.input_requirements.iter().filter(|req| !matches!(req, InputRequirement::Bitcoin {..})).cloned().collect::<Vec<_>>();
        if total_bitcoin_needed > 0 {
            final_requirements.push(InputRequirement::Bitcoin { amount: total_bitcoin_needed });
        }
        let selected_utxos = self.select_utxos(&final_requirements, &params.from_addresses).await?;
        let runestone_script = self.construct_runestone_script(&params.protostones, outputs.len())?;
        let (psbt, fee) = self.build_psbt_and_fee(selected_utxos.clone(), outputs, Some(runestone_script), params.fee_rate, None).await?;

        let unsigned_tx = &psbt.unsigned_tx;
        let analysis = crate::transaction::analysis::analyze_transaction(unsigned_tx);
        let inspection_result = self.inspect_from_protostones(&params.protostones).await.ok();

        Ok(ExecutionState::ReadyToSign(ReadyToSignTx {
            psbt,
            analysis,
            fee,
            inspection_result,
        }))
    }

    pub fn validate_protostones(&self, protostones: &[ProtostoneSpec], num_outputs: usize) -> Result<()> {
        log::info!("Validating {} protostones against {} outputs", protostones.len(), num_outputs);
        
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
            
            for edict in &protostone.edicts {
                match edict.target {
                    OutputTarget::Output(v) => {
                        if v as usize >= num_outputs {
                            return Err(AlkanesError::Validation(format!(
                                "Edict in protostone {i} targets output v{v} but only {num_outputs} outputs exist"
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

    async fn select_utxos(&self, requirements: &[InputRequirement], from_addresses: &Option<Vec<String>>) -> Result<Vec<OutPoint>> {
        log::info!("Selecting UTXOs for {} requirements", requirements.len());
        if let Some(addrs) = from_addresses {
            log::info!("Sourcing UTXOs from: {addrs:?}");
        }

        let utxos = self.provider.get_utxos(true, from_addresses.clone()).await?;
        log::debug!("Found {} total wallet UTXOs from specified sources", utxos.len());

        let spendable_utxos: Vec<(OutPoint, UtxoInfo)> = utxos.into_iter()
            .filter(|(_, info)| !info.frozen)
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
                InputRequirement::Alkanes { block, tx, amount } => {
                    let key = (*block, *tx);
                    *alkanes_needed.entry(key).or_insert(0) += amount;
                }
            }
        }

        log::info!("Need {} sats Bitcoin and {} different alkanes tokens", bitcoin_needed, alkanes_needed.len());

        let mut bitcoin_collected = 0u64;

        for (outpoint, utxo) in spendable_utxos {
            if bitcoin_collected < bitcoin_needed {
                bitcoin_collected += utxo.amount;
                selected_outpoints.push(outpoint);
            } else {
                break;
            }
        }

        if bitcoin_collected < bitcoin_needed {
            return Err(AlkanesError::Wallet(format!(
                "Insufficient funds: need {bitcoin_needed} sats, have {bitcoin_collected}"
            )));
        }

        log::info!("Selected {} UTXOs meeting Bitcoin requirements", selected_outpoints.len());
        Ok(selected_outpoints)
    }

    async fn create_outputs(
        &self,
        to_addresses: &[String],
        change_address: &Option<String>,
        input_requirements: &[InputRequirement],
    ) -> Result<Vec<TxOut>> {
        let mut outputs = Vec::new();
        let network = self.provider.get_network();

        let total_explicit_bitcoin: u64 = input_requirements.iter().filter_map(|req| {
            if let InputRequirement::Bitcoin { amount } = req { Some(*amount) } else { None }
        }).sum();

        if total_explicit_bitcoin > 0 && to_addresses.is_empty() {
            return Err(AlkanesError::Validation("Bitcoin input requirement provided but no recipient addresses.".to_string()));
        }

        let amount_per_recipient = if total_explicit_bitcoin > 0 {
            total_explicit_bitcoin / to_addresses.len() as u64
        } else {
            DUST_LIMIT
        };

        for addr_str in to_addresses {
            log::debug!("Parsing to_address in create_outputs: '{addr_str}'");
            let address = Address::from_str(addr_str)?.require_network(network)?;
            outputs.push(TxOut {
                value: bitcoin::Amount::from_sat(amount_per_recipient.max(DUST_LIMIT)),
                script_pubkey: address.script_pubkey(),
            });
        }

        if let Some(change_addr_str) = change_address {
            log::debug!("Parsing change_address in create_outputs: '{change_addr_str}'");
            let address = Address::from_str(change_addr_str)?.require_network(network)?;
            outputs.push(TxOut {
                value: bitcoin::Amount::from_sat(0),
                script_pubkey: address.script_pubkey(),
            });
        }

        Ok(outputs)
    }

    fn convert_protostone_specs(&self, specs: &[ProtostoneSpec]) -> Result<Vec<Protostone>> {
        specs.iter().map(|spec| {
            let edicts = spec.edicts.iter().map(|e| {
                Ok(ProtostoneEdict {
                    id: crate::alkanes::balance_sheet::ProtoruneRuneId {
                        block: e.alkane_id.block as u128,
                        tx: e.alkane_id.tx as u128,
                    },
                    amount: e.amount as u128,
                    output: match e.target {
                        OutputTarget::Output(v) => v as u128,
                        _ => 0, // Other targets not directly representable in ProtostoneEdict
                    },
                })
            }).collect::<Result<Vec<_>>>()?;

            Ok(Protostone {
                protocol_tag: 2, // ALKANE protocol tag
                burn: None,
                refund: None,
                pointer: spec.bitcoin_transfer.as_ref().map(|t| match t.target {
                    OutputTarget::Output(v) => v,
                    _ => 0,
                }),
                from: None,
                message: spec.cellpack.as_ref().map(|c| c.encipher()).unwrap_or_default(),
                edicts,
            })
        }).collect()
    }

    fn construct_runestone_script(&self, protostones: &[ProtostoneSpec], _num_outputs: usize) -> Result<ScriptBuf> {
        log::info!("Constructing runestone with {} protostones", protostones.len());
        
        let converted_protostones = self.convert_protostone_specs(protostones)?;

        let runestone = Runestone {
            protocol: Some(
                converted_protostones
                    .iter()
                    .map(|p| p.to_integers().map_err(|e| AlkanesError::Other(e.to_string())))
                    .collect::<Result<Vec<_>>>()?
                    .into_iter()
                    .flatten()
                    .collect::<Vec<u128>>(),
            ),
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
    ) -> Result<(Psbt, u64)> {
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
        for outpoint in &utxos {
            let utxo = self.provider.get_utxo(outpoint).await? 
                .ok_or_else(|| AlkanesError::Wallet(format!("UTXO not found: {outpoint}")))?;
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
            if envelope.is_some() && i == 0 {
                // This is the commit input, which will be a script-path spend.
                // The witness will be: <signature> <script> <control_block>
                // We use a larger placeholder to get a more accurate fee estimation.
                // A value of 400 bytes should be sufficient for most contract sizes.
                input.witness.push([0u8; 400]);
            } else {
                // Regular p2tr key-path spend or other witness types.
                // A 65-byte witness is a good estimate for a P2TR key-path spend.
                input.witness.push([0u8; 65]);
            }
        }
    
        let fee_rate_sat_vb = fee_rate.unwrap_or(600.0);
        let estimated_fee = (fee_rate_sat_vb * temp_tx.vsize() as f32).ceil() as u64;
        let capped_fee = estimated_fee.min(MAX_FEE_SATS);
        log::info!("Estimated fee: {estimated_fee}, Capped fee: {capped_fee}");
    
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
        
        Ok((psbt, capped_fee))
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
    ) -> Result<(bitcoin::psbt::Psbt, u64)> {
        self.validate_protostones(&params.protostones, params.to_addresses.len())?;

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
            let additional_utxos = self.select_utxos(&additional_reqs, &params.from_addresses).await?;
            selected_utxos.extend(additional_utxos);
        }

        let outputs = self.create_outputs(&params.to_addresses, &params.change_address, &params.input_requirements).await?;
        let runestone_script = self.construct_runestone_script(&params.protostones, outputs.len())?;
        
        let (mut psbt, fee) = self.build_psbt_and_fee(selected_utxos, outputs, Some(runestone_script), params.fee_rate, Some(envelope)).await?;
        
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

        Ok((psbt, fee))
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

        let signature = self.provider.sign_taproot_script_spend(sighash.into()).await?;
        
        let taproot_signature = taproot::Signature {
            signature,
            sighash_type: TapSighashType::Default,
        };

        let signature_bytes = taproot_signature.to_vec();
        
        log::info!("✅ Created taproot script-path signature: {} bytes", signature_bytes.len());

        Ok(signature_bytes)
    }

    /// Traces the reveal transaction to get the results of protostone execution.
    async fn trace_reveal_transaction(&self, txid: &str, params: &EnhancedExecuteParams) -> Result<Option<Vec<serde_json::Value>>> {
        log::info!("Starting enhanced transaction tracing for reveal transaction: {txid}");
        
        let tx_hex = self.provider.get_transaction_hex(txid).await?;
        let tx_bytes = hex::decode(&tx_hex).map_err(|e| AlkanesError::Hex(e.to_string()))?;
        let tx: Transaction = bitcoin::consensus::deserialize(&tx_bytes).map_err(|e| AlkanesError::Serialization(e.to_string()))?;
        
        if let Ok(decoded) = crate::runestone_enhanced::format_runestone_with_decoded_messages(&tx) {
            log::debug!("Decoded Runestone for tracing:\n{decoded:#?}");
        }

        let mut traces = Vec::new();
        // The vout for a protostone trace is a virtual vout, not a real output index.
        // It's calculated as tx.output.len() + 1 + protostone_index.
        for (i, _) in params.protostones.iter().enumerate() {
            let vout = (tx.output.len() as u32) + 1 + (i as u32);
            log::info!("Tracing protostone #{i} at virtual vout {vout}...");
            match self.provider.trace_outpoint(txid, vout).await {
                Ok(trace_result) => {
                    if let Some(events) = trace_result.get("events").and_then(|e| e.as_array()) {
                        if events.is_empty() {
                            log::warn!("Trace for {txid}:{vout} came back with an empty 'events' array.");
                        }
                    } else {
                        log::warn!("Trace for {txid}:{vout} did not contain an 'events' array.");
                    }
                    log::debug!("Trace result for vout {vout}: {trace_result:?}");
                    traces.push(trace_result);
                },
                Err(e) => {
                    log::warn!("Failed to trace vout {vout}: {e}");
                }
            }
        }
        
        if traces.is_empty() {
            Ok(None)
        } else {
            Ok(Some(traces))
        }
    }

    /// Mines blocks on the regtest network if the provider is configured for it.
    async fn mine_blocks_if_regtest(&self, params: &EnhancedExecuteParams) -> Result<()> {
        if self.provider.get_network() == bitcoin::Network::Regtest {
            log::info!("Mining blocks on regtest network...");
            sleep(Duration::from_secs(2)).await;
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
        raw_output: bool,
    ) -> Result<()> {
        if raw_output {
            println!("{}", serde_json::to_string_pretty(analysis)?);
        } else {
            println!("\n🔍 Transaction Preview");
            println!("═══════════════════════");
            println!("📋 Transaction ID: {}", tx.compute_txid());
            println!("💰 Estimated Fee: {fee} sats");
            println!("📊 Transaction Size: {} vbytes", tx.vsize());
            println!("📈 Fee Rate: {:.2} sat/vB", fee as f64 / tx.vsize() as f64);

            crate::runestone_enhanced::print_human_readable_runestone(tx, analysis);
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
        let executor = EnhancedAlkanesExecutor::new(&mut provider);
        let to_addresses = vec![addr1.clone(), addr1];
        let input_requirements = vec![];

        let outputs = executor.create_outputs(&to_addresses, &None, &input_requirements).await.unwrap();

        assert_eq!(outputs.len(), 2);
        for output in outputs {
            assert_eq!(output.value, Amount::from_sat(546));
        }
    }

    #[tokio::test]
    async fn test_create_outputs_with_explicit_bitcoin() {
        let mut provider = MockProvider::new(Network::Regtest);
        let addr1 = WalletProvider::get_address(&provider).await.unwrap();
        let executor = EnhancedAlkanesExecutor::new(&mut provider);
        let to_addresses = vec![addr1.clone(), addr1];
        let input_requirements = vec![InputRequirement::Bitcoin { amount: 20000 }];

        let outputs = executor.create_outputs(&to_addresses, &None, &input_requirements).await.unwrap();

        assert_eq!(outputs.len(), 2);
        for output in outputs {
            assert_eq!(output.value, Amount::from_sat(10000));
        }
    }
}