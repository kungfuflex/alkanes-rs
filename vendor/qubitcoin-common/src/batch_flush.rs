//! Optimized batch flush for UTXO cache.
//!
//! Bitcoin Core's CoinsViewCache::Flush() writes each dirty entry individually.
//! We improve on this by:
//!
//! 1. Collecting all dirty entries into a single database WriteBatch
//! 2. Sorting keys for sequential writes (better for LSM-tree storage)
//! 3. Configurable flush thresholds based on cache size
//! 4. Tracking flush statistics for monitoring
//!
//! This reduces I/O amplification and improves write throughput significantly,
//! especially during initial block download.

use std::time::Instant;

/// Configuration for batch flush behavior.
#[derive(Debug, Clone)]
pub struct FlushConfig {
    /// Maximum number of entries to accumulate before forcing a flush.
    pub max_entries: usize,
    /// Maximum memory usage (estimated bytes) before forcing a flush.
    pub max_memory_bytes: usize,
    /// Whether to sort keys before writing (better for LSM-tree backends).
    pub sort_keys: bool,
    /// Whether to use sync writes (fsync after write).
    pub sync_writes: bool,
}

impl Default for FlushConfig {
    fn default() -> Self {
        FlushConfig {
            max_entries: 500_000,
            max_memory_bytes: 450 * 1024 * 1024, // 450 MB default (Bitcoin Core default)
            sort_keys: true,
            sync_writes: false,
        }
    }
}

/// Statistics from a flush operation.
#[derive(Debug, Clone, Default)]
pub struct FlushStats {
    /// Number of entries written.
    pub entries_written: usize,
    /// Number of entries deleted.
    pub entries_deleted: usize,
    /// Total bytes written.
    pub bytes_written: usize,
    /// Time taken for the flush in microseconds.
    pub elapsed_us: u64,
    /// Write throughput in entries per second.
    pub entries_per_sec: f64,
}

impl FlushStats {
    /// Create stats from a completed flush.
    pub fn from_flush(written: usize, deleted: usize, bytes: usize, start: Instant) -> Self {
        let elapsed = start.elapsed();
        let elapsed_us = elapsed.as_micros() as u64;
        let entries_per_sec = if elapsed_us > 0 {
            ((written + deleted) as f64) / elapsed.as_secs_f64()
        } else {
            0.0
        };
        FlushStats {
            entries_written: written,
            entries_deleted: deleted,
            bytes_written: bytes,
            elapsed_us,
            entries_per_sec,
        }
    }
}

/// A batch of UTXO changes ready for flushing.
///
/// Keys are sorted for sequential write performance on LSM-tree backends.
pub struct CoinsBatch {
    /// Entries to write (key -> serialized value).
    pub puts: Vec<(Vec<u8>, Vec<u8>)>,
    /// Keys to delete.
    pub deletes: Vec<Vec<u8>>,
}

impl CoinsBatch {
    /// Create an empty `CoinsBatch`.
    pub fn new() -> Self {
        CoinsBatch {
            puts: Vec::new(),
            deletes: Vec::new(),
        }
    }

    /// Create a `CoinsBatch` with pre-allocated capacity.
    ///
    /// Allocates `cap` slots for puts and `cap / 4` for deletes.
    pub fn with_capacity(cap: usize) -> Self {
        CoinsBatch {
            puts: Vec::with_capacity(cap),
            deletes: Vec::with_capacity(cap / 4),
        }
    }

    /// Sort keys for optimal write ordering.
    pub fn sort_keys(&mut self) {
        self.puts.sort_by(|a, b| a.0.cmp(&b.0));
        self.deletes.sort();
    }

    /// Total number of operations in this batch.
    pub fn len(&self) -> usize {
        self.puts.len() + self.deletes.len()
    }

    /// Returns `true` if the batch contains no operations.
    pub fn is_empty(&self) -> bool {
        self.puts.is_empty() && self.deletes.is_empty()
    }

    /// Estimated memory usage of this batch in bytes.
    pub fn estimated_memory(&self) -> usize {
        self.puts
            .iter()
            .map(|(k, v)| k.len() + v.len())
            .sum::<usize>()
            + self.deletes.iter().map(|k| k.len()).sum::<usize>()
    }
}

impl Default for CoinsBatch {
    fn default() -> Self {
        Self::new()
    }
}

/// Flush a CoinsBatch to a database backend.
///
/// Generic over the database type via the `Database` trait.
pub fn flush_coins_batch<D: qubitcoin_storage::Database>(
    db: &D,
    mut batch: CoinsBatch,
    config: &FlushConfig,
) -> Result<FlushStats, String> {
    use qubitcoin_storage::DbBatch;

    let start = Instant::now();

    if config.sort_keys {
        batch.sort_keys();
    }

    let mut db_batch = db.new_batch();
    let mut bytes_written = 0usize;

    for (key, value) in &batch.puts {
        db_batch.put(key, value);
        bytes_written += key.len() + value.len();
    }

    for key in &batch.deletes {
        db_batch.delete(key);
    }

    db.write_batch(db_batch, config.sync_writes)
        .map_err(|e| format!("flush failed: {}", e))?;

    Ok(FlushStats::from_flush(
        batch.puts.len(),
        batch.deletes.len(),
        bytes_written,
        start,
    ))
}

/// A flush scheduler that determines when to trigger flushes based on
/// cache pressure.
pub struct FlushScheduler {
    config: FlushConfig,
    /// Estimated current cache size in entries.
    current_entries: usize,
    /// Estimated current memory usage.
    current_memory: usize,
    /// Total flushes performed.
    total_flushes: u64,
    /// Accumulated flush statistics.
    total_stats: FlushStats,
}

impl FlushScheduler {
    /// Create a new `FlushScheduler` with the given configuration.
    pub fn new(config: FlushConfig) -> Self {
        FlushScheduler {
            config,
            current_entries: 0,
            current_memory: 0,
            total_flushes: 0,
            total_stats: FlushStats::default(),
        }
    }

    /// Update the current cache state.
    pub fn update_cache_state(&mut self, entries: usize, memory: usize) {
        self.current_entries = entries;
        self.current_memory = memory;
    }

    /// Check if a flush should be triggered.
    pub fn should_flush(&self) -> bool {
        self.current_entries >= self.config.max_entries
            || self.current_memory >= self.config.max_memory_bytes
    }

    /// Record a completed flush.
    pub fn record_flush(&mut self, stats: &FlushStats) {
        self.total_flushes += 1;
        self.total_stats.entries_written += stats.entries_written;
        self.total_stats.entries_deleted += stats.entries_deleted;
        self.total_stats.bytes_written += stats.bytes_written;
        self.total_stats.elapsed_us += stats.elapsed_us;
        self.current_entries = 0;
        self.current_memory = 0;
    }

    /// Get total number of flushes performed.
    pub fn total_flushes(&self) -> u64 {
        self.total_flushes
    }

    /// Get accumulated statistics.
    pub fn total_stats(&self) -> &FlushStats {
        &self.total_stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flush_config_default() {
        let config = FlushConfig::default();
        assert_eq!(config.max_entries, 500_000);
        assert!(config.sort_keys);
        assert!(!config.sync_writes);
    }

    #[test]
    fn test_coins_batch_empty() {
        let batch = CoinsBatch::new();
        assert!(batch.is_empty());
        assert_eq!(batch.len(), 0);
    }

    #[test]
    fn test_coins_batch_puts() {
        let mut batch = CoinsBatch::new();
        batch.puts.push((b"key1".to_vec(), b"val1".to_vec()));
        batch.puts.push((b"key2".to_vec(), b"val2".to_vec()));
        assert_eq!(batch.len(), 2);
        assert!(!batch.is_empty());
    }

    #[test]
    fn test_coins_batch_sort() {
        let mut batch = CoinsBatch::new();
        batch.puts.push((b"c".to_vec(), b"3".to_vec()));
        batch.puts.push((b"a".to_vec(), b"1".to_vec()));
        batch.puts.push((b"b".to_vec(), b"2".to_vec()));
        batch.sort_keys();
        assert_eq!(batch.puts[0].0, b"a");
        assert_eq!(batch.puts[1].0, b"b");
        assert_eq!(batch.puts[2].0, b"c");
    }

    #[test]
    fn test_coins_batch_memory() {
        let mut batch = CoinsBatch::new();
        batch.puts.push((vec![0; 100], vec![0; 200]));
        batch.deletes.push(vec![0; 50]);
        assert_eq!(batch.estimated_memory(), 350);
    }

    #[test]
    fn test_flush_stats() {
        let start = Instant::now();
        std::thread::sleep(std::time::Duration::from_millis(1));
        let stats = FlushStats::from_flush(100, 10, 5000, start);
        assert_eq!(stats.entries_written, 100);
        assert_eq!(stats.entries_deleted, 10);
        assert_eq!(stats.bytes_written, 5000);
        assert!(stats.elapsed_us > 0);
        assert!(stats.entries_per_sec > 0.0);
    }

    #[test]
    fn test_flush_scheduler_threshold() {
        let config = FlushConfig {
            max_entries: 100,
            max_memory_bytes: 1024,
            ..Default::default()
        };
        let mut scheduler = FlushScheduler::new(config);

        scheduler.update_cache_state(50, 512);
        assert!(!scheduler.should_flush());

        scheduler.update_cache_state(100, 512);
        assert!(scheduler.should_flush());

        scheduler.update_cache_state(50, 1024);
        assert!(scheduler.should_flush());
    }

    #[test]
    fn test_flush_scheduler_record() {
        let config = FlushConfig::default();
        let mut scheduler = FlushScheduler::new(config);

        let stats = FlushStats {
            entries_written: 100,
            entries_deleted: 10,
            bytes_written: 5000,
            elapsed_us: 1000,
            entries_per_sec: 110000.0,
        };

        scheduler.record_flush(&stats);
        assert_eq!(scheduler.total_flushes(), 1);
        assert_eq!(scheduler.total_stats().entries_written, 100);
    }

    #[test]
    fn test_flush_to_memory_db() {
        use qubitcoin_storage::memory::MemoryDb;
        use qubitcoin_storage::Database;

        let db = MemoryDb::new();
        let mut batch = CoinsBatch::new();
        batch
            .puts
            .push((b"utxo:aaa".to_vec(), b"coin_data_1".to_vec()));
        batch
            .puts
            .push((b"utxo:bbb".to_vec(), b"coin_data_2".to_vec()));
        batch
            .puts
            .push((b"utxo:ccc".to_vec(), b"coin_data_3".to_vec()));

        let config = FlushConfig::default();
        let stats = flush_coins_batch(&db, batch, &config).unwrap();

        assert_eq!(stats.entries_written, 3);
        assert_eq!(stats.entries_deleted, 0);

        // Verify data was written
        assert_eq!(db.read(b"utxo:aaa").unwrap(), Some(b"coin_data_1".to_vec()));
        assert_eq!(db.read(b"utxo:bbb").unwrap(), Some(b"coin_data_2".to_vec()));
        assert_eq!(db.read(b"utxo:ccc").unwrap(), Some(b"coin_data_3".to_vec()));
    }

    #[test]
    fn test_flush_with_deletes() {
        use qubitcoin_storage::memory::MemoryDb;
        use qubitcoin_storage::Database;
        use qubitcoin_storage::DbBatch;

        let db = MemoryDb::new();

        // Pre-populate
        let mut pre_batch = db.new_batch();
        pre_batch.put(b"key1", b"val1");
        pre_batch.put(b"key2", b"val2");
        db.write_batch(pre_batch, false).unwrap();

        // Flush with deletes
        let mut batch = CoinsBatch::new();
        batch.puts.push((b"key3".to_vec(), b"val3".to_vec()));
        batch.deletes.push(b"key1".to_vec());

        let config = FlushConfig::default();
        let stats = flush_coins_batch(&db, batch, &config).unwrap();

        assert_eq!(stats.entries_written, 1);
        assert_eq!(stats.entries_deleted, 1);
        assert!(db.read(b"key1").unwrap().is_none());
        assert_eq!(db.read(b"key2").unwrap(), Some(b"val2".to_vec()));
        assert_eq!(db.read(b"key3").unwrap(), Some(b"val3".to_vec()));
    }
}
