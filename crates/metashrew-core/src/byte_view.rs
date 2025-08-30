//! # Byte Serialization and Conversion Utilities
//!
//! This module provides the [`ByteView`] trait and related utilities for converting between
//! Rust primitive types and their byte representations. This is fundamental to the Metashrew
//! storage system, enabling type-safe serialization and deserialization of values stored
//! in the key-value database.
//!
//! ## Core Concepts
//!
//! The [`ByteView`] trait provides a standardized interface for:
//! - **Bidirectional conversion**: Converting types to/from byte vectors
//! - **Boundary values**: Accessing minimum (zero) and maximum values for types
//! - **Little-endian encoding**: Consistent byte ordering across all numeric types
//! - **Type safety**: Compile-time guarantees for serialization operations
//!
//! ## Usage Examples
//!
//! ```rust
//! use metashrew_core::byte_view::ByteView;
//!
//! // Convert a u32 to bytes and back
//! let value: u32 = 42;
//! let bytes = value.to_bytes();
//! let restored = u32::from_bytes(bytes);
//! assert_eq!(value, restored);
//!
//! // Access boundary values
//! let max_u16 = u16::maximum();
//! let zero_u64 = u64::zero();
//! ```
//!
//! ## Integration with Storage
//!
//! The [`ByteView`] trait is used extensively throughout Metashrew's storage layer:
//! - **Index pointers**: Type-safe value storage and retrieval
//! - **SMT operations**: Serializing keys and values for Sparse Merkle Tree operations
//! - **Database storage**: Converting Rust types to storage-compatible byte arrays
//!
//! ## Supported Types
//!
//! Implementations are provided for all standard unsigned integer types:
//! `u8`, `u16`, `u32`, `u64`, `u128`, and `usize`.

/// Core trait for bidirectional conversion between Rust types and byte representations.
///
/// This trait enables type-safe serialization and deserialization of values in the
/// Metashrew storage system. All implementations use little-endian byte ordering
/// for consistency across platforms.
///
/// # Type Safety
///
/// The trait provides compile-time guarantees that:
/// - Values can be round-trip converted (to_bytes → from_bytes → original value)
/// - Boundary values (zero, maximum) are correctly represented
/// - Byte representations are consistent and deterministic
///
/// # Implementation Requirements
///
/// Implementors must ensure:
/// - `from_bytes(value.to_bytes()) == value` for all valid values
/// - `zero().to_bytes()` produces the minimal byte representation
/// - `maximum().to_bytes()` produces the maximal byte representation
/// - Little-endian byte ordering is used for multi-byte types
#[allow(dead_code)]
pub trait ByteView {
    /// Convert a byte vector to the implementing type.
    ///
    /// # Parameters
    /// - `v`: Byte vector containing the serialized representation
    ///
    /// # Returns
    /// The deserialized value of the implementing type
    ///
    /// # Panics
    /// May panic if the byte vector has incorrect length for the target type
    fn from_bytes(v: Vec<u8>) -> Self;

    /// Convert the implementing type to a byte vector.
    ///
    /// # Returns
    /// Byte vector containing the little-endian serialized representation
    fn to_bytes(&self) -> Vec<u8>;

    /// Return the maximum possible value for the implementing type.
    ///
    /// # Returns
    /// The maximum value (e.g., `u32::MAX` for `u32`)
    fn maximum() -> Self;

    /// Return the zero value for the implementing type.
    ///
    /// # Returns
    /// The zero/minimum value (e.g., `0` for numeric types)
    fn zero() -> Self;
}

/// Utility function to shrink a byte vector by removing elements from the front.
///
/// This function removes the first `v` bytes from the input vector. If the vector
/// is shorter than `v` bytes, it returns an empty vector.
///
/// # Parameters
/// - `b`: Input byte vector to shrink
/// - `v`: Number of bytes to remove from the front
///
/// # Returns
/// New byte vector with the first `v` bytes removed
///
/// # Examples
/// ```rust,ignore
/// use metashrew_core::byte_view::shrink_back;
///
/// let data = vec![1, 2, 3, 4, 5];
/// let result = shrink_back(data, 2);
/// assert_eq!(result, vec![3, 4, 5]);
/// ```
#[allow(dead_code)]
pub fn shrink_back(b: Vec<u8>, v: usize) -> Vec<u8> {
    if v > b.len() {
        return vec![];
    }
    b[v..].to_vec()
}

/// Implementation of [`ByteView`] for 8-bit unsigned integers.
///
/// Provides single-byte serialization with direct byte representation.
/// This is the most efficient implementation as no endianness conversion is needed.
#[allow(dead_code)]
impl ByteView for u8 {
    fn to_bytes(&self) -> Vec<u8> {
        Vec::<u8>::from(self.to_le_bytes())
    }
    fn from_bytes(v: Vec<u8>) -> u8 {
        u8::from_le_bytes(v.as_slice().try_into().expect("incorrect length"))
    }
    fn maximum() -> u8 {
        u8::MAX
    }
    fn zero() -> u8 {
        0
    }
}

/// Implementation of [`ByteView`] for 16-bit unsigned integers.
///
/// Serializes to 2 bytes in little-endian format for consistent cross-platform behavior.
#[allow(dead_code)]
impl ByteView for u16 {
    fn to_bytes(&self) -> Vec<u8> {
        Vec::<u8>::from(self.to_le_bytes())
    }
    fn from_bytes(v: Vec<u8>) -> u16 {
        u16::from_le_bytes(v.as_slice().try_into().expect("incorrect length"))
    }
    fn maximum() -> u16 {
        u16::MAX
    }
    fn zero() -> u16 {
        0
    }
}

/// Implementation of [`ByteView`] for 32-bit unsigned integers.
///
/// Serializes to 4 bytes in little-endian format. This is commonly used for
/// block heights, transaction indices, and other numeric identifiers in Bitcoin indexing.
#[allow(dead_code)]
impl ByteView for u32 {
    fn to_bytes(&self) -> Vec<u8> {
        Vec::<u8>::from(self.to_le_bytes())
    }
    fn from_bytes(v: Vec<u8>) -> u32 {
        u32::from_le_bytes(v.as_slice().try_into().expect("incorrect length"))
    }
    fn maximum() -> u32 {
        u32::MAX
    }
    fn zero() -> u32 {
        0
    }
}

/// Implementation of [`ByteView`] for 64-bit unsigned integers.
///
/// Serializes to 8 bytes in little-endian format. Frequently used for
/// satoshi amounts, timestamps, and large numeric values in Bitcoin applications.
#[allow(dead_code)]
impl ByteView for u64 {
    fn to_bytes(&self) -> Vec<u8> {
        Vec::<u8>::from(self.to_le_bytes())
    }
    fn from_bytes(v: Vec<u8>) -> u64 {
        u64::from_le_bytes(v.as_slice().try_into().expect("incorrect length"))
    }
    fn maximum() -> u64 {
        u64::MAX
    }
    fn zero() -> u64 {
        0
    }
}

/// Implementation of [`ByteView`] for 128-bit unsigned integers.
///
/// Serializes to 16 bytes in little-endian format. Used for very large numeric
/// values and cryptographic operations requiring extended precision.
#[allow(dead_code)]
impl ByteView for u128 {
    fn to_bytes(&self) -> Vec<u8> {
        Vec::<u8>::from(self.to_le_bytes())
    }
    fn from_bytes(v: Vec<u8>) -> u128 {
        u128::from_le_bytes(v.as_slice().try_into().expect("incorrect length"))
    }
    fn maximum() -> u128 {
        u128::MAX
    }
    fn zero() -> u128 {
        0
    }
}

/// Implementation of [`ByteView`] for platform-dependent unsigned integers.
///
/// Serializes to platform-specific byte length (4 bytes on 32-bit, 8 bytes on 64-bit).
/// Used for array indices, memory sizes, and other platform-dependent values.
#[allow(dead_code)]
impl ByteView for usize {
    fn to_bytes(&self) -> Vec<u8> {
        Vec::<u8>::from(self.to_le_bytes())
    }
    fn from_bytes(v: Vec<u8>) -> usize {
        usize::from_le_bytes(v.as_slice().try_into().expect("incorrect length"))
    }

    fn maximum() -> usize {
        usize::MAX
    }
    fn zero() -> usize {
        0
    }
}