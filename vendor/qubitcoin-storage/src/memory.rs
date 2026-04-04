//! In-memory database implementation backed by `BTreeMap`.
//!
//! Used for testing -- no disk I/O required. All operations are
//! infallible and thread-safe via `parking_lot::RwLock`.

use crate::traits::{Database, DbBatch, DbIterator};
use parking_lot::RwLock;
use std::collections::BTreeMap;

/// Error type for [`MemoryDb`].
///
/// `MemoryDb` operations are infallible, but the [`Database`] trait requires
/// an associated error type. This enum is uninhabited.
#[derive(Debug, thiserror::Error)]
pub enum MemoryDbError {
    // MemoryDb operations are infallible, but we need an error type for the trait.
}

/// In-memory key-value database backed by a `BTreeMap`.
///
/// Thread-safe via `RwLock`. Suitable for testing and in-memory blockchain operation.
/// Equivalent to using a `CDBWrapper` with an in-memory backend in Bitcoin Core.
pub struct MemoryDb {
    /// The in-memory sorted key-value store.
    data: RwLock<BTreeMap<Vec<u8>, Vec<u8>>>,
}

impl MemoryDb {
    /// Create a new, empty `MemoryDb`.
    pub fn new() -> Self {
        MemoryDb {
            data: RwLock::new(BTreeMap::new()),
        }
    }
}

impl Default for MemoryDb {
    fn default() -> Self {
        Self::new()
    }
}

impl Database for MemoryDb {
    type Batch = MemoryBatch;
    type Iterator<'a> = MemoryIterator;
    type Error = MemoryDbError;

    fn read(&self, key: &[u8]) -> Result<Option<Vec<u8>>, Self::Error> {
        let data = self.data.read();
        Ok(data.get(key).cloned())
    }

    fn exists(&self, key: &[u8]) -> Result<bool, Self::Error> {
        let data = self.data.read();
        Ok(data.contains_key(key))
    }

    fn write_batch(&self, batch: Self::Batch, _sync: bool) -> Result<(), Self::Error> {
        let mut data = self.data.write();
        for op in batch.ops {
            match op {
                BatchOp::Put(k, v) => {
                    data.insert(k, v);
                }
                BatchOp::Delete(k) => {
                    data.remove(&k);
                }
            }
        }
        Ok(())
    }

    fn new_batch(&self) -> Self::Batch {
        MemoryBatch { ops: Vec::new() }
    }

    fn new_iterator(&self) -> Self::Iterator<'_> {
        let data = self.data.read();
        let entries: Vec<(Vec<u8>, Vec<u8>)> =
            data.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        MemoryIterator {
            entries,
            pos: 0,
            valid: false,
        }
    }

    fn compact(&self) -> Result<(), Self::Error> {
        // No-op for in-memory
        Ok(())
    }

    fn estimated_size(&self) -> Result<u64, Self::Error> {
        let data = self.data.read();
        let size: usize = data.iter().map(|(k, v)| k.len() + v.len()).sum();
        Ok(size as u64)
    }
}

enum BatchOp {
    Put(Vec<u8>, Vec<u8>),
    Delete(Vec<u8>),
}

/// Write batch for [`MemoryDb`].
///
/// Collects put and delete operations to be applied atomically.
pub struct MemoryBatch {
    /// Ordered list of pending operations.
    ops: Vec<BatchOp>,
}

impl DbBatch for MemoryBatch {
    fn put(&mut self, key: &[u8], value: &[u8]) {
        self.ops.push(BatchOp::Put(key.to_vec(), value.to_vec()));
    }

    fn delete(&mut self, key: &[u8]) {
        self.ops.push(BatchOp::Delete(key.to_vec()));
    }

    fn clear(&mut self) {
        self.ops.clear();
    }
}

/// Iterator over [`MemoryDb`] entries.
///
/// Takes a snapshot of all entries at construction time, so mutations to
/// the database after the iterator is created are not visible.
pub struct MemoryIterator {
    /// Snapshot of all key-value pairs, sorted by key.
    entries: Vec<(Vec<u8>, Vec<u8>)>,
    /// Current cursor position within `entries`.
    pos: usize,
    /// Whether the iterator is positioned at a valid entry.
    valid: bool,
}

impl DbIterator for MemoryIterator {
    fn seek(&mut self, key: &[u8]) {
        self.pos = self.entries.partition_point(|(k, _)| k.as_slice() < key);
        self.valid = self.pos < self.entries.len();
    }

    fn seek_to_first(&mut self) {
        self.pos = 0;
        self.valid = !self.entries.is_empty();
    }

    fn valid(&self) -> bool {
        self.valid
    }

    fn next(&mut self) {
        if self.valid {
            self.pos += 1;
            self.valid = self.pos < self.entries.len();
        }
    }

    fn key(&self) -> &[u8] {
        &self.entries[self.pos].0
    }

    fn value(&self) -> &[u8] {
        &self.entries[self.pos].1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_write() {
        let db = MemoryDb::new();
        let mut batch = db.new_batch();
        batch.put(b"key1", b"value1");
        batch.put(b"key2", b"value2");
        db.write_batch(batch, false).unwrap();

        assert_eq!(db.read(b"key1").unwrap(), Some(b"value1".to_vec()));
        assert_eq!(db.read(b"key2").unwrap(), Some(b"value2".to_vec()));
        assert_eq!(db.read(b"key3").unwrap(), None);
    }

    #[test]
    fn test_exists() {
        let db = MemoryDb::new();
        let mut batch = db.new_batch();
        batch.put(b"key1", b"value1");
        db.write_batch(batch, false).unwrap();

        assert!(db.exists(b"key1").unwrap());
        assert!(!db.exists(b"key2").unwrap());
    }

    #[test]
    fn test_delete() {
        let db = MemoryDb::new();

        let mut batch = db.new_batch();
        batch.put(b"key1", b"value1");
        db.write_batch(batch, false).unwrap();

        let mut batch = db.new_batch();
        batch.delete(b"key1");
        db.write_batch(batch, false).unwrap();

        assert!(!db.exists(b"key1").unwrap());
    }

    #[test]
    fn test_iterator() {
        let db = MemoryDb::new();
        let mut batch = db.new_batch();
        batch.put(b"a", b"1");
        batch.put(b"b", b"2");
        batch.put(b"c", b"3");
        db.write_batch(batch, false).unwrap();

        let mut iter = db.new_iterator();
        iter.seek_to_first();
        assert!(iter.valid());
        assert_eq!(iter.key(), b"a");
        assert_eq!(iter.value(), b"1");

        iter.next();
        assert_eq!(iter.key(), b"b");

        iter.next();
        assert_eq!(iter.key(), b"c");

        iter.next();
        assert!(!iter.valid());
    }

    #[test]
    fn test_iterator_seek() {
        let db = MemoryDb::new();
        let mut batch = db.new_batch();
        batch.put(b"a", b"1");
        batch.put(b"c", b"3");
        batch.put(b"e", b"5");
        db.write_batch(batch, false).unwrap();

        let mut iter = db.new_iterator();
        iter.seek(b"b");
        assert!(iter.valid());
        assert_eq!(iter.key(), b"c"); // first key >= "b"
    }
}
