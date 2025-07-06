//! # WASM Compatibility and Memory Management Utilities
//!
//! This module provides essential utilities for WebAssembly (WASM) host-guest communication
//! and memory management. These functions handle the low-level details of passing data
//! between the Metashrew runtime (host) and WASM indexer modules (guest), ensuring
//! safe and efficient memory operations across the WASM boundary.
//!
//! ## Core Concepts
//!
//! ### WASM Memory Layout
//! WASM modules use linear memory that must be carefully managed when passing data
//! between host and guest. This module implements the ArrayBuffer layout convention:
//! - **Length prefix**: 4-byte little-endian length at the beginning
//! - **Data payload**: Actual data following the length prefix
//! - **Pointer arithmetic**: Safe conversion between Rust pointers and WASM addresses
//!
//! ### Memory Safety
//! All functions in this module handle the unsafe aspects of WASM memory management:
//! - **Pointer conversion**: Safe casting between Rust and WASM address spaces
//! - **Memory layout**: Consistent data structure layout across boundaries
//! - **Lifetime management**: Proper handling of memory ownership transfer
//!
//! ## Usage Examples
//!
//! ```rust,ignore
//! use metashrew_support::compat::*;
//!
//! // Prepare data for export to WASM
//! let data = vec![1, 2, 3, 4, 5];
//! let wasm_ptr = export_bytes(data);
//!
//! // Convert data to ArrayBuffer layout
//! let buffer = to_arraybuffer_layout(&[1, 2, 3]);
//! // Result: [3, 0, 0, 0, 1, 2, 3] (length + data)
//!
//! // Get pointer for passing to WASM
//! let mut data = vec![1, 2, 3];
//! let ptr = to_ptr(&mut data);
//! ```
//!
//! ## Integration with Metashrew
//!
//! These utilities are fundamental to Metashrew's WASM execution model:
//! - **Host functions**: Passing data from runtime to WASM modules
//! - **Return values**: Getting results back from WASM indexer functions
//! - **Memory management**: Safe handling of dynamic data across boundaries

/// Convert a mutable vector reference to a WASM-compatible pointer.
///
/// This function extracts the raw pointer from a Rust vector and converts it
/// to a 32-bit integer suitable for use in WASM linear memory addressing.
/// The pointer points directly to the vector's data buffer.
///
/// # Parameters
/// - `v`: Mutable reference to the vector to get pointer for
///
/// # Returns
/// 32-bit integer representing the WASM memory address of the vector data
///
/// # Safety
/// The returned pointer is only valid as long as the vector remains alive
/// and is not reallocated. The caller must ensure proper lifetime management.
///
/// # Usage
/// This function is typically used when passing vector data to WASM functions
/// that expect raw memory pointers.
pub fn to_ptr(v: &mut Vec<u8>) -> i32 {
    return v.as_mut_ptr() as usize as i32;
}

/// Convert a mutable vector reference to a passback pointer.
///
/// This function creates a pointer that points 4 bytes past the start of the
/// vector data, skipping over the length prefix in ArrayBuffer layout. This
/// is used when the WASM module expects to receive data without the length
/// prefix.
///
/// # Parameters
/// - `v`: Mutable reference to the vector to get passback pointer for
///
/// # Returns
/// 32-bit integer representing the WASM memory address 4 bytes into the vector
///
/// # ArrayBuffer Layout
/// When data is stored in ArrayBuffer layout:
/// - Bytes 0-3: Length as little-endian u32
/// - Bytes 4+: Actual data payload
///
/// This function returns a pointer to the data payload, skipping the length.
pub fn to_passback_ptr(v: &mut Vec<u8>) -> i32 {
    to_ptr(v) + 4
}

/// Convert data to ArrayBuffer layout with length prefix.
///
/// This function creates a new vector that follows the ArrayBuffer convention
/// used in WASM communication. The resulting vector contains a 4-byte
/// little-endian length prefix followed by the original data.
///
/// # Type Parameters
/// - `T`: Type that can be converted to a byte slice reference
///
/// # Parameters
/// - `v`: Data to convert to ArrayBuffer layout
///
/// # Returns
/// New vector containing length prefix followed by data
///
/// # Layout
/// The returned vector has the following structure:
/// ```text
/// [length_byte_0, length_byte_1, length_byte_2, length_byte_3, data_byte_0, data_byte_1, ...]
/// ```
/// Where the length is stored in little-endian format.
///
/// # Examples
/// ```rust,ignore
/// use metashrew_support::compat::to_arraybuffer_layout;
///
/// let data = vec![0x41, 0x42, 0x43]; // "ABC"
/// let buffer = to_arraybuffer_layout(&data);
/// // Result: [3, 0, 0, 0, 0x41, 0x42, 0x43]
/// //         ^length=3^  ^---data---^
/// ```
pub fn to_arraybuffer_layout<T: AsRef<[u8]>>(v: T) -> Vec<u8> {
    let mut buffer = Vec::<u8>::new();
    buffer.extend_from_slice(&(v.as_ref().len() as u32).to_le_bytes());
    buffer.extend_from_slice(v.as_ref());
    return buffer;
}

/// Export bytes to WASM memory with ArrayBuffer layout and return pointer.
///
/// This function is the primary interface for returning data from host functions
/// to WASM modules. It converts the input data to ArrayBuffer layout, allocates
/// it in WASM-accessible memory, and returns a pointer that the WASM module
/// can use to access the data.
///
/// # Parameters
/// - `v`: Vector of bytes to export to WASM memory
///
/// # Returns
/// 32-bit integer pointer to the data in WASM memory (pointing past length prefix)
///
/// # Memory Management
/// This function uses `Box::leak()` to transfer ownership of the data to the
/// WASM memory space. The caller (typically the WASM module) becomes responsible
/// for the memory's lifetime. The returned pointer points to the data payload,
/// not the length prefix.
///
/// # Usage Pattern
/// This function is typically used in host function implementations:
/// ```rust,ignore
/// // In a host function that returns data to WASM
/// fn host_function() -> i32 {
///     let result_data = vec![1, 2, 3, 4];
///     export_bytes(result_data) // Returns pointer for WASM
/// }
/// ```
///
/// # Memory Layout
/// The allocated memory has ArrayBuffer layout:
/// - Bytes -4 to -1: Length as little-endian u32
/// - Bytes 0+: Data payload (returned pointer points here)
pub fn export_bytes(v: Vec<u8>) -> i32 {
    let response: Vec<u8> = to_arraybuffer_layout(&v);
    Box::leak(Box::new(response)).as_mut_ptr() as usize as i32 + 4
}
