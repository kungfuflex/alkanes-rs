//! Generic Bitcoin indexer synchronization framework
//!
//! This crate provides a generic framework for building Bitcoin indexers that can work
//! with different storage backends and Bitcoin node interfaces. It abstracts the core
//! synchronization logic from rockshrew-mono to enable testing and modularity.

pub mod adapters;
pub mod error;
pub mod jsonrpc;
pub mod snapshot;
pub mod snapshot_sync;
pub mod sync;
pub mod traits;
pub mod types;

#[cfg(any(test, feature = "mock"))]
pub mod mock;

#[cfg(any(test, feature = "mock"))]
pub mod mock_snapshot;

pub use adapters::*;
pub use error::*;
pub use jsonrpc::*;
pub use snapshot::*;
pub use snapshot_sync::*;
pub use sync::*;
pub use traits::*;
pub use types::*;

#[cfg(any(test, feature = "mock"))]
pub use mock::*;

#[cfg(any(test, feature = "mock"))]
pub use mock_snapshot::*;
