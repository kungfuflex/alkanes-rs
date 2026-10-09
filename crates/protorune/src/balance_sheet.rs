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

/// Maximum entry count a VIEW read will walk in a stored balance sheet.
///
/// A healthy outpoint carries a handful of protorune balances; a stored
/// `/runes` length counter in the thousands-and-up is a corruption signature,
/// not a real wallet. Incident (mainnet, 2026-08-22): the protocol-1
/// OUTPOINT_TO_RUNES record for outpoint 2a1538bf…2e28:0 (h961009, written
/// during the 2026-08-07 reconcile-rollback window) carries a pathological
/// entry list; `load_sheet`'s unbounded walk — two host KV reads per entry —
/// made `protorunesbyoutpoint` HANG until the JSON-RPC layer's 60s timeout,
/// on every request, pinning a view-runtime permit each time. Sibling vouts
/// of the same tx (healthy records) answered in <1s.
pub const VIEW_SHEET_MAX_ENTRIES: u32 = 4096;

/// Bounded, fallible variant of [`load_sheet`] for VIEW paths only.
///
/// Refuses (fast, with a diagnostic error the JSON-RPC layer surfaces as a
/// normal view error) instead of walking a corrupt stored length. This turns
/// the hang class above into the same fast-error class as the known
/// `option::unwrap_failed` per-outpoint panics, which downstream consumers
/// already tolerate per-outpoint.
///
/// CONSENSUS SAFETY: the indexer keeps calling the unbounded [`load_sheet`] —
/// this function must only ever be reached from view functions. Never swap it
/// into block-application paths: refusing a sheet during indexing would
/// change indexed state.
pub fn load_sheet_bounded<T: KeyValuePointer + Clone>(
    ptr: &T,
    max_entries: u32,
) -> Result<BalanceSheet<T>> {
    let runes_ptr = ptr.keyword("/runes");
    let balances_ptr = ptr.keyword("/balances");
    let length = runes_ptr.length();
    if length > max_entries {
        return Err(anyhow!(
            "balance sheet entry count {} exceeds view cap {} — corrupt stored record, refusing unbounded walk",
            length,
            max_entries
        ));
    }
    let mut result = BalanceSheet::default();
    for i in 0..length {
        let rune = ProtoruneRuneId::from(runes_ptr.select_index(i).get());
        let balance = balances_ptr.select_index(i).get_value::<u128>();
        result.set(&rune, balance);
    }
    Ok(result)
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

#[cfg(test)]
mod view_bound_tests {
    use super::*;
    use protorune_support::balance_sheet::BalanceSheetOperations;

    #[test]
    fn load_sheet_bounded_matches_load_sheet_on_healthy_records() {
        metashrew_core::clear();
        let ptr = IndexPointer::from_keyword("/test/bounded-ok");
        let mut sheet: BalanceSheet<IndexPointer> = BalanceSheet::default();
        sheet.set(&ProtoruneRuneId::new(2, 0), 1_000);
        sheet.set(&ProtoruneRuneId::new(32, 7), 5);
        sheet.save(&ptr, false);

        let bounded = load_sheet_bounded(&ptr, VIEW_SHEET_MAX_ENTRIES).unwrap();
        let unbounded = load_sheet(&ptr);
        assert_eq!(bounded.balances(), unbounded.balances());
        assert_eq!(bounded.get_cached(&ProtoruneRuneId::new(2, 0)), 1_000);
        assert_eq!(bounded.get_cached(&ProtoruneRuneId::new(32, 7)), 5);
    }

    #[test]
    fn load_sheet_bounded_boundary_at_view_cap() {
        metashrew_core::clear();
        // Exactly VIEW_SHEET_MAX_ENTRIES real entries: loads fine.
        let ptr = IndexPointer::from_keyword("/test/bounded-boundary");
        let mut sheet: BalanceSheet<IndexPointer> = BalanceSheet::default();
        for i in 0..VIEW_SHEET_MAX_ENTRIES {
            sheet.set(&ProtoruneRuneId::new(2, i as u128), 1);
        }
        sheet.save(&ptr, false);
        let loaded = load_sheet_bounded(&ptr, VIEW_SHEET_MAX_ENTRIES).unwrap();
        assert_eq!(loaded.balances().len(), VIEW_SHEET_MAX_ENTRIES as usize);

        // One past the cap (forged counter on the same record): refused
        // before any entry read.
        ptr.keyword("/runes")
            .length_key()
            .set_value::<u32>(VIEW_SHEET_MAX_ENTRIES + 1);
        let err = load_sheet_bounded(&ptr, VIEW_SHEET_MAX_ENTRIES).unwrap_err();
        assert!(err.to_string().contains("exceeds view cap"));
    }

    #[test]
    fn view_outpoint_response_refuses_corrupt_record_instead_of_walking() {
        // End-to-end through the ACTUAL converted view path (the function
        // that hung on mainnet outpoint 2a1538bf…2e28:0): a protocol-1
        // OUTPOINT_TO_RUNES record with a forged length must surface as a
        // fast view Err — the same error class as the known per-outpoint
        // panics — never an unbounded walk.
        use bitcoin::hashes::Hash;
        metashrew_core::clear();
        let outpoint = bitcoin::OutPoint {
            txid: bitcoin::Txid::all_zeros(),
            vout: 0,
        };
        let outpoint_bytes = crate::view::outpoint_to_bytes(&outpoint).unwrap();
        crate::tables::RuneTable::for_protocol(1)
            .OUTPOINT_TO_RUNES
            .select(&outpoint_bytes)
            .keyword("/runes")
            .length_key()
            .set_value::<u32>(u32::MAX);

        let err = crate::view::protorune_outpoint_to_outpoint_response(&outpoint, 1)
            .unwrap_err();
        assert!(
            err.to_string().contains("exceeds view cap"),
            "expected the bounded-loader refusal, got: {err}"
        );
    }

    #[test]
    fn load_sheet_bounded_refuses_corrupt_length_fast() {
        metashrew_core::clear();
        // Forge the 2026-08-22 hang-incident shape: a stored /runes/length
        // far beyond any real wallet, with no matching entries. The
        // unbounded walker would spin for length iterations (two host reads
        // each); the bounded loader must refuse before the first read.
        let ptr = IndexPointer::from_keyword("/test/bounded-corrupt");
        ptr.keyword("/runes")
            .length_key()
            .set_value::<u32>(u32::MAX);

        let err = load_sheet_bounded(&ptr, VIEW_SHEET_MAX_ENTRIES).unwrap_err();
        assert!(err.to_string().contains("exceeds view cap"));
    }
}
