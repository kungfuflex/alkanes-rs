//! Native (indexer-scope) recycle capture for the `8:dead` recycle bin.
//!
//! flex's requirement (FB6, 2026-06-06): the *capture* side must NOT invoke
//! wasmi for every accidental burn — it is plain indexer code that writes to
//! `8:dead`'s storage + inventory. The WASM at `8:dead` (alkanes-std-recycle) is
//! only executed when someone actually *claims* (opcode 3).
//!
//! ## What this does
//!
//! After protorune indexing of a block, any protocol-tag (alkane) balance that
//! is still recorded at a *spent* input outpoint is "stranded": it was spent in
//! a transaction with no protostone, so `index_protostones` never ran to move or
//! clear it (a Runestone-bearing spend clears its inputs at
//! `protorune::index_protostones`; a bare-BTC spend does not). We sweep every
//! such stranded balance into the recycle bin:
//!   1. credit `8:dead`'s alkane **inventory** with the balance, and
//!   2. append it to the per-recipient **ledger** in `8:dead`'s storage at
//!      `/recycle/<script_pubkey>`, keyed by `default_output(tx).script_pubkey`
//!      (the EOA that *would* have received it), then
//!   3. clear the stranded input balance.
//!
//! Because (1) and (2) are written together and the claim WASM only ever emits
//! from inventory clamped to the ledger, a claim can never mint alkanes the bin
//! was not actually given (the core safety invariant; see audit).
//!
//! Non-EOA recipients (script-path / bare scripts) are left burned — this is the
//! intended garbage-collection of spam alkanes spent by non-alkanes wallets.

use crate::utils::alkane_inventory_pointer;
use alkanes_support::id::AlkaneId;
use anyhow::{anyhow, Result};
use bitcoin::blockdata::block::Block;
use bitcoin::{ScriptBuf, Transaction};
use metashrew_core::index_pointer::{AtomicPointer, IndexPointer};
use metashrew_support::index_pointer::KeyValuePointer;
use protorune::balance_sheet::{clear_chunked_balances, load_sheet_chunked};
use protorune::message::MessageContextParcel;
use protorune::tables::RuneTable;
use protorune_support::balance_sheet::{BalanceSheet, BalanceSheetOperations, ProtoruneRuneId};
use protorune_support::rune_transfer::RuneTransfer;
use protorune_support::utils::consensus_encode;
use std::sync::Arc;

/// The recycle bin alkane. `8:*` is the reserved namespace for precompiled
/// "life WASMs" embedded in the indexer; `0xdead` is the recycle bin.
pub const RECYCLE_ALKANE_ID: AlkaneId = AlkaneId {
    block: 8,
    tx: 0xdead,
};

/// Storage keyword for the per-recipient ledger inside `8:dead`. MUST match
/// `alkanes_std_recycle::RECYCLE_LEDGER_PREFIX` exactly — the WASM reads what
/// this writes.
const RECYCLE_LEDGER_PREFIX: &str = "/recycle/";

/// First non-OP_RETURN output index. Mirrors `protorune::default_output` and the
/// WASM's `default_output` so capture + claim agree on the ledger key.
pub(crate) fn default_output(tx: &Transaction) -> Option<usize> {
    tx.output
        .iter()
        .position(|o| !o.script_pubkey.is_op_return())
}

/// EOA = key-path spendable. MUST match the WASM's `is_eoa`.
pub(crate) fn is_eoa(spk: &ScriptBuf) -> bool {
    spk.is_p2tr() || spk.is_p2wpkh() || spk.is_p2pkh()
}

/// Ledger codec — flat LE (block, tx, value) u128 triples. MUST match
/// `alkanes_std_recycle::{encode,decode}_ledger`.
pub(crate) fn decode_ledger(raw: &[u8]) -> Vec<(ProtoruneRuneId, u128)> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i + 48 <= raw.len() {
        let block = u128::from_le_bytes(raw[i..i + 16].try_into().unwrap());
        let tx = u128::from_le_bytes(raw[i + 16..i + 32].try_into().unwrap());
        let value = u128::from_le_bytes(raw[i + 32..i + 48].try_into().unwrap());
        out.push((ProtoruneRuneId { block, tx }, value));
        i += 48;
    }
    out
}

pub(crate) fn encode_ledger(entries: &[(ProtoruneRuneId, u128)]) -> Vec<u8> {
    let mut out = Vec::with_capacity(entries.len() * 48);
    for (id, value) in entries {
        out.extend_from_slice(&id.block.to_le_bytes());
        out.extend_from_slice(&id.tx.to_le_bytes());
        out.extend_from_slice(&value.to_le_bytes());
    }
    out
}

/// Build the `8:dead` storage pointer for `/recycle/<spk>` exactly as the WASM's
/// `StoragePointer::from_keyword("/recycle/").select(spk)` resolves once the host
/// persists it under `/alkanes/<id>/storage/<key>` (see `pipe_storagemap_to`).
fn ledger_pointer(atomic: &mut AtomicPointer, spk: &[u8]) -> AtomicPointer {
    // inner key == the WASM StoragePointer key for /recycle/<spk>. StoragePointer
    // and IndexPointer share the KeyValuePointer keyword/select impls, so this is
    // byte-identical to what the contract writes/reads.
    let inner_key: Vec<u8> = IndexPointer::from_keyword(RECYCLE_LEDGER_PREFIX)
        .select(&spk.to_vec())
        .unwrap()
        .as_ref()
        .clone();
    let id_bytes: Vec<u8> = RECYCLE_ALKANE_ID.into();
    atomic
        .derive(&IndexPointer::default())
        .keyword("/alkanes/")
        .select(&id_bytes)
        .keyword("/storage/")
        .select(&inner_key)
}

/// Credit `8:dead`'s inventory with `value` of alkane `what` (so the claim WASM
/// can legitimately transfer it out of inventory — no minting).
fn credit_inventory(atomic: &mut AtomicPointer, what: &ProtoruneRuneId, value: u128) {
    let what_id: AlkaneId = AlkaneId {
        block: what.block,
        tx: what.tx,
    };
    // Build the balance pointer directly rather than via `balance_pointer`, whose
    // append-on-existing-balance side-effect would add a duplicate `/inventory/`
    // entry on every subsequent credit (audit H). Register `what` in 8:dead's
    // inventory index exactly once — on first credit (prev == 0).
    let what_bytes: Vec<u8> = what_id.into();
    let who_bytes: Vec<u8> = RECYCLE_ALKANE_ID.into();
    let mut bp = atomic
        .derive(&IndexPointer::default())
        .keyword("/alkanes/")
        .select(&what_bytes)
        .keyword("/balances/")
        .select(&who_bytes);
    let prev = bp.get_value::<u128>();
    bp.set_value::<u128>(prev.saturating_add(value));
    if prev == 0 {
        alkane_inventory_pointer(&RECYCLE_ALKANE_ID).append(Arc::new(what_bytes));
    }
}

/// Sweep stranded protocol-tag balances in `block` into the recycle bin.
/// Runs once per block, after `Protorune::index_block`. Idempotent on reindex
/// (it clears each input it sweeps).
pub fn capture_block(block: &Block, height: u64, protocol_tag: u128) -> Result<()> {
    let table = RuneTable::for_protocol(protocol_tag);
    for tx in block.txdata.iter() {
        // Recipient = first non-OP_RETURN output, EOA only. Compute once.
        let recipient: Option<ScriptBuf> = default_output(tx).and_then(|v| {
            let spk = tx.output[v].script_pubkey.clone();
            if is_eoa(&spk) {
                Some(spk)
            } else {
                None
            }
        });

        for input in tx.input.iter() {
            let mut atomic = AtomicPointer::default();
            let key = consensus_encode(&input.previous_output)?;
            // v3 stores outpoint balances chunked — read/clear with the chunked
            // family so capture sees real (protocol-written) strandings.
            let sheet: BalanceSheet<AtomicPointer> =
                load_sheet_chunked(&atomic.derive(&table.OUTPOINT_TO_RUNES.select(&key)));
            let balances = sheet.balances();
            if balances.is_empty() {
                continue; // not stranded (already consumed by a protostone, or empty)
            }

            match &recipient {
                None => {
                    // No EOA recipient: leave burned (spam GC). Clear the ghost so
                    // protorunesbyoutpoint stops reporting a spent outpoint.
                    clear_chunked_balances(
                        &mut atomic.derive(&table.OUTPOINT_TO_RUNES.select(&key)),
                        height as u32,
                    );
                }
                Some(spk) => {
                    // 1+2: credit inventory + append to ledger, atomically.
                    let mut ledger = decode_ledger(ledger_pointer(&mut atomic, spk.as_bytes()).get().as_ref());
                    for (rune, amount) in balances.iter() {
                        if *amount == 0 {
                            continue;
                        }
                        credit_inventory(&mut atomic, rune, *amount);
                        match ledger.iter_mut().find(|(id, _)| id == rune) {
                            Some((_, v)) => *v = v.saturating_add(*amount),
                            None => ledger.push((rune.clone(), *amount)),
                        }
                    }
                    ledger_pointer(&mut atomic, spk.as_bytes())
                        .set(Arc::new(encode_ledger(&ledger)));
                    // 3: clear the stranded input balance.
                    clear_chunked_balances(
                        &mut atomic.derive(&table.OUTPOINT_TO_RUNES.select(&key)),
                        height as u32,
                    );
                }
            }
            atomic.commit();
        }
    }
    Ok(())
}

/// Native recycle CLAIM handler (alkanes layer), invoked from `handle_message`
/// for a `8:dead` opcode-3 cellpack.
///
/// SECURITY (fixes the ksyao theft PoC, PR #266): the release MUST be bound to
/// the *actual payout destination*. A wasm claim cannot see `parcel.pointer`
/// (the protostone output the response is routed to), so it had to key the
/// ledger off `default_output(tx)` — letting an attacker name a victim's spk at
/// vout 0 while pointing the payout at their own output. Here we key the ledger
/// off the spk of `tx.output[parcel.pointer]` — the address that will actually
/// receive the payout — so a claim can only ever release the ledger of the
/// address it pays. Naming a victim's spk only returns funds *to the victim*;
/// theft and grief-burn are both impossible.
pub fn handle_claim(
    parcel: &MessageContextParcel,
) -> Result<(Vec<RuneTransfer>, BalanceSheet<AtomicPointer>)> {
    // A claim NEVER aborts indexing: an invalid/empty claim is a graceful no-op
    // (empty outgoing, runtime passed through) so a malformed claim tx can't halt
    // the indexer or be used as a refund-grief vector. Only a valid claim whose
    // payout output owns a non-empty ledger releases anything.
    let runtime = (*parcel.runtime_balances).clone();
    let noop = Ok((Vec::<RuneTransfer>::new(), runtime.clone()));

    let tx = &parcel.transaction;
    // The payout destination = the protostone pointer's output. Must be a real,
    // non-OP_RETURN, EOA (key-path) output. Shadow vouts / OP_RETURN / out-of-
    // range pointers have no legitimate recycle payout → no-op.
    let p = parcel.pointer as usize;
    if p >= tx.output.len() {
        return noop;
    }
    let spk = tx.output[p].script_pubkey.clone();
    if spk.is_op_return() || !is_eoa(&spk) {
        return noop;
    }

    let mut atomic = parcel.atomic.derive(&IndexPointer::default());
    let owed = decode_ledger(ledger_pointer(&mut atomic, spk.as_bytes()).get().as_ref());
    if owed.is_empty() {
        // No ledger for the payout spk — e.g. an attacker pointing the payout at
        // their own (empty) output. Release nothing, touch nothing.
        return noop;
    }

    let mut outgoing: Vec<RuneTransfer> = Vec::with_capacity(owed.len());
    for (rune, amount) in owed.iter() {
        if *amount == 0 {
            continue;
        }
        // Debit 8:dead inventory, CLAMPED to the held balance so a ledger /
        // inventory desync can never over-release (anti-mint backstop). Never
        // errors — clamps and proceeds.
        let what_id = AlkaneId { block: rune.block, tx: rune.tx };
        let what_bytes: Vec<u8> = what_id.into();
        let who_bytes: Vec<u8> = RECYCLE_ALKANE_ID.into();
        let mut bp = atomic
            .derive(&IndexPointer::default())
            .keyword("/alkanes/")
            .select(&what_bytes)
            .keyword("/balances/")
            .select(&who_bytes);
        let held = bp.get_value::<u128>();
        let release = (*amount).min(held);
        if release == 0 {
            continue;
        }
        bp.set_value::<u128>(held - release);
        outgoing.push(RuneTransfer {
            id: what_id.into(),
            value: release,
        });
    }

    // Single-use: zero the ledger so the claim can't be replayed.
    ledger_pointer(&mut atomic, spk.as_bytes()).set(Arc::new(vec![]));
    atomic.commit();

    // Runtime passed through unchanged; `outgoing` is routed to parcel.pointer
    // (the payout output whose ledger we just released) by `reconcile`.
    Ok((outgoing, runtime))
}

/// True if a cellpack targets the recycle bin's claim opcode (3).
pub fn is_recycle_claim(target: &AlkaneId, inputs: &[u128]) -> bool {
    target.block == RECYCLE_ALKANE_ID.block
        && target.tx == RECYCLE_ALKANE_ID.tx
        && inputs.first() == Some(&3u128)
}
