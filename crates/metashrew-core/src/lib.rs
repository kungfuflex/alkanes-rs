//! Metashrew Core - WebAssembly bindings for Bitcoin indexers
//!
//! This crate provides the core WebAssembly bindings and utilities for building
//! Bitcoin indexers that run within the Metashrew framework. It defines the
//! host-guest interface between the Metashrew runtime and WASM modules.
//!
//! # Architecture
//!
//! The core library implements the guest side of the WebAssembly interface:
//!
//! - **Host Functions**: Bindings to runtime-provided functions like `__get`, `__flush`, etc.
//! - **Memory Management**: Utilities for passing data between host and guest
//! - **Caching Layer**: In-memory cache for efficient key-value operations
//! - **Protocol Buffers**: Serialization for structured data exchange
//!
//! # Key Components
//!
//! ## Host Interface
//!
//! The library provides bindings to host functions that enable WASM modules to:
//! - Read blockchain data via [`input()`]
//! - Query the database via [`get()`]
//! - Write state changes via [`flush()`]
//! - Log debug information via [`stdout`]
//!
//! ## Memory Layout
//!
//! Data is exchanged using AssemblyScript's ArrayBuffer layout:
//! ```text
//! [4 bytes length][data bytes...]
//! ```
//!
//! ## Caching Strategy
//!
//! The library maintains an in-memory cache of database reads and pending writes:
//! - **Read Cache**: Avoids repeated host calls for the same key
//! - **Write Buffer**: Batches writes for efficient flushing
//! - **Automatic Management**: Cache is managed transparently
//!
//! # Usage
//!
//! WASM modules typically use this library by:
//!
//! 1. **Initialization**: Call [`initialize()`] to set up the cache
//! 2. **Input Processing**: Use [`input()`] to get block data
//! 3. **State Queries**: Use [`get()`] to read existing state
//! 4. **State Updates**: Use [`set()`] to update state
//! 5. **Commit Changes**: Call [`flush()`] to persist changes
//!
//! # Example
//!
//! ```rust,no_run
//! use metashrew_core::{initialize, input, get, set, flush};
//! use std::sync::Arc;
//!
//! // Initialize the cache system
//! initialize();
//!
//! // Get the input data (height + block)
//! let input_data = input();
//! let height = u32::from_le_bytes(input_data[0..4].try_into().unwrap());
//! let block_data = &input_data[4..];
//!
//! // Read existing state
//! let key = Arc::new(b"some_key".to_vec());
//! let existing_value = get(key.clone());
//!
//! // Update state
//! let new_value = Arc::new(b"new_value".to_vec());
//! set(key, new_value);
//!
//! // Commit all changes
//! flush();
//! ```

extern crate alloc;
use protobuf::Message;
use std::collections::HashMap;
#[allow(unused_imports)]
use std::fmt::Write;
#[cfg(feature = "panic-hook")]
use std::panic;
use std::sync::Arc;

#[cfg(feature = "panic-hook")]
pub mod compat;
pub mod imports;
pub mod index_pointer;
pub mod macros;
pub mod stdio;

#[cfg(feature = "panic-hook")]
use crate::compat::panic_hook;
use crate::imports::{__flush, __get, __get_len, __host_len, __load_input};
pub use crate::stdio::stdout;
#[allow(unused_imports)]
use metashrew_support::{
    compat::{to_arraybuffer_layout, to_passback_ptr, to_ptr},
    proto::metashrew::{IndexerMetadata, KeyValueFlush, ViewFunction},
};

/// Global cache for storing key-value pairs read from the database
///
/// This cache avoids repeated host calls for the same key during block processing.
/// It's automatically managed by the library and should not be accessed directly.
static mut CACHE: Option<HashMap<Arc<Vec<u8>>, Arc<Vec<u8>>>> = None;

/// Global buffer for tracking keys that need to be flushed to the database
///
/// This buffer accumulates all keys that have been modified during block processing
/// and need to be written back to the database when [`flush()`] is called.
static mut TO_FLUSH: Option<Vec<Arc<Vec<u8>>>> = None;

/// Get a reference to the internal cache
///
/// This function provides read-only access to the internal cache for debugging
/// or inspection purposes. The cache contains all key-value pairs that have
/// been read from or written to during the current block processing.
///
/// # Safety
///
/// This function accesses global mutable state and should only be called
/// after [`initialize()`] has been called.
///
/// # Returns
///
/// A reference to the internal cache HashMap.
#[allow(static_mut_refs)]
pub fn get_cache() -> &'static HashMap<Arc<Vec<u8>>, Arc<Vec<u8>>> {
    unsafe { CACHE.as_ref().unwrap() }
}

/// Get a value from the database with caching
///
/// This function retrieves a value for the given key, first checking the local
/// cache and only making a host call if the key is not cached. The result is
/// automatically cached for future lookups.
///
/// # Arguments
///
/// * `v` - The key to look up, wrapped in an Arc for efficient sharing
///
/// # Returns
///
/// The value associated with the key, or an empty Vec if the key doesn't exist.
/// The result is wrapped in an Arc for efficient sharing.
///
/// # Host Interface
///
/// This function calls the host's `__get_len` and `__get` functions to retrieve
/// data from the underlying database. The host functions use the AssemblyScript
/// ArrayBuffer memory layout.
///
/// # Example
///
/// ```rust,no_run
/// use metashrew_core::{initialize, get};
/// use std::sync::Arc;
///
/// initialize();
/// let key = Arc::new(b"my_key".to_vec());
/// let value = get(key);
/// ```
#[allow(static_mut_refs)]
pub fn get(v: Arc<Vec<u8>>) -> Arc<Vec<u8>> {
    unsafe {
        initialize();
        if CACHE.as_ref().unwrap().contains_key(&v.clone()) {
            return CACHE.as_ref().unwrap().get(&v.clone()).unwrap().clone();
        }
        let length: i32 = __get_len(to_passback_ptr(&mut to_arraybuffer_layout(v.as_ref())));
        let mut buffer = Vec::<u8>::new();
        buffer.extend_from_slice(&length.to_le_bytes());
        buffer.resize((length as usize) + 4, 0);
        __get(
            to_passback_ptr(&mut to_arraybuffer_layout(v.as_ref())),
            to_passback_ptr(&mut buffer),
        );
        let value = Arc::new(buffer[4..].to_vec());
        CACHE.as_mut().unwrap().insert(v.clone(), value.clone());
        value
    }
}

/// Set a value in the cache for later flushing to the database
///
/// This function stores a key-value pair in the local cache and marks the key
/// for flushing to the database when [`flush()`] is called. The value is not
/// immediately written to the database.
///
/// # Arguments
///
/// * `k` - The key to store, wrapped in an Arc for efficient sharing
/// * `v` - The value to associate with the key, wrapped in an Arc for efficient sharing
///
/// # Behavior
///
/// - The key-value pair is immediately available via [`get()`]
/// - The key is added to the flush queue for batch writing
/// - Multiple calls with the same key will overwrite the previous value
///
/// # Example
///
/// ```rust,no_run
/// use metashrew_core::{initialize, set, flush};
/// use std::sync::Arc;
///
/// initialize();
/// let key = Arc::new(b"my_key".to_vec());
/// let value = Arc::new(b"my_value".to_vec());
/// set(key, value);
/// flush(); // Actually write to database
/// ```
#[allow(static_mut_refs)]
pub fn set(k: Arc<Vec<u8>>, v: Arc<Vec<u8>>) {
    unsafe {
        initialize();
        CACHE.as_mut().unwrap().insert(k.clone(), v.clone());
        TO_FLUSH.as_mut().unwrap().push(k.clone());
    }
}

/// Flush all pending writes to the database
///
/// This function serializes all key-value pairs that have been set since the
/// last flush and sends them to the host for atomic writing to the database.
/// After flushing, the write queue is cleared.
///
/// # Protocol
///
/// The function uses Protocol Buffers to serialize the key-value pairs into
/// a [`KeyValueFlush`] message, which is then sent to the host via the
/// `__flush` function.
///
/// # Atomicity
///
/// All key-value pairs in a single flush operation are written atomically
/// by the host. Either all writes succeed or all fail.
///
/// # Example
///
/// ```rust,no_run
/// use metashrew_core::{initialize, set, flush};
/// use std::sync::Arc;
///
/// initialize();
///
/// // Set multiple values
/// set(Arc::new(b"key1".to_vec()), Arc::new(b"value1".to_vec()));
/// set(Arc::new(b"key2".to_vec()), Arc::new(b"value2".to_vec()));
///
/// // Flush all changes atomically
/// flush();
/// ```
#[allow(static_mut_refs)]
pub fn flush() {
    unsafe {
        initialize();
        let mut to_encode: Vec<Vec<u8>> = Vec::<Vec<u8>>::new();
        for item in TO_FLUSH.as_ref().unwrap() {
            to_encode.push((*item.clone()).clone());
            to_encode.push((*(CACHE.as_ref().unwrap().get(item).unwrap().clone())).clone());
        }
        TO_FLUSH = Some(Vec::<Arc<Vec<u8>>>::new());
        let mut buffer = KeyValueFlush::new();
        buffer.list = to_encode;
        let serialized = buffer.write_to_bytes().unwrap();
        __flush(to_ptr(&mut to_arraybuffer_layout(&serialized.to_vec())) + 4);
    }
}

/// Get the input data for the current block
///
/// This function retrieves the input data provided by the host, which typically
/// contains the block height (first 4 bytes) followed by the serialized block data.
/// The data format follows the standard Metashrew convention.
///
/// # Returns
///
/// A `Vec<u8>` containing the complete input data. The first 4 bytes represent
/// the block height in little-endian format, followed by the block data.
///
/// # Host Interface
///
/// This function calls the host's `__host_len` and `__load_input` functions
/// to retrieve the input data using the AssemblyScript ArrayBuffer memory layout.
///
/// # Example
///
/// ```rust,no_run
/// use metashrew_core::{initialize, input};
///
/// initialize();
/// let input_data = input();
///
/// // Extract height and block data
/// let height = u32::from_le_bytes(input_data[0..4].try_into().unwrap());
/// let block_data = &input_data[4..];
///
/// println!("Processing block {} with {} bytes", height, block_data.len());
/// ```
#[allow(unused_unsafe)]
pub fn input() -> Vec<u8> {
    initialize();
    unsafe {
        let length: i32 = __host_len().into();
        let mut buffer = Vec::<u8>::new();
        buffer.extend_from_slice(&length.to_le_bytes());
        buffer.resize((length as usize) + 4, 0);
        __load_input(to_ptr(&mut buffer) + 4);
        buffer[4..].to_vec()
    }
}

/// Initialize the cache and flush systems
///
/// This function sets up the global cache and flush queue if they haven't been
/// initialized yet. It's automatically called by other functions but can be
/// called explicitly to ensure initialization.
///
/// # Safety
///
/// This function modifies global mutable state and should be called before
/// any other cache operations.
///
/// # Panic Hook
///
/// When the "panic-hook" feature is enabled, this function also installs
/// a custom panic hook for better error reporting in the WASM environment.
///
/// # Example
///
/// ```rust
/// use metashrew_core::initialize;
///
/// // Explicitly initialize (optional, as other functions call this automatically)
/// initialize();
/// ```
#[allow(static_mut_refs)]
pub fn initialize() -> () {
    unsafe {
        if CACHE.is_none() {
            reset();
            CACHE = Some(HashMap::<Arc<Vec<u8>>, Arc<Vec<u8>>>::new());
            #[cfg(feature = "panic-hook")]
            panic::set_hook(Box::new(panic_hook));
        }
    }
}

/// Export bytes to the host with proper length prefix
///
/// This function prepares data for return to the host by adding the required
/// length prefix according to the AssemblyScript ArrayBuffer memory layout.
/// It's typically used by view functions to return results.
///
/// # Arguments
///
/// * `bytes` - The data to export to the host
///
/// # Returns
///
/// A pointer to the buffer containing the length-prefixed data. The host
/// can use this pointer to read the data from WASM memory.
///
/// # Memory Layout
///
/// The returned buffer has the format:
/// ```text
/// [4 bytes length (little-endian)][data bytes...]
/// ```
///
/// # Example
///
/// ```rust,no_run
/// use metashrew_core::export_bytes;
///
/// let result_data = b"Hello, host!".to_vec();
/// let ptr = export_bytes(result_data);
/// // Return ptr from your view function
/// ```
pub fn export_bytes(bytes: Vec<u8>) -> i32 {
    // Create a buffer with the length prefix
    let mut buffer = Vec::with_capacity(bytes.len() + 4);
    let len = bytes.len() as u32;
    buffer.extend_from_slice(&len.to_le_bytes());
    buffer.extend_from_slice(&bytes);

    // Return a pointer to the buffer
    to_ptr(&mut buffer)
}

/// Reset the flush queue
///
/// This function clears the flush queue, removing all pending writes without
/// flushing them to the database. This is primarily used internally for
/// initialization and testing.
///
/// # Safety
///
/// This function modifies global mutable state and should be used with caution.
/// Any pending writes will be lost.
///
/// # Usage
///
/// This is typically called internally by [`initialize()`] and [`clear()`].
/// Most applications should not call this directly.
pub fn reset() -> () {
    unsafe {
        TO_FLUSH = Some(Vec::<Arc<Vec<u8>>>::new());
    }
}

/// Clear both the cache and flush queue
///
/// This function completely resets the cache system, clearing both the
/// read cache and the write queue. All cached data and pending writes
/// are lost.
///
/// # Safety
///
/// This function modifies global mutable state and should be used with caution.
/// Any cached data and pending writes will be lost.
///
/// # Usage
///
/// This is primarily used for testing or when you need to completely
/// reset the cache state. Most applications should not need to call this.
///
/// # Example
///
/// ```rust,no_run
/// use metashrew_core::{initialize, set, clear};
/// use std::sync::Arc;
///
/// initialize();
/// set(Arc::new(b"key".to_vec()), Arc::new(b"value".to_vec()));
///
/// // Clear everything
/// clear();
///
/// // Cache is now empty and reinitialized
/// ```
pub fn clear() -> () {
    unsafe {
        reset();
        CACHE = Some(HashMap::<Arc<Vec<u8>>, Arc<Vec<u8>>>::new());
    }
}
