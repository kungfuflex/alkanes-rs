//! Storage and runtime traits for indexer adapters.
//!
//! Both the native (wasmtime + RocksDB) and web (js-sys + HashMap) adapters
//! implement these traits, allowing shared logic in this crate.

/// Read-only access to indexer key-value storage.
pub trait IndexerStorageReader {
    /// Raw get by key.
    fn get(&self, key: &[u8]) -> Option<Vec<u8>>;

    /// Get the latest value for a logical key (last entry in the append list).
    fn get_latest(&self, key: &[u8]) -> Option<Vec<u8>>;

    /// Get the length of the append list for a key.
    fn get_length(&self, key: &[u8]) -> u32;

    /// Get the stored indexer tip height.
    fn tip_height(&self) -> u32;
}

/// Write access to indexer key-value storage.
pub trait IndexerStorageWriter: IndexerStorageReader {
    /// Raw put (single key).
    fn put(&self, key: &[u8], value: &[u8]) -> Result<(), String>;

    /// Append a value to the append-only list for `key` at `height`.
    fn append(&self, key: &[u8], value: &[u8], height: u32) -> Result<(), String>;

    /// Set the indexer tip height.
    fn set_tip_height(&self, height: u32) -> Result<(), String>;

    /// Delete a list of keys.
    fn delete_batch(&self, keys: &[Vec<u8>]) -> Result<(), String>;
}

/// Combined storage trait for backends that support both read and write,
/// plus export/import and key enumeration for rollback/stateRoot.
///
/// This is the primary trait used by `DevnetState` storage fields.
/// Both `WebIndexerStorage` and `ExternalStorage` implement this.
pub trait IndexerStorage: IndexerStorageWriter {
    /// Export all key-value pairs as a flat binary blob.
    fn export_bytes(&self) -> Vec<u8>;

    /// Import key-value pairs from a flat binary blob, replacing all existing data.
    fn import_bytes(&self, data: &[u8]) -> Result<usize, String>;

    /// Get all logical keys and their append-list lengths (for rollback/stateRoot).
    fn keys_with_lengths(&self) -> Vec<(Vec<u8>, u32)>;
}

/// A WASM indexer runtime that can process blocks and handle view calls.
pub trait IndexerRuntime {
    /// Run a block through the indexer, returning key-value pairs to flush.
    fn run_block(&self, input: Vec<u8>) -> Result<Vec<(Vec<u8>, Vec<u8>)>, String>;

    /// Call a view function, returning the result bytes.
    fn call_view(&self, fn_name: &str, input: Vec<u8>) -> Result<Vec<u8>, String>;
}
