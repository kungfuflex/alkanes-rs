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

pub mod allocator;
#[cfg(feature = "panic-hook")]
pub mod compat;
pub mod imports;
pub mod index_pointer;
pub mod lazy_cell;
pub mod macros;
pub mod stdio;

// Re-export the procedural macros from metashrew-macros
pub use metashrew_macros::{main, view};

#[cfg(test)]
pub mod tests;

#[cfg(feature = "panic-hook")]
use crate::compat::panic_hook;
use crate::imports::{__flush, __get, __get_len, __host_len, __load_input};
pub use crate::stdio::stdout;
#[allow(unused_imports)]
use metashrew_support::{
    compat::{to_arraybuffer_layout, to_passback_ptr, to_ptr},
    lru_cache::{
        api_cache_get, api_cache_remove, api_cache_set, clear_lru_cache, clear_view_height,
        force_evict_to_target, force_evict_to_target_percentage, get_actual_lru_cache_memory_limit,
        get_cache_allocation_mode, get_cache_stats, get_height_partitioned_cache, get_lru_cache,
        get_min_lru_cache_memory_limit, get_total_memory_usage, get_view_height,
        initialize_lru_cache, is_cache_below_recommended_minimum, is_lru_cache_initialized,
        set_cache_allocation_mode, set_height_partitioned_cache, set_lru_cache, set_view_height,
        CacheAllocationMode, CacheStats, LruDebugStats, KeyPrefixStats, PrefixAnalysisConfig,
        key_parser,
    },
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

        // First check: immediate cache (CACHE)
        if CACHE.as_ref().unwrap().contains_key(&v.clone()) {
            return CACHE.as_ref().unwrap().get(&v.clone()).unwrap().clone();
        }

        // Second check: height-partitioned cache (for view functions) or main LRU cache
        if is_lru_cache_initialized() {
            // Check if we're in a view function with a specific height
            if let Some(height) = get_view_height() {
                // Use height-partitioned cache for view functions
                if let Some(cached_value) = get_height_partitioned_cache(height, &v) {
                    // Found in height-partitioned cache, populate immediate cache for faster subsequent access
                    CACHE
                        .as_mut()
                        .unwrap()
                        .insert(v.clone(), cached_value.clone());
                    return cached_value;
                }
            } else {
                // Use main LRU cache for indexer functions
                if let Some(cached_value) = get_lru_cache(&v) {
                    // Found in LRU cache, populate immediate cache for faster subsequent access
                    CACHE
                        .as_mut()
                        .unwrap()
                        .insert(v.clone(), cached_value.clone());
                    return cached_value;
                }
            }
        }

        // Third fallback: host calls (__get_len and __get)
        let length: i32 = __get_len(to_passback_ptr(&mut to_arraybuffer_layout(v.as_ref())));
        
        // CRITICAL FIX: Validate length to prevent capacity overflow
        // Reject negative lengths - this indicates corrupted host response
        if length < 0 {
            panic!("FATAL: Invalid negative length {} returned from __get_len for key: {:?}. This indicates corrupted host response and indexer must halt to prevent incorrect results.",
                   length, String::from_utf8_lossy(v.as_ref()));
        }
        
        // CRITICAL FIX: Handle large allocations with LRU cache eviction and retry
        const MAX_ALLOCATION_SIZE: usize = 512 * 1024 * 1024; // 512MB
        let length_usize = length as usize;
        let total_size = length_usize + 4;
        
        // First attempt: Try allocation normally
        let mut buffer = Vec::<u8>::new();
        let allocation_result = buffer.try_reserve_exact(total_size);
        
        match allocation_result {
            Ok(()) => {
                // Allocation succeeded, proceed normally
                buffer.extend_from_slice(&length.to_le_bytes());
                buffer.resize(total_size, 0);
            }
            Err(allocation_error) => {
                println!("WARNING: Initial allocation failed for {} bytes. Attempting LRU cache eviction and retry. Error: {:?}",
                         total_size, allocation_error);
                
                // Second attempt: Force LRU cache eviction to ~50% utilization and retry
                if is_lru_cache_initialized() {
                    println!("Forcing LRU cache eviction to 50% utilization to free memory...");
                    force_evict_to_target_percentage(50); // Evict to 50% of current usage
                    
                    // Retry allocation after eviction
                    let mut retry_buffer = Vec::<u8>::new();
                    match retry_buffer.try_reserve_exact(total_size) {
                        Ok(()) => {
                            println!("SUCCESS: Allocation succeeded after LRU cache eviction");
                            buffer = retry_buffer;
                            buffer.extend_from_slice(&length.to_le_bytes());
                            buffer.resize(total_size, 0);
                        }
                        Err(retry_error) => {
                            // Final failure: Panic to halt indexer and prevent incorrect results
                            panic!("FATAL: Failed to allocate {} bytes for buffer even after LRU cache eviction. Key: {:?}. Initial error: {:?}. Retry error: {:?}. Indexer must halt to prevent incorrect results.",
                                   total_size, String::from_utf8_lossy(v.as_ref()), allocation_error, retry_error);
                        }
                    }
                } else {
                    // No LRU cache available for eviction, check if size is unreasonable
                    if length_usize > MAX_ALLOCATION_SIZE {
                        panic!("FATAL: Requested allocation size {} bytes exceeds maximum reasonable size {} bytes for key: {:?}. This likely indicates corrupted host response. Indexer must halt to prevent incorrect results.",
                               length_usize, MAX_ALLOCATION_SIZE, String::from_utf8_lossy(v.as_ref()));
                    } else {
                        // Reasonable size but allocation failed - system memory issue
                        panic!("FATAL: Failed to allocate {} bytes for buffer (reasonable size but insufficient system memory). Key: {:?}. Error: {:?}. Indexer must halt to prevent incorrect results.",
                               total_size, String::from_utf8_lossy(v.as_ref()), allocation_error);
                    }
                }
            }
        };
        
        __get(
            to_passback_ptr(&mut to_arraybuffer_layout(v.as_ref())),
            to_passback_ptr(&mut buffer),
        );
        let value = Arc::new(buffer[4..].to_vec());

        // Populate caches with the retrieved value
        CACHE.as_mut().unwrap().insert(v.clone(), value.clone());
        if is_lru_cache_initialized() {
            if let Some(height) = get_view_height() {
                // Store in height-partitioned cache for view functions
                set_height_partitioned_cache(height, v.clone(), value.clone());
            } else {
                // Store in main LRU cache for indexer functions
                set_lru_cache(v.clone(), value.clone());
            }
        }

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

        // Also update LRU cache if initialized
        if is_lru_cache_initialized() {
            set_lru_cache(k.clone(), v.clone());
        }
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
        // Ensure initialization before proceeding
        if CACHE.is_none() || TO_FLUSH.is_none() {
            initialize();
        }

        let mut to_encode: Vec<Vec<u8>> = Vec::<Vec<u8>>::new();

        // Safely access TO_FLUSH and try to get values from CACHE first, then LRU cache
        if let Some(to_flush) = TO_FLUSH.as_ref() {
            for item in to_flush {
                let mut value_found = false;
                
                // First try immediate CACHE
                if let Some(cache) = CACHE.as_ref() {
                    if let Some(value) = cache.get(item) {
                        to_encode.push(item.as_ref().clone());
                        to_encode.push(value.as_ref().clone());
                        value_found = true;
                    }
                }
                
                // If not in immediate cache, try LRU cache (after flush_to_lru() was called)
                if !value_found && is_lru_cache_initialized() {
                    if let Some(value) = get_lru_cache(item) {
                        to_encode.push(item.as_ref().clone());
                        to_encode.push(value.as_ref().clone());
                        value_found = true;
                    }
                }
                
                // If still not found, this is an error condition
                if !value_found {
                    panic!("flush(): Key in TO_FLUSH not found in any cache: {:?}",
                           String::from_utf8_lossy(item.as_ref()));
                }
            }
        }

        // Reset flush queue
        TO_FLUSH = Some(Vec::<Arc<Vec<u8>>>::new());

        // Always call the host __flush function to ensure context state is set to 1
        // This is critical for proper indexer completion signaling
        let mut buffer = KeyValueFlush::new();
        buffer.list = to_encode;

        // Handle serialization errors gracefully
        match buffer.write_to_bytes() {
            Ok(serialized) => {
                // Always call host function, even with empty data
                let serialized_vec = serialized.to_vec();
                __flush(to_ptr(&mut to_arraybuffer_layout(&serialized_vec)) + 4);
            }
            Err(_) => {
                panic!("failed to serialize KeyValueFlush");
            }
        }

        // Always clear the immediate cache after flushing, regardless of success
        // This maintains consistency and prevents accumulation of stale data
        CACHE = Some(HashMap::<Arc<Vec<u8>>, Arc<Vec<u8>>>::new());
        
        // Force eviction if memory usage exceeds 1GB limit
        // This ensures we don't accumulate too much memory over time
        if is_lru_cache_initialized() {
            force_evict_to_target();
        }
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

    #[cfg(feature = "test-utils")]
    {
        // In test mode, return the mock input data directly
        use crate::imports::_INPUT;
        unsafe {
            match _INPUT.as_ref() {
                Some(v) => v.clone(),
                None => vec![],
            }
        }
    }

    #[cfg(not(feature = "test-utils"))]
    unsafe {
        let length: i32 = __host_len().into();
        
        // CRITICAL FIX: Validate length to prevent capacity overflow
        // Reject negative lengths - this indicates corrupted host response
        if length < 0 {
            panic!("FATAL: Invalid negative length {} returned from __host_len. This indicates corrupted host response and indexer must halt to prevent incorrect results.", length);
        }
        
        // CRITICAL FIX: Handle large allocations with LRU cache eviction and retry
        const MAX_ALLOCATION_SIZE: usize = 512 * 1024 * 1024; // 512MB
        let length_usize = length as usize;
        let total_size = length_usize + 4;
        
        // First attempt: Try allocation normally
        let mut buffer = Vec::<u8>::new();
        let allocation_result = buffer.try_reserve_exact(total_size);
        
        match allocation_result {
            Ok(()) => {
                // Allocation succeeded, proceed normally
                buffer.extend_from_slice(&length.to_le_bytes());
                buffer.resize(total_size, 0);
            }
            Err(allocation_error) => {
                println!("WARNING: Initial input allocation failed for {} bytes. Attempting LRU cache eviction and retry. Error: {:?}",
                         total_size, allocation_error);
                
                // Second attempt: Force LRU cache eviction to ~50% utilization and retry
                if is_lru_cache_initialized() {
                    println!("Forcing LRU cache eviction to 50% utilization to free memory...");
                    force_evict_to_target_percentage(50); // Evict to 50% of current usage
                    
                    // Retry allocation after eviction
                    let mut retry_buffer = Vec::<u8>::new();
                    match retry_buffer.try_reserve_exact(total_size) {
                        Ok(()) => {
                            println!("SUCCESS: Input allocation succeeded after LRU cache eviction");
                            buffer = retry_buffer;
                            buffer.extend_from_slice(&length.to_le_bytes());
                            buffer.resize(total_size, 0);
                        }
                        Err(retry_error) => {
                            // Final failure: Panic to halt indexer and prevent incorrect results
                            panic!("FATAL: Failed to allocate {} bytes for input buffer even after LRU cache eviction. Initial error: {:?}. Retry error: {:?}. Indexer must halt to prevent incorrect results.",
                                   total_size, allocation_error, retry_error);
                        }
                    }
                } else {
                    // No LRU cache available for eviction, check if size is unreasonable
                    if length_usize > MAX_ALLOCATION_SIZE {
                        panic!("FATAL: Requested input allocation size {} bytes exceeds maximum reasonable size {} bytes. This likely indicates corrupted host response. Indexer must halt to prevent incorrect results.",
                               length_usize, MAX_ALLOCATION_SIZE);
                    } else {
                        // Reasonable size but allocation failed - system memory issue
                        panic!("FATAL: Failed to allocate {} bytes for input buffer (reasonable size but insufficient system memory). Error: {:?}. Indexer must halt to prevent incorrect results.",
                               total_size, allocation_error);
                    }
                }
            }
        };
        
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
    // CRITICAL: Set cache mode to indexer for deterministic memory layout
    // metashrew-core is used for indexer operations which need consistent memory layout
    set_cache_allocation_mode(CacheAllocationMode::Indexer);
    
    // CRITICAL: Ensure LRU cache memory is preallocated FIRST (only in indexer mode)
    // This must happen before any other memory allocations to guarantee
    // consistent memory layout for WASM execution in indexer mode
    allocator::ensure_preallocated_memory();
    
    // Enable the preallocated allocator for deterministic memory layout
    allocator::enable_preallocated_allocator();
    println!("INFO: Enabled preallocated allocator for deterministic memory layout (indexer mode)");
    
    unsafe {
        if CACHE.is_none() {
            reset();
            CACHE = Some(HashMap::<Arc<Vec<u8>>, Arc<Vec<u8>>>::new());
            #[cfg(feature = "panic-hook")]
            panic::set_hook(Box::new(panic_hook));
        }
    }

    // Initialize LRU cache if not already initialized
    // This is safe to call multiple times
    initialize_lru_cache();
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

/// Flush CACHE contents to LRU_CACHE
///
/// This function moves all entries from the immediate CACHE to the persistent
/// LRU_CACHE and then clears the CACHE. This is called by the main indexer
/// function before flush() to ensure that cached values persist across blocks.
///
/// This function should NOT be called during view functions.
///
/// # Safety
///
/// This function modifies global mutable state and should only be called
/// from the main indexer function.
#[allow(static_mut_refs)]
pub fn flush_to_lru() {
    unsafe {
        initialize();

        // Only proceed if LRU cache is initialized and we're not in a view function
        if is_lru_cache_initialized() && get_view_height().is_none() {
            // Move all CACHE entries to LRU_CACHE
            if let Some(cache) = CACHE.as_ref() {
                let cache_size = cache.len();
                let total_data_size: usize = cache.iter()
                    .map(|(k, v)| k.len() + v.len())
                    .sum();
                
                println!("flush_to_lru: Moving {} items ({} bytes of data) from CACHE to LRU_CACHE",
                         cache_size, total_data_size);
                
                for (key, value) in cache.iter() {
                    set_lru_cache(key.clone(), value.clone());
                }
                
                // Log LRU cache stats after transfer
                let stats = lru_cache_stats();
                println!("flush_to_lru: LRU cache now has {} items, {} bytes memory usage",
                         stats.items, stats.memory_usage);
            }

            // DO NOT clear TO_FLUSH here - the subsequent flush() call needs it to write data to the database
            // Only clear the immediate cache since the data is now in LRU_CACHE
            // The flush() function will clear TO_FLUSH after writing to the database
            CACHE = Some(HashMap::<Arc<Vec<u8>>, Arc<Vec<u8>>>::new());
        }
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

    // Also clear LRU cache if initialized
    if is_lru_cache_initialized() {
        clear_lru_cache();
    }
}

/// Set the current view height for height-partitioned caching
///
/// This function sets the current view height, which causes subsequent get()
/// operations to use height-partitioned caching instead of the main LRU cache.
/// This is used by view functions to ensure cache isolation by block height.
///
/// # Arguments
///
/// * `height` - The block height to use for partitioned caching
pub fn set_view_for_height(height: u32) {
    set_view_height(height);
}

/// Clear the current view height and immediate cache
///
/// This function clears the current view height and clears the immediate CACHE,
/// causing subsequent get() operations to use the main LRU cache instead of
/// height-partitioned caching. This should be called at the end of view functions.
#[allow(static_mut_refs)]
pub fn clear_view_cache() {
    clear_view_height();

    // Clear the immediate cache for view functions
    unsafe {
        CACHE = Some(HashMap::<Arc<Vec<u8>>, Arc<Vec<u8>>>::new());
    }
}

// LRU Cache API Functions
// These functions provide access to the LRU cache system for WASM programs

/// Get cache statistics for monitoring and debugging
///
/// This function returns current cache statistics including hit/miss ratios,
/// memory usage, and eviction counts. Useful for monitoring cache performance.
///
/// # Returns
///
/// A `CacheStats` struct containing current cache metrics.
///
/// # Example
///
/// ```rust,no_run
/// use metashrew_core::{initialize, lru_cache_stats};
///
/// initialize();
/// let stats = lru_cache_stats();
/// println!("Cache hits: {}, misses: {}", stats.hits, stats.misses);
/// ```
pub fn lru_cache_stats() -> CacheStats {
    // Get the cached stats but update memory_usage with current actual usage
    let mut stats = get_cache_stats();
    stats.memory_usage = get_total_memory_usage();
    stats
}

/// Get the total memory usage of the LRU cache system
///
/// This function returns the total memory usage in bytes of both the main
/// LRU cache and the API cache.
///
/// # Returns
///
/// Total memory usage in bytes.
///
/// # Example
///
/// ```rust,no_run
/// use metashrew_core::{initialize, lru_cache_memory_usage};
///
/// initialize();
/// let memory_usage = lru_cache_memory_usage();
/// println!("LRU cache using {} bytes", memory_usage);
/// ```
pub fn lru_cache_memory_usage() -> usize {
    get_total_memory_usage()
}

/// Store a value in the API cache
///
/// This function allows WASM programs to cache arbitrary data using string keys.
/// The API cache shares the same memory limit as the main LRU cache but uses
/// a separate namespace to avoid conflicts with key-value store operations.
///
/// # Arguments
///
/// * `key` - A string key to identify the cached value
/// * `value` - The value to cache (as bytes)
///
/// # Example
///
/// ```rust,no_run
/// use metashrew_core::{initialize, cache_set};
/// use std::sync::Arc;
///
/// initialize();
/// let computed_result = Arc::new(b"expensive_computation_result".to_vec());
/// cache_set("computation_key".to_string(), computed_result);
/// ```
pub fn cache_set(key: String, value: Arc<Vec<u8>>) {
    initialize();
    api_cache_set(key, value);
}

/// Retrieve a value from the API cache
///
/// This function retrieves a previously cached value using its string key.
/// The access updates the LRU ordering for the item.
///
/// # Arguments
///
/// * `key` - The string key to look up
///
/// # Returns
///
/// `Some(value)` if the key exists in the cache, `None` otherwise.
///
/// # Example
///
/// ```rust,no_run
/// use metashrew_core::{initialize, cache_get};
///
/// initialize();
/// if let Some(cached_result) = cache_get("computation_key") {
///     println!("Found cached result: {:?}", cached_result);
/// } else {
///     println!("Cache miss, need to compute");
/// }
/// ```
pub fn cache_get(key: &str) -> Option<Arc<Vec<u8>>> {
    initialize();
    api_cache_get(key)
}

/// Remove a value from the API cache
///
/// This function removes a specific key-value pair from the API cache.
///
/// # Arguments
///
/// * `key` - The string key to remove
///
/// # Returns
///
/// `Some(value)` if the key existed and was removed, `None` if the key didn't exist.
///
/// # Example
///
/// ```rust,no_run
/// use metashrew_core::{initialize, cache_remove};
///
/// initialize();
/// if let Some(removed_value) = cache_remove("computation_key") {
///     println!("Removed cached value: {:?}", removed_value);
/// }
/// ```
pub fn cache_remove(key: &str) -> Option<Arc<Vec<u8>>> {
    initialize();
    api_cache_remove(key)
}

/// Check if the LRU cache system is initialized and available
///
/// This function returns true if the LRU cache system has been properly
/// initialized and is available for use.
///
/// # Returns
///
/// `true` if the LRU cache is available, `false` otherwise.
///
/// # Example
///
/// ```rust,no_run
/// use metashrew_core::{initialize, is_lru_cache_available};
///
/// initialize();
/// if is_lru_cache_available() {
///     println!("LRU cache is ready for use");
/// }
/// ```
pub fn is_lru_cache_available() -> bool {
    is_lru_cache_initialized()
}

/// Set the cache allocation mode
///
/// This function sets how memory should be allocated across the different caches.
/// - Indexer mode: All memory goes to main LRU cache
/// - View mode: Memory split between height-partitioned and API caches
///
/// # Arguments
///
/// * `mode` - The cache allocation mode to use
///
/// # Example
///
/// ```rust,no_run
/// use metashrew_core::{initialize, set_cache_mode};
/// use metashrew_support::lru_cache::CacheAllocationMode;
///
/// initialize();
/// set_cache_mode(CacheAllocationMode::View);
/// ```
pub fn set_cache_mode(mode: CacheAllocationMode) {
    set_cache_allocation_mode(mode);
}

/// Get the current cache allocation mode
///
/// # Returns
///
/// The current cache allocation mode.
///
/// # Example
///
/// ```rust,no_run
/// use metashrew_core::{initialize, get_cache_mode};
///
/// initialize();
/// let mode = get_cache_mode();
/// println!("Current cache mode: {:?}", mode);
/// ```
pub fn get_cache_mode() -> CacheAllocationMode {
    get_cache_allocation_mode()
}

/// Get the actual LRU cache memory limit determined at runtime
///
/// This function returns the memory limit that was determined based on available
/// system memory, which may be less than the ideal 1GB limit on resource-constrained systems.
/// This is useful for monitoring and debugging memory usage.
///
/// # Returns
///
/// The actual memory limit in bytes that will be used for LRU cache allocation.
///
/// # Example
///
/// ```rust,no_run
/// use metashrew_core::{initialize, get_actual_cache_memory_limit};
///
/// initialize();
/// let limit = get_actual_cache_memory_limit();
/// println!("LRU cache memory limit: {} bytes ({} MB)", limit, limit / (1024 * 1024));
/// ```
pub fn get_actual_cache_memory_limit() -> usize {
    get_actual_lru_cache_memory_limit()
}

/// Get the minimum recommended LRU cache memory limit
///
/// This function returns the minimum recommended memory size for optimal LRU cache
/// performance. Cache sizes below this threshold may result in degraded performance
/// due to frequent evictions.
///
/// # Returns
///
/// The minimum recommended memory limit in bytes (256MB).
///
/// # Example
///
/// ```rust,no_run
/// use metashrew_core::{initialize, get_min_cache_memory_limit};
///
/// initialize();
/// let min_limit = get_min_cache_memory_limit();
/// println!("Minimum recommended cache size: {} bytes ({} MB)", min_limit, min_limit / (1024 * 1024));
/// ```
pub fn get_min_cache_memory_limit() -> usize {
    get_min_lru_cache_memory_limit()
}

/// Check if the current cache is operating below the recommended minimum
///
/// This function compares the actual allocated cache size with the recommended
/// minimum and returns true if the cache is operating in a degraded mode.
///
/// # Returns
///
/// `true` if the cache size is below the recommended minimum, `false` otherwise.
///
/// # Example
///
/// ```rust,no_run
/// use metashrew_core::{initialize, is_cache_below_minimum};
///
/// initialize();
/// if is_cache_below_minimum() {
///     println!("⚠️  Cache is operating below recommended minimum - performance may be degraded");
/// } else {
///     println!("✅ Cache size is adequate");
/// }
/// ```
pub fn is_cache_below_minimum() -> bool {
    is_cache_below_recommended_minimum()
}

// LRU Cache Debugging API Functions

/// Enable LRU cache debugging mode
///
/// When enabled, the cache will track key prefix statistics for analysis.
/// This adds some overhead but provides valuable insights into cache usage patterns.
///
/// # Example
///
/// ```rust,no_run
/// use metashrew_core::{initialize, enable_lru_debug_mode, generate_lru_debug_report};
///
/// initialize();
/// enable_lru_debug_mode();
///
/// // ... perform cache operations ...
///
/// let report = generate_lru_debug_report();
/// println!("{}", report);
/// ```
pub fn enable_lru_debug_mode() {
    metashrew_support::lru_cache::enable_lru_debug_mode();
}

/// Disable LRU cache debugging mode
pub fn disable_lru_debug_mode() {
    metashrew_support::lru_cache::disable_lru_debug_mode();
}

/// Check if LRU cache debugging mode is enabled
pub fn is_lru_debug_mode_enabled() -> bool {
    metashrew_support::lru_cache::is_lru_debug_mode_enabled()
}

/// Set the prefix analysis configuration
///
/// # Arguments
///
/// * `config` - Configuration for prefix analysis including min/max prefix lengths
///
/// # Example
///
/// ```rust,no_run
/// use metashrew_core::{set_prefix_analysis_config, PrefixAnalysisConfig};
///
/// let config = PrefixAnalysisConfig {
///     min_prefix_length: 8,
///     max_prefix_length: 20,
///     min_keys_per_prefix: 3,
/// };
/// set_prefix_analysis_config(config);
/// ```
pub fn set_prefix_analysis_config(config: PrefixAnalysisConfig) {
    metashrew_support::lru_cache::set_prefix_analysis_config(config);
}

/// Get the current prefix analysis configuration
pub fn get_prefix_analysis_config() -> PrefixAnalysisConfig {
    metashrew_support::lru_cache::get_prefix_analysis_config()
}

/// Clear all prefix hit statistics
///
/// This resets all collected prefix statistics. Useful when you want to start
/// fresh analysis for a new period.
pub fn clear_prefix_hit_stats() {
    metashrew_support::lru_cache::clear_prefix_hit_stats();
}

/// Get comprehensive LRU debug statistics
///
/// Returns detailed statistics about cache usage and key prefix patterns.
///
/// # Returns
///
/// `LruDebugStats` containing overall cache stats and prefix analysis
pub fn get_lru_debug_stats() -> LruDebugStats {
    metashrew_support::lru_cache::get_lru_debug_stats()
}

/// Generate a formatted debug report
///
/// Creates a human-readable report of LRU cache usage patterns and key prefix analysis.
/// This is useful for understanding which parts of your key space are being accessed most.
///
/// # Returns
///
/// A formatted string containing the debug report
///
/// # Example
///
/// ```rust,no_run
/// use metashrew_core::{initialize, enable_lru_debug_mode, generate_lru_debug_report};
///
/// initialize();
/// enable_lru_debug_mode();
///
/// // ... perform cache operations ...
///
/// let report = generate_lru_debug_report();
/// println!("{}", report);
/// ```
pub fn generate_lru_debug_report() -> String {
    metashrew_support::lru_cache::generate_lru_debug_report()
}

/// Parse a cache key into human-readable format
///
/// This function intelligently parses cache keys that contain mixed UTF-8 and binary data,
/// formatting them in a human-readable way. Keys are expected to follow patterns like
/// "/path/segments/binary_data" where path segments are UTF-8 strings separated by '/'
/// and binary data is displayed as hexadecimal.
///
/// # Arguments
///
/// * `key` - The raw key bytes to parse
///
/// # Returns
///
/// A formatted string representation of the key
///
/// # Example
///
/// ```rust,no_run
/// use metashrew_core::parse_cache_key;
///
/// let key = b"/blockhash/byheight/\x01\x00\x00\x00";
/// let formatted = parse_cache_key(key);
/// // Result: "/blockhash/byheight/01000000"
/// ```
pub fn parse_cache_key(key: &[u8]) -> String {
    key_parser::parse_key_default(key)
}

/// Parse a cache key with enhanced pattern recognition
///
/// This function applies additional heuristics to detect common patterns like
/// little-endian integers, hash-like data, and timestamps.
///
/// # Arguments
///
/// * `key` - The raw key bytes to parse
///
/// # Returns
///
/// A formatted string with enhanced pattern recognition
///
/// # Example
///
/// ```rust,no_run
/// use metashrew_core::parse_cache_key_enhanced;
///
/// let key = b"/blockhash/byheight/\x01\x00\x00\x00";
/// let formatted = parse_cache_key_enhanced(key);
/// // Result: "/blockhash/byheight/00000001" (recognizes as little-endian u32)
/// ```
pub fn parse_cache_key_enhanced(key: &[u8]) -> String {
    key_parser::parse_key_enhanced(key, &key_parser::KeyParseConfig::default())
}

/// Parse a cache key with custom configuration
///
/// This function allows full control over the parsing behavior through configuration.
///
/// # Arguments
///
/// * `key` - The raw key bytes to parse
/// * `config` - Configuration for parsing behavior
///
/// # Returns
///
/// A formatted string representation of the key
///
/// # Example
///
/// ```rust,no_run
/// use metashrew_core::{parse_cache_key_with_config, key_parser::KeyParseConfig};
///
/// let config = KeyParseConfig {
///     max_utf8_segment_length: 20,
///     max_binary_segment_length: 8,
///     show_full_short_binary: true,
///     min_utf8_segment_length: 3,
/// };
/// let key = b"/very/long/path/segment/\x01\x02\x03\x04";
/// let formatted = parse_cache_key_with_config(key, &config);
/// ```
pub fn parse_cache_key_with_config(key: &[u8], config: &key_parser::KeyParseConfig) -> String {
    key_parser::parse_key_readable(key, config)
}

