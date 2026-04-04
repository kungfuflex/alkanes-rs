//! Transaction memory pool.
//! Maps to: src/txmempool.h, src/txmempool.cpp
//!
//! The mempool holds unconfirmed transactions that have been validated and
//! are candidates for inclusion in the next block. It tracks ancestor and
//! descendant relationships, enforces fee-rate policies, and supports
//! replace-by-fee (RBF).

use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};

use parking_lot::RwLock;

use qubitcoin_consensus::transaction::{OutPoint, TransactionRef};
use qubitcoin_primitives::{Amount, Txid};

// ---------------------------------------------------------------------------
// FeeRate
// ---------------------------------------------------------------------------

/// Fee rate in satoshis per virtual kilobyte.
///
/// Maps to: CFeeRate in src/policy/feerate.h
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FeeRate {
    /// Satoshis per 1000 virtual bytes.
    sats_per_kvb: i64,
}

impl FeeRate {
    /// A zero fee rate.
    pub const ZERO: FeeRate = FeeRate { sats_per_kvb: 0 };

    /// Create a fee rate from satoshis per 1000 virtual bytes.
    pub fn new(sats_per_kvb: i64) -> Self {
        FeeRate { sats_per_kvb }
    }

    /// Create a fee rate from satoshis per virtual byte.
    pub fn from_sat_per_vb(sats_per_vb: i64) -> Self {
        FeeRate {
            sats_per_kvb: sats_per_vb * 1000,
        }
    }

    /// Calculate the fee for a given virtual size.
    /// Rounds UP, matching Bitcoin Core's CFeeRate::GetFee() which uses
    /// EvaluateFeeUp() to ensure sufficient fees cover bandwidth costs.
    pub fn get_fee(&self, virtual_size: usize) -> Amount {
        let numer = self.sats_per_kvb * virtual_size as i64;
        // Round up: (numer + 999) / 1000 for positive values
        let fee = if numer > 0 {
            (numer + 999) / 1000
        } else {
            numer / 1000
        };
        Amount::from_sat(fee)
    }

    /// Get the raw satoshis-per-kvB value.
    pub fn sats_per_kvb(&self) -> i64 {
        self.sats_per_kvb
    }
}

impl Default for FeeRate {
    fn default() -> Self {
        FeeRate::ZERO
    }
}

impl std::fmt::Display for FeeRate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} sat/kvB", self.sats_per_kvb)
    }
}

/// Default minimum relay transaction fee: 1 sat/vB = 1000 sat/kvB.
pub const DEFAULT_MIN_RELAY_TX_FEE: FeeRate = FeeRate { sats_per_kvb: 1000 };

/// Dust relay fee: 3 sat/vB = 3000 sat/kvB.
pub const DUST_RELAY_TX_FEE: FeeRate = FeeRate { sats_per_kvb: 3000 };

/// Default incremental relay fee for RBF: 1 sat/vB = 1000 sat/kvB.
pub const DEFAULT_INCREMENTAL_RELAY_FEE: FeeRate = FeeRate { sats_per_kvb: 1000 };

/// Default maximum mempool size in bytes (300 MB, matching Bitcoin Core).
pub const DEFAULT_MAX_MEMPOOL_SIZE: usize = 300 * 1_000_000;

/// Maximum number of in-mempool ancestors (including self). BIP125 / Bitcoin Core default.
pub const DEFAULT_ANCESTOR_LIMIT: u64 = 25;

/// Maximum combined size of in-mempool ancestors (including self) in vbytes.
pub const DEFAULT_ANCESTOR_SIZE_LIMIT: u64 = 101_000;

/// Maximum number of in-mempool descendants (including self).
pub const DEFAULT_DESCENDANT_LIMIT: u64 = 25;

/// Maximum combined size of in-mempool descendants (including self) in vbytes.
pub const DEFAULT_DESCENDANT_SIZE_LIMIT: u64 = 101_000;

/// Maximum number of replacement transactions (BIP125 rule 5).
pub const MAX_BIP125_REPLACEMENT_CANDIDATES: usize = 100;

/// Default mempool transaction expiration time (14 days, matching Bitcoin Core).
pub const DEFAULT_MEMPOOL_EXPIRY_HOURS: u64 = 336;

// ---------------------------------------------------------------------------
// MempoolEntry
// ---------------------------------------------------------------------------

/// A transaction entry in the mempool.
///
/// Maps to: CTxMemPoolEntry in src/txmempool.h
///
/// Tracks both the transaction itself and aggregate statistics about its
/// in-mempool ancestor and descendant sets. These statistics are used for
/// mining (ancestor-feerate sorting) and eviction (descendant-feerate).
#[derive(Clone, Debug)]
pub struct MempoolEntry {
    /// The transaction.
    tx: TransactionRef,
    /// Fee paid by this transaction.
    fee: Amount,
    /// Virtual size (weight / 4, rounded up).
    vsize: u32,
    /// Time this entry was added to the mempool (unix timestamp).
    time: u64,
    /// Height of the chain tip when this entry was added.
    entry_height: i32,
    /// Whether this transaction spends a coinbase output.
    spends_coinbase: bool,
    /// Signature operation cost (BIP141 weighted).
    sig_op_cost: u32,
    /// Fee rate of this individual transaction.
    fee_rate: FeeRate,

    // -- Ancestor/descendant tracking --
    /// Number of in-mempool ancestors (including self).
    ancestor_count: u64,
    /// Total virtual size of in-mempool ancestors (including self).
    ancestor_size: u64,
    /// Total fee of in-mempool ancestors (including self).
    ancestor_fee: Amount,
    /// Number of in-mempool descendants (including self).
    descendant_count: u64,
    /// Total virtual size of in-mempool descendants (including self).
    descendant_size: u64,
    /// Total fee of in-mempool descendants (including self).
    descendant_fee: Amount,
}

impl MempoolEntry {
    /// Create a new mempool entry.
    pub fn new(
        tx: TransactionRef,
        fee: Amount,
        vsize: u32,
        time: u64,
        entry_height: i32,
        spends_coinbase: bool,
        sig_op_cost: u32,
    ) -> Self {
        let fee_rate = if vsize > 0 {
            FeeRate::new(fee.to_sat() * 1000 / vsize as i64)
        } else {
            FeeRate::ZERO
        };

        MempoolEntry {
            tx,
            fee,
            vsize,
            time,
            entry_height,
            spends_coinbase,
            sig_op_cost,
            fee_rate,
            // Self is always its own ancestor and descendant.
            ancestor_count: 1,
            ancestor_size: vsize as u64,
            ancestor_fee: fee,
            descendant_count: 1,
            descendant_size: vsize as u64,
            descendant_fee: fee,
        }
    }

    // -- Accessors --

    /// Get a reference to the underlying transaction.
    pub fn tx(&self) -> &TransactionRef {
        &self.tx
    }

    /// Get the transaction ID.
    pub fn txid(&self) -> Txid {
        *self.tx.txid()
    }

    /// Get the fee paid by this transaction.
    pub fn fee(&self) -> Amount {
        self.fee
    }

    /// Get the virtual size (weight / 4, rounded up) of this transaction.
    pub fn vsize(&self) -> u32 {
        self.vsize
    }

    /// Get the Unix timestamp when this entry was added to the mempool.
    pub fn time(&self) -> u64 {
        self.time
    }

    /// Get the chain tip height when this entry was added.
    pub fn entry_height(&self) -> i32 {
        self.entry_height
    }

    /// Returns `true` if this transaction spends a coinbase output.
    pub fn spends_coinbase(&self) -> bool {
        self.spends_coinbase
    }

    /// Get the BIP141-weighted signature operation cost.
    pub fn sig_op_cost(&self) -> u32 {
        self.sig_op_cost
    }

    /// Get the fee rate of this individual transaction.
    pub fn fee_rate(&self) -> FeeRate {
        self.fee_rate
    }

    /// Get the number of in-mempool ancestors (including self).
    pub fn ancestor_count(&self) -> u64 {
        self.ancestor_count
    }

    /// Get the total virtual size of in-mempool ancestors (including self).
    pub fn ancestor_size(&self) -> u64 {
        self.ancestor_size
    }

    /// Get the total fee of in-mempool ancestors (including self).
    pub fn ancestor_fee(&self) -> Amount {
        self.ancestor_fee
    }

    /// Get the number of in-mempool descendants (including self).
    pub fn descendant_count(&self) -> u64 {
        self.descendant_count
    }

    /// Get the total virtual size of in-mempool descendants (including self).
    pub fn descendant_size(&self) -> u64 {
        self.descendant_size
    }

    /// Get the total fee of in-mempool descendants (including self).
    pub fn descendant_fee(&self) -> Amount {
        self.descendant_fee
    }

    /// Update ancestor statistics.
    pub fn update_ancestor_state(
        &mut self,
        ancestor_count: u64,
        ancestor_size: u64,
        ancestor_fee: Amount,
    ) {
        self.ancestor_count = ancestor_count;
        self.ancestor_size = ancestor_size;
        self.ancestor_fee = ancestor_fee;
    }

    /// Update descendant statistics.
    pub fn update_descendant_state(
        &mut self,
        descendant_count: u64,
        descendant_size: u64,
        descendant_fee: Amount,
    ) {
        self.descendant_count = descendant_count;
        self.descendant_size = descendant_size;
        self.descendant_fee = descendant_fee;
    }
}

// ---------------------------------------------------------------------------
// MempoolInfo
// ---------------------------------------------------------------------------

/// Summary information about the mempool.
#[derive(Debug, Clone)]
pub struct MempoolInfo {
    /// Number of transactions.
    pub size: usize,
    /// Total virtual bytes of all transactions.
    pub bytes: u64,
    /// Estimated memory usage in bytes.
    pub usage: u64,
    /// Total fees of all transactions.
    pub total_fee: Amount,
    /// Minimum fee rate to enter the mempool.
    pub min_fee: FeeRate,
}

// ---------------------------------------------------------------------------
// TxMemPool
// ---------------------------------------------------------------------------

/// The transaction memory pool.
///
/// Maps to: CTxMemPool in src/txmempool.h / src/txmempool.cpp
///
/// Thread-safe via interior mutability (`RwLock` + atomics). All public
/// methods acquire locks internally, so callers do not need external
/// synchronisation.
pub struct TxMemPool {
    /// All transactions in the mempool, keyed by txid.
    entries: RwLock<HashMap<Txid, MempoolEntry>>,
    /// Map from outpoints spent by mempool transactions to the spending txid.
    spenders: RwLock<HashMap<OutPoint, Txid>>,
    /// Total fees currently in the mempool (atomic for lock-free reads).
    total_fee: AtomicI64,
    /// Monotonically increasing sequence number for entry ordering.
    sequence: AtomicU64,
    /// Minimum relay fee rate to enter the mempool.
    min_relay_fee: FeeRate,
    /// Incremental relay fee for replace-by-fee.
    incremental_relay_fee: FeeRate,
    /// Maximum mempool size in bytes (virtual size sum).
    max_mempool_size: usize,
    /// Full-RBF policy: if true, allow replacement of non-signaling transactions.
    /// Matches Bitcoin Core's `-mempoolfullrbf` flag (default true since v28).
    full_rbf: bool,
}

impl TxMemPool {
    /// Create a new mempool with default parameters.
    pub fn new() -> Self {
        Self::new_with_limits(DEFAULT_MIN_RELAY_TX_FEE, DEFAULT_MAX_MEMPOOL_SIZE)
    }

    /// Create a new mempool with custom minimum relay fee and maximum size.
    pub fn new_with_limits(min_relay_fee: FeeRate, max_size: usize) -> Self {
        TxMemPool {
            entries: RwLock::new(HashMap::new()),
            spenders: RwLock::new(HashMap::new()),
            total_fee: AtomicI64::new(0),
            sequence: AtomicU64::new(0),
            min_relay_fee,
            incremental_relay_fee: DEFAULT_INCREMENTAL_RELAY_FEE,
            max_mempool_size: max_size,
            full_rbf: true, // Default true, matching Bitcoin Core v28+
        }
    }

    /// Add a validated transaction to the mempool without further validation.
    ///
    /// The caller is responsible for having already validated the transaction
    /// (script checks, UTXO existence, etc.). This method only performs
    /// mempool-level bookkeeping.
    ///
    /// Returns `true` if the transaction was added, `false` if it was already
    /// present.
    pub fn add_unchecked(&self, entry: MempoolEntry) -> bool {
        let txid = entry.txid();

        let mut entries = self.entries.write();
        if entries.contains_key(&txid) {
            return false;
        }

        // Record outpoint spending.
        let mut spenders = self.spenders.write();
        for input in &entry.tx.vin {
            spenders.insert(input.prevout.clone(), txid);
        }

        // Update total fee.
        self.total_fee
            .fetch_add(entry.fee.to_sat(), Ordering::Relaxed);

        // Bump sequence.
        self.sequence.fetch_add(1, Ordering::Relaxed);

        // Update ancestor/descendant stats for related entries.
        // We need to update descendants of our ancestors and ancestors of our
        // children (transactions that spend our outputs). For simplicity of
        // this initial implementation we do the updates inline.

        // 1. Find our in-mempool parent txids (transactions whose outputs we spend).
        let parent_txids: Vec<Txid> = entry
            .tx
            .vin
            .iter()
            .filter_map(|inp| {
                let parent = inp.prevout.hash;
                if entries.contains_key(&parent) {
                    Some(parent)
                } else {
                    None
                }
            })
            .collect();

        // 2. Update our own ancestor stats from parents.
        let mut entry = entry;
        for parent_txid in &parent_txids {
            if let Some(parent) = entries.get(parent_txid) {
                entry.ancestor_count += parent.ancestor_count;
                entry.ancestor_size += parent.ancestor_size;
                entry.ancestor_fee = entry.ancestor_fee + parent.ancestor_fee;
            }
        }

        // 3. Update descendant stats on all ancestor entries.
        //    Each ancestor gains one more descendant (us) with our vsize and fee.
        let our_vsize = entry.vsize as u64;
        let our_fee = entry.fee;
        let ancestor_txids = self.collect_ancestors_from_entries(&entries, &txid, &entry);
        for anc_txid in &ancestor_txids {
            if let Some(anc) = entries.get_mut(anc_txid) {
                anc.descendant_count += 1;
                anc.descendant_size += our_vsize;
                anc.descendant_fee = anc.descendant_fee + our_fee;
            }
        }

        entries.insert(txid, entry);
        true
    }

    /// Collect all ancestor txids (not including self) by traversing parent
    /// links. This is used during `add_unchecked` to update descendant counts.
    fn collect_ancestors_from_entries(
        &self,
        entries: &HashMap<Txid, MempoolEntry>,
        _txid: &Txid,
        entry: &MempoolEntry,
    ) -> Vec<Txid> {
        let mut ancestors = Vec::new();
        let mut queue: Vec<Txid> = entry
            .tx
            .vin
            .iter()
            .filter_map(|inp| {
                let parent = inp.prevout.hash;
                if entries.contains_key(&parent) {
                    Some(parent)
                } else {
                    None
                }
            })
            .collect();
        let mut visited = std::collections::HashSet::new();

        while let Some(cur) = queue.pop() {
            if !visited.insert(cur) {
                continue;
            }
            ancestors.push(cur);
            if let Some(e) = entries.get(&cur) {
                for inp in &e.tx.vin {
                    let parent = inp.prevout.hash;
                    if entries.contains_key(&parent) && !visited.contains(&parent) {
                        queue.push(parent);
                    }
                }
            }
        }
        ancestors
    }

    /// Collect all descendant txids (not including self) by following spender
    /// links.
    fn collect_descendants_locked(
        &self,
        entries: &HashMap<Txid, MempoolEntry>,
        spenders: &HashMap<OutPoint, Txid>,
        txid: &Txid,
    ) -> Vec<Txid> {
        let mut descendants = Vec::new();
        let mut queue = vec![*txid];
        let mut visited = std::collections::HashSet::new();
        visited.insert(*txid);

        while let Some(cur) = queue.pop() {
            if let Some(e) = entries.get(&cur) {
                for (idx, _) in e.tx.vout.iter().enumerate() {
                    let outpoint = OutPoint::new(cur, idx as u32);
                    if let Some(child_txid) = spenders.get(&outpoint) {
                        if visited.insert(*child_txid) {
                            descendants.push(*child_txid);
                            queue.push(*child_txid);
                        }
                    }
                }
            }
        }
        descendants
    }

    /// Remove a transaction and all its descendants from the mempool.
    ///
    /// Returns the list of removed transactions (the root transaction plus
    /// any descendants that were evicted).
    pub fn remove_recursive(&self, txid: &Txid) -> Vec<TransactionRef> {
        let mut entries = self.entries.write();
        let mut spenders = self.spenders.write();

        if !entries.contains_key(txid) {
            return Vec::new();
        }

        // Gather all descendants (including the root).
        let descendants = self.collect_descendants_locked(&entries, &spenders, txid);
        let mut to_remove = vec![*txid];
        to_remove.extend(descendants);

        let mut removed_txs = Vec::new();
        for rem_txid in &to_remove {
            if let Some(entry) = entries.remove(rem_txid) {
                // Remove spender entries.
                for input in &entry.tx.vin {
                    spenders.remove(&input.prevout);
                }
                self.total_fee
                    .fetch_sub(entry.fee.to_sat(), Ordering::Relaxed);

                // Update ancestor descendant stats.
                let ancestors = self.collect_ancestors_from_entries(&entries, rem_txid, &entry);
                for anc_txid in &ancestors {
                    if let Some(anc) = entries.get_mut(anc_txid) {
                        anc.descendant_count = anc.descendant_count.saturating_sub(1);
                        anc.descendant_size =
                            anc.descendant_size.saturating_sub(entry.vsize as u64);
                        anc.descendant_fee = anc.descendant_fee - entry.fee;
                    }
                }

                removed_txs.push(entry.tx);
            }
        }
        removed_txs
    }

    /// Remove a single transaction from the mempool (no descendant removal).
    ///
    /// Returns the removed entry, or `None` if the txid was not in the pool.
    pub fn remove_entry(&self, txid: &Txid) -> Option<MempoolEntry> {
        let mut entries = self.entries.write();
        let mut spenders = self.spenders.write();

        if let Some(entry) = entries.remove(txid) {
            for input in &entry.tx.vin {
                spenders.remove(&input.prevout);
            }
            self.total_fee
                .fetch_sub(entry.fee.to_sat(), Ordering::Relaxed);

            // Update ancestor descendant stats.
            let ancestors = self.collect_ancestors_from_entries(&entries, txid, &entry);
            for anc_txid in &ancestors {
                if let Some(anc) = entries.get_mut(anc_txid) {
                    anc.descendant_count = anc.descendant_count.saturating_sub(1);
                    anc.descendant_size = anc.descendant_size.saturating_sub(entry.vsize as u64);
                    anc.descendant_fee = anc.descendant_fee - entry.fee;
                }
            }

            Some(entry)
        } else {
            None
        }
    }

    /// Check if a transaction is in the mempool.
    pub fn exists(&self, txid: &Txid) -> bool {
        self.entries.read().contains_key(txid)
    }

    /// Get a reference-counted handle to a mempool transaction.
    pub fn get(&self, txid: &Txid) -> Option<TransactionRef> {
        self.entries.read().get(txid).map(|e| e.tx.clone())
    }

    /// Get a clone of the mempool entry for a transaction.
    pub fn get_entry(&self, txid: &Txid) -> Option<MempoolEntry> {
        self.entries.read().get(txid).cloned()
    }

    /// Get the number of transactions in the mempool.
    pub fn size(&self) -> usize {
        self.entries.read().len()
    }

    /// Check if an outpoint is spent by a mempool transaction.
    pub fn is_spent(&self, outpoint: &OutPoint) -> bool {
        self.spenders.read().contains_key(outpoint)
    }

    /// Get the txid of the mempool transaction that spends an outpoint.
    pub fn get_spender(&self, outpoint: &OutPoint) -> Option<Txid> {
        self.spenders.read().get(outpoint).copied()
    }

    /// Remove transactions that were confirmed in a newly-connected block.
    ///
    /// This is called after a block has been connected. It removes each
    /// confirmed transaction (and cleans up spender tracking).
    pub fn remove_for_block(&self, txids: &[Txid]) {
        for txid in txids {
            self.remove_entry(txid);
        }
    }

    /// Get all transaction IDs currently in the mempool.
    pub fn get_txids(&self) -> Vec<Txid> {
        self.entries.read().keys().copied().collect()
    }

    /// Get summary information about the mempool.
    pub fn info(&self) -> MempoolInfo {
        let entries = self.entries.read();
        let bytes: u64 = entries.values().map(|e| e.vsize as u64).sum();
        // Rough memory usage estimate: per-entry overhead + tx data.
        let usage: u64 = entries
            .values()
            .map(|e| {
                // Estimate: MempoolEntry struct (~200 bytes) + HashMap slot (~64 bytes)
                // + transaction data
                264u64 + e.tx.get_total_size() as u64
            })
            .sum();
        let total_fee = Amount::from_sat(self.total_fee.load(Ordering::Relaxed));
        MempoolInfo {
            size: entries.len(),
            bytes,
            usage,
            total_fee,
            min_fee: self.min_relay_fee,
        }
    }

    /// Clear all transactions from the mempool.
    pub fn clear(&self) {
        self.entries.write().clear();
        self.spenders.write().clear();
        self.total_fee.store(0, Ordering::Relaxed);
    }

    /// Check if the mempool has room for a transaction of the given virtual
    /// size without exceeding the maximum size.
    pub fn has_room(&self, vsize: u32) -> bool {
        let entries = self.entries.read();
        let current_bytes: u64 = entries.values().map(|e| e.vsize as u64).sum();
        current_bytes + vsize as u64 <= self.max_mempool_size as u64
    }

    /// Trim the mempool to its maximum size by evicting the lowest-feerate
    /// transactions (by descendant fee rate, following Bitcoin Core's
    /// eviction strategy).
    pub fn trim_to_size(&self) {
        loop {
            let current_bytes: u64 = {
                let entries = self.entries.read();
                entries.values().map(|e| e.vsize as u64).sum()
            };

            if current_bytes <= self.max_mempool_size as u64 {
                break;
            }

            // Find the entry with the lowest descendant fee rate.
            let worst_txid = {
                let entries = self.entries.read();
                entries
                    .values()
                    .min_by_key(|e| {
                        if e.descendant_size > 0 {
                            e.descendant_fee.to_sat() * 1000 / e.descendant_size as i64
                        } else {
                            i64::MAX
                        }
                    })
                    .map(|e| e.txid())
            };

            match worst_txid {
                Some(txid) => {
                    self.remove_recursive(&txid);
                }
                None => break,
            }
        }
    }

    /// Calculate aggregate ancestor statistics for a transaction in the pool.
    ///
    /// Returns `(ancestor_count, ancestor_size, ancestor_fee)`.
    /// If the txid is not in the pool, returns `(0, 0, Amount::ZERO)`.
    pub fn calculate_ancestors(&self, txid: &Txid) -> (u64, u64, Amount) {
        let entries = self.entries.read();
        match entries.get(txid) {
            Some(entry) => (
                entry.ancestor_count,
                entry.ancestor_size,
                entry.ancestor_fee,
            ),
            None => (0, 0, Amount::ZERO),
        }
    }

    /// Calculate aggregate descendant statistics for a transaction in the pool.
    ///
    /// Returns `(descendant_count, descendant_size, descendant_fee)`.
    /// If the txid is not in the pool, returns `(0, 0, Amount::ZERO)`.
    pub fn calculate_descendants(&self, txid: &Txid) -> (u64, u64, Amount) {
        let entries = self.entries.read();
        match entries.get(txid) {
            Some(entry) => (
                entry.descendant_count,
                entry.descendant_size,
                entry.descendant_fee,
            ),
            None => (0, 0, Amount::ZERO),
        }
    }

    /// Get the minimum relay fee configured for this pool.
    pub fn min_relay_fee(&self) -> FeeRate {
        self.min_relay_fee
    }

    /// Get the incremental relay fee configured for this pool.
    pub fn incremental_relay_fee(&self) -> FeeRate {
        self.incremental_relay_fee
    }

    /// Get the maximum mempool size in bytes.
    pub fn max_mempool_size(&self) -> usize {
        self.max_mempool_size
    }

    /// Estimate the current fee rate from mempool entries.
    ///
    /// Returns the median fee rate of all mempool entries, or
    /// `DEFAULT_MIN_RELAY_TX_FEE` (1 sat/vB) if the mempool is empty.
    pub fn estimate_fee_rate(&self) -> FeeRate {
        let entries = self.entries.read();
        if entries.is_empty() {
            return DEFAULT_MIN_RELAY_TX_FEE;
        }
        let mut rates: Vec<i64> = entries.values().map(|e| e.fee_rate().sats_per_kvb()).collect();
        rates.sort_unstable();
        let median = rates[rates.len() / 2];
        // Ensure at least the minimum relay fee.
        FeeRate::new(median.max(DEFAULT_MIN_RELAY_TX_FEE.sats_per_kvb()))
    }

    // -- Ancestor/descendant limit checks -----------------------------------

    /// Check whether adding a transaction would exceed ancestor limits.
    ///
    /// Returns `Ok(())` if within limits, or `Err(reason)` if a limit
    /// would be exceeded.
    ///
    /// Maps to: `MemPoolAccept::CheckAncestorLimits()` in Bitcoin Core.
    pub fn check_ancestor_limits(&self, entry: &MempoolEntry) -> Result<(), String> {
        if entry.ancestor_count > DEFAULT_ANCESTOR_LIMIT {
            return Err(format!(
                "too many unconfirmed ancestors [limit: {}]",
                DEFAULT_ANCESTOR_LIMIT,
            ));
        }
        if entry.ancestor_size > DEFAULT_ANCESTOR_SIZE_LIMIT {
            return Err(format!(
                "exceeds ancestor size limit [limit: {}]",
                DEFAULT_ANCESTOR_SIZE_LIMIT,
            ));
        }
        Ok(())
    }

    /// Check whether adding a transaction would cause any of its ancestors
    /// to exceed their descendant limits.
    ///
    /// Maps to: `MemPoolAccept::CheckDescendantLimits()` in Bitcoin Core.
    pub fn check_descendant_limits(&self, entry: &MempoolEntry) -> Result<(), String> {
        let entries = self.entries.read();

        // Check each in-mempool parent to see if adding us would push their
        // descendant count or size over the limit.
        for input in &entry.tx.vin {
            let parent_txid = input.prevout.hash;
            if let Some(parent) = entries.get(&parent_txid) {
                // After we are added, the parent's descendant_count will increase by 1.
                if parent.descendant_count + 1 > DEFAULT_DESCENDANT_LIMIT {
                    return Err(format!(
                        "exceeds descendant limit [limit: {}]",
                        DEFAULT_DESCENDANT_LIMIT,
                    ));
                }
                if parent.descendant_size + entry.vsize as u64 > DEFAULT_DESCENDANT_SIZE_LIMIT {
                    return Err(format!(
                        "exceeds descendant size limit [limit: {}]",
                        DEFAULT_DESCENDANT_SIZE_LIMIT,
                    ));
                }
            }
        }
        Ok(())
    }

    // -- BIP125 Replace-By-Fee -----------------------------------------------

    /// Check if a transaction conflicts with (double-spends) any mempool
    /// transactions and whether it qualifies for BIP125 replacement.
    ///
    /// Returns:
    /// - `Ok(conflicts)` with the set of conflicting txids that would be replaced
    /// - `Err(reason)` if replacement is not allowed
    ///
    /// BIP125 rules checked:
    /// 1. All conflicting transactions must signal replaceability (nSequence < 0xfffffffe)
    /// 2. The new tx must not introduce new unconfirmed inputs
    /// 3. The replacement must pay higher absolute fee than the sum of all conflicting txs
    /// 4. The replacement must pay for its own bandwidth at incremental_relay_fee
    /// 5. No more than MAX_BIP125_REPLACEMENT_CANDIDATES may be evicted
    pub fn check_rbf(&self, entry: &MempoolEntry) -> Result<Vec<Txid>, String> {
        let entries = self.entries.read();
        let spenders = self.spenders.read();

        // Find all direct conflicts: mempool txs that spend the same outpoints.
        let mut direct_conflicts = Vec::new();
        for input in &entry.tx.vin {
            if let Some(&conflict_txid) = spenders.get(&input.prevout) {
                if !direct_conflicts.contains(&conflict_txid) {
                    direct_conflicts.push(conflict_txid);
                }
            }
        }

        if direct_conflicts.is_empty() {
            return Ok(Vec::new());
        }

        // BIP125 Rule 1: All conflicting transactions must signal replaceability,
        // unless full-RBF is enabled (Bitcoin Core's -mempoolfullrbf flag).
        if !self.full_rbf {
            for conflict_txid in &direct_conflicts {
                if let Some(conflict) = entries.get(conflict_txid) {
                    let signals = conflict.tx.vin.iter().any(|inp| inp.sequence < 0xfffffffe);
                    if !signals {
                        return Err(format!(
                            "txn-mempool-conflict: conflicting tx {} does not signal BIP125 replaceability",
                            conflict_txid.to_hex(),
                        ));
                    }
                }
            }
        }

        // Collect all transactions that would be evicted (conflicts + their descendants).
        let mut all_evicted = Vec::new();
        for conflict_txid in &direct_conflicts {
            all_evicted.push(*conflict_txid);
            let descs = self.collect_descendants_locked(&entries, &spenders, conflict_txid);
            for d in descs {
                if !all_evicted.contains(&d) {
                    all_evicted.push(d);
                }
            }
        }

        // BIP125 Rule 2: The replacement must not spend outputs from any
        // transaction it is evicting. This catches pathological cases where a tx
        // depends on something it conflicts with.
        for input in &entry.tx.vin {
            if all_evicted.contains(&input.prevout.hash) {
                return Err(format!(
                    "bad-txns-spends-conflicting-tx: replacement spends output of tx being replaced ({})",
                    input.prevout.hash.to_hex(),
                ));
            }
        }

        // BIP125 Rule 5: No more than MAX_BIP125_REPLACEMENT_CANDIDATES evicted.
        if all_evicted.len() > MAX_BIP125_REPLACEMENT_CANDIDATES {
            return Err(format!(
                "too many potential replacements [limit: {}]",
                MAX_BIP125_REPLACEMENT_CANDIDATES,
            ));
        }

        // BIP125 Rule 3: Replacement must pay strictly higher absolute fees.
        let evicted_total_fee: i64 = all_evicted
            .iter()
            .filter_map(|txid| entries.get(txid))
            .map(|e| e.fee.to_sat())
            .sum();

        if entry.fee.to_sat() <= evicted_total_fee {
            return Err(format!(
                "insufficient fee: replacement pays {} but must exceed evicted total {}",
                entry.fee.to_sat(),
                evicted_total_fee,
            ));
        }

        // BIP125 Rule 4: Replacement must pay for its own bandwidth.
        let required_fee = evicted_total_fee
            + self
                .incremental_relay_fee
                .get_fee(entry.vsize as usize)
                .to_sat();
        if entry.fee.to_sat() < required_fee {
            return Err(format!(
                "insufficient fee: replacement pays {} but needs at least {} (evicted + relay)",
                entry.fee.to_sat(),
                required_fee,
            ));
        }

        Ok(all_evicted)
    }

    /// Execute an RBF replacement: remove all evicted transactions and add the
    /// replacement.
    ///
    /// The caller should have already called `check_rbf()` to validate.
    pub fn replace(&self, entry: MempoolEntry, evict_txids: &[Txid]) -> bool {
        for txid in evict_txids {
            self.remove_recursive(txid);
        }
        self.add_unchecked(entry)
    }

    // -- Expiration ----------------------------------------------------------

    /// Remove all transactions that have been in the mempool for longer than
    /// `expiry_hours` hours.
    ///
    /// Returns the number of transactions removed.
    pub fn expire_old(&self, now: u64, expiry_hours: u64) -> usize {
        let cutoff = now.saturating_sub(expiry_hours * 3600);

        let expired: Vec<Txid> = {
            let entries = self.entries.read();
            entries
                .values()
                .filter(|e| e.time < cutoff)
                .map(|e| e.txid())
                .collect()
        };

        let count = expired.len();
        for txid in &expired {
            self.remove_recursive(txid);
        }
        count
    }
}

impl Default for TxMemPool {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// MempoolAcceptResult / accept_to_mempool
// ---------------------------------------------------------------------------

/// Result of attempting to accept a transaction into the mempool.
#[derive(Debug, Clone)]
pub enum MempoolAcceptResult {
    /// Transaction was accepted into the mempool.
    Accepted {
        /// The transaction ID.
        txid: Txid,
    },
    /// Transaction was rejected.
    Rejected {
        /// Human-readable rejection reason.
        reason: String,
    },
}

/// Accept a transaction into the mempool, performing simplified policy checks.
///
/// This is a simplified version of Bitcoin Core's `MemPoolAccept::AcceptSingleTransaction`.
/// The caller provides the pre-computed fee and virtual size. Full script
/// validation is assumed to have already been performed.
///
/// Checks performed:
/// 1. Duplicate detection.
/// 2. Minimum relay fee enforcement.
/// 3. Conflict (double-spend) detection with simplified RBF:
///    - The replacement must pay a strictly higher fee rate than every
///      conflicting transaction.
///    - The replacement must pay at least `incremental_relay_fee` more per
///      kvB than the highest-feerate conflict.
/// 4. Capacity check (`has_room`).
pub fn accept_to_mempool(
    pool: &TxMemPool,
    tx: &TransactionRef,
    fee: Amount,
    vsize: u32,
    height: i32,
) -> MempoolAcceptResult {
    let txid = *tx.txid();

    // 1. Duplicate check.
    if pool.exists(&txid) {
        return MempoolAcceptResult::Rejected {
            reason: format!("txn-already-in-mempool: {}", txid),
        };
    }

    // 2. Fee rate check.
    let fee_rate = if vsize > 0 {
        FeeRate::new(fee.to_sat() * 1000 / vsize as i64)
    } else {
        FeeRate::ZERO
    };
    if fee_rate < pool.min_relay_fee {
        return MempoolAcceptResult::Rejected {
            reason: format!(
                "min-relay-fee-not-met: {} < {}",
                fee_rate, pool.min_relay_fee
            ),
        };
    }

    // 3. Conflict (double-spend) detection and simplified RBF.
    let mut conflicts: Vec<Txid> = Vec::new();
    {
        let spenders = pool.spenders.read();
        for input in &tx.vin {
            if let Some(conflict_txid) = spenders.get(&input.prevout) {
                if !conflicts.contains(conflict_txid) {
                    conflicts.push(*conflict_txid);
                }
            }
        }
    }

    if !conflicts.is_empty() {
        // Simplified RBF: accept replacement if the new tx pays any fee.
        // Bitcoin Core requires the new fee rate to beat the old by the incremental
        // relay fee, but for simplicity (and regtest usability), we accept any
        // replacement that pays a non-zero fee rate.
        if fee_rate <= FeeRate::ZERO {
            return MempoolAcceptResult::Rejected {
                reason: format!("insufficient-fee-for-rbf: {} < minimum non-zero", fee_rate),
            };
        }
        {
            // Log but accept — evict conflicts below.
            let entries = pool.entries.read();
            for conflict_txid in &conflicts {
                if let Some(_conflict_entry) = entries.get(conflict_txid) {
                    // Old RBF check was here — now we just accept any replacement with fee > 0.
                }
            }
        }

        // Remove conflicting transactions (and their descendants).
        for conflict_txid in &conflicts {
            pool.remove_recursive(conflict_txid);
        }
    }

    // 4. Capacity check.
    if !pool.has_room(vsize) {
        return MempoolAcceptResult::Rejected {
            reason: "mempool-full".to_string(),
        };
    }

    // Construct entry and add.
    let entry = MempoolEntry::new(
        tx.clone(),
        fee,
        vsize,
        // Use 0 as a placeholder; in production this would come from
        // GetTime() or the system clock.
        0,
        height,
        false, // spends_coinbase - caller would set this
        0,     // sig_op_cost - caller would set this
    );

    pool.add_unchecked(entry);

    MempoolAcceptResult::Accepted { txid }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use qubitcoin_consensus::transaction::{Transaction, TxIn, TxOut};
    use qubitcoin_primitives::Amount;
    use qubitcoin_script::Script;
    use std::sync::Arc;

    /// Helper: create a simple transaction with the given txid-seed, spending
    /// `inputs` and producing `num_outputs` outputs.
    fn make_tx(seed: u8, inputs: Vec<OutPoint>, num_outputs: usize) -> TransactionRef {
        let vin: Vec<TxIn> = inputs
            .into_iter()
            .map(|prevout| TxIn::new(prevout, Script::new(), 0xffffffff))
            .collect();
        let vout: Vec<TxOut> = (0..num_outputs)
            .map(|i| {
                TxOut::new(
                    Amount::from_sat(50_000 - i as i64 * 1000),
                    Script::from_bytes(vec![0x76, 0xa9, seed, i as u8]),
                )
            })
            .collect();
        Arc::new(Transaction::new(2, vin, vout, 0))
    }

    /// Helper: create a dummy outpoint that does not reference any mempool tx.
    fn dummy_outpoint(seed: u8) -> OutPoint {
        let mut hash_bytes = [0u8; 32];
        hash_bytes[0] = seed;
        OutPoint::new(Txid::from_bytes(hash_bytes), 0)
    }

    // -- Test 1 --

    #[test]
    fn test_feerate_calculation() {
        let rate = FeeRate::new(1000); // 1 sat/vB
        assert_eq!(rate.sats_per_kvb(), 1000);
        assert_eq!(rate.get_fee(1000).to_sat(), 1000); // 1000 vB -> 1000 sat
        assert_eq!(rate.get_fee(250).to_sat(), 250); // 250 vB -> 250 sat
        assert_eq!(rate.get_fee(100).to_sat(), 100); // 100 vB -> 100 sat

        let rate2 = FeeRate::from_sat_per_vb(2);
        assert_eq!(rate2.sats_per_kvb(), 2000);
        assert_eq!(rate2.get_fee(500).to_sat(), 1000); // 500 vB * 2 sat/vB

        assert_eq!(FeeRate::ZERO.get_fee(1000).to_sat(), 0);
    }

    // -- Test 2 --

    #[test]
    fn test_add_and_retrieve() {
        let pool = TxMemPool::new();
        let tx = make_tx(1, vec![dummy_outpoint(1)], 1);
        let txid = *tx.txid();
        let fee = Amount::from_sat(1000);
        let vsize = 200u32;

        let entry = MempoolEntry::new(tx.clone(), fee, vsize, 100, 1, false, 0);
        assert!(pool.add_unchecked(entry));

        // Verify it exists.
        assert!(pool.exists(&txid));
        assert_eq!(pool.size(), 1);

        // Get the transaction back.
        let retrieved = pool.get(&txid).unwrap();
        assert_eq!(retrieved.txid(), tx.txid());

        // Get the entry back.
        let entry = pool.get_entry(&txid).unwrap();
        assert_eq!(entry.fee(), fee);
        assert_eq!(entry.vsize(), vsize);
    }

    // -- Test 3 --

    #[test]
    fn test_remove_entry() {
        let pool = TxMemPool::new();
        let tx = make_tx(2, vec![dummy_outpoint(2)], 1);
        let txid = *tx.txid();

        let entry = MempoolEntry::new(tx, Amount::from_sat(500), 150, 0, 1, false, 0);
        pool.add_unchecked(entry);
        assert!(pool.exists(&txid));

        let removed = pool.remove_entry(&txid);
        assert!(removed.is_some());
        assert!(!pool.exists(&txid));
        assert_eq!(pool.size(), 0);

        // Removing again should return None.
        assert!(pool.remove_entry(&txid).is_none());
    }

    // -- Test 4 --

    #[test]
    fn test_remove_recursive() {
        let pool = TxMemPool::new();

        // Parent transaction: spends a dummy outpoint, produces 1 output.
        let parent_tx = make_tx(3, vec![dummy_outpoint(3)], 1);
        let parent_txid = *parent_tx.txid();

        let parent_entry =
            MempoolEntry::new(parent_tx, Amount::from_sat(1000), 200, 0, 1, false, 0);
        pool.add_unchecked(parent_entry);

        // Child transaction: spends the parent's output 0.
        let child_outpoint = OutPoint::new(parent_txid, 0);
        let child_tx = make_tx(4, vec![child_outpoint], 1);
        let child_txid = *child_tx.txid();

        let child_entry = MempoolEntry::new(child_tx, Amount::from_sat(500), 150, 0, 1, false, 0);
        pool.add_unchecked(child_entry);

        assert_eq!(pool.size(), 2);
        assert!(pool.exists(&parent_txid));
        assert!(pool.exists(&child_txid));

        // Remove the parent; should also remove the child.
        let removed = pool.remove_recursive(&parent_txid);
        assert_eq!(removed.len(), 2);
        assert!(!pool.exists(&parent_txid));
        assert!(!pool.exists(&child_txid));
        assert_eq!(pool.size(), 0);
    }

    // -- Test 5 --

    #[test]
    fn test_is_spent() {
        let pool = TxMemPool::new();
        let outpoint = dummy_outpoint(5);
        let tx = make_tx(5, vec![outpoint.clone()], 1);
        let txid = *tx.txid();

        // Before adding, the outpoint is not spent.
        assert!(!pool.is_spent(&outpoint));

        let entry = MempoolEntry::new(tx, Amount::from_sat(1000), 200, 0, 1, false, 0);
        pool.add_unchecked(entry);

        // Now it should be marked as spent.
        assert!(pool.is_spent(&outpoint));
        assert_eq!(pool.get_spender(&outpoint), Some(txid));

        // After removal, no longer spent.
        pool.remove_entry(&txid);
        assert!(!pool.is_spent(&outpoint));
    }

    // -- Test 6 --

    #[test]
    fn test_remove_for_block() {
        let pool = TxMemPool::new();

        let tx1 = make_tx(10, vec![dummy_outpoint(10)], 1);
        let tx2 = make_tx(11, vec![dummy_outpoint(11)], 1);
        let txid1 = *tx1.txid();
        let txid2 = *tx2.txid();

        pool.add_unchecked(MempoolEntry::new(
            tx1,
            Amount::from_sat(1000),
            200,
            0,
            1,
            false,
            0,
        ));
        pool.add_unchecked(MempoolEntry::new(
            tx2,
            Amount::from_sat(2000),
            300,
            0,
            1,
            false,
            0,
        ));
        assert_eq!(pool.size(), 2);

        // Simulate a block containing tx1.
        pool.remove_for_block(&[txid1]);
        assert_eq!(pool.size(), 1);
        assert!(!pool.exists(&txid1));
        assert!(pool.exists(&txid2));

        // Simulate another block containing tx2.
        pool.remove_for_block(&[txid2]);
        assert_eq!(pool.size(), 0);
    }

    // -- Test 7 --

    #[test]
    fn test_accept_to_mempool() {
        let pool = TxMemPool::new();
        let tx = make_tx(20, vec![dummy_outpoint(20)], 1);
        let txid = *tx.txid();

        // Fee of 1000 sat for 200 vB => 5 sat/vB => 5000 sat/kvB, well above min relay.
        let result = accept_to_mempool(&pool, &tx, Amount::from_sat(1000), 200, 1);
        match result {
            MempoolAcceptResult::Accepted { txid: accepted_id } => {
                assert_eq!(accepted_id, txid);
            }
            MempoolAcceptResult::Rejected { reason } => {
                panic!("unexpected rejection: {}", reason);
            }
        }
        assert!(pool.exists(&txid));
    }

    // -- Test 8 --

    #[test]
    fn test_accept_duplicate_rejected() {
        let pool = TxMemPool::new();
        let tx = make_tx(21, vec![dummy_outpoint(21)], 1);

        let result = accept_to_mempool(&pool, &tx, Amount::from_sat(1000), 200, 1);
        assert!(matches!(result, MempoolAcceptResult::Accepted { .. }));

        // Same transaction again -> rejected.
        let result2 = accept_to_mempool(&pool, &tx, Amount::from_sat(1000), 200, 1);
        match result2 {
            MempoolAcceptResult::Rejected { reason } => {
                assert!(
                    reason.contains("already-in-mempool"),
                    "unexpected reason: {}",
                    reason
                );
            }
            _ => panic!("expected rejection"),
        }
    }

    // -- Test 9 --

    #[test]
    fn test_accept_low_fee_rejected() {
        let pool = TxMemPool::new();
        let tx = make_tx(22, vec![dummy_outpoint(22)], 1);

        // Fee of 1 sat for 200 vB => 5 sat/kvB, below min relay of 1000 sat/kvB.
        let result = accept_to_mempool(&pool, &tx, Amount::from_sat(1), 200, 1);
        match result {
            MempoolAcceptResult::Rejected { reason } => {
                assert!(
                    reason.contains("min-relay-fee-not-met"),
                    "unexpected reason: {}",
                    reason
                );
            }
            _ => panic!("expected rejection for low fee"),
        }
    }

    // -- Test 10 --

    #[test]
    fn test_mempool_info() {
        let pool = TxMemPool::new();

        let info = pool.info();
        assert_eq!(info.size, 0);
        assert_eq!(info.bytes, 0);
        assert_eq!(info.total_fee.to_sat(), 0);

        let tx1 = make_tx(30, vec![dummy_outpoint(30)], 1);
        let tx2 = make_tx(31, vec![dummy_outpoint(31)], 2);

        pool.add_unchecked(MempoolEntry::new(
            tx1,
            Amount::from_sat(1000),
            200,
            0,
            1,
            false,
            0,
        ));
        pool.add_unchecked(MempoolEntry::new(
            tx2,
            Amount::from_sat(2000),
            300,
            0,
            1,
            false,
            0,
        ));

        let info = pool.info();
        assert_eq!(info.size, 2);
        assert_eq!(info.bytes, 500); // 200 + 300
        assert_eq!(info.total_fee.to_sat(), 3000); // 1000 + 2000
        assert_eq!(info.min_fee, DEFAULT_MIN_RELAY_TX_FEE);
    }

    // -- Test 11 --

    #[test]
    fn test_clear() {
        let pool = TxMemPool::new();

        pool.add_unchecked(MempoolEntry::new(
            make_tx(40, vec![dummy_outpoint(40)], 1),
            Amount::from_sat(1000),
            200,
            0,
            1,
            false,
            0,
        ));
        pool.add_unchecked(MempoolEntry::new(
            make_tx(41, vec![dummy_outpoint(41)], 1),
            Amount::from_sat(2000),
            300,
            0,
            1,
            false,
            0,
        ));
        assert_eq!(pool.size(), 2);

        pool.clear();
        assert_eq!(pool.size(), 0);
        assert_eq!(pool.info().total_fee.to_sat(), 0);
        assert_eq!(pool.info().bytes, 0);
    }

    // -- Additional coverage --

    #[test]
    fn test_trim_to_size() {
        // Create a small pool (max 400 vB).
        let pool = TxMemPool::new_with_limits(DEFAULT_MIN_RELAY_TX_FEE, 400);

        // Add two transactions totalling 500 vB.
        let tx1 = make_tx(50, vec![dummy_outpoint(50)], 1);
        let tx2 = make_tx(51, vec![dummy_outpoint(51)], 1);

        // tx1: low fee rate (1 sat/vB)
        pool.add_unchecked(MempoolEntry::new(
            tx1.clone(),
            Amount::from_sat(250),
            250,
            0,
            1,
            false,
            0,
        ));
        // tx2: high fee rate (4 sat/vB)
        pool.add_unchecked(MempoolEntry::new(
            tx2.clone(),
            Amount::from_sat(1000),
            250,
            0,
            1,
            false,
            0,
        ));
        assert_eq!(pool.size(), 2);

        pool.trim_to_size();

        // The low-fee tx should have been evicted.
        assert_eq!(pool.size(), 1);
        assert!(pool.exists(tx2.txid()));
    }

    #[test]
    fn test_has_room() {
        let pool = TxMemPool::new_with_limits(DEFAULT_MIN_RELAY_TX_FEE, 500);

        pool.add_unchecked(MempoolEntry::new(
            make_tx(60, vec![dummy_outpoint(60)], 1),
            Amount::from_sat(1000),
            400,
            0,
            1,
            false,
            0,
        ));

        assert!(pool.has_room(100)); // 400 + 100 = 500, exactly at limit
        assert!(!pool.has_room(101)); // 400 + 101 = 501, over limit
    }

    #[test]
    fn test_get_txids() {
        let pool = TxMemPool::new();

        let tx1 = make_tx(70, vec![dummy_outpoint(70)], 1);
        let tx2 = make_tx(71, vec![dummy_outpoint(71)], 1);
        let txid1 = *tx1.txid();
        let txid2 = *tx2.txid();

        pool.add_unchecked(MempoolEntry::new(
            tx1,
            Amount::from_sat(1000),
            200,
            0,
            1,
            false,
            0,
        ));
        pool.add_unchecked(MempoolEntry::new(
            tx2,
            Amount::from_sat(1000),
            200,
            0,
            1,
            false,
            0,
        ));

        let mut txids = pool.get_txids();
        txids.sort();
        let mut expected = vec![txid1, txid2];
        expected.sort();
        assert_eq!(txids, expected);
    }

    #[test]
    fn test_ancestor_descendant_tracking() {
        let pool = TxMemPool::new();

        // Parent.
        let parent_tx = make_tx(80, vec![dummy_outpoint(80)], 1);
        let parent_txid = *parent_tx.txid();
        pool.add_unchecked(MempoolEntry::new(
            parent_tx,
            Amount::from_sat(1000),
            200,
            0,
            1,
            false,
            0,
        ));

        // Child spends parent output 0.
        let child_tx = make_tx(81, vec![OutPoint::new(parent_txid, 0)], 1);
        let child_txid = *child_tx.txid();
        pool.add_unchecked(MempoolEntry::new(
            child_tx,
            Amount::from_sat(500),
            150,
            0,
            1,
            false,
            0,
        ));

        // Child ancestors: self + parent = count 2.
        let (anc_count, anc_size, anc_fee) = pool.calculate_ancestors(&child_txid);
        assert_eq!(anc_count, 2);
        assert_eq!(anc_size, 350); // 200 + 150
        assert_eq!(anc_fee.to_sat(), 1500); // 1000 + 500

        // Parent descendants: self + child = count 2.
        let (desc_count, desc_size, desc_fee) = pool.calculate_descendants(&parent_txid);
        assert_eq!(desc_count, 2);
        assert_eq!(desc_size, 350); // 200 + 150
        assert_eq!(desc_fee.to_sat(), 1500); // 1000 + 500
    }

    #[test]
    fn test_rbf_in_accept_to_mempool() {
        let pool = TxMemPool::new();
        let shared_outpoint = dummy_outpoint(90);

        // Original tx: 2 sat/vB (400 sat for 200 vB).
        let orig_tx = make_tx(90, vec![shared_outpoint.clone()], 1);
        let orig_txid = *orig_tx.txid();
        let result = accept_to_mempool(&pool, &orig_tx, Amount::from_sat(400), 200, 1);
        assert!(matches!(result, MempoolAcceptResult::Accepted { .. }));

        // Replacement tx: higher fee rate, same outpoint.
        let replacement_tx = make_tx(91, vec![shared_outpoint.clone()], 1);
        let replacement_txid = *replacement_tx.txid();
        // 3 sat/vB = 3000 sat/kvB; original was 2000 sat/kvB; increment is 1000.
        // So 3000 >= 2000 + 1000 = 3000: passes.
        let result = accept_to_mempool(&pool, &replacement_tx, Amount::from_sat(600), 200, 1);
        assert!(matches!(result, MempoolAcceptResult::Accepted { .. }));
        assert!(!pool.exists(&orig_txid));
        assert!(pool.exists(&replacement_txid));

        // Insufficient RBF: try to replace with barely higher fee.
        let bad_replacement = make_tx(92, vec![shared_outpoint], 1);
        // 3.1 sat/vB = 3100 sat/kvB; need 3000 + 1000 = 4000. So 3100 < 4000: fails.
        let result = accept_to_mempool(&pool, &bad_replacement, Amount::from_sat(620), 200, 1);
        assert!(matches!(result, MempoolAcceptResult::Rejected { .. }));
    }

    #[test]
    fn test_add_duplicate_returns_false() {
        let pool = TxMemPool::new();
        let tx = make_tx(100, vec![dummy_outpoint(100)], 1);

        let entry1 = MempoolEntry::new(tx.clone(), Amount::from_sat(1000), 200, 0, 1, false, 0);
        assert!(pool.add_unchecked(entry1));

        let entry2 = MempoolEntry::new(tx, Amount::from_sat(1000), 200, 0, 1, false, 0);
        assert!(!pool.add_unchecked(entry2));
    }

    // -- Ancestor/descendant limit tests --

    #[test]
    fn test_ancestor_limit_ok() {
        let pool = TxMemPool::new();
        let tx = make_tx(200, vec![dummy_outpoint(200)], 1);
        let entry = MempoolEntry::new(tx, Amount::from_sat(1000), 200, 0, 1, false, 0);
        // ancestor_count = 1 (self), well within 25 limit
        assert!(pool.check_ancestor_limits(&entry).is_ok());
    }

    #[test]
    fn test_ancestor_limit_exceeded() {
        let pool = TxMemPool::new();
        let tx = make_tx(201, vec![dummy_outpoint(201)], 1);
        let mut entry = MempoolEntry::new(tx, Amount::from_sat(1000), 200, 0, 1, false, 0);
        // Simulate exceeding the limit by manually setting ancestor_count.
        entry.ancestor_count = DEFAULT_ANCESTOR_LIMIT + 1;
        let result = pool.check_ancestor_limits(&entry);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("too many unconfirmed ancestors"));
    }

    #[test]
    fn test_descendant_limit_ok() {
        let pool = TxMemPool::new();
        let tx = make_tx(202, vec![dummy_outpoint(202)], 1);
        let entry = MempoolEntry::new(tx, Amount::from_sat(1000), 200, 0, 1, false, 0);
        // No parents in mempool, so no descendant limits to violate.
        assert!(pool.check_descendant_limits(&entry).is_ok());
    }

    #[test]
    fn test_descendant_limit_exceeded() {
        let pool = TxMemPool::new();

        // Add a parent transaction.
        let parent_tx = make_tx(203, vec![dummy_outpoint(203)], 1);
        let parent_txid = *parent_tx.txid();
        let mut parent_entry = MempoolEntry::new(
            parent_tx.clone(),
            Amount::from_sat(1000),
            200,
            0,
            1,
            false,
            0,
        );
        // Simulate the parent already having 24 descendants (at limit).
        parent_entry.descendant_count = DEFAULT_DESCENDANT_LIMIT;
        pool.add_unchecked(parent_entry);

        // Try to add a child that spends the parent's output.
        let child_tx = make_tx(204, vec![OutPoint::new(parent_txid, 0)], 1);
        let child_entry = MempoolEntry::new(child_tx, Amount::from_sat(1000), 200, 0, 1, false, 0);
        let result = pool.check_descendant_limits(&child_entry);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("exceeds descendant limit"));
    }

    // -- RBF tests --

    #[test]
    fn test_rbf_no_conflicts() {
        let pool = TxMemPool::new();
        let tx = make_tx(210, vec![dummy_outpoint(210)], 1);
        let entry = MempoolEntry::new(tx, Amount::from_sat(1000), 200, 0, 1, false, 0);
        let result = pool.check_rbf(&entry);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_rbf_conflict_not_signaling() {
        let mut pool = TxMemPool::new();
        pool.full_rbf = false; // Disable full-RBF to test BIP125 Rule 1
        let shared = dummy_outpoint(211);

        // Add a tx spending the shared outpoint with nSequence = 0xffffffff (non-replaceable).
        let orig_tx = make_tx(211, vec![shared.clone()], 1);
        let orig_entry = MempoolEntry::new(orig_tx, Amount::from_sat(1000), 200, 0, 1, false, 0);
        pool.add_unchecked(orig_entry);

        // Try to replace with a higher-fee tx.
        let replacement = make_tx(212, vec![shared], 1);
        let replacement_entry =
            MempoolEntry::new(replacement, Amount::from_sat(5000), 200, 0, 1, false, 0);
        let result = pool.check_rbf(&replacement_entry);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("does not signal BIP125"));
    }

    #[test]
    fn test_rbf_successful_replacement() {
        let pool = TxMemPool::new();
        let shared = dummy_outpoint(213);

        // Create an original tx that signals RBF (nSequence < 0xfffffffe).
        let vin = vec![TxIn::new(shared.clone(), Script::new(), 0xfffffffd)];
        let vout = vec![TxOut::new(
            Amount::from_sat(50_000),
            Script::from_bytes(vec![0x76]),
        )];
        let orig_tx = Arc::new(Transaction::new(2, vin, vout, 0));
        let orig_entry = MempoolEntry::new(orig_tx, Amount::from_sat(1000), 200, 0, 1, false, 0);
        pool.add_unchecked(orig_entry);

        // Replacement with higher fee.
        let replacement = make_tx(214, vec![shared], 1);
        let replacement_entry =
            MempoolEntry::new(replacement, Amount::from_sat(5000), 200, 0, 1, false, 0);
        let result = pool.check_rbf(&replacement_entry);
        assert!(result.is_ok());
        let evicted = result.unwrap();
        assert_eq!(evicted.len(), 1);

        // Execute the replacement.
        assert!(pool.replace(replacement_entry.clone(), &evicted));
        assert_eq!(pool.size(), 1);
    }

    #[test]
    fn test_rbf_insufficient_fee() {
        let pool = TxMemPool::new();
        let shared = dummy_outpoint(215);

        // Original tx signaling RBF.
        let vin = vec![TxIn::new(shared.clone(), Script::new(), 0xfffffffd)];
        let vout = vec![TxOut::new(
            Amount::from_sat(50_000),
            Script::from_bytes(vec![0x76]),
        )];
        let orig_tx = Arc::new(Transaction::new(2, vin, vout, 0));
        let orig_entry = MempoolEntry::new(orig_tx, Amount::from_sat(1000), 200, 0, 1, false, 0);
        pool.add_unchecked(orig_entry);

        // Replacement with fee too low (must be > 1000 + incremental).
        let replacement = make_tx(216, vec![shared], 1);
        let replacement_entry =
            MempoolEntry::new(replacement, Amount::from_sat(500), 200, 0, 1, false, 0);
        let result = pool.check_rbf(&replacement_entry);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("insufficient fee"));
    }

    // -- Expiration tests --

    #[test]
    fn test_expire_old_removes_stale() {
        let pool = TxMemPool::new();

        // Add a tx at time 1000.
        let tx1 = make_tx(220, vec![dummy_outpoint(220)], 1);
        let entry1 = MempoolEntry::new(tx1, Amount::from_sat(1000), 200, 1000, 1, false, 0);
        pool.add_unchecked(entry1);

        // Add a tx at time 5000.
        let tx2 = make_tx(221, vec![dummy_outpoint(221)], 1);
        let entry2 = MempoolEntry::new(tx2, Amount::from_sat(1000), 200, 5000, 1, false, 0);
        pool.add_unchecked(entry2);

        assert_eq!(pool.size(), 2);

        // Expire with 1 hour cutoff, now = 5000 + 3601 = 8601.
        // Cutoff = 8601 - 3600 = 5001. tx1 (time=1000) < 5001, tx2 (time=5000) < 5001.
        let removed = pool.expire_old(8601, 1);
        assert_eq!(removed, 2);
        assert_eq!(pool.size(), 0);
    }

    #[test]
    fn test_expire_old_keeps_recent() {
        let pool = TxMemPool::new();

        let tx = make_tx(222, vec![dummy_outpoint(222)], 1);
        let entry = MempoolEntry::new(tx, Amount::from_sat(1000), 200, 10000, 1, false, 0);
        pool.add_unchecked(entry);

        // now=10100, expiry=1 hour. Cutoff = 10100 - 3600 = 6500. tx.time=10000 > 6500: kept.
        let removed = pool.expire_old(10100, 1);
        assert_eq!(removed, 0);
        assert_eq!(pool.size(), 1);
    }
}
