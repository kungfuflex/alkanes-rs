use anyhow::{anyhow, Result};
use metashrew_core::index_pointer::{AtomicPointer, IndexPointer};
use metashrew_support::index_pointer::KeyValuePointer;
use protorune_support::balance_sheet::{BalanceSheet, BalanceSheetOperations, ProtoruneRuneId};
use protorune_support::rune_transfer::{increase_balances_using_sheet, RuneTransfer};
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

/// Hard ceiling on balance-sheet entries a single view call will read.
///
/// `load_sheet` trusts the stored `/runes` length and does two host reads per
/// entry, so a view over a pathological outpoint runs for as long as that
/// length allows while holding one of metashrew's finite view permits. Views
/// go through `load_sheet_page` instead, which reads at most this many entries.
/// The indexer keeps using `load_sheet`: consensus needs the whole sheet.
pub const MAX_VIEW_SHEET_ENTRIES: u32 = 1000;

/// One page of a stored balance sheet, read by `load_sheet_page`.
pub struct SheetPage<T: KeyValuePointer + Clone> {
    pub sheet: BalanceSheet<T>,
    /// Stored `/runes` length — the size of the whole sheet.
    pub total_entries: u32,
    /// Offset of the next page, or 0 when this page reaches the end.
    pub next_offset: u32,
}

/// Bounded `load_sheet` for views: reads stored entries `[offset, offset + n)`
/// where `n` is `limit` clamped to `1..=MAX_VIEW_SHEET_ENTRIES` (0 = the cap).
/// Pages follow storage order; entries within a page are keyed by rune id as
/// usual, so a rune stored twice collapses within a page but not across pages.
pub fn load_sheet_page<T: KeyValuePointer + Clone>(
    ptr: &T,
    offset: u32,
    limit: u32,
) -> SheetPage<T> {
    let runes_ptr = ptr.keyword("/runes");
    let balances_ptr = ptr.keyword("/balances");
    let total_entries = runes_ptr.length();
    let limit = if limit == 0 {
        MAX_VIEW_SHEET_ENTRIES
    } else {
        limit.min(MAX_VIEW_SHEET_ENTRIES)
    };
    let start = offset.min(total_entries);
    let end = start.saturating_add(limit).min(total_entries);
    let mut sheet = BalanceSheet::default();

    for i in start..end {
        let rune = ProtoruneRuneId::from(runes_ptr.select_index(i).get());
        let balance = balances_ptr.select_index(i).get_value::<u128>();
        sheet.set(&rune, balance);
    }
    SheetPage {
        sheet,
        total_entries,
        next_offset: if end < total_entries { end } else { 0 },
    }
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

impl<P: KeyValuePointer + Clone + std::fmt::Debug> PersistentRecord for BalanceSheet<P> {}
