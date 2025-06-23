//! RocksDB-specific implementation of MetashrewRuntime

pub mod adapter;
pub mod generic_adapters;

// Re-export the adapter and related types
pub use adapter::{query_height, RocksDBBatch, RocksDBRuntimeAdapter};

// Re-export generic adapter implementations
pub use generic_adapters::{
    RocksDBHeightTracker, RocksDBStateRootManager, RocksDBBatchProcessor,
    RocksDBBlockHashManager, RocksDBStorageAdapterCore,
};

// Re-export core runtime with RocksDB adapter
pub use metashrew_runtime::{MetashrewRuntime, MetashrewRuntimeContext};

/// Type alias for MetashrewRuntime using RocksDB backend
pub type RocksDBRuntime = MetashrewRuntime<RocksDBRuntimeAdapter>;

/// Type alias for MetashrewRuntimeContext using RocksDB backend
pub type RocksDBRuntimeContext = MetashrewRuntimeContext<RocksDBRuntimeAdapter>;

// Re-export other useful types from metashrew-runtime
pub use metashrew_runtime::{
    get_label, has_label, set_label, to_labeled_key, wait_timeout, BSTHelper, BSTStatistics,
    BatchLike, KVTrackerFn, KeyValueStoreLike, OptimizedBST, OptimizedBSTStatistics,
    // Re-export generic adapter traits
    adapters::{
        HeightTracker, StateRootManager, BatchProcessor,
        GenericHeightTracker, GenericStateRootManager,
    },
};

// Re-export traits from metashrew-runtime
pub use metashrew_runtime::adapters::traits::{BlockHashManager, StorageAdapterCore};
