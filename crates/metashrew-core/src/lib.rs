
//! Metashrew Core - WebAssembly bindings for Bitcoin indexers
//!
//! This crate provides the core WebAssembly bindings and utilities for building
//! Bitcoin indexers that run within the Metashrew framework. It defines the
//! host-guest interface between the Metashrew runtime and WASM modules.

extern crate alloc;
#[allow(unused_imports)]
use std::fmt::Write;
#[cfg(feature = "panic-hook")]
use std::panic;

#[cfg(feature = "panic-hook")]
pub mod compat;
pub mod environment;

pub mod macros;

#[cfg(feature = "panic-hook")]
use crate::compat::panic_hook;

#[allow(unused_imports)]
use metashrew_support::{
    compat::{to_arraybuffer_layout, to_passback_ptr, to_ptr},
    proto::metashrew::{IndexerMetadata, KeyValueFlush, ViewFunction},
};

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
