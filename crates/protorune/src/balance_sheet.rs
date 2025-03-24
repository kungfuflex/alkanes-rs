use anyhow::{anyhow, Result};
use metashrew::index_pointer::{AtomicPointer, IndexPointer};
use metashrew_support::index_pointer::KeyValuePointer;
use protorune_support::balance_sheet::{BalanceSheet, LazyBalanceSheet, ProtoruneRuneId};
use protorune_support::rune_transfer::{increase_balances_using_sheet, RuneTransfer};
use std::collections::HashMap;

use metashrew::{println, stdio::stdout};
use std::fmt::Write;

// use metashrew::{println, stdio::stdout};
// use std::fmt::Write;
//

pub trait PersistentRecord {
    fn save<T: KeyValuePointer>(&self, ptr: &T, is_cenotaph: bool) {
        let runes_ptr = ptr.keyword("/runes");
        let balances_ptr = ptr.keyword("/balances");

        for (rune, balance) in self.balances() {
            if *balance != 0u128 && !is_cenotaph {
                runes_ptr.append((*rune).into());

                balances_ptr.append_value::<u128>(*balance);
            }
        }
    }
    fn balances(&self) -> &HashMap<ProtoruneRuneId, u128>;
    fn save_index<T: KeyValuePointer>(
        &self,
        rune: &ProtoruneRuneId,
        ptr: &T,
        is_cenotaph: bool,
    ) -> Result<()> {
        let runes_ptr = ptr.keyword("/runes");
        let balances_ptr = ptr.keyword("/balances");
        let balance = self
            .balances()
            .get(rune)
            .ok_or(anyhow!("no balance found"))?;
        if *balance != 0u128 && !is_cenotaph {
            runes_ptr.append((*rune).into());
            balances_ptr.append_value::<u128>(*balance);
        }

        Ok(())
    }
}

pub trait Mintable {
    fn mintable_in_protocol(&self, atomic: &mut AtomicPointer) -> bool;
}

impl Mintable for ProtoruneRuneId {
    fn mintable_in_protocol(&self, atomic: &mut AtomicPointer) -> bool {
        atomic
            .derive(
                &IndexPointer::from_keyword("/etching/byruneid/").select(&(self.clone().into())),
            )
            .get()
            .len()
            == 0
    }
}

pub trait OutgoingRunes {
    fn reconcile(
        &self,
        atomic: &mut AtomicPointer,
        balances_by_output: &mut HashMap<u32, BalanceSheet>,
        vout: u32,
        pointer: u32,
        refund_pointer: u32,
    ) -> Result<()>;
}

pub trait MintableDebit {
    fn debit_mintable(&mut self, sheet: &BalanceSheet, atomic: &mut AtomicPointer) -> Result<()>;
}

impl MintableDebit for BalanceSheet {
    fn debit_mintable(&mut self, sheet: &BalanceSheet, atomic: &mut AtomicPointer) -> Result<()> {
        for (rune, balance) in &sheet.balances {
            let mut amount = *balance;
            let current = self.get(&rune);
            if sheet.get(&rune) > current {
                if rune.mintable_in_protocol(atomic) {
                    amount = current;
                } else {
                    return Err(anyhow!("balance underflow during debit"));
                }
            }
            self.decrease(rune, amount);
        }
        Ok(())
    }
}
// This implementation is kept for backward compatibility
impl OutgoingRunes for (Vec<RuneTransfer>, BalanceSheet) {
    fn reconcile(
        &self,
        atomic: &mut AtomicPointer,
        balances_by_output: &mut HashMap<u32, BalanceSheet>,
        vout: u32,
        pointer: u32,
        refund_pointer: u32,
    ) -> Result<()> {
        // Convert BalanceSheet to LazyBalanceSheet for compatibility
        let lazy_balance_sheet = LazyBalanceSheet::from_balance_sheet(&self.1, "/runtime_balances".to_string());
        
        // Use the LazyBalanceSheet implementation
        let as_lazy = (self.0.clone(), lazy_balance_sheet);
        as_lazy.reconcile(atomic, balances_by_output, vout, pointer, refund_pointer)
    }
}

// Primary implementation using LazyBalanceSheet
impl OutgoingRunes for (Vec<RuneTransfer>, LazyBalanceSheet) {
    fn reconcile(
        &self,
        atomic: &mut AtomicPointer,
        balances_by_output: &mut HashMap<u32, BalanceSheet>,
        vout: u32,
        pointer: u32,
        refund_pointer: u32,
    ) -> Result<()> {
        // Get the runtime initial balance
        let runtime_initial = balances_by_output
            .get(&u32::MAX)
            .map(|v| v.clone())
            .unwrap_or_else(|| BalanceSheet::default());
        
        // Get the incoming initial balance
        let incoming_initial = balances_by_output
            .get(&vout)
            .ok_or("")
            .map_err(|_| anyhow!("balance sheet not found"))?
            .clone();
        
        // Merge the balances
        let mut initial = BalanceSheet::merge(&incoming_initial, &runtime_initial);

        // self.0 is the amount to forward to the pointer
        // self.1 is the amount to put into the runtime balance
        let outgoing: BalanceSheet = self.0.clone().into();
        
        // Convert LazyBalanceSheet to BalanceSheet for compatibility with existing methods
        let outgoing_runtime_sheet = BalanceSheet::from(self.1.clone());

        // Subtract outgoing and outgoing runtime balance from the initial amount
        initial.debit_mintable(&outgoing, atomic)?;
        initial.debit_mintable(&outgoing_runtime_sheet, atomic)?;

        // Remove the protomessage vout balances
        balances_by_output.remove(&vout);

        // Increase the pointer by the outgoing runes balancesheet
        increase_balances_using_sheet(balances_by_output, &outgoing, pointer);

        // Set the runtime to the ending runtime balance sheet
        // note that u32::MAX is the runtime vout
        balances_by_output.insert(u32::MAX, outgoing_runtime_sheet);

        // Refund the remaining amount to the refund pointer
        increase_balances_using_sheet(balances_by_output, &initial, refund_pointer);
        
        Ok(())
    }
}

pub fn load_sheet<T: KeyValuePointer>(ptr: &T) -> BalanceSheet {
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

// New function that returns a LazyBalanceSheet
pub fn load_lazy_sheet<T: KeyValuePointer>(_ptr: &T, storage_path: String) -> LazyBalanceSheet {
    // Create a new LazyBalanceSheet with the specified storage path
    let mut lazy_sheet = LazyBalanceSheet::new(storage_path);
    
    // We don't need to load all balances upfront - that's the whole point of LazyBalanceSheet
    // The balances will be loaded on demand when get() is called
    
    // Mark as not modified since we're just loading
    lazy_sheet.reset_modified();
    
    lazy_sheet
}

pub fn clear_balances<T: KeyValuePointer>(ptr: &T) {
    let runes_ptr = ptr.keyword("/runes");
    let balances_ptr = ptr.keyword("/balances");
    let length = runes_ptr.length();

    for i in 0..length {
        balances_ptr.select_index(i).set_value::<u128>(0);
    }
}

impl PersistentRecord for BalanceSheet {
    fn balances(&self) -> &HashMap<ProtoruneRuneId, u128> {
        &self.balances
    }
}
