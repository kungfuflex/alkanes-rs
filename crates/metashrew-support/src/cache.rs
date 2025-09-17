use crate::environment::RuntimeEnvironment;
use crate::proto::metashrew::KeyValueFlush;
use protobuf::Message;
use std::collections::HashMap;
use std::sync::Arc;

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
#[allow(static_mut_refs)]
pub fn get<E: RuntimeEnvironment>(v: Arc<Vec<u8>>) -> Arc<Vec<u8>> {
    unsafe {
        initialize();
        if CACHE.as_ref().unwrap().contains_key(&v.clone()) {
            return CACHE.as_ref().unwrap().get(&v.clone()).unwrap().clone();
        }
        let value = E::get(v.as_ref()).map_or(vec![], |v| v);
        let value = Arc::new(value);
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
#[allow(static_mut_refs)]
pub fn flush<E: RuntimeEnvironment>() {
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
        E::flush(&serialized).unwrap();
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
#[allow(static_mut_refs)]
pub fn initialize() -> () {
    unsafe {
        if CACHE.is_none() {
            reset();
            CACHE = Some(HashMap::<Arc<Vec<u8>>, Arc<Vec<u8>>>::new());
        }
    }
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
pub fn clear() -> () {
    unsafe {
        reset();
        CACHE = Some(HashMap::<Arc<Vec<u8>>, Arc<Vec<u8>>>::new());
    }
}
