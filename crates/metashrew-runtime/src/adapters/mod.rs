//! Common adapter functionality for all storage backends
//!
//! This module provides shared traits and implementations that can be used
//! across different storage backends (RocksDB, in-memory, etc.)

pub mod height_tracker;
pub mod state_root_manager;
pub mod traits;

// Re-export common traits
pub use traits::{HeightTracker, StateRootManager, BatchProcessor};
pub use height_tracker::GenericHeightTracker;
pub use state_root_manager::GenericStateRootManager;