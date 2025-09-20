use anyhow::{anyhow, Result};
use metashrew_support::index_pointer::{AtomicPointer, IndexPointer};
use metashrew_support::index_pointer::KeyValuePointer;
use protorune_support::balance_sheet::{BalanceSheet, ProtoruneRuneId};
pub use protorune_support::balance_sheet::{BalanceSheetOperations};
use protorune_support::rune_transfer::{increase_balances_using_sheet, RuneTransfer};
use std::collections::BTreeMap;
use metashrew_support::environment::RuntimeEnvironment;
use std::sync::Arc;

pub trait PersistentRecord<E: RuntimeEnvironment>: BalanceSheetOperations<E> {
    fn save<T: KeyValuePointer<E>>(&self, ptr: &T, is_cenotaph: bool, env: &mut E) {
        let mut runes_ptr = ptr.keyword("/runes");
        let mut balances_ptr = ptr.keyword("/balances");
        let runes_to_balances_ptr = ptr.keyword("/id_to_balance");

        env.log(&format!("Saving balance sheet with {} entries", self.balances().len()));
        for (rune, balance) in self.balances() {
            env.log(&format!("  Rune: {:?}, Balance: {}", rune, balance));
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

pub trait Mintable<E: RuntimeEnvironment + Clone> {
    fn mintable_in_protocol(&self, atomic: &mut AtomicPointer<E>, env: &mut E) -> bool;
}

impl<E: RuntimeEnvironment + Default + Clone> Mintable<E> for ProtoruneRuneId {
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

pub trait OutgoingRunes<E: RuntimeEnvironment + Clone, P: KeyValuePointer<E> + Clone> {
    fn reconcile(
        self,
        atomic: &mut AtomicPointer<E>,
        balances_by_output: &mut BTreeMap<u32, BalanceSheet<E, P>>,
        vout: u32,
        pointer: u32,
        env: &mut E,
    ) -> Result<()>;
}

pub trait MintableDebit<E: RuntimeEnvironment + Clone, P: KeyValuePointer<E> + Clone + std::fmt::Debug> {
    fn debit_mintable(&mut self, sheet: &BalanceSheet<E, P>, atomic: &mut AtomicPointer<E>, env: &mut E)
        -> Result<()>;
}

impl<E: RuntimeEnvironment + Default + Clone, P: KeyValuePointer<E> + Clone + std::fmt::Debug> MintableDebit<E, P> for BalanceSheet<E, P> {
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
                    return Err(anyhow!("balance underflow"));
                }
            }
            self.decrease(rune, amount, env);
        }
        Ok(())
    }
}
impl<E: RuntimeEnvironment + Default + Clone, P: KeyValuePointer<E> + std::fmt::Debug + Clone> OutgoingRunes<E, P>
    for (Vec<RuneTransfer>, BalanceSheet<E, P>)
{
    fn reconcile(
        self,
        atomic: &mut AtomicPointer<E>,
        balances_by_output: &mut BTreeMap<u32, BalanceSheet<E, P>>,
        vout: u32,
        pointer: u32,
        env: &mut E,
    ) -> Result<()> {
        let mut runtime_initial = balances_by_output
            .get_mut(&u32::MAX)
            .map(|v| v.clone())
            .unwrap_or_else(|| BalanceSheet::default());
        let mut incoming_initial = balances_by_output
            .get_mut(&vout)
            .ok_or("")
            .map_err(|_| anyhow!("balance sheet not found"))?
            .clone();
        let mut initial = BalanceSheet::merge(&mut incoming_initial, &mut runtime_initial, env)?;

        // self.0 is the amount to forward to the pointer
        // self.1 is the amount to put into the runtime balance
        let outgoing: BalanceSheet<E, P> = self.0.try_into()?;
        let outgoing_runtime = self.1;

        // we want to subtract outgoing and the outgoing runtime balance
        // amount from the initial amount
        initial.debit_mintable(&outgoing, atomic, env)?;
        initial.debit_mintable(&outgoing_runtime, atomic, env)?;
        for (id, balance) in initial.balances() {
            if *balance != 0 {
                env.log(&format!("BIG ERROR: NONZERO {:?} {}", id, balance));
            }
        }

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

pub fn load_sheet<E: RuntimeEnvironment + Clone, T: KeyValuePointer<E> + Clone>(
    ptr: &T,
    env: &mut E,
) -> BalanceSheet<E, T> {
    let runes_ptr = ptr.keyword("/runes");
    let balances_ptr = ptr.keyword("/balances");
    let length = runes_ptr.length(env);
    let mut result = BalanceSheet::default();
    env.log(&format!("Loading balance sheet, length: {}", length));
    for i in 0..length {
        let rune = ProtoruneRuneId::from(runes_ptr.select_index(i).get(env));
        let balance = balances_ptr.select_index(i).get_value::<u128>(env);
        env.log(&format!("  Loaded rune: {:?}, balance: {}", rune, balance));
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

impl<E: RuntimeEnvironment + Default + Clone, P: KeyValuePointer<E> + std::fmt::Debug + Clone> PersistentRecord<E>
    for BalanceSheet<E, P>
{
}
