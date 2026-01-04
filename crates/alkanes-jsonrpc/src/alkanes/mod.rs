//! Alkanes JSON-RPC method handlers
//!
//! This module provides the `alkanes_*` namespace methods that wrap `metashrew_view` calls
//! with automatic protobuf encoding/decoding.

pub mod types;
pub mod encode;
pub mod decode;

pub use types::*;
pub use encode::*;
pub use decode::*;
