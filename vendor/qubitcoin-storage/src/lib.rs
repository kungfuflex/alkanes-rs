//! Database abstraction and block storage for Qubitcoin.
//!
//! Maps to: `src/dbwrapper.h` and `src/node/blockstorage.h` in Bitcoin Core.
//!
//! Provides:
//! - [`Database`], [`DbBatch`], [`DbIterator`] -- abstract traits for key-value storage.
//! - [`MemoryDb`] -- `BTreeMap`-backed in-memory database (for testing).
//! - [`RocksDatabase`] -- production RocksDB backend (behind `rocksdb-backend` feature).
//! - [`DbWrapper`] -- typed serialization with optional XOR obfuscation.
//! - [`BlockFileManager`] -- flat-file block storage (`blk?????.dat`).

/// Flat-file block storage manager. Equivalent to `FlatFileSeq` / `BlockManager` in Bitcoin Core.
#[cfg(not(target_arch = "wasm32"))]
pub mod block_file;
/// In-memory database backend for testing.
pub mod memory;
/// RocksDB production database backend (requires the `rocksdb-backend` feature).
#[cfg(feature = "rocksdb-backend")]
pub mod rocks;
/// Core database abstraction traits ([`Database`], [`DbBatch`], [`DbIterator`]).
pub mod traits;
/// Typed serialization wrapper with XOR obfuscation. Equivalent to `CDBWrapper` in Bitcoin Core.
pub mod wrapper;

#[cfg(not(target_arch = "wasm32"))]
pub use block_file::{
    BlockFileManager, BlockFilePos, MAINNET_MAGIC, MAX_BLOCKFILE_SIZE, STORAGE_HEADER_BYTES,
};
pub use memory::{MemoryBatch, MemoryDb, MemoryIterator};
#[cfg(feature = "rocksdb-backend")]
pub use rocks::RocksDatabase;
pub use traits::{Database, DbBatch, DbIterator};
pub use wrapper::DbWrapper;
