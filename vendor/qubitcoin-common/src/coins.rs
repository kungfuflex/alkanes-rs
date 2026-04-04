//! UTXO coin types and cache hierarchy.
//!
//! Maps to: `src/coins.h` and `src/coins.cpp` in Bitcoin Core.
//!
//! This module provides:
//! - `Coin`: A UTXO entry containing the output, height, and coinbase flag.
//! - `CoinsCacheEntry`: A coin stored in a cache level, with DIRTY/FRESH flags.
//! - `CoinsView`: Abstract trait for reading the UTXO set.
//! - `CoinsViewCache`: In-memory cache over a `CoinsView` backend.
//! - `CoinsViewDB`: Database-backed `CoinsView` implementation.
//! - Amount compression/decompression matching Bitcoin Core's `CompressAmount`/`DecompressAmount`.
//! - Script compression/decompression matching Bitcoin Core's `ScriptCompression`.

use qubitcoin_consensus::{OutPoint, Transaction, TxOut};
use qubitcoin_primitives::{Amount, BlockHash};
use qubitcoin_script::Script;
use qubitcoin_serialize::{read_varint, write_varint, Decodable, Encodable, Error as SerError};
use qubitcoin_storage::{Database, DbWrapper};

use parking_lot::RwLock;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicU64, Ordering};

// ---------------------------------------------------------------------------
// Amount compression (port of Bitcoin Core's compressor.cpp)
// ---------------------------------------------------------------------------

/// Compress a satoshi amount into a smaller representation.
///
/// Port of Bitcoin Core's `CompressAmount`. The algorithm:
/// - If amount is 0, output 0.
/// - Divide by the largest power of 10 possible (exponent `e`, max 9).
/// - If `e < 9`, store the last non-zero digit `d` and remaining quotient `n`:
///   output `1 + 10*(9*n + d - 1) + e`.
/// - If `e == 9`, output `1 + 10*(n - 1) + 9`.
pub fn compress_amount(n: u64) -> u64 {
    if n == 0 {
        return 0;
    }
    let mut n = n;
    let mut e: u32 = 0;
    while (n % 10) == 0 && e < 9 {
        n /= 10;
        e += 1;
    }
    if e < 9 {
        let d = n % 10;
        debug_assert!(d >= 1 && d <= 9);
        n /= 10;
        1 + (n * 9 + d - 1) * 10 + e as u64
    } else {
        1 + (n - 1) * 10 + 9
    }
}

/// Decompress a compressed amount back to satoshis.
///
/// Port of Bitcoin Core's `DecompressAmount`. Inverse of [`compress_amount`].
pub fn decompress_amount(x: u64) -> u64 {
    if x == 0 {
        return 0;
    }
    let x = x - 1;
    let e = (x % 10) as u32;
    let x = x / 10;
    let mut n: u64 = if e < 9 {
        let d = (x % 9) + 1;
        let x = x / 9;
        x * 10 + d
    } else {
        x + 1
    };
    for _ in 0..e {
        n *= 10;
    }
    n
}

// ---------------------------------------------------------------------------
// Script compression (port of Bitcoin Core's ScriptCompression)
// ---------------------------------------------------------------------------

/// Number of special compressed script types.
/// Types 0-5 are specially encoded; scripts beyond that are stored verbatim
/// with their size offset by this constant.
const N_SPECIAL_SCRIPTS: u32 = 6;

/// Attempt to compress a script into a special short form.
///
/// Returns `Some(compressed_bytes)` for recognized patterns:
/// - Type 0x00: P2PKH (25 bytes -> 21 bytes)
/// - Type 0x01: P2SH  (23 bytes -> 21 bytes)
/// - Type 0x02/0x03: P2PK with compressed pubkey (35 bytes -> 33 bytes)
/// - Type 0x04/0x05: P2PK with uncompressed pubkey (67 bytes -> 33 bytes)
///
/// Returns `None` if the script does not match any special pattern.
fn compress_script(script: &Script) -> Option<Vec<u8>> {
    let data = script.as_bytes();

    // P2PKH: OP_DUP OP_HASH160 <20 bytes> OP_EQUALVERIFY OP_CHECKSIG
    if data.len() == 25
        && data[0] == 0x76  // OP_DUP
        && data[1] == 0xa9  // OP_HASH160
        && data[2] == 20
        && data[23] == 0x88 // OP_EQUALVERIFY
        && data[24] == 0xac
    // OP_CHECKSIG
    {
        let mut out = vec![0u8; 21];
        out[0] = 0x00;
        out[1..21].copy_from_slice(&data[3..23]);
        return Some(out);
    }

    // P2SH: OP_HASH160 <20 bytes> OP_EQUAL
    if data.len() == 23
        && data[0] == 0xa9  // OP_HASH160
        && data[1] == 20
        && data[22] == 0x87
    // OP_EQUAL
    {
        let mut out = vec![0u8; 21];
        out[0] = 0x01;
        out[1..21].copy_from_slice(&data[2..22]);
        return Some(out);
    }

    // P2PK with compressed pubkey: <33 bytes pubkey> OP_CHECKSIG
    if data.len() == 35
        && data[0] == 33
        && data[34] == 0xac // OP_CHECKSIG
        && (data[1] == 0x02 || data[1] == 0x03)
    {
        let mut out = vec![0u8; 33];
        out[0] = data[1]; // 0x02 or 0x03
        out[1..33].copy_from_slice(&data[2..34]);
        return Some(out);
    }

    // P2PK with uncompressed pubkey: <65 bytes pubkey> OP_CHECKSIG
    if data.len() == 67
        && data[0] == 65
        && data[66] == 0xac // OP_CHECKSIG
        && data[1] == 0x04
    {
        let mut out = vec![0u8; 33];
        out[0] = 0x04 | (data[65] & 0x01);
        out[1..33].copy_from_slice(&data[2..34]);
        return Some(out);
    }

    None
}

/// Get the data size for a special compressed script type.
fn get_special_script_size(n_size: u32) -> usize {
    match n_size {
        0 | 1 => 20,
        2 | 3 | 4 | 5 => 32,
        _ => 0,
    }
}

/// Decompress a special script from its type tag and compressed data.
///
/// Returns the full script bytes for types 0-3, or `None` for types 4-5
/// (which require EC point decompression, simplified here to return a
/// placeholder compressed-pubkey P2PK script).
fn decompress_script(n_size: u32, data: &[u8]) -> Option<Script> {
    match n_size {
        // P2PKH
        0x00 => {
            let mut script = vec![0u8; 25];
            script[0] = 0x76; // OP_DUP
            script[1] = 0xa9; // OP_HASH160
            script[2] = 20;
            script[3..23].copy_from_slice(&data[..20]);
            script[23] = 0x88; // OP_EQUALVERIFY
            script[24] = 0xac; // OP_CHECKSIG
            Some(Script::from_bytes(script))
        }
        // P2SH
        0x01 => {
            let mut script = vec![0u8; 23];
            script[0] = 0xa9; // OP_HASH160
            script[1] = 20;
            script[2..22].copy_from_slice(&data[..20]);
            script[22] = 0x87; // OP_EQUAL
            Some(Script::from_bytes(script))
        }
        // Compressed P2PK (0x02 or 0x03 prefix)
        0x02 | 0x03 => {
            let mut script = vec![0u8; 35];
            script[0] = 33;
            script[1] = n_size as u8;
            script[2..34].copy_from_slice(&data[..32]);
            script[34] = 0xac; // OP_CHECKSIG
            Some(Script::from_bytes(script))
        }
        // Uncompressed P2PK (0x04/0x05 -> needs EC point decompression)
        // Reconstruct the compressed key (parity byte + 32 bytes x-coord),
        // then use secp256k1 to decompress to a full 65-byte uncompressed
        // public key, matching Bitcoin Core's `CPubKey::Decompress()`.
        0x04 | 0x05 => {
            let mut compressed = [0u8; 33];
            compressed[0] = (n_size - 2) as u8; // 0x02 or 0x03
            compressed[1..33].copy_from_slice(&data[..32]);
            let pubkey = match secp256k1::PublicKey::from_slice(&compressed) {
                Ok(pk) => pk,
                Err(_) => return None, // Invalid point, same as Bitcoin Core returning false
            };
            let uncompressed = pubkey.serialize_uncompressed();
            assert!(uncompressed.len() == 65);
            let mut script = vec![0u8; 67];
            script[0] = 65;
            script[1..66].copy_from_slice(&uncompressed);
            script[66] = 0xac; // OP_CHECKSIG
            Some(Script::from_bytes(script))
        }
        _ => None,
    }
}

/// Write a compressed script to a writer.
///
/// Port of Bitcoin Core's `ScriptCompression::Ser`. Special script patterns
/// are encoded compactly; other scripts are prefixed with a VarInt-encoded
/// size (offset by `N_SPECIAL_SCRIPTS`).
fn write_compressed_script<W: Write>(w: &mut W, script: &Script) -> Result<usize, SerError> {
    if let Some(compressed) = compress_script(script) {
        w.write_all(&compressed)?;
        Ok(compressed.len())
    } else {
        let n_size = script.len() as u32 + N_SPECIAL_SCRIPTS;
        let mut size = write_varint(w, n_size as u64)?;
        w.write_all(script.as_bytes())?;
        size += script.len();
        Ok(size)
    }
}

/// Read a compressed script from a reader.
///
/// Port of Bitcoin Core's `ScriptCompression::Unser`.
fn read_compressed_script<R: Read>(r: &mut R) -> Result<Script, SerError> {
    let n_size = read_varint(r)? as u32;
    if n_size < N_SPECIAL_SCRIPTS {
        let special_size = get_special_script_size(n_size);
        let mut data = vec![0u8; special_size];
        r.read_exact(&mut data)?;
        match decompress_script(n_size, &data) {
            Some(script) => Ok(script),
            None => {
                // Decompression failed (e.g., invalid EC point for type 4/5);
                // return an OP_RETURN script as Bitcoin Core does for invalid data.
                Ok(Script::from_bytes(vec![0x6a])) // OP_RETURN
            }
        }
    } else {
        let script_len = n_size - N_SPECIAL_SCRIPTS;
        if script_len > 10_000 {
            // Overly long script -- skip the bytes and return OP_RETURN.
            let mut discard = vec![0u8; script_len as usize];
            r.read_exact(&mut discard)?;
            Ok(Script::from_bytes(vec![0x6a])) // OP_RETURN
        } else {
            let mut data = vec![0u8; script_len as usize];
            r.read_exact(&mut data)?;
            Ok(Script::from_bytes(data))
        }
    }
}

// ---------------------------------------------------------------------------
// Coin
// ---------------------------------------------------------------------------

/// A UTXO entry.
///
/// Port of Bitcoin Core's `Coin` (`CCoin`). Represents a single unspent
/// transaction output along with metadata about the transaction that created it.
///
/// Serialized format (matching Bitcoin Core):
/// - `VARINT((height << 1) | coinbase_flag)`
/// - Compressed amount via `compress_amount` encoded as VarInt
/// - Compressed script (see `write_compressed_script`)
#[derive(Clone, Debug)]
pub struct Coin {
    /// The unspent transaction output.
    pub tx_out: TxOut,
    /// Block height at which the containing transaction was included.
    pub height: u32,
    /// Whether the containing transaction was a coinbase.
    pub coinbase: bool,
}

impl Coin {
    /// Construct a new Coin.
    pub fn new(tx_out: TxOut, height: u32, coinbase: bool) -> Self {
        Coin {
            tx_out,
            height,
            coinbase,
        }
    }

    /// Construct an empty (spent) coin.
    pub fn empty() -> Self {
        Coin {
            tx_out: TxOut::null(),
            height: 0,
            coinbase: false,
        }
    }

    /// Check whether this coin has been spent (or never existed).
    ///
    /// A spent coin has a null TxOut (value == -1).
    pub fn is_spent(&self) -> bool {
        self.tx_out.is_null()
    }

    /// Clear the coin, marking it as spent.
    pub fn clear(&mut self) {
        self.tx_out = TxOut::null();
        self.coinbase = false;
        self.height = 0;
    }

    /// Estimate dynamic memory usage of this coin entry in the cache.
    ///
    /// Includes an approximation of the fixed per-entry overhead:
    /// OutPoint key (36 bytes) + CoinsCacheEntry struct (~40 bytes) +
    /// HashMap bucket overhead (~64 bytes) + heap-allocated script bytes.
    pub fn dynamic_memory_usage(&self) -> usize {
        // 140 bytes fixed overhead per HashMap<OutPoint, CoinsCacheEntry> entry
        140 + self.tx_out.script_pubkey.len()
    }
}

impl Default for Coin {
    fn default() -> Self {
        Self::empty()
    }
}

impl PartialEq for Coin {
    fn eq(&self, other: &Self) -> bool {
        self.tx_out == other.tx_out
            && self.height == other.height
            && self.coinbase == other.coinbase
    }
}

impl Eq for Coin {}

impl Encodable for Coin {
    /// Serialize a Coin in Bitcoin Core's compressed format.
    ///
    /// Format:
    /// 1. VarInt of `(height << 1) | coinbase`
    /// 2. VarInt of compressed amount
    /// 3. Compressed script
    fn encode<W: Write>(&self, w: &mut W) -> Result<usize, SerError> {
        assert!(!self.is_spent(), "Cannot serialize a spent coin");
        let code: u32 = (self.height << 1) | (self.coinbase as u32);
        let mut size = write_varint(w, code as u64)?;

        // Compress the amount
        let compressed_amount = compress_amount(self.tx_out.value.to_sat() as u64);
        size += write_varint(w, compressed_amount)?;

        // Compress the script
        size += write_compressed_script(w, &self.tx_out.script_pubkey)?;

        Ok(size)
    }
}

impl Decodable for Coin {
    /// Deserialize a Coin from Bitcoin Core's compressed format.
    fn decode<R: Read>(r: &mut R) -> Result<Self, SerError> {
        let code = read_varint(r)? as u32;
        let height = code >> 1;
        let coinbase = (code & 1) != 0;

        // Decompress the amount
        let compressed_amount = read_varint(r)?;
        let amount = decompress_amount(compressed_amount);

        // Decompress the script
        let script = read_compressed_script(r)?;

        Ok(Coin {
            tx_out: TxOut::new(Amount::from_sat(amount as i64), script),
            height,
            coinbase,
        })
    }
}

// ---------------------------------------------------------------------------
// CoinsCacheFlags and CoinsCacheEntry
// ---------------------------------------------------------------------------

/// Flags for a cache entry, indicating its state relative to the parent view.
///
/// These flags control the flush behavior of the coins cache hierarchy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CoinsCacheFlags(u8);

impl CoinsCacheFlags {
    /// No flags set.
    pub const NONE: CoinsCacheFlags = CoinsCacheFlags(0);

    /// The entry is potentially different from the version in the parent cache.
    ///
    /// Failure to mark a coin as DIRTY when it differs from the parent will
    /// cause a consensus failure, since the modified state will not be written
    /// to the parent during flush.
    pub const DIRTY: CoinsCacheFlags = CoinsCacheFlags(0x01);

    /// The parent cache does not have this coin, or it is spent in the parent.
    ///
    /// If a FRESH coin is later spent, it can be deleted entirely without ever
    /// needing to be flushed to the parent. This is a performance optimization.
    /// Marking a coin as FRESH when it exists unspent in the parent will cause
    /// a consensus failure.
    pub const FRESH: CoinsCacheFlags = CoinsCacheFlags(0x02);

    /// Both DIRTY and FRESH flags set (common for newly created UTXOs).
    pub const DIRTY_FRESH: CoinsCacheFlags = CoinsCacheFlags(0x01 | 0x02);

    /// Check if the DIRTY flag is set.
    pub fn is_dirty(self) -> bool {
        self.0 & Self::DIRTY.0 != 0
    }

    /// Check if the FRESH flag is set.
    pub fn is_fresh(self) -> bool {
        self.0 & Self::FRESH.0 != 0
    }

    /// Check if any flags are set.
    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Return the raw flag bits.
    pub fn bits(self) -> u8 {
        self.0
    }

    /// Create a `CoinsCacheFlags` from raw bits.
    pub fn from_bits(bits: u8) -> Self {
        CoinsCacheFlags(bits)
    }
}

impl std::ops::BitOr for CoinsCacheFlags {
    type Output = CoinsCacheFlags;
    fn bitor(self, rhs: Self) -> Self::Output {
        CoinsCacheFlags(self.0 | rhs.0)
    }
}

impl std::ops::BitOrAssign for CoinsCacheFlags {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl std::ops::BitAnd for CoinsCacheFlags {
    type Output = CoinsCacheFlags;
    fn bitand(self, rhs: Self) -> Self::Output {
        CoinsCacheFlags(self.0 & rhs.0)
    }
}

/// A coin stored in a cache level, along with flags indicating its relationship
/// to the parent cache level.
///
/// Port of Bitcoin Core's `CCoinsCacheEntry`.
#[derive(Clone, Debug)]
pub struct CoinsCacheEntry {
    /// The cached coin data.
    pub coin: Coin,
    /// Flags describing the entry's state relative to the parent view.
    pub flags: CoinsCacheFlags,
}

impl CoinsCacheEntry {
    /// Create a new cache entry with the given coin and flags.
    pub fn new(coin: Coin, flags: CoinsCacheFlags) -> Self {
        CoinsCacheEntry { coin, flags }
    }

    /// Create a new cache entry with no flags.
    pub fn clean(coin: Coin) -> Self {
        CoinsCacheEntry {
            coin,
            flags: CoinsCacheFlags::NONE,
        }
    }
}

// ---------------------------------------------------------------------------
// CoinsView trait
// ---------------------------------------------------------------------------

/// Abstract view on the open UTXO dataset.
///
/// Port of Bitcoin Core's `CCoinsView`. This is the base trait for all
/// views into the UTXO set, whether backed by a database, an in-memory
/// cache, or a chain of caches.
pub trait CoinsView: Send + Sync {
    /// Retrieve the coin (unspent transaction output) for a given outpoint.
    ///
    /// Returns `None` if the coin does not exist or has been spent.
    fn get_coin(&self, outpoint: &OutPoint) -> Option<Coin>;

    /// Check whether a given outpoint is unspent.
    ///
    /// Default implementation delegates to [`get_coin`](CoinsView::get_coin).
    fn have_coin(&self, outpoint: &OutPoint) -> bool {
        self.get_coin(outpoint).is_some()
    }

    /// Retrieve the block hash whose state this view currently represents.
    fn get_best_block(&self) -> BlockHash;

    /// Batch-fetch multiple coins in a single operation.
    ///
    /// Default implementation falls back to individual `get_coin` calls.
    /// Database-backed views override this with a true multi-get.
    fn get_coins(&self, outpoints: &[OutPoint]) -> Vec<Option<Coin>> {
        outpoints.iter().map(|op| self.get_coin(op)).collect()
    }

    /// Estimate the size of this view's backing store in bytes.
    ///
    /// Returns 0 if not implemented or not applicable.
    fn estimate_size(&self) -> u64 {
        0
    }
}

// ---------------------------------------------------------------------------
// EmptyCoinsView (for testing / as a base)
// ---------------------------------------------------------------------------

/// An empty coins view that contains no coins.
///
/// Useful as the base of a cache hierarchy in tests or for an initial state.
pub struct EmptyCoinsView;

impl CoinsView for EmptyCoinsView {
    fn get_coin(&self, _outpoint: &OutPoint) -> Option<Coin> {
        None
    }

    fn get_best_block(&self) -> BlockHash {
        BlockHash::ZERO
    }
}

// ---------------------------------------------------------------------------
// CoinsViewCache
// ---------------------------------------------------------------------------

/// In-memory cache layered on top of a [`CoinsView`] backend.
///
/// Port of Bitcoin Core's `CCoinsViewCache`. This provides efficient read/write
/// access to the UTXO set by caching entries in a `HashMap` and only flushing
/// dirty entries to the base view on demand.
///
/// Thread safety: all mutable state is protected by `RwLock` or atomics,
/// making this safe to share across threads.
pub struct CoinsViewCache {
    /// The backing view that this cache sits on top of.
    base: Box<dyn CoinsView>,
    /// The in-memory cache of coin entries.
    cache: RwLock<HashMap<OutPoint, CoinsCacheEntry>>,
    /// The block hash representing the current tip of this view.
    best_block: RwLock<BlockHash>,
    /// Approximate dynamic memory usage tracked by the cache.
    usage: AtomicU64,
}

impl CoinsViewCache {
    /// Create a new cache layered on top of the given base view.
    pub fn new(base: Box<dyn CoinsView>) -> Self {
        let best_block = base.get_best_block();
        CoinsViewCache {
            base,
            cache: RwLock::new(HashMap::new()),
            best_block: RwLock::new(best_block),
            usage: AtomicU64::new(0),
        }
    }

    /// Set the best block hash for this cache level.
    pub fn set_best_block(&self, hash: BlockHash) {
        *self.best_block.write() = hash;
    }

    /// Get the number of entries in the cache.
    pub fn cache_size(&self) -> usize {
        self.cache.read().len()
    }

    /// Get the approximate dynamic memory usage of cached coins.
    pub fn dynamic_memory_usage(&self) -> u64 {
        self.usage.load(Ordering::Relaxed)
    }

    /// Batch-prefetch coins into the cache.
    ///
    /// Checks the cache for each outpoint; any misses are fetched from the
    /// base view in a single `get_coins` call (which maps to RocksDB
    /// `multi_get` in production) and inserted into the cache.
    pub fn prefetch_coins(&self, outpoints: &[OutPoint]) {
        // Determine which outpoints are not yet cached.
        let miss_outpoints: Vec<OutPoint> = {
            let cache = self.cache.read();
            outpoints
                .iter()
                .filter(|op| !cache.contains_key(op))
                .cloned()
                .collect()
        };

        if miss_outpoints.is_empty() {
            return;
        }

        // Batch-fetch from the base view (multi_get in RocksDB).
        let results = self.base.get_coins(&miss_outpoints);

        // Insert fetched coins into the cache with a single write-lock.
        let mut cache = self.cache.write();
        let mut added_usage: u64 = 0;
        for (outpoint, maybe_coin) in miss_outpoints.into_iter().zip(results) {
            if cache.contains_key(&outpoint) {
                continue; // Another thread inserted it.
            }
            if let Some(coin) = maybe_coin {
                if !coin.is_spent() {
                    added_usage += coin.dynamic_memory_usage() as u64;
                    cache.insert(outpoint, CoinsCacheEntry::clean(coin));
                }
            }
        }
        drop(cache);
        if added_usage > 0 {
            self.usage.fetch_add(added_usage, Ordering::Relaxed);
        }
    }

    /// Look up a coin, first in the cache, then in the base view.
    ///
    /// If found in the base view, the coin is cached (without DIRTY/FRESH flags)
    /// for subsequent lookups.
    ///
    /// Returns a clone of the coin if found and unspent, or `None`.
    pub fn fetch_coin(&self, outpoint: &OutPoint) -> Option<Coin> {
        // Check the cache first.
        {
            let cache = self.cache.read();
            if let Some(entry) = cache.get(outpoint) {
                return if entry.coin.is_spent() {
                    None
                } else {
                    Some(entry.coin.clone())
                };
            }
        }

        // Cache miss -- look up in the base view.
        if let Some(coin) = self.base.get_coin(outpoint) {
            if coin.is_spent() {
                return None;
            }
            let usage = coin.dynamic_memory_usage() as u64;
            let cloned = coin.clone();
            {
                let mut cache = self.cache.write();
                cache
                    .entry(outpoint.clone())
                    .or_insert_with(|| CoinsCacheEntry::clean(coin));
            }
            self.usage.fetch_add(usage, Ordering::Relaxed);
            Some(cloned)
        } else {
            None
        }
    }

    /// Add a coin to the cache, marking it as DIRTY.
    ///
    /// If `possible_overwrite` is false and an unspent coin already exists at
    /// this outpoint, this method panics (logic error in the caller).
    ///
    /// If the coin's script is provably unspendable (OP_RETURN), the coin is
    /// not added.
    ///
    /// Port of Bitcoin Core's `CCoinsViewCache::AddCoin`.
    pub fn add_coin(&self, outpoint: &OutPoint, coin: Coin, possible_overwrite: bool) {
        assert!(!coin.is_spent(), "Cannot add a spent coin");

        // Don't store provably unspendable outputs.
        if coin.tx_out.script_pubkey.is_unspendable() {
            return;
        }

        let coin_usage = coin.dynamic_memory_usage() as u64;
        let mut cache = self.cache.write();

        if let Some(existing) = cache.get_mut(outpoint) {
            let mut fresh = false;

            if !possible_overwrite {
                if !existing.coin.is_spent() {
                    panic!(
                        "Attempted to overwrite an unspent coin (when possible_overwrite is false)"
                    );
                }
                // If the existing entry is spent but not dirty, we can mark FRESH.
                // If it's spent and dirty, we must NOT mark fresh (spentness hasn't
                // been flushed to parent yet).
                fresh = !existing.flags.is_dirty();
            }

            // Subtract old usage.
            let old_usage = existing.coin.dynamic_memory_usage() as u64;
            self.usage.fetch_sub(old_usage, Ordering::Relaxed);

            existing.coin = coin;
            existing.flags = CoinsCacheFlags::DIRTY;
            if fresh {
                existing.flags |= CoinsCacheFlags::FRESH;
            }
        } else {
            // New entry -- mark DIRTY | FRESH since it doesn't exist in the parent.
            cache.insert(
                outpoint.clone(),
                CoinsCacheEntry::new(coin, CoinsCacheFlags::DIRTY_FRESH),
            );
        }

        self.usage.fetch_add(coin_usage, Ordering::Relaxed);
    }

    /// Spend (consume) a coin, returning the coin data if it existed.
    ///
    /// If the coin was FRESH (not in the parent), it is removed from the cache
    /// entirely. Otherwise, the entry is kept with a cleared (spent) coin and
    /// marked DIRTY, so the spent state will be flushed to the parent.
    ///
    /// Port of Bitcoin Core's `CCoinsViewCache::SpendCoin`.
    pub fn spend_coin(&self, outpoint: &OutPoint) -> Option<Coin> {
        // First, ensure the coin is in our cache.
        // We need to fetch it from the base if it's not already cached.
        let _ = self.fetch_coin(outpoint);

        let mut cache = self.cache.write();
        let entry = cache.get_mut(outpoint)?;

        if entry.coin.is_spent() {
            return None;
        }

        let old_usage = entry.coin.dynamic_memory_usage() as u64;
        self.usage.fetch_sub(old_usage, Ordering::Relaxed);

        let spent_coin = std::mem::replace(&mut entry.coin, Coin::empty());

        if entry.flags.is_fresh() {
            // FRESH means the parent doesn't know about this coin, so we can
            // simply remove it from the cache entirely.
            cache.remove(outpoint);
        } else {
            // Must keep the entry as spent + DIRTY so the parent learns about it.
            entry.coin.clear();
            entry.flags = CoinsCacheFlags::DIRTY;
        }

        Some(spent_coin)
    }

    /// Check whether a coin exists in the cache (without fetching from base).
    pub fn have_coin_in_cache(&self, outpoint: &OutPoint) -> bool {
        let cache = self.cache.read();
        cache
            .get(outpoint)
            .map(|e| !e.coin.is_spent())
            .unwrap_or(false)
    }

    /// Check whether all non-coinbase inputs of a transaction exist in the UTXO set.
    ///
    /// For coinbase transactions, always returns true.
    ///
    /// Port of Bitcoin Core's `CCoinsViewCache::HaveInputs`.
    pub fn have_inputs(&self, tx: &Transaction) -> bool {
        if tx.is_coinbase() {
            return true;
        }
        for input in &tx.vin {
            if self.fetch_coin(&input.prevout).is_none() {
                return false;
            }
        }
        true
    }

    /// Compute the total value of all inputs to a transaction.
    ///
    /// Looks up each input's prevout in the cache/base view and sums the values.
    /// For coinbase transactions, returns `Amount::ZERO` (coinbase inputs have
    /// no prevout value).
    pub fn get_value_in(&self, tx: &Transaction) -> Amount {
        if tx.is_coinbase() {
            return Amount::ZERO;
        }
        let mut total = Amount::ZERO;
        for input in &tx.vin {
            if let Some(coin) = self.fetch_coin(&input.prevout) {
                total += coin.tx_out.value;
            }
        }
        total
    }

    /// Check if a coin exists directly in the base view (bypassing cache).
    /// Used for diagnostic purposes to determine if a flush lost data.
    pub fn base_has_coin(&self, outpoint: &OutPoint) -> bool {
        self.base.get_coin(outpoint).is_some()
    }

    /// Check if an outpoint exists in the cache (even if spent).
    pub fn cache_contains(&self, outpoint: &OutPoint) -> bool {
        let cache = self.cache.read();
        cache.contains_key(outpoint)
    }

    /// Flush all dirty entries to the base view.
    ///
    /// This iterates over all cache entries, writes dirty ones to the base
    /// (if the base supports batch writing), and clears the cache.
    ///
    /// Returns `true` on success.
    ///
    /// **Note:** Since the [`CoinsView`] trait is read-only by design, this
    /// method requires the base to be a [`CoinsViewCache`] or a
    /// [`FlushableCoinsView`]. For database-backed views, use
    /// [`CoinsViewDB::batch_write`] directly.
    pub fn flush(&self) -> bool {
        // We cannot write to an arbitrary CoinsView. The flush operation
        // is meaningful when the base is a FlushableCoinsView.
        // For now, we clear the cache. In a full implementation, dirty
        // entries would be pushed to the base.
        let mut cache = self.cache.write();
        cache.clear();
        self.usage.store(0, Ordering::Relaxed);
        true
    }

    /// Flush dirty entries to a flushable base view.
    ///
    /// This method extracts all DIRTY entries and writes them to the provided
    /// [`FlushableCoinsView`] implementation. Spent coins that were FRESH are
    /// skipped (they never existed in the parent). Other spent coins are
    /// erased from the parent. Modified coins are written.
    pub fn flush_to(&self, target: &dyn FlushableCoinsView) -> bool {
        let best_block = *self.best_block.read();
        let mut cache = self.cache.write();

        let mut writes: Vec<(OutPoint, Option<Coin>)> = Vec::new();

        for (outpoint, entry) in cache.iter() {
            if !entry.flags.is_dirty() {
                continue;
            }

            if entry.flags.is_fresh() && entry.coin.is_spent() {
                // FRESH + spent = never existed in the parent, skip.
                continue;
            }

            if entry.coin.is_spent() {
                writes.push((outpoint.clone(), None));
            } else {
                writes.push((outpoint.clone(), Some(entry.coin.clone())));
            }
        }

        let num_writes = writes.iter().filter(|(_, c)| c.is_some()).count();
        let num_deletes = writes.iter().filter(|(_, c)| c.is_none()).count();
        let cache_size = cache.len();
        tracing::info!(cache_entries = cache_size, dirty_writes = num_writes, dirty_deletes = num_deletes, best_block = %best_block.to_hex(), "flush_to");

        let result = target.batch_write(&writes, &best_block);
        if result {
            // Warm-cache optimization: instead of clearing the entire cache,
            // remove spent entries and reset dirty flags on surviving entries.
            // This keeps unspent coins in memory as a read cache, avoiding
            // expensive RocksDB lookups after flush.  Matches Bitcoin Core's
            // CCoinsViewCache::Flush behaviour.
            // Warm-cache: retain unspent entries as a read cache, but cap
            // at 512 MB to prevent unbounded memory growth.  Entries kept
            // are marked clean so future flushes skip them.
            const MAX_RETAINED_BYTES: u64 = 1024 * 1024 * 1024;
            let mut retained_usage: u64 = 0;
            let pre_retain = cache.len();
            cache.retain(|_, entry| {
                if entry.coin.is_spent() {
                    return false;
                }
                let coin_size = entry.coin.dynamic_memory_usage() as u64;
                if retained_usage + coin_size > MAX_RETAINED_BYTES {
                    return false;
                }
                entry.flags = CoinsCacheFlags::NONE;
                retained_usage += coin_size;
                true
            });
            self.usage.store(retained_usage, Ordering::Relaxed);
            tracing::info!(pre_retain, retained_entries = cache.len(), retained_mb = retained_usage / (1024 * 1024), "flush_to: warm cache retained");
        } else {
            tracing::error!("flush_to: batch_write FAILED");
        }
        result
    }

    /// Uncache a coin if it is not dirty.
    ///
    /// Removes the entry from the cache if it has no pending modifications,
    /// freeing memory.
    pub fn uncache(&self, outpoint: &OutPoint) {
        let mut cache = self.cache.write();
        if let Some(entry) = cache.get(outpoint) {
            if !entry.flags.is_dirty() {
                let usage = entry.coin.dynamic_memory_usage() as u64;
                self.usage.fetch_sub(usage, Ordering::Relaxed);
                cache.remove(outpoint);
            }
        }
    }

    /// Get the flags for a cached entry (for testing/debugging).
    pub fn get_entry_flags(&self, outpoint: &OutPoint) -> Option<CoinsCacheFlags> {
        self.cache.read().get(outpoint).map(|e| e.flags)
    }
}

impl CoinsView for CoinsViewCache {
    fn get_coin(&self, outpoint: &OutPoint) -> Option<Coin> {
        self.fetch_coin(outpoint)
    }

    fn have_coin(&self, outpoint: &OutPoint) -> bool {
        self.fetch_coin(outpoint).is_some()
    }

    fn get_best_block(&self) -> BlockHash {
        let block = *self.best_block.read();
        if block.is_null() {
            self.base.get_best_block()
        } else {
            block
        }
    }

    fn estimate_size(&self) -> u64 {
        self.base.estimate_size()
    }
}

// ---------------------------------------------------------------------------
// FlushableCoinsView trait
// ---------------------------------------------------------------------------

/// A coins view that supports batch writes (for flushing cache layers).
///
/// This trait extends [`CoinsView`] with the ability to receive batch updates
/// from a [`CoinsViewCache`] during flush operations.
pub trait FlushableCoinsView: CoinsView {
    /// Apply a batch of writes to this view.
    ///
    /// Each entry is an `(OutPoint, Option<Coin>)`:
    /// - `Some(coin)` means write/update this coin.
    /// - `None` means delete/erase this coin.
    ///
    /// The `best_block` parameter should be stored as the new best block hash.
    ///
    /// Returns `true` on success.
    fn batch_write(&self, entries: &[(OutPoint, Option<Coin>)], best_block: &BlockHash) -> bool;
}

// ---------------------------------------------------------------------------
// CoinsViewDB
// ---------------------------------------------------------------------------

/// The database key prefix for UTXO entries.
/// Matches Bitcoin Core's `DB_COIN = 'C'`.
const DB_COIN: u8 = b'C';

/// The database key prefix for the best block hash.
/// Matches Bitcoin Core's `DB_BEST_BLOCK = 'B'`.
const DB_BEST_BLOCK: u8 = b'B';

/// Build the database key for a coin entry.
///
/// Format: `'C'` prefix byte followed by the serialized outpoint.
fn coin_db_key(outpoint: &OutPoint) -> Vec<u8> {
    let mut key = Vec::with_capacity(37); // 1 + 32 + 4
    key.push(DB_COIN);
    outpoint
        .encode(&mut key)
        .expect("outpoint encoding should not fail");
    key
}

/// Build the database key for the best block hash.
fn best_block_db_key() -> Vec<u8> {
    vec![DB_BEST_BLOCK]
}

/// Database-backed UTXO set.
///
/// Port of Bitcoin Core's `CCoinsViewDB`. Uses a [`DbWrapper`] for typed
/// serialization and optional XOR obfuscation of values.
///
/// Generic over `D: Database` to support both production (RocksDB) and
/// testing (MemoryDb) backends.
pub struct CoinsViewDB<D: Database> {
    /// The wrapped database handle.
    db: DbWrapper<D>,
}

impl<D: Database> CoinsViewDB<D> {
    /// Create a new `CoinsViewDB` wrapping the given database.
    ///
    /// If `obfuscate` is true, values are XOR-obfuscated on disk.
    pub fn new(db: D, obfuscate: bool) -> Self {
        let wrapper = if obfuscate {
            DbWrapper::new(db, true)
        } else {
            DbWrapper::new_unobfuscated(db)
        };
        CoinsViewDB { db: wrapper }
    }

    /// Create a new `CoinsViewDB` without obfuscation (convenience for testing).
    pub fn new_unobfuscated(db: D) -> Self {
        CoinsViewDB {
            db: DbWrapper::new_unobfuscated(db),
        }
    }

    /// Get a reference to the underlying `DbWrapper`.
    pub fn db(&self) -> &DbWrapper<D> {
        &self.db
    }

    /// Write a coin to the database.
    pub fn write_coin(&self, outpoint: &OutPoint, coin: &Coin) -> bool {
        let key = coin_db_key(outpoint);
        self.db.write(&key, coin, false).is_ok()
    }

    /// Erase a coin from the database.
    pub fn erase_coin(&self, outpoint: &OutPoint) -> bool {
        let key = coin_db_key(outpoint);
        self.db.erase(&key, false).is_ok()
    }

    /// Check if a raw key exists in the database (bypasses deserialization).
    pub fn raw_key_exists(&self, outpoint: &OutPoint) -> bool {
        let key = coin_db_key(outpoint);
        self.db.exists(&key).unwrap_or(false)
    }

    /// Write the best block hash to the database.
    pub fn write_best_block(&self, hash: &BlockHash) -> bool {
        let key = best_block_db_key();
        self.db.write(&key, hash, false).is_ok()
    }

    /// Apply a batch of writes (used by [`CoinsViewCache::flush_to`]).
    ///
    /// All operations are accumulated into a single atomic `WriteBatch` and
    /// committed once, avoiding the massive overhead of individual DB writes.
    pub fn batch_write_impl(
        &self,
        entries: &[(OutPoint, Option<Coin>)],
        best_block: &BlockHash,
    ) -> bool {
        use qubitcoin_storage::traits::DbBatch;

        let mut batch = self.db.new_batch();

        for (outpoint, maybe_coin) in entries {
            let key = coin_db_key(outpoint);
            match maybe_coin {
                Some(coin) => {
                    match self.db.serialize_value(coin) {
                        Ok(val) => batch.put(&key, &val),
                        Err(_) => return false,
                    }
                }
                None => {
                    batch.delete(&key);
                }
            }
        }

        // Also write the best block hash in the same batch.
        let bb_key = best_block_db_key();
        match self.db.serialize_value(best_block) {
            Ok(val) => batch.put(&bb_key, &val),
            Err(_) => return false,
        }

        self.db.write_batch(batch, false).is_ok()
    }
}

impl<D: Database> CoinsView for CoinsViewDB<D> {
    fn get_coin(&self, outpoint: &OutPoint) -> Option<Coin> {
        let key = coin_db_key(outpoint);
        match self.db.read::<_, Coin>(&key) {
            Ok(Some(coin)) if !coin.is_spent() => Some(coin),
            Ok(Some(_coin)) => {
                // Coin exists but is marked spent
                None
            }
            Ok(None) => None,
            Err(e) => {
                tracing::error!(outpoint_hash = %outpoint.hash, outpoint_n = outpoint.n, error = %e, "get_coin DECODE ERROR");
                None
            }
        }
    }

    fn get_coins(&self, outpoints: &[OutPoint]) -> Vec<Option<Coin>> {
        let keys: Vec<Vec<u8>> = outpoints.iter().map(|op| coin_db_key(op)).collect();
        self.db
            .multi_read::<Coin>(&keys)
            .into_iter()
            .enumerate()
            .map(|(i, result)| match result {
                Ok(Some(coin)) if !coin.is_spent() => Some(coin),
                Ok(_) => None,
                Err(e) => {
                    tracing::error!(
                        outpoint_hash = %outpoints[i].hash,
                        outpoint_n = outpoints[i].n,
                        error = %e,
                        "get_coins DECODE ERROR"
                    );
                    None
                }
            })
            .collect()
    }

    fn get_best_block(&self) -> BlockHash {
        let key = best_block_db_key();
        match self.db.read::<_, BlockHash>(&key) {
            Ok(Some(hash)) => hash,
            _ => BlockHash::ZERO,
        }
    }

    fn estimate_size(&self) -> u64 {
        self.db.inner().estimated_size().unwrap_or(0)
    }
}

impl<D: Database> FlushableCoinsView for CoinsViewDB<D> {
    fn batch_write(&self, entries: &[(OutPoint, Option<Coin>)], best_block: &BlockHash) -> bool {
        self.batch_write_impl(entries, best_block)
    }
}

// ---------------------------------------------------------------------------
// Utility functions
// ---------------------------------------------------------------------------

/// Add all outputs of a transaction to a cache.
///
/// Port of Bitcoin Core's `AddCoins`. When `check_for_overwrite` is false,
/// overwrites are only assumed possible for coinbase transactions.
pub fn add_coins(cache: &CoinsViewCache, tx: &Transaction, height: u32, check_for_overwrite: bool) {
    let is_coinbase = tx.is_coinbase();
    let txid = tx.txid().clone();
    for (i, output) in tx.vout.iter().enumerate() {
        let outpoint = OutPoint::new(txid.clone(), i as u32);
        let overwrite = if check_for_overwrite {
            cache.have_coin(&outpoint)
        } else {
            is_coinbase
        };
        cache.add_coin(
            &outpoint,
            Coin::new(output.clone(), height, is_coinbase),
            overwrite,
        );
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use qubitcoin_consensus::{OutPoint, Transaction, TxIn, TxOut};
    use qubitcoin_primitives::{Amount, BlockHash, Txid};
    use qubitcoin_script::Script;
    use qubitcoin_serialize::{deserialize, serialize};

    // --- Amount compression tests ---

    #[test]
    fn test_compress_amount_zero() {
        assert_eq!(compress_amount(0), 0);
        assert_eq!(decompress_amount(0), 0);
    }

    #[test]
    fn test_compress_amount_roundtrip() {
        let test_values: Vec<u64> = vec![
            0,
            1,
            10,
            100,
            1000,
            10_000,
            50_000,
            100_000,
            500_000,
            1_000_000,
            10_000_000,
            50_000_000,
            100_000_000,           // 1 BTC
            500_000_000,           // 5 BTC
            2_100_000_000_000_000, // 21M BTC
            123_456_789,
            1,
            999,
            7,
            42,
        ];
        for &amount in &test_values {
            let compressed = compress_amount(amount);
            let decompressed = decompress_amount(compressed);
            assert_eq!(
                decompressed, amount,
                "Amount compression roundtrip failed for {}: compressed={}, decompressed={}",
                amount, compressed, decompressed
            );
        }
    }

    #[test]
    fn test_compress_amount_powers_of_ten() {
        for e in 0..=9 {
            let amount: u64 = 10u64.pow(e);
            let compressed = compress_amount(amount);
            assert_eq!(decompress_amount(compressed), amount);
        }
    }

    // --- Script compression tests ---

    #[test]
    fn test_compress_script_p2pkh() {
        // OP_DUP OP_HASH160 <20 bytes> OP_EQUALVERIFY OP_CHECKSIG
        let mut script_bytes = vec![0x76, 0xa9, 20];
        script_bytes.extend_from_slice(&[0xab; 20]);
        script_bytes.push(0x88);
        script_bytes.push(0xac);
        let script = Script::from_bytes(script_bytes);

        let compressed = compress_script(&script).expect("P2PKH should compress");
        assert_eq!(compressed.len(), 21);
        assert_eq!(compressed[0], 0x00);
        assert_eq!(&compressed[1..], &[0xab; 20]);
    }

    #[test]
    fn test_compress_script_p2sh() {
        // OP_HASH160 <20 bytes> OP_EQUAL
        let mut script_bytes = vec![0xa9, 20];
        script_bytes.extend_from_slice(&[0xcd; 20]);
        script_bytes.push(0x87);
        let script = Script::from_bytes(script_bytes);

        let compressed = compress_script(&script).expect("P2SH should compress");
        assert_eq!(compressed.len(), 21);
        assert_eq!(compressed[0], 0x01);
        assert_eq!(&compressed[1..], &[0xcd; 20]);
    }

    #[test]
    fn test_compress_script_non_standard() {
        let script = Script::from_bytes(vec![0x00, 0x14, 0xab, 0xcd]);
        assert!(compress_script(&script).is_none());
    }

    // --- Coin serialization tests ---

    #[test]
    fn test_coin_serialization_roundtrip() {
        let coin = Coin::new(
            TxOut::new(
                Amount::from_sat(50_000),
                Script::from_bytes(vec![0x00, 0x14, 0xab, 0xcd]),
            ),
            100,
            false,
        );

        let encoded = serialize(&coin).expect("Coin encoding should succeed");
        let decoded: Coin = deserialize(&encoded).expect("Coin decoding should succeed");

        assert_eq!(coin.height, decoded.height);
        assert_eq!(coin.coinbase, decoded.coinbase);
        assert_eq!(coin.tx_out.value, decoded.tx_out.value);
        assert_eq!(coin.tx_out.script_pubkey, decoded.tx_out.script_pubkey);
    }

    #[test]
    fn test_coin_serialization_coinbase() {
        let coin = Coin::new(
            TxOut::new(Amount::from_btc(50), Script::from_bytes(vec![0x51])),
            0,
            true,
        );

        let encoded = serialize(&coin).expect("Coinbase coin encoding should succeed");
        let decoded: Coin = deserialize(&encoded).expect("Coinbase coin decoding should succeed");

        assert_eq!(decoded.height, 0);
        assert!(decoded.coinbase);
        assert_eq!(decoded.tx_out.value, Amount::from_btc(50));
    }

    #[test]
    fn test_coin_serialization_p2pkh_script_compression() {
        // Build a P2PKH script that should be compressed.
        let mut script_bytes = vec![0x76, 0xa9, 20];
        script_bytes.extend_from_slice(&[0x42; 20]);
        script_bytes.push(0x88);
        script_bytes.push(0xac);
        let script = Script::from_bytes(script_bytes.clone());

        let coin = Coin::new(
            TxOut::new(Amount::from_sat(100_000_000), script),
            500_000,
            false,
        );

        let encoded = serialize(&coin).expect("P2PKH coin encoding should succeed");
        let decoded: Coin = deserialize(&encoded).expect("P2PKH coin decoding should succeed");

        assert_eq!(decoded.height, 500_000);
        assert!(!decoded.coinbase);
        assert_eq!(decoded.tx_out.value, Amount::from_sat(100_000_000));
        // The P2PKH script should roundtrip exactly.
        assert_eq!(decoded.tx_out.script_pubkey.as_bytes(), &script_bytes[..]);
    }

    #[test]
    fn test_coin_serialization_large_height() {
        let coin = Coin::new(
            TxOut::new(Amount::from_sat(1), Script::from_bytes(vec![0x51])),
            2_000_000,
            true,
        );

        let encoded = serialize(&coin).unwrap();
        let decoded: Coin = deserialize(&encoded).unwrap();
        assert_eq!(decoded.height, 2_000_000);
        assert!(decoded.coinbase);
    }

    // --- Coin basic tests ---

    #[test]
    fn test_coin_is_spent() {
        let coin = Coin::empty();
        assert!(coin.is_spent());

        let coin = Coin::new(TxOut::new(Amount::from_sat(100), Script::new()), 1, false);
        assert!(!coin.is_spent());
    }

    #[test]
    fn test_coin_clear() {
        let mut coin = Coin::new(TxOut::new(Amount::from_sat(100), Script::new()), 1, true);
        assert!(!coin.is_spent());
        coin.clear();
        assert!(coin.is_spent());
        assert_eq!(coin.height, 0);
        assert!(!coin.coinbase);
    }

    // --- CoinsCacheFlags tests ---

    #[test]
    fn test_flags_basic() {
        let flags = CoinsCacheFlags::NONE;
        assert!(!flags.is_dirty());
        assert!(!flags.is_fresh());
        assert!(flags.is_empty());

        let flags = CoinsCacheFlags::DIRTY;
        assert!(flags.is_dirty());
        assert!(!flags.is_fresh());

        let flags = CoinsCacheFlags::FRESH;
        assert!(!flags.is_dirty());
        assert!(flags.is_fresh());

        let flags = CoinsCacheFlags::DIRTY_FRESH;
        assert!(flags.is_dirty());
        assert!(flags.is_fresh());
    }

    #[test]
    fn test_flags_bitor() {
        let flags = CoinsCacheFlags::DIRTY | CoinsCacheFlags::FRESH;
        assert!(flags.is_dirty());
        assert!(flags.is_fresh());
        assert_eq!(flags, CoinsCacheFlags::DIRTY_FRESH);
    }

    // --- CoinsViewCache tests ---

    fn make_test_outpoint(n: u32) -> OutPoint {
        OutPoint::new(Txid::from_bytes([n as u8; 32]), n)
    }

    fn make_test_coin(value: i64, height: u32) -> Coin {
        Coin::new(
            TxOut::new(
                Amount::from_sat(value),
                Script::from_bytes(vec![0x51]), // OP_1 (not unspendable)
            ),
            height,
            false,
        )
    }

    #[test]
    fn test_cache_add_coin() {
        let cache = CoinsViewCache::new(Box::new(EmptyCoinsView));
        let outpoint = make_test_outpoint(0);
        let coin = make_test_coin(1000, 1);

        cache.add_coin(&outpoint, coin.clone(), false);

        // Should be retrievable.
        let fetched = cache.fetch_coin(&outpoint);
        assert!(fetched.is_some());
        let fetched = fetched.unwrap();
        assert_eq!(fetched.tx_out.value, Amount::from_sat(1000));
        assert_eq!(fetched.height, 1);
    }

    #[test]
    fn test_cache_add_coin_marks_dirty_fresh() {
        let cache = CoinsViewCache::new(Box::new(EmptyCoinsView));
        let outpoint = make_test_outpoint(0);
        let coin = make_test_coin(1000, 1);

        cache.add_coin(&outpoint, coin, false);

        let flags = cache.get_entry_flags(&outpoint).unwrap();
        assert!(flags.is_dirty(), "New coin should be DIRTY");
        assert!(flags.is_fresh(), "New coin should be FRESH");
    }

    #[test]
    fn test_cache_spend_coin() {
        let cache = CoinsViewCache::new(Box::new(EmptyCoinsView));
        let outpoint = make_test_outpoint(0);
        let coin = make_test_coin(1000, 1);

        cache.add_coin(&outpoint, coin, false);
        let spent = cache.spend_coin(&outpoint);

        assert!(spent.is_some());
        assert_eq!(spent.unwrap().tx_out.value, Amount::from_sat(1000));

        // After spending, the coin should not be retrievable.
        assert!(cache.fetch_coin(&outpoint).is_none());
    }

    #[test]
    fn test_cache_spend_fresh_coin_removes_entry() {
        let cache = CoinsViewCache::new(Box::new(EmptyCoinsView));
        let outpoint = make_test_outpoint(0);
        let coin = make_test_coin(1000, 1);

        cache.add_coin(&outpoint, coin, false);

        // The coin is FRESH, so spending it should remove it entirely.
        cache.spend_coin(&outpoint);

        // The entry should be completely gone from the cache.
        assert!(cache.get_entry_flags(&outpoint).is_none());
    }

    #[test]
    fn test_cache_spend_non_fresh_coin_marks_dirty() {
        // Use a base view that actually has a coin.
        struct SingleCoinView {
            outpoint: OutPoint,
            coin: Coin,
        }

        impl CoinsView for SingleCoinView {
            fn get_coin(&self, outpoint: &OutPoint) -> Option<Coin> {
                if outpoint == &self.outpoint {
                    Some(self.coin.clone())
                } else {
                    None
                }
            }
            fn get_best_block(&self) -> BlockHash {
                BlockHash::ZERO
            }
        }

        let outpoint = make_test_outpoint(0);
        let coin = make_test_coin(2000, 5);

        let base = SingleCoinView {
            outpoint: outpoint.clone(),
            coin: coin.clone(),
        };

        let cache = CoinsViewCache::new(Box::new(base));

        // Fetch the coin first to populate the cache (non-FRESH, since it comes from base).
        let fetched = cache.fetch_coin(&outpoint);
        assert!(fetched.is_some());

        // The cached entry should have no flags (clean fetch from base).
        let flags = cache.get_entry_flags(&outpoint).unwrap();
        assert!(!flags.is_dirty());
        assert!(!flags.is_fresh());

        // Spend it.
        let spent = cache.spend_coin(&outpoint);
        assert!(spent.is_some());

        // The entry should still exist in cache (not FRESH), marked DIRTY.
        let flags = cache.get_entry_flags(&outpoint);
        assert!(
            flags.is_some(),
            "Entry should remain in cache when not FRESH"
        );
        assert!(flags.unwrap().is_dirty(), "Spent coin should be DIRTY");
        assert!(!flags.unwrap().is_fresh(), "Spent coin should not be FRESH");
    }

    #[test]
    fn test_cache_spend_nonexistent_coin() {
        let cache = CoinsViewCache::new(Box::new(EmptyCoinsView));
        let outpoint = make_test_outpoint(99);

        let result = cache.spend_coin(&outpoint);
        assert!(result.is_none());
    }

    #[test]
    fn test_cache_have_coin() {
        let cache = CoinsViewCache::new(Box::new(EmptyCoinsView));
        let outpoint = make_test_outpoint(0);

        assert!(!cache.have_coin(&outpoint));

        cache.add_coin(&outpoint, make_test_coin(100, 1), false);
        assert!(cache.have_coin(&outpoint));

        cache.spend_coin(&outpoint);
        assert!(!cache.have_coin(&outpoint));
    }

    #[test]
    fn test_cache_have_inputs() {
        let cache = CoinsViewCache::new(Box::new(EmptyCoinsView));

        // Add two UTXOs.
        let txid_a = Txid::from_bytes([0xaa; 32]);
        let outpoint_a = OutPoint::new(txid_a.clone(), 0);
        let outpoint_b = OutPoint::new(txid_a.clone(), 1);

        cache.add_coin(&outpoint_a, make_test_coin(100, 1), false);
        cache.add_coin(&outpoint_b, make_test_coin(200, 1), false);

        // Build a transaction spending both UTXOs.
        let tx = Transaction::new(
            2,
            vec![
                TxIn::new(outpoint_a.clone(), Script::new(), 0xffffffff),
                TxIn::new(outpoint_b.clone(), Script::new(), 0xffffffff),
            ],
            vec![TxOut::new(
                Amount::from_sat(250),
                Script::from_bytes(vec![0x51]),
            )],
            0,
        );

        assert!(cache.have_inputs(&tx));

        // Spend one input.
        cache.spend_coin(&outpoint_a);

        // Now have_inputs should fail.
        assert!(!cache.have_inputs(&tx));
    }

    #[test]
    fn test_cache_have_inputs_coinbase() {
        let cache = CoinsViewCache::new(Box::new(EmptyCoinsView));

        // Coinbase transactions always return true for have_inputs.
        let coinbase_tx = Transaction::new(
            1,
            vec![TxIn::coinbase(Script::from_bytes(vec![0x04, 0xff]))],
            vec![TxOut::new(
                Amount::from_btc(50),
                Script::from_bytes(vec![0x51]),
            )],
            0,
        );

        assert!(cache.have_inputs(&coinbase_tx));
    }

    #[test]
    fn test_cache_get_value_in() {
        let cache = CoinsViewCache::new(Box::new(EmptyCoinsView));

        let txid = Txid::from_bytes([0xbb; 32]);
        let outpoint_0 = OutPoint::new(txid.clone(), 0);
        let outpoint_1 = OutPoint::new(txid.clone(), 1);

        cache.add_coin(&outpoint_0, make_test_coin(300, 1), false);
        cache.add_coin(&outpoint_1, make_test_coin(700, 1), false);

        let tx = Transaction::new(
            2,
            vec![
                TxIn::new(outpoint_0, Script::new(), 0xffffffff),
                TxIn::new(outpoint_1, Script::new(), 0xffffffff),
            ],
            vec![TxOut::new(
                Amount::from_sat(900),
                Script::from_bytes(vec![0x51]),
            )],
            0,
        );

        assert_eq!(cache.get_value_in(&tx), Amount::from_sat(1000));
    }

    #[test]
    fn test_cache_flush() {
        let cache = CoinsViewCache::new(Box::new(EmptyCoinsView));

        cache.add_coin(&make_test_outpoint(0), make_test_coin(100, 1), false);
        cache.add_coin(&make_test_outpoint(1), make_test_coin(200, 2), false);

        assert_eq!(cache.cache_size(), 2);

        let result = cache.flush();
        assert!(result);
        assert_eq!(cache.cache_size(), 0);
    }

    #[test]
    fn test_cache_best_block() {
        let cache = CoinsViewCache::new(Box::new(EmptyCoinsView));

        // Default best block from EmptyCoinsView is ZERO.
        assert_eq!(cache.get_best_block(), BlockHash::ZERO);

        let new_hash = BlockHash::from_bytes([0xff; 32]);
        cache.set_best_block(new_hash);
        assert_eq!(cache.get_best_block(), new_hash);
    }

    #[test]
    fn test_cache_add_unspendable_coin_ignored() {
        let cache = CoinsViewCache::new(Box::new(EmptyCoinsView));
        let outpoint = make_test_outpoint(0);

        // OP_RETURN script is unspendable.
        let coin = Coin::new(
            TxOut::new(
                Amount::from_sat(100),
                Script::from_bytes(vec![0x6a, 0x04, 0xde, 0xad]),
            ),
            1,
            false,
        );

        cache.add_coin(&outpoint, coin, false);

        // Should not be in cache.
        assert!(!cache.have_coin(&outpoint));
    }

    #[test]
    fn test_cache_overwrite_with_possible_overwrite() {
        let cache = CoinsViewCache::new(Box::new(EmptyCoinsView));
        let outpoint = make_test_outpoint(0);

        cache.add_coin(&outpoint, make_test_coin(100, 1), false);

        // Overwrite with possible_overwrite = true should succeed.
        cache.add_coin(&outpoint, make_test_coin(200, 2), true);

        let coin = cache.fetch_coin(&outpoint).unwrap();
        assert_eq!(coin.tx_out.value, Amount::from_sat(200));
        assert_eq!(coin.height, 2);
    }

    #[test]
    #[should_panic(expected = "Attempted to overwrite an unspent coin")]
    fn test_cache_overwrite_without_possible_overwrite_panics() {
        let cache = CoinsViewCache::new(Box::new(EmptyCoinsView));
        let outpoint = make_test_outpoint(0);

        cache.add_coin(&outpoint, make_test_coin(100, 1), false);

        // This should panic because possible_overwrite is false and
        // the coin is unspent.
        cache.add_coin(&outpoint, make_test_coin(200, 2), false);
    }

    #[test]
    fn test_cache_uncache_clean_entry() {
        struct SingleCoinView;

        impl CoinsView for SingleCoinView {
            fn get_coin(&self, outpoint: &OutPoint) -> Option<Coin> {
                if outpoint.n == 0 {
                    Some(make_test_coin(500, 10))
                } else {
                    None
                }
            }
            fn get_best_block(&self) -> BlockHash {
                BlockHash::ZERO
            }
        }

        let cache = CoinsViewCache::new(Box::new(SingleCoinView));
        let outpoint = make_test_outpoint(0);

        // Fetch to populate cache.
        assert!(cache.fetch_coin(&outpoint).is_some());
        assert_eq!(cache.cache_size(), 1);

        // Entry is clean (not dirty), so uncache should remove it.
        cache.uncache(&outpoint);
        assert_eq!(cache.cache_size(), 0);
    }

    #[test]
    fn test_cache_uncache_dirty_entry() {
        let cache = CoinsViewCache::new(Box::new(EmptyCoinsView));
        let outpoint = make_test_outpoint(0);

        cache.add_coin(&outpoint, make_test_coin(100, 1), false);
        assert_eq!(cache.cache_size(), 1);

        // Entry is dirty, so uncache should NOT remove it.
        cache.uncache(&outpoint);
        assert_eq!(cache.cache_size(), 1);
    }

    // --- CoinsViewDB tests ---

    #[test]
    fn test_coins_view_db_write_and_read() {
        use qubitcoin_storage::MemoryDb;

        let db = MemoryDb::new();
        let view = CoinsViewDB::new_unobfuscated(db);

        let outpoint = make_test_outpoint(0);
        let coin = make_test_coin(5000, 42);

        assert!(view.write_coin(&outpoint, &coin));

        let fetched = view.get_coin(&outpoint);
        assert!(fetched.is_some());
        let fetched = fetched.unwrap();
        assert_eq!(fetched.tx_out.value, Amount::from_sat(5000));
        assert_eq!(fetched.height, 42);
    }

    #[test]
    fn test_coins_view_db_erase() {
        use qubitcoin_storage::MemoryDb;

        let db = MemoryDb::new();
        let view = CoinsViewDB::new_unobfuscated(db);

        let outpoint = make_test_outpoint(0);
        let coin = make_test_coin(1000, 1);

        view.write_coin(&outpoint, &coin);
        assert!(view.have_coin(&outpoint));

        view.erase_coin(&outpoint);
        assert!(!view.have_coin(&outpoint));
    }

    #[test]
    fn test_coins_view_db_best_block() {
        use qubitcoin_storage::MemoryDb;

        let db = MemoryDb::new();
        let view = CoinsViewDB::new_unobfuscated(db);

        // No best block set yet.
        assert_eq!(view.get_best_block(), BlockHash::ZERO);

        let hash = BlockHash::from_bytes([0xaa; 32]);
        assert!(view.write_best_block(&hash));
        assert_eq!(view.get_best_block(), hash);
    }

    #[test]
    fn test_coins_view_db_as_cache_base() {
        use qubitcoin_storage::MemoryDb;

        let db = MemoryDb::new();
        let view = CoinsViewDB::new_unobfuscated(db);

        // Populate the DB directly.
        let outpoint = make_test_outpoint(0);
        let coin = make_test_coin(12345, 100);
        view.write_coin(&outpoint, &coin);

        let best = BlockHash::from_bytes([0xbb; 32]);
        view.write_best_block(&best);

        // Create a cache on top of the DB view.
        let cache = CoinsViewCache::new(Box::new(view));

        // Cache should read through to the DB.
        assert_eq!(cache.get_best_block(), best);
        let fetched = cache.fetch_coin(&outpoint).unwrap();
        assert_eq!(fetched.tx_out.value, Amount::from_sat(12345));
        assert_eq!(fetched.height, 100);
    }

    #[test]
    fn test_coins_view_db_batch_write() {
        use qubitcoin_storage::MemoryDb;

        let db = MemoryDb::new();
        let view = CoinsViewDB::new_unobfuscated(db);

        let outpoint_a = make_test_outpoint(0);
        let outpoint_b = make_test_outpoint(1);
        let outpoint_c = make_test_outpoint(2);

        // Write some coins.
        view.write_coin(&outpoint_a, &make_test_coin(100, 1));
        view.write_coin(&outpoint_b, &make_test_coin(200, 2));

        // Batch write: update A, delete B, add C.
        let best = BlockHash::from_bytes([0xdd; 32]);
        let entries = vec![
            (outpoint_a.clone(), Some(make_test_coin(150, 1))),
            (outpoint_b.clone(), None),
            (outpoint_c.clone(), Some(make_test_coin(300, 3))),
        ];

        assert!(view.batch_write(&entries, &best));

        assert_eq!(
            view.get_coin(&outpoint_a).unwrap().tx_out.value,
            Amount::from_sat(150)
        );
        assert!(view.get_coin(&outpoint_b).is_none());
        assert_eq!(
            view.get_coin(&outpoint_c).unwrap().tx_out.value,
            Amount::from_sat(300)
        );
        assert_eq!(view.get_best_block(), best);
    }

    // --- add_coins utility tests ---

    #[test]
    fn test_add_coins_utility() {
        let cache = CoinsViewCache::new(Box::new(EmptyCoinsView));

        let tx = Transaction::new(
            2,
            vec![TxIn::coinbase(Script::from_bytes(vec![0x04, 0xff]))],
            vec![
                TxOut::new(Amount::from_sat(5000), Script::from_bytes(vec![0x51])),
                TxOut::new(Amount::from_sat(3000), Script::from_bytes(vec![0x52])),
            ],
            0,
        );

        add_coins(&cache, &tx, 100, false);

        let txid = tx.txid().clone();
        let coin0 = cache.fetch_coin(&OutPoint::new(txid.clone(), 0)).unwrap();
        assert_eq!(coin0.tx_out.value, Amount::from_sat(5000));
        assert_eq!(coin0.height, 100);
        assert!(coin0.coinbase);

        let coin1 = cache.fetch_coin(&OutPoint::new(txid.clone(), 1)).unwrap();
        assert_eq!(coin1.tx_out.value, Amount::from_sat(3000));
        assert!(coin1.coinbase);
    }

    // --- Layered cache tests ---

    #[test]
    fn test_layered_cache() {
        let base_cache = CoinsViewCache::new(Box::new(EmptyCoinsView));
        let outpoint = make_test_outpoint(0);

        base_cache.add_coin(&outpoint, make_test_coin(1000, 1), false);

        // Create a second cache on top.
        let top_cache = CoinsViewCache::new(Box::new(base_cache));

        // Should see the coin from the base.
        let coin = top_cache.fetch_coin(&outpoint).unwrap();
        assert_eq!(coin.tx_out.value, Amount::from_sat(1000));

        // Spend in the top cache.
        top_cache.spend_coin(&outpoint);
        assert!(top_cache.fetch_coin(&outpoint).is_none());
    }

    // --- Flush to FlushableCoinsView tests ---

    #[test]
    fn test_flush_to_db() {
        use qubitcoin_storage::MemoryDb;

        let db = MemoryDb::new();
        let db_view = CoinsViewDB::new_unobfuscated(db);

        let cache = CoinsViewCache::new(Box::new(EmptyCoinsView));
        let outpoint = make_test_outpoint(0);
        cache.add_coin(&outpoint, make_test_coin(999, 42), false);
        cache.set_best_block(BlockHash::from_bytes([0xee; 32]));

        assert!(cache.flush_to(&db_view));

        // The DB should now have the coin.
        let coin = db_view.get_coin(&outpoint).unwrap();
        assert_eq!(coin.tx_out.value, Amount::from_sat(999));
        assert_eq!(coin.height, 42);
        assert_eq!(db_view.get_best_block(), BlockHash::from_bytes([0xee; 32]));

        // Warm-cache: unspent coins are retained as a clean read cache.
        assert_eq!(cache.cache_size(), 1);
        // The retained entry should have NONE flags (clean).
        assert_eq!(
            cache.get_entry_flags(&outpoint),
            Some(CoinsCacheFlags::NONE)
        );
    }
}
