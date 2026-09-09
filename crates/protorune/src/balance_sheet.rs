use anyhow::{anyhow, Result};
use metashrew_core::index_pointer::{AtomicPointer, IndexPointer};
use metashrew_support::index_pointer::KeyValuePointer;
use protorune_support::balance_sheet::{BalanceSheet, BalanceSheetOperations, ProtoruneRuneId};
use protorune_support::proto;
use protorune_support::rune_transfer::{increase_balances_using_sheet, RuneTransfer};
use prost::Message;
use std::collections::BTreeMap;

#[allow(unused_imports)]
use {
    metashrew_core::{println, stdio::stdout},
    std::fmt::Write,
};

// use metashrew_core::{println, stdio::stdout};
// use std::fmt::Write;
//

pub trait PersistentRecord: BalanceSheetOperations {
    fn save<T: KeyValuePointer>(&self, ptr: &T, is_cenotaph: bool) {
        let runes_ptr = ptr.keyword("/runes");
        let balances_ptr = ptr.keyword("/balances");
        let runes_to_balances_ptr = ptr.keyword("/id_to_balance");

        for (rune, balance) in self.balances() {
            if *balance != 0u128 && !is_cenotaph {
                let rune_bytes: Vec<u8> = (*rune).into();
                runes_ptr.append(rune_bytes.clone().into());

                balances_ptr.append_value::<u128>(*balance);

                runes_to_balances_ptr
                    .select(&rune_bytes)
                    .set_value::<u128>(*balance);
            }
        }
    }
    fn save_index<T: KeyValuePointer>(
        &self,
        rune: &ProtoruneRuneId,
        ptr: &T,
        is_cenotaph: bool,
    ) -> Result<()> {
        let runes_ptr = ptr.keyword("/runes");
        let balances_ptr = ptr.keyword("/balances");
        let runes_to_balances_ptr = ptr.keyword("/id_to_balance");
        let balance = self
            .balances()
            .get(rune)
            .ok_or(anyhow!("no balance found"))?;
        if *balance != 0u128 && !is_cenotaph {
            let rune_bytes: Vec<u8> = (*rune).into();
            runes_ptr.append(rune_bytes.clone().into());
            balances_ptr.append_value::<u128>(*balance);
            runes_to_balances_ptr
                .select(&rune_bytes)
                .set_value::<u128>(*balance);
        }

        Ok(())
    }
}

pub trait Mintable {
    fn mintable_in_protocol(&self, atomic: &mut AtomicPointer) -> bool;
}

impl Mintable for ProtoruneRuneId {
    fn mintable_in_protocol(&self, atomic: &mut AtomicPointer) -> bool {
        // if it was not etched via runes-like etch in the Runestone and protoburned, then it is considered mintable
        atomic
            .derive(
                &IndexPointer::from_keyword("/etching/byruneid/").select(&(self.clone().into())),
            )
            .get()
            .len()
            == 0
    }
}

pub trait OutgoingRunes<P: KeyValuePointer + Clone> {
    fn reconcile(
        &self,
        atomic: &mut AtomicPointer,
        balances_by_output: &mut BTreeMap<u32, BalanceSheet<P>>,
        vout: u32,
        pointer: u32,
    ) -> Result<()>;
}

pub trait MintableDebit<P: KeyValuePointer + Clone + std::fmt::Debug> {
    fn debit_mintable(&mut self, sheet: &BalanceSheet<P>, atomic: &mut AtomicPointer)
        -> Result<()>;
}

impl<P: KeyValuePointer + Clone + std::fmt::Debug> MintableDebit<P> for BalanceSheet<P> {
    // logically, this will debit the input sheet from the self sheet, and if it would produce a negative value
    // it will check if the rune id is mintable (if it was etched and protoburned or if it is an alkane).
    // if it is mintable, we assume the extra amount was minted and do not decrease the amount.
    // NOTE: if it was a malicious case where an alkane was minted by another alkane, this will not check for that.
    // such a case should be checked in debit_balances in src/utils.rs
    fn debit_mintable(
        &mut self,
        sheet: &BalanceSheet<P>,
        atomic: &mut AtomicPointer,
    ) -> Result<()> {
        for (rune, balance) in sheet.balances() {
            let mut amount = *balance;
            let current = self.get(&rune);
            if amount > current {
                if rune.mintable_in_protocol(atomic) {
                    amount = current;
                } else {
                    return Err(anyhow!("balance underflow during debit_mintable"));
                }
            }
            self.decrease(rune, amount);
        }
        Ok(())
    }
}
impl<P: KeyValuePointer + Clone + std::fmt::Debug> OutgoingRunes<P>
    for (Vec<RuneTransfer>, BalanceSheet<P>)
{
    fn reconcile(
        &self,
        atomic: &mut AtomicPointer,
        balances_by_output: &mut BTreeMap<u32, BalanceSheet<P>>,
        vout: u32,
        pointer: u32,
    ) -> Result<()> {
        let runtime_initial = balances_by_output
            .get(&u32::MAX)
            .map(|v| v.clone())
            .unwrap_or_else(|| BalanceSheet::default());
        let incoming_initial = balances_by_output
            .get(&vout)
            .ok_or("")
            .map_err(|_| anyhow!("balance sheet not found"))?
            .clone();
        let mut initial = BalanceSheet::merge(&incoming_initial, &runtime_initial)?;

        // self.0 is the amount to forward to the pointer
        // self.1 is the amount to put into the runtime balance
        let outgoing: BalanceSheet<P> = self.0.clone().try_into()?;
        let outgoing_runtime = self.1.clone();

        // we want to subtract outgoing and the outgoing runtime balance
        // amount from the initial amount
        initial.debit_mintable(&outgoing, atomic)?;
        initial.debit_mintable(&outgoing_runtime, atomic)?;
        for (id, balance) in initial.balances() {
            if *balance != 0 {
                println!("BIG ERROR: NONZERO {:?} {}", id, balance);
            }
        }

        // now lets update balances_by_output to correct values

        // SECURITY: `balances_by_output` is the NON-transactional in-memory map;
        // `atomic.rollback()` does NOT unwind it. Forwarding `outgoing` onto the
        // `pointer` output (below) can overflow when `pointer` already holds a
        // near-MAX balance of a rune `outgoing` also carries. If we `remove(&vout)`
        // FIRST and then overflowed, `process_message`'s reconcile-Err branch would
        // call `refund_to_refund_pointer` against an already-removed `vout`, refund
        // nothing, and silently BURN the caller's incoming balance. Validate the
        // whole forward up front — before any mutation — so an overflow leaves the
        // map (incl. `vout`) untouched and the refund path stays whole. `pipe` is
        // itself all-or-nothing, so this pre-check never rejects a case that would
        // otherwise have succeeded; the success-path ordering is unchanged.
        {
            let target = balances_by_output.get(&pointer);
            for (rune, amount) in outgoing.balances() {
                let current = target.map(|s| s.get(rune)).unwrap_or(0);
                current.checked_add(*amount).ok_or("").map_err(|_| {
                    anyhow!(format!(
                        "overflow error during balance sheet increase, current({}) + additional({})",
                        current, amount
                    ))
                })?;
            }
        }

        // first remove the protomessage vout balances
        balances_by_output.remove(&vout);

        // increase the pointer by the outgoing runes balancesheet
        increase_balances_using_sheet(balances_by_output, &outgoing, pointer)?;

        // set the runtime to the ending runtime balance sheet
        // note that u32::MAX is the runtime vout
        balances_by_output.insert(u32::MAX, outgoing_runtime);
        Ok(())
    }
}

pub fn load_sheet<T: KeyValuePointer + Clone>(ptr: &T) -> BalanceSheet<T> {
    let runes_ptr = ptr.keyword("/runes");
    let balances_ptr = ptr.keyword("/balances");
    let length = runes_ptr.length();
    let mut result = BalanceSheet::default();

    for i in 0..length {
        let rune = ProtoruneRuneId::from(runes_ptr.select_index(i).get());
        let balance = balances_ptr.select_index(i).get_value::<u128>();
        result.set(&rune, balance);
    }
    result
}

pub fn clear_balances<T: KeyValuePointer>(ptr: &T) {
    let runes_ptr = ptr.keyword("/runes");
    let balances_ptr = ptr.keyword("/balances");
    let length = runes_ptr.length();
    let runes_to_balances_ptr = ptr.keyword("/id_to_balance");

    for i in 0..length {
        balances_ptr.select_index(i).set_value::<u128>(0);
        let rune = balances_ptr.select_index(i).get();
        runes_to_balances_ptr.select(&rune).set_value::<u128>(0);
    }
}

// v3 chunked-outpoint API
// ============================================================================
//
// The legacy `save` / `load_sheet` / `clear_balances` family writes a logical
// balance-sheet to N keys under three sub-pointers (`/runes`, `/balances`,
// `/id_to_balance`) — one append per rune balance per outpoint, plus a
// `/length` increment. On a mainnet block with thousands of touched
// outpoints this is the dominant write source.
//
// The chunked path collapses one outpoint's whole balance sheet into a
// single protobuf-serialized chunk written under the parent pointer via
// the v10 metashrew `set_chunk` API — one chain entry per outpoint per
// block, regardless of the number of balances inside. Reads are one
// `get_chunk` + protobuf decode.
//
// Used only for `OUTPOINT_TO_RUNES.select(outpoint)`. Other balance-sheet
// callers (`RUNTIME_BALANCE`, `CAP`, address-keyed legacy callers) keep
// the multi-key `save`/`load_sheet` for now — the chunked path was
// motivated by the per-outpoint write explosion and the other callers
// don't have that profile.

/// Serialize a balance sheet to the v3 chunked protobuf and write it as a
/// single chunk at the current height.
///
/// If `is_cenotaph` is true, the sheet is treated as empty (all balances are
/// burned), and the chunk is written as a zero-entries protobuf — preserving
/// the chain semantics that `load_sheet_chunked` returns an empty sheet for
/// cenotaph-burned outpoints, same as the legacy `save(is_cenotaph=true)`.
///
/// **Overwrite semantics** — the chunk REPLACES any prior chunk at the same
/// outpoint. Safe for the normal block-apply path because `balances_by_output`
/// already represents the full per-output balance computed from a single tx's
/// edicts. Unsafe when MULTIPLE callers write to the same outpoint within the
/// same block (e.g. genesis init where `setup_diesel` + `setup_frsigil`
/// each call `save_chunked` against `GENESIS_OUTPOINT` — the second wipes
/// the first). For those call sites use [`save_chunked_merging`] instead.
pub fn save_chunked<P, T>(sheet: &BalanceSheet<P>, ptr: &mut T, is_cenotaph: bool)
where
    P: KeyValuePointer + Clone,
    T: KeyValuePointer,
{
    let proto_sheet: proto::protorune::BalanceSheet = if is_cenotaph {
        proto::protorune::BalanceSheet {
            entries: Vec::new(),
            spent_at_height: None,
        }
    } else {
        sheet.clone().into()
    };
    let encoded = proto_sheet.encode_to_vec();
    ptr.set_chunk(&encoded);
}

/// Like [`save_chunked`] but APPENDS the new balances to whatever chunk
/// already exists at `ptr`. Required at genesis where multiple
/// `setup_*` functions each contribute a different alkane to the
/// shared `GENESIS_OUTPOINT` — using plain `save_chunked` would let
/// the second write wipe out the first.
///
/// `is_cenotaph=true` bypasses the merge and writes an empty chunk
/// (matching the cenotaph contract of [`save_chunked`]).
///
/// Returns `Err` only if balance addition would overflow `u128` — a
/// theoretical concern only; genesis premines are well below that
/// bound.
pub fn save_chunked_merging<P, T>(
    sheet: &BalanceSheet<P>,
    ptr: &mut T,
    is_cenotaph: bool,
) -> Result<()>
where
    P: KeyValuePointer + Clone,
    T: KeyValuePointer + Clone,
{
    if is_cenotaph {
        save_chunked(sheet, ptr, true);
        return Ok(());
    }
    // Read existing chunk (empty BalanceSheet if absent).
    let existing: BalanceSheet<T> = load_sheet_chunked(ptr);
    // Start from the new sheet (cheap clone — cached BTreeMap), then
    // add every (rune, balance) from existing. `increase` does
    // saturating-ish add with overflow check; we propagate on error.
    let mut combined: BalanceSheet<P> = sheet.clone();
    for (rune, value) in existing.balances() {
        combined.increase(rune, *value)?;
    }
    save_chunked(&combined, ptr, false);
    Ok(())
}

/// Read the chunked balance sheet for an outpoint at the current read
/// height. Returns an empty sheet if no chunk has been written.
///
/// Note: the returned sheet has `load_ptrs = Vec::new()` because the chunked
/// encoding doesn't reference per-balance pointers. Callers that need to
/// re-save modifications should call `save_chunked` again rather than
/// mutate-and-save.
pub fn load_sheet_chunked<T: KeyValuePointer + Clone>(ptr: &T) -> BalanceSheet<T> {
    match ptr.get_chunk() {
        Some(bytes) => match proto::protorune::BalanceSheet::decode(bytes.as_slice()) {
            Ok(proto_sheet) => proto_sheet.into(),
            Err(_) => BalanceSheet::default(),
        },
        None => BalanceSheet::default(),
    }
}

/// Read the raw `spent_at_height` field from the chunked balance sheet for
/// an outpoint at the current read height. Returns `None` if the outpoint
/// has never had a chunk written OR if the chunk has the field unset.
///
/// Used by `crates/alkanes/src/unwrap.rs::is_payment_unfulfilled` to
/// determine whether an outpoint has been consumed as a transaction input.
pub fn get_chunked_spent_at_height<T: KeyValuePointer + Clone>(ptr: &T) -> Option<u32> {
    let bytes = ptr.get_chunk()?;
    let proto_sheet = proto::protorune::BalanceSheet::decode(bytes.as_slice()).ok()?;
    proto_sheet.spent_at_height
}

/// Mark an outpoint's chunk as spent at the given height. Reads the current
/// chunk, sets `spent_at_height = Some(h)`, and writes the chunk back at
/// the current height. If no chunk exists for this outpoint, writes a
/// minimal "empty + spent" chunk (no balance entries, just the spent
/// marker).
///
/// This is the v3 replacement for the v2 `OUTPOINT_SPENDABLE_BY[outpoint]
/// .len() > 1` spentness probe, which was an incidental side-effect of the
/// (now removed) address index.
pub fn set_chunked_spent_at_height<T: KeyValuePointer + Clone>(ptr: &mut T, height: u32) {
    let mut proto_sheet = match ptr.get_chunk() {
        Some(bytes) => proto::protorune::BalanceSheet::decode(bytes.as_slice())
            .unwrap_or_else(|_| proto::protorune::BalanceSheet {
                entries: Vec::new(),
                spent_at_height: None,
            }),
        None => proto::protorune::BalanceSheet {
            entries: Vec::new(),
            spent_at_height: None,
        },
    };
    proto_sheet.spent_at_height = Some(height);
    let encoded = proto_sheet.encode_to_vec();
    ptr.set_chunk(&encoded);
}

/// Clear the chunked balance sheet for an outpoint at the current height.
/// Writes a zero-entries protobuf with `spent_at_height = Some(height)`.
///
/// This is the v3 replacement for the legacy `clear_balances` when the
/// caller knows the outpoint is being consumed (e.g. by an input in
/// `index_unspendables` or `index_protostones`).
pub fn clear_chunked_balances<T: KeyValuePointer>(ptr: &mut T, height: u32) {
    let proto_sheet = proto::protorune::BalanceSheet {
        entries: Vec::new(),
        spent_at_height: Some(height),
    };
    let encoded = proto_sheet.encode_to_vec();
    ptr.set_chunk(&encoded);
}

impl<P: KeyValuePointer + Clone + std::fmt::Debug> PersistentRecord for BalanceSheet<P> {}
