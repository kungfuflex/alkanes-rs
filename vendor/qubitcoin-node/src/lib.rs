//! Qubitcoin node implementation: block storage, validation, chainstate, and mempool.
//!
//! This crate is the core of the Qubitcoin full node, analogous to Bitcoin Core's
//! `libbitcoin_node` static library. It provides:
//!
//! - **Block storage** ([`block_storage`], [`mmap_storage`]): Reading and writing blocks
//!   to flat files (`blk*.dat` / `rev*.dat`), with optional memory-mapped I/O.
//! - **Block index database** ([`block_index_db`]): Persistent block metadata keyed by
//!   block hash, backed by a key-value store.
//! - **Transaction index** ([`tx_index_db`]): Maps txid to disk position for
//!   `getrawtransaction` support.
//! - **Chainstate management** ([`chainstate`]): Arena-based block index, active chain
//!   tracking, UTXO cache, and block processing pipeline.
//! - **Consensus validation** ([`validation`]): Context-free and context-dependent checks
//!   implementing the full Bitcoin consensus rules.
//! - **Script verification** ([`script_check`]): Parallel script checking via Rayon,
//!   including ECDSA, Schnorr, and timelocks.
//! - **Memory pool** ([`mempool`]): Unconfirmed transaction pool with ancestor/descendant
//!   tracking, fee-rate policies, and replace-by-fee (BIP125).
//! - **Undo data** ([`undo`]): Serializable undo records for chain reorganisation.
//! - **Test framework** ([`test_framework`]): In-memory blockchain for downstream tests.

/// Block index database: persists `BlockIndexRecord` entries to a key-value store.
/// Equivalent to `CBlockTreeDB` in Bitcoin Core.
pub mod block_index_db;

/// Block storage using flat files (`blk*.dat` and `rev*.dat`).
/// Equivalent to `src/node/blockstorage.cpp` in Bitcoin Core.
#[cfg(feature = "filesystem")]
pub mod block_storage;

/// Transaction index database: maps txid to on-disk position.
/// Enables `getrawtransaction` for confirmed transactions.
pub mod tx_index_db;

/// Chainstate management: `BlockMap` arena, active chain, UTXO cache.
/// Equivalent to `Chainstate` / `ChainstateManager` in Bitcoin Core.
pub mod chainstate;

/// Transaction memory pool with fee-rate policies and RBF support.
/// Equivalent to `CTxMemPool` in Bitcoin Core.
pub mod mempool;

/// Memory-mapped block file I/O for zero-copy reads.
/// An improvement over Bitcoin Core's standard file I/O approach.
#[cfg(feature = "filesystem")]
pub mod mmap_storage;

/// Parallel script verification using Rayon's work-stealing scheduler.
/// Replaces Bitcoin Core's `CCheckQueue` thread pool.
pub mod script_check;

/// In-memory blockchain testing framework.
/// Provides `TestChain` for creating test chains without disk or network.
pub mod test_framework;

/// Block undo data (`TxUndo` / `BlockUndo`) for chain reorganisation.
/// Equivalent to `CTxUndo` / `CBlockUndo` in Bitcoin Core.
pub mod undo;

/// Core block and transaction validation functions.
/// Equivalent to `validation.cpp` in Bitcoin Core.
pub mod validation;
