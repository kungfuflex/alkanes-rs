//! Fixed-size opaque blob types: Uint160, Uint256.
//! Maps to: `src/uint256.h` (`base_blob`)
//!
//! These are NOT integer types - they are opaque byte blobs used for hashes.
//! For arithmetic operations, use ArithUint256.
//!
//! Hex representation is in REVERSE byte order (little-endian display),
//! matching Bitcoin Core's convention where hash hex strings show the
//! most significant byte first even though internal storage is LE.

use std::fmt;

/// A 256-bit opaque byte blob used for all Bitcoin hash types (`Txid`, `BlockHash`, etc.).
///
/// This is **not** an integer type -- it has no arithmetic operations. For arithmetic
/// on 256-bit values (e.g., difficulty target comparison), convert to `ArithUint256`
/// via `uint256_to_arith`.
///
/// Internal storage is a 32-byte array in little-endian order. Hex representation
/// reverses byte order to match Bitcoin Core's display convention.
///
/// Equivalent to `base_blob<256>` / `uint256` in Bitcoin Core.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Uint256 {
    /// The raw 32 bytes stored in little-endian order.
    data: [u8; 32],
}

/// A 160-bit opaque byte blob used for RIPEMD-160 and Hash160 outputs.
///
/// Equivalent to `base_blob<160>` / `uint160` in Bitcoin Core.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Uint160 {
    /// The raw 20 bytes stored in little-endian order.
    data: [u8; 20],
}

impl Uint256 {
    /// The all-zero 256-bit value, used as a null/sentinel constant.
    pub const ZERO: Uint256 = Uint256 { data: [0u8; 32] };
    /// The value one (byte 0 is `0x01`, all others `0x00`).
    pub const ONE: Uint256 = Uint256 {
        data: [
            1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0,
        ],
    };
    /// The size of a `Uint256` in bytes (always 32).
    pub const SIZE: usize = 32;

    /// Creates a `Uint256` from a single byte value, stored at position 0 with the rest zeroed.
    pub const fn from_u8(v: u8) -> Self {
        let mut data = [0u8; 32];
        data[0] = v;
        Uint256 { data }
    }

    /// Creates a `Uint256` from a 32-byte slice.
    ///
    /// # Panics
    /// Panics if `slice.len() != 32`.
    pub fn from_slice(slice: &[u8]) -> Self {
        assert!(
            slice.len() == 32,
            "Uint256::from_slice requires exactly 32 bytes"
        );
        let mut data = [0u8; 32];
        data.copy_from_slice(slice);
        Uint256 { data }
    }

    /// Creates a `Uint256` from a 32-byte array (no byte-order conversion).
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Uint256 { data: bytes }
    }

    /// Parses from a reversed-byte hex string (Bitcoin Core display convention).
    ///
    /// The hex string represents the hash in display order (most significant byte first),
    /// which is the reverse of the internal little-endian byte order. Returns `None` if
    /// the string is not exactly 64 hex characters or contains invalid hex digits.
    pub fn from_hex(hex_str: &str) -> Option<Self> {
        if hex_str.len() != 64 {
            return None;
        }
        let bytes = hex::decode(hex_str).ok()?;
        let mut data = [0u8; 32];
        // Reverse byte order: hex is display (BE), storage is LE
        for i in 0..32 {
            data[i] = bytes[31 - i];
        }
        Some(Uint256 { data })
    }

    /// Parses from a reversed-byte hex string with optional `0x` prefix and automatic zero-padding.
    ///
    /// Short hex strings are left-padded with zeros to 64 characters before parsing.
    /// For example, `"0x1"` is treated as `"0000...0001"`.
    pub fn from_user_hex(input: &str) -> Option<Self> {
        let input = input.strip_prefix("0x").unwrap_or(input);
        if input.len() > 64 {
            return None;
        }
        let padded = format!("{:0>64}", input);
        Self::from_hex(&padded)
    }

    /// Returns the reversed-byte hex string (Bitcoin Core display convention).
    ///
    /// The output shows the most significant byte first, which is the standard
    /// display format for block hashes and transaction IDs.
    pub fn to_hex(&self) -> String {
        let mut reversed = [0u8; 32];
        for i in 0..32 {
            reversed[i] = self.data[31 - i];
        }
        hex::encode(reversed)
    }

    /// Returns `true` if all 32 bytes are zero (null hash).
    pub fn is_null(&self) -> bool {
        self.data.iter().all(|&b| b == 0)
    }

    /// Sets all 32 bytes to zero, making this a null hash.
    pub fn set_null(&mut self) {
        self.data = [0u8; 32];
    }

    /// Performs lexicographic byte comparison (compares from byte 0 upward).
    pub fn compare(&self, other: &Uint256) -> std::cmp::Ordering {
        self.data.cmp(&other.data)
    }

    /// Reads a little-endian `u64` at the given `pos` (0-indexed, in units of 8 bytes).
    ///
    /// Position 0 reads bytes 0..8, position 1 reads bytes 8..16, etc.
    pub fn get_u64(&self, pos: usize) -> u64 {
        let offset = pos * 8;
        u64::from_le_bytes([
            self.data[offset],
            self.data[offset + 1],
            self.data[offset + 2],
            self.data[offset + 3],
            self.data[offset + 4],
            self.data[offset + 5],
            self.data[offset + 6],
            self.data[offset + 7],
        ])
    }

    /// Returns an immutable reference to the underlying 32-byte array.
    pub fn data(&self) -> &[u8; 32] {
        &self.data
    }

    /// Returns a mutable reference to the underlying 32-byte array.
    pub fn data_mut(&mut self) -> &mut [u8; 32] {
        &mut self.data
    }

    /// Returns the underlying data as a byte slice.
    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }

    /// Returns the size of a `Uint256` in bytes (always 32).
    pub const fn size() -> usize {
        32
    }
}

impl Ord for Uint256 {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.data.cmp(&other.data)
    }
}

impl PartialOrd for Uint256 {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Debug for Uint256 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Uint256({})", self.to_hex())
    }
}

impl fmt::Display for Uint256 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

impl AsRef<[u8]> for Uint256 {
    fn as_ref(&self) -> &[u8] {
        &self.data
    }
}

impl From<[u8; 32]> for Uint256 {
    fn from(data: [u8; 32]) -> Self {
        Uint256 { data }
    }
}

impl From<Uint256> for [u8; 32] {
    fn from(val: Uint256) -> Self {
        val.data
    }
}

// --- Uint160 ---

impl Uint160 {
    /// The all-zero 160-bit value, used as a null/sentinel constant.
    pub const ZERO: Uint160 = Uint160 { data: [0u8; 20] };
    /// The size of a `Uint160` in bytes (always 20).
    pub const SIZE: usize = 20;

    /// Creates a `Uint160` from a 20-byte array (no byte-order conversion).
    pub const fn from_bytes(bytes: [u8; 20]) -> Self {
        Uint160 { data: bytes }
    }

    /// Creates a `Uint160` from a 20-byte slice.
    ///
    /// # Panics
    /// Panics if `slice.len() != 20`.
    pub fn from_slice(slice: &[u8]) -> Self {
        assert!(
            slice.len() == 20,
            "Uint160::from_slice requires exactly 20 bytes"
        );
        let mut data = [0u8; 20];
        data.copy_from_slice(slice);
        Uint160 { data }
    }

    /// Parses from a reversed-byte hex string (Bitcoin Core display convention).
    /// Returns `None` if the string is not exactly 40 hex characters or contains invalid hex digits.
    pub fn from_hex(hex_str: &str) -> Option<Self> {
        if hex_str.len() != 40 {
            return None;
        }
        let bytes = hex::decode(hex_str).ok()?;
        let mut data = [0u8; 20];
        for i in 0..20 {
            data[i] = bytes[19 - i];
        }
        Some(Uint160 { data })
    }

    /// Returns the reversed-byte hex string (Bitcoin Core display convention).
    pub fn to_hex(&self) -> String {
        let mut reversed = [0u8; 20];
        for i in 0..20 {
            reversed[i] = self.data[19 - i];
        }
        hex::encode(reversed)
    }

    /// Returns `true` if all 20 bytes are zero (null hash).
    pub fn is_null(&self) -> bool {
        self.data.iter().all(|&b| b == 0)
    }

    /// Returns an immutable reference to the underlying 20-byte array.
    pub fn data(&self) -> &[u8; 20] {
        &self.data
    }

    /// Returns the underlying data as a byte slice.
    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }
}

impl fmt::Debug for Uint160 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Uint160({})", self.to_hex())
    }
}

impl fmt::Display for Uint160 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

impl AsRef<[u8]> for Uint160 {
    fn as_ref(&self) -> &[u8] {
        &self.data
    }
}

impl From<[u8; 20]> for Uint160 {
    fn from(data: [u8; 20]) -> Self {
        Uint160 { data }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uint256_zero() {
        let z = Uint256::ZERO;
        assert!(z.is_null());
        assert_eq!(
            z.to_hex(),
            "0000000000000000000000000000000000000000000000000000000000000000"
        );
    }

    #[test]
    fn test_uint256_one() {
        let one = Uint256::ONE;
        assert!(!one.is_null());
        // ONE stored as LE: [1, 0, 0, ...] -> display hex is reversed: "00...01"
        assert_eq!(
            one.to_hex(),
            "0000000000000000000000000000000000000000000000000000000000000001"
        );
    }

    #[test]
    fn test_uint256_from_hex_roundtrip() {
        let hex_str = "000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f";
        let u = Uint256::from_hex(hex_str).unwrap();
        assert_eq!(u.to_hex(), hex_str);
    }

    #[test]
    fn test_uint256_from_u8() {
        let u = Uint256::from_u8(42);
        assert_eq!(u.data()[0], 42);
        for i in 1..32 {
            assert_eq!(u.data()[i], 0);
        }
    }

    #[test]
    fn test_uint256_get_u64() {
        let mut data = [0u8; 32];
        data[0] = 0x01;
        data[1] = 0x02;
        let u = Uint256::from_bytes(data);
        assert_eq!(u.get_u64(0), 0x0201);
    }

    #[test]
    fn test_uint256_comparison() {
        let a = Uint256::ZERO;
        let b = Uint256::ONE;
        assert!(a < b);
        assert_eq!(a, Uint256::ZERO);
    }

    #[test]
    fn test_uint256_from_user_hex() {
        let u = Uint256::from_user_hex("0x1").unwrap();
        assert_eq!(u, Uint256::ONE);

        let u2 = Uint256::from_user_hex("1").unwrap();
        assert_eq!(u2, Uint256::ONE);
    }

    #[test]
    fn test_uint160_from_hex() {
        let hex_str = "0000000000000000000000000000000000000001";
        let u = Uint160::from_hex(hex_str).unwrap();
        assert_eq!(u.data()[0], 1);
        assert_eq!(u.to_hex(), hex_str);
    }
}
