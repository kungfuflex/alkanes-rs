//! # Utility Functions for Bitcoin Data Parsing and Encoding
//!
//! This module provides essential utility functions for parsing Bitcoin blockchain data,
//! consensus encoding/decoding, and data manipulation operations. These utilities are
//! fundamental to the Metashrew indexing system's ability to process Bitcoin blocks
//! and transactions efficiently.
//!
//! ## Core Functionality
//!
//! The utilities in this module handle:
//! - **Bitcoin consensus encoding/decoding**: Converting between Rust types and Bitcoin's wire format
//! - **Variable-length integer parsing**: Handling Bitcoin's varint encoding scheme
//! - **Binary data consumption**: Reading fixed and variable-length data from byte streams
//! - **Memory management**: Safe conversion between WASM pointers and Rust vectors
//! - **Key formatting**: Human-readable representation of hierarchical storage keys
//!
//! ## Usage Examples
//!
//! ```rust,ignore
//! use metashrew_support::utils::*;
//! use std::io::Cursor;
//!
//! // Parse a varint from a byte stream
//! let data = vec![0xfd, 0x00, 0x01]; // varint encoding of 256
//! let mut cursor = Cursor::new(data);
//! let value = consume_varint(&mut cursor).unwrap();
//! assert_eq!(value, 256);
//!
//! // Read exact number of bytes
//! let data = vec![1, 2, 3, 4, 5];
//! let mut cursor = Cursor::new(data);
//! let bytes = consume_exact(&mut cursor, 3).unwrap();
//! assert_eq!(bytes, vec![1, 2, 3]);
//! ```
//!
//! ## Integration with Bitcoin Protocol
//!
//! These utilities implement Bitcoin's specific encoding schemes:
//! - **Varint encoding**: Bitcoin's variable-length integer format
//! - **Consensus encoding**: Bitcoin Core's serialization format
//! - **Little-endian integers**: Consistent with Bitcoin's byte ordering

use crate::byte_view::ByteView;
use anyhow::Result;
use bitcoin::consensus::{
    deserialize_partial,
    encode::{Decodable, Encodable},
};
use std::io::BufRead;
use std::io::Read;
use std::mem::size_of;

/// Encode a value using Bitcoin consensus encoding.
///
/// This function serializes any type implementing the [`Encodable`] trait into
/// Bitcoin's standard wire format. This is used for creating byte representations
/// of Bitcoin data structures that are compatible with Bitcoin Core.
///
/// # Type Parameters
/// - `T`: Type implementing [`Encodable`] trait
///
/// # Parameters
/// - `v`: Reference to the value to encode
///
/// # Returns
/// Byte vector containing the consensus-encoded representation
///
/// # Examples
/// ```rust,ignore
/// use metashrew_support::utils::consensus_encode;
/// use bitcoin::Transaction;
///
/// // Encode a transaction to bytes
/// let tx = Transaction::default();
/// let encoded = consensus_encode(&tx).unwrap();
/// ```
pub fn consensus_encode<T: Encodable>(v: &T) -> Result<Vec<u8>> {
    let mut result = Vec::<u8>::new();
    <T as Encodable>::consensus_encode::<Vec<u8>>(v, &mut result)?;
    Ok(result)
}

/// Decode a value from Bitcoin consensus encoding using a cursor.
///
/// This function deserializes Bitcoin wire format data into Rust types implementing
/// the [`Decodable`] trait. The cursor position is automatically advanced by the
/// number of bytes consumed during deserialization.
///
/// # Type Parameters
/// - `T`: Type implementing [`Decodable`] trait
///
/// # Parameters
/// - `cursor`: Mutable reference to cursor positioned at the data to decode
///
/// # Returns
/// Deserialized value of type `T`
///
/// # Examples
/// ```rust,ignore
/// use metashrew_support::utils::consensus_decode;
/// use bitcoin::Transaction;
/// use std::io::Cursor;
///
/// // Decode a transaction from bytes
/// let mut cursor = Cursor::new(encoded_tx_bytes);
/// let tx: Transaction = consensus_decode(&mut cursor).unwrap();
/// ```
pub fn consensus_decode<T: Decodable>(cursor: &mut std::io::Cursor<Vec<u8>>) -> Result<T> {
    let slice = &cursor.get_ref()[cursor.position() as usize..cursor.get_ref().len() as usize];
    let deserialized: (T, usize) = deserialize_partial(slice)?;
    cursor.consume(deserialized.1);
    Ok(deserialized.0)
}

/// Consume a fixed-size integer from a cursor using [`ByteView`] trait.
///
/// This function reads exactly `size_of::<T>()` bytes from the cursor and converts
/// them to the target type using the [`ByteView`] trait's deserialization method.
/// This ensures consistent little-endian byte ordering across all numeric types.
///
/// # Type Parameters
/// - `T`: Type implementing [`ByteView`] trait
///
/// # Parameters
/// - `cursor`: Mutable reference to cursor positioned at the integer data
///
/// # Returns
/// Deserialized integer value of type `T`
///
/// # Examples
/// ```rust,ignore
/// use metashrew_support::utils::consume_sized_int;
/// use std::io::Cursor;
///
/// let data = vec![0x42, 0x00, 0x00, 0x00]; // 66 in little-endian
/// let mut cursor = Cursor::new(data);
/// let value: u32 = consume_sized_int(&mut cursor).unwrap();
/// assert_eq!(value, 66);
/// ```
pub fn consume_sized_int<T: ByteView>(cursor: &mut std::io::Cursor<Vec<u8>>) -> Result<T> {
    let buffer = consume_exact(cursor, size_of::<T>())?;
    Ok(T::from_bytes(buffer))
}

/// Consume all remaining bytes from a cursor.
///
/// This function reads from the current cursor position to the end of the buffer,
/// returning all remaining bytes. If the cursor is already at the end, returns
/// an empty vector.
///
/// # Parameters
/// - `cursor`: Mutable reference to cursor
///
/// # Returns
/// Vector containing all remaining bytes from the cursor position
///
/// # Examples
/// ```rust,ignore
/// use metashrew_support::utils::consume_to_end;
/// use std::io::Cursor;
///
/// let data = vec![1, 2, 3, 4, 5];
/// let mut cursor = Cursor::new(data);
/// cursor.set_position(2); // Skip first 2 bytes
/// let remaining = consume_to_end(&mut cursor).unwrap();
/// assert_eq!(remaining, vec![3, 4, 5]);
/// ```
pub fn consume_to_end(cursor: &mut std::io::Cursor<Vec<u8>>) -> Result<Vec<u8>> {
    if is_empty(cursor) {
        return Ok(vec![]);
    }
    let mut result: Vec<u8> = vec![];
    cursor.read_to_end(&mut result)?;
    Ok(result)
}

/// Consume exactly `n` bytes from a cursor.
///
/// This function reads exactly the specified number of bytes from the cursor,
/// advancing the cursor position accordingly. If insufficient bytes are available,
/// an error is returned.
///
/// # Parameters
/// - `cursor`: Mutable reference to cursor
/// - `n`: Number of bytes to consume
///
/// # Returns
/// Vector containing exactly `n` bytes read from the cursor
///
/// # Examples
/// ```rust,ignore
/// use metashrew_support::utils::consume_exact;
/// use std::io::Cursor;
///
/// let data = vec![1, 2, 3, 4, 5];
/// let mut cursor = Cursor::new(data);
/// let bytes = consume_exact(&mut cursor, 3).unwrap();
/// assert_eq!(bytes, vec![1, 2, 3]);
/// assert_eq!(cursor.position(), 3);
/// ```
pub fn consume_exact(cursor: &mut std::io::Cursor<Vec<u8>>, n: usize) -> Result<Vec<u8>> {
    let mut buffer: Vec<u8> = vec![0u8; n];
    cursor.read_exact(&mut buffer[0..n])?;
    Ok(buffer)
}

/// Parse a Bitcoin variable-length integer (varint) from a cursor.
///
/// Bitcoin uses a variable-length encoding scheme for integers to save space:
/// - Values 0-252: encoded as single byte
/// - Values 253-65535: 0xfd + 2-byte little-endian
/// - Values 65536-4294967295: 0xfe + 4-byte little-endian  
/// - Values 4294967296+: 0xff + 8-byte little-endian
///
/// # Parameters
/// - `cursor`: Mutable reference to cursor positioned at varint data
///
/// # Returns
/// Decoded 64-bit unsigned integer value
///
/// # Examples
/// ```rust,ignore
/// use metashrew_support::utils::consume_varint;
/// use std::io::Cursor;
///
/// // Single byte varint (value < 253)
/// let data = vec![42];
/// let mut cursor = Cursor::new(data);
/// assert_eq!(consume_varint(&mut cursor).unwrap(), 42);
///
/// // Multi-byte varint (value >= 253)
/// let data = vec![0xfd, 0x00, 0x01]; // 256 in varint encoding
/// let mut cursor = Cursor::new(data);
/// assert_eq!(consume_varint(&mut cursor).unwrap(), 256);
/// ```
pub fn consume_varint(cursor: &mut std::io::Cursor<Vec<u8>>) -> Result<u64> {
    Ok(match consume_sized_int::<u8>(cursor)? {
        0xff => consume_sized_int::<u64>(cursor)?,
        0xfe => consume_sized_int::<u32>(cursor)? as u64,
        0xfd => consume_sized_int::<u16>(cursor)? as u64,
        v => v as u64,
    })
}

/// Consume a 128-bit unsigned integer from a cursor.
///
/// This is a convenience function for reading 128-bit integers using the
/// [`consume_sized_int`] function with explicit type specification.
///
/// # Parameters
/// - `cursor`: Mutable reference to cursor positioned at 128-bit integer data
///
/// # Returns
/// 128-bit unsigned integer value
pub fn consume_u128(cursor: &mut std::io::Cursor<Vec<u8>>) -> Result<u128> {
    consume_sized_int::<u128>(cursor)
}

/// Check if a cursor has reached the end of its buffer.
///
/// This function determines whether the cursor position has reached or exceeded
/// the length of the underlying buffer, indicating no more data is available.
///
/// # Parameters
/// - `cursor`: Mutable reference to cursor to check
///
/// # Returns
/// `true` if cursor is at or beyond end of buffer, `false` otherwise
pub fn is_empty(cursor: &mut std::io::Cursor<Vec<u8>>) -> bool {
    cursor.position() >= cursor.get_ref().len() as u64
}

/// Get a slice of remaining bytes from a cursor without advancing position.
///
/// This function returns a slice view of all bytes from the current cursor
/// position to the end of the buffer, without modifying the cursor position.
///
/// # Parameters
/// - `cursor`: Mutable reference to cursor
///
/// # Returns
/// Slice containing all remaining bytes from cursor position
pub fn remaining_slice(cursor: &mut std::io::Cursor<Vec<u8>>) -> &[u8] {
    &cursor.get_ref()[(cursor.position() as usize)..cursor.get_ref().len()]
}

/// Convert a WASM memory pointer to a Rust vector.
///
/// This function performs unsafe conversion from a WASM linear memory pointer
/// to a Rust `Vec<u8>`. The length is read from 4 bytes before the pointer
/// address, following WASM memory layout conventions.
///
/// # Safety
/// This function is unsafe because it:
/// - Dereferences raw pointers
/// - Assumes specific memory layout (length at ptr-4)
/// - Creates Vec from raw parts without ownership verification
///
/// # Parameters
/// - `ptr`: WASM memory pointer as 32-bit integer
///
/// # Returns
/// Vector containing the data from WASM memory
///
/// # Note
/// This function is specifically designed for WASM host-guest communication
/// and should only be used in that context.
pub fn ptr_to_vec(ptr: i32) -> Vec<u8> {
    unsafe {
        let len = *((ptr - 4) as usize as *const usize);
        Box::leak(Box::new(Vec::<u8>::from_raw_parts(
            ptr as usize as *mut u8,
            len,
            len,
        )))
        .clone()
    }
}

/// Format a hierarchical key for human-readable display.
///
/// This function converts a byte vector representing a hierarchical storage key
/// into a human-readable string format. Key components are separated by forward
/// slashes, with ASCII components displayed as text and non-ASCII components
/// displayed as hexadecimal.
///
/// # Parameters
/// - `v`: Reference to byte vector containing the hierarchical key
///
/// # Returns
/// Formatted string representation of the key
///
/// # Examples
/// ```rust,ignore
/// use metashrew_support::utils::format_key;
///
/// // ASCII key components
/// let key = b"users/alice/balance".to_vec();
/// let formatted = format_key(&key);
/// // Result: "/users/alice/balance"
///
/// // Mixed ASCII and binary components
/// let key = vec![117, 115, 101, 114, 115, 47, 0x01, 0x02, 47, 98, 97, 108];
/// let formatted = format_key(&key);
/// // Result: "/users/0102/bal"
/// ```
pub fn format_key(v: &Vec<u8>) -> String {
    v.clone()
        .split(|c| *c == 47)
        .map(|bytes| {
            let v = bytes.to_vec();
            if v.len() == 0 {
                return "".to_owned();
            }
            let r = String::from_utf8(v);
            let is_ascii = match r {
                Ok(ref s) => s.is_ascii(),
                Err(_) => false,
            };
            if is_ascii {
                "/".to_owned() + r.unwrap().as_str()
            } else {
                "/".to_owned() + hex::encode(bytes).as_str()
            }
        })
        .collect::<Vec<String>>()
        .join("")
}
