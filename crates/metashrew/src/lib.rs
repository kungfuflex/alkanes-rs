//! # Metashrew Library
//!
//! A library for building Metashrew indexer programs with convenient macros and primitives.
//! This library simplifies the development of WASM modules for the Metashrew Bitcoin indexer framework.

#[macro_use]
extern crate log;

pub mod macros;
pub mod index_pointer;
pub mod host;
pub mod indexer;
pub mod stdio;
pub mod view;
pub mod utils;
pub mod wasm;
pub mod wasm_exports;
#[cfg(feature = "native")]
pub mod native;

/// Re-export key components for easier access
pub use macros::*;
pub use host::*;
pub use indexer::*;
pub use view::*;
pub use wasm::{get, set, input, flush, initialize, reset, clear};
// wasm_exports is already available as a module

// Re-export the declare_indexer macro for backward compatibility
#[doc(hidden)]
pub use declare_indexer as metashrew_proto_program;
