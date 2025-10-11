use crate::alkanes::rune_transfer::RuneTransfer;
use crate::index_pointer::KeyValuePointer;
use crate::vendored_ord::RuneId;
use anyhow::{anyhow, Result};
use hex;
use metashrew_support::environment::RuntimeEnvironment;
use protorune_support::balance_sheet::RuneIdentifier;
use protorune_support::proto::protorune::{BalanceSheetItem, Rune};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
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
            metashrew_support::utils::consume_sized_int::<u128>(&mut cursor)?,
            metashrew_support::utils::consume_sized_int::<u128>(&mut cursor)?,
        );
        Ok(ProtoruneRuneId { block, tx })
    }
}

impl From<protorune_support::proto::protorune::ProtoruneRuneId> for ProtoruneRuneId {
    fn from(v: protorune_support::proto::protorune::ProtoruneRuneId) -> ProtoruneRuneId {
        ProtoruneRuneId {
            block: v.height.unwrap().into(),
            tx: v.txindex.unwrap().into(),
        }
    }
}

impl From<ProtoruneRuneId> for protorune_support::proto::protorune::ProtoruneRuneId {
    fn from(v: ProtoruneRuneId) -> protorune_support::proto::protorune::ProtoruneRuneId {
        let mut result = protorune_support::proto::protorune::ProtoruneRuneId::default();
        result.height = Some(v.block.into());
        result.txindex = Some(v.tx.into());
        result
    }
}

impl<E: RuntimeEnvironment, P: KeyValuePointer<E> + Clone> From<protorune_support::proto::protorune::BalanceSheet> for BalanceSheet<E, P> {
    fn from(balance_sheet: protorune_support::proto::protorune::BalanceSheet) -> BalanceSheet<E, P> {
        BalanceSheet {
            cached: CachedBalanceSheet {
                balances: BTreeMap::<ProtoruneRuneId, u128>::from_iter(
                    balance_sheet.entries.into_iter().map(|v| {
                        let id = ProtoruneRuneId::new(
                            v.rune.unwrap().rune_id.unwrap().height.unwrap().into(),
                            v.rune.unwrap().rune_id.unwrap().txindex.unwrap().into(),
                        );
                        (id, v.balance.unwrap().into())
                    }),
                ),
            },
            load_ptrs: Vec::new(),
            _phantom: PhantomData,
        }
    }
}

impl<E: RuntimeEnvironment, P: KeyValuePointer<E> + Clone> From<BalanceSheet<E, P>> for protorune_support::proto::protorune::BalanceSheet {
    fn from(balance_sheet: BalanceSheet<E, P>) -> protorune_support::proto::protorune::BalanceSheet {
        protorune_support::proto::protorune::BalanceSheet {
            entries: balance_sheet
                .balances()
                .clone()
                .iter()
                .map(|(k, v)| BalanceSheetItem {
                    rune: Some(Rune {
                        rune_id: Some(protorune_support::proto::protorune::ProtoruneRuneId {
                            height: Some(k.block.into()),
                            txindex: Some(k.tx.into()),
                        }),
                        name: "UNKNOWN".to_owned(),
                        divisibility: 1,
                        spacers: 1,
                        symbol: "0".to_owned(),
                    }),
                    balance: Some((*v).into()),
                })
                .collect::<Vec<BalanceSheetItem>>(),
        }
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
        // Wrap the Vec in an Arc
        Arc::new(bytes)
    }
}

impl From<Arc<Vec<u8>>> for ProtoruneRuneId {
    fn from(arc_bytes: Arc<Vec<u8>>) -> Self {
        // Convert the Arc<Vec<u8>> to a slice of bytes
        let bytes: &[u8] = arc_bytes.as_ref();

        // Extract the u32 and u64 from the byte slice
        let block = u128::from_le_bytes((&bytes[0..16]).try_into().unwrap());
        let tx = u128::from_le_bytes((&bytes[16..32]).try_into().unwrap());

        // Return the deserialized MyStruct
        ProtoruneRuneId { block, tx }
    }
}
pub trait BalanceSheetOperations<E: RuntimeEnvironment>: Sized {
    fn new() -> Self;
    fn from_pairs(runes: Vec<ProtoruneRuneId>, balances: Vec<u128>, env: &mut E) -> Self {
        let mut sheet = Self::new();
        for i in 0..runes.len() {
            sheet.set(&runes[i], balances[i], env);
        }
        return sheet;
    }
    fn concat(ary: Vec<Self>, env: &mut E) -> Result<Self> {
        let mut concatenated = Self::new();
        for sheet in ary {
            concatenated = Self::merge(&concatenated, &sheet, env)?;
        }
        Ok(concatenated)
    }
    fn get(&self, rune: &ProtoruneRuneId, env: &mut E) -> u128;

    /// Set the balance for a rune
    fn set(&mut self, rune: &ProtoruneRuneId, value: u128, env: &mut E);

    /// Increase the balance for a rune by the cached amount
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

    /// Decrease the balance for a rune by the cached amount
    fn decrease(&mut self, rune: &ProtoruneRuneId, value: u128, env: &mut E) -> bool {
        let current_balance = self.get(rune, env);
        if current_balance < value {
            false
        } else {
            self.set(rune, current_balance - value, env);
            true
        }
    }

    // pipes a balancesheet onto itself
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

    fn merge(a: &Self, b: &Self, env: &mut E) -> Result<Self>;

    fn merge_sheets(&mut self, a: &Self, b: &Self, env: &mut E) -> Result<()> {
        // Merge balances
        for (rune, balance) in a.balances() {
            self.increase(rune, *balance, env)?;
        }
        for (rune, balance) in b.balances() {
            self.increase(rune, *balance, env)?;
        }
        Ok(())
    }

    /// Get all balances
    fn balances(&self) -> &BTreeMap<ProtoruneRuneId, u128>;
}

/// A basic balance sheet that only stores balances in memory
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct CachedBalanceSheet {
    pub balances: BTreeMap<ProtoruneRuneId, u128>, // Using BTreeMap to map runes to their balances
}

impl<E: RuntimeEnvironment> BalanceSheetOperations<E> for CachedBalanceSheet {
    fn get(&self, rune: &ProtoruneRuneId, _env: &mut E) -> u128 {
        *self.balances.get(rune).unwrap_or(&0u128)
    }

    fn set(&mut self, rune: &ProtoruneRuneId, value: u128, _env: &mut E) {
        self.balances.insert(rune.clone(), value);
    }

    fn new() -> Self {
        CachedBalanceSheet {
            balances: BTreeMap::new(),
        }
    }

    fn merge(a: &CachedBalanceSheet, b: &CachedBalanceSheet, env: &mut E) -> Result<CachedBalanceSheet> {
        let mut merged = CachedBalanceSheet::new();
        merged.merge_sheets(a, b, env)?;
        Ok(merged)
    }

    fn balances(&self) -> &BTreeMap<ProtoruneRuneId, u128> {
        &self.balances
    }
}

// We still need this implementation to customize the equality comparison
impl PartialEq for CachedBalanceSheet {
    fn eq(&self, other: &Self) -> bool {
        self.balances == other.balances
    }
}

// Implementing Eq for CachedBalanceSheet
impl Eq for CachedBalanceSheet {}

/// The full BalanceSheet that extends CachedBalanceSheet with loading functionality
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BalanceSheet<E: RuntimeEnvironment, P: KeyValuePointer<E> + Clone> {
    pub cached: CachedBalanceSheet,
    #[serde(skip)]
    pub load_ptrs: Vec<P>,
    #[serde(skip)]
    _phantom: PhantomData<E>,
}

// We still need this implementation to customize the equality comparison
impl<E: RuntimeEnvironment, P: KeyValuePointer<E> + Clone> PartialEq for BalanceSheet<E, P> {
    fn eq(&self, other: &Self) -> bool {
        // Get all unique rune IDs from both balance sheets
        let mut all_runes = self.balances().keys().collect::<BTreeSet<_>>();
        all_runes.extend(other.balances().keys());

        // Compare balances for each rune using get() which checks both cached and stored values
        for rune in all_runes {
            if self.get(rune, &mut E::default()) != other.get(rune, &mut E::default()) {
                return false;
            }
        }

        true
    }
}

// Implementing Eq for BalanceSheet
impl<E: RuntimeEnvironment, P: KeyValuePointer<E> + Clone> Eq for BalanceSheet<E, P> {}

impl<E: RuntimeEnvironment, P: KeyValuePointer<E> + Clone> Default for BalanceSheet<E, P> {
    fn default() -> Self {
        BalanceSheet {
            cached: CachedBalanceSheet::default(),
            load_ptrs: Vec::new(),
            _phantom: PhantomData,
        }
    }
}



impl<E: RuntimeEnvironment, P: KeyValuePointer<E> + Clone> From<protorune_support::proto::protorune::OutpointResponse>
    for BalanceSheet<E, P>
{
    fn from(v: protorune_support::proto::protorune::OutpointResponse) -> BalanceSheet<E, P> {
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
        BalanceSheet::from_pairs(ids, balances, &mut E::default())
    }
}

impl<E: RuntimeEnvironment, P: KeyValuePointer<E> + Clone> BalanceSheet<E, P> {
    pub fn new_ptr_backed(ptr: P) -> Self {
        BalanceSheet {
            cached: CachedBalanceSheet::new(),
            load_ptrs: vec![ptr],
            _phantom: PhantomData,
        }
    }

    pub fn load_balance(&self, rune: &ProtoruneRuneId, env: &mut E) -> u128 {
        // If already in cache, return it
        if let Some(balance) = self.balances().get(rune) {
            return *balance;
        }

        // Try to load from storage using the stored pointer
        let mut total_stored_balance = 0;
        let rune_clone = rune.clone(); // Clone the rune to avoid borrowing issues

        // First, collect all stored balances
        for ptr in &self.load_ptrs {
            let runes_to_balances_ptr = ptr
                .clone()
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

    pub fn get_cached(&self, rune: &ProtoruneRuneId, env: &mut E) -> u128 {
        self.cached.get(rune, env)
    }
}

impl<E: RuntimeEnvironment, P: KeyValuePointer<E> + Clone> BalanceSheetOperations<E> for BalanceSheet<E, P> {
    fn balances(&self) -> &BTreeMap<ProtoruneRuneId, u128> {
        self.cached.balances()
    }

    fn new() -> Self {
        BalanceSheet {
            cached: CachedBalanceSheet::new(),
            load_ptrs: Vec::new(),
            _phantom: PhantomData,
        }
    }

    fn get(&self, rune: &ProtoruneRuneId, env: &mut E) -> u128 {
        self.load_balance(rune, env)
    }

    fn set(&mut self, rune: &ProtoruneRuneId, value: u128, env: &mut E) {
        self.cached.set(rune, value, env);
    }

    fn merge(a: &BalanceSheet<E, P>, b: &BalanceSheet<E, P>, env: &mut E) -> Result<BalanceSheet<E, P>> {
        let mut merged = BalanceSheet::new();

        // Merge load_ptrs
        merged.load_ptrs.extend(a.load_ptrs.iter().cloned());
        merged.load_ptrs.extend(b.load_ptrs.iter().cloned());

        // Merge balances
        merged.merge_sheets(a, b, env)?;

        Ok(merged)
    }
}

impl<E: RuntimeEnvironment, P: KeyValuePointer<E> + Clone> TryFrom<Vec<RuneTransfer>> for BalanceSheet<E, P> {
    type Error = anyhow::Error;

    fn try_from(v: Vec<RuneTransfer>) -> Result<BalanceSheet<E, P>> {
        let mut balance_sheet = BalanceSheet::new();

        for transfer in v {
            balance_sheet.increase(&transfer.id, transfer.value, &mut E::default())?;
        }

        Ok(balance_sheet)
    }
}

impl<E: RuntimeEnvironment> TryFrom<Vec<RuneTransfer>> for CachedBalanceSheet {
    type Error = anyhow::Error;

    fn try_from(v: Vec<RuneTransfer>) -> Result<CachedBalanceSheet> {
        let mut balance_sheet = CachedBalanceSheet::new();

        for transfer in v {
            balance_sheet.increase(&transfer.id, transfer.value, &mut E::default())?;
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