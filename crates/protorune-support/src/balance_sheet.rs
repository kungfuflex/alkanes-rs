use anyhow::{anyhow, Result};
use crate::host::Host;
pub use crate::proto::protorune::{BalanceSheetItem, ProtoruneRuneId, Uint128};
use std::collections::BTreeMap;
use std::marker::PhantomData;

/*
 * Chadson's Journal:
 *
 * The `Balance` struct in `alkanes-support/src/view.rs` needs to derive `Hash`.
 * To allow this, the `BalanceSheet` struct, which it contains, must also
 * implement `Hash`. I'm adding `Hash` to the derive macro for `BalanceSheet`
 * to resolve this compilation error.
 */
use std::hash::Hash;

use protobuf::{Message, CodedInputStream, CodedOutputStream, SpecialFields};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct BalanceSheet<H: Host> {
    pub balances: BTreeMap<ProtoruneRuneId, u128>,
    pub special_fields: SpecialFields,
    _phantom: PhantomData<H>,
}

impl<H: Host> Default for BalanceSheet<H> {
    fn default() -> Self {
        Self {
            balances: BTreeMap::new(),
            special_fields: SpecialFields::default(),
            _phantom: PhantomData,
        }
    }
}

use protobuf::rt;

impl<H: Host + Clone + Default + Send + Sync + PartialEq + 'static> Message for BalanceSheet<H> {
    const NAME: &'static str = "BalanceSheet";

    fn is_initialized(&self) -> bool {
        true
    }

    fn merge_from(&mut self, is: &mut CodedInputStream) -> protobuf::Result<()> {
        while let Some(tag) = is.read_raw_tag_or_eof()? {
            match tag {
                10 => {
                    let item = is.read_message::<BalanceSheetItem>()?;
                    if let Some(rune_id) = item.rune.as_ref().and_then(|r| r.runeId.as_ref()) {
                        if let Some(balance) = item.balance.as_ref() {
                            self.balances.insert(rune_id.clone(), balance.clone().into());
                        }
                    }
                }
                _ => {
                    rt::read_unknown_or_skip_group(tag, is, self.mut_unknown_fields())?;
                }
            }
        }
        Ok(())
    }

    fn write_to_with_cached_sizes(&self, os: &mut CodedOutputStream) -> protobuf::Result<()> {
        for (rune_id, balance) in &self.balances {
            let mut item = BalanceSheetItem::new();
            let mut rune = crate::proto::protorune::Rune::new();
            rune.runeId = protobuf::MessageField::some(rune_id.clone());
            item.rune = protobuf::MessageField::some(rune);
            item.balance = protobuf::MessageField::some((*balance).into());
            os.write_message(1, &item)?;
        }
        os.write_unknown_fields(self.special_fields().unknown_fields())?;
        Ok(())
    }

    fn special_fields(&self) -> &SpecialFields {
        &self.special_fields
    }

    fn mut_special_fields(&mut self) -> &mut SpecialFields {
        &mut self.special_fields
    }

    fn new() -> Self {
        Self::default()
    }

    fn clear(&mut self) {
        self.balances.clear();
        self.special_fields.clear();
    }

    fn compute_size(&self) -> u64 {
        let mut size = 0;
        for (rune_id, balance) in &self.balances {
            let mut item = BalanceSheetItem::new();
            let mut rune = crate::proto::protorune::Rune::new();
            rune.runeId = protobuf::MessageField::some(rune_id.clone());
            item.rune = protobuf::MessageField::some(rune);
            item.balance = protobuf::MessageField::some((*balance).into());
            let len = item.compute_size();
            size += 1 + protobuf::rt::compute_raw_varint64_size(len) + len;
        }
        size += rt::unknown_fields_size(self.special_fields.unknown_fields());
        size
    }

    fn default_instance() -> &'static Self {
        panic!("default_instance() is not implemented for generic BalanceSheet");
    }
}
pub use crate::rune_transfer::RuneTransfer;
use crate::rune_transfer::{increase_balances_using_sheet};

pub trait BalanceSheetOperations<H: Host> {
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

pub trait PersistentRecord<H: Host>: BalanceSheetOperations<H> {
    fn save(&self, host: &H, outpoint: &bitcoin::OutPoint, is_cenotaph: bool) -> Result<()>;
}


pub trait Mintable<H: Host> {
    fn mintable_in_protocol(&self, host: &H) -> bool;
}

impl<H: Host> Mintable<H> for ProtoruneRuneId {
    fn mintable_in_protocol(&self, host: &H) -> bool {
        host.is_rune_mintable(self).unwrap_or(false)
    }
}

pub trait OutgoingRunes<H: Host> {
    fn reconcile(
        &self,
        host: &H,
        balances_by_output: &mut BTreeMap<u32, BalanceSheet<H>>,
        vout: u32,
        pointer: u32,
        refund_pointer: u32,
    ) -> Result<()>;
}

pub trait MintableDebit<H: Host> {
    fn debit_mintable(&mut self, sheet: &BalanceSheet<H>, host: &H) -> Result<()>;
}

impl<H: Host + Clone + Default> MintableDebit<H> for BalanceSheet<H> {
    fn debit_mintable(&mut self, sheet: &BalanceSheet<H>, host: &H) -> Result<()> {
        for (rune, balance) in sheet.balances.iter() {
            let mut amount = *balance;
            let current = self.get(&rune);
            if amount > current {
                if rune.mintable_in_protocol(host) {
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

impl<H: Host + Clone + Default> TryFrom<Vec<RuneTransfer>> for BalanceSheet<H> {
    type Error = anyhow::Error;

    fn try_from(transfers: Vec<RuneTransfer>) -> Result<Self, Self::Error> {
        let mut sheet = BalanceSheet::<H>::default();
        for transfer in transfers {
            sheet.increase(&transfer.id, transfer.value)?;
        }
        Ok(sheet)
    }
}

impl<H: Host + Clone + Default> OutgoingRunes<H> for (Vec<RuneTransfer>, BalanceSheet<H>) {
    fn reconcile(
        &self,
        host: &H,
        balances_by_output: &mut BTreeMap<u32, BalanceSheet<H>>,
        vout: u32,
        pointer: u32,
        refund_pointer: u32,
    ) -> Result<()> {
        let runtime_initial = balances_by_output
            .get(&u32::MAX)
            .map(|v| v.clone())
            .unwrap_or_else(|| BalanceSheet::<H>::default());
        let incoming_initial = balances_by_output
            .get(&vout)
            .ok_or("")
            .map_err(|_| anyhow!("balance sheet not found"))?
            .clone();
        let mut initial = BalanceSheet::<H>::merge(&incoming_initial, &runtime_initial)?;

        let outgoing: BalanceSheet<H> = self.0.clone().try_into()?;
        let outgoing_runtime = self.1.clone();

        initial.debit_mintable(&outgoing, host)?;
        initial.debit_mintable(&outgoing_runtime, host)?;

        balances_by_output.remove(&vout);

        increase_balances_using_sheet(balances_by_output, &mut outgoing.clone(), pointer)?;

        balances_by_output.insert(u32::MAX, outgoing_runtime);

        increase_balances_using_sheet(balances_by_output, &mut initial.clone(), refund_pointer)?;
        Ok(())
    }
}

pub fn load_sheet<H: Host>(host: &H, outpoint_bytes: &[u8]) -> Result<BalanceSheet<H>> {
    host.get_balance_sheet(outpoint_bytes)
}

pub fn clear_balances<H: Host>(host: &H, outpoint_bytes: &[u8]) -> Result<()> {
    host.clear_balances(outpoint_bytes)
}

impl<H: Host + Clone + Default> PersistentRecord<H> for BalanceSheet<H> {
    fn save(&self, host: &H, outpoint: &bitcoin::OutPoint, is_cenotaph: bool) -> Result<()> {
        if !is_cenotaph {
            host.save_balance_sheet(outpoint, self)?;
        }
        Ok(())
    }
}

impl<H: Host + Clone + Default> BalanceSheetOperations<H> for BalanceSheet<H> {
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
        let mut sheet = BalanceSheet::<H>::default();
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
        let mut result = BalanceSheet::<H>::default();
        for mut sheet in sheets {
            sheet.pipe(&mut result)?;
        }
        Ok(result)
    }

    fn merge(&self, other: &Self) -> Result<Self> {
        let mut result = self.clone();
        for (rune, balance) in other.balances.iter() {
            result.increase(&rune, *balance)?;
        }
        Ok(result)
    }
}
