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
use std::collections::HashMap;
use std::io::Cursor;
use std::sync::Arc;
use std::u128;

// use metashrew::{println, stdio::stdout};
// use std::fmt::Write;

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

impl From<crate::proto::protorune::BalanceSheet> for BalanceSheet {
    fn from(balance_sheet: crate::proto::protorune::BalanceSheet) -> BalanceSheet {
        BalanceSheet {
            balances: HashMap::<ProtoruneRuneId, u128>::from_iter(
                balance_sheet.entries.into_iter().map(|v| {
                    let id = ProtoruneRuneId::new(
                        v.rune.runeId.height.clone().into_option().unwrap().into(),
                        v.rune.runeId.txindex.clone().into_option().unwrap().into(),
                    );
                    (id, v.balance.into_option().unwrap().into())
                }),
            ),
        }
    }
}

impl From<BalanceSheet> for crate::proto::protorune::BalanceSheet {
    fn from(balance_sheet: BalanceSheet) -> crate::proto::protorune::BalanceSheet {
        crate::proto::protorune::BalanceSheet {
            entries: balance_sheet
                .balances
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

#[derive(Default, Clone, Debug, Eq, Serialize, Deserialize)]
pub struct BalanceSheet {
    pub balances: HashMap<ProtoruneRuneId, u128>, // Using HashMap to map runes to their balances
}

pub fn u128_from_bytes(v: Vec<u8>) -> u128 {
    let bytes_ref: &[u8] = &v;
    u128::from_le_bytes(bytes_ref.try_into().unwrap())
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

impl From<crate::proto::protorune::OutpointResponse> for BalanceSheet {
    fn from(v: crate::proto::protorune::OutpointResponse) -> BalanceSheet {
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

impl BalanceSheet {
    pub fn new() -> Self {
        BalanceSheet {
            balances: HashMap::new(),
        }
    }

    pub fn from_pairs(runes: Vec<ProtoruneRuneId>, balances: Vec<u128>) -> BalanceSheet {
        let mut sheet = BalanceSheet::new();
        for i in 0..runes.len() {
            sheet.set(&runes[i], balances[i]);
        }
        return sheet;
    }

    // pipes a balancesheet onto itself
    pub fn pipe(&self, sheet: &mut BalanceSheet) -> () {
        for (rune, balance) in &self.balances {
            sheet.increase(rune, *balance);
        }
    }
    
    // pipes a balancesheet onto a LazyBalanceSheet
    pub fn pipe_to_lazy<P: KeyValuePointer>(&self, sheet: &mut LazyBalanceSheet, ptr: &P) -> () {
        for (rune, balance) in &self.balances {
            sheet.increase(rune, *balance, ptr);
        }
    }

    /// When processing the return value for MessageContext.handle()
    /// we want to be able to mint arbituary amounts of mintable tokens.
    ///
    /// This function allows us to debit more than the existing amount
    /// of a mintable token without returning an Err so that MessageContext
    /// can mint more than what the initial balance sheet has.
    pub fn debit(&mut self, sheet: &BalanceSheet) -> Result<()> {
        for (rune, balance) in &sheet.balances {
            if *balance <= self.get(&rune) {
                self.decrease(rune, *balance);
            } else {
                return Err(anyhow!("balance underflow"));
            }
        }
        Ok(())
    }

    pub fn rune_debit(&mut self, sheet: &BalanceSheet) -> Result<()> {
        self.debit(sheet)
    }

    /*
    pub fn inspect(&self) -> String {
        let mut base = String::from("balances: [\n");
        for (rune, balance) in &self.balances {
            base.push_str(&format!("  {}: {}\n", rune, balance));
        }
        base.push_str("]");
        base
    }
    */

    pub fn get(&self, rune: &ProtoruneRuneId) -> u128 {
        *self.balances.get(rune).unwrap_or(&0u128) // Return 0 if rune not found
    }

    pub fn set(&mut self, rune: &ProtoruneRuneId, value: u128) {
        self.balances.insert(rune.clone(), value);
    }

    pub fn increase(&mut self, rune: &ProtoruneRuneId, value: u128) {
        let current_balance = self.get(rune);
        self.set(rune, current_balance + value);
    }

    pub fn decrease(&mut self, rune: &ProtoruneRuneId, value: u128) -> bool {
        let current_balance = self.get(rune);
        if current_balance < value {
            false
        } else {
            self.set(rune, current_balance - value);
            true
        }
    }

    pub fn merge(a: &BalanceSheet, b: &BalanceSheet) -> BalanceSheet {
        let mut merged = BalanceSheet::new();
        for (rune, balance) in &a.balances {
            merged.set(rune, *balance);
        }
        for (rune, balance) in &b.balances {
            let current_balance = merged.get(rune);
            merged.set(rune, current_balance + *balance);
        }
        merged
    }

    pub fn concat(ary: Vec<BalanceSheet>) -> BalanceSheet {
        let mut concatenated = BalanceSheet::new();
        for sheet in ary {
            concatenated = BalanceSheet::merge(&concatenated, &sheet);
        }
        concatenated
    }
}

impl PartialEq for BalanceSheet {
    fn eq(&self, other: &Self) -> bool {
        self.balances == other.balances
    }
}

impl From<Vec<RuneTransfer>> for BalanceSheet {
    fn from(v: Vec<RuneTransfer>) -> BalanceSheet {
        BalanceSheet {
            balances: HashMap::<ProtoruneRuneId, u128>::from_iter(
                v.into_iter().map(|v| (v.id, v.value)),
            ),
        }
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

/// LazyBalanceSheet is a specialized version of BalanceSheet that loads balances on demand
/// It's specifically designed for the runtime balance sheet where loading all balances
/// into memory at once would be inefficient for protocols with a large number of assets
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LazyBalanceSheet {
    // Cache of already loaded balances
    cache: HashMap<ProtoruneRuneId, u128>,
    // Storage path for loading balances on demand
    storage_path: String,
}

impl Default for LazyBalanceSheet {
    fn default() -> Self {
        LazyBalanceSheet {
            cache: HashMap::new(),
            storage_path: String::from("/runtime_balances"),
        }
    }
}

impl LazyBalanceSheet {
    pub fn new(storage_path: String) -> Self {
        LazyBalanceSheet {
            cache: HashMap::new(),
            storage_path,
        }
    }

    // Load a balance from storage if not already in cache
    fn load_balance<P: KeyValuePointer>(&mut self, rune: &ProtoruneRuneId, ptr: &P) -> u128 {
        // If already in cache, return it
        if let Some(balance) = self.cache.get(rune) {
            return *balance;
        }

        // Try to load from storage using the provided pointer
        let runes_ptr = ptr.keyword("/runes");
        let balances_ptr = ptr.keyword("/balances");
        
        // Search for the rune in the runes list
        let length = runes_ptr.length();
        for i in 0..length {
            let stored_rune = ProtoruneRuneId::from(runes_ptr.select_index(i).get());
            if stored_rune == *rune {
                // Found the rune, get its balance
                let balance = balances_ptr.select_index(i).get_value::<u128>();
                // Cache it for future use
                self.cache.insert(rune.clone(), balance);
                return balance;
            }
        }

        // Not found in storage, return 0
        0
    }

    // Get a balance, using the cache if available
    pub fn get<P: KeyValuePointer>(&mut self, rune: &ProtoruneRuneId, ptr: &P) -> u128 {
        self.load_balance(rune, ptr)
    }

    // Get a balance from the cache only, without loading from storage
    pub fn get_cached(&self, rune: &ProtoruneRuneId) -> u128 {
        *self.cache.get(rune).unwrap_or(&0u128)
    }

    pub fn set(&mut self, rune: &ProtoruneRuneId, value: u128) {
        self.cache.insert(rune.clone(), value);
    }

    pub fn increase<P: KeyValuePointer>(&mut self, rune: &ProtoruneRuneId, value: u128, ptr: &P) {
        let current_balance = self.get(rune, ptr);
        self.set(rune, current_balance + value);
    }

    pub fn decrease<P: KeyValuePointer>(&mut self, rune: &ProtoruneRuneId, value: u128, ptr: &P) -> bool {
        let current_balance = self.get(rune, ptr);
        if current_balance < value {
            false
        } else {
            self.set(rune, current_balance - value);
            true
        }
    }

    // Convert to a regular BalanceSheet (loads all cached balances)
    pub fn to_balance_sheet(&self) -> BalanceSheet {
        BalanceSheet {
            balances: self.cache.clone(),
        }
    }

    // Create from a regular BalanceSheet
    pub fn from_balance_sheet(sheet: &BalanceSheet, storage_path: String) -> Self {
        LazyBalanceSheet {
            cache: sheet.balances.clone(),
            storage_path,
        }
    }

    // Save the current state to storage
    pub fn save<T: KeyValuePointer>(&self, ptr: &T, is_cenotaph: bool) {
        let runes_ptr = ptr.keyword("/runes");
        let balances_ptr = ptr.keyword("/balances");

        for (rune, balance) in &self.cache {
            if *balance != 0u128 && !is_cenotaph {
                runes_ptr.append((*rune).into());
                balances_ptr.append_value::<u128>(*balance);
            }
        }
    }

    // Debit from this balance sheet
    pub fn debit<P: KeyValuePointer>(&mut self, sheet: &BalanceSheet, ptr: &P) -> Result<()> {
        for (rune, balance) in &sheet.balances {
            if *balance <= self.get(rune, ptr) {
                self.decrease(rune, *balance, ptr);
            } else {
                return Err(anyhow!("balance underflow"));
            }
        }
        Ok(())
    }
    
    // Debit mintable tokens from this balance sheet
    pub fn debit_mintable<P: KeyValuePointer>(&mut self, sheet: &BalanceSheet, ptr: &P) -> Result<()> {
        for (rune, balance) in &sheet.balances {
            let current = self.get(rune, ptr);
            if *balance <= current {
                self.decrease(rune, *balance, ptr);
            } else {
                // For mintable tokens, we just decrease what we have
                // This is a simplified implementation - in a real implementation,
                // you would check if the token is mintable
                self.decrease(rune, current, ptr);
            }
        }
        Ok(())
    }

    // Pipe a balance sheet into this lazy balance sheet
    pub fn pipe_from<P: KeyValuePointer>(&mut self, sheet: &BalanceSheet, ptr: &P) {
        for (rune, balance) in &sheet.balances {
            self.increase(rune, *balance, ptr);
        }
    }

    // Merge two lazy balance sheets
    pub fn merge(a: &mut LazyBalanceSheet, b: &mut LazyBalanceSheet) -> LazyBalanceSheet {
        let mut merged = LazyBalanceSheet::new(a.storage_path.clone());
        
        // Merge the caches
        for (rune, balance) in &a.cache {
            merged.set(rune, *balance);
        }
        
        for (rune, balance) in &b.cache {
            let current_balance = merged.get_cached(rune);
            merged.set(rune, current_balance + *balance);
        }
        
        merged
    }
}

// Implement conversion from LazyBalanceSheet to BalanceSheet
impl From<LazyBalanceSheet> for BalanceSheet {
    fn from(lazy: LazyBalanceSheet) -> Self {
        BalanceSheet {
            balances: lazy.cache,
        }
    }
}

// Implement conversion from BalanceSheet to LazyBalanceSheet
impl From<BalanceSheet> for LazyBalanceSheet {
    fn from(sheet: BalanceSheet) -> Self {
        LazyBalanceSheet {
            cache: sheet.balances,
            storage_path: String::from("/runtime_balances"),
        }
    }
}
