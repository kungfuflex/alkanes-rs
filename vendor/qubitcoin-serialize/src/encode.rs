//! `Encodable`/`Decodable` traits and implementations for primitive types.
//!
//! Maps to: `src/serialize.h` (`Serialize`/`Unserialize` template functions) in Bitcoin Core.
//!
//! In Bitcoin Core, serialization is template-based with duck typing.
//! In Rust, we use traits: [`Encodable`] and [`Decodable`].

use std::io::{self, Read, Write};

/// Maximum size of a serialized object in bytes (32 MB, `0x02000000`).
///
/// Any object whose serialized representation exceeds this limit is rejected.
/// Matches `MAX_SIZE` in Bitcoin Core's `serialize.h`.
pub const MAX_SIZE: u64 = 0x02000000;

/// Maximum number of elements to pre-allocate when deserializing a vector.
///
/// Prevents a malicious peer from causing excessive memory allocation
/// by advertising a very large vector length before the actual data arrives.
pub const MAX_VECTOR_ALLOCATE: usize = 5_000_000;

/// Error type for serialization/deserialization operations.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// An underlying I/O error occurred during reading or writing.
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    /// A CompactSize value was not encoded in the smallest possible form.
    #[error("Non-canonical compact size encoding")]
    NonCanonicalCompactSize,

    /// A CompactSize value exceeded [`MAX_SIZE`].
    #[error("Compact size too large: {0}")]
    CompactSizeTooLarge(u64),

    /// A VarInt value overflowed the maximum representable `u64`.
    #[error("VarInt too large")]
    VarIntTooLarge,

    /// The data contained an invalid or unexpected encoding (e.g. invalid UTF-8).
    #[error("Invalid encoding: {0}")]
    InvalidEncoding(String),

    /// The reader ran out of data before the expected amount was consumed.
    #[error("End of data")]
    EndOfData,

    /// A vector or byte sequence length exceeded [`MAX_SIZE`].
    #[error("Size exceeds max: {0}")]
    OversizedVector(u64),
}

/// Trait for types that can be serialized to a byte stream.
///
/// Port of Bitcoin Core's `Serialize(Stream&)` template method.
/// All integers are written in little-endian byte order.
pub trait Encodable {
    /// Writes the serialized form of `self` to `writer`.
    ///
    /// Returns the number of bytes written.
    fn encode<W: Write>(&self, writer: &mut W) -> Result<usize, Error>;
}

/// Trait for types that can be deserialized from a byte stream.
///
/// Port of Bitcoin Core's `Unserialize(Stream&)` template method.
pub trait Decodable: Sized {
    /// Reads and returns an instance of `Self` from `reader`.
    fn decode<R: Read>(reader: &mut R) -> Result<Self, Error>;
}

// --- Primitive type implementations ---
// All integers are serialized in little-endian format.

impl Encodable for u8 {
    fn encode<W: Write>(&self, w: &mut W) -> Result<usize, Error> {
        w.write_all(&[*self])?;
        Ok(1)
    }
}

impl Decodable for u8 {
    fn decode<R: Read>(r: &mut R) -> Result<Self, Error> {
        let mut buf = [0u8; 1];
        r.read_exact(&mut buf)?;
        Ok(buf[0])
    }
}

impl Encodable for i8 {
    fn encode<W: Write>(&self, w: &mut W) -> Result<usize, Error> {
        w.write_all(&[*self as u8])?;
        Ok(1)
    }
}

impl Decodable for i8 {
    fn decode<R: Read>(r: &mut R) -> Result<Self, Error> {
        let mut buf = [0u8; 1];
        r.read_exact(&mut buf)?;
        Ok(buf[0] as i8)
    }
}

impl Encodable for bool {
    fn encode<W: Write>(&self, w: &mut W) -> Result<usize, Error> {
        let val: u8 = if *self { 1 } else { 0 };
        val.encode(w)
    }
}

impl Decodable for bool {
    fn decode<R: Read>(r: &mut R) -> Result<Self, Error> {
        let val = u8::decode(r)?;
        Ok(val != 0)
    }
}

impl Encodable for u16 {
    fn encode<W: Write>(&self, w: &mut W) -> Result<usize, Error> {
        w.write_all(&self.to_le_bytes())?;
        Ok(2)
    }
}

impl Decodable for u16 {
    fn decode<R: Read>(r: &mut R) -> Result<Self, Error> {
        let mut buf = [0u8; 2];
        r.read_exact(&mut buf)?;
        Ok(u16::from_le_bytes(buf))
    }
}

impl Encodable for i16 {
    fn encode<W: Write>(&self, w: &mut W) -> Result<usize, Error> {
        w.write_all(&self.to_le_bytes())?;
        Ok(2)
    }
}

impl Decodable for i16 {
    fn decode<R: Read>(r: &mut R) -> Result<Self, Error> {
        let mut buf = [0u8; 2];
        r.read_exact(&mut buf)?;
        Ok(i16::from_le_bytes(buf))
    }
}

impl Encodable for u32 {
    fn encode<W: Write>(&self, w: &mut W) -> Result<usize, Error> {
        w.write_all(&self.to_le_bytes())?;
        Ok(4)
    }
}

impl Decodable for u32 {
    fn decode<R: Read>(r: &mut R) -> Result<Self, Error> {
        let mut buf = [0u8; 4];
        r.read_exact(&mut buf)?;
        Ok(u32::from_le_bytes(buf))
    }
}

impl Encodable for i32 {
    fn encode<W: Write>(&self, w: &mut W) -> Result<usize, Error> {
        w.write_all(&self.to_le_bytes())?;
        Ok(4)
    }
}

impl Decodable for i32 {
    fn decode<R: Read>(r: &mut R) -> Result<Self, Error> {
        let mut buf = [0u8; 4];
        r.read_exact(&mut buf)?;
        Ok(i32::from_le_bytes(buf))
    }
}

impl Encodable for u64 {
    fn encode<W: Write>(&self, w: &mut W) -> Result<usize, Error> {
        w.write_all(&self.to_le_bytes())?;
        Ok(8)
    }
}

impl Decodable for u64 {
    fn decode<R: Read>(r: &mut R) -> Result<Self, Error> {
        let mut buf = [0u8; 8];
        r.read_exact(&mut buf)?;
        Ok(u64::from_le_bytes(buf))
    }
}

impl Encodable for i64 {
    fn encode<W: Write>(&self, w: &mut W) -> Result<usize, Error> {
        w.write_all(&self.to_le_bytes())?;
        Ok(8)
    }
}

impl Decodable for i64 {
    fn decode<R: Read>(r: &mut R) -> Result<Self, Error> {
        let mut buf = [0u8; 8];
        r.read_exact(&mut buf)?;
        Ok(i64::from_le_bytes(buf))
    }
}

// Fixed-size byte arrays
impl Encodable for [u8; 4] {
    fn encode<W: Write>(&self, w: &mut W) -> Result<usize, Error> {
        w.write_all(self)?;
        Ok(4)
    }
}

impl Decodable for [u8; 4] {
    fn decode<R: Read>(r: &mut R) -> Result<Self, Error> {
        let mut buf = [0u8; 4];
        r.read_exact(&mut buf)?;
        Ok(buf)
    }
}

impl Encodable for [u8; 20] {
    fn encode<W: Write>(&self, w: &mut W) -> Result<usize, Error> {
        w.write_all(self)?;
        Ok(20)
    }
}

impl Decodable for [u8; 20] {
    fn decode<R: Read>(r: &mut R) -> Result<Self, Error> {
        let mut buf = [0u8; 20];
        r.read_exact(&mut buf)?;
        Ok(buf)
    }
}

impl Encodable for [u8; 32] {
    fn encode<W: Write>(&self, w: &mut W) -> Result<usize, Error> {
        w.write_all(self)?;
        Ok(32)
    }
}

impl Decodable for [u8; 32] {
    fn decode<R: Read>(r: &mut R) -> Result<Self, Error> {
        let mut buf = [0u8; 32];
        r.read_exact(&mut buf)?;
        Ok(buf)
    }
}

impl Encodable for [u8; 33] {
    fn encode<W: Write>(&self, w: &mut W) -> Result<usize, Error> {
        w.write_all(self)?;
        Ok(33)
    }
}

impl Decodable for [u8; 33] {
    fn decode<R: Read>(r: &mut R) -> Result<Self, Error> {
        let mut buf = [0u8; 33];
        r.read_exact(&mut buf)?;
        Ok(buf)
    }
}

impl Encodable for [u8; 64] {
    fn encode<W: Write>(&self, w: &mut W) -> Result<usize, Error> {
        w.write_all(self)?;
        Ok(64)
    }
}

impl Decodable for [u8; 64] {
    fn decode<R: Read>(r: &mut R) -> Result<Self, Error> {
        let mut buf = [0u8; 64];
        r.read_exact(&mut buf)?;
        Ok(buf)
    }
}

// Variable-length byte vectors (prefixed with CompactSize length)
impl Encodable for Vec<u8> {
    fn encode<W: Write>(&self, w: &mut W) -> Result<usize, Error> {
        let mut size = crate::compact_size::write_compact_size(w, self.len() as u64)?;
        w.write_all(self)?;
        size += self.len();
        Ok(size)
    }
}

impl Decodable for Vec<u8> {
    fn decode<R: Read>(r: &mut R) -> Result<Self, Error> {
        let len = crate::compact_size::read_compact_size(r)?;
        if len > MAX_SIZE {
            return Err(Error::OversizedVector(len));
        }
        let mut buf = vec![0u8; len as usize];
        r.read_exact(&mut buf)?;
        Ok(buf)
    }
}

// String (prefixed with CompactSize length)
impl Encodable for String {
    fn encode<W: Write>(&self, w: &mut W) -> Result<usize, Error> {
        let bytes = self.as_bytes();
        let mut size = crate::compact_size::write_compact_size(w, bytes.len() as u64)?;
        w.write_all(bytes)?;
        size += bytes.len();
        Ok(size)
    }
}

impl Decodable for String {
    fn decode<R: Read>(r: &mut R) -> Result<Self, Error> {
        let bytes = Vec::<u8>::decode(r)?;
        String::from_utf8(bytes).map_err(|e| Error::InvalidEncoding(e.to_string()))
    }
}

// Implement for Uint256
impl Encodable for qubitcoin_primitives::Uint256 {
    fn encode<W: Write>(&self, w: &mut W) -> Result<usize, Error> {
        w.write_all(self.as_bytes())?;
        Ok(32)
    }
}

impl Decodable for qubitcoin_primitives::Uint256 {
    fn decode<R: Read>(r: &mut R) -> Result<Self, Error> {
        let mut buf = [0u8; 32];
        r.read_exact(&mut buf)?;
        Ok(qubitcoin_primitives::Uint256::from_bytes(buf))
    }
}

// Implement for hash types
impl Encodable for qubitcoin_primitives::Txid {
    fn encode<W: Write>(&self, w: &mut W) -> Result<usize, Error> {
        w.write_all(self.as_bytes())?;
        Ok(32)
    }
}

impl Decodable for qubitcoin_primitives::Txid {
    fn decode<R: Read>(r: &mut R) -> Result<Self, Error> {
        let mut buf = [0u8; 32];
        r.read_exact(&mut buf)?;
        Ok(qubitcoin_primitives::Txid::from_bytes(buf))
    }
}

impl Encodable for qubitcoin_primitives::Wtxid {
    fn encode<W: Write>(&self, w: &mut W) -> Result<usize, Error> {
        w.write_all(self.as_bytes())?;
        Ok(32)
    }
}

impl Decodable for qubitcoin_primitives::Wtxid {
    fn decode<R: Read>(r: &mut R) -> Result<Self, Error> {
        let mut buf = [0u8; 32];
        r.read_exact(&mut buf)?;
        Ok(qubitcoin_primitives::Wtxid::from_bytes(buf))
    }
}

impl Encodable for qubitcoin_primitives::BlockHash {
    fn encode<W: Write>(&self, w: &mut W) -> Result<usize, Error> {
        w.write_all(self.as_bytes())?;
        Ok(32)
    }
}

impl Decodable for qubitcoin_primitives::BlockHash {
    fn decode<R: Read>(r: &mut R) -> Result<Self, Error> {
        let mut buf = [0u8; 32];
        r.read_exact(&mut buf)?;
        Ok(qubitcoin_primitives::BlockHash::from_bytes(buf))
    }
}

/// Encodes a slice of [`Encodable`] items as a CompactSize length prefix followed by each item.
///
/// This is the standard Bitcoin serialization format for vectors of objects.
/// Returns the total number of bytes written.
pub fn encode_vec<W: Write, T: Encodable>(w: &mut W, vec: &[T]) -> Result<usize, Error> {
    let mut size = crate::compact_size::write_compact_size(w, vec.len() as u64)?;
    for item in vec {
        size += item.encode(w)?;
    }
    Ok(size)
}

/// Decodes a CompactSize-prefixed vector of [`Decodable`] items from `r`.
///
/// Reads a CompactSize length, then decodes that many items sequentially.
/// Rejects vectors whose declared length exceeds [`MAX_SIZE`].
pub fn decode_vec<R: Read, T: Decodable>(r: &mut R) -> Result<Vec<T>, Error> {
    let len = crate::compact_size::read_compact_size(r)?;
    if len > MAX_SIZE {
        return Err(Error::OversizedVector(len));
    }
    let mut vec = Vec::with_capacity(std::cmp::min(len as usize, MAX_VECTOR_ALLOCATE));
    for _ in 0..len {
        vec.push(T::decode(r)?);
    }
    Ok(vec)
}

/// Serializes an [`Encodable`] value into a new `Vec<u8>`.
///
/// This is a convenience wrapper around [`Encodable::encode`].
pub fn serialize<T: Encodable>(obj: &T) -> Result<Vec<u8>, Error> {
    let mut buf = Vec::new();
    obj.encode(&mut buf)?;
    Ok(buf)
}

/// Deserializes a [`Decodable`] value from a byte slice.
///
/// This is a convenience wrapper around [`Decodable::decode`].
pub fn deserialize<T: Decodable>(data: &[u8]) -> Result<T, Error> {
    let mut cursor = io::Cursor::new(data);
    T::decode(&mut cursor)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_u8_roundtrip() {
        let val: u8 = 42;
        let data = serialize(&val).unwrap();
        assert_eq!(data, vec![42]);
        let decoded: u8 = deserialize(&data).unwrap();
        assert_eq!(decoded, 42);
    }

    #[test]
    fn test_u32_le() {
        let val: u32 = 0x12345678;
        let data = serialize(&val).unwrap();
        assert_eq!(data, vec![0x78, 0x56, 0x34, 0x12]);
        let decoded: u32 = deserialize(&data).unwrap();
        assert_eq!(decoded, 0x12345678);
    }

    #[test]
    fn test_i64_roundtrip() {
        let val: i64 = -1;
        let data = serialize(&val).unwrap();
        let decoded: i64 = deserialize(&data).unwrap();
        assert_eq!(decoded, -1);
    }

    #[test]
    fn test_bool_roundtrip() {
        let data = serialize(&true).unwrap();
        assert_eq!(data, vec![1]);
        let decoded: bool = deserialize(&data).unwrap();
        assert!(decoded);
    }

    #[test]
    fn test_fixed_bytes_roundtrip() {
        let val: [u8; 32] = [0xab; 32];
        let data = serialize(&val).unwrap();
        assert_eq!(data.len(), 32);
        let decoded: [u8; 32] = deserialize(&data).unwrap();
        assert_eq!(decoded, val);
    }

    #[test]
    fn test_vec_u8_roundtrip() {
        let val = vec![1u8, 2, 3, 4, 5];
        let data = serialize(&val).unwrap();
        // CompactSize(5) + 5 bytes = 6 bytes
        assert_eq!(data.len(), 6);
        let decoded: Vec<u8> = deserialize(&data).unwrap();
        assert_eq!(decoded, val);
    }

    #[test]
    fn test_string_roundtrip() {
        let val = "hello world".to_string();
        let data = serialize(&val).unwrap();
        let decoded: String = deserialize(&data).unwrap();
        assert_eq!(decoded, val);
    }

    #[test]
    fn test_uint256_roundtrip() {
        let val = qubitcoin_primitives::Uint256::from_hex(
            "000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f",
        )
        .unwrap();
        let data = serialize(&val).unwrap();
        assert_eq!(data.len(), 32);
        let decoded: qubitcoin_primitives::Uint256 = deserialize(&data).unwrap();
        assert_eq!(decoded, val);
    }
}
