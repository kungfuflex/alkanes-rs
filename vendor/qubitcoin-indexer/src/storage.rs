//! RocksDB storage for a single indexer instance.
//!
//! Implements the append-only key-value model where each logical key
//! accumulates values over time, enabling rollback by height.

use crate::state;
use std::collections::HashMap;
use std::path::Path;

/// Storage backend for a single indexer, wrapping a dedicated RocksDB instance.
pub struct IndexerStorage {
    db: rocksdb::DB,
    /// Cached ReadOptions with checksum verification disabled for hot-path
    /// reads. Saves ~6% CPU from XXH3 hash computation on every read.
    fast_read_opts: rocksdb::ReadOptions,
}

impl IndexerStorage {
    /// Open (or create) a RocksDB database at `path` with optimized settings.
    ///
    /// Tuned for the metashrew append-only workload pattern based on profiling
    /// that identified bloom filter I/O, memory copying, and page faults as
    /// the primary bottlenecks.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let mut opts = rocksdb::Options::default();
        opts.create_if_missing(true);

        // --- Memory ---
        // Large write buffers reduce write stalls during block processing.
        opts.set_write_buffer_size(256 * 1024 * 1024); // 256 MB per memtable
        opts.set_max_write_buffer_number(4);
        opts.set_min_write_buffer_number_to_merge(2);

        // --- Block cache ---
        // 25% of available memory (min 512 MB, max 4 GB per indexer).
        let cache_bytes = get_available_memory_bytes()
            .map(|m| (m / 4).clamp(512 * 1024 * 1024, 4 * 1024 * 1024 * 1024))
            .unwrap_or(2 * 1024 * 1024 * 1024);
        let cache = rocksdb::Cache::new_lru_cache(cache_bytes);

        let mut block_opts = rocksdb::BlockBasedOptions::default();
        block_opts.set_block_cache(&cache);
        // Larger blocks reduce bloom filter I/O overhead.
        block_opts.set_block_size(256 * 1024); // 256 KB
        // Pin index/filter blocks in cache to avoid repeated I/O.
        block_opts.set_cache_index_and_filter_blocks(true);
        block_opts.set_pin_l0_filter_and_index_blocks_in_cache(true);
        // 20 bits/key bloom filter for fewer false positives.
        block_opts.set_bloom_filter(20.0, false);
        block_opts.set_whole_key_filtering(true);
        block_opts.set_format_version(5);
        opts.set_block_based_table_factory(&block_opts);

        // --- Compaction ---
        opts.set_compaction_style(rocksdb::DBCompactionStyle::Level);
        opts.set_level_compaction_dynamic_level_bytes(true);
        opts.set_level_zero_file_num_compaction_trigger(8);
        opts.set_level_zero_slowdown_writes_trigger(20);
        opts.set_level_zero_stop_writes_trigger(36);
        opts.set_target_file_size_base(256 * 1024 * 1024); // 256 MB

        // --- Per-level compression: none for hot, LZ4 for warm, Zstd for cold ---
        opts.set_compression_per_level(&[
            rocksdb::DBCompressionType::None,
            rocksdb::DBCompressionType::None,
            rocksdb::DBCompressionType::Lz4,
            rocksdb::DBCompressionType::Zstd,
            rocksdb::DBCompressionType::Zstd,
            rocksdb::DBCompressionType::Zstd,
            rocksdb::DBCompressionType::Zstd,
        ]);

        // --- Parallelism ---
        let cpus = std::thread::available_parallelism()
            .map(|n| n.get() as i32)
            .unwrap_or(4);
        opts.set_max_background_jobs(cpus.max(4));
        opts.set_max_open_files(4096);

        // --- Write optimizations ---
        opts.set_bytes_per_sync(16 * 1024 * 1024);
        opts.set_wal_bytes_per_sync(16 * 1024 * 1024);
        opts.set_allow_concurrent_memtable_write(true);
        opts.set_enable_write_thread_adaptive_yield(true);

        let db = rocksdb::DB::open(&opts, path).map_err(|e| format!("indexer db open: {}", e))?;

        // Fast reads: skip checksum verification (saves ~6% CPU).
        // Data integrity is guaranteed by the write path and compaction.
        let mut fast_read_opts = rocksdb::ReadOptions::default();
        fast_read_opts.set_verify_checksums(false);

        Ok(IndexerStorage { db, fast_read_opts })
    }

    /// Raw get (uses fast reads with checksum verification disabled).
    pub fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        self.db.get_opt(key, &self.fast_read_opts).ok().flatten()
    }

    /// Raw put (single key).
    pub fn put(&self, key: &[u8], value: &[u8]) -> Result<(), String> {
        self.db
            .put(key, value)
            .map_err(|e| format!("indexer db put: {}", e))
    }

    /// Write a batch of key-value pairs atomically.
    pub fn write_batch(&self, pairs: &[(Vec<u8>, Vec<u8>)]) -> Result<(), String> {
        let mut batch = rocksdb::WriteBatch::default();
        for (k, v) in pairs {
            batch.put(k, v);
        }
        self.db
            .write(batch)
            .map_err(|e| format!("indexer db batch: {}", e))
    }

    /// Append a value to the append-only list for `key` at `height`.
    ///
    /// Updates length, stores value at the next index, and records the height.
    pub fn append(&self, key: &[u8], value: &[u8], height: u32) -> Result<(), String> {
        let len_key = state::length_key(key);
        let current_len = self.get_u32(&len_key).unwrap_or(0);

        let idx_key = state::index_key(key, current_len);
        let h_key = state::entry_height_key(key, current_len);

        let mut batch = rocksdb::WriteBatch::default();
        batch.put(&len_key, &(current_len + 1).to_le_bytes());
        batch.put(&idx_key, value);
        batch.put(&h_key, &height.to_le_bytes());
        self.db
            .write(batch)
            .map_err(|e| format!("indexer db append: {}", e))
    }

    /// Atomically append all key-value pairs from a block and record the
    /// per-height key set for efficient rollback.
    ///
    /// If `block_hash8` is provided (first 8 bytes of the block hash),
    /// it is stored alongside the height in each entry's height record,
    /// enabling deferred rollback canonicity checks.
    pub fn append_batch(
        &self,
        pairs: &[(Vec<u8>, Vec<u8>)],
        height: u32,
    ) -> Result<(), String> {
        self.append_batch_with_hash(pairs, height, None)
    }

    /// Like `append_batch` but also records a block hash prefix for
    /// deferred rollback canonicity validation.
    pub fn append_batch_with_hash(
        &self,
        pairs: &[(Vec<u8>, Vec<u8>)],
        height: u32,
        block_hash8: Option<&[u8]>,
    ) -> Result<(), String> {
        if pairs.is_empty() {
            return Ok(());
        }

        let mut batch = rocksdb::WriteBatch::default();

        // Track in-flight length updates for keys that appear multiple times
        // in the same block (the DB read won't see uncommitted batch writes).
        let mut length_cache: HashMap<Vec<u8>, u32> = HashMap::new();

        // Collect the set of logical keys modified at this height.
        let mut modified_keys: Vec<Vec<u8>> = Vec::with_capacity(pairs.len());

        // Height record: height_le32 [++ blockhash8] for canonicity checks.
        let height_record = match block_hash8 {
            Some(hash) if hash.len() >= 8 => {
                let mut rec = Vec::with_capacity(12);
                rec.extend_from_slice(&height.to_le_bytes());
                rec.extend_from_slice(&hash[..8]);
                rec
            }
            _ => height.to_le_bytes().to_vec(),
        };

        for (key, value) in pairs {
            let len_key = state::length_key(key);
            let current_len = length_cache
                .get(key)
                .copied()
                .unwrap_or_else(|| self.get_u32(&len_key).unwrap_or(0));

            batch.put(&len_key, &(current_len + 1).to_le_bytes());
            batch.put(&state::index_key(key, current_len), value);
            batch.put(
                &state::entry_height_key(key, current_len),
                &height_record,
            );

            length_cache.insert(key.clone(), current_len + 1);
            modified_keys.push(key.clone());
        }

        // Write the per-height key set for fast rollback.
        let keyset_key = state::height_keyset_key(height);
        let keyset_data = state::encode_key_set(&modified_keys);
        batch.put(&keyset_key, &keyset_data);

        // Store canonical hash mapping for this height.
        if let Some(hash) = block_hash8 {
            if hash.len() >= 8 {
                batch.put(&state::height_to_hash_key(height), &hash[..8]);
            }
        }

        // Update tip height in the same batch.
        batch.put(state::HEIGHT_KEY, &height.to_le_bytes());

        self.db
            .write(batch)
            .map_err(|e| format!("indexer db append_batch: {}", e))
    }

    /// Write a raw RocksDB WriteBatch.
    pub fn write_raw_batch(&self, batch: rocksdb::WriteBatch) -> Result<(), String> {
        self.db
            .write(batch)
            .map_err(|e| format!("indexer db raw batch: {}", e))
    }

    /// Get the latest value for a logical key (last entry in the append list).
    pub fn get_latest(&self, key: &[u8]) -> Option<Vec<u8>> {
        let len_key = state::length_key(key);
        let len = self.get_u32(&len_key)?;
        if len == 0 {
            return None;
        }
        let idx_key = state::index_key(key, len - 1);
        self.get(&idx_key)
    }

    /// Get the value at a specific index.
    pub fn get_at_index(&self, key: &[u8], index: u32) -> Option<Vec<u8>> {
        let idx_key = state::index_key(key, index);
        self.get(&idx_key)
    }

    /// Get the length of the append list for a key.
    pub fn get_length(&self, key: &[u8]) -> u32 {
        let len_key = state::length_key(key);
        self.get_u32(&len_key).unwrap_or(0)
    }

    /// Read a u32 from a key.
    pub fn get_u32(&self, key: &[u8]) -> Option<u32> {
        let data = self.get(key)?;
        if data.len() >= 4 {
            Some(u32::from_le_bytes([data[0], data[1], data[2], data[3]]))
        } else {
            None
        }
    }

    /// Get the stored indexer tip height.
    pub fn tip_height(&self) -> u32 {
        self.get_u32(state::HEIGHT_KEY).unwrap_or(0)
    }

    /// Set the indexer tip height.
    pub fn set_tip_height(&self, height: u32) -> Result<(), String> {
        self.put(state::HEIGHT_KEY, &height.to_le_bytes())
    }

    // -----------------------------------------------------------------------
    // Deferred rollback / reorg support
    // -----------------------------------------------------------------------

    /// Store the canonical block hash (first 8 bytes) for a height.
    pub fn set_canonical_hash(&self, height: u32, hash8: &[u8]) -> Result<(), String> {
        self.put(&state::height_to_hash_key(height), hash8)
    }

    /// Get the canonical block hash prefix for a height.
    pub fn get_canonical_hash(&self, height: u32) -> Option<Vec<u8>> {
        self.get(&state::height_to_hash_key(height))
    }

    /// Set the reorg height marker. Uses min(existing, new) to handle
    /// cascading reorgs — the marker only moves backwards.
    pub fn set_reorg_height(&self, height: u32) -> Result<(), String> {
        let current = self.reorg_height();
        let effective = match current {
            Some(existing) => existing.min(height),
            None => height,
        };
        self.put(state::REORG_HEIGHT_KEY, &effective.to_le_bytes())
    }

    /// Get the reorg height marker, or `None` if no reorg is pending.
    pub fn reorg_height(&self) -> Option<u32> {
        self.get_u32(state::REORG_HEIGHT_KEY)
    }

    /// Clear the reorg height marker (after background pruning completes).
    pub fn clear_reorg_height(&self) -> Result<(), String> {
        self.db
            .delete(state::REORG_HEIGHT_KEY)
            .map_err(|e| format!("indexer db delete reorg height: {}", e))
    }

    /// Reorg-aware read: get the latest canonical value for a key.
    ///
    /// If no reorg is pending (reorg_height is None), uses the fast path.
    /// Otherwise, walks backward through entries validating each against
    /// the canonical block hash for its height.
    pub fn get_latest_canonical(&self, key: &[u8]) -> Option<Vec<u8>> {
        let reorg_h = self.reorg_height();
        let len = self.get_length(key);
        if len == 0 {
            return None;
        }

        match reorg_h {
            None => {
                // Fast path: no reorg pending, latest entry is canonical.
                self.get_at_index(key, len - 1)
            }
            Some(rh) => {
                // Walk backward, skip entries at heights >= rh that aren't canonical.
                for idx in (0..len).rev() {
                    let h_key = state::entry_height_key(key, idx);
                    if let Some(h_data) = self.get(&h_key) {
                        if h_data.len() >= 4 {
                            let entry_height = u32::from_le_bytes([
                                h_data[0], h_data[1], h_data[2], h_data[3],
                            ]);
                            if entry_height < rh {
                                // Below reorg boundary — always canonical.
                                return self.get_at_index(key, idx);
                            }
                            // At or above reorg boundary — check canonical hash.
                            // If we have a blockhash8 stored in the height data
                            // (bytes 4..12), validate it.
                            if h_data.len() >= 12 {
                                let entry_hash8 = &h_data[4..12];
                                if let Some(canonical) =
                                    self.get_canonical_hash(entry_height)
                                {
                                    if canonical.len() >= 8
                                        && &canonical[..8] == entry_hash8
                                    {
                                        return self.get_at_index(key, idx);
                                    }
                                    // Non-canonical: skip this entry.
                                    continue;
                                }
                            }
                            // No blockhash8 in height data (old format) — treat as canonical.
                            return self.get_at_index(key, idx);
                        }
                    }
                    // No height data — treat as canonical (legacy).
                    return self.get_at_index(key, idx);
                }
                None
            }
        }
    }

    /// Delete a range of keys using a WriteBatch.
    pub fn delete_batch(&self, keys: &[Vec<u8>]) -> Result<(), String> {
        let mut batch = rocksdb::WriteBatch::default();
        for k in keys {
            batch.delete(k);
        }
        self.db
            .write(batch)
            .map_err(|e| format!("indexer db delete batch: {}", e))
    }

    /// Create a raw RocksDB iterator.
    pub fn raw_iterator(&self) -> rocksdb::DBIterator<'_> {
        self.db.iterator(rocksdb::IteratorMode::Start)
    }

    /// Prefix iterator for scanning keys.
    pub fn prefix_iterator(&self, prefix: &[u8]) -> rocksdb::DBIterator<'_> {
        self.db.prefix_iterator(prefix)
    }

    /// Flush WAL to ensure durability.
    pub fn flush(&self) -> Result<(), String> {
        self.db
            .flush()
            .map_err(|e| format!("indexer db flush: {}", e))
    }
}

/// Get available system memory in bytes (Linux only, fallback 16 GB).
fn get_available_memory_bytes() -> Option<usize> {
    #[cfg(target_os = "linux")]
    {
        if let Ok(meminfo) = std::fs::read_to_string("/proc/meminfo") {
            for line in meminfo.lines() {
                if line.starts_with("MemAvailable:") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        if let Ok(kb) = parts[1].parse::<usize>() {
                            return Some(kb * 1024);
                        }
                    }
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_storage() -> (IndexerStorage, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let storage = IndexerStorage::open(dir.path()).unwrap();
        (storage, dir)
    }

    #[test]
    fn test_put_get() {
        let (storage, _dir) = temp_storage();
        storage.put(b"key1", b"value1").unwrap();
        assert_eq!(storage.get(b"key1"), Some(b"value1".to_vec()));
    }

    #[test]
    fn test_get_missing() {
        let (storage, _dir) = temp_storage();
        assert_eq!(storage.get(b"nonexistent"), None);
    }

    #[test]
    fn test_write_batch() {
        let (storage, _dir) = temp_storage();
        let pairs = vec![
            (b"k1".to_vec(), b"v1".to_vec()),
            (b"k2".to_vec(), b"v2".to_vec()),
            (b"k3".to_vec(), b"v3".to_vec()),
        ];
        storage.write_batch(&pairs).unwrap();
        assert_eq!(storage.get(b"k1"), Some(b"v1".to_vec()));
        assert_eq!(storage.get(b"k2"), Some(b"v2".to_vec()));
        assert_eq!(storage.get(b"k3"), Some(b"v3".to_vec()));
    }

    #[test]
    fn test_append_and_get_latest() {
        let (storage, _dir) = temp_storage();
        storage.append(b"log", b"first", 100).unwrap();
        assert_eq!(storage.get_latest(b"log"), Some(b"first".to_vec()));
        assert_eq!(storage.get_length(b"log"), 1);

        storage.append(b"log", b"second", 101).unwrap();
        assert_eq!(storage.get_latest(b"log"), Some(b"second".to_vec()));
        assert_eq!(storage.get_length(b"log"), 2);

        assert_eq!(storage.get_at_index(b"log", 0), Some(b"first".to_vec()));
        assert_eq!(storage.get_at_index(b"log", 1), Some(b"second".to_vec()));
    }

    #[test]
    fn test_get_latest_empty() {
        let (storage, _dir) = temp_storage();
        assert_eq!(storage.get_latest(b"empty"), None);
        assert_eq!(storage.get_length(b"empty"), 0);
    }

    #[test]
    fn test_tip_height() {
        let (storage, _dir) = temp_storage();
        assert_eq!(storage.tip_height(), 0);
        storage.set_tip_height(500).unwrap();
        assert_eq!(storage.tip_height(), 500);
    }

    #[test]
    fn test_append_batch_atomic() {
        let (storage, _dir) = temp_storage();

        let pairs = vec![
            (b"alpha".to_vec(), b"a1".to_vec()),
            (b"beta".to_vec(), b"b1".to_vec()),
            (b"alpha".to_vec(), b"a2".to_vec()),
        ];
        storage.append_batch(&pairs, 10).unwrap();

        // alpha had two appends in the same batch — lengths accumulate.
        // First append: alpha len 0→1, second: alpha len 1→2.
        assert_eq!(storage.get_length(b"alpha"), 2);
        assert_eq!(storage.get_length(b"beta"), 1);
        assert_eq!(storage.get_at_index(b"alpha", 0), Some(b"a1".to_vec()));
        assert_eq!(storage.get_at_index(b"alpha", 1), Some(b"a2".to_vec()));
        assert_eq!(storage.tip_height(), 10);

        // Per-height keyset was recorded.
        let keyset_key = state::height_keyset_key(10);
        assert!(storage.get(&keyset_key).is_some());
    }

    #[test]
    fn test_append_batch_empty() {
        let (storage, _dir) = temp_storage();
        storage.append_batch(&[], 5).unwrap();
        assert_eq!(storage.tip_height(), 0); // no change
    }

    #[test]
    fn test_delete_batch() {
        let (storage, _dir) = temp_storage();
        storage.put(b"a", b"1").unwrap();
        storage.put(b"b", b"2").unwrap();
        storage.put(b"c", b"3").unwrap();

        storage
            .delete_batch(&[b"a".to_vec(), b"c".to_vec()])
            .unwrap();

        assert_eq!(storage.get(b"a"), None);
        assert_eq!(storage.get(b"b"), Some(b"2".to_vec()));
        assert_eq!(storage.get(b"c"), None);
    }

    #[test]
    fn test_multiple_keys_append() {
        let (storage, _dir) = temp_storage();
        storage.append(b"key_a", b"val_a1", 1).unwrap();
        storage.append(b"key_b", b"val_b1", 1).unwrap();
        storage.append(b"key_a", b"val_a2", 2).unwrap();

        assert_eq!(storage.get_length(b"key_a"), 2);
        assert_eq!(storage.get_length(b"key_b"), 1);
        assert_eq!(storage.get_latest(b"key_a"), Some(b"val_a2".to_vec()));
        assert_eq!(storage.get_latest(b"key_b"), Some(b"val_b1".to_vec()));
    }

    #[test]
    fn test_get_u32() {
        let (storage, _dir) = temp_storage();
        storage.put(b"num", &42u32.to_le_bytes()).unwrap();
        assert_eq!(storage.get_u32(b"num"), Some(42));
    }

    #[test]
    fn test_get_u32_missing() {
        let (storage, _dir) = temp_storage();
        assert_eq!(storage.get_u32(b"missing"), None);
    }

    // -- Reorg / canonical hash tests --------------------------------------

    #[test]
    fn test_append_batch_with_hash_stores_hash() {
        let (storage, _dir) = temp_storage();
        let hash8 = b"ABCDEFGH";
        storage
            .append_batch_with_hash(&[(b"key".to_vec(), b"val".to_vec())], 10, Some(hash8))
            .unwrap();
        // Canonical hash should be stored.
        let stored = storage.get_canonical_hash(10).unwrap();
        assert_eq!(&stored[..8], hash8);
        // Height record should include the hash (12 bytes: 4 height + 8 hash).
        let h_key = state::entry_height_key(b"key", 0);
        let h_data = storage.get(&h_key).unwrap();
        assert_eq!(h_data.len(), 12);
        assert_eq!(&h_data[4..12], hash8);
    }

    #[test]
    fn test_append_batch_with_hash_none_no_hash() {
        let (storage, _dir) = temp_storage();
        storage
            .append_batch_with_hash(&[(b"key".to_vec(), b"val".to_vec())], 10, None)
            .unwrap();
        // No canonical hash stored.
        assert!(storage.get_canonical_hash(10).is_none());
        // Height record should be 4 bytes only.
        let h_key = state::entry_height_key(b"key", 0);
        let h_data = storage.get(&h_key).unwrap();
        assert_eq!(h_data.len(), 4);
    }

    #[test]
    fn test_set_get_canonical_hash() {
        let (storage, _dir) = temp_storage();
        storage.set_canonical_hash(100, b"12345678").unwrap();
        let hash = storage.get_canonical_hash(100).unwrap();
        assert_eq!(&hash, b"12345678");
    }

    #[test]
    fn test_get_canonical_hash_missing() {
        let (storage, _dir) = temp_storage();
        assert!(storage.get_canonical_hash(999).is_none());
    }

    #[test]
    fn test_set_reorg_height_initial() {
        let (storage, _dir) = temp_storage();
        assert!(storage.reorg_height().is_none());
        storage.set_reorg_height(50).unwrap();
        assert_eq!(storage.reorg_height(), Some(50));
    }

    #[test]
    fn test_set_reorg_height_uses_min() {
        let (storage, _dir) = temp_storage();
        storage.set_reorg_height(100).unwrap();
        assert_eq!(storage.reorg_height(), Some(100));
        // Setting a higher value should keep the existing min.
        storage.set_reorg_height(200).unwrap();
        assert_eq!(storage.reorg_height(), Some(100));
        // Setting a lower value should update.
        storage.set_reorg_height(50).unwrap();
        assert_eq!(storage.reorg_height(), Some(50));
    }

    #[test]
    fn test_clear_reorg_height() {
        let (storage, _dir) = temp_storage();
        storage.set_reorg_height(100).unwrap();
        assert!(storage.reorg_height().is_some());
        storage.clear_reorg_height().unwrap();
        assert!(storage.reorg_height().is_none());
    }

    #[test]
    fn test_get_latest_canonical_fast_path() {
        let (storage, _dir) = temp_storage();
        // No reorg — fast path returns latest.
        storage.append(b"key", b"val1", 10).unwrap();
        storage.append(b"key", b"val2", 20).unwrap();
        assert_eq!(
            storage.get_latest_canonical(b"key"),
            Some(b"val2".to_vec())
        );
    }

    #[test]
    fn test_get_latest_canonical_filters_orphaned() {
        let (storage, _dir) = temp_storage();
        let hash_a = b"AAAAAAAA";
        let hash_b = b"BBBBBBBB";

        // Block 10 with hash A.
        storage
            .append_batch_with_hash(&[(b"k".to_vec(), b"v10".to_vec())], 10, Some(hash_a))
            .unwrap();
        // Block 20 with hash B.
        storage
            .append_batch_with_hash(&[(b"k".to_vec(), b"v20".to_vec())], 20, Some(hash_b))
            .unwrap();

        // Set reorg at height 16.
        storage.set_reorg_height(16).unwrap();
        // Set canonical hash for height 20 to something different.
        storage.set_canonical_hash(20, b"CCCCCCCC").unwrap();

        // Should skip v20 (non-canonical) and return v10.
        assert_eq!(
            storage.get_latest_canonical(b"k"),
            Some(b"v10".to_vec())
        );
    }

    #[test]
    fn test_get_latest_canonical_old_format_always_canonical() {
        let (storage, _dir) = temp_storage();
        // Use plain append (no hash in height record — old format).
        storage.append(b"key", b"old_val", 10).unwrap();
        storage.set_reorg_height(5).unwrap();
        // Old format entries are always treated as canonical.
        assert_eq!(
            storage.get_latest_canonical(b"key"),
            Some(b"old_val".to_vec())
        );
    }

    #[test]
    fn test_get_latest_canonical_empty_key() {
        let (storage, _dir) = temp_storage();
        assert!(storage.get_latest_canonical(b"empty").is_none());
    }
}
