//! Core traits for generic key-value storage backends
//!
//! This module defines the fundamental traits that enable Metashrew to work with
//! different storage backends in a generic way. The primary traits are:
//!
//! - [`KeyValueStoreLike`]: The main storage interface that abstracts database operations
//! - [`BatchLike`]: Interface for atomic batch operations
//! - [`AtomicBlockResult`]: Result type for atomic block processing
//!
//! These traits enable dependency injection and allow Metashrew to work with
//! RocksDB, in-memory stores, or other key-value databases without changing
//! the core runtime logic.
//!
//! # Architecture
//!
//! The storage layer follows a generic design pattern where the runtime is
//! parameterized over a storage type `T: KeyValueStoreLike`. This allows:
//!
//! - **Testing**: Use in-memory stores for fast unit tests
//! - **Production**: Use RocksDB for persistent, high-performance storage
//! - **Future extensibility**: Add support for other databases like DynamoDB
//!
//! # Key-Value Tracking
//!
//! The storage layer includes optional key-value tracking functionality that
//! enables features like:
//!
//! - **Snapshots**: Track changes for efficient snapshot creation
//! - **Chain reorganizations**: Identify affected keys during reorgs
//! - **Auditing**: Maintain a complete history of state changes

use anyhow::Result;

/// Trait for atomic batch operations on key-value stores
///
/// This trait defines the interface for collecting multiple database operations
/// into a single atomic batch that can be committed all at once. This is essential
/// for maintaining database consistency during block processing.
///
/// # Examples
///
/// ```rust
/// use metashrew_runtime::traits::BatchLike;
///
/// fn example_batch_usage<B: BatchLike>(mut batch: B) {
///     // Add multiple operations to the batch
///     batch.put(b"key1", b"value1");
///     batch.put(b"key2", b"value2");
///     batch.delete(b"old_key");
///
///     // Batch will be committed when passed to storage.write()
/// }
/// ```
pub trait BatchLike {
    /// Add a key-value pair to the batch
    ///
    /// # Arguments
    ///
    /// * `key` - The key to store (any type that can be converted to bytes)
    /// * `value` - The value to store (any type that can be converted to bytes)
    fn put<K: AsRef<[u8]>, V: AsRef<[u8]>>(&mut self, key: K, value: V);

    /// Mark a key for deletion in the batch
    ///
    /// # Arguments
    ///
    /// * `key` - The key to delete (any type that can be converted to bytes)
    fn delete<K: AsRef<[u8]>>(&mut self, key: K);

    /// Create a new empty batch
    ///
    /// This is used to create a fresh batch for collecting operations.
    fn default() -> Self;
}

/// Generic trait for key-value storage backends
///
/// This is the core storage abstraction that allows Metashrew to work with
/// different database implementations. It provides both mutable and immutable
/// operations, batch processing, and specialized features needed for blockchain
/// indexing.
///
/// # Design Principles
///
/// - **Generic**: Works with any key-value store that implements this trait
/// - **Atomic**: Supports batch operations for consistency
/// - **Efficient**: Provides prefix scanning for range queries
/// - **Extensible**: Includes hooks for tracking and monitoring
///
/// # Implementation Notes
///
/// Implementors should ensure that:
/// - Operations are thread-safe when required
/// - Batch operations are atomic
/// - Prefix scanning is efficient for the underlying storage
/// - Error types provide meaningful debugging information
///
/// # Examples
///
/// ```rust
/// use metashrew_runtime::traits::{KeyValueStoreLike, BatchLike};
/// use anyhow::Result;
///
/// fn example_storage_usage<T: KeyValueStoreLike>(mut storage: T) -> Result<()> {
///     // Single operations
///     storage.put(b"key", b"value")?;
///     let value = storage.get(b"key")?;
///
///     // Batch operations
///     let mut batch = storage.create_batch();
///     batch.put(b"key1", b"value1");
///     batch.put(b"key2", b"value2");
///     storage.write(batch)?;
///
///     // Prefix scanning
///     let entries = storage.scan_prefix(b"prefix:")?;
///
///     Ok(())
/// }
/// ```
pub trait KeyValueStoreLike {
    /// The error type returned by storage operations
    ///
    /// This must implement standard error traits to enable proper error handling
    /// and debugging throughout the system.
    type Error: std::fmt::Debug + Send + Sync + std::error::Error + 'static;
    
    /// The batch type used for atomic operations
    ///
    /// This allows different storage backends to use their own optimized batch
    /// implementations while maintaining a consistent interface.
    type Batch: BatchLike;

    /// Write a batch of operations atomically to the store
    ///
    /// This is the primary method for committing multiple operations as a single
    /// atomic transaction. All operations in the batch either succeed together
    /// or fail together, ensuring database consistency.
    ///
    /// # Arguments
    ///
    /// * `batch` - The batch containing all operations to commit
    ///
    /// # Errors
    ///
    /// Returns an error if the batch cannot be written, which may indicate:
    /// - Storage system failure
    /// - Insufficient disk space
    /// - Corruption or consistency issues
    fn write(&mut self, batch: Self::Batch) -> Result<(), Self::Error>;

    /// Get a value by key (mutable version)
    ///
    /// This method allows mutable access to the storage, which may be required
    /// for some storage implementations that need to update internal state
    /// during read operations.
    ///
    /// # Arguments
    ///
    /// * `key` - The key to look up
    ///
    /// # Returns
    ///
    /// * `Ok(Some(value))` - If the key exists
    /// * `Ok(None)` - If the key does not exist
    /// * `Err(error)` - If there was a storage error
    fn get<K: AsRef<[u8]>>(&mut self, key: K) -> Result<Option<Vec<u8>>, Self::Error>;

    /// Get a value by key (immutable version)
    ///
    /// This is the preferred method for read-only operations as it doesn't
    /// require mutable access to the storage. Used extensively by view functions
    /// and other read-only operations.
    ///
    /// # Arguments
    ///
    /// * `key` - The key to look up
    ///
    /// # Returns
    ///
    /// * `Ok(Some(value))` - If the key exists
    /// * `Ok(None)` - If the key does not exist
    /// * `Err(error)` - If there was a storage error
    fn get_immutable<K: AsRef<[u8]>>(&self, key: K) -> Result<Option<Vec<u8>>, Self::Error>;

    /// Store a single key-value pair
    ///
    /// This is a convenience method for single operations. For multiple operations,
    /// prefer using batches for better performance and atomicity.
    ///
    /// # Arguments
    ///
    /// * `key` - The key to store
    /// * `value` - The value to associate with the key
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails due to storage issues.
    fn put<K, V>(&mut self, key: K, value: V) -> Result<(), Self::Error>
    where
        K: AsRef<[u8]>,
        V: AsRef<[u8]>;

    /// Delete a key from the store
    ///
    /// Removes the key and its associated value from the storage. If the key
    /// doesn't exist, this operation typically succeeds silently.
    ///
    /// # Arguments
    ///
    /// * `key` - The key to delete
    ///
    /// # Errors
    ///
    /// Returns an error if the deletion fails due to storage issues.
    fn delete<K: AsRef<[u8]>>(&mut self, key: K) -> Result<(), Self::Error>;

    /// Scan all key-value pairs with a given prefix
    ///
    /// This is essential for range queries and is heavily used by the append-only
    /// store for historical state queries. The implementation should
    /// be efficient for the underlying storage system.
    ///
    /// # Arguments
    ///
    /// * `prefix` - The prefix to search for
    ///
    /// # Returns
    ///
    /// A vector of (key, value) pairs where each key starts with the given prefix.
    /// The order may be implementation-dependent but should be consistent.
    ///
    /// # Performance Notes
    ///
    /// This operation can be expensive for large datasets. Implementations should
    /// optimize for common prefix patterns used by Metashrew.
    fn scan_prefix<K: AsRef<[u8]>>(
        &self,
        prefix: K,
    ) -> Result<Vec<(Vec<u8>, Vec<u8>)>, Self::Error>;

    /// Scan keys with a given prefix (mutable version for compatibility)
    ///
    /// This provides a mutable interface to prefix scanning for storage
    /// implementations that require mutable access. The default implementation
    /// delegates to the immutable version.
    ///
    /// # Arguments
    ///
    /// * `prefix` - The prefix to search for
    ///
    /// # Returns
    ///
    /// A vector of (key, value) pairs where each key starts with the given prefix.
    fn scan_prefix_mut<K: AsRef<[u8]>>(
        &mut self,
        prefix: K,
    ) -> Result<Vec<(Vec<u8>, Vec<u8>)>, Self::Error> {
        self.scan_prefix(prefix)
    }

    /// Create a new empty batch for collecting operations
    ///
    /// Batches are used to group multiple operations together for atomic
    /// execution. This is the preferred way to perform multiple related
    /// operations.
    ///
    /// # Returns
    ///
    /// A new empty batch that can be populated with operations.
    fn create_batch(&self) -> Self::Batch;

    /// Write a batch atomically (alias for write method)
    ///
    /// This is an alias for the `write` method provided for API consistency
    /// and clarity when working with batches.
    ///
    /// # Arguments
    ///
    /// * `batch` - The batch to write atomically
    fn write_batch(&mut self, batch: Self::Batch) -> Result<(), Self::Error> {
        self.write(batch)
    }

    /// Get an iterator over all keys in the store
    ///
    /// This is used for operations that need to examine all keys, such as
    /// database maintenance, statistics collection, or full scans.
    ///
    /// # Returns
    ///
    /// An iterator that yields all keys in the store. The order is
    /// implementation-dependent.
    ///
    /// # Performance Notes
    ///
    /// This operation can be very expensive for large databases. Use with caution
    /// and consider implementing pagination or filtering at the storage level.
    fn keys<'a>(&'a self) -> Result<Box<dyn Iterator<Item = Vec<u8>> + 'a>, Self::Error>;

    /// Track key-value updates for snapshots and monitoring
    ///
    /// This optional method allows storage implementations to track changes
    /// for features like snapshot creation, chain reorganization handling,
    /// and auditing. The default implementation does nothing.
    ///
    /// # Arguments
    ///
    /// * `key` - The key that was updated
    /// * `value` - The new value for the key
    ///
    /// # Usage
    ///
    /// This is called automatically by the runtime during block processing
    /// to track all state changes. Storage implementations can use this to:
    /// - Build change logs for snapshots
    /// - Maintain statistics
    /// - Implement custom monitoring
    fn track_kv_update(&mut self, _key: Vec<u8>, _value: Vec<u8>) {
        // Default implementation does nothing
    }

    /// Check if the storage connection is open and operational
    ///
    /// This can be used for health checks and connection management.
    /// The default implementation assumes the storage is always available.
    ///
    /// # Returns
    ///
    /// `true` if the storage is operational, `false` otherwise.
    fn is_open(&self) -> bool {
        true // Default implementation assumes always open
    }

    /// Set the current block height for height-aware operations
    ///
    /// Some storage implementations may need to track the current block height
    /// for features like height-indexed keys or automatic cleanup. The default
    /// implementation ignores this.
    ///
    /// # Arguments
    ///
    /// * `height` - The current block height being processed
    fn set_height(&mut self, _height: u32) {
        // Default implementation does nothing
    }

    /// Get the current block height
    ///
    /// Returns the current block height if the storage implementation tracks it.
    /// The default implementation returns 0.
    ///
    /// # Returns
    ///
    /// The current block height, or 0 if not tracked.
    fn get_height(&self) -> u32 {
        0 // Default implementation returns 0
    }

    /// Create an isolated copy for preview operations
    ///
    /// This creates a separate instance of the storage that can be used for
    /// preview operations without affecting the main storage state. Used by
    /// the preview functionality to test block processing without committing
    /// changes.
    ///
    /// # Returns
    ///
    /// An isolated copy of the storage that can be modified independently.
    ///
    /// # Default Implementation
    ///
    /// The default implementation simply clones the storage, which works for
    /// in-memory implementations but may not be suitable for all storage types.
    fn create_isolated_copy(&self) -> Self
    where
        Self: Clone,
    {
        self.clone()
    }

    fn set_kv_tracker(&mut self, _tracker: Option<KVTrackerFn>) {
        // Default implementation does nothing
    }
}

/// Type definition for key-value tracker function
///
/// This function type is used for tracking key-value updates in storage
/// implementations. It allows external components to monitor all changes
/// to the database for features like snapshots, auditing, and monitoring.
///
/// # Parameters
///
/// * First `Vec<u8>` - The key that was updated
/// * Second `Vec<u8>` - The new value for the key
///
/// # Thread Safety
///
/// The function must be `Send + Sync` to allow use across thread boundaries
/// in the async runtime environment.
///
/// # Example Usage
///
/// ```rust
/// use metashrew_runtime::traits::KVTrackerFn;
///
/// let tracker: KVTrackerFn = Box::new(|key, value| {
///     println!("Key updated: {} -> {}",
///         hex::encode(&key),
///         hex::encode(&value)
///     );
/// });
/// ```
pub type KVTrackerFn = Box<dyn Fn(Vec<u8>, Vec<u8>) + Send + Sync>;

/// Result of atomic block processing containing all operations and metadata
///
/// This structure encapsulates the complete result of processing a Bitcoin block
/// atomically, including the calculated state root, all database operations,
/// and block metadata. It enables atomic block processing where all operations
/// can be verified before committing to the database.
///
/// # Fields
///
/// * `state_root` - The cryptographic hash representing the complete state after processing
/// * `batch_data` - Serialized database operations that can be applied atomically
/// * `height` - The Bitcoin block height that was processed
/// * `block_hash` - The Bitcoin block hash for verification and tracking
///
/// # Usage
///
/// This is returned by `MetashrewRuntime::process_block_atomic` and can be used to:
/// - Verify state consistency before committing changes
/// - Create snapshots with verified state roots
/// - Implement rollback functionality for chain reorganizations
/// - Audit block processing results
///
/// # Example
///
/// ```rust
/// use metashrew_runtime::traits::AtomicBlockResult;
///
/// fn verify_block_result(result: &AtomicBlockResult) -> bool {
///     // Verify the state root matches expected value
///     !result.state_root.is_empty() &&
///     result.height > 0 &&
///     !result.block_hash.is_empty()
/// }
/// ```
#[derive(Debug, Clone)]
pub struct AtomicBlockResult {
    /// The calculated state root after block processing
    ///
    /// This is a cryptographic hash (typically SHA-256) that represents
    /// the complete state of the database after processing the block.
    /// It can be used for:
    /// - State verification and consistency checks
    /// - Snapshot metadata and integrity verification
    /// - Chain reorganization detection and handling
    pub state_root: Vec<u8>,
    
    /// Serialized batch data containing all database operations
    ///
    /// This contains all the key-value operations that were performed
    /// during block processing, serialized in a format that can be
    /// applied atomically to the database. The exact format depends
    /// on the storage implementation.
    pub batch_data: Vec<u8>,
    
    /// The block height that was processed
    ///
    /// This is the Bitcoin block height (block number) that was processed
    /// to generate this result. Used for ordering and verification.
    pub height: u32,
    
    /// The block hash
    ///
    /// This is the Bitcoin block hash (32 bytes) that uniquely identifies
    /// the block that was processed. Used for verification and tracking.
    pub block_hash: Vec<u8>,
}
