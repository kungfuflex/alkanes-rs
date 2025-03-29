use anyhow::{anyhow, Result};
use metashrew::index_pointer::{AtomicPointer, IndexPointer};
use metashrew_support::index_pointer::KeyValuePointer;
use protorune_support::balance_sheet::{BalanceSheet, ProtoruneRuneId};
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
    fn balances(&self) -> &HashMap<ProtoruneRuneId, u128>;
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
        balances_by_output: &mut HashMap<u32, BalanceSheet<P>>,
        vout: u32,
        pointer: u32,
        refund_pointer: u32,
    ) -> Result<()>;
}

pub trait MintableDebit<P: KeyValuePointer + Clone> {
    fn debit_mintable(&mut self, sheet: &BalanceSheet<P>, atomic: &mut AtomicPointer)
        -> Result<()>;
}

impl<P: KeyValuePointer + Clone> MintableDebit<P> for BalanceSheet<P> {
    fn debit_mintable(
        &mut self,
        sheet: &BalanceSheet<P>,
        atomic: &mut AtomicPointer,
    ) -> Result<()> {
        for (rune, balance) in &sheet.balances {
            let mut amount = *balance;
            let current = self.get(&rune);
            if amount > current {
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
impl<P: KeyValuePointer + Clone> OutgoingRunes<P> for (Vec<RuneTransfer>, BalanceSheet<P>) {
    fn reconcile(
        &self,
        atomic: &mut AtomicPointer,
        balances_by_output: &mut HashMap<u32, BalanceSheet<P>>,
        vout: u32,
        pointer: u32,
        refund_pointer: u32,
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
        let mut initial = BalanceSheet::merge(&incoming_initial, &runtime_initial);

        // self.0 is the amount to forward to the pointer
        // self.1 is the amount to put into the runtime balance
        let outgoing: BalanceSheet<P> = self.0.clone().into();
        let outgoing_runtime = self.1.clone();

        // we want to subtract outgoing and the outgoing runtime balance
        // amount from the initial amount
        initial.debit_mintable(&outgoing, atomic)?;
        initial.debit_mintable(&outgoing_runtime, atomic)?;

        // now lets update balances_by_output to correct values

        // first remove the protomessage vout balances
        balances_by_output.remove(&vout);

        // increase the pointer by the outgoing runes balancesheet
        increase_balances_using_sheet(balances_by_output, &outgoing, pointer);

        // set the runtime to the ending runtime balance sheet
        // note that u32::MAX is the runtime vout
        balances_by_output.insert(u32::MAX, outgoing_runtime);

        // refund the remaining amount to the refund pointer
        increase_balances_using_sheet(balances_by_output, &initial, refund_pointer);
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

impl<P: KeyValuePointer + Clone> PersistentRecord for BalanceSheet<P> {
    fn balances(&self) -> &HashMap<ProtoruneRuneId, u128> {
        &self.balances
    }
}
