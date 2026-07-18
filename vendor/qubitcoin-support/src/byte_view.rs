//! ByteView — type-safe serialization for KV pointer values.
//!
//! Mirrors metashrew-support's ByteView trait so indexers can
//! store/retrieve typed values (u32, u128, etc.) from storage.

/// A type that can be serialized to/from a byte vector (little-endian).
pub trait ByteView: Sized {
    fn from_bytes(v: Vec<u8>) -> Self;
    fn to_bytes(&self) -> Vec<u8>;
    fn maximum() -> Self;
    fn zero() -> Self;
}

impl ByteView for u8 {
    fn from_bytes(v: Vec<u8>) -> Self { if v.is_empty() { 0 } else { v[0] } }
    fn to_bytes(&self) -> Vec<u8> { vec![*self] }
    fn maximum() -> Self { u8::MAX }
    fn zero() -> Self { 0 }
}

impl ByteView for u16 {
    fn from_bytes(v: Vec<u8>) -> Self {
        let mut b = [0u8; 2];
        let n = v.len().min(2);
        b[..n].copy_from_slice(&v[..n]);
        u16::from_le_bytes(b)
    }
    fn to_bytes(&self) -> Vec<u8> { self.to_le_bytes().to_vec() }
    fn maximum() -> Self { u16::MAX }
    fn zero() -> Self { 0 }
}

impl ByteView for u32 {
    fn from_bytes(v: Vec<u8>) -> Self {
        let mut b = [0u8; 4];
        let n = v.len().min(4);
        b[..n].copy_from_slice(&v[..n]);
        u32::from_le_bytes(b)
    }
    fn to_bytes(&self) -> Vec<u8> { self.to_le_bytes().to_vec() }
    fn maximum() -> Self { u32::MAX }
    fn zero() -> Self { 0 }
}

impl ByteView for u64 {
    fn from_bytes(v: Vec<u8>) -> Self {
        let mut b = [0u8; 8];
        let n = v.len().min(8);
        b[..n].copy_from_slice(&v[..n]);
        u64::from_le_bytes(b)
    }
    fn to_bytes(&self) -> Vec<u8> { self.to_le_bytes().to_vec() }
    fn maximum() -> Self { u64::MAX }
    fn zero() -> Self { 0 }
}

impl ByteView for u128 {
    fn from_bytes(v: Vec<u8>) -> Self {
        let mut b = [0u8; 16];
        let n = v.len().min(16);
        b[..n].copy_from_slice(&v[..n]);
        u128::from_le_bytes(b)
    }
    fn to_bytes(&self) -> Vec<u8> { self.to_le_bytes().to_vec() }
    fn maximum() -> Self { u128::MAX }
    fn zero() -> Self { 0 }
}
