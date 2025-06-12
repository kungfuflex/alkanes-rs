use crate::proto;
use crate::proto::protorune::{BalanceSheetItem, Rune};
use crate::rune_transfer::RuneTransfer;
use anyhow::{anyhow, Result};
use hex;
use metashrew_support::index_pointer::KeyValuePointer;
use metashrew_support::utils::consume_sized_int;
use ordinals::RuneId;
use protobuf::{MessageField, SpecialFields};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::io::Cursor;
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
        let mut result = crate::proto::protorune::ProtoruneRuneId::new();
        result.height = MessageField::some(v.block.into());
        result.txindex = MessageField::some(v.tx.into());
        result
    }
}

impl<P: KeyValuePointer + Clone> From<crate::proto::protorune::BalanceSheet> for BalanceSheet<P> {
    fn from(balance_sheet: crate::proto::protorune::BalanceSheet) -> BalanceSheet<P> {
        BalanceSheet {
            cached: CachedBalanceSheet {
                balances: HashMap::<ProtoruneRuneId, u128>::from_iter(
                    balance_sheet.entries.into_iter().map(|v| {
                        let id = ProtoruneRuneId::new(
                            v.rune.runeId.height.clone().into_option().unwrap().into(),
                            v.rune.runeId.txindex.clone().into_option().unwrap().into(),
                        );
                        (id, v.balance.into_option().unwrap().into())
                    }),
                ),
            },
            load_ptrs: Vec::new(),
        }
    }
}

impl<P: KeyValuePointer + Clone> From<BalanceSheet<P>> for crate::proto::protorune::BalanceSheet {
    fn from(balance_sheet: BalanceSheet<P>) -> crate::proto::protorune::BalanceSheet {
        crate::proto::protorune::BalanceSheet {
            entries: balance_sheet
                .balances()
                .clone()
                .iter()
                .map(|(k, v)| BalanceSheetItem {
                    special_fields: SpecialFields::new(),
                    rune: MessageField::some(Rune {
                        special_fields: SpecialFields::new(),
                        runeId: MessageField::some(proto::protorune::ProtoruneRuneId {
                            special_fields: SpecialFields::new(),
                            height: MessageField::some(k.block.into()),
                            txindex: MessageField::some(k.tx.into()),
                        }),
                        name: "UNKNOWN".to_owned(),
                        divisibility: 1,
                        spacers: 1,
                        symbol: "0".to_owned(),
                    }),
                    balance: MessageField::some((*v).into()),
                })
                .collect::<Vec<BalanceSheetItem>>(),
            special_fields: SpecialFields::new(),
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

/*
impl fmt::Display for ProtoruneRuneId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "RuneId {{ block: {}, tx: {} }}", self.block, self.tx)
    }
}
*/

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

pub trait BalanceSheetOperations: Sized {
    fn new() -> Self;
    fn from_pairs(runes: Vec<ProtoruneRuneId>, balances: Vec<u128>) -> Self {
        let mut sheet = Self::new();
        for i in 0..runes.len() {
            sheet.set(&runes[i], balances[i]);
        }
        return sheet;
    }
    fn concat(ary: Vec<Self>) -> Result<Self> {
        let mut concatenated = Self::new();
        for sheet in ary {
            concatenated = Self::merge(&concatenated, &sheet)?;
        }
        Ok(concatenated)
    }
    fn get(&self, rune: &ProtoruneRuneId) -> u128;

    /// Set the balance for a rune
    fn set(&mut self, rune: &ProtoruneRuneId, value: u128);

    /// Increase the balance for a rune by the cached amount
    fn increase(&mut self, rune: &ProtoruneRuneId, value: u128) -> Result<()> {
        let current_balance = self.get(rune);
        self.set(
            rune,
            current_balance.checked_add(value).ok_or("").map_err(|_| {
                anyhow!(format!(
                    "overflow error during balance sheet increase, current({}) + additional({})",
                    current_balance, value
                ))
            })?,
        );
        Ok(())
    }

    /// Decrease the balance for a rune by the cached amount
    fn decrease(&mut self, rune: &ProtoruneRuneId, value: u128) -> bool {
        let current_balance = self.get(rune);
        if current_balance < value {
            false
        } else {
            self.set(rune, current_balance - value);
            true
        }
    }

    // pipes a balancesheet onto itself
    fn pipe(&self, sheet: &mut Self) -> Result<()> {
        for (rune, balance) in self.balances() {
            sheet.increase(rune, *balance)?;
        }
        Ok(())
    }

    fn debit(&mut self, sheet: &Self) -> Result<()> {
        for (rune, balance) in sheet.balances() {
            if *balance <= self.get(&rune) {
                self.decrease(rune, *balance);
            } else {
                return Err(anyhow!("balance underflow"));
            }
        }
        Ok(())
    }

    fn rune_debit(&mut self, sheet: &Self) -> Result<()> {
        self.debit(sheet)
    }

    fn merge(a: &Self, b: &Self) -> Result<Self>;

    fn merge_sheets(&mut self, a: &Self, b: &Self) -> Result<()> {
        // Merge balances
        for (rune, balance) in a.balances() {
            self.increase(rune, *balance)?;
        }
        for (rune, balance) in b.balances() {
            self.increase(rune, *balance)?;
        }
        Ok(())
    }

    /// Get all balances
    fn balances(&self) -> &HashMap<ProtoruneRuneId, u128>;
}

/// A basic balance sheet that only stores balances in memory
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct CachedBalanceSheet {
    pub balances: HashMap<ProtoruneRuneId, u128>, // Using HashMap to map runes to their balances
}

impl BalanceSheetOperations for CachedBalanceSheet {
    fn get(&self, rune: &ProtoruneRuneId) -> u128 {
        *self.balances.get(rune).unwrap_or(&0u128)
    }

    fn set(&mut self, rune: &ProtoruneRuneId, value: u128) {
        self.balances.insert(rune.clone(), value);
    }

    fn new() -> Self {
        CachedBalanceSheet {
            balances: HashMap::new(),
        }
    }

    fn merge(a: &CachedBalanceSheet, b: &CachedBalanceSheet) -> Result<CachedBalanceSheet> {
        let mut merged = CachedBalanceSheet::new();
        merged.merge_sheets(a, b)?;
        Ok(merged)
    }

    fn balances(&self) -> &HashMap<ProtoruneRuneId, u128> {
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
pub struct BalanceSheet<P: KeyValuePointer + Clone> {
    pub cached: CachedBalanceSheet,
    #[serde(skip)]
    pub load_ptrs: Vec<P>,
}

// We still need this implementation to customize the equality comparison
impl<P: KeyValuePointer + Clone> PartialEq for BalanceSheet<P> {
    fn eq(&self, other: &Self) -> bool {
        // Get all unique rune IDs from both balance sheets
        let mut all_runes = self.balances().keys().collect::<HashSet<_>>();
        all_runes.extend(other.balances().keys());

        // Compare balances for each rune using get() which checks both cached and stored values
        for rune in all_runes {
            if self.get(rune) != other.get(rune) {
                return false;
            }
        }

        true
    }
}

// Implementing Eq for BalanceSheet
impl<P: KeyValuePointer + Clone> Eq for BalanceSheet<P> {}

impl<P: KeyValuePointer + Clone> Default for BalanceSheet<P> {
    fn default() -> Self {
        BalanceSheet {
            cached: CachedBalanceSheet::default(),
            load_ptrs: Vec::new(),
        }
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
            crate::proto::protorune::Uint128::new();
        container.lo = u64::from_le_bytes((&bytes[0..8]).try_into().unwrap());
        container.hi = u64::from_le_bytes((&bytes[8..16]).try_into().unwrap());
        container
    }
}

impl<P: KeyValuePointer + Clone> From<crate::proto::protorune::OutpointResponse>
    for BalanceSheet<P>
{
    fn from(v: crate::proto::protorune::OutpointResponse) -> BalanceSheet<P> {
        let pairs = v
            .balances
            .entries
            .clone()
            .into_iter()
            .map(|v| {
                (
                    ProtoruneRuneId::new(
                        v.rune
                            .clone()
                            .unwrap()
                            .runeId
                            .unwrap()
                            .height
                            .unwrap()
                            .into(),
                        v.rune.unwrap().runeId.unwrap().txindex.unwrap().into(),
                    ),
                    v.balance.into_option().unwrap().into(),
                )
            })
            .collect::<Vec<(ProtoruneRuneId, u128)>>();
        let ids = pairs
            .iter()
            .map(|(id, _)| id.clone())
            .collect::<Vec<ProtoruneRuneId>>();
        let balances = pairs.iter().map(|(_, v)| v.clone()).collect::<Vec<u128>>();
        BalanceSheet::from_pairs(ids, balances)
    }
}

impl<P: KeyValuePointer + Clone> BalanceSheet<P> {
    pub fn new_ptr_backed(ptr: P) -> Self {
        BalanceSheet {
            cached: CachedBalanceSheet::new(),
            load_ptrs: vec![ptr],
        }
    }

    pub fn load_balance(&self, rune: &ProtoruneRuneId) -> u128 {
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
            if runes_to_balances_ptr.get().len() != 0 {
                let stored_balance = runes_to_balances_ptr.get_value::<u128>();
                total_stored_balance += stored_balance;
            }
        }
        return total_stored_balance;
    }

    pub fn get_and_update(&mut self, rune: &ProtoruneRuneId) -> u128 {
        let balance = self.load_balance(rune);
        self.set(rune, balance);
        balance
    }

    pub fn get_cached(&self, rune: &ProtoruneRuneId) -> u128 {
        self.cached.get(rune)
    }
}

impl<P: KeyValuePointer + Clone> BalanceSheetOperations for BalanceSheet<P> {
    fn balances(&self) -> &HashMap<ProtoruneRuneId, u128> {
        self.cached.balances()
    }

    fn new() -> Self {
        BalanceSheet {
            cached: CachedBalanceSheet::new(),
            load_ptrs: Vec::new(),
        }
    }

    fn get(&self, rune: &ProtoruneRuneId) -> u128 {
        self.load_balance(rune)
    }

    fn set(&mut self, rune: &ProtoruneRuneId, value: u128) {
        self.cached.set(rune, value);
    }

    fn merge(a: &BalanceSheet<P>, b: &BalanceSheet<P>) -> Result<BalanceSheet<P>> {
        let mut merged = BalanceSheet::new();

        // Merge load_ptrs
        merged.load_ptrs.extend(a.load_ptrs.iter().cloned());
        merged.load_ptrs.extend(b.load_ptrs.iter().cloned());

        // Merge balances
        merged.merge_sheets(a, b)?;

        Ok(merged)
    }
}

impl<P: KeyValuePointer + Clone> TryFrom<Vec<RuneTransfer>> for BalanceSheet<P> {
    type Error = anyhow::Error;

    fn try_from(v: Vec<RuneTransfer>) -> Result<BalanceSheet<P>> {
        let mut balance_sheet = BalanceSheet::new();

        for transfer in v {
            balance_sheet.increase(&transfer.id, transfer.value)?;
        }

        Ok(balance_sheet)
    }
}

impl TryFrom<Vec<RuneTransfer>> for CachedBalanceSheet {
    type Error = anyhow::Error;

    fn try_from(v: Vec<RuneTransfer>) -> Result<CachedBalanceSheet> {
        let mut balance_sheet = CachedBalanceSheet::new();

        for transfer in v {
            balance_sheet.increase(&transfer.id, transfer.value)?;
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
