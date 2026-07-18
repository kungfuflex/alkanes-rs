//! Newtype wrappers for specific hash uses.
//! Maps to: various typedefs in Bitcoin Core (uint256 used as Txid, BlockHash, etc.)
//!
//! These provide type safety - you can't accidentally pass a Txid where a BlockHash is expected.

use crate::uint256::Uint256;
use std::fmt;

macro_rules! define_hash_type {
    ($name:ident, $doc:expr) => {
        #[doc = $doc]
        #[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
        pub struct $name(Uint256);

        impl $name {
            /// Creates a new instance from a [`Uint256`].
            pub const fn from_uint256(hash: Uint256) -> Self {
                $name(hash)
            }

            /// Creates a new instance from a byte slice. Panics if `slice.len() != 32`.
            pub fn from_slice(slice: &[u8]) -> Self {
                $name(Uint256::from_slice(slice))
            }

            /// Creates a new instance from a 32-byte array.
            pub fn from_bytes(bytes: [u8; 32]) -> Self {
                $name(Uint256::from_bytes(bytes))
            }

            /// Parses from a reversed-byte hex string (Bitcoin Core display convention).
            /// Returns `None` if the hex string is invalid or not exactly 64 characters.
            pub fn from_hex(hex_str: &str) -> Option<Self> {
                Uint256::from_hex(hex_str).map($name)
            }

            /// Returns the reversed-byte hex representation (Bitcoin Core display convention).
            pub fn to_hex(&self) -> String {
                self.0.to_hex()
            }

            /// Returns `true` if all bytes are zero (null hash).
            pub fn is_null(&self) -> bool {
                self.0.is_null()
            }

            /// Returns a reference to the underlying [`Uint256`].
            pub fn as_uint256(&self) -> &Uint256 {
                &self.0
            }

            /// Consumes this value and returns the underlying [`Uint256`].
            pub fn into_uint256(self) -> Uint256 {
                self.0
            }

            /// Returns the raw bytes as a slice.
            pub fn as_bytes(&self) -> &[u8] {
                self.0.as_bytes()
            }

            /// Returns a reference to the underlying 32-byte array.
            pub fn data(&self) -> &[u8; 32] {
                self.0.data()
            }
        }

        impl From<Uint256> for $name {
            fn from(hash: Uint256) -> Self {
                $name(hash)
            }
        }

        impl From<$name> for Uint256 {
            fn from(hash: $name) -> Uint256 {
                hash.0
            }
        }

        impl From<[u8; 32]> for $name {
            fn from(bytes: [u8; 32]) -> Self {
                $name(Uint256::from_bytes(bytes))
            }
        }

        impl AsRef<[u8]> for $name {
            fn as_ref(&self) -> &[u8] {
                self.0.as_ref()
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}({})", stringify!($name), self.0.to_hex())
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0.to_hex())
            }
        }
    };
}

define_hash_type!(
    Txid,
    "Transaction ID (double SHA256 of serialized tx without witness)."
);
define_hash_type!(
    Wtxid,
    "Witness transaction ID (double SHA256 of serialized tx with witness)."
);
define_hash_type!(BlockHash, "Block hash (double SHA256 of block header).");

/// Null/zero constants for hash types.
impl Txid {
    /// The all-zero transaction ID, used as a sentinel/null value.
    pub const ZERO: Txid = Txid(Uint256::ZERO);
}

impl Wtxid {
    /// The all-zero witness transaction ID, used as a sentinel/null value.
    pub const ZERO: Wtxid = Wtxid(Uint256::ZERO);
}

impl BlockHash {
    /// The all-zero block hash, used as a sentinel/null value.
    pub const ZERO: BlockHash = BlockHash(Uint256::ZERO);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_txid_from_hex() {
        let hex = "4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b";
        let txid = Txid::from_hex(hex).unwrap();
        assert_eq!(txid.to_hex(), hex);
    }

    #[test]
    fn test_type_safety() {
        let txid = Txid::from_bytes([1u8; 32]);
        let block_hash = BlockHash::from_bytes([1u8; 32]);
        // These are different types even though same bytes
        let _: Uint256 = txid.into();
        let _: Uint256 = block_hash.into();
    }

    #[test]
    fn test_null() {
        assert!(Txid::ZERO.is_null());
        assert!(BlockHash::ZERO.is_null());
    }
}
