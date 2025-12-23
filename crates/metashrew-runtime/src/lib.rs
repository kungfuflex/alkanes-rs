//! Generic MetashrewRuntime that works with any storage backend

use anyhow::Result;
use std::sync::OnceLock;

// Core modules
pub mod context;
pub mod helpers;
pub mod key_utils;
pub mod proto;
pub mod runtime;
pub mod smt;
pub mod traits;


// Re-export core types and traits
pub use context::MetashrewRuntimeContext;
pub use runtime::{MetashrewRuntime, State, TIP_HEIGHT_KEY};
pub use traits::{BatchLike, KVTrackerFn, KeyValueStoreLike};

// Re-export helper types
pub use smt::{BatchedSMTHelper, SMTHelper, SMTNode};

// Thread-safe label storage using OnceLock for deterministic behavior
static LABEL: OnceLock<String> = OnceLock::new();

const TIMEOUT: u64 = 1500;

use std::{thread, time};

pub fn wait_timeout() {
    thread::sleep(time::Duration::from_millis(TIMEOUT));
}

/// Sets the label prefix for all database keys.
/// This can only be called once - subsequent calls will be ignored.
/// For deterministic behavior, this should be called during initialization.
pub fn set_label(s: String) {
    // OnceLock::set returns Err if already set, which we ignore
    // This ensures the label can only be set once for determinism
    let _ = LABEL.set(s + "://");
}

/// Gets the label prefix if set.
/// Returns the label string reference, panics if not set.
pub fn get_label() -> &'static String {
    LABEL.get().expect("Label not initialized - call set_label first")
}

/// Checks if a label has been set.
pub fn has_label() -> bool {
    LABEL.get().is_some()
}

pub fn to_labeled_key(key: &Vec<u8>) -> Vec<u8> {
    if has_label() {
        let mut result: Vec<u8> = vec![];
        result.extend(get_label().as_str().as_bytes());
        result.extend(key);
        result
    } else {
        key.clone()
    }
}

/// Generic function to query height from any storage backend
pub async fn query_height<T: KeyValueStoreLike>(mut db: T, start_block: u32) -> Result<u32>
where
    T::Error: std::error::Error + Send + Sync + 'static,
{
    let height_key = TIP_HEIGHT_KEY.as_bytes().to_vec();
    let bytes = match db
        .get(&to_labeled_key(&height_key))
        .map_err(|e| anyhow::anyhow!("Database error: {:?}", e))?
    {
        Some(v) => v,
        None => {
            return Ok(start_block);
        }
    };
    if bytes.len() == 0 {
        return Ok(start_block);
    }
    let bytes_ref: &[u8] = &bytes;
    Ok(u32::from_le_bytes(bytes_ref.try_into().unwrap()))
}
