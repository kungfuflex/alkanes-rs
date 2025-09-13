use anyhow::{anyhow, Result};
use metashrew_core::index_pointer::{AtomicPointer, IndexPointer};
use metashrew_support::index_pointer::KeyValuePointer;
pub use crate::proto::protorune::{ProtoruneRuneId, BalanceSheetItem, Uint128};
use std::marker::PhantomData;
use protobuf::Message;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct BalanceSheet<P: KeyValuePointer + Default + Clone> {
    pub pointer: P,
    pub balances: BTreeMap<ProtoruneRuneId, u128>,
    _phantom: PhantomData<P>,
}
pub use crate::rune_transfer::RuneTransfer;
use crate::rune_transfer::{increase_balances_using_sheet};
use std::collections::BTreeMap;

pub trait BalanceSheetOperations {
    fn balances(&self) -> BTreeMap<ProtoruneRuneId, u128>;
    fn get(&self, rune: &ProtoruneRuneId) -> u128;
    fn set(&mut self, rune: &ProtoruneRuneId, amount: u128);
    fn increase(&mut self, rune: &ProtoruneRuneId, amount: u128) -> Result<()>;
    fn decrease(&mut self, rune: &ProtoruneRuneId, amount: u128);
    fn get_and_update(&mut self, rune: &ProtoruneRuneId) -> u128;
    fn from_pairs(runes: Vec<ProtoruneRuneId>, balances: Vec<u128>) -> Self;
    fn pipe(&mut self, other: &mut Self) -> Result<()>;
    fn concat(sheets: Vec<Self>) -> Result<Self>
    where
        Self: Sized;
    fn merge(&self, other: &Self) -> Result<Self>
    where
        Self: Sized;
}

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
            if balance != 0u128 && !is_cenotaph {
                let rune_bytes: Vec<u8> = rune.clone().into();
                runes_ptr.append(rune_bytes.clone().into());

                balances_ptr.append_value::<u128>(balance);

                runes_to_balances_ptr
                    .select(&rune_bytes)
                    .set_value::<u128>(balance);
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
        let balances = self.balances();
        let balance = balances.get(rune).ok_or(anyhow!("no balance found"))?;
        if *balance != 0u128 && !is_cenotaph {
            let rune_bytes: Vec<u8> = rune.clone().into();
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

pub trait OutgoingRunes<P: KeyValuePointer + Default + Clone> {
    fn reconcile(
        &self,
        atomic: &mut AtomicPointer,
        balances_by_output: &mut BTreeMap<u32, BalanceSheet<P>>,
        vout: u32,
        pointer: u32,
        refund_pointer: u32,
    ) -> Result<()>;
}

pub trait MintableDebit<P: KeyValuePointer + Default + Clone> {
    fn debit_mintable(&mut self, sheet: &BalanceSheet<P>, atomic: &mut AtomicPointer) -> Result<()>;
}

impl<P: KeyValuePointer + Default + Clone> MintableDebit<P> for BalanceSheet<P> {
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
            let mut amount = balance;
            let current = self.get(&rune);
            if amount > current {
                if rune.mintable_in_protocol(atomic) {
                    amount = current;
                } else {
                    return Err(anyhow!("balance underflow during debit_mintable"));
                }
            }
            self.decrease(&rune, amount);
        }
        Ok(())
    }
}
impl<P: KeyValuePointer + Default + Clone> TryFrom<Vec<RuneTransfer>> for BalanceSheet<P> {
    type Error = anyhow::Error;

    fn try_from(transfers: Vec<RuneTransfer>) -> Result<Self, Self::Error> {
        let mut sheet = BalanceSheet::<P>::default();
        for transfer in transfers {
            sheet.increase(&transfer.id, transfer.value)?;
        }
        Ok(sheet)
    }
}

impl<P: KeyValuePointer + Default + Clone> OutgoingRunes<P> for (Vec<RuneTransfer>, BalanceSheet<P>) {
    fn reconcile(
        &self,
        atomic: &mut AtomicPointer,
        balances_by_output: &mut BTreeMap<u32, BalanceSheet<P>>,
        vout: u32,
        pointer: u32,
        refund_pointer: u32,
    ) -> Result<()> {
        let runtime_initial = balances_by_output
            .get(&u32::MAX)
            .map(|v| v.clone())
            .unwrap_or_else(|| BalanceSheet::<P>::default());
        let incoming_initial = balances_by_output
            .get(&vout)
            .ok_or("")
            .map_err(|_| anyhow!("balance sheet not found"))?
            .clone();
        let mut initial = BalanceSheet::<P>::merge(&incoming_initial, &runtime_initial)?;

        // self.0 is the amount to forward to the pointer
        // self.1 is the amount to put into the runtime balance
        let outgoing: BalanceSheet<P> = self.0.clone().try_into()?;
        let outgoing_runtime = self.1.clone();

        // we want to subtract outgoing and the outgoing runtime balance
        // amount from the initial amount
        initial.debit_mintable(&outgoing, atomic)?;
        initial.debit_mintable(&outgoing_runtime, atomic)?;

        // now lets update balances_by_output to correct values

        // first remove the protomessage vout balances
        balances_by_output.remove(&vout);

        // increase the pointer by the outgoing runes balancesheet
        increase_balances_using_sheet(balances_by_output, &mut outgoing.clone(), pointer)?;

        // set the runtime to the ending runtime balance sheet
        // note that u32::MAX is the runtime vout
        balances_by_output.insert(u32::MAX, outgoing_runtime);

        // refund the remaining amount to the refund pointer
        increase_balances_using_sheet(balances_by_output, &mut initial.clone(), refund_pointer)?;
        Ok(())
    }
}

pub fn load_sheet<P: KeyValuePointer + Default + Clone>(ptr: &P) -> BalanceSheet<P> {
    let runes_ptr = ptr.keyword("/runes");
    let balances_ptr = ptr.keyword("/balances");
    let length = runes_ptr.length();
    let mut result = BalanceSheet::<P>::default();

    for i in 0..length {
        if let Ok(rune) = ProtoruneRuneId::parse_from_bytes(&runes_ptr.select_index(i).get()) {
            let balance = balances_ptr.select_index(i).get_value::<u128>();
            result.set(&rune, balance);
        }
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

impl<P: KeyValuePointer + Default + Clone> PersistentRecord for BalanceSheet<P> {}

impl<P: KeyValuePointer + Default + Clone> BalanceSheetOperations for BalanceSheet<P> {
    fn balances(&self) -> BTreeMap<ProtoruneRuneId, u128> {
        self.balances.clone()
    }

    fn get(&self, rune: &ProtoruneRuneId) -> u128 {
        self.balances.get(&rune.clone()).map_or(0, |v| *v)
    }

    fn set(&mut self, rune: &ProtoruneRuneId, amount: u128) {
        self.balances.insert(rune.clone(), amount);
    }

    fn increase(&mut self, rune: &ProtoruneRuneId, amount: u128) -> Result<()> {
        let current = self.get(rune);
        self.set(rune, current + amount);
        Ok(())
    }

    fn decrease(&mut self, rune: &ProtoruneRuneId, amount: u128) {
        let current = self.get(rune);
        self.set(rune, current - amount);
    }

    fn get_and_update(&mut self, rune: &ProtoruneRuneId) -> u128 {
        let amount = self.get(rune);
        self.set(rune, 0);
        amount
    }

    fn from_pairs(runes: Vec<ProtoruneRuneId>, balances: Vec<u128>) -> Self {
        let mut sheet = BalanceSheet::<P>::default();
        for (rune, balance) in runes.into_iter().zip(balances.into_iter()) {
            sheet.set(&rune, balance);
        }
        sheet
    }

    fn pipe(&mut self, other: &mut Self) -> Result<()> {
        for (rune, balance) in self.balances() {
            other.increase(&rune, balance)?;
        }
        Ok(())
    }

    fn concat(sheets: Vec<Self>) -> Result<Self> {
        let mut result = BalanceSheet::<P>::default();
        for mut sheet in sheets {
            sheet.pipe(&mut result)?;
        }
        Ok(result)
    }

    fn merge(&self, other: &Self) -> Result<Self> {
        let mut result = self.clone();
        for (rune, balance) in other.balances() {
            result.increase(&rune, balance)?;
        }
        Ok(result)
    }
}
