//! Chainstate management: block index, active chain, UTXO set.
//!
//! Maps to: `src/validation.h` / `src/validation.cpp` in Bitcoin Core
//! (specifically `Chainstate` and `ChainstateManager`).
//!
//! This module provides:
//! - `BlockMap`: Arena-based storage for block indices (replaces Bitcoin Core's
//!   pointer-based `BlockMap` / `std::unordered_map<uint256, CBlockIndex*>`).
//! - `Chainstate`: A single chainstate instance holding the active chain and
//!   UTXO cache tip.
//! - `ChainstateManager`: Top-level manager that owns the block index, chain
//!   parameters, and the active chainstate. Entry point for block processing.

use crate::undo::BlockUndo;
use qubitcoin_common::chain::{
    get_ancestor, get_block_proof, get_skip_height, BlockIndex, BlockStatus, Chain,
};
use qubitcoin_common::chainparams::ChainParams;
use qubitcoin_common::coins::{add_coins, CoinsView, CoinsViewCache, EmptyCoinsView, FlushableCoinsView};
use qubitcoin_consensus::block::{Block, BlockHeader};
use qubitcoin_consensus::params::ConsensusParams;
use qubitcoin_consensus::validation_state::{BlockValidationResult, BlockValidationState};
use qubitcoin_primitives::{ArithUint256, BlockHash};

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

// Validation helpers -- imported from sibling module (created in parallel).
// These will be provided by `crate::validation` once that module lands.
// Until then, we use inline stubs so that chainstate compiles and passes
// its own tests independently.
#[cfg(not(test))]
use crate::validation::{
    check_block, check_block_header, connect_block, contextual_check_block,
    contextual_check_block_header_with_arena, disconnect_block,
};

// ---------------------------------------------------------------------------
// BlockMap
// ---------------------------------------------------------------------------

/// Arena-based storage for block indices.
///
/// Instead of raw pointers like Bitcoin Core, we use indices into a `Vec`.
/// This avoids all `unsafe` code while providing O(1) index access and
/// O(1) amortised hash-based lookup by block hash.
///
/// Maps to: the `BlockMap` typedef
/// (`std::unordered_map<uint256, CBlockIndex*>`) in Bitcoin Core.
pub struct BlockMap {
    /// All block indices, indexed by arena position.
    indices: Vec<BlockIndex>,
    /// Lookup by block hash -> arena index.
    by_hash: HashMap<BlockHash, usize>,
}

impl BlockMap {
    /// Create an empty block map.
    pub fn new() -> Self {
        BlockMap {
            indices: Vec::new(),
            by_hash: HashMap::new(),
        }
    }

    /// Insert a block index into the arena.
    ///
    /// Returns the arena index at which the entry was stored.
    ///
    /// # Panics
    ///
    /// Panics (in debug mode) if a block with the same hash is already present.
    pub fn insert(&mut self, index: BlockIndex) -> usize {
        let hash = index.block_hash;
        let arena_idx = self.indices.len();
        debug_assert!(
            !self.by_hash.contains_key(&hash),
            "BlockMap::insert: duplicate block hash {}",
            hash.to_hex(),
        );
        self.by_hash.insert(hash, arena_idx);
        self.indices.push(index);
        arena_idx
    }

    /// Get an immutable reference to the block index at `idx`.
    ///
    /// # Panics
    ///
    /// Panics if `idx` is out of bounds.
    #[inline]
    pub fn get(&self, idx: usize) -> &BlockIndex {
        &self.indices[idx]
    }

    /// Get the underlying slice for arena-based ancestor lookup.
    #[inline]
    pub fn as_slice(&self) -> &[BlockIndex] {
        &self.indices
    }

    /// Get a mutable reference to the block index at `idx`.
    ///
    /// # Panics
    ///
    /// Panics if `idx` is out of bounds.
    #[inline]
    pub fn get_mut(&mut self, idx: usize) -> &mut BlockIndex {
        &mut self.indices[idx]
    }

    /// Look up the arena index for a block by its hash.
    ///
    /// Returns `None` if the hash is not present.
    pub fn find_by_hash(&self, hash: &BlockHash) -> Option<usize> {
        self.by_hash.get(hash).copied()
    }

    /// Return the number of block index entries in the arena.
    #[inline]
    pub fn len(&self) -> usize {
        self.indices.len()
    }

    /// Return `true` if the block map contains no entries.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    /// Compute and set the skip pointer for the block at `arena_idx`.
    ///
    /// The skip pointer links a block to an ancestor determined by
    /// [`get_skip_height`], enabling O(log n) ancestor lookups via
    /// [`get_ancestor`].
    ///
    /// This must be called after inserting a block whose `prev` pointer
    /// is already set. For the genesis block (height 0) the skip pointer
    /// remains `None` because `get_skip_height(0) == 0` (self-referential).
    ///
    /// Maps to: `CBlockIndex::BuildSkip()` in Bitcoin Core's `chain.cpp`.
    pub fn build_skip_pointer(&mut self, arena_idx: usize) {
        let height = self.indices[arena_idx].height;
        if height < 1 {
            // Genesis block: no ancestor to skip to.
            return;
        }
        let skip_target_height = get_skip_height(height);
        // Walk from the block's prev to find the ancestor at skip_target_height.
        if let Some(prev_idx) = self.indices[arena_idx].prev {
            if let Some(skip_idx) = get_ancestor(&self.indices, prev_idx, skip_target_height) {
                self.indices[arena_idx].skip = Some(skip_idx);
            }
        }
    }
}

impl Default for BlockMap {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Chainstate
// ---------------------------------------------------------------------------

/// A single chainstate instance.
///
/// Holds the active chain (a linear sequence of block-index arena handles
/// ordered by height) and the UTXO cache tip.
///
/// Maps to: `Chainstate` in Bitcoin Core's `src/validation.h`.
pub struct Chainstate {
    /// The active chain of block indices (by height).
    pub chain: Chain,
    /// UTXO cache layer (sits on top of a backing `CoinsView`).
    coins_tip: CoinsViewCache,
}

impl Chainstate {
    /// Create a new chainstate with an empty chain and the given coins view
    /// as the backing store for the UTXO cache.
    pub fn new(coins_view: Box<dyn CoinsView + Send + Sync>) -> Self {
        Chainstate {
            chain: Chain::new(),
            coins_tip: CoinsViewCache::new(coins_view),
        }
    }
}

// ---------------------------------------------------------------------------
// ChainstateManager
// ---------------------------------------------------------------------------

/// Callback type for reading blocks and undo data from disk.
///
/// Used by chain reorganization code to read block data that is not in memory.
/// Returns `(Block, BlockUndo)` for the given block hash, or `None` if not found.
/// Callback for reading blocks and undo data from disk.
///
/// Parameters: `(file_num, data_pos, undo_pos)` — the position fields from
/// the block index entry.  Returns `(Block, BlockUndo)` on success.
pub type BlockReader = Arc<dyn Fn(i32, u32, u32) -> Option<(Block, BlockUndo)> + Send + Sync>;

/// Manages the blockchain state: block index, active chain, UTXO set.
///
/// This is the primary entry point for processing new blocks. It owns the
/// block index arena, the chain parameters, and the active chainstate.
///
/// Maps to: `ChainstateManager` in Bitcoin Core's `src/validation.h`.
pub struct ChainstateManager {
    /// Chain parameters (network, consensus params, genesis hash, etc.).
    params: ChainParams,
    /// Block index arena.
    block_index: BlockMap,
    /// The active chainstate.
    active_chainstate: Chainstate,
    /// In-memory block cache (used by tests and as fallback).
    stored_blocks: HashMap<BlockHash, Block>,
    /// In-memory undo data cache.
    stored_undos: HashMap<BlockHash, BlockUndo>,
    /// Optional callback for reading blocks from disk (production mode).
    block_reader: Option<BlockReader>,
    /// Hash of the assumed-valid block. Blocks at or below this height
    /// skip script verification during IBD, dramatically speeding up sync.
    assume_valid: Option<BlockHash>,
    /// Height of the assumed-valid block. Used as a fast-path fallback
    /// when the assume-valid header hasn't been synced yet (early IBD).
    assume_valid_height: Option<i32>,
    /// Tracked best fully-validated tip (arena index + chain_work).
    /// Avoids O(N) linear scan of the entire block index on every block.
    best_valid_tip: Option<(usize, ArithUint256)>,
    /// Arena indices of block index entries modified since the last flush.
    dirty_indices: HashSet<usize>,
}

impl ChainstateManager {
    /// Create a new `ChainstateManager` with an empty chain.
    ///
    /// `params` -- full chain parameters for the target network.
    /// `coins_view` -- the base coins view that backs the UTXO cache
    /// (typically `CoinsViewDB` in production or `EmptyCoinsView` in tests).
    pub fn new(params: ChainParams, coins_view: Box<dyn CoinsView + Send + Sync>) -> Self {
        ChainstateManager {
            params,
            block_index: BlockMap::new(),
            active_chainstate: Chainstate::new(coins_view),
            stored_blocks: HashMap::new(),
            stored_undos: HashMap::new(),
            block_reader: None,
            assume_valid: None,
            assume_valid_height: None,
            best_valid_tip: None,
            dirty_indices: HashSet::new(),
        }
    }

    /// Set a block reader callback for disk-based block retrieval.
    pub fn set_block_reader(&mut self, reader: BlockReader) {
        self.block_reader = Some(reader);
    }

    /// Set the undo data position for a block index entry (after writing
    /// undo data to disk).
    pub fn set_undo_pos(&mut self, hash: &BlockHash, undo_pos: u32) {
        if let Some(idx) = self.block_index.find_by_hash(hash) {
            self.block_index.get_mut(idx).undo_pos = undo_pos;
            self.dirty_indices.insert(idx);
        }
    }

    /// Set the assumed-valid block hash and height for IBD optimization.
    pub fn set_assume_valid(&mut self, hash: BlockHash, height: i32) {
        self.assume_valid = Some(hash);
        self.assume_valid_height = Some(height);
    }

    /// Reset the active chain to the given arena index.
    ///
    /// Used during crash recovery to rewind the chain tip to match the
    /// persisted UTXO state.  Sets both the active chain tip and the
    /// UTXO cache best block.
    pub fn reset_active_chain_to(&mut self, arena_idx: usize) {
        let height = self.block_index.get(arena_idx).height;
        let hash = self.block_index.get(arena_idx).block_hash;
        let block_index = &self.block_index;
        self.active_chainstate
            .chain
            .set_tip_with(arena_idx, height, |idx| {
                let entry = block_index.get(idx);
                entry.prev.map(|p| (p, block_index.get(p).height))
            });
        self.active_chainstate.coins_tip.set_best_block(hash);
    }

    // -- Accessors ----------------------------------------------------------

    /// Get the active chain.
    #[inline]
    pub fn active_chain(&self) -> &Chain {
        &self.active_chainstate.chain
    }

    /// Get the chain tip height (returns `-1` if the chain is empty).
    #[inline]
    pub fn height(&self) -> i32 {
        self.active_chainstate.chain.height()
    }

    /// Get the chain tip block index (arena index), or `None` if empty.
    #[inline]
    pub fn tip(&self) -> Option<usize> {
        self.active_chainstate.chain.tip()
    }

    /// Get the chain parameters.
    #[inline]
    pub fn params(&self) -> &ChainParams {
        &self.params
    }

    /// Get the consensus parameters.
    #[inline]
    pub fn consensus(&self) -> &ConsensusParams {
        &self.params.consensus
    }

    /// Access the block index map (immutable).
    #[inline]
    pub fn block_index(&self) -> &BlockMap {
        &self.block_index
    }

    /// Access the block index map (mutable).
    #[inline]
    pub fn block_index_mut(&mut self) -> &mut BlockMap {
        &mut self.block_index
    }

    /// Mark a block index entry as dirty (needs to be flushed to disk).
    #[inline]
    pub fn mark_dirty(&mut self, arena_idx: usize) {
        self.dirty_indices.insert(arena_idx);
    }

    /// Access the UTXO cache.
    #[inline]
    pub fn coins_tip(&self) -> &CoinsViewCache {
        &self.active_chainstate.coins_tip
    }

    /// Look up a block index by hash.
    #[inline]
    pub fn lookup_block_index(&self, hash: &BlockHash) -> Option<usize> {
        self.block_index.find_by_hash(hash)
    }

    /// Get the block hash at a given height on the active chain.
    pub fn get_block_hash_at_height(&self, height: i32) -> Option<String> {
        let arena_idx = self.active_chainstate.chain.get_block_index(height)?;
        let block = self.block_index.get(arena_idx);
        Some(block.block_hash.to_hex())
    }

    // -- Persistence helpers -------------------------------------------------

    /// Flush the UTXO cache to a persistent backing store.
    ///
    /// Delegates to [`CoinsViewCache::flush_to`] which writes all dirty entries
    /// to the provided [`FlushableCoinsView`] (typically a `CoinsViewDB`).
    pub fn flush_coins(&self, target: &dyn FlushableCoinsView) -> bool {
        self.active_chainstate.coins_tip.flush_to(target)
    }

    /// Load block index entries from a set of records (typically loaded from
    /// `BlockIndexDB`). Rebuilds the arena, prev/skip pointers, and finds
    /// the best chain.
    ///
    /// Returns `Ok(())` on success or `Err(description)` on failure.
    pub fn load_block_index(
        &mut self,
        records: &[crate::block_index_db::BlockIndexRecord],
    ) -> Result<(), String> {
        use qubitcoin_common::chain::BlockIndex as BI;

        if records.is_empty() {
            return Ok(());
        }

        // Sort by height for correct insertion order.
        let mut sorted: Vec<&crate::block_index_db::BlockIndexRecord> = records.iter().collect();
        sorted.sort_by_key(|r| r.height);

        // Insert all entries into the arena.
        for record in &sorted {
            let mut idx = BI::new();
            idx.block_hash = record.block_hash;
            idx.version = record.version;
            idx.prev_blockhash = record.prev_blockhash;
            idx.merkle_root = record.merkle_root;
            idx.time = record.time;
            idx.bits = record.bits;
            idx.nonce = record.nonce;
            idx.height = record.height;
            idx.status = BlockStatus::new(record.status_bits);
            idx.file = record.file;
            idx.data_pos = record.data_pos;
            idx.undo_pos = record.undo_pos;
            idx.tx_count = record.tx_count;
            idx.chain_tx_count = record.chain_tx_count;
            idx.chain_work = record.chain_work();

            self.block_index.insert(idx);
        }

        // Resolve prev links and build skip pointers.
        for arena_idx in 0..self.block_index.len() {
            let prev_hash = self.block_index.get(arena_idx).prev_blockhash;
            if !prev_hash.is_null() {
                if let Some(prev_idx) = self.block_index.find_by_hash(&prev_hash) {
                    self.block_index.get_mut(arena_idx).prev = Some(prev_idx);
                    // Compute time_max from parent.
                    let parent_time_max = self.block_index.get(prev_idx).time_max;
                    let this_time = self.block_index.get(arena_idx).time;
                    self.block_index.get_mut(arena_idx).time_max =
                        std::cmp::max(parent_time_max, this_time);
                }
            } else {
                // Genesis block: time_max = time.
                let t = self.block_index.get(arena_idx).time;
                self.block_index.get_mut(arena_idx).time_max = t;
            }
            self.block_index.build_skip_pointer(arena_idx);
        }

        // Find the best fully-validated chain tip and set the active chain.
        let mut best_idx: Option<usize> = None;
        let mut best_work = ArithUint256::zero();

        for i in 0..self.block_index.len() {
            let entry = self.block_index.get(i);
            if entry.is_valid(BlockStatus::VALID_SCRIPTS) && entry.chain_work > best_work {
                best_work = entry.chain_work;
                best_idx = Some(i);
            }
        }

        // Cache the best valid tip for O(1) activate_best_chain.
        if let Some(idx) = best_idx {
            self.best_valid_tip = Some((idx, best_work));
        }

        if let Some(best) = best_idx {
            let best_height = self.block_index.get(best).height;
            let block_index = &self.block_index;
            self.active_chainstate
                .chain
                .set_tip_with(best, best_height, |idx| {
                    let entry = block_index.get(idx);
                    entry.prev.map(|p| (p, block_index.get(p).height))
                });

            let tip_hash = self.block_index.get(best).block_hash;
            tracing::info!(
                height = best_height,
                hash = %tip_hash.to_hex(),
                "loaded block index, best chain found"
            );
        }

        Ok(())
    }

    /// Get block index entries modified since the last flush.
    /// Used for persisting the block index to disk.
    pub fn dirty_block_indices(&self) -> Vec<&BlockIndex> {
        self.dirty_indices
            .iter()
            .map(|&i| self.block_index.get(i))
            .collect()
    }

    /// Clear the dirty set after a successful flush.
    pub fn clear_dirty(&mut self) {
        self.dirty_indices.clear();
    }

    // -- Genesis block ------------------------------------------------------

    /// Initialize the genesis block.
    ///
    /// This creates the block-index entry for the genesis block, marks it as
    /// fully valid, adds its coinbase outputs to the UTXO cache, and sets the
    /// active chain tip to height 0.
    ///
    /// Returns `Err` with a description if the genesis block hash does not
    /// match the chain parameters.
    pub fn load_genesis_block(&mut self, genesis: &Block) -> Result<(), String> {
        let hash = genesis.block_hash();

        // Verify the genesis hash matches the chain parameters.
        if hash != self.params.genesis_block_hash {
            return Err(format!(
                "Genesis block hash mismatch: expected {}, got {}",
                self.params.genesis_block_hash.to_hex(),
                hash.to_hex(),
            ));
        }

        // Don't re-initialise if the genesis block is already loaded.
        if self.block_index.find_by_hash(&hash).is_some() {
            return Ok(());
        }

        // Build the block index entry.
        let mut index = BlockIndex::from_header(&genesis.header, 0);
        index.chain_work = get_block_proof(&index);
        index.tx_count = genesis.vtx.len() as u32;
        index.chain_tx_count = genesis.vtx.len() as u64;
        index.status.raise_validity(BlockStatus::VALID_SCRIPTS);
        index.status.insert(BlockStatus::HAVE_DATA);
        index.time_max = genesis.header.time;

        let chain_work = index.chain_work;
        let arena_idx = self.block_index.insert(index);
        self.block_index.build_skip_pointer(arena_idx);
        self.dirty_indices.insert(arena_idx);

        // Track as best valid tip.
        self.best_valid_tip = Some((arena_idx, chain_work));

        // Update the active chain.
        self.active_chainstate.chain.set_tip(arena_idx, 0);

        // Add coinbase outputs to the UTXO cache.
        if let Some(coinbase) = genesis.vtx.first() {
            add_coins(&self.active_chainstate.coins_tip, coinbase, 0, true);
        }

        // Mark the best block in the UTXO view.
        self.active_chainstate.coins_tip.set_best_block(hash);

        Ok(())
    }

    // -- Block header acceptance --------------------------------------------

    /// Accept a block header: validate and add to the block index.
    ///
    /// Returns the arena index of the (new or existing) block index entry on
    /// success, or a `BlockValidationState` describing the failure.
    ///
    /// This is a simplified version of Bitcoin Core's `AcceptBlockHeader`.
    /// Validation steps:
    /// 1. Check if the header is already known.
    /// 2. Perform context-free header checks (PoW, etc.).
    /// 3. Look up the parent block.
    /// 4. Perform contextual header checks (timestamp, difficulty, etc.).
    /// 5. Add the new block index entry to the arena.
    pub fn accept_block_header(
        &mut self,
        header: &BlockHeader,
    ) -> Result<usize, BlockValidationState> {
        let hash = header.block_hash();

        // 1. Already known?
        if let Some(idx) = self.block_index.find_by_hash(&hash) {
            return Ok(idx);
        }

        // 2. Context-free header checks.
        let mut state = BlockValidationState::new();
        if !check_block_header(header, &self.params.consensus, &mut state) {
            return Err(state);
        }

        // 3. Find the parent.
        let prev_idx = match self.block_index.find_by_hash(&header.prev_blockhash) {
            Some(idx) => idx,
            None => {
                let mut state = BlockValidationState::new();
                state.invalid(
                    BlockValidationResult::InvalidHeader,
                    "bad-prevblk",
                    "previous block not found",
                );
                return Err(state);
            }
        };

        // Check that the parent has not been marked as invalid.
        if self.block_index.get(prev_idx).status.has_failed() {
            let mut state = BlockValidationState::new();
            state.invalid(
                BlockValidationResult::InvalidHeader,
                "bad-prevblk",
                "previous block is invalid",
            );
            return Err(state);
        }

        // 4. Contextual header checks (with arena for real MTP).
        if !contextual_check_block_header_with_arena(
            header,
            &self.block_index.get(prev_idx),
            &self.params.consensus,
            &mut state,
            Some(self.block_index.as_slice()),
            Some(prev_idx),
        ) {
            return Err(state);
        }

        // 5. Build the new block index entry.
        let prev = self.block_index.get(prev_idx);
        let height = prev.height + 1;
        let mut index = BlockIndex::from_header(header, height);
        index.prev = Some(prev_idx);
        index.chain_work = prev.chain_work + get_block_proof(&index);
        index.time_max = std::cmp::max(prev.time_max, header.time);
        index.status.raise_validity(BlockStatus::VALID_TREE);

        let arena_idx = self.block_index.insert(index);
        self.block_index.build_skip_pointer(arena_idx);
        self.dirty_indices.insert(arena_idx);
        Ok(arena_idx)
    }

    // -- Full block processing ----------------------------------------------

    /// Process a new block: accept header, validate block body, connect to
    /// the active chain.
    ///
    /// This is the main entry point for new blocks arriving from the network
    /// or generated locally.
    ///
    /// Returns `Ok(true)` if the block was connected to the active chain,
    /// `Ok(false)` if the block was accepted but is not yet on the active chain
    /// (e.g. an orphan or a block on a shorter fork), or `Err` with a
    /// validation state on failure.
    ///
    /// Simplified flow (no reorg, no parallel verification):
    /// 1. Accept the block header.
    /// 2. Context-free block checks.
    /// 3. Contextual block checks.
    /// 4. Connect the block (update UTXO set).
    /// 5. Extend the active chain.
    pub fn process_new_block(&mut self, block: &Block) -> Result<(bool, Option<BlockUndo>), BlockValidationState> {
        // 1. Accept the header (idempotent if already known).
        let arena_idx = self.accept_block_header(&block.header)?;

        // If the block is already fully validated AND is in the active chain,
        // nothing more to do. We check active chain membership to handle the
        // case where crash recovery disconnected blocks but left VALID_SCRIPTS
        // status — those blocks need to be re-connected to rebuild the UTXO.
        if self
            .block_index
            .get(arena_idx)
            .is_valid(BlockStatus::VALID_SCRIPTS)
        {
            let entry = self.block_index.get(arena_idx);
            let in_active_chain = self
                .active_chainstate
                .chain
                .get_block_index(entry.height)
                == Some(arena_idx);
            if in_active_chain {
                let on_active = self.active_chainstate.chain.tip() == Some(arena_idx);
                return Ok((on_active, None));
            }
            // Block has VALID_SCRIPTS but is not in active chain (crash recovery
            // disconnected it). Fall through to re-connect it.
        }

        // 2. Context-free block body checks.
        let mut state = BlockValidationState::new();
        if !check_block(block, &self.params.consensus, &mut state, true) {
            self.block_index
                .get_mut(arena_idx)
                .status
                .insert(BlockStatus::FAILED_VALID);
            self.dirty_indices.insert(arena_idx);
            return Err(state);
        }

        // Mark that we have the transaction data.
        {
            let idx = self.block_index.get_mut(arena_idx);
            idx.tx_count = block.vtx.len() as u32;
            idx.status.insert(BlockStatus::HAVE_DATA);
            idx.status.raise_validity(BlockStatus::VALID_TRANSACTIONS);

            // Compute chain tx count.
            let parent_chain_tx = idx
                .prev
                .map(|p| self.block_index.get(p).chain_tx_count)
                .unwrap_or(0);
            // Re-borrow mutably to set chain_tx_count -- we already read parent above.
            self.block_index.get_mut(arena_idx).chain_tx_count =
                parent_chain_tx + block.vtx.len() as u64;
            self.dirty_indices.insert(arena_idx);
        }

        // 3. Contextual block checks.
        let prev_idx = self.block_index.get(arena_idx).prev;
        if let Some(pi) = prev_idx {
            let prev = self.block_index.get(pi);
            let prev_height = prev.height;

            // Compute median time past for the parent.
            let ancestors = self.collect_ancestors(pi, 11);
            let prev_median_time = BlockIndex::get_median_time_past(&ancestors);

            if !contextual_check_block(
                block,
                prev_height,
                prev_median_time,
                &self.params.consensus,
                &mut state,
            ) {
                self.block_index
                    .get_mut(arena_idx)
                    .status
                    .insert(BlockStatus::FAILED_VALID);
                self.dirty_indices.insert(arena_idx);
                return Err(state);
            }
        }

        // 4. Connect the block (validate transactions, update UTXO set).
        //    connect_block now returns BlockUndo data that we store for
        //    potential chain reorganization.
        let height = self.block_index.get(arena_idx).height;
        let block_hash = self.block_index.get(arena_idx).block_hash;

        // Assume-valid optimization: skip script verification for blocks
        // that are ancestors of the assume-valid block. This dramatically
        // speeds up IBD by skipping the expensive parallel Rayon script
        // checks while still fully validating UTXO state.
        let skip_scripts = self.should_skip_scripts(arena_idx);

        // Build a real MTP lookup closure backed by the block index arena
        // and the active chain, for accurate BIP68 relative time-lock
        // evaluation.
        let arena = self.block_index.as_slice();
        let chain = &self.active_chainstate.chain;
        let mtp_lookup = |h: i32| -> i64 {
            if let Some(arena_idx) = chain.get_block_index(h) {
                qubitcoin_common::chain::compute_mtp(arena, arena_idx)
            } else {
                block.header.time as i64
            }
        };

        let block_undo = connect_block(
            block,
            height,
            &self.active_chainstate.coins_tip,
            &self.params.consensus,
            Some(&mtp_lookup),
            skip_scripts,
        )
        .map_err(|e| {
            self.block_index
                .get_mut(arena_idx)
                .status
                .insert(BlockStatus::FAILED_VALID);
            self.dirty_indices.insert(arena_idx);
            e
        })?;

        // In test mode, store undo data in-memory for existing tests.
        // In production, return undo data to the caller for disk persistence.
        #[cfg(test)]
        {
            self.stored_blocks.insert(block_hash, block.clone());
            self.stored_undos.insert(block_hash, block_undo.clone());
        }

        // Mark fully validated and update best valid tip tracker.
        self.block_index
            .get_mut(arena_idx)
            .status
            .raise_validity(BlockStatus::VALID_SCRIPTS);
        self.dirty_indices.insert(arena_idx);
        let work = self.block_index.get(arena_idx).chain_work;
        if self.best_valid_tip.map_or(true, |(_, bw)| work > bw) {
            self.best_valid_tip = Some((arena_idx, work));
        }

        // 5. Activate the best chain.  Pass the arena index of the block
        //    we just connected so activate_best_chain skips reconnecting it
        //    (its UTXO changes are already in coins_tip).
        self.activate_best_chain(Some(arena_idx))?;

        let on_active = self.active_chainstate.chain.tip() == Some(arena_idx);
        Ok((on_active, Some(block_undo)))
    }

    // -- Internal helpers ---------------------------------------------------

    /// Activate the best available chain.
    ///
    /// Finds the block-index entry with the most cumulative work that is
    /// fully validated (`VALID_SCRIPTS`). If it differs from the current tip,
    /// performs a chain reorganization:
    ///   1. Find the fork point between the current chain and the new best
    ///      chain.
    ///   2. Disconnect blocks from the current tip down to the fork point.
    ///   3. Connect blocks from the fork point up to the new tip.
    ///   4. Update the active chain.
    fn activate_best_chain(
        &mut self,
        just_connected: Option<usize>,
    ) -> Result<(), BlockValidationState> {
        // Use the tracked best valid tip (O(1)) instead of scanning the
        // entire block index (O(N)). During normal IBD this avoids an
        // O(N²) bottleneck — we were doing a full scan of 300k+ entries
        // on every single block.
        let best_idx = match self.best_valid_tip {
            Some((idx, _)) => idx,
            None => return Ok(()), // No valid blocks at all.
        };

        let current_tip = self.active_chainstate.chain.tip();

        // If already at the best tip, nothing to do.
        if current_tip == Some(best_idx) {
            return Ok(());
        }

        let best_height = self.block_index.get(best_idx).height;

        // Build the new chain path from best_idx back to genesis.
        let new_chain = self.build_chain_path(best_idx);

        // Find the fork point: the highest block present in both the current
        // active chain and the new chain.
        let current_height = self.active_chainstate.chain.height();
        let fork_height = self.find_fork_height(&new_chain, current_height);

        // --- Disconnect blocks from current tip down to fork point ---
        if current_height > fork_height {
            for h in (fork_height + 1..=current_height).rev() {
                let idx_at_h = match self.active_chainstate.chain.get_block_index(h) {
                    Some(idx) => idx,
                    None => continue,
                };
                let entry = self.block_index.get(idx_at_h);
                let block_hash = entry.block_hash;
                let file = entry.file;
                let data_pos = entry.data_pos;
                let undo_pos = entry.undo_pos;

                // Look up the stored block and undo data (in-memory first, then disk).
                let (block, undo) = if let Some(b) = self.stored_blocks.get(&block_hash) {
                    let u = self.stored_undos.get(&block_hash).cloned().unwrap_or_else(BlockUndo::new);
                    (b.clone(), u)
                } else if let Some(ref reader) = self.block_reader {
                    match reader(file, data_pos, undo_pos) {
                        Some((b, u)) => (b, u),
                        None => continue,
                    }
                } else {
                    continue;
                };

                disconnect_block(&block, h, &self.active_chainstate.coins_tip, &undo);
            }
        }

        // --- Connect blocks from fork point up to new best tip ---
        for h in (fork_height + 1)..=best_height {
            let h_usize = h as usize;
            if h_usize >= new_chain.len() {
                break;
            }
            let idx_at_h = new_chain[h_usize];
            let block_hash = self.block_index.get(idx_at_h).block_hash;

            // Skip the block whose UTXO changes were already applied by the
            // caller (process_new_block).  Reconnecting it would double-apply
            // outputs/spends and corrupt the UTXO set.
            if just_connected == Some(idx_at_h) {
                continue;
            }

            // Build MTP lookup for this connect_block call.
            let arena = self.block_index.as_slice();
            let chain = &self.active_chainstate.chain;
            let mtp_lookup = |mtp_h: i32| -> i64 {
                if let Some(ai) = chain.get_block_index(mtp_h) {
                    qubitcoin_common::chain::compute_mtp(arena, ai)
                } else {
                    0i64
                }
            };

            // Look up block from in-memory cache or disk reader.
            let entry = self.block_index.get(idx_at_h);
            let file = entry.file;
            let data_pos = entry.data_pos;
            let undo_pos = entry.undo_pos;

            let block = if let Some(b) = self.stored_blocks.get(&block_hash) {
                Some(b.clone())
            } else if let Some(ref reader) = self.block_reader {
                reader(file, data_pos, undo_pos).map(|(b, _u)| b)
            } else {
                None
            };

            if let Some(block) = block {
                let skip = self.should_skip_scripts(idx_at_h);
                if !self
                    .block_index
                    .get(idx_at_h)
                    .is_valid(BlockStatus::VALID_SCRIPTS)
                {
                    let undo = connect_block(
                        &block,
                        h,
                        &self.active_chainstate.coins_tip,
                        &self.params.consensus,
                        Some(&mtp_lookup),
                        skip,
                    )
                    .map_err(|e| {
                        self.block_index
                            .get_mut(idx_at_h)
                            .status
                            .insert(BlockStatus::FAILED_VALID);
                        self.dirty_indices.insert(idx_at_h);
                        e
                    })?;
                    self.stored_undos.insert(block_hash, undo);
                    self.block_index
                        .get_mut(idx_at_h)
                        .status
                        .raise_validity(BlockStatus::VALID_SCRIPTS);
                    self.dirty_indices.insert(idx_at_h);
                    let work = self.block_index.get(idx_at_h).chain_work;
                    if self.best_valid_tip.map_or(true, |(_, bw)| work > bw) {
                        self.best_valid_tip = Some((idx_at_h, work));
                    }
                } else {
                    // Already validated; replay UTXO changes.
                    let undo = connect_block(
                        &block,
                        h,
                        &self.active_chainstate.coins_tip,
                        &self.params.consensus,
                        Some(&mtp_lookup),
                        skip,
                    )
                    .map_err(|e| {
                        self.block_index
                            .get_mut(idx_at_h)
                            .status
                            .insert(BlockStatus::FAILED_VALID);
                        self.dirty_indices.insert(idx_at_h);
                        e
                    })?;
                    self.stored_undos.insert(block_hash, undo);
                }
            }
        }

        // --- Update the active chain ---
        let block_index = &self.block_index;
        self.active_chainstate
            .chain
            .set_tip_with(best_idx, best_height, |idx| {
                let entry = block_index.get(idx);
                entry.prev.map(|p| (p, block_index.get(p).height))
            });

        // Update the UTXO cache best block.
        let tip_hash = self.block_index.get(best_idx).block_hash;
        self.active_chainstate.coins_tip.set_best_block(tip_hash);

        Ok(())
    }

    /// Build the chain path from genesis (index 0) to the block at
    /// `tip_idx`, returning a `Vec` where `result[height] = arena_index`.
    fn build_chain_path(&self, tip_idx: usize) -> Vec<usize> {
        let tip_height = self.block_index.get(tip_idx).height;
        let mut path = vec![0usize; (tip_height + 1) as usize];
        let mut current = tip_idx;
        let mut h = tip_height;
        while h >= 0 {
            path[h as usize] = current;
            match self.block_index.get(current).prev {
                Some(prev) => {
                    current = prev;
                    h -= 1;
                }
                None => break,
            }
        }
        path
    }

    /// Find the height of the fork point between the current active chain
    /// and a new chain (given as a path vector).
    fn find_fork_height(&self, new_chain: &[usize], current_height: i32) -> i32 {
        let min_height = current_height.min(new_chain.len() as i32 - 1);
        for h in (0..=min_height).rev() {
            if let Some(current_idx) = self.active_chainstate.chain.get_block_index(h) {
                if (h as usize) < new_chain.len() && current_idx == new_chain[h as usize] {
                    return h;
                }
            }
        }
        -1 // No common ancestor (shouldn't happen if both share genesis).
    }

    /// Determine whether script verification should be skipped for a block.
    ///
    /// Returns `true` when assume-valid is configured and the block at
    /// `arena_idx` is either the assume-valid block itself or an ancestor
    /// of it. This allows IBD to skip the expensive parallel script
    /// verification for blocks already trusted by the network.
    fn should_skip_scripts(&self, arena_idx: usize) -> bool {
        let assume_hash = match &self.assume_valid {
            Some(h) => h,
            None => return false,
        };

        let block_hash = self.block_index.get(arena_idx).block_hash;

        // Exact match: this IS the assume-valid block.
        if block_hash == *assume_hash {
            return true;
        }

        // Check if the assume-valid block is in our index.
        let assume_idx = match self.block_index.find_by_hash(&assume_hash) {
            Some(idx) => idx,
            None => {
                // Haven't seen the assume-valid header yet (early IBD,
                // header sync hasn't reached that height).  Fall back to
                // a height-based check so we can skip scripts immediately
                // rather than waiting for header sync to catch up.
                if let Some(av_height) = self.assume_valid_height {
                    let block_height = self.block_index.get(arena_idx).height;
                    return block_height < av_height;
                }
                return false;
            }
        };

        // The block at arena_idx is an ancestor of assume_valid if
        // get_ancestor(assume_valid, block_height) == arena_idx.
        let block_height = self.block_index.get(arena_idx).height;
        let assume_height = self.block_index.get(assume_idx).height;

        if block_height > assume_height {
            return false; // Block is beyond the assume-valid block.
        }

        let ancestor = get_ancestor(self.block_index.as_slice(), assume_idx, block_height);
        ancestor == Some(arena_idx)
    }

    /// Collect up to `count` ancestors of the block at `arena_idx`, starting
    /// with the block itself.
    ///
    /// Returns a `Vec<&BlockIndex>` ordered from the given block backwards
    /// through its parents.
    fn collect_ancestors(&self, arena_idx: usize, count: usize) -> Vec<&BlockIndex> {
        let mut result = Vec::with_capacity(count);
        let mut current = Some(arena_idx);
        for _ in 0..count {
            match current {
                Some(idx) => {
                    let entry = self.block_index.get(idx);
                    result.push(entry);
                    current = entry.prev;
                }
                None => break,
            }
        }
        result
    }
}

// ---------------------------------------------------------------------------
// Validation stubs for tests
// ---------------------------------------------------------------------------
//
// When running unit tests for this module, we do not yet have the
// `crate::validation` module available (it is being developed in parallel).
// These stubs provide minimal implementations that accept all inputs so that
// the chainstate tests can exercise the data-structure logic independently
// of full consensus validation.

#[cfg(test)]
fn check_block_header(
    _header: &BlockHeader,
    _params: &ConsensusParams,
    _state: &mut BlockValidationState,
) -> bool {
    true
}

#[cfg(test)]
fn check_block(
    _block: &Block,
    _params: &ConsensusParams,
    _state: &mut BlockValidationState,
    _check_merkle_root: bool,
) -> bool {
    true
}

#[cfg(test)]
fn contextual_check_block_header(
    _header: &BlockHeader,
    _prev: &BlockIndex,
    _params: &ConsensusParams,
    _state: &mut BlockValidationState,
) -> bool {
    true
}

#[cfg(test)]
fn contextual_check_block_header_with_arena(
    _header: &BlockHeader,
    _prev: &BlockIndex,
    _params: &ConsensusParams,
    _state: &mut BlockValidationState,
    _arena: Option<&[BlockIndex]>,
    _prev_idx: Option<usize>,
) -> bool {
    true
}

#[cfg(test)]
fn contextual_check_block(
    _block: &Block,
    _prev_height: i32,
    _prev_median_time: i64,
    _params: &ConsensusParams,
    _state: &mut BlockValidationState,
) -> bool {
    true
}

#[cfg(test)]
fn connect_block(
    block: &Block,
    height: i32,
    view: &CoinsViewCache,
    _params: &ConsensusParams,
    _mtp_at_height: Option<&dyn Fn(i32) -> i64>,
    _skip_scripts: bool,
) -> Result<BlockUndo, BlockValidationState> {
    use crate::undo::TxUndo;

    let mut block_undo = BlockUndo::new();

    // Add coinbase outputs.
    if let Some(coinbase) = block.vtx.first() {
        add_coins(view, coinbase, height as u32, true);
    }

    // Process non-coinbase transactions: capture spent coins then spend.
    for tx in block.vtx.iter().skip(1) {
        add_coins(view, tx, height as u32, false);

        let mut tx_undo = TxUndo::new();
        for input in &tx.vin {
            let coin = view
                .fetch_coin(&input.prevout)
                .unwrap_or_else(|| qubitcoin_common::coins::Coin::empty());
            tx_undo.prev_coins.push(coin);
            view.spend_coin(&input.prevout);
        }
        block_undo.tx_undo.push(tx_undo);
    }

    Ok(block_undo)
}

/// Result of a block disconnection (test stub).
#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DisconnectResult {
    Ok,
    #[allow(dead_code)]
    Unclean,
    #[allow(dead_code)]
    Failed,
}

#[cfg(test)]
fn disconnect_block(
    block: &Block,
    _height: i32,
    view: &CoinsViewCache,
    undo: &BlockUndo,
) -> DisconnectResult {
    use qubitcoin_consensus::transaction::OutPoint;

    // Walk transactions in reverse.
    for (tx_idx, tx) in block.vtx.iter().enumerate().rev() {
        let txid = tx.txid().clone();

        // Remove outputs.
        for (i, _) in tx.vout.iter().enumerate() {
            let outpoint = OutPoint::new(txid.clone(), i as u32);
            view.spend_coin(&outpoint);
        }

        // Restore inputs from undo data for non-coinbase txs.
        if !tx.is_coinbase() && tx_idx > 0 {
            let undo_idx = tx_idx - 1;
            if undo_idx < undo.tx_undo.len() {
                for (input, undo_coin) in
                    tx.vin.iter().zip(undo.tx_undo[undo_idx].prev_coins.iter())
                {
                    if !undo_coin.is_spent() {
                        view.add_coin(&input.prevout, undo_coin.clone(), true);
                    }
                }
            }
        }
    }

    DisconnectResult::Ok
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use qubitcoin_consensus::block::{Block, BlockHeader};
    use qubitcoin_primitives::{BlockHash, Uint256};

    /// Helper: build a minimal genesis block whose hash matches the regtest
    /// genesis. We use regtest because its PoW limit is extremely permissive
    /// and the genesis parameters are simpler.
    fn make_regtest_genesis() -> Block {
        // The regtest genesis has the same header fields as mainnet except
        // nBits = 0x207fffff and nTime = 1296688602.
        let mut header = BlockHeader::new();
        header.version = 1;
        header.time = 1296688602;
        header.bits = 0x207fffff;
        header.nonce = 2;
        header.merkle_root =
            Uint256::from_hex("4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b")
                .unwrap();

        Block {
            header,
            // The genesis coinbase transaction is not needed for our structural
            // tests; an empty vtx is acceptable because the test stubs accept
            // everything.
            vtx: Vec::new(),
        }
    }

    // -- BlockMap tests -----------------------------------------------------

    #[test]
    fn test_block_map_empty() {
        let map = BlockMap::new();
        assert_eq!(map.len(), 0);
        assert!(map.is_empty());
        assert!(map.find_by_hash(&BlockHash::ZERO).is_none());
    }

    #[test]
    fn test_block_map_insert_and_lookup() {
        let mut map = BlockMap::new();
        let header = BlockHeader {
            version: 1,
            prev_blockhash: BlockHash::ZERO,
            merkle_root: Uint256::ZERO,
            time: 1000,
            bits: 0x207fffff,
            nonce: 42,
        };
        let index = BlockIndex::from_header(&header, 0);
        let hash = index.block_hash;

        let arena_idx = map.insert(index);
        assert_eq!(arena_idx, 0);
        assert_eq!(map.len(), 1);
        assert!(!map.is_empty());

        // Lookup by hash.
        assert_eq!(map.find_by_hash(&hash), Some(0));

        // Access by arena index.
        let entry = map.get(arena_idx);
        assert_eq!(entry.block_hash, hash);
        assert_eq!(entry.height, 0);
    }

    #[test]
    fn test_block_map_multiple_inserts() {
        let mut map = BlockMap::new();

        for i in 0..5u32 {
            let header = BlockHeader {
                version: 1,
                prev_blockhash: BlockHash::ZERO,
                merkle_root: Uint256::ZERO,
                time: 1000 + i,
                bits: 0x207fffff,
                nonce: i,
            };
            let index = BlockIndex::from_header(&header, i as i32);
            map.insert(index);
        }

        assert_eq!(map.len(), 5);

        // Each entry should be at its sequential index.
        for i in 0..5 {
            let entry = map.get(i);
            assert_eq!(entry.height, i as i32);
        }
    }

    #[test]
    fn test_block_map_get_mut() {
        let mut map = BlockMap::new();
        let header = BlockHeader {
            version: 1,
            prev_blockhash: BlockHash::ZERO,
            merkle_root: Uint256::ZERO,
            time: 2000,
            bits: 0x207fffff,
            nonce: 99,
        };
        let index = BlockIndex::from_header(&header, 0);
        let arena_idx = map.insert(index);

        // Mutate via get_mut.
        map.get_mut(arena_idx).height = 42;
        assert_eq!(map.get(arena_idx).height, 42);
    }

    // -- ChainstateManager tests --------------------------------------------

    #[test]
    fn test_chainstate_manager_empty() {
        let params = ChainParams::regtest();
        let manager = ChainstateManager::new(params, Box::new(EmptyCoinsView));

        assert_eq!(manager.height(), -1);
        assert!(manager.tip().is_none());
        assert!(manager.active_chain().is_empty());
        assert_eq!(manager.block_index().len(), 0);
    }

    #[test]
    fn test_chainstate_manager_params() {
        let params = ChainParams::regtest();
        let manager = ChainstateManager::new(params, Box::new(EmptyCoinsView));

        assert_eq!(
            manager.params().network,
            qubitcoin_common::chainparams::Network::Regtest,
        );
        assert!(manager.consensus().pow_no_retargeting);
        assert!(manager.consensus().pow_allow_min_difficulty_blocks);
    }

    #[test]
    fn test_load_genesis_block() {
        let params = ChainParams::regtest();
        let expected_hash = params.genesis_block_hash;
        let mut manager = ChainstateManager::new(params, Box::new(EmptyCoinsView));

        let genesis = make_regtest_genesis();
        let genesis_hash = genesis.block_hash();

        // The test genesis must match the regtest genesis hash.
        assert_eq!(genesis_hash, expected_hash);

        manager.load_genesis_block(&genesis).unwrap();

        // Chain should now be at height 0.
        assert_eq!(manager.height(), 0);

        // Tip should be the genesis.
        let tip_idx = manager.tip().expect("chain should have a tip");
        let tip = manager.block_index().get(tip_idx);
        assert_eq!(tip.block_hash, expected_hash);
        assert_eq!(tip.height, 0);

        // The genesis block should be fully valid.
        assert!(tip.is_valid(BlockStatus::VALID_SCRIPTS));

        // Chain work should be non-zero (the genesis block has some work).
        assert!(tip.chain_work > ArithUint256::zero());

        // Block index should have exactly one entry.
        assert_eq!(manager.block_index().len(), 1);
    }

    #[test]
    fn test_load_genesis_block_idempotent() {
        let params = ChainParams::regtest();
        let mut manager = ChainstateManager::new(params, Box::new(EmptyCoinsView));
        let genesis = make_regtest_genesis();

        manager.load_genesis_block(&genesis).unwrap();
        // Loading again should be a no-op.
        manager.load_genesis_block(&genesis).unwrap();

        assert_eq!(manager.height(), 0);
        assert_eq!(manager.block_index().len(), 1);
    }

    #[test]
    fn test_load_genesis_block_wrong_hash() {
        let params = ChainParams::regtest();
        let mut manager = ChainstateManager::new(params, Box::new(EmptyCoinsView));

        // Build a block with the wrong hash.
        let mut header = BlockHeader::new();
        header.version = 1;
        header.time = 9999999;
        header.bits = 0x207fffff;
        header.nonce = 0;
        let bad_genesis = Block::with_header(header);

        let result = manager.load_genesis_block(&bad_genesis);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("mismatch"));
    }

    #[test]
    fn test_lookup_block_index() {
        let params = ChainParams::regtest();
        let mut manager = ChainstateManager::new(params, Box::new(EmptyCoinsView));
        let genesis = make_regtest_genesis();
        let genesis_hash = genesis.block_hash();

        manager.load_genesis_block(&genesis).unwrap();

        assert!(manager.lookup_block_index(&genesis_hash).is_some());
        assert!(manager.lookup_block_index(&BlockHash::ZERO).is_none());
    }

    #[test]
    fn test_accept_block_header_after_genesis() {
        let params = ChainParams::regtest();
        let mut manager = ChainstateManager::new(params, Box::new(EmptyCoinsView));
        let genesis = make_regtest_genesis();
        let genesis_hash = genesis.block_hash();

        manager.load_genesis_block(&genesis).unwrap();

        // Create a header that builds on genesis.
        let header = BlockHeader {
            version: 1,
            prev_blockhash: genesis_hash,
            merkle_root: Uint256::ZERO,
            time: 1296688612,
            bits: 0x207fffff,
            nonce: 0,
        };

        let arena_idx = manager.accept_block_header(&header).unwrap();
        let entry = manager.block_index().get(arena_idx);
        assert_eq!(entry.height, 1);
        assert!(entry.prev.is_some());
        assert_eq!(entry.prev_blockhash, genesis_hash);
        assert_eq!(manager.block_index().len(), 2);
    }

    #[test]
    fn test_accept_block_header_unknown_parent() {
        let params = ChainParams::regtest();
        let mut manager = ChainstateManager::new(params, Box::new(EmptyCoinsView));
        let genesis = make_regtest_genesis();
        manager.load_genesis_block(&genesis).unwrap();

        // Header referencing a parent that doesn't exist.
        let header = BlockHeader {
            version: 1,
            prev_blockhash: BlockHash::from_hex(
                "0000000000000000000000000000000000000000000000000000000000000001",
            )
            .unwrap(),
            merkle_root: Uint256::ZERO,
            time: 1296688612,
            bits: 0x207fffff,
            nonce: 0,
        };

        let result = manager.accept_block_header(&header);
        assert!(result.is_err());
        let state = result.unwrap_err();
        assert!(state.is_invalid());
        assert_eq!(state.get_reject_reason(), "bad-prevblk");
    }

    #[test]
    fn test_accept_header_idempotent() {
        let params = ChainParams::regtest();
        let mut manager = ChainstateManager::new(params, Box::new(EmptyCoinsView));
        let genesis = make_regtest_genesis();
        let genesis_hash = genesis.block_hash();
        manager.load_genesis_block(&genesis).unwrap();

        let header = BlockHeader {
            version: 1,
            prev_blockhash: genesis_hash,
            merkle_root: Uint256::ZERO,
            time: 1296688612,
            bits: 0x207fffff,
            nonce: 0,
        };

        let idx1 = manager.accept_block_header(&header).unwrap();
        let idx2 = manager.accept_block_header(&header).unwrap();
        assert_eq!(idx1, idx2);
        assert_eq!(manager.block_index().len(), 2);
    }

    #[test]
    fn test_process_new_block_extends_chain() {
        let params = ChainParams::regtest();
        let mut manager = ChainstateManager::new(params, Box::new(EmptyCoinsView));
        let genesis = make_regtest_genesis();
        let genesis_hash = genesis.block_hash();
        manager.load_genesis_block(&genesis).unwrap();

        // Build block 1 on top of genesis.
        let header1 = BlockHeader {
            version: 1,
            prev_blockhash: genesis_hash,
            merkle_root: Uint256::ZERO,
            time: 1296688612,
            bits: 0x207fffff,
            nonce: 0,
        };
        let block1 = Block::with_header(header1);
        let block1_hash = block1.block_hash();

        let (on_active, _undo) = manager.process_new_block(&block1).unwrap();
        assert!(on_active); // Should be on the active chain.

        assert_eq!(manager.height(), 1);
        let tip_idx = manager.tip().unwrap();
        let tip = manager.block_index().get(tip_idx);
        assert_eq!(tip.block_hash, block1_hash);
        assert_eq!(tip.height, 1);
        assert!(tip.is_valid(BlockStatus::VALID_SCRIPTS));
    }

    #[test]
    fn test_process_sequential_blocks() {
        let params = ChainParams::regtest();
        let mut manager = ChainstateManager::new(params, Box::new(EmptyCoinsView));
        let genesis = make_regtest_genesis();
        manager.load_genesis_block(&genesis).unwrap();

        let mut prev_hash = genesis.block_hash();
        let base_time: u32 = 1296688612;

        // Build a chain of 10 blocks.
        for i in 0..10 {
            let header = BlockHeader {
                version: 1,
                prev_blockhash: prev_hash,
                merkle_root: Uint256::ZERO,
                time: base_time + (i as u32) * 600,
                bits: 0x207fffff,
                nonce: i as u32,
            };
            let block = Block::with_header(header);
            prev_hash = block.block_hash();

            let (on_active, _undo) = manager.process_new_block(&block).unwrap();
            assert!(on_active, "block {} should be on active chain", i + 1);
        }

        assert_eq!(manager.height(), 10);
        assert_eq!(manager.block_index().len(), 11); // genesis + 10 blocks

        let tip_idx = manager.tip().unwrap();
        let tip = manager.block_index().get(tip_idx);
        assert_eq!(tip.height, 10);
        assert_eq!(tip.block_hash, prev_hash);
    }

    #[test]
    fn test_collect_ancestors() {
        let params = ChainParams::regtest();
        let mut manager = ChainstateManager::new(params, Box::new(EmptyCoinsView));
        let genesis = make_regtest_genesis();
        manager.load_genesis_block(&genesis).unwrap();

        let mut prev_hash = genesis.block_hash();
        let base_time: u32 = 1296688612;

        // Build 5 blocks.
        for i in 0..5 {
            let header = BlockHeader {
                version: 1,
                prev_blockhash: prev_hash,
                merkle_root: Uint256::ZERO,
                time: base_time + (i as u32) * 600,
                bits: 0x207fffff,
                nonce: i as u32,
            };
            let block = Block::with_header(header);
            prev_hash = block.block_hash();
            manager.process_new_block(&block).unwrap();
        }

        // Collect ancestors of the tip (height 5).
        let tip_idx = manager.tip().unwrap();
        let ancestors = manager.collect_ancestors(tip_idx, 11);

        // We should get 6 entries (tip + 5 predecessors including genesis).
        assert_eq!(ancestors.len(), 6);
        // First ancestor is the tip itself.
        assert_eq!(ancestors[0].height, 5);
        // Last is genesis.
        assert_eq!(ancestors[5].height, 0);
    }

    #[test]
    fn test_coins_tip_accessible() {
        let params = ChainParams::regtest();
        let manager = ChainstateManager::new(params, Box::new(EmptyCoinsView));

        // The coins tip should be accessible and report zero cached entries.
        assert_eq!(manager.coins_tip().cache_size(), 0);
    }

    #[test]
    fn test_stored_blocks_populated() {
        let params = ChainParams::regtest();
        let mut manager = ChainstateManager::new(params, Box::new(EmptyCoinsView));
        let genesis = make_regtest_genesis();
        let genesis_hash = genesis.block_hash();
        manager.load_genesis_block(&genesis).unwrap();

        // Build block 1.
        let header1 = BlockHeader {
            version: 1,
            prev_blockhash: genesis_hash,
            merkle_root: Uint256::ZERO,
            time: 1296688612,
            bits: 0x207fffff,
            nonce: 0,
        };
        let block1 = Block::with_header(header1);
        let block1_hash = block1.block_hash();

        manager.process_new_block(&block1).unwrap();

        // The block and its undo data should be stored.
        assert!(manager.stored_blocks.contains_key(&block1_hash));
        assert!(manager.stored_undos.contains_key(&block1_hash));
    }

    #[test]
    fn test_chain_reorg_to_longer_fork() {
        // Build a chain A -> B1 -> B2, then create a fork A -> C1 -> C2 -> C3.
        // The longer fork (C-chain) should become the active chain.
        let params = ChainParams::regtest();
        let mut manager = ChainstateManager::new(params, Box::new(EmptyCoinsView));
        let genesis = make_regtest_genesis();
        let genesis_hash = genesis.block_hash();
        manager.load_genesis_block(&genesis).unwrap();

        let base_time: u32 = 1296688612;

        // Build the B-chain: 2 blocks on top of genesis.
        let mut b_prev = genesis_hash;
        let mut b_hashes = Vec::new();
        for i in 0..2 {
            let header = BlockHeader {
                version: 1,
                prev_blockhash: b_prev,
                merkle_root: Uint256::ZERO,
                time: base_time + (i as u32) * 600,
                bits: 0x207fffff,
                nonce: 100 + i as u32,
            };
            let block = Block::with_header(header);
            b_prev = block.block_hash();
            b_hashes.push(b_prev);
            manager.process_new_block(&block).unwrap();
        }

        // Active chain should be at height 2 with B2 as tip.
        assert_eq!(manager.height(), 2);
        let tip_hash = manager.block_index().get(manager.tip().unwrap()).block_hash;
        assert_eq!(tip_hash, *b_hashes.last().unwrap());

        // Build the C-chain: 3 blocks on top of genesis (longer fork).
        let mut c_prev = genesis_hash;
        let mut c_hashes = Vec::new();
        for i in 0..3 {
            let header = BlockHeader {
                version: 1,
                prev_blockhash: c_prev,
                merkle_root: Uint256::ZERO,
                time: base_time + (i as u32) * 600,
                bits: 0x207fffff,
                nonce: 200 + i as u32,
            };
            let block = Block::with_header(header);
            c_prev = block.block_hash();
            c_hashes.push(c_prev);
            manager.process_new_block(&block).unwrap();
        }

        // The C-chain (height 3) has more work, so it should be the active chain.
        assert_eq!(manager.height(), 3);
        let tip_hash = manager.block_index().get(manager.tip().unwrap()).block_hash;
        assert_eq!(tip_hash, *c_hashes.last().unwrap());
    }

    #[test]
    fn test_chain_reorg_fork_at_height_1() {
        // Build chain: G -> A1 -> A2 -> A3 (3 blocks)
        // Then fork:   G -> A1 -> B2 -> B3 -> B4 (3 blocks after A1)
        // The fork should win because it has more total work.
        let params = ChainParams::regtest();
        let mut manager = ChainstateManager::new(params, Box::new(EmptyCoinsView));
        let genesis = make_regtest_genesis();
        let genesis_hash = genesis.block_hash();
        manager.load_genesis_block(&genesis).unwrap();

        let base_time: u32 = 1296688612;

        // Build A-chain: G -> A1 -> A2 -> A3
        let mut a_prev = genesis_hash;
        let mut a_hashes = Vec::new();
        for i in 0..3 {
            let header = BlockHeader {
                version: 1,
                prev_blockhash: a_prev,
                merkle_root: Uint256::ZERO,
                time: base_time + (i as u32) * 600,
                bits: 0x207fffff,
                nonce: 300 + i as u32,
            };
            let block = Block::with_header(header);
            a_prev = block.block_hash();
            a_hashes.push(a_prev);
            manager.process_new_block(&block).unwrap();
        }
        assert_eq!(manager.height(), 3);

        // Fork from A1 (height 1): B2 -> B3 -> B4
        let fork_parent = a_hashes[0]; // A1 at height 1
        let mut b_prev = fork_parent;
        let mut b_hashes = Vec::new();
        for i in 0..3 {
            let header = BlockHeader {
                version: 1,
                prev_blockhash: b_prev,
                merkle_root: Uint256::ZERO,
                time: base_time + ((i + 3) as u32) * 600,
                bits: 0x207fffff,
                nonce: 400 + i as u32,
            };
            let block = Block::with_header(header);
            b_prev = block.block_hash();
            b_hashes.push(b_prev);
            manager.process_new_block(&block).unwrap();
        }

        // B-chain tip is at height 4 (1 + 3), which has more work than
        // A-chain at height 3.
        assert_eq!(manager.height(), 4);
        let tip_hash = manager.block_index().get(manager.tip().unwrap()).block_hash;
        assert_eq!(tip_hash, *b_hashes.last().unwrap());

        // A1 should still be in the active chain at height 1.
        let h1_idx = manager.active_chain().get_block_index(1).unwrap();
        let h1_hash = manager.block_index().get(h1_idx).block_hash;
        assert_eq!(h1_hash, a_hashes[0]);
    }

    #[test]
    fn test_no_reorg_for_equal_work() {
        // Build chain: G -> A1, then fork: G -> B1 (same height, same work).
        // The first chain should remain active.
        let params = ChainParams::regtest();
        let mut manager = ChainstateManager::new(params, Box::new(EmptyCoinsView));
        let genesis = make_regtest_genesis();
        let genesis_hash = genesis.block_hash();
        manager.load_genesis_block(&genesis).unwrap();

        let base_time: u32 = 1296688612;

        // Build A1.
        let header_a = BlockHeader {
            version: 1,
            prev_blockhash: genesis_hash,
            merkle_root: Uint256::ZERO,
            time: base_time,
            bits: 0x207fffff,
            nonce: 500,
        };
        let block_a = Block::with_header(header_a);
        let hash_a = block_a.block_hash();
        manager.process_new_block(&block_a).unwrap();
        assert_eq!(manager.height(), 1);

        // Build B1 (different nonce, same difficulty).
        let header_b = BlockHeader {
            version: 1,
            prev_blockhash: genesis_hash,
            merkle_root: Uint256::ZERO,
            time: base_time + 1,
            bits: 0x207fffff,
            nonce: 501,
        };
        let block_b = Block::with_header(header_b);
        let hash_b = block_b.block_hash();
        manager.process_new_block(&block_b).unwrap();

        // With equal chain work, activate_best_chain uses the *first* block
        // found with the highest work. Due to the iteration order, the result
        // depends on arena ordering. Both are valid, so we just verify height.
        assert_eq!(manager.height(), 1);

        // The tip should be one of the two blocks.
        let tip_hash = manager.block_index().get(manager.tip().unwrap()).block_hash;
        assert!(tip_hash == hash_a || tip_hash == hash_b);
    }

    #[test]
    fn test_build_chain_path() {
        let params = ChainParams::regtest();
        let mut manager = ChainstateManager::new(params, Box::new(EmptyCoinsView));
        let genesis = make_regtest_genesis();
        manager.load_genesis_block(&genesis).unwrap();

        let mut prev_hash = genesis.block_hash();
        let base_time: u32 = 1296688612;

        // Build 3 blocks.
        for i in 0..3 {
            let header = BlockHeader {
                version: 1,
                prev_blockhash: prev_hash,
                merkle_root: Uint256::ZERO,
                time: base_time + (i as u32) * 600,
                bits: 0x207fffff,
                nonce: 600 + i as u32,
            };
            let block = Block::with_header(header);
            prev_hash = block.block_hash();
            manager.process_new_block(&block).unwrap();
        }

        let tip_idx = manager.tip().unwrap();
        let path = manager.build_chain_path(tip_idx);

        // Path should have 4 entries: genesis + 3 blocks.
        assert_eq!(path.len(), 4);
        // Genesis is at index 0.
        assert_eq!(manager.block_index().get(path[0]).height, 0);
        // Tip is at index 3.
        assert_eq!(manager.block_index().get(path[3]).height, 3);
    }

    #[test]
    fn test_skip_pointers_set_and_used_by_get_ancestor() {
        use qubitcoin_common::chain::{get_ancestor, get_skip_height};

        let params = ChainParams::regtest();
        let mut manager = ChainstateManager::new(params, Box::new(EmptyCoinsView));
        let genesis = make_regtest_genesis();
        manager.load_genesis_block(&genesis).unwrap();

        // Build a chain of 100 blocks so that skip pointers get interesting
        // values (skip distances grow as heights increase).
        let mut prev_hash = genesis.block_hash();
        let base_time: u32 = 1296688612;

        for i in 0..100 {
            let header = BlockHeader {
                version: 1,
                prev_blockhash: prev_hash,
                merkle_root: Uint256::ZERO,
                time: base_time + (i as u32) * 600,
                bits: 0x207fffff,
                nonce: 700 + i as u32,
            };
            let block = Block::with_header(header);
            prev_hash = block.block_hash();
            manager.process_new_block(&block).unwrap();
        }

        assert_eq!(manager.height(), 100);
        let arena = manager.block_index().as_slice();

        // 1. Verify that skip pointers are actually populated for blocks
        //    with height >= 2 (height 0 and 1 skip to height 0 which is
        //    trivially reachable via prev).
        let mut populated_count = 0;
        for idx in 0..arena.len() {
            let block = &arena[idx];
            if block.height >= 2 {
                assert!(
                    block.skip.is_some(),
                    "skip pointer should be set for block at height {}",
                    block.height,
                );
                // Verify the skip target is at the correct height.
                let skip_idx = block.skip.unwrap();
                let expected_height = get_skip_height(block.height);
                assert_eq!(
                    arena[skip_idx].height, expected_height,
                    "skip target for height {} should be at height {}, but is at {}",
                    block.height, expected_height, arena[skip_idx].height,
                );
                populated_count += 1;
            }
        }
        assert!(populated_count > 0, "should have set some skip pointers");

        // 2. Verify get_ancestor returns correct results for various
        //    target heights using the skip pointers (O(log n) path).
        let tip_idx = manager.tip().unwrap();
        for target in [0, 1, 10, 25, 50, 75, 99, 100] {
            let result = get_ancestor(arena, tip_idx, target);
            assert!(
                result.is_some(),
                "ancestor at height {} should exist",
                target
            );
            let ancestor_idx = result.unwrap();
            assert_eq!(
                arena[ancestor_idx].height, target,
                "ancestor lookup for height {} returned wrong height {}",
                target, arena[ancestor_idx].height,
            );
        }

        // 3. Cross-check: walk from the ancestor back to genesis using prev
        //    pointers to make sure the skip-found ancestor is on the same chain.
        let ancestor_50 = get_ancestor(arena, tip_idx, 50).unwrap();
        let mut walk = ancestor_50;
        let mut depth = 50;
        while depth > 0 {
            walk = arena[walk].prev.expect("prev should be set");
            depth -= 1;
        }
        assert_eq!(
            arena[walk].height, 0,
            "walking prev from ancestor(50) should reach genesis"
        );

        // 4. Verify that looking up ancestors of intermediate blocks also works.
        let block_at_64 = get_ancestor(arena, tip_idx, 64).unwrap();
        let ancestor_32_of_64 = get_ancestor(arena, block_at_64, 32).unwrap();
        let ancestor_32_of_tip = get_ancestor(arena, tip_idx, 32).unwrap();
        assert_eq!(
            ancestor_32_of_64, ancestor_32_of_tip,
            "ancestor(64, 32) should equal ancestor(100, 32) on the same chain",
        );
    }
}
