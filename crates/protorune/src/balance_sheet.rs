use anyhow::{anyhow, Result};
use metashrew_support::index_pointer::{AtomicPointer, IndexPointer};
use metashrew_support::index_pointer::KeyValuePointer;
use protorune_support::balance_sheet::{BalanceSheet, BalanceSheetOperations, ProtoruneRuneId};
use protorune_support::rune_transfer::{increase_balances_using_sheet, RuneTransfer};
use std::collections::BTreeMap;
use metashrew_support::environment::RuntimeEnvironment;
use std::sync::Arc;

#[allow(unused_imports)]


// use metashrew_core::{println, stdio::stdout};
// use std::fmt::Write;
//

pub trait PersistentRecord<E: RuntimeEnvironment>: BalanceSheetOperations<E> {
    fn save<T: KeyValuePointer<E>>(&self, ptr: &T, is_cenotaph: bool, env: &mut E) {
        let mut runes_ptr = ptr.keyword("/runes");
        let mut balances_ptr = ptr.keyword("/balances");
        let runes_to_balances_ptr = ptr.keyword("/id_to_balance");

        for (rune, balance) in self.balances() {
            if *balance != 0u128 && !is_cenotaph {
                let rune_bytes: Vec<u8> = (*rune).into();
                runes_ptr.append(env, Arc::new(rune_bytes.clone()));

                balances_ptr.append_value::<u128>(env, *balance);

                runes_to_balances_ptr
                    .select(&rune_bytes)
                    .set_value::<u128>(env, *balance);
            }
        }
    }
    fn save_index<T: KeyValuePointer<E>>(
        &self,
        rune: &ProtoruneRuneId,
        ptr: &T,
        is_cenotaph: bool,
        env: &mut E,
    ) -> Result<()> {
        let mut runes_ptr = ptr.keyword("/runes");
        let mut balances_ptr = ptr.keyword("/balances");
        let runes_to_balances_ptr = ptr.keyword("/id_to_balance");
        let balance = self
            .balances()
            .get(rune)
            .ok_or(anyhow!("no balance found"))?;
        if *balance != 0u128 && !is_cenotaph {
            let rune_bytes: Vec<u8> = (*rune).into();
            runes_ptr.append(env, Arc::new(rune_bytes.clone()));
            balances_ptr.append_value::<u128>(env, *balance);
            runes_to_balances_ptr
                .select(&rune_bytes)
                .set_value::<u128>(env, *balance);
        }

        Ok(())
    }
}

pub trait Mintable<E: RuntimeEnvironment> {
    fn mintable_in_protocol(&self, atomic: &mut AtomicPointer<E>, env: &mut E) -> bool;
}

impl<E: RuntimeEnvironment + Default> Mintable<E> for ProtoruneRuneId {
    fn mintable_in_protocol(&self, atomic: &mut AtomicPointer<E>, env: &mut E) -> bool {
        // if it was not etched via runes-like etch in the Runestone and protoburned, then it is considered mintable
        atomic
            .derive(
                &IndexPointer::<E>::default().keyword("/etching/byruneid/").select(&(self.clone().into())),
            )
            .get(env)
            .len()
            == 0
    }
}

pub trait OutgoingRunes<E: RuntimeEnvironment, P: KeyValuePointer<E> + Clone> {
    fn reconcile(
        &self,
        balances_by_output: &mut BTreeMap<u32, BalanceSheet<E, P>>,
        vout: u32,
        pointer: u32,
        env: &mut E,
    ) -> Result<()>;
}

pub trait MintableDebit<E: RuntimeEnvironment, P: KeyValuePointer<E> + Clone + std::fmt::Debug> {
    fn debit_mintable(&mut self, sheet: &BalanceSheet<E, P>, atomic: &mut AtomicPointer<E>, env: &mut E)
        -> Result<()>;
}

impl<E: RuntimeEnvironment + Default, P: KeyValuePointer<E> + Clone + std::fmt::Debug> MintableDebit<E, P> for BalanceSheet<E, P> {
    // logically, this will debit the input sheet from the self sheet, and if it would produce a negative value
    // it will check if the rune id is mintable (if it was etched and protoburned or if it is an alkane).
    // if it is mintable, we assume the extra amount was minted and do not decrease the amount.
    // NOTE: if it was a malicious case where an alkane was minted by another alkane, this will not check for that.
    // such a case should be checked in debit_balances in src/utils.rs
    fn debit_mintable(
        &mut self,
        sheet: &BalanceSheet<E, P>,
        atomic: &mut AtomicPointer<E>,
        env: &mut E,
    ) -> Result<()> {
        for (rune, balance) in sheet.balances() {
            let mut amount = *balance;
            let current = self.get(&rune, env);
            if amount > current {
                if rune.mintable_in_protocol(atomic, env) {
                    amount = current;
                } else {
                    return Err(anyhow!("balance underflow during debit_mintable"));
                }
            }
            self.decrease(rune, amount, env);
        }
        Ok(())
    }
}
impl<E: RuntimeEnvironment, P: KeyValuePointer<E> + Clone + std::fmt::Debug> OutgoingRunes<E, P>
    for (Vec<RuneTransfer>, BalanceSheet<E, P>)
{
    fn reconcile(
        &self,
        balances_by_output: &mut BTreeMap<u32, BalanceSheet<E, P>>,
        vout: u32,
        pointer: u32,
        env: &mut E,
    ) -> Result<()> {
        // self.0 is the amount to forward to the pointer
        // self.1 is the amount to put into the runtime balance
        let outgoing: BalanceSheet<E, P> = self.0.clone().try_into()?;
        let outgoing_runtime = self.1.clone();

        // now lets update balances_by_output to correct values

        // first remove the protomessage vout balances
        balances_by_output.remove(&vout);

        // increase the pointer by the outgoing runes balancesheet
        increase_balances_using_sheet(balances_by_output, &outgoing, pointer, env)?;

        // set the runtime to the ending runtime balance sheet
        // note that u32::MAX is the runtime vout
        balances_by_output.insert(u32::MAX, outgoing_runtime);
        Ok(())
    }
}

pub fn load_sheet<E: RuntimeEnvironment, T: KeyValuePointer<E> + Clone>(
    ptr: &T,
    env: &mut E,
) -> BalanceSheet<E, T> {
    let runes_ptr = ptr.keyword("/runes");
    let balances_ptr = ptr.keyword("/balances");
    let length = runes_ptr.length(env);
    let mut result = BalanceSheet::default();

    for i in 0..length {
        let rune = ProtoruneRuneId::from(runes_ptr.select_index(i).get(env));
        let balance = balances_ptr.select_index(i).get_value::<u128>(env);
        result.set(&rune, balance, env);
    }
    result
}

pub fn clear_balances<E: RuntimeEnvironment, T: KeyValuePointer<E>>(ptr: &T, env: &mut E) {
    let runes_ptr = ptr.keyword("/runes");
    let balances_ptr = ptr.keyword("/balances");
    let length = runes_ptr.length(env);
    let runes_to_balances_ptr = ptr.keyword("/id_to_balance");

    for i in 0..length {
        balances_ptr.select_index(i).set_value::<u128>(env, 0);
        let rune = balances_ptr.select_index(i).get(env);
        runes_to_balances_ptr.select(&rune).set_value::<u128>(env, 0);
    }
}

impl<E: RuntimeEnvironment, P: KeyValuePointer<E> + Clone + std::fmt::Debug> PersistentRecord<E>
    for BalanceSheet<E, P>
{
}
