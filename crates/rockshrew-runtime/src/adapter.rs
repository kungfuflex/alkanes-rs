//! RocksDB implementation of KeyValueStoreLike trait

use anyhow::Result;
use metashrew_runtime::{
    BatchLike, KVTrackerFn, KeyValueStoreLike, TIP_HEIGHT_KEY,
};
use rocksdb::{Options, WriteBatch, WriteBatchIterator, DB};
use std::sync::{Arc, Mutex};

/// Optimized labeled key creation that avoids unnecessary allocations
#[inline]
fn make_labeled_key_fast(key: &[u8]) -> Vec<u8> {
    if metashrew_runtime::has_label() {
        let label = metashrew_runtime::get_label();
        let label_bytes = label.as_bytes();
        let mut result = Vec::with_capacity(label_bytes.len() + key.len());
        result.extend_from_slice(label_bytes);
        result.extend_from_slice(key);
        result
    } else {
        key.to_vec()
    }
}

#[derive(Clone)]
pub struct RocksDBRuntimeAdapter {
    pub db: Arc<DB>,
    pub fork_db: Option<Arc<DB>>,
    pub height: u32,
    pub kv_tracker: Arc<Mutex<Option<KVTrackerFn>>>,
}

impl RocksDBRuntimeAdapter {
    /// Create a new adapter from an existing DB handle
    pub fn new(db: Arc<DB>) -> Self {
        RocksDBRuntimeAdapter {
            db,
            fork_db: None,
            height: 0,
            kv_tracker: Arc::new(Mutex::new(None)),
        }
    }

    pub fn open_fork(
        primary_path: String,
        fork_path: String,
        opts: Options,
    ) -> Result<RocksDBRuntimeAdapter> {
        let db = DB::open(&opts, primary_path)?;
        let fork_db = DB::open_for_read_only(&opts, fork_path, false)?;
        Ok(RocksDBRuntimeAdapter {
            db: Arc::new(db),
            fork_db: Some(Arc::new(fork_db)),
            height: 0,
            kv_tracker: Arc::new(Mutex::new(None)),
        })
    }

    pub fn open(path: String, opts: Options) -> Result<RocksDBRuntimeAdapter> {
        let db = DB::open(&opts, path)?;
        Ok(RocksDBRuntimeAdapter {
            db: Arc::new(db),
            fork_db: None,
            height: 0,
            kv_tracker: Arc::new(Mutex::new(None)),
        })
    }

    /// Open RocksDB with optimized configuration for metashrew workloads
    ///
    /// This uses performance-optimized settings based on profiling analysis that identified
    /// bloom filter and memory allocation bottlenecks as the primary performance issues.
    pub fn get_optimized_options() -> Options {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.set_write_buffer_size(256 * 1024 * 1024);
        opts.set_max_write_buffer_number(4);
        opts.increase_parallelism(num_cpus::get() as i32);
        opts
    }

    pub fn open_optimized(path: String) -> Result<RocksDBRuntimeAdapter> {
        let opts = Self::get_optimized_options();
        Self::open(path, opts)
    }

    /// Create a new adapter from an existing DB handle
    pub fn from_db(db: Arc<DB>) -> Self {
        RocksDBRuntimeAdapter {
            db,
            fork_db: None,
            height: 0,
            kv_tracker: Arc::new(Mutex::new(None)),
        }
    }

    /// Set a key-value tracker function that will be called for each key-value update
    pub fn set_kv_tracker(&mut self, tracker: Option<KVTrackerFn>) {
        if let Ok(mut guard) = self.kv_tracker.lock() {
            *guard = tracker;
        }
    }

    /// Track a key-value update using the registered tracker function
    pub fn track_kv_update_internal(&self, key: Vec<u8>, value: Vec<u8>) {
        if let Ok(guard) = self.kv_tracker.lock() {
            if let Some(tracker) = &*guard {
                tracker(key, value);
            }
        }
    }

    /// Create an atomic batch that includes all operations plus height update
    /// This ensures atomicity for block processing
    pub fn create_atomic_batch(&self, operations: RocksDBBatch) -> WriteBatch {
        let mut atomic_batch = WriteBatch::default();

        // Add the height update
        let height_key = TIP_HEIGHT_KEY.as_bytes();
        let height_bytes = (self.height + 1).to_le_bytes();
        let labeled_height_key = make_labeled_key_fast(height_key);
        atomic_batch.put(&labeled_height_key, &height_bytes);

        // Track operations and add them to the atomic batch
        let kv_tracker_clone = self.kv_tracker.clone();
        let mut batch_tracker = BatchTracker {
            inner_batch: &mut atomic_batch,
            kv_tracker: kv_tracker_clone,
        };

        // Use the batch tracker to capture key-value pairs for tracking
        operations.0.iterate(&mut batch_tracker);

        atomic_batch
    }

    /// Write an atomic batch to the database
    pub fn write_atomic_batch(&self, batch: WriteBatch) -> Result<(), rocksdb::Error> {
        self.db.write(batch)
    }
}

pub struct RocksDBBatch(pub WriteBatch);

impl BatchLike for RocksDBBatch {
    fn default() -> Self {
        Self(WriteBatch::default())
    }

    fn put<K: AsRef<[u8]>, V: AsRef<[u8]>>(&mut self, k: K, v: V) {
        let labeled_key = make_labeled_key_fast(k.as_ref());
        self.0.put(labeled_key, v);
    }

    fn delete<K: AsRef<[u8]>>(&mut self, k: K) {
        let labeled_key = make_labeled_key_fast(k.as_ref());
        self.0.delete(labeled_key);
    }
}

// BatchTracker captures key-value pairs during batch operations for tracking
pub struct BatchTracker<'a> {
    inner_batch: &'a mut WriteBatch,
    kv_tracker: Arc<Mutex<Option<KVTrackerFn>>>,
}

impl<'a> WriteBatchIterator for BatchTracker<'a> {
    fn put(&mut self, key: Box<[u8]>, value: Box<[u8]>) {
        // Forward to the inner batch first (more efficient)
        self.inner_batch.put(key.as_ref(), value.as_ref());

        // Track the key-value update if a tracker is registered
        // Only clone if we actually have a tracker to avoid unnecessary allocations
        if let Ok(guard) = self.kv_tracker.lock() {
            if let Some(tracker) = &*guard {
                // Only clone when we actually need to track
                tracker(key.to_vec(), value.to_vec());
            }
        }
    }

    fn delete(&mut self, key: Box<[u8]>) {
        // Forward to the inner batch
        self.inner_batch.delete(key.as_ref());
    }
}

impl KeyValueStoreLike for RocksDBRuntimeAdapter {
    type Batch = RocksDBBatch;
    type Error = rocksdb::Error;

    fn track_kv_update(&mut self, key: Vec<u8>, value: Vec<u8>) {
        self.track_kv_update_internal(key, value);
    }

    fn write(&mut self, batch: RocksDBBatch) -> Result<(), Self::Error> {
        // Create atomic batch with height update
        let atomic_batch = self.create_atomic_batch(batch);

        // Write atomically
        self.write_atomic_batch(atomic_batch)
    }

    fn get<K: AsRef<[u8]>>(&mut self, key: K) -> Result<Option<Vec<u8>>, Self::Error> {
        let labeled_key = make_labeled_key_fast(key.as_ref());
        match self.db.get(&labeled_key)? {
            Some(value) => Ok(Some(value.to_vec())),
            None => {
                if let Some(fork_db) = &self.fork_db {
                    fork_db.get(labeled_key).map(|opt| opt.map(|v| v.to_vec()))
                } else {
                    Ok(None)
                }
            }
        }
    }

    fn get_immutable<K: AsRef<[u8]>>(&self, key: K) -> Result<Option<Vec<u8>>, Self::Error> {
        let labeled_key = make_labeled_key_fast(key.as_ref());
        match self.db.get(&labeled_key)? {
            Some(value) => Ok(Some(value.to_vec())),
            None => {
                if let Some(fork_db) = &self.fork_db {
                    fork_db.get(labeled_key).map(|opt| opt.map(|v| v.to_vec()))
                } else {
                    Ok(None)
                }
            }
        }
    }

    fn delete<K: AsRef<[u8]>>(&mut self, key: K) -> Result<(), Self::Error> {
        let labeled_key = make_labeled_key_fast(key.as_ref());
        self.db.delete(labeled_key)
    }

    fn put<K: AsRef<[u8]>, V: AsRef<[u8]>>(&mut self, key: K, value: V) -> Result<(), Self::Error> {
        let key_slice = key.as_ref();
        let value_slice = value.as_ref();

        // Track the key-value update if a tracker is registered
        // Only clone if we actually have a tracker to avoid unnecessary allocations
        let should_track = if let Ok(guard) = self.kv_tracker.lock() {
            guard.is_some()
        } else {
            false
        };
        
        if should_track {
            self.track_kv_update(key_slice.to_vec(), value_slice.to_vec());
        }

        // Perform the actual database update
        let labeled_key = make_labeled_key_fast(key_slice);
        self.db.put(labeled_key, value_slice)
    }

    fn scan_prefix<K: AsRef<[u8]>>(
        &self,
        prefix: K,
    ) -> Result<Vec<(Vec<u8>, Vec<u8>)>, Self::Error> {
        let mut results = Vec::new();
        let prefix_bytes = make_labeled_key_fast(prefix.as_ref());

        let mut iter = self.db.raw_iterator();
        iter.seek(&prefix_bytes);

        while iter.valid() {
            if let Some(key) = iter.key() {
                if !key.starts_with(&prefix_bytes) {
                    break;
                }
                if let Some(value) = iter.value() {
                    results.push((key.to_vec(), value.to_vec()));
                }
            }
            iter.next();
        }

        Ok(results)
    }

    fn create_batch(&self) -> Self::Batch {
        RocksDBBatch::default()
    }

    fn keys<'a>(&'a self) -> Result<Box<dyn Iterator<Item = Vec<u8>> + 'a>, Self::Error> {
        let iter = self.db.iterator(rocksdb::IteratorMode::Start);
        Ok(Box::new(iter.map(|item| {
            let (key, _) = item.unwrap();
            key.to_vec()
        })))
    }

    fn is_open(&self) -> bool {
        true // RocksDB doesn't need connection management like Redis
    }

    fn set_height(&mut self, height: u32) {
        self.height = height;
    }

    fn get_height(&self) -> u32 {
        self.height
    }
}


/// Query height from RocksDB
pub async fn query_height(db: Arc<DB>, start_block: u32) -> Result<u32> {
    let height_key = TIP_HEIGHT_KEY.as_bytes();
    let labeled_key = make_labeled_key_fast(height_key);
    let bytes = match db.get(&labeled_key)? {
        Some(v) => v,
        None => {
            return Ok(start_block);
        }
    };
    if bytes.is_empty() {
        return Ok(start_block);
    }
    Ok(u32::from_le_bytes(bytes[..4].try_into().unwrap()))
}
/// Query height from a legacy RocksDB instance
pub async fn query_height_legacy(db: Arc<DB>, start_block: u32) -> Result<u32> {
    let height_key = TIP_HEIGHT_KEY.as_bytes();
    let labeled_key = if metashrew_runtime::has_label() {
        metashrew_runtime::to_labeled_key(&height_key.to_vec())
    } else {
        height_key.to_vec()
    };
    let bytes = match db.get(&labeled_key)? {
        Some(v) => v,
        None => {
            return Ok(start_block);
        }
    };
    if bytes.is_empty() {
        return Ok(start_block);
    }
    Ok(u32::from_le_bytes(bytes[..4].try_into().unwrap()))
}
