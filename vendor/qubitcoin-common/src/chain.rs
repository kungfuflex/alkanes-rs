//! Block index and chain types.
//!
//! Maps to: `src/chain.h` in Bitcoin Core.
//!
//! Provides:
//! - `BlockStatus`: Validation and storage status flags for a block.
//! - `BlockIndex`: In-memory representation of a block's metadata (port of `CBlockIndex`).
//! - `DiskBlockIndex`: On-disk serializable form of a block index entry (port of `CDiskBlockIndex`).
//! - `Chain`: An in-memory indexed chain of blocks (port of `CChain`).
//! - `get_bits_proof` / `get_block_proof`: Proof-of-work calculations.

use qubitcoin_consensus::BlockHeader;
use qubitcoin_primitives::{ArithUint256, BlockHash, Uint256};

use std::fmt;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum amount of time (in seconds) that a block timestamp is allowed to
/// exceed the current time before the block will be rejected.
pub const MAX_FUTURE_BLOCK_TIME: i64 = 2 * 60 * 60;

/// Timestamp window used as a grace period by code that compares external
/// timestamps (such as timestamps passed to RPCs, or wallet key creation
/// times) to block timestamps.
pub const TIMESTAMP_WINDOW: i64 = MAX_FUTURE_BLOCK_TIME;

/// Number of blocks whose timestamps are used to compute the median time past.
pub const MEDIAN_TIME_SPAN: usize = 11;

/// Sequence id assigned to blocks belonging to the best chain when loaded from disk.
pub const SEQ_ID_BEST_CHAIN_FROM_DISK: i32 = 0;

/// Sequence id assigned to blocks loaded from disk that do not belong to the best chain.
pub const SEQ_ID_INIT_FROM_DISK: i32 = 1;

// ---------------------------------------------------------------------------
// BlockStatus
// ---------------------------------------------------------------------------

/// Validation and storage status flags for a block index entry.
///
/// The lower 3 bits (`0x07`) encode a *validity level* as an integer (not
/// independent flags). The remaining bits are independent boolean flags.
///
/// This is a newtype wrapper rather than a bitflags enum because the validity
/// levels are ordinal values packed into the low bits, which does not fit the
/// bitflags model.
///
/// Maps to: `enum BlockStatus` in Bitcoin Core's `src/chain.h`.
#[derive(Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct BlockStatus(u32);

impl BlockStatus {
    // -- Validity levels (packed into bits 0-2 as an integer) ---------------

    /// Unused / unknown validity.
    pub const VALID_UNKNOWN: u32 = 0;
    /// Reserved (was VALID_HEADER).
    pub const VALID_RESERVED: u32 = 1;
    /// All parent headers found, difficulty matches, timestamp >= median previous.
    pub const VALID_TREE: u32 = 2;
    /// Transactions validated: coinbase ok, no duplicate txids, sigops/size ok, merkle root ok.
    pub const VALID_TRANSACTIONS: u32 = 3;
    /// Outputs do not overspend inputs, no double spends, coinbase output ok.
    pub const VALID_CHAIN: u32 = 4;
    /// Scripts and signatures ok.
    pub const VALID_SCRIPTS: u32 = 5;
    /// Mask covering the validity level bits.
    pub const VALID_MASK: u32 = 0x07;

    // -- Storage flags ------------------------------------------------------

    /// Full block available in `blk*.dat`.
    pub const HAVE_DATA: u32 = 8;
    /// Undo data available in `rev*.dat`.
    pub const HAVE_UNDO: u32 = 16;
    /// Mask covering both storage flags.
    pub const HAVE_MASK: u32 = Self::HAVE_DATA | Self::HAVE_UNDO;

    // -- Failure flags ------------------------------------------------------

    /// Stage after last reached validness failed.
    pub const FAILED_VALID: u32 = 32;
    /// Descendant of a failed block (unused in recent Core but kept for compatibility).
    pub const FAILED_CHILD: u32 = 64;
    /// Mask covering both failure flags.
    pub const FAILED_MASK: u32 = Self::FAILED_VALID | Self::FAILED_CHILD;

    // -- Witness flag -------------------------------------------------------

    /// Block data in `blk*.dat` was received with a witness-enforcing client.
    pub const OPT_WITNESS: u32 = 128;

    // -- Reserved -----------------------------------------------------------

    /// Reserved flag (previously used for assumeutxo snapshot blocks).
    pub const STATUS_RESERVED: u32 = 256;

    // -- Constructors / accessors -------------------------------------------

    /// Create a `BlockStatus` from raw bits.
    #[inline]
    pub const fn new(bits: u32) -> Self {
        BlockStatus(bits)
    }

    /// Return the raw `u32` representation.
    #[inline]
    pub const fn bits(self) -> u32 {
        self.0
    }

    /// Extract the validity level (lower 3 bits interpreted as an integer).
    #[inline]
    pub const fn validity(self) -> u32 {
        self.0 & Self::VALID_MASK
    }

    /// Check whether the validity level is at least `up_to` **and** the block
    /// has not been marked as failed.
    ///
    /// Mirrors `CBlockIndex::IsValid` in Bitcoin Core.
    #[inline]
    pub const fn is_valid(self, up_to: u32) -> bool {
        if self.0 & Self::FAILED_VALID != 0 {
            return false;
        }
        self.validity() >= up_to
    }

    /// Raise the validity level to `level` if the current level is lower and
    /// the block has not been marked as failed.
    ///
    /// Returns `true` if the level was actually changed.
    ///
    /// Mirrors `CBlockIndex::RaiseValidity` in Bitcoin Core.
    #[inline]
    pub fn raise_validity(&mut self, level: u32) -> bool {
        debug_assert!(
            level & !Self::VALID_MASK == 0,
            "only validity flags allowed"
        );
        if self.0 & Self::FAILED_VALID != 0 {
            return false;
        }
        if self.validity() < level {
            self.0 = (self.0 & !Self::VALID_MASK) | level;
            return true;
        }
        false
    }

    // -- Convenience boolean queries ----------------------------------------

    /// Full block data available on disk.
    #[inline]
    pub const fn has_data(self) -> bool {
        self.0 & Self::HAVE_DATA != 0
    }

    /// Undo data available on disk.
    #[inline]
    pub const fn has_undo(self) -> bool {
        self.0 & Self::HAVE_UNDO != 0
    }

    /// Block (or an ancestor) has been marked as failed.
    #[inline]
    pub const fn has_failed(self) -> bool {
        self.0 & Self::FAILED_MASK != 0
    }

    /// Block itself was marked as invalid (FAILED_VALID set).
    #[inline]
    pub const fn is_invalid(self) -> bool {
        self.0 & Self::FAILED_VALID != 0
    }

    /// Witness data was received for this block.
    #[inline]
    pub const fn has_opt_witness(self) -> bool {
        self.0 & Self::OPT_WITNESS != 0
    }

    // -- Bitwise helpers ----------------------------------------------------

    /// Set a flag.
    #[inline]
    pub fn insert(&mut self, flag: u32) {
        self.0 |= flag;
    }

    /// Clear a flag.
    #[inline]
    pub fn remove(&mut self, flag: u32) {
        self.0 &= !flag;
    }

    /// Test whether a flag is set.
    #[inline]
    pub const fn contains(self, flag: u32) -> bool {
        self.0 & flag == flag
    }
}

impl fmt::Debug for BlockStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BlockStatus(0x{:04x})", self.0)
    }
}

impl fmt::Display for BlockStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{:04x}", self.0)
    }
}

impl From<u32> for BlockStatus {
    fn from(bits: u32) -> Self {
        BlockStatus(bits)
    }
}

impl From<BlockStatus> for u32 {
    fn from(status: BlockStatus) -> u32 {
        status.0
    }
}

// ---------------------------------------------------------------------------
// FlatFilePos (minimal helper, mirrors flatfile.h)
// ---------------------------------------------------------------------------

/// Position of data within a flat file pair (`blk*.dat` / `rev*.dat`).
///
/// A minimal port of `FlatFilePos` from Bitcoin Core.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct FlatFilePos {
    /// File number (`blk?????.dat` / `rev?????.dat`).
    pub file: i32,
    /// Byte offset within the file.
    pub pos: u32,
}

impl FlatFilePos {
    /// A sentinel value representing an invalid / absent position.
    pub const NULL: FlatFilePos = FlatFilePos { file: -1, pos: 0 };

    /// A null position indicates the data is not available.
    pub fn is_null(&self) -> bool {
        self.file == -1
    }
}

// ---------------------------------------------------------------------------
// BlockIndex
// ---------------------------------------------------------------------------

/// In-memory representation of a block's metadata.
///
/// Stores the block header fields, chain-state bookkeeping (height, work,
/// status, file positions) and an optional link to the parent index entry.
///
/// This type uses arena-style indexing (`Option<usize>`) instead of raw
/// pointers for parent/skip links, since Rust does not permit the kind of
/// self-referential pointer graphs used in Bitcoin Core's `CBlockIndex`.
///
/// Maps to: `CBlockIndex` in Bitcoin Core's `src/chain.h`.
pub struct BlockIndex {
    // -- Block hash (owned) -------------------------------------------------
    /// The block hash. In Bitcoin Core this is an externally-allocated
    /// `uint256*`; here we own it directly.
    pub block_hash: BlockHash,

    // -- Block header fields ------------------------------------------------
    /// Block version information (signals soft-fork support).
    pub version: i32,
    /// Hash of the previous block header.
    pub prev_blockhash: BlockHash,
    /// Merkle root of the transactions in the block.
    pub merkle_root: Uint256,
    /// Block timestamp (seconds since Unix epoch).
    pub time: u32,
    /// Compact difficulty target (`nBits`).
    pub bits: u32,
    /// Proof-of-work nonce.
    pub nonce: u32,

    // -- Chain state --------------------------------------------------------
    /// Height of this entry in the chain. The genesis block has height 0.
    pub height: i32,
    /// Total amount of work (expected number of hashes) in the chain up to
    /// and including this block. Memory only.
    pub chain_work: ArithUint256,
    /// Number of transactions in this block (`nTx`). Non-zero once the block
    /// reaches `VALID_TRANSACTIONS`.
    pub tx_count: u32,
    /// Number of transactions in the chain up to and including this block.
    /// Non-zero if this block and all ancestors back to genesis (or an
    /// assumeutxo snapshot block) have reached `VALID_TRANSACTIONS`.
    /// Memory only.
    pub chain_tx_count: u64,
    /// Verification status of this block.
    pub status: BlockStatus,

    // -- File position (guarded by cs_main in Bitcoin Core) -----------------
    /// Which `blk?????.dat` file this block is stored in.
    pub file: i32,
    /// Byte offset within `blk?????.dat` where this block's data is stored.
    pub data_pos: u32,
    /// Byte offset within `rev?????.dat` where this block's undo data is stored.
    pub undo_pos: u32,

    // -- Sequencing (memory only) -------------------------------------------
    /// Sequential id assigned to distinguish the order in which blocks are
    /// received.
    pub sequence_id: i32,
    /// Maximum `nTime` in the chain up to and including this block. Memory only.
    pub time_max: u32,

    // -- Links (arena indices) ----------------------------------------------
    /// Index of the parent block in an external arena / `Vec<BlockIndex>`.
    pub prev: Option<usize>,
    /// Index of a further ancestor used for the skip-list optimization.
    pub skip: Option<usize>,
}

impl BlockIndex {
    // -- Constructors -------------------------------------------------------

    /// Create a new `BlockIndex` from a block header and a height.
    ///
    /// The block hash is computed from the header. All chain-state fields are
    /// initialised to their defaults.
    pub fn from_header(header: &BlockHeader, height: i32) -> Self {
        BlockIndex {
            block_hash: header.block_hash(),
            version: header.version,
            prev_blockhash: header.prev_blockhash,
            merkle_root: header.merkle_root,
            time: header.time,
            bits: header.bits,
            nonce: header.nonce,
            height,
            chain_work: ArithUint256::zero(),
            tx_count: 0,
            chain_tx_count: 0,
            status: BlockStatus::default(),
            file: 0,
            data_pos: 0,
            undo_pos: 0,
            sequence_id: SEQ_ID_INIT_FROM_DISK,
            time_max: 0,
            prev: None,
            skip: None,
        }
    }

    /// Create a default (null) `BlockIndex`.
    pub fn new() -> Self {
        BlockIndex {
            block_hash: BlockHash::ZERO,
            version: 0,
            prev_blockhash: BlockHash::ZERO,
            merkle_root: Uint256::ZERO,
            time: 0,
            bits: 0,
            nonce: 0,
            height: 0,
            chain_work: ArithUint256::zero(),
            tx_count: 0,
            chain_tx_count: 0,
            status: BlockStatus::default(),
            file: 0,
            data_pos: 0,
            undo_pos: 0,
            sequence_id: SEQ_ID_INIT_FROM_DISK,
            time_max: 0,
            prev: None,
            skip: None,
        }
    }

    // -- Header reconstruction ----------------------------------------------

    /// Reconstruct a [`BlockHeader`] from this index entry.
    ///
    /// Note: `prev_blockhash` is taken from the stored field. In Bitcoin Core
    /// this is derived from `pprev->GetBlockHash()`, but since we store the
    /// previous block hash directly in the header fields, they are equivalent.
    pub fn get_block_header(&self) -> BlockHeader {
        BlockHeader {
            version: self.version,
            prev_blockhash: self.prev_blockhash,
            merkle_root: self.merkle_root,
            time: self.time,
            bits: self.bits,
            nonce: self.nonce,
        }
    }

    // -- Hash ---------------------------------------------------------------

    /// Return a reference to the block hash.
    #[inline]
    pub fn get_block_hash(&self) -> &BlockHash {
        &self.block_hash
    }

    // -- Time ---------------------------------------------------------------

    /// Return the block timestamp as `i64`.
    #[inline]
    pub fn get_block_time(&self) -> i64 {
        self.time as i64
    }

    /// Return the maximum timestamp in the chain up to this block as `i64`.
    #[inline]
    pub fn get_block_time_max(&self) -> i64 {
        self.time_max as i64
    }

    /// Compute the median time past using the provided ancestor chain.
    ///
    /// `ancestors` should contain up to [`MEDIAN_TIME_SPAN`] block references
    /// ending with `self`, ordered from newest to oldest (i.e. `ancestors[0]`
    /// is `self`, `ancestors[1]` is the parent, etc.).
    ///
    /// If you have a flat arena of `BlockIndex` entries, you can build the
    /// ancestor slice by following `prev` links.
    ///
    /// This is a free function rather than a method because navigating the
    /// `prev` links requires access to the arena, which is external to this
    /// struct.
    pub fn get_median_time_past(ancestors: &[&BlockIndex]) -> i64 {
        let count = ancestors.len().min(MEDIAN_TIME_SPAN);
        if count == 0 {
            return 0;
        }
        let mut times: Vec<i64> = ancestors[..count]
            .iter()
            .map(|bi| bi.get_block_time())
            .collect();
        times.sort_unstable();
        times[times.len() / 2]
    }

    // -- Tx chain count -----------------------------------------------------

    /// Returns `true` if this block and all previous blocks back to the
    /// genesis block (or an assumeutxo snapshot block) have had their
    /// transactions downloaded.
    #[inline]
    pub fn have_num_chain_txs(&self) -> bool {
        self.chain_tx_count != 0
    }

    // -- Validity -----------------------------------------------------------

    /// Check whether this block index entry is valid up to the given level.
    ///
    /// Mirrors `CBlockIndex::IsValid`.
    #[inline]
    pub fn is_valid(&self, up_to: u32) -> bool {
        self.status.is_valid(up_to)
    }

    /// Raise the validity level of this block index entry. Returns `true` if
    /// the validity was actually changed.
    ///
    /// Mirrors `CBlockIndex::RaiseValidity`.
    #[inline]
    pub fn raise_validity(&mut self, up_to: u32) -> bool {
        self.status.raise_validity(up_to)
    }

    // -- File positions -----------------------------------------------------

    /// Return the position of the block data on disk, or `None` if not
    /// available.
    pub fn get_block_pos(&self) -> Option<FlatFilePos> {
        if self.status.has_data() {
            Some(FlatFilePos {
                file: self.file,
                pos: self.data_pos,
            })
        } else {
            None
        }
    }

    /// Return the position of the undo data on disk, or `None` if not
    /// available.
    pub fn get_undo_pos(&self) -> Option<FlatFilePos> {
        if self.status.has_undo() {
            Some(FlatFilePos {
                file: self.file,
                pos: self.undo_pos,
            })
        } else {
            None
        }
    }
}

impl Default for BlockIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for BlockIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BlockIndex")
            .field("hash", &self.block_hash.to_hex())
            .field("height", &self.height)
            .field("status", &self.status)
            .finish()
    }
}

impl fmt::Display for BlockIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "BlockIndex(hash={}, height={}, status={})",
            self.block_hash.to_hex(),
            self.height,
            self.status,
        )
    }
}

// ---------------------------------------------------------------------------
// DiskBlockIndex
// ---------------------------------------------------------------------------

/// On-disk serializable form of a block index entry.
///
/// Maps to: `CDiskBlockIndex` in Bitcoin Core.
///
/// This bundles the fields that are persisted to the block-index LevelDB
/// database. It stores `hash_prev` explicitly (since the `prev` pointer is
/// meaningless on disk).
#[derive(Clone, Debug, Default)]
pub struct DiskBlockIndex {
    // Header fields
    /// Block version information.
    pub version: i32,
    /// Hash of the previous block header.
    pub hash_prev: BlockHash,
    /// Merkle root of the transactions in the block.
    pub merkle_root: Uint256,
    /// Block timestamp (seconds since Unix epoch).
    pub time: u32,
    /// Compact difficulty target (`nBits`).
    pub bits: u32,
    /// Proof-of-work nonce.
    pub nonce: u32,

    // Chain bookkeeping
    /// Height of this block in the chain.
    pub height: i32,
    /// Validation and storage status flags.
    pub status: BlockStatus,
    /// Number of transactions in this block.
    pub tx_count: u32,

    // File positions
    /// Which `blk?????.dat` file this block is stored in.
    pub file: i32,
    /// Byte offset of the block data within `blk?????.dat`.
    pub data_pos: u32,
    /// Byte offset of the undo data within `rev?????.dat`.
    pub undo_pos: u32,
}

impl DiskBlockIndex {
    /// Build a `DiskBlockIndex` from a live `BlockIndex`.
    pub fn from_block_index(index: &BlockIndex) -> Self {
        DiskBlockIndex {
            version: index.version,
            hash_prev: index.prev_blockhash,
            merkle_root: index.merkle_root,
            time: index.time,
            bits: index.bits,
            nonce: index.nonce,
            height: index.height,
            status: index.status,
            tx_count: index.tx_count,
            file: index.file,
            data_pos: index.data_pos,
            undo_pos: index.undo_pos,
        }
    }

    /// Construct the block hash by building a header and hashing it.
    pub fn construct_block_hash(&self) -> BlockHash {
        let header = BlockHeader {
            version: self.version,
            prev_blockhash: self.hash_prev,
            merkle_root: self.merkle_root,
            time: self.time,
            bits: self.bits,
            nonce: self.nonce,
        };
        header.block_hash()
    }
}

// ---------------------------------------------------------------------------
// Proof-of-work helpers
// ---------------------------------------------------------------------------

/// Compute the amount of work that an `nBits` compact target value represents.
///
/// Returns `(2^256) / (target + 1)`, which is the expected number of hashes
/// needed to satisfy the target.
///
/// Maps to: `GetBitsProof` in Bitcoin Core.
pub fn get_bits_proof(bits: u32) -> ArithUint256 {
    let mut target = ArithUint256::zero();
    let (negative, overflow) = target.set_compact(bits);
    if negative || overflow || target == ArithUint256::zero() {
        return ArithUint256::zero();
    }
    // We need to compute 2**256 / (target+1), but we can't represent 2**256
    // as it's too large for a uint256. However, as 2**256 is at least as
    // large as target+1, it is equal to ((2**256 - target - 1) / (target+1)) + 1,
    // or ~target / (target+1) + 1.
    (!target / (target + ArithUint256::from_u64(1))) + ArithUint256::from_u64(1)
}

/// Compute the amount of work that a block index entry's target represents.
///
/// Maps to: `GetBlockProof(const CBlockIndex&)` in Bitcoin Core.
#[inline]
pub fn get_block_proof(index: &BlockIndex) -> ArithUint256 {
    get_bits_proof(index.bits)
}

/// Compute the amount of work that a block header's target represents.
///
/// Maps to: `GetBlockProof(const CBlockHeader&)` in Bitcoin Core.
#[inline]
pub fn get_header_proof(header: &BlockHeader) -> ArithUint256 {
    get_bits_proof(header.bits)
}

// ---------------------------------------------------------------------------
// Chain
// ---------------------------------------------------------------------------

/// An in-memory indexed chain of blocks.
///
/// Stores a linear sequence of block-index handles (arena indices) ordered by
/// height. The first entry (index 0 in the internal `Vec`) corresponds to
/// height 0 (the genesis block).
///
/// Maps to: `CChain` in Bitcoin Core's `src/chain.h`.
pub struct Chain {
    /// Block-index arena indices ordered by height. `chain[0]` is the genesis
    /// block, `chain[height]` is the tip.
    chain: Vec<usize>,
}

impl Chain {
    /// Create an empty chain.
    pub fn new() -> Self {
        Chain { chain: Vec::new() }
    }

    /// Returns the arena index of the genesis block, or `None` if the chain
    /// is empty.
    #[inline]
    pub fn genesis(&self) -> Option<usize> {
        self.chain.first().copied()
    }

    /// Returns the arena index of the tip, or `None` if the chain is empty.
    #[inline]
    pub fn tip(&self) -> Option<usize> {
        self.chain.last().copied()
    }

    /// Returns the maximal height in the chain, or `-1` if the chain is empty.
    #[inline]
    pub fn height(&self) -> i32 {
        self.chain.len() as i32 - 1
    }

    /// Returns the arena index of the block at the given `height`, or `None`
    /// if the height is out of range.
    #[inline]
    pub fn get_block_index(&self, height: i32) -> Option<usize> {
        if height < 0 || height as usize >= self.chain.len() {
            None
        } else {
            Some(self.chain[height as usize])
        }
    }

    /// Check whether the given arena index is present in this chain at its
    /// expected height.
    ///
    /// The caller must supply the `height` associated with `index` because the
    /// `Chain` stores only arena indices.
    pub fn contains(&self, index: usize, height: i32) -> bool {
        self.get_block_index(height) == Some(index)
    }

    /// Return the arena index of the block following the one at `height`, or
    /// `None` if `height` is at or beyond the tip.
    #[inline]
    pub fn next(&self, height: i32) -> Option<usize> {
        self.get_block_index(height + 1)
    }

    /// Set / initialise the chain so that it ends at the given block.
    ///
    /// `entries` must be an iterator yielding `(height, arena_index)` pairs
    /// from the tip down to the genesis block (or to the point where the chain
    /// already matches). For convenience, this method truncates the chain to
    /// the given tip height and fills in the entries.
    ///
    /// The simple variant takes a single tip and its height and resizes the
    /// chain accordingly.
    pub fn set_tip(&mut self, index: usize, height: i32) {
        if height < 0 {
            self.chain.clear();
            return;
        }
        let h = height as usize;
        self.chain.resize(h + 1, 0);
        self.chain[h] = index;
    }

    /// Populate the chain from genesis to tip using a callback that resolves
    /// an arena index to its `(prev_index, height)`.
    ///
    /// This walks from `tip_index` backwards through `prev` links until it
    /// reaches an entry already present in the chain (or exhausts all
    /// ancestors). The callback `get_prev` should return `(prev_arena_index,
    /// height)` for a given arena index, or `None` if the block has no parent.
    pub fn set_tip_with<F>(&mut self, tip_index: usize, tip_height: i32, mut get_prev: F)
    where
        F: FnMut(usize) -> Option<(usize, i32)>,
    {
        if tip_height < 0 {
            self.chain.clear();
            return;
        }

        let new_len = (tip_height + 1) as usize;

        // Truncate if the new tip is lower.
        self.chain.resize(new_len, 0);

        // Walk backwards from the tip, filling in entries.
        let mut current = tip_index;
        let mut h = tip_height;
        loop {
            if h < 0 {
                break;
            }
            let hu = h as usize;
            // If the chain already has the correct entry at this height, stop.
            if hu < self.chain.len() && self.chain[hu] == current && h < tip_height {
                break;
            }
            self.chain[hu] = current;
            match get_prev(current) {
                Some((prev_idx, _prev_height)) => {
                    current = prev_idx;
                    h -= 1;
                }
                None => break,
            }
        }
    }

    /// Find the height of the last common block between this chain and another
    /// chain.
    ///
    /// Returns `None` if either chain is empty or they have no common prefix.
    pub fn find_fork(&self, other: &Chain) -> Option<i32> {
        let min_height = self.height().min(other.height());
        if min_height < 0 {
            return None;
        }
        for h in (0..=min_height as usize).rev() {
            if self.chain[h] == other.chain[h] {
                return Some(h as i32);
            }
        }
        None
    }

    /// Find the earliest block in this chain with a timestamp >= `min_time`
    /// and height >= `min_height`, given a lookup function.
    ///
    /// `get_time` maps an arena index to the block's `time_max` value.
    /// Returns the height of the first matching block, or `None`.
    pub fn find_earliest_at_least<F>(
        &self,
        min_time: i64,
        min_height: i32,
        get_time: F,
    ) -> Option<i32>
    where
        F: Fn(usize) -> i64,
    {
        let start = if min_height < 0 {
            0
        } else {
            min_height as usize
        };
        for h in start..self.chain.len() {
            if get_time(self.chain[h]) >= min_time {
                return Some(h as i32);
            }
        }
        None
    }

    /// Return the number of entries in the chain.
    #[inline]
    pub fn len(&self) -> usize {
        self.chain.len()
    }

    /// Check if the chain is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.chain.is_empty()
    }
}

impl Default for Chain {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for Chain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Chain")
            .field("height", &self.height())
            .field("tip", &self.tip())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Skip-list helpers
// ---------------------------------------------------------------------------

/// Compute the skip-list height for a given block height.
///
/// The skip pointer at height `h` points to the ancestor at height
/// `h - skip_height(h)`. This achieves O(log n) ancestor lookups.
///
/// Maps to: `static int InvertLowestOne(int n)` and the skip-list logic in
/// Bitcoin Core's `chain.cpp`.
fn invert_lowest_one(n: i32) -> i32 {
    n & (n - 1)
}

/// Compute the height of the block that the skip pointer for `height` should
/// point to.
///
/// Maps to: `static inline int GetSkipHeight(int height)` in Bitcoin Core.
pub fn get_skip_height(height: i32) -> i32 {
    if height < 2 {
        return 0;
    }
    // Determine which height to skip to. Uses a contiguous range of "1" bits
    // starting from the LSB except the lowest bit.
    if (height & 1) != 0 {
        invert_lowest_one(invert_lowest_one(height - 1)) + 1
    } else {
        invert_lowest_one(height)
    }
}

// ---------------------------------------------------------------------------
// Arena-based ancestor lookup
// ---------------------------------------------------------------------------

/// Find the ancestor of block at `arena_idx` at the given `target_height`
/// using skip-list pointers for O(log n) traversal.
///
/// Maps to: `CBlockIndex::GetAncestor()` in Bitcoin Core's `chain.cpp`.
pub fn get_ancestor(arena: &[BlockIndex], arena_idx: usize, target_height: i32) -> Option<usize> {
    let block = arena.get(arena_idx)?;
    if target_height > block.height || target_height < 0 {
        return None;
    }

    let mut walk_idx = arena_idx;
    let mut walk_height = block.height;

    while walk_height > target_height {
        let skip_height = get_skip_height(walk_height);
        let block = arena.get(walk_idx)?;

        if let Some(skip_idx) = block.skip {
            let skip_prev_height = get_skip_height(walk_height - 1);
            if skip_height == target_height
                || (skip_height > target_height
                    && !(skip_prev_height < skip_height - 2 && skip_prev_height >= target_height))
            {
                walk_idx = skip_idx;
                walk_height = skip_height;
                continue;
            }
        }

        // Fall back to following prev pointer.
        walk_idx = block.prev?;
        walk_height -= 1;
    }

    Some(walk_idx)
}

/// Collect up to `MEDIAN_TIME_SPAN` ancestor BlockIndex references starting
/// from `arena_idx`, following `prev` links.
///
/// Returns a vector suitable for passing to `BlockIndex::get_median_time_past()`.
pub fn collect_mtp_ancestors<'a>(arena: &'a [BlockIndex], arena_idx: usize) -> Vec<&'a BlockIndex> {
    let mut result = Vec::with_capacity(MEDIAN_TIME_SPAN);
    let mut idx = Some(arena_idx);
    for _ in 0..MEDIAN_TIME_SPAN {
        match idx {
            Some(i) => {
                if let Some(block) = arena.get(i) {
                    result.push(block);
                    idx = block.prev;
                } else {
                    break;
                }
            }
            None => break,
        }
    }
    result
}

/// Compute the median time past for the block at `arena_idx`.
pub fn compute_mtp(arena: &[BlockIndex], arena_idx: usize) -> i64 {
    let ancestors = collect_mtp_ancestors(arena, arena_idx);
    BlockIndex::get_median_time_past(&ancestors)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use qubitcoin_consensus::BlockHeader;
    use qubitcoin_primitives::{BlockHash, Uint256};

    // -- BlockStatus tests --------------------------------------------------

    #[test]
    fn test_block_status_default() {
        let status = BlockStatus::default();
        assert_eq!(status.bits(), 0);
        assert_eq!(status.validity(), BlockStatus::VALID_UNKNOWN);
        assert!(!status.has_data());
        assert!(!status.has_undo());
        assert!(!status.has_failed());
        assert!(!status.is_invalid());
    }

    #[test]
    fn test_block_status_validity_levels() {
        let mut status = BlockStatus::new(0);

        // Raise from UNKNOWN to TREE
        assert!(status.raise_validity(BlockStatus::VALID_TREE));
        assert_eq!(status.validity(), BlockStatus::VALID_TREE);
        assert!(status.is_valid(BlockStatus::VALID_TREE));
        assert!(!status.is_valid(BlockStatus::VALID_TRANSACTIONS));

        // Raise from TREE to TRANSACTIONS
        assert!(status.raise_validity(BlockStatus::VALID_TRANSACTIONS));
        assert_eq!(status.validity(), BlockStatus::VALID_TRANSACTIONS);
        assert!(status.is_valid(BlockStatus::VALID_TREE));
        assert!(status.is_valid(BlockStatus::VALID_TRANSACTIONS));

        // Raising to a lower level should be a no-op
        assert!(!status.raise_validity(BlockStatus::VALID_TREE));
        assert_eq!(status.validity(), BlockStatus::VALID_TRANSACTIONS);

        // Raise to SCRIPTS
        assert!(status.raise_validity(BlockStatus::VALID_SCRIPTS));
        assert_eq!(status.validity(), BlockStatus::VALID_SCRIPTS);
        assert!(status.is_valid(BlockStatus::VALID_CHAIN));
        assert!(status.is_valid(BlockStatus::VALID_SCRIPTS));
    }

    #[test]
    fn test_block_status_validity_preserves_flags() {
        // Start with HAVE_DATA | VALID_TREE
        let mut status = BlockStatus::new(BlockStatus::HAVE_DATA | BlockStatus::VALID_TREE);
        assert!(status.has_data());
        assert_eq!(status.validity(), BlockStatus::VALID_TREE);

        // Raise validity should preserve HAVE_DATA
        status.raise_validity(BlockStatus::VALID_SCRIPTS);
        assert!(status.has_data());
        assert_eq!(status.validity(), BlockStatus::VALID_SCRIPTS);
        assert_eq!(
            status.bits(),
            BlockStatus::HAVE_DATA | BlockStatus::VALID_SCRIPTS
        );
    }

    #[test]
    fn test_block_status_failed_blocks_validity() {
        let mut status =
            BlockStatus::new(BlockStatus::VALID_TRANSACTIONS | BlockStatus::FAILED_VALID);
        assert!(status.is_invalid());
        assert!(status.has_failed());
        // is_valid should return false for failed blocks
        assert!(!status.is_valid(BlockStatus::VALID_UNKNOWN));

        // raise_validity should return false for failed blocks
        assert!(!status.raise_validity(BlockStatus::VALID_SCRIPTS));
    }

    #[test]
    fn test_block_status_flag_operations() {
        let mut status = BlockStatus::new(0);

        status.insert(BlockStatus::HAVE_DATA);
        assert!(status.has_data());
        assert!(!status.has_undo());

        status.insert(BlockStatus::HAVE_UNDO);
        assert!(status.has_undo());
        assert!(status.contains(BlockStatus::HAVE_MASK));

        status.insert(BlockStatus::OPT_WITNESS);
        assert!(status.has_opt_witness());

        status.remove(BlockStatus::HAVE_DATA);
        assert!(!status.has_data());
        assert!(status.has_undo());
    }

    #[test]
    fn test_block_status_from_u32() {
        // 0x8d = 0b1000_1101 = OPT_WITNESS(128) | HAVE_DATA(8) | VALID_SCRIPTS(5)
        let status = BlockStatus::from(0x8du32);
        assert_eq!(status.validity(), BlockStatus::VALID_SCRIPTS);
        assert!(status.has_data());
        assert!(status.has_opt_witness());
        assert!(!status.has_undo());
        assert!(!status.has_failed());

        // 0x85 = 0b1000_0101 = OPT_WITNESS(128) | VALID_SCRIPTS(5) -- no HAVE_DATA
        let status2 = BlockStatus::from(0x85u32);
        assert_eq!(status2.validity(), BlockStatus::VALID_SCRIPTS);
        assert!(!status2.has_data());
        assert!(status2.has_opt_witness());
    }

    #[test]
    fn test_block_status_display() {
        let status = BlockStatus::new(0x85);
        assert_eq!(format!("{}", status), "0x0085");
        assert_eq!(format!("{:?}", status), "BlockStatus(0x0085)");
    }

    // -- BlockIndex tests ---------------------------------------------------

    fn make_test_header(time: u32, nonce: u32) -> BlockHeader {
        BlockHeader {
            version: 0x20000000,
            prev_blockhash: BlockHash::ZERO,
            merkle_root: Uint256::ZERO,
            time,
            bits: 0x1d00ffff,
            nonce,
        }
    }

    #[test]
    fn test_block_index_from_header() {
        let header = make_test_header(1700000000, 42);
        let index = BlockIndex::from_header(&header, 100);

        assert_eq!(index.height, 100);
        assert_eq!(index.version, 0x20000000);
        assert_eq!(index.time, 1700000000);
        assert_eq!(index.bits, 0x1d00ffff);
        assert_eq!(index.nonce, 42);
        assert_eq!(index.prev_blockhash, BlockHash::ZERO);
        assert_eq!(index.merkle_root, Uint256::ZERO);
        assert_eq!(index.tx_count, 0);
        assert_eq!(index.chain_tx_count, 0);
        assert_eq!(index.status, BlockStatus::default());
        assert!(index.prev.is_none());
        assert!(index.skip.is_none());
        assert_eq!(index.sequence_id, SEQ_ID_INIT_FROM_DISK);

        // The block hash should match what the header computes
        assert_eq!(index.block_hash, header.block_hash());
    }

    #[test]
    fn test_block_index_header_roundtrip() {
        let header = make_test_header(1700000000, 42);
        let index = BlockIndex::from_header(&header, 0);
        let reconstructed = index.get_block_header();

        assert_eq!(reconstructed.version, header.version);
        assert_eq!(reconstructed.prev_blockhash, header.prev_blockhash);
        assert_eq!(reconstructed.merkle_root, header.merkle_root);
        assert_eq!(reconstructed.time, header.time);
        assert_eq!(reconstructed.bits, header.bits);
        assert_eq!(reconstructed.nonce, header.nonce);
    }

    #[test]
    fn test_block_index_get_block_hash() {
        let header = make_test_header(1700000000, 99);
        let index = BlockIndex::from_header(&header, 5);
        assert_eq!(*index.get_block_hash(), header.block_hash());
    }

    #[test]
    fn test_block_index_validity() {
        let mut index = BlockIndex::new();
        assert!(index.is_valid(BlockStatus::VALID_UNKNOWN));
        assert!(!index.is_valid(BlockStatus::VALID_TREE));

        assert!(index.raise_validity(BlockStatus::VALID_TREE));
        assert!(index.is_valid(BlockStatus::VALID_TREE));

        assert!(index.raise_validity(BlockStatus::VALID_SCRIPTS));
        assert!(index.is_valid(BlockStatus::VALID_SCRIPTS));

        // Mark as failed
        index.status.insert(BlockStatus::FAILED_VALID);
        assert!(!index.is_valid(BlockStatus::VALID_UNKNOWN));
        assert!(!index.raise_validity(BlockStatus::VALID_SCRIPTS));
    }

    #[test]
    fn test_block_index_file_positions() {
        let mut index = BlockIndex::new();

        // No data/undo yet
        assert!(index.get_block_pos().is_none());
        assert!(index.get_undo_pos().is_none());

        // Set data position
        index.file = 3;
        index.data_pos = 12345;
        index.status.insert(BlockStatus::HAVE_DATA);
        let pos = index.get_block_pos().unwrap();
        assert_eq!(pos.file, 3);
        assert_eq!(pos.pos, 12345);
        assert!(index.get_undo_pos().is_none());

        // Set undo position
        index.undo_pos = 67890;
        index.status.insert(BlockStatus::HAVE_UNDO);
        let upos = index.get_undo_pos().unwrap();
        assert_eq!(upos.file, 3);
        assert_eq!(upos.pos, 67890);
    }

    #[test]
    fn test_block_index_have_num_chain_txs() {
        let mut index = BlockIndex::new();
        assert!(!index.have_num_chain_txs());
        index.chain_tx_count = 1;
        assert!(index.have_num_chain_txs());
    }

    #[test]
    fn test_block_index_display() {
        let header = make_test_header(1700000000, 42);
        let index = BlockIndex::from_header(&header, 100);
        let s = format!("{}", index);
        assert!(s.starts_with("BlockIndex(hash="));
        assert!(s.contains("height=100"));
    }

    #[test]
    fn test_block_index_median_time_past() {
        // Create 11 block indices with known times
        let mut blocks: Vec<BlockIndex> = Vec::new();
        for i in 0..11u32 {
            let mut bi = BlockIndex::new();
            bi.time = 1000 + i * 100; // 1000, 1100, 1200, ..., 2000
            blocks.push(bi);
        }

        // Build the ancestor slice (newest first)
        let ancestors: Vec<&BlockIndex> = blocks.iter().rev().collect();
        let median = BlockIndex::get_median_time_past(&ancestors);
        // Times sorted: 1000..2000, median at index 5 = 1500
        assert_eq!(median, 1500);
    }

    #[test]
    fn test_block_index_median_time_past_fewer_blocks() {
        let mut blocks: Vec<BlockIndex> = Vec::new();
        for i in 0..3u32 {
            let mut bi = BlockIndex::new();
            bi.time = 100 + i * 50; // 100, 150, 200
            blocks.push(bi);
        }
        let ancestors: Vec<&BlockIndex> = blocks.iter().rev().collect();
        let median = BlockIndex::get_median_time_past(&ancestors);
        // Sorted: [100, 150, 200], median at index 1 = 150
        assert_eq!(median, 150);
    }

    #[test]
    fn test_block_index_median_time_past_empty() {
        let ancestors: Vec<&BlockIndex> = Vec::new();
        let median = BlockIndex::get_median_time_past(&ancestors);
        assert_eq!(median, 0);
    }

    // -- DiskBlockIndex tests -----------------------------------------------

    #[test]
    fn test_disk_block_index_from_block_index() {
        let header = make_test_header(1700000000, 42);
        let mut index = BlockIndex::from_header(&header, 100);
        index.file = 5;
        index.data_pos = 999;
        index.undo_pos = 888;
        index.tx_count = 50;
        index.status.insert(BlockStatus::HAVE_DATA);
        index.status.raise_validity(BlockStatus::VALID_TRANSACTIONS);

        let disk = DiskBlockIndex::from_block_index(&index);
        assert_eq!(disk.version, index.version);
        assert_eq!(disk.hash_prev, index.prev_blockhash);
        assert_eq!(disk.merkle_root, index.merkle_root);
        assert_eq!(disk.time, index.time);
        assert_eq!(disk.bits, index.bits);
        assert_eq!(disk.nonce, index.nonce);
        assert_eq!(disk.height, 100);
        assert_eq!(disk.tx_count, 50);
        assert_eq!(disk.file, 5);
        assert_eq!(disk.data_pos, 999);
        assert_eq!(disk.undo_pos, 888);
    }

    #[test]
    fn test_disk_block_index_construct_hash() {
        let header = make_test_header(1700000000, 42);
        let index = BlockIndex::from_header(&header, 0);
        let disk = DiskBlockIndex::from_block_index(&index);
        assert_eq!(disk.construct_block_hash(), header.block_hash());
    }

    // -- Chain tests --------------------------------------------------------

    #[test]
    fn test_chain_new_empty() {
        let chain = Chain::new();
        assert!(chain.is_empty());
        assert_eq!(chain.height(), -1);
        assert_eq!(chain.tip(), None);
        assert_eq!(chain.genesis(), None);
        assert_eq!(chain.len(), 0);
    }

    #[test]
    fn test_chain_set_tip() {
        let mut chain = Chain::new();

        // Set genesis
        chain.set_tip(100, 0); // arena index 100 at height 0
        assert_eq!(chain.height(), 0);
        assert_eq!(chain.tip(), Some(100));
        assert_eq!(chain.genesis(), Some(100));
        assert_eq!(chain.len(), 1);

        // Extend to height 2
        chain.set_tip(101, 1);
        chain.set_tip(102, 2);
        assert_eq!(chain.height(), 2);
        assert_eq!(chain.tip(), Some(102));
        assert_eq!(chain.genesis(), Some(100));

        // Verify block at each height
        assert_eq!(chain.get_block_index(0), Some(100));
        assert_eq!(chain.get_block_index(1), Some(101));
        assert_eq!(chain.get_block_index(2), Some(102));
        assert_eq!(chain.get_block_index(3), None);
        assert_eq!(chain.get_block_index(-1), None);
    }

    #[test]
    fn test_chain_contains() {
        let mut chain = Chain::new();
        chain.set_tip(10, 0);
        chain.set_tip(20, 1);
        chain.set_tip(30, 2);

        assert!(chain.contains(10, 0));
        assert!(chain.contains(20, 1));
        assert!(chain.contains(30, 2));
        assert!(!chain.contains(10, 1)); // wrong height
        assert!(!chain.contains(99, 0)); // wrong index
        assert!(!chain.contains(30, 5)); // out of range
    }

    #[test]
    fn test_chain_next() {
        let mut chain = Chain::new();
        chain.set_tip(10, 0);
        chain.set_tip(20, 1);
        chain.set_tip(30, 2);

        assert_eq!(chain.next(0), Some(20));
        assert_eq!(chain.next(1), Some(30));
        assert_eq!(chain.next(2), None); // at tip
        assert_eq!(chain.next(-1), Some(10)); // before genesis
    }

    #[test]
    fn test_chain_find_fork() {
        let mut chain_a = Chain::new();
        chain_a.set_tip(10, 0);
        chain_a.set_tip(20, 1);
        chain_a.set_tip(30, 2);
        chain_a.set_tip(40, 3);

        // Chain B forks at height 1
        let mut chain_b = Chain::new();
        chain_b.set_tip(10, 0);
        chain_b.set_tip(20, 1);
        chain_b.set_tip(50, 2); // different block at height 2
        chain_b.set_tip(60, 3);

        assert_eq!(chain_a.find_fork(&chain_b), Some(1));
    }

    #[test]
    fn test_chain_find_fork_same_chain() {
        let mut chain_a = Chain::new();
        chain_a.set_tip(10, 0);
        chain_a.set_tip(20, 1);

        let mut chain_b = Chain::new();
        chain_b.set_tip(10, 0);
        chain_b.set_tip(20, 1);

        assert_eq!(chain_a.find_fork(&chain_b), Some(1));
    }

    #[test]
    fn test_chain_find_fork_no_common() {
        let mut chain_a = Chain::new();
        chain_a.set_tip(10, 0);

        let mut chain_b = Chain::new();
        chain_b.set_tip(99, 0); // different genesis

        assert_eq!(chain_a.find_fork(&chain_b), None);
    }

    #[test]
    fn test_chain_find_fork_empty() {
        let chain_a = Chain::new();
        let chain_b = Chain::new();
        assert_eq!(chain_a.find_fork(&chain_b), None);
    }

    #[test]
    fn test_chain_set_tip_with() {
        // Simulate an arena of block indices
        // Arena: [genesis(h=0), block1(h=1), block2(h=2)]
        // prev links: block2 -> block1 -> genesis -> None
        struct FakeArena {
            prev: Vec<Option<usize>>,
            height: Vec<i32>,
        }
        let arena = FakeArena {
            prev: vec![None, Some(0), Some(1)],
            height: vec![0, 1, 2],
        };

        let mut chain = Chain::new();
        chain.set_tip_with(2, 2, |idx| arena.prev[idx].map(|p| (p, arena.height[p])));

        assert_eq!(chain.height(), 2);
        assert_eq!(chain.get_block_index(0), Some(0));
        assert_eq!(chain.get_block_index(1), Some(1));
        assert_eq!(chain.get_block_index(2), Some(2));
    }

    #[test]
    fn test_chain_find_earliest_at_least() {
        let mut chain = Chain::new();
        // times: [100, 200, 300, 400, 500]
        for i in 0..5 {
            chain.set_tip(i, i as i32);
        }

        let times = [100i64, 200, 300, 400, 500];
        let get_time = |idx: usize| -> i64 { times[idx] };

        assert_eq!(chain.find_earliest_at_least(250, 0, &get_time), Some(2));
        assert_eq!(chain.find_earliest_at_least(100, 0, &get_time), Some(0));
        assert_eq!(chain.find_earliest_at_least(500, 0, &get_time), Some(4));
        assert_eq!(chain.find_earliest_at_least(600, 0, &get_time), None);
        assert_eq!(chain.find_earliest_at_least(100, 3, &get_time), Some(3));
    }

    // -- Proof-of-work tests ------------------------------------------------

    #[test]
    fn test_get_bits_proof_genesis() {
        // Genesis block nBits = 0x1d00ffff
        let proof = get_bits_proof(0x1d00ffff);
        // Should be non-zero
        assert!(proof > ArithUint256::zero());
    }

    #[test]
    fn test_get_bits_proof_zero() {
        assert_eq!(get_bits_proof(0), ArithUint256::zero());
    }

    #[test]
    fn test_get_block_proof() {
        let header = make_test_header(1700000000, 42);
        let index = BlockIndex::from_header(&header, 0);
        let proof = get_block_proof(&index);
        let header_proof = get_header_proof(&header);
        // Both should produce the same result
        assert_eq!(proof, header_proof);
    }

    // -- Skip-list tests ----------------------------------------------------

    #[test]
    fn test_get_skip_height() {
        assert_eq!(get_skip_height(0), 0);
        assert_eq!(get_skip_height(1), 0);
        assert_eq!(get_skip_height(2), 0);
        assert_eq!(get_skip_height(3), 1);
        assert_eq!(get_skip_height(4), 0);
        assert_eq!(get_skip_height(5), 1);
        // Verify skip heights are always less than the input
        for h in 2..1000 {
            let skip = get_skip_height(h);
            assert!(
                skip < h,
                "get_skip_height({}) = {} should be < {}",
                h,
                skip,
                h
            );
        }
    }

    // -- FlatFilePos tests --------------------------------------------------

    #[test]
    fn test_flat_file_pos() {
        let null = FlatFilePos::NULL;
        assert!(null.is_null());

        let pos = FlatFilePos { file: 5, pos: 1234 };
        assert!(!pos.is_null());
    }
}
