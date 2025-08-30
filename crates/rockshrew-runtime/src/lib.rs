//! RocksDB-specific implementation of MetashrewRuntime

pub mod adapter;
pub mod fork_adapter;
pub mod optimized_config;
pub mod storage_adapter;
pub mod retry;

// Re-export the adapter and related types
pub use adapter::{query_height, RocksDBBatch, RocksDBRuntimeAdapter};
pub use storage_adapter::RocksDBStorageAdapter;

// Re-export optimized configuration functions
pub use optimized_config::{
    create_optimized_options, create_secondary_options, log_performance_stats,
};

// Re-export core runtime with RocksDB adapter
pub use metashrew_runtime::{MetashrewRuntime, MetashrewRuntimeContext};

/// Type alias for MetashrewRuntime using RocksDB backend
pub type RocksDBRuntime = MetashrewRuntime<RocksDBRuntimeAdapter>;

/// Type alias for MetashrewRuntimeContext using RocksDB backend
pub type RocksDBRuntimeContext = MetashrewRuntimeContext<RocksDBRuntimeAdapter>;

// Re-export other useful types from metashrew-runtime
pub use metashrew_runtime::{
    get_label, has_label, set_label, to_labeled_key, wait_timeout, BatchLike, KVTrackerFn,
    KeyValueStoreLike,
};
