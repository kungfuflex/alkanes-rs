//! Database abstraction traits.
//!
//! Maps to: `src/dbwrapper.h` (`CDBWrapper` interface) in Bitcoin Core.
//!
//! These traits define the contract for key-value storage backends used
//! throughout Qubitcoin, including UTXO databases, block indexes, and more.

/// A key-value database backend.
///
/// Equivalent to the `CDBWrapper` interface in Bitcoin Core's `src/dbwrapper.h`.
/// Implementations must be thread-safe (`Send + Sync`).
pub trait Database: Send + Sync {
    /// The write-batch type produced by [`new_batch`](Database::new_batch).
    type Batch: DbBatch;
    /// The iterator type produced by [`new_iterator`](Database::new_iterator).
    type Iterator<'a>: DbIterator
    where
        Self: 'a;
    /// The error type returned by fallible operations.
    type Error: std::error::Error + Send + Sync + 'static;

    /// Read a value by key. Returns None if key doesn't exist.
    fn read(&self, key: &[u8]) -> Result<Option<Vec<u8>>, Self::Error>;

    /// Read multiple keys in a single batch operation.
    /// Returns results in the same order as the input keys.
    fn multi_read(&self, keys: &[&[u8]]) -> Vec<Result<Option<Vec<u8>>, Self::Error>> {
        keys.iter().map(|k| self.read(k)).collect()
    }

    /// Check if a key exists.
    fn exists(&self, key: &[u8]) -> Result<bool, Self::Error>;

    /// Write a batch of operations atomically.
    fn write_batch(&self, batch: Self::Batch, sync: bool) -> Result<(), Self::Error>;

    /// Create a new write batch.
    fn new_batch(&self) -> Self::Batch;

    /// Create a new iterator over all key-value pairs.
    fn new_iterator(&self) -> Self::Iterator<'_>;

    /// Compact the database (optimize storage).
    fn compact(&self) -> Result<(), Self::Error>;

    /// Estimate the size of the database in bytes.
    fn estimated_size(&self) -> Result<u64, Self::Error> {
        Ok(0)
    }
}

/// A batch of write operations to be applied atomically.
pub trait DbBatch {
    /// Put a key-value pair in the batch.
    fn put(&mut self, key: &[u8], value: &[u8]);

    /// Delete a key from the batch.
    fn delete(&mut self, key: &[u8]);

    /// Clear all operations in the batch.
    fn clear(&mut self);
}

/// An iterator over database key-value pairs.
pub trait DbIterator {
    /// Seek to the first key >= the given key.
    fn seek(&mut self, key: &[u8]);

    /// Seek to the first key.
    fn seek_to_first(&mut self);

    /// Check if the iterator is at a valid position.
    fn valid(&self) -> bool;

    /// Move to the next key-value pair.
    fn next(&mut self);

    /// Get the current key (panics if !valid).
    fn key(&self) -> &[u8];

    /// Get the current value (panics if !valid).
    fn value(&self) -> &[u8];
}
