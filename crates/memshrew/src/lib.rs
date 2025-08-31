//! In-memory implementation of MetashrewRuntime for fast testing

pub mod adapter;

// Re-export the adapter and related types
pub use adapter::{MemStoreAdapter, MemStoreBatch};

// Re-export core runtime with MemStore adapter
pub use metashrew_runtime::{MetashrewRuntime, MetashrewRuntimeContext};

use metashrew_core::indexer::Indexer;

/// Type alias for MetashrewRuntime using in-memory backend
pub type MemStoreRuntime<I: Indexer> = MetashrewRuntime<MemStoreAdapter, I>;

/// Type alias for MetashrewRuntimeContext using in-memory backend
pub type MemStoreRuntimeContext = MetashrewRuntimeContext<MemStoreAdapter>;

// Re-export other useful types from metashrew-runtime
pub use metashrew_runtime::{
    get_label, has_label, set_label, to_labeled_key, wait_timeout, BatchLike, KVTrackerFn,
    KeyValueStoreLike,
};
