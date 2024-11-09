use crate::balance_sheet::{load_sheet, PersistentRecord};
use crate::message::MessageContext;
use crate::protostone::{
    add_to_indexable_protocols, initialized_protocol_index, MessageProcessor, Protostones,
};
use anyhow::{anyhow, Ok, Result};
use bitcoin::blockdata::block::Block;
use bitcoin::hashes::Hash;
use bitcoin::script::Instruction;
use bitcoin::{opcodes, Address, OutPoint, ScriptBuf, Transaction, TxOut};
use metashrew::index_pointer::{AtomicPointer, IndexPointer};
#[allow(unused_imports)]
use metashrew::{
    flush, input, println,
    stdio::{stdout, Write},
};
use metashrew_support::index_pointer::KeyValuePointer;
use metashrew_support::{
    compat::{to_arraybuffer_layout, to_ptr},
    utils::consume_to_end,
};
use ordinals::Etching;
use ordinals::{Artifact, Runestone};
use proto::protorune::{Output, RunesResponse, WalletResponse};
use protobuf::{Message, SpecialFields};
use protorune_support::constants;
use protorune_support::tables::RuneTable;
use protorune_support::{
    balance_sheet::{BalanceSheet, ProtoruneRuneId},
    protostone::{into_protostone_edicts, Protostone, ProtostoneEdict},
    utils::{consensus_encode, field_to_name},
};
use std::collections::HashMap;
use std::io::Cursor;
use std::ops::Sub;
use std::sync::Arc;

pub mod balance_sheet;
pub mod message;
pub mod proto;
pub mod protoburn;
pub mod protostone;
pub mod tables;
#[cfg(feature = "test-utils")]
pub mod test_helpers;
#[cfg(test)]
pub mod tests;
pub mod view;

pub struct Protorune(());

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

#[no_mangle]
pub fn runesbyaddress() -> i32 {
    let mut data: Cursor<Vec<u8>> = Cursor::new(input());
    let result: WalletResponse =
        view::runes_by_address(&consume_to_end(&mut data).unwrap()).unwrap();
    to_ptr(&mut to_arraybuffer_layout::<&[u8]>(
        result.write_to_bytes().unwrap().as_ref(),
    )) + 4
}

#[no_mangle]
pub fn protorunesbyaddress() -> i32 {
    let mut data: Cursor<Vec<u8>> = Cursor::new(input());
    let result: WalletResponse =
        view::protorunes_by_address(&consume_to_end(&mut data).unwrap()).unwrap();
    to_ptr(&mut to_arraybuffer_layout::<&[u8]>(
        &result.write_to_bytes().unwrap(),
    )) + 4
}

#[no_mangle]
pub fn runesbyheight() -> i32 {
    let mut data: Cursor<Vec<u8>> = Cursor::new(input());
    let result: RunesResponse = view::runes_by_height(&consume_to_end(&mut data).unwrap()).unwrap();
    let buffer: Vec<u8> = result.write_to_bytes().unwrap();
    to_ptr(&mut to_arraybuffer_layout::<&[u8]>(buffer.as_ref())) + 4
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
                Ok(load_sheet(
                    &mut atomic.derive(
                        &tables::RUNES
                            .OUTPOINT_TO_RUNES
                            .select(&consensus_encode(&input.previous_output)?),
                    ),
                ))
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
        balance_sheet.decrease(rune_id, amount);
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
            if (edict.output as usize) == tx.output.len() {
                if edict.amount == 0 {
                    let count = num_non_op_return_outputs(tx) as u128;
                    if count != 0 {
                        let max = balances.get(&edict.id.into());
                        let mut spread: u128 = 0;
                        for i in 0..tx.output.len() as u32 {
                            if tx.output[i as usize].script_pubkey.is_op_return() {
                                continue;
                            }
                            let rem: u128 = if (max % (count as u128)) - spread != 0 {
                                1
                            } else {
                                0
                            };
                            spread = spread + rem;
                            Self::update_balances_for_edict(
                                balances_by_output,
                                balances,
                                max / count + rem,
                                i,
                                &edict.id.into(),
                            )?;
                        }
                    }
                } else {
                    let count = num_non_op_return_outputs(tx) as u128;
                    if count != 0 {
                        let amount = edict.amount;
                        for i in 0..tx.output.len() as u32 {
                            if tx.output[i as usize].script_pubkey.is_op_return() {
                                continue;
                            }
                            Self::update_balances_for_edict(
                                balances_by_output,
                                balances,
                                amount,
                                i,
                                &edict.id.into(),
                            )?;
                        }
                    }
                }
            } else {
                Self::update_balances_for_edict(
                    balances_by_output,
                    balances,
                    edict.amount,
                    edict.output as u32,
                    &edict.id.into(),
                )?;
            }
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
        balances: &mut BalanceSheet,
        balances_by_output: &mut HashMap<u32, BalanceSheet>,
        unallocated_to: u32,
    ) -> Result<()> {
        match balances_by_output.get_mut(&unallocated_to) {
            Some(v) => balances.pipe(v),
            None => {
                balances_by_output.insert(unallocated_to, balances.clone());
            }
        }
        Ok(())
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
        if remaining != 0 {
            let height_start: u64 = tables::RUNES.HEIGHTSTART.select(&name).get_value();
            let height_end: u64 = tables::RUNES.HEIGHTEND.select(&name).get_value();
            let offset_start: u64 = tables::RUNES.OFFSETSTART.select(&name).get_value();
            let offset_end: u64 = tables::RUNES.OFFSETEND.select(&name).get_value();
            let etching_height: u64 = tables::RUNES.RUNE_ID_TO_HEIGHT.select(&name).get_value();

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
    ) -> Result<()> {
        if let Some(name) = etching.rune {
            let _name = field_to_name(&name.0);
            //Self::get_reserved_name(height, index, name);
            let rune_id = ProtoruneRuneId::new(height.into(), index.into());
            atomic
                .derive(&tables::RUNES.RUNE_ID_TO_ETCHING.select(&rune_id.into()))
                .set(Arc::new(name.0.to_string().into_bytes()));
            atomic
                .derive(
                    &tables::RUNES
                        .ETCHING_TO_RUNE_ID
                        .select(&_name.as_bytes().to_vec()),
                )
                .set(rune_id.into());
            atomic
                .derive(&tables::RUNES.RUNE_ID_TO_HEIGHT.select(&rune_id.into()))
                .set_value(height);

            if let Some(divisibility) = etching.divisibility {
                atomic
                    .derive(
                        &tables::RUNES
                            .DIVISIBILITY
                            .select(&_name.as_bytes().to_vec()),
                    )
                    .set_value(divisibility);
            }
            if let Some(premine) = etching.premine {
                atomic
                    .derive(&tables::RUNES.PREMINE.select(&_name.as_bytes().to_vec()))
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
                        .derive(&tables::RUNES.AMOUNT.select(&_name.as_bytes().to_vec()))
                        .set_value(amount);
                }
                if let Some(cap) = terms.cap {
                    atomic
                        .derive(&tables::RUNES.CAP.select(&_name.as_bytes().to_vec()))
                        .set_value(cap);
                    atomic
                        .derive(
                            &tables::RUNES
                                .MINTS_REMAINING
                                .select(&_name.as_bytes().to_vec()),
                        )
                        .set_value(cap);
                }
                if let (Some(height_start), Some(height_end)) = (terms.height.0, terms.height.1) {
                    atomic
                        .derive(&tables::RUNES.HEIGHTSTART.select(&_name.as_bytes().to_vec()))
                        .set_value(height_start);

                    atomic
                        .derive(&tables::RUNES.HEIGHTEND.select(&_name.as_bytes().to_vec()))
                        .set_value(height_end);
                }
                if let (Some(offset_start), Some(offset_end)) = (terms.offset.0, terms.offset.1) {
                    atomic
                        .derive(&tables::RUNES.OFFSETSTART.select(&_name.as_bytes().to_vec()))
                        .set_value(offset_start);
                    atomic
                        .derive(&tables::RUNES.OFFSETEND.select(&_name.as_bytes().to_vec()))
                        .set_value(offset_end);
                }
            }
            if let Some(symbol) = etching.symbol {
                atomic
                    .derive(&tables::RUNES.SYMBOL.select(&_name.as_bytes().to_vec()))
                    .set_value(symbol as u32);
            }

            if let Some(spacers) = etching.spacers {
                atomic
                    .derive(&tables::RUNES.SPACERS.select(&_name.as_bytes().to_vec()))
                    .set_value(spacers);
            }

            atomic
                .derive(&tables::RUNES.ETCHINGS.select(&_name.as_bytes().to_vec()))
                .append(Arc::new(_name.as_bytes().to_vec()));

            atomic
                .derive(&tables::HEIGHT_TO_RUNES.select_value(height))
                .append(Arc::new(_name.as_bytes().to_vec()));
        }
        Ok(())
    }

    // fn get_reserved_name(height: u32, index: u32, name: u128) -> Result<Vec<u8>, Error> {
    //     let mut interval: i64 =
    //         ((height - constants::GENESIS) as i64) / (constants::HEIGHT_INTERVAL as i64);
    //     let mut minimum_name: u128 = constants::MINIMUM_NAME;
    //     while interval > 0 {
    //         minimum_name = minimum_name.sub(1) / constants::TWENTY_SIX;
    //         interval -= 1;
    //     }
    //     if name < minimum_name || name >= constants::RESERVED_NAME {
    //         Ok(Vec::new())
    //     } else {
    //         Error::new("No reserve name")
    //     }
    // }

    pub fn build_rune_id(height: u64, tx: u32) -> Arc<Vec<u8>> {
        let rune_id = ProtoruneRuneId::new(height as u128, tx as u128)
            .to_string()
            .into_bytes();
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
        }
        Ok(())
    }
    pub fn index_spendables(txdata: &Vec<Transaction>) -> Result<()> {
        for (_txindex, transaction) in txdata.iter().enumerate() {
            let tx_id = transaction.compute_txid();

            for (index, output) in transaction.output.iter().enumerate() {
                let outpoint = OutPoint {
                    txid: tx_id.clone(),
                    vout: index as u32,
                };
                let output_script_pubkey: &ScriptBuf = &output.script_pubkey;
                if Address::from_script(&output_script_pubkey, constants::NETWORK).is_ok() {
                    let outpoint_bytes: Vec<u8> = consensus_encode(&outpoint)?;
                    let address = Address::from_script(&output_script_pubkey, constants::NETWORK)?;
                    tables::OUTPOINTS_FOR_ADDRESS
                        .select(&address.to_string().into_bytes())
                        .append(Arc::new(outpoint_bytes.clone()));
                    tables::OUTPOINT_SPENDABLE_BY
                        .select(&outpoint_bytes.clone())
                        .set(Arc::new(address.to_string().into_bytes()));
                }
            }
        }
        Ok(())
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
            let ptr = atomic.derive(
                &tables::RUNES
                    .OUTPOINT_TO_HEIGHT
                    .select(&tx.compute_txid().as_byte_array().to_vec()),
            );
            for i in 0..tx.output.len() {
                ptr.select_value(i as u32).set_value(height);
                atomic
                    .derive(
                        &tables::OUTPOINT_TO_OUTPUT.select(
                            &consensus_encode(
                                &(OutPoint {
                                    txid: tx.compute_txid(),
                                    vout: i as u32,
                                }),
                            )
                            .unwrap(),
                        ),
                    )
                    .set(Arc::new(
                        (Output {
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
    pub fn save_balances(
        atomic: &mut AtomicPointer,
        table: &RuneTable,
        tx: &Transaction,
        map: &HashMap<u32, BalanceSheet>,
    ) -> Result<()> {
        // TODO: check is -1 necessary? it seems like this is trying to skip the op return, but the op return doesn't have to be at the end
        for i in 0..tx.output.len() - 1 {
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
            protostones
                .into_iter()
                .enumerate()
                .map(|(i, stone)| {
                    if !stone.edicts.is_empty() {
                        Self::process_edicts(
                            tx,
                            &stone.edicts.clone().into(),
                            &mut proto_balances_by_output,
                            &mut balance_sheet,
                            &tx.output,
                        )?;
                        Self::handle_leftover_runes(
                            &mut balance_sheet,
                            &mut proto_balances_by_output.clone(),
                            unallocated_to,
                        )?;
                        for (vout, sheet) in balances_by_output.clone() {
                            let outpoint = OutPoint::new(tx.compute_txid(), vout);
                            sheet.save(
                                &mut atomic.derive(
                                    &table
                                        .OUTPOINT_TO_RUNES
                                        .select(&consensus_encode(&outpoint)?),
                                ),
                                false,
                            );
                        }
                    }
                    if stone.is_message() {
                        stone.process_message::<T>(
                            &mut atomic.derive(&IndexPointer::default()),
                            tx,
                            txindex,
                            block,
                            height,
                            runestone_output_index,
                            (tx.output.len() as u32) + 1 + (i as u32),
                            &mut proto_balances_by_output,
                            unallocated_to,
                        )?;
                    }
                    Ok(())
                })
                .collect::<Result<()>>()?;
            Self::save_balances(
                &mut atomic.derive(&IndexPointer::default()),
                &table,
                tx,
                &mut proto_balances_by_output,
            )?;
        }
        Ok(())
    }

    pub fn index_block<T: MessageContext>(block: Block, height: u64) -> Result<()> {
        initialized_protocol_index().map_err(|e| anyhow!(e.to_string()))?;
        add_to_indexable_protocols(T::protocol_tag()).map_err(|e| anyhow!(e.to_string()))?;
        tables::RUNES
            .HEIGHT_TO_BLOCKHASH
            .select_value::<u64>(height)
            .set(Arc::new(consensus_encode(&block.block_hash())?));
        tables::RUNES
            .BLOCKHASH_TO_HEIGHT
            .select(&consensus_encode(&block.block_hash())?)
            .set_value::<u64>(height);
        Self::index_spendables(&block.txdata)?;
        Self::index_transaction_ids(&block, height)?;
        Self::index_outpoints(&block, height)?;
        Self::index_unspendables::<T>(&block, height)?;
        flush();
        Ok(())
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
