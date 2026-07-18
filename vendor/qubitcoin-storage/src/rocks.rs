//! RocksDB database backend for production use.
//!
//! Maps to: `src/dbwrapper.cpp` in Bitcoin Core (which historically used LevelDB;
//! this implementation uses RocksDB instead).

use crate::traits::{Database, DbBatch, DbIterator};
use std::path::Path;

/// Error type for RocksDB operations.
#[derive(Debug, thiserror::Error)]
pub enum RocksError {
    /// An error originating from the underlying `rocksdb` library.
    #[error("RocksDB error: {0}")]
    Rocks(#[from] rocksdb::Error),
}

/// RocksDB-backed key-value database for production use.
///
/// Wraps a `rocksdb::DB` handle and implements the [`Database`] trait.
pub struct RocksDatabase {
    /// The underlying RocksDB handle.
    db: rocksdb::DB,
}

impl RocksDatabase {
    /// Open a RocksDB database at the given path.
    ///
    /// Tuned for the UTXO workload: random point reads dominate, with
    /// periodic large batch writes every ~2000 blocks.
    pub fn open<P: AsRef<Path>>(path: P, cache_size_mb: usize) -> Result<Self, RocksError> {
        let mut opts = rocksdb::Options::default();
        opts.create_if_missing(true);
        opts.set_compression_type(rocksdb::DBCompressionType::Lz4);

        // Keep ALL SST file descriptors open — eliminates table cache
        // lookup overhead on every read. Safe with 31 GB RAM.
        opts.set_max_open_files(-1);

        // Write buffer (memtable): 64 MB for IBD batch writes.
        let write_buf_mb = std::cmp::min(cache_size_mb, 64);
        opts.set_write_buffer_size(write_buf_mb * 1024 * 1024);
        opts.set_max_write_buffer_number(3);
        opts.set_level_zero_file_num_compaction_trigger(4);

        // Dynamic level sizing — automatically adjusts level targets based
        // on actual data size, reducing space amplification to ~1.1x and
        // cutting unnecessary compaction work.
        opts.set_level_compaction_dynamic_level_bytes(true);

        // Skip bloom filters on the last level (~90% of data).
        // UTXO lookups during validation are predominantly positive (coins
        // that DO exist), so the rare negative lookup cost is acceptable.
        opts.set_optimize_filters_for_hits(true);

        // Direct I/O: bypass OS page cache to avoid double-buffering with
        // the RocksDB block cache. Frees OS memory for the UTXO cache.
        opts.set_use_direct_reads(true);
        opts.set_use_direct_io_for_flush_and_compaction(true);
        opts.set_compaction_readahead_size(2 * 1024 * 1024);

        // Block cache for read-heavy UTXO lookups.
        // Bitcoin Core gives LevelDB only 8 MB, putting 98% of cache budget
        // into the in-memory UTXO cache. We follow a similar strategy but
        // with a larger DB cache since RocksDB benefits more from it.
        let block_cache_mb = std::cmp::min(
            std::cmp::max(cache_size_mb.saturating_sub(write_buf_mb), 64),
            2048,
        );
        let mut block_opts = rocksdb::BlockBasedOptions::default();
        block_opts.set_block_cache(&rocksdb::Cache::new_lru_cache(block_cache_mb * 1024 * 1024));
        // Bloom filter: 10 bits/key, ~1% false positive rate.
        block_opts.set_bloom_filter(10.0, false);
        // set_optimize_filters_for_memory requires rocksdb >= 0.22
        // block_opts.set_optimize_filters_for_memory(true);
        // Put index/filter blocks IN the block cache for predictable memory.
        block_opts.set_cache_index_and_filter_blocks(true);
        // Pin L0 index/filter — most frequently accessed, prevent eviction.
        block_opts.set_pin_l0_filter_and_index_blocks_in_cache(true);
        // Hash index inside data blocks — ~10% throughput for point lookups,
        // ~4.6% space overhead.
        block_opts.set_data_block_index_type(rocksdb::DataBlockIndexType::BinaryAndHash);
        block_opts.set_data_block_hash_ratio(0.75);
        opts.set_block_based_table_factory(&block_opts);

        // 8 background threads for compaction + flush on multi-core machines.
        opts.increase_parallelism(8);
        opts.set_max_background_jobs(8);

        let db = rocksdb::DB::open(&opts, path)?;
        Ok(RocksDatabase { db })
    }

    /// Open a RocksDB database with default options (4 MB write buffer).
    pub fn open_default<P: AsRef<Path>>(path: P) -> Result<Self, RocksError> {
        Self::open(path, 4) // 4 MB default write buffer
    }
}

impl Database for RocksDatabase {
    type Batch = RocksBatch;
    type Iterator<'a> = RocksIterator<'a>;
    type Error = RocksError;

    fn read(&self, key: &[u8]) -> Result<Option<Vec<u8>>, Self::Error> {
        Ok(self.db.get(key)?)
    }

    fn multi_read(&self, keys: &[&[u8]]) -> Vec<Result<Option<Vec<u8>>, Self::Error>> {
        self.db
            .multi_get(keys)
            .into_iter()
            .map(|r| r.map_err(RocksError::from))
            .collect()
    }

    fn exists(&self, key: &[u8]) -> Result<bool, Self::Error> {
        Ok(self.db.get(key)?.is_some())
    }

    fn write_batch(&self, batch: Self::Batch, sync: bool) -> Result<(), Self::Error> {
        let mut write_opts = rocksdb::WriteOptions::default();
        write_opts.set_sync(sync);
        self.db.write_opt(batch.inner, &write_opts)?;
        Ok(())
    }

    fn new_batch(&self) -> Self::Batch {
        RocksBatch {
            inner: rocksdb::WriteBatch::default(),
        }
    }

    fn new_iterator(&self) -> Self::Iterator<'_> {
        let iter = self.db.iterator(rocksdb::IteratorMode::Start);
        RocksIterator {
            inner: iter,
            current: None,
        }
    }

    fn compact(&self) -> Result<(), Self::Error> {
        self.db.compact_range::<&[u8], &[u8]>(None, None);
        Ok(())
    }

    fn estimated_size(&self) -> Result<u64, Self::Error> {
        let prop = self
            .db
            .property_value("rocksdb.estimate-live-data-size")
            .unwrap_or(None)
            .unwrap_or_default();
        Ok(prop.parse::<u64>().unwrap_or(0))
    }
}

/// Write batch for [`RocksDatabase`].
///
/// Wraps a `rocksdb::WriteBatch` and implements the [`DbBatch`] trait.
pub struct RocksBatch {
    /// The underlying RocksDB write batch.
    inner: rocksdb::WriteBatch,
}

impl DbBatch for RocksBatch {
    fn put(&mut self, key: &[u8], value: &[u8]) {
        self.inner.put(key, value);
    }

    fn delete(&mut self, key: &[u8]) {
        self.inner.delete(key);
    }

    fn clear(&mut self) {
        self.inner.clear();
    }
}

/// Iterator over [`RocksDatabase`] entries.
///
/// Wraps a `rocksdb::DBIterator` and implements the [`DbIterator`] trait.
pub struct RocksIterator<'a> {
    /// The underlying RocksDB iterator.
    inner: rocksdb::DBIterator<'a>,
    /// The current key-value pair, or `None` if the iterator is exhausted.
    current: Option<(Box<[u8]>, Box<[u8]>)>,
}

impl<'a> DbIterator for RocksIterator<'a> {
    fn seek(&mut self, key: &[u8]) {
        self.inner.set_mode(rocksdb::IteratorMode::From(
            key,
            rocksdb::Direction::Forward,
        ));
        self.advance();
    }

    fn seek_to_first(&mut self) {
        self.inner.set_mode(rocksdb::IteratorMode::Start);
        self.advance();
    }

    fn valid(&self) -> bool {
        self.current.is_some()
    }

    fn next(&mut self) {
        self.advance();
    }

    fn key(&self) -> &[u8] {
        &self.current.as_ref().unwrap().0
    }

    fn value(&self) -> &[u8] {
        &self.current.as_ref().unwrap().1
    }
}

impl<'a> RocksIterator<'a> {
    fn advance(&mut self) {
        self.current = self.inner.next().map(|result| {
            let (k, v) = result.unwrap();
            (k, v)
        });
    }
}
