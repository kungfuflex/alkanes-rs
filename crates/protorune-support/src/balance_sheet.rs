use crate::rune_transfer::RuneTransfer;
use anyhow::{anyhow, Result};
use hex;
use metashrew_support::environment::RuntimeEnvironment;
use metashrew_support::index_pointer::KeyValuePointer;
use metashrew_support::utils::consume_sized_int;
use ordinals::RuneId;

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap};
use std::io::Cursor;
use std::marker::PhantomData;
use std::sync::Arc;
use std::u128;

#[derive(
    Eq, PartialOrd, Ord, PartialEq, Hash, Clone, Copy, Debug, Default, Serialize, Deserialize,
)]
pub struct ProtoruneRuneId {
    pub block: u128,
    pub tx: u128,
}

impl TryFrom<Vec<u8>> for ProtoruneRuneId {
    type Error = anyhow::Error;
    fn try_from(v: Vec<u8>) -> Result<ProtoruneRuneId> {
        let mut cursor: Cursor<Vec<u8>> = Cursor::<Vec<u8>>::new(v);
        let (block, tx) = (
            consume_sized_int::<u128>(&mut cursor)?,
            consume_sized_int::<u128>(&mut cursor)?,
        );
        Ok(ProtoruneRuneId { block, tx })
    }
}

pub trait RuneIdentifier {
    fn to_pair(&self) -> (u128, u128);
}

impl From<crate::proto::protorune::ProtoruneRuneId> for ProtoruneRuneId {
    fn from(v: crate::proto::protorune::ProtoruneRuneId) -> ProtoruneRuneId {
        ProtoruneRuneId {
            block: v.height.unwrap().into(),
            tx: v.txindex.unwrap().into(),
        }
    }
}

impl From<ProtoruneRuneId> for crate::proto::protorune::ProtoruneRuneId {
    fn from(v: ProtoruneRuneId) -> crate::proto::protorune::ProtoruneRuneId {
        let mut result = crate::proto::protorune::ProtoruneRuneId::default();
        result.height = Some(v.block.into());
        result.txindex = Some(v.tx.into());
        result
    }
}

impl ProtoruneRuneId {
    pub fn new(block: u128, tx: u128) -> Self {
        ProtoruneRuneId { block, tx }
    }
    pub fn delta(self, next: ProtoruneRuneId) -> Option<(u128, u128)> {
        let block = next.block.checked_sub(self.block)?;

        let tx = if block == 0 {
            next.tx.checked_sub(self.tx)?
        } else {
            next.tx
        };

        Some((block.into(), tx.into()))
    }
}

impl RuneIdentifier for ProtoruneRuneId {
    fn to_pair(&self) -> (u128, u128) {
        return (self.block, self.tx);
    }
}

impl RuneIdentifier for RuneId {
    fn to_pair(&self) -> (u128, u128) {
        return (self.block as u128, self.tx as u128);
    }
}

impl From<RuneId> for ProtoruneRuneId {
    fn from(v: RuneId) -> ProtoruneRuneId {
        let (block, tx) = v.to_pair();
        ProtoruneRuneId::new(block as u128, tx as u128)
    }
}


impl From<ProtoruneRuneId> for Vec<u8> {
    fn from(rune_id: ProtoruneRuneId) -> Self {
        let mut bytes = Vec::new();
        let (block, tx) = rune_id.to_pair();

        bytes.extend(&block.to_le_bytes());
        bytes.extend(&tx.to_le_bytes());
        bytes
    }
}

impl From<ProtoruneRuneId> for Arc<Vec<u8>> {
    fn from(rune_id: ProtoruneRuneId) -> Self {
        let bytes = rune_id.into();
        Arc::new(bytes)
    }
}

impl From<Arc<Vec<u8>>> for ProtoruneRuneId {
    fn from(arc_bytes: Arc<Vec<u8>>) -> Self {
        let bytes: &[u8] = arc_bytes.as_ref();
        let block = u128::from_le_bytes((&bytes[0..16]).try_into().unwrap());
        let tx = u128::from_le_bytes((&bytes[16..32]).try_into().unwrap());
        ProtoruneRuneId { block, tx }
    }
}

pub trait BalanceSheetOperations<E: RuntimeEnvironment>: Sized {
    fn from_pairs(runes: Vec<ProtoruneRuneId>, balances: Vec<u128>, env: &mut E) -> Self;
    fn concat(ary: &mut Vec<Self>, env: &mut E) -> Result<Self>;
    fn get(&self, rune: &ProtoruneRuneId, env: &mut E) -> u128;
    fn set(&mut self, rune: &ProtoruneRuneId, value: u128, env: &mut E);
    fn increase(&mut self, rune: &ProtoruneRuneId, value: u128, env: &mut E) -> Result<()> {
        let current_balance = self.get(rune, env);
        self.set(
            rune,
            current_balance.checked_add(value).ok_or("").map_err(|_| {
                anyhow!(format!(
                    "overflow error during balance sheet increase, current({}) + additional({})",
                    current_balance, value
                ))
            })?,
            env
        );
        Ok(())
    }
    fn decrease(&mut self, rune: &ProtoruneRuneId, value: u128, env: &mut E) -> bool {
        let current_balance = self.get(rune, env);
        if current_balance < value {
            false
        } else {
            self.set(rune, current_balance - value, env);
            true
        }
    }
    fn pipe(&self, sheet: &mut Self, env: &mut E) -> Result<()> {
        for (rune, balance) in self.balances() {
            sheet.increase(rune, *balance, env)?;
        }
        Ok(())
    }
    fn debit(&mut self, sheet: &Self, env: &mut E) -> Result<()> {
        for (rune, balance) in sheet.balances() {
            if *balance <= self.get(&rune, env) {
                self.decrease(rune, *balance, env);
            } else {
                return Err(anyhow!("balance underflow"));
            }
        }
        Ok(())
    }
    fn rune_debit(&mut self, sheet: &Self, env: &mut E) -> Result<()> {
        self.debit(sheet, env)
    }
    fn merge(a: &mut Self, b: &mut Self, env: &mut E) -> Result<Self>;
    fn merge_sheets(&mut self, a: &Self, b: &Self, env: &mut E) -> Result<()> {
        for (rune, balance) in a.balances() {
            self.increase(rune, *balance, env)?;
        }
        for (rune, balance) in b.balances() {
            self.increase(rune, *balance, env)?;
        }
        Ok(())
    }
    fn balances(&self) -> &BTreeMap<ProtoruneRuneId, u128>;
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct CachedBalanceSheet {
    pub balances: BTreeMap<ProtoruneRuneId, u128>,
}

impl<E: RuntimeEnvironment> BalanceSheetOperations<E> for CachedBalanceSheet {
    fn get(&self, rune: &ProtoruneRuneId, _env: &mut E) -> u128 {
        *self.balances.get(rune).unwrap_or(&0u128)
    }
    fn set(&mut self, rune: &ProtoruneRuneId, value: u128, _env: &mut E) {
        self.balances.insert(rune.clone(), value);
    }
    fn from_pairs(runes: Vec<ProtoruneRuneId>, balances: Vec<u128>, _env: &mut E) -> Self {
        let mut sheet = Self::default();
        for i in 0..runes.len() {
            sheet.balances.insert(runes[i], balances[i]);
        }
        return sheet;
    }
    fn concat(ary: &mut Vec<Self>, env: &mut E) -> Result<Self> {
        let mut concatenated = Self::default();
        for sheet in ary {
            for (rune, balance) in <CachedBalanceSheet as BalanceSheetOperations<E>>::balances(sheet) {
                concatenated.increase(rune, *balance, env)?;
            }
        }
        Ok(concatenated)
    }
    fn merge(a: &mut CachedBalanceSheet, b: &mut CachedBalanceSheet, env: &mut E) -> Result<CachedBalanceSheet> {
        let mut merged = CachedBalanceSheet::default();
        merged.merge_sheets(a, b, env)?;
        Ok(merged)
    }
    fn balances(&self) -> &BTreeMap<ProtoruneRuneId, u128> {
        &self.balances
    }
}

impl PartialEq for CachedBalanceSheet {
    fn eq(&self, other: &Self) -> bool {
        self.balances == other.balances
    }
}

impl Eq for CachedBalanceSheet {}


#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BalanceSheet<E: RuntimeEnvironment, P: KeyValuePointer<E> + Clone> {
    pub cached: CachedBalanceSheet,
    #[serde(skip)]
    pub load_ptrs: Vec<P>,
    #[serde(skip)]
    _phantom: PhantomData<E>,
}

impl<E: RuntimeEnvironment, P: KeyValuePointer<E> + Clone> Default for BalanceSheet<E, P> {
    fn default() -> Self {
        BalanceSheet {
            cached: CachedBalanceSheet::default(),
            load_ptrs: Vec::new(),
            _phantom: PhantomData,
        }
    }
}

impl<E: RuntimeEnvironment, P: KeyValuePointer<E> + Clone> BalanceSheetOperations<E> for BalanceSheet<E, P> {
    fn get(&self, rune: &ProtoruneRuneId, env: &mut E) -> u128 {
        self.load_balance(rune, env)
    }
    fn set(&mut self, rune: &ProtoruneRuneId, value: u128, _env: &mut E) {
        self.cached.balances.insert(rune.clone(), value);
    }
    fn from_pairs(runes: Vec<ProtoruneRuneId>, balances: Vec<u128>, env: &mut E) -> Self {
        let mut sheet = Self::default();
        for i in 0..runes.len() {
            sheet.set(&runes[i], balances[i], env);
        }
        return sheet;
    }
    fn concat(ary: &mut Vec<Self>, env: &mut E) -> Result<Self> {
        let mut concatenated = Self::default();
        for sheet in ary {
            concatenated = Self::merge(&mut concatenated, sheet, env)?;
        }
        Ok(concatenated)
    }
    fn merge(a: &mut BalanceSheet<E, P>, b: &mut BalanceSheet<E, P>, env: &mut E) -> Result<BalanceSheet<E, P>> {
        let mut merged = Self::default();
        merged.merge_sheets(a, b, env)?;
        merged.load_ptrs.extend(a.load_ptrs.drain(..));
        merged.load_ptrs.extend(b.load_ptrs.drain(..));
        Ok(merged)
    }
    fn balances(&self) -> &BTreeMap<ProtoruneRuneId, u128> {
        &self.cached.balances
    }
}


impl From<crate::proto::protorune::Uint128> for u128 {
    fn from(v: crate::proto::protorune::Uint128) -> u128 {
        let mut result: Vec<u8> = Vec::<u8>::with_capacity(16);
        result.extend(&v.lo.to_le_bytes());
        result.extend(&v.hi.to_le_bytes());
        let bytes_ref: &[u8] = &result;
        u128::from_le_bytes(bytes_ref.try_into().unwrap())
    }
}

impl From<u128> for crate::proto::protorune::Uint128 {
    fn from(v: u128) -> crate::proto::protorune::Uint128 {
        let bytes = v.to_le_bytes().to_vec();
        let mut container: crate::proto::protorune::Uint128 =
            crate::proto::protorune::Uint128::default();
        container.lo = u64::from_le_bytes((&bytes[0..8]).try_into().unwrap());
        container.hi = u64::from_le_bytes((&bytes[8..16]).try_into().unwrap());
        container
    }
}

impl<E: RuntimeEnvironment, P: KeyValuePointer<E> + Clone> From<crate::proto::protorune::OutpointResponse>
    for BalanceSheet<E, P>
{
    fn from(v: crate::proto::protorune::OutpointResponse) -> BalanceSheet<E, P> {
        let pairs = v
            .balances
            .unwrap()
            .entries
            .clone()
            .into_iter()
            .map(|v| {
                (
                    ProtoruneRuneId::new(
                        v.rune
                            .clone()
                            .unwrap()
                            .rune_id
                            .unwrap()
                            .height
                            .unwrap()
                            .into(),
                        v.rune.unwrap().rune_id.unwrap().txindex.unwrap().into(),
                    ),
                    v.balance.unwrap().into(),
                )
            })
            .collect::<Vec<(ProtoruneRuneId, u128)>>();
        let ids = pairs
            .iter()
            .map(|(id, _)| id.clone())
            .collect::<Vec<ProtoruneRuneId>>();
        let balances = pairs.iter().map(|(_, v)| v.clone()).collect::<Vec<u128>>();
        let mut sheet = Self::default();
        for i in 0..ids.len() {
            sheet.cached.balances.insert(ids[i], balances[i]);
        }
        sheet
    }
}

impl<E: RuntimeEnvironment, P: KeyValuePointer<E> + Clone> BalanceSheet<E, P> {
    

    pub fn new_ptr_backed(ptr: P) -> Self {
        BalanceSheet {
            cached: CachedBalanceSheet::default(),
            load_ptrs: vec![ptr],
            _phantom: PhantomData,
        }
    }

    pub fn load_balance(&self, rune: &ProtoruneRuneId, env: &mut E) -> u128 {
        if let Some(balance) = <CachedBalanceSheet as BalanceSheetOperations<E>>::balances(&self.cached).get(rune) {
            return *balance;
        }
        let mut total_stored_balance = 0;
        let rune_clone = rune.clone();
        for ptr in &self.load_ptrs {
            let runes_to_balances_ptr = ptr
                .keyword("/id_to_balance")
                .select(&rune_clone.into());
            if runes_to_balances_ptr.get(env).len() != 0 {
                let stored_balance = runes_to_balances_ptr.get_value::<u128>(env);
                total_stored_balance += stored_balance;
            }
        }
        return total_stored_balance;
    }

    pub fn get_and_update(&mut self, rune: &ProtoruneRuneId, env: &mut E) -> u128 {
        let balance = self.load_balance(rune, env);
        self.set(rune, balance, env);
        balance
    }
}



impl<E: RuntimeEnvironment, P: KeyValuePointer<E> + Clone> TryFrom<Vec<RuneTransfer>>
    for BalanceSheet<E, P>
{
    type Error = anyhow::Error;

    fn try_from(v: Vec<RuneTransfer>) -> Result<BalanceSheet<E, P>> {
        let mut balance_sheet = Self::default();
        for transfer in v {
            let current_balance = balance_sheet.cached.balances.get(&transfer.id).unwrap_or(&0);
            balance_sheet.cached.balances.insert(transfer.id, current_balance + transfer.value);
        }
        Ok(balance_sheet)
    }
}




pub trait IntoString {
    fn to_str(&self) -> String;
}

impl IntoString for Vec<u8> {
    fn to_str(&self) -> String {
        hex::encode(self)
    }
}
