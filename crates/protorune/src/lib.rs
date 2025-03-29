use crate::balance_sheet::{load_sheet, PersistentRecord};
use crate::message::MessageContext;
use crate::protorune_init::index_unique_protorunes;
use crate::protostone::{
    add_to_indexable_protocols, initialized_protocol_index, MessageProcessor, Protostones,
};
use crate::tables::RuneTable;
use anyhow::{anyhow, Ok, Result};
use balance_sheet::clear_balances;
use bitcoin::blockdata::block::Block;
use bitcoin::hashes::Hash;
use bitcoin::script::Instruction;
use bitcoin::{opcodes, Network, OutPoint, ScriptBuf, Transaction, TxOut};
use metashrew::index_pointer::{AtomicPointer, IndexPointer};
#[allow(unused_imports)]
use metashrew::{
    flush, input, println,
    stdio::{stdout, Write},
};
use metashrew_support::address::Payload;
use metashrew_support::index_pointer::KeyValuePointer;
use ordinals::{Artifact, Runestone};
use ordinals::{Etching, Rune};
use protobuf::{Message, SpecialFields};
use protorune_support::constants;
use protorune_support::network::to_address_str;
use protorune_support::proto;
use protorune_support::{
    balance_sheet::{BalanceSheet, ProtoruneRuneId},
    protostone::{into_protostone_edicts, Protostone, ProtostoneEdict},
    utils::{consensus_encode, field_to_name, outpoint_encode},
};
use std::collections::{HashMap, HashSet};
use std::ops::Sub;
use std::sync::Arc;

pub mod balance_sheet;
pub mod message;
pub mod protoburn;
pub mod protorune_init;
pub mod protostone;
pub mod tables;
#[cfg(feature = "test-utils")]
pub mod test_helpers;
#[cfg(test)]
pub mod tests;
pub mod view;

pub struct Protorune(());

// TODO: these library functions could move to support modules (ie protorune-support)

pub fn default_output(tx: &Transaction) -> u32 {
    for i in 0..tx.output.len() {
        if !tx.output[i].script_pubkey.is_op_return() {
            return i as u32;
        }
    }
    0
}

pub fn num_op_return_outputs(tx: &Transaction) -> usize {
    tx.output
        .iter()
        .filter(|out| (*out.script_pubkey).is_op_return())
        .count()
}

pub fn num_non_op_return_outputs(tx: &Transaction) -> usize {
    tx.output
        .iter()
        .filter(|out| !(*out.script_pubkey).is_op_return())
        .count()
}

/// vout : the vout to transfer runes to
/// amount : the amount to transfer to the vout
/// max_amount : max amount available to transfer
/// tx : Transaction
pub fn handle_transfer_runes_to_vout(
    vout: u128,
    amount: u128,
    max_amount: u128,
    tx: &Transaction,
) -> HashMap<u32, u128> {
    // pointer should not call this function if amount is 0
    let mut output: HashMap<u32, u128> = HashMap::<u32, u128>::new();
    if (vout as usize) == tx.output.len() {
        // "special vout" -- give amount to all non-op_return vouts
        if amount == 0 {
            // this means we need to evenly distribute all runes to all
            // non op return vouts
            let count = num_non_op_return_outputs(tx) as u128;
            if count != 0 {
                let mut spread: u128 = 0;
                for i in 0..tx.output.len() as u32 {
                    if tx.output[i as usize].script_pubkey.is_op_return() {
                        continue;
                    }
                    let rem: u128 = if (max_amount % (count as u128)) - spread != 0 {
                        1
                    } else {
                        0
                    };
                    spread = spread + rem;
                    output.insert(i, max_amount / count + rem);
                }
            }
        } else {
            let count = num_non_op_return_outputs(tx) as u128;
            let mut remaining = max_amount;
            if count != 0 {
                for i in 0..tx.output.len() as u32 {
                    let amount_outpoint = std::cmp::min(remaining, amount);
                    remaining -= amount_outpoint;
                    if tx.output[i as usize].script_pubkey.is_op_return() {
                        continue;
                    }
                    output.insert(i, amount_outpoint);
                }
            }
        }
    } else {
        // trivial case. raise if can not fit u128 into u32

        // every vout should try to get the amount until we run out
        if amount == 0 {
            // we should transfer everything to this vout
            output.insert(vout.try_into().unwrap(), max_amount);
        } else {
            output.insert(vout.try_into().unwrap(), amount);
        }
    }

    return output;
}

#[cfg(not(test))]
pub fn validate_rune_etch(tx: &Transaction, commitment: Vec<u8>, height: u64) -> Result<bool> {
    for input in &tx.input {
        // extracting a tapscript does not indicate that the input being spent
        // was actually a taproot output. this is checked below, when we load the
        // output's entry from the database
        let Some(tapscript) = input.witness.tapscript() else {
            continue;
        };

        for instruction in tapscript.instructions() {
            // ignore errors, since the extracted script may not be valid
            let instruction = match instruction {
                core::result::Result::Ok(i) => i,
                Err(_) => break,
            };
            let Some(pushbytes) = instruction.push_bytes() else {
                continue;
            };

            if pushbytes.as_bytes() != commitment {
                continue;
            }

            let h: u64 = tables::RUNES
                .OUTPOINT_TO_HEIGHT
                .select(&consensus_encode(&input.previous_output)?)
                .get_value();

            // add 1 to follow the ordinals spec: https://github.com/ordinals/ord/blob/master/src/index/updater/rune_updater.rs#L454
            let confirmations = height - h + 1;
            if confirmations >= 6 {
                return Ok(true);
            }
        }
    }
    Ok(false)
}
#[cfg(test)]
pub fn validate_rune_etch(tx: &Transaction, commitment: Vec<u8>, height: u64) -> Result<bool> {
    Ok(true)
}

impl Protorune {
    pub fn index_runestone<T: MessageContext>(
        atomic: &mut AtomicPointer,
        tx: &Transaction,
        runestone: &Runestone,
        height: u64,
        index: u32,
        block: &Block,
        runestone_output_index: u32,
    ) -> Result<()> {
        let sheets: Vec<BalanceSheet> = tx
            .input
            .iter()
            .map(|input| {
                let outpoint_bytes = consensus_encode(&input.previous_output)?;
                Ok(load_sheet(&mut atomic.derive(
                    &tables::RUNES.OUTPOINT_TO_RUNES.select(&outpoint_bytes),
                )))
            })
            .collect::<Result<Vec<BalanceSheet>>>()?;
        let mut balance_sheet = BalanceSheet::concat(sheets);
        let mut balances_by_output = HashMap::<u32, BalanceSheet>::new();
        let unallocated_to = match runestone.pointer {
            Some(v) => v,
            None => default_output(tx),
        };
        if let Some(etching) = runestone.etching.as_ref() {
            Self::index_etching(
                atomic,
                etching,
                index,
                height,
                &mut balances_by_output,
                unallocated_to,
                tx,
            )?;
        }
        if let Some(mint) = runestone.mint {
            if !mint.to_string().is_empty() {
                Self::index_mint(&mint.into(), height, &mut balance_sheet)?;
            }
        }
        Self::process_edicts(
            tx,
            &into_protostone_edicts(runestone.edicts.clone()),
            &mut balances_by_output,
            &mut balance_sheet,
            &tx.output,
        )?;
        Self::handle_leftover_runes(&mut balance_sheet, &mut balances_by_output, unallocated_to)?;
        for (vout, sheet) in balances_by_output.clone() {
            let outpoint = OutPoint::new(tx.compute_txid(), vout);
            // println!(
            //     "Saving balance sheet {:?} to outpoint {:?}",
            //     sheet, outpoint
            // );
            sheet.save(
                &mut atomic.derive(
                    &tables::RUNES
                        .OUTPOINT_TO_RUNES
                        .select(&consensus_encode(&outpoint)?),
                ),
                false,
            );
        }
        Self::index_protostones::<T>(
            atomic,
            tx,
            index,
            block,
            height,
            runestone,
            runestone_output_index,
            &mut balances_by_output,
            unallocated_to,
        )?;
        Ok(())
    }
    pub fn update_balances_for_edict(
        balances_by_output: &mut HashMap<u32, BalanceSheet>,
        balance_sheet: &mut BalanceSheet,
        edict_amount: u128,
        edict_output: u32,
        rune_id: &ProtoruneRuneId,
    ) -> Result<()> {
        if !balances_by_output.contains_key(&edict_output) {
            balances_by_output.insert(edict_output, BalanceSheet::default());
        }
        let sheet: &mut BalanceSheet = balances_by_output
            .get_mut(&edict_output)
            .ok_or("")
            .map_err(|_| anyhow!("balance sheet not present"))?;
        let amount = if edict_amount == 0 {
            balance_sheet.get(&(*rune_id).into())
        } else {
            std::cmp::min(edict_amount, balance_sheet.get(&(*rune_id).into()))
        };
        // Ensure we decrease the source balance first
        balance_sheet.decrease(rune_id, amount);
        // Then increase the destination balance
        sheet.increase(rune_id, amount);
        Ok(())
    }
    pub fn process_edict(
        tx: &Transaction,
        edict: &ProtostoneEdict,
        balances_by_output: &mut HashMap<u32, BalanceSheet>,
        balances: &mut BalanceSheet,
        _outs: &Vec<TxOut>,
    ) -> Result<()> {
        if edict.id.block == 0 && edict.id.tx != 0 {
            Err(anyhow!("invalid edict"))
        } else {
            let max = balances.get(&edict.id.into());

            let transfer_targets =
                handle_transfer_runes_to_vout(edict.output, edict.amount, max, tx);

            transfer_targets.iter().try_for_each(|(vout, amount)| {
                Self::update_balances_for_edict(
                    balances_by_output,
                    balances,
                    *amount,
                    *vout,
                    &edict.id.into(),
                )?;
                Ok(())
            })?;
            Ok(())
        }
    }
    pub fn process_edicts(
        tx: &Transaction,
        edicts: &Vec<ProtostoneEdict>,
        balances_by_output: &mut HashMap<u32, BalanceSheet>,
        balances: &mut BalanceSheet,
        outs: &Vec<TxOut>,
    ) -> Result<()> {
        for edict in edicts {
            Self::process_edict(tx, edict, balances_by_output, balances, outs)?;
        }
        Ok(())
    }
    pub fn handle_leftover_runes(
        remaining_balances: &mut BalanceSheet,
        balances_by_output: &mut HashMap<u32, BalanceSheet>,
        unallocated_to: u32,
    ) -> Result<()> {
        // grab the balances of the vout to send unallocated to
        match balances_by_output.get_mut(&unallocated_to) {
            // if it already has balances, then send the remaining balances over
            Some(v) => remaining_balances.pipe(v),
            None => {
                balances_by_output.insert(unallocated_to, remaining_balances.clone());
            }
        }
        Ok(())

        // This piece of logic allows the pointer to evenly distribute if set == number of tx outputs.
        // We discovered that when the pointer == number of tx outputs, the decipher step
        // thinks it is a cenotaph. This logic used right now, but keep it here in case we need it.
        // for (rune, balance) in &remaining_balances.balances {
        //     // amount is 0 to evenly distribute
        //     let transfer_targets =
        //         handle_transfer_runes_to_vout(unallocated_to as u128, 0, *balance, tx);

        //     transfer_targets.iter().for_each(|(vout, amount)| {
        //         // grab the balances of the vout to send unallocated to
        //         match balances_by_output.get_mut(vout) {
        //             // if it already has balances, then send the remaining balances over
        //             Some(v) => v.increase(rune, *amount),
        //             None => {
        //                 balances_by_output.insert(unallocated_to, remaining_balances.clone());
        //             }
        //         }
        //     });
        // }
    }
    pub fn index_mint(
        mint: &ProtoruneRuneId,
        height: u64,
        balance_sheet: &mut BalanceSheet,
    ) -> Result<()> {
        let name = tables::RUNES
            .RUNE_ID_TO_ETCHING
            .select(&mint.clone().into())
            .get();
        let remaining: u128 = tables::RUNES.MINTS_REMAINING.select(&name).get_value();
        let amount: u128 = tables::RUNES.AMOUNT.select(&name).get_value();

        if remaining == 0 {
            // 2 ways we can reach this statement:
            //   - etching and mint are in the same runestone
            //   - the rune has reached the cap of mints
            return Ok(());
        }
        if remaining > 0 {
            let height_start: u64 = tables::RUNES.HEIGHTSTART.select(&name).get_value();
            let height_end: u64 = tables::RUNES.HEIGHTEND.select(&name).get_value();
            let offset_start: u64 = tables::RUNES.OFFSETSTART.select(&name).get_value();
            let offset_end: u64 = tables::RUNES.OFFSETEND.select(&name).get_value();
            // the other mint terms are stored from the rune name, the etching height is
            // stored by the rune id
            let etching_height: u64 = tables::RUNES
                .RUNE_ID_TO_HEIGHT
                .select(&mint.to_owned().into())
                .get_value();
            if (height_start == 0 || height >= height_start)
                && (height_end == 0 || height < height_end)
                && (offset_start == 0 || height >= offset_start + etching_height)
                && (offset_end == 0 || height < etching_height + offset_end)
            {
                tables::RUNES
                    .MINTS_REMAINING
                    .select(&name)
                    .set_value(remaining.sub(1));
                balance_sheet.increase(
                    &(ProtoruneRuneId {
                        block: u128::from(mint.block),
                        tx: u128::from(mint.tx),
                    }),
                    amount,
                );
            } else {
                return Ok(());
            }
        }
        Ok(())
    }

    pub fn index_etching(
        atomic: &mut AtomicPointer,
        etching: &Etching,
        index: u32,
        height: u64,
        balances_by_output: &mut HashMap<u32, BalanceSheet>,
        unallocated_to: u32,
        tx: &Transaction,
    ) -> Result<()> {
        let etching_rune = match etching.rune {
            Some(rune) => {
                if Self::verify_non_reserved_name(height.try_into()?, &rune).is_ok()
                    && validate_rune_etch(tx, rune.commitment(), height)?
                {
                    rune
                } else {
                    // if the non reserved name is incorrect, the etching is ignored
                    return Ok(());
                }
            }
            None => Rune::reserved(height, index),
        };

        let name = field_to_name(&etching_rune.0);
        let indexer_rune_name = name.as_bytes().to_vec();

        // check if rune name alredy exists
        if let std::result::Result::Ok(rune_id) = ProtoruneRuneId::try_from(
            (*tables::RUNES
                .ETCHING_TO_RUNE_ID
                .select(&indexer_rune_name)
                .get())
            .clone(),
        ) {
            println!(
                "Found duplicate rune name {} with rune id {:?}: . Skipping this etching.",
                name, rune_id
            );
            return Ok(());
        }
        let rune_id = ProtoruneRuneId::new(height.into(), index.into());
        atomic
            .derive(&tables::RUNES.RUNE_ID_TO_ETCHING.select(&rune_id.into()))
            .set(Arc::new(indexer_rune_name.clone()));
        atomic
            .derive(&tables::RUNES.ETCHING_TO_RUNE_ID.select(&indexer_rune_name))
            .set(rune_id.into());
        atomic
            .derive(&tables::RUNES.RUNE_ID_TO_HEIGHT.select(&rune_id.into()))
            .set_value(height);

        if let Some(divisibility) = etching.divisibility {
            atomic
                .derive(&tables::RUNES.DIVISIBILITY.select(&indexer_rune_name))
                .set_value(divisibility);
        }
        if let Some(premine) = etching.premine {
            atomic
                .derive(&tables::RUNES.PREMINE.select(&indexer_rune_name))
                .set_value(premine);
            let rune = ProtoruneRuneId {
                block: u128::from(height),
                tx: u128::from(index),
            };
            let sheet = BalanceSheet::from_pairs(vec![rune], vec![premine]);
            //.pipe(balance_sheet);
            balances_by_output.insert(unallocated_to, sheet);
        }
        if let Some(terms) = etching.terms {
            if let Some(amount) = terms.amount {
                atomic
                    .derive(&tables::RUNES.AMOUNT.select(&indexer_rune_name))
                    .set_value(amount);
            }
            if let Some(cap) = terms.cap {
                atomic
                    .derive(&tables::RUNES.CAP.select(&indexer_rune_name))
                    .set_value(cap);
                atomic
                    .derive(&tables::RUNES.MINTS_REMAINING.select(&indexer_rune_name))
                    .set_value(cap);
            }
            if let (Some(height_start), Some(height_end)) = (terms.height.0, terms.height.1) {
                atomic
                    .derive(&tables::RUNES.HEIGHTSTART.select(&indexer_rune_name))
                    .set_value(height_start);

                atomic
                    .derive(&tables::RUNES.HEIGHTEND.select(&indexer_rune_name))
                    .set_value(height_end);
            }
            if let (Some(offset_start), Some(offset_end)) = (terms.offset.0, terms.offset.1) {
                atomic
                    .derive(&tables::RUNES.OFFSETSTART.select(&indexer_rune_name))
                    .set_value(offset_start);
                atomic
                    .derive(&tables::RUNES.OFFSETEND.select(&indexer_rune_name))
                    .set_value(offset_end);
            }
        }

        // runes spec states this is the default symbol if symbol is omitted
        let symbol = etching.symbol.unwrap_or('¤');
        atomic
            .derive(&tables::RUNES.SYMBOL.select(&indexer_rune_name))
            .set_value(symbol as u32);

        if let Some(spacers) = etching.spacers {
            atomic
                .derive(&tables::RUNES.SPACERS.select(&indexer_rune_name))
                .set_value(spacers);
        }

        atomic
            .derive(&tables::RUNES.ETCHINGS.select(&indexer_rune_name))
            .append(Arc::new(indexer_rune_name.clone()));

        atomic
            .derive(&tables::HEIGHT_TO_RUNES.select_value(height))
            .append(Arc::new(indexer_rune_name.clone()));

        Ok(())
    }

    fn verify_non_reserved_name(block: u32, rune: &Rune) -> Result<()> {
        // TODO: chain name
        let minimum_name = Rune::minimum_at_height(Network::Bitcoin, ordinals::Height(block));
        if rune.n() < minimum_name.n() {
            println!("error not unlocked");
            return Err(anyhow!("Given name is not unlocked yet"));
        }
        if rune.n() >= constants::RESERVED_NAME {
            return Err(anyhow!("Given name is reserved"));
        }

        Ok(())
    }

    pub fn build_rune_id(height: u64, tx: u32) -> Arc<Vec<u8>> {
        let rune_id = <ProtoruneRuneId as Into<Vec<u8>>>::into(ProtoruneRuneId::new(
            height as u128,
            tx as u128,
        ));
        return Arc::new(rune_id);
    }

    pub fn get_runestone_output_index(transaction: &Transaction) -> Result<u32> {
        // search transaction outputs for payload
        for (i, output) in transaction.output.iter().enumerate() {
            let mut instructions = output.script_pubkey.instructions();

            // Check if the first instruction is OP_RETURN
            if let Some(std::result::Result::Ok(Instruction::Op(opcodes::all::OP_RETURN))) =
                instructions.next()
            {
                return Ok(i as u32);
            }
        }

        // If no matching output is found, return an error
        Err(anyhow!("did not find a output index"))
    }

    pub fn index_unspendables<T: MessageContext>(block: &Block, height: u64) -> Result<()> {
        for (index, tx) in block.txdata.iter().enumerate() {
            if let Some(Artifact::Runestone(ref runestone)) = Runestone::decipher(tx) {
                let mut atomic = AtomicPointer::default();
                let runestone_output_index: u32 = Self::get_runestone_output_index(tx)?;
                match Self::index_runestone::<T>(
                    &mut atomic,
                    tx,
                    runestone,
                    height,
                    index as u32,
                    block,
                    runestone_output_index,
                ) {
                    Err(e) => {
                        println!("err: {:?}", e);
                        atomic.rollback();
                    }
                    _ => {
                        atomic.commit();
                    }
                };
            }
            for input in &tx.input {
                //all inputs must be used up, even in cenotaphs
                let key = consensus_encode(&input.previous_output)?;
                clear_balances(&mut tables::RUNES.OUTPOINT_TO_RUNES.select(&key));
            }
        }
        Ok(())
    }
    pub fn index_spendables(txdata: &Vec<Transaction>) -> Result<HashSet<Vec<u8>>> {
        // Track unique addresses that have their spendable outpoints updated
        #[cfg(feature = "cache")]
        let mut updated_addresses: HashSet<Vec<u8>> = HashSet::new();
        
        #[cfg(not(feature = "cache"))]
        let mut updated_addresses: HashSet<Vec<u8>> = HashSet::new();
        
        for (txindex, transaction) in txdata.iter().enumerate() {
            let tx_id = transaction.compute_txid();
            tables::RUNES
                .TXID_TO_TXINDEX
                .select(&tx_id.as_byte_array().to_vec())
                .set_value(txindex as u32);
            for (index, output) in transaction.output.iter().enumerate() {
                let outpoint = OutPoint {
                    txid: tx_id.clone(),
                    vout: index as u32,
                };
                let output_script_pubkey: &ScriptBuf = &output.script_pubkey;
                if Payload::from_script(output_script_pubkey).is_ok() {
                    let outpoint_bytes: Vec<u8> = consensus_encode(&outpoint)?;
                    let address_str = to_address_str(output_script_pubkey).unwrap();
                    let address = address_str.into_bytes();
                    
                    // Add address to the set of updated addresses
                    #[cfg(feature = "cache")]
                    updated_addresses.insert(address.to_vec());
                    
                    tables::OUTPOINTS_FOR_ADDRESS
                        .select(&address.clone())
                        .append(Arc::new(outpoint_bytes.clone()));
                    tables::OUTPOINT_SPENDABLE_BY
                        .select(&outpoint_bytes.clone())
                        .set(Arc::new(address.clone()))
                }
            }
        }
        
        // Return the set of updated addresses
        Ok(updated_addresses)
    }
    pub fn index_spendables_ll(txdata: &Vec<Transaction>) -> Result<HashSet<Vec<u8>>> {
        // Track unique addresses that have their spendable outpoints updated
        #[cfg(feature = "cache")]
        let mut updated_addresses: HashSet<Vec<u8>> = HashSet::new();
        
        #[cfg(not(feature = "cache"))]
        let mut updated_addresses: HashSet<Vec<u8>> = HashSet::new();
        
        for (txindex, transaction) in txdata.iter().enumerate() {
            let tx_id = transaction.compute_txid();
            tables::RUNES
                .TXID_TO_TXINDEX
                .select(&tx_id.as_byte_array().to_vec())
                .set_value(txindex as u32);
            for (index, output) in transaction.output.iter().enumerate() {
                let outpoint = OutPoint {
                    txid: tx_id.clone(),
                    vout: index as u32,
                };
                let output_script_pubkey: &ScriptBuf = &output.script_pubkey;
                if Payload::from_script(output_script_pubkey).is_ok() {
                    let outpoint_bytes: Vec<u8> = consensus_encode(&outpoint)?;
                    let address_str = to_address_str(output_script_pubkey).unwrap();
                    let address = address_str.into_bytes();
                    
                    // Add address to the set of updated addresses
                    #[cfg(feature = "cache")]
                    if address.len() > 0 {
                        updated_addresses.insert(address.to_vec());
                    }
                    
                    tables::OUTPOINTS_FOR_ADDRESS
                        .select(&address.clone())
                        .append(Arc::new(outpoint_bytes.clone()));
                    if address.len() > 0 {
                        tables::OUTPOINT_SPENDABLE_BY_ADDRESS
                            .select(&address.clone())
                            .append_ll(Arc::new(outpoint_bytes.clone()));
                        let pos = tables::OUTPOINT_SPENDABLE_BY_ADDRESS
                            .select(&address.clone())
                            .length()
                            - 1;
                        tables::OUTPOINT_SPENDABLE_BY_ADDRESS
                            .select(&outpoint_bytes.clone())
                            .set_value(pos);
                    }
                    tables::OUTPOINT_SPENDABLE_BY
                        .select(&outpoint_bytes.clone())
                        .set(Arc::new(address.clone()))
                }
            }
            for input in transaction.input.iter() {
                let outpoint_bytes = consensus_encode(&input.previous_output)?;
                let pos: u32 = tables::OUTPOINT_SPENDABLE_BY_ADDRESS
                    .select(&outpoint_bytes)
                    .get_value();
                let address = tables::OUTPOINT_SPENDABLE_BY.select(&outpoint_bytes).get();
                if address.len() > 0 {
                    // Add address to the set of updated addresses (for spent inputs)
                    #[cfg(feature = "cache")]
                    updated_addresses.insert(address.as_ref().to_vec());
                    
                    tables::OUTPOINT_SPENDABLE_BY_ADDRESS
                        .select(&address)
                        .delete_value(pos);
                    if pos > 0 {
                        tables::OUTPOINT_SPENDABLE_BY_ADDRESS
                            .select(&outpoint_bytes)
                            .nullify();
                    }
                }
            }
        }
        
        // Return the set of updated addresses
        Ok(updated_addresses)
    }

    pub fn index_transaction_ids(block: &Block, height: u64) -> Result<()> {
        let ptr = tables::RUNES
            .HEIGHT_TO_TRANSACTION_IDS
            .select_value::<u64>(height);
        for tx in &block.txdata {
            ptr.append(Arc::new(tx.compute_txid().as_byte_array().to_vec()));
        }
        Ok(())
    }
    pub fn index_outpoints(block: &Block, height: u64) -> Result<()> {
        let mut atomic = AtomicPointer::default();
        for tx in &block.txdata {
            for i in 0..tx.output.len() {
                let outpoint_bytes = outpoint_encode(
                    &(OutPoint {
                        txid: tx.compute_txid(),
                        vout: i as u32,
                    }),
                )?;
                atomic
                    .derive(&tables::RUNES.OUTPOINT_TO_HEIGHT.select(&outpoint_bytes))
                    .set_value(height);
                atomic
                    .derive(&tables::OUTPOINT_TO_OUTPUT.select(&outpoint_bytes))
                    .set(Arc::new(
                        (proto::protorune::Output {
                            script: tx.output[i].clone().script_pubkey.into_bytes(),
                            value: tx.output[i].clone().value.to_sat(),
                            special_fields: SpecialFields::new(),
                        })
                        .write_to_bytes()
                        .unwrap(),
                    ));
            }
        }
        atomic.commit();
        Ok(())
    }
    pub fn save_balances<T: MessageContext>(
        height: u64,
        atomic: &mut AtomicPointer,
        table: &RuneTable,
        tx: &Transaction,
        map: &HashMap<u32, BalanceSheet>,
    ) -> Result<()> {
        // Process all outputs, including the last one
        // The OP_RETURN doesn't have to be at the end
        for i in 0..tx.output.len() {
            // Skip OP_RETURN outputs
            if tx.output[i].script_pubkey.is_op_return() {
                continue;
            }

            let sheet = map
                .get(&(i as u32))
                .map(|v| v.clone())
                .unwrap_or_else(|| BalanceSheet::default());
            let outpoint = OutPoint {
                txid: tx.compute_txid(),
                vout: i as u32,
            };
            // println!(
            //     "saving balancesheet: {:#?} to outpoint: {:#?}",
            //     sheet, outpoint
            // );
            sheet.save(
                &mut atomic.derive(
                    &table
                        .OUTPOINT_TO_RUNES
                        .select(&consensus_encode(&outpoint)?),
                ),
                false,
            );
        }
        if map.contains_key(&u32::MAX) {
            map.get(&u32::MAX)
                .map(|v| v.clone())
                .unwrap_or_else(|| BalanceSheet::default())
                .save(&mut atomic.derive(&table.RUNTIME_BALANCE), false);
        }
        index_unique_protorunes::<T>(
            atomic,
            height,
            map.iter()
                .fold(BalanceSheet::default(), |mut r, (_k, v)| {
                    v.pipe(&mut r);
                    r
                })
                .balances
                .keys()
                .clone()
                .into_iter()
                .map(|v| v.clone())
                .collect::<Vec<ProtoruneRuneId>>(),
        );
        Ok(())
    }

    pub fn index_protostones<T: MessageContext>(
        atomic: &mut AtomicPointer,
        tx: &Transaction,
        txindex: u32,
        block: &Block,
        height: u64,
        runestone: &Runestone,
        runestone_output_index: u32,
        balances_by_output: &mut HashMap<u32, BalanceSheet>,
        unallocated_to: u32,
    ) -> Result<()> {
        let protostones = Protostone::from_runestone(runestone)?;
        if protostones.len() != 0 {
            let mut proto_balances_by_output = HashMap::<u32, BalanceSheet>::new();
            let table = tables::RuneTable::for_protocol(T::protocol_tag());

            // set the starting runtime balance
            proto_balances_by_output.insert(
                u32::MAX,
                load_sheet(&mut atomic.derive(&table.RUNTIME_BALANCE)),
            );

            // load the balance sheets
            let sheets: Vec<BalanceSheet> = tx
                .input
                .iter()
                .map(|input| {
                    Ok(load_sheet(
                        &mut atomic.derive(
                            &table
                                .OUTPOINT_TO_RUNES
                                .select(&consensus_encode(&input.previous_output)?),
                        ),
                    ))
                })
                .collect::<Result<Vec<BalanceSheet>>>()?;
            let mut balance_sheet = BalanceSheet::concat(sheets);
            protostones.process_burns(
                &mut atomic.derive(&IndexPointer::default()),
                runestone,
                runestone_output_index,
                balances_by_output,
                &mut proto_balances_by_output,
                unallocated_to,
                tx.compute_txid(),
            )?;

            let num_protostones = protostones.len();
            let protostones_iter = protostones.into_iter();
            // by default, all protorunes that come in as input will be given to the
            // first protostone with a matching protocol_tag
            if let Some(position) = protostones_iter
                .clone()
                .position(|s| s.protocol_tag == T::protocol_tag())
            {
                Self::handle_leftover_runes(
                    &mut balance_sheet,
                    &mut proto_balances_by_output,
                    (tx.output.len() as u32) + 1 + position as u32,
                )?;
            }
            protostones_iter
                .enumerate()
                .map(|(i, stone)| {
                    let shadow_vout = (i as u32) + (tx.output.len() as u32) + 1;
                    if !proto_balances_by_output.contains_key(&shadow_vout) {
                        proto_balances_by_output.insert(shadow_vout, BalanceSheet::default());
                    }
                    let protostone_unallocated_to = match stone.pointer {
                        Some(v) => v,
                        None => default_output(tx),
                    };
                    // README: now calculates the amount left over for edicts in this fashion:
                    // the protomessage is executed first, and all the runes that go to the refund pointer are available for the edicts to then transfer
                    // if there is no protomessage, all incoming runes will be available to be transferred by the edict
                    let mut prior_balance_sheet = BalanceSheet::default();
                    let is_message = stone.is_message();
                    if is_message {
                        let refund = stone.refund.unwrap();
                        // Start with a fresh balance sheet for edicts
                        prior_balance_sheet = match proto_balances_by_output.get(&refund) {
                            Some(sheet) => sheet.clone(),
                            None => BalanceSheet::default(),
                        };
                        stone.process_message::<T>(
                            &mut atomic.derive(&IndexPointer::default()),
                            tx,
                            txindex,
                            block,
                            height,
                            runestone_output_index,
                            shadow_vout,
                            &mut proto_balances_by_output,
                            protostone_unallocated_to,
                            num_protostones,
                        )?;
                        // Get the post-message balance to use for edicts
                        prior_balance_sheet = match proto_balances_by_output.get(&refund) {
                            Some(sheet) => sheet.clone(),
                            None => prior_balance_sheet,
                        };
                    } else {
                        prior_balance_sheet = match proto_balances_by_output.remove(&shadow_vout) {
                            Some(sheet) => sheet.clone(),
                            None => prior_balance_sheet,
                        };
                    }

                    // Process edicts using the current balance state
                    Self::process_edicts(
                        tx,
                        &stone.edicts,
                        &mut proto_balances_by_output,
                        &mut prior_balance_sheet,
                        &tx.output,
                    )?;

                    // Handle any remaining balance
                    if !is_message {
                        Self::handle_leftover_runes(
                            &mut prior_balance_sheet,
                            &mut proto_balances_by_output,
                            protostone_unallocated_to,
                        )?;
                    }

                    Ok(())
                })
                .collect::<Result<()>>()?;
            // println!(
            //     "protocol id: {}, saving sheets: {:#?}",
            //     T::protocol_tag(),
            //     proto_balances_by_output
            // );
            Self::save_balances::<T>(
                height,
                &mut atomic.derive(&IndexPointer::default()),
                &table,
                tx,
                &mut proto_balances_by_output,
            )?;
            for input in &tx.input {
                //all inputs must be used up, even in cenotaphs
                let key = consensus_encode(&input.previous_output)?;
                clear_balances(&mut table.OUTPOINT_TO_RUNES.select(&key));
            }
        }
        Ok(())
    }

    pub fn index_block<T: MessageContext>(block: Block, height: u64) -> Result<HashSet<Vec<u8>>> {
        let init_result = initialized_protocol_index().map_err(|e| anyhow!(e.to_string()));
        let add_result =
            add_to_indexable_protocols(T::protocol_tag()).map_err(|e| anyhow!(e.to_string()));
        init_result?;
        add_result?;
        tables::RUNES
            .HEIGHT_TO_BLOCKHASH
            .select_value::<u64>(height)
            .set(Arc::new(consensus_encode(&block.block_hash())?));
        tables::RUNES
            .BLOCKHASH_TO_HEIGHT
            .select(&consensus_encode(&block.block_hash())?)
            .set_value::<u64>(height);
        Self::index_transaction_ids(&block, height)?;
        Self::index_outpoints(&block, height)?;
        
        // Get the set of updated addresses
        let updated_addresses = Self::index_spendables(&block.txdata)?;
        
        Self::index_unspendables::<T>(&block, height)?;
        
        // Return the set of updated addresses
        Ok(updated_addresses)
    }
}

// GENESIS RUNE REF

//     const name = nameToArrayBuffer("UNCOMMONGOODS");
//     const spacers = 128;
//     const runeId = new ProtoruneRuneId(1, 0).toBytes();
//     ETCHING_TO_RUNE_ID.select(name).set(runeId);
//     RUNE_ID_TO_ETCHING.select(runeId).set(name);
//     RUNE_ID_TO_HEIGHT.select(runeId).setValue<u32>(GENESIS);
//     DIVISIBILITY.select(name).setValue<u8>(1);
//     AMOUNT.select(name).set(toArrayBuffer(u128.from(1)));
//     CAP.select(name).set(toArrayBuffer(u128.Max));
//     MINTS_REMAINING.select(name).set(toArrayBuffer(u128.Max));
//     OFFSETEND.select(name).setValue<u64>(SUBSIDY_HALVING_INTERVAL);
//     SPACERS.select(name).setValue<u32>(128);
//     SYMBOL.select(name).setValue<u8>(<u8>"\u{29C9}".charCodeAt(0));
//     ETCHINGS.append(name);
//   }
