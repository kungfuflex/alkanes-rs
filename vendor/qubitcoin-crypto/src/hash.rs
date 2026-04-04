//! Cryptographic hash functions wrapping bitcoin_hashes.
//! Maps to: src/crypto/ (SHA256, RIPEMD160, etc.)

/// Re-export of HASH160 (RIPEMD160(SHA256)) types from `bitcoin_hashes`.
pub use bitcoin_hashes::hash160;
/// Re-export of RIPEMD160 types from `bitcoin_hashes`.
pub use bitcoin_hashes::ripemd160;
/// Re-export of SHA-1 types from `bitcoin_hashes`.
pub use bitcoin_hashes::sha1;
/// Re-export of SHA-256 types from `bitcoin_hashes`.
pub use bitcoin_hashes::sha256;
/// Re-export of double-SHA-256 types from `bitcoin_hashes`.
pub use bitcoin_hashes::sha256d;
/// Re-export of SHA-512 types from `bitcoin_hashes`.
pub use bitcoin_hashes::sha512;
/// Re-export of the `Hash` trait from `bitcoin_hashes`.
pub use bitcoin_hashes::Hash;
/// Re-export of the `HashEngine` trait from `bitcoin_hashes` for incremental hashing.
pub use bitcoin_hashes::HashEngine;

/// Computes the double-SHA256 hash: `SHA256(SHA256(data))`.
///
/// This is the primary hash used in Bitcoin for block hashes, transaction IDs, and
/// Merkle trees. Equivalent to `CHashWriter` / `Hash()` in Bitcoin Core.
///
/// Returns a 32-byte digest.
#[inline]
pub fn hash256(data: &[u8]) -> [u8; 32] {
    let hash = sha256d::Hash::hash(data);
    let bytes: &[u8] = hash.as_ref();
    let mut result = [0u8; 32];
    result.copy_from_slice(bytes);
    result
}

/// Computes HASH160: `RIPEMD160(SHA256(data))`.
///
/// Used for Bitcoin addresses (P2PKH, P2SH). Equivalent to `CHash160` in Bitcoin Core.
///
/// Returns a 20-byte digest.
#[inline]
pub fn hash160(data: &[u8]) -> [u8; 20] {
    let hash = bitcoin_hashes::hash160::Hash::hash(data);
    let bytes: &[u8] = hash.as_ref();
    let mut result = [0u8; 20];
    result.copy_from_slice(bytes);
    result
}

/// Computes a single SHA-256 hash of the input data.
///
/// Returns a 32-byte digest. For double-hashing, use [`hash256`] instead.
#[inline]
pub fn sha256_hash(data: &[u8]) -> [u8; 32] {
    let hash = sha256::Hash::hash(data);
    let bytes: &[u8] = hash.as_ref();
    let mut result = [0u8; 32];
    result.copy_from_slice(bytes);
    result
}

/// Computes a RIPEMD-160 hash of the input data.
///
/// Returns a 20-byte digest. Rarely used standalone; typically combined with
/// SHA-256 via [`hash160()`].
#[inline]
pub fn ripemd160_hash(data: &[u8]) -> [u8; 20] {
    let hash = ripemd160::Hash::hash(data);
    let bytes: &[u8] = hash.as_ref();
    let mut result = [0u8; 20];
    result.copy_from_slice(bytes);
    result
}

/// Computes a SHA-1 hash of the input data.
///
/// Returns a 20-byte digest. Used by the `OP_SHA1` script opcode.
/// SHA-1 is considered cryptographically weak; this exists only for script compatibility.
#[inline]
pub fn sha1_hash(data: &[u8]) -> [u8; 20] {
    let hash = sha1::Hash::hash(data);
    let bytes: &[u8] = hash.as_ref();
    let mut result = [0u8; 20];
    result.copy_from_slice(bytes);
    result
}

/// Computes a tagged hash per BIP-340: `SHA256(SHA256(tag) || SHA256(tag) || msg)`.
///
/// Tagged hashes provide domain separation, ensuring that hashes computed for
/// different purposes (e.g., Taproot key tweaks vs. signature challenges) cannot collide.
///
/// Returns a 32-byte digest.
pub fn tagged_hash(tag: &[u8], msg: &[u8]) -> [u8; 32] {
    let tag_hash = sha256_hash(tag);
    let mut engine = sha256::HashEngine::default();
    bitcoin_hashes::HashEngine::input(&mut engine, &tag_hash);
    bitcoin_hashes::HashEngine::input(&mut engine, &tag_hash);
    bitcoin_hashes::HashEngine::input(&mut engine, msg);
    let hash = sha256::Hash::from_engine(engine);
    let bytes: &[u8] = hash.as_ref();
    let mut result = [0u8; 32];
    result.copy_from_slice(bytes);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash256_empty() {
        // SHA256d of empty input
        let result = hash256(b"");
        let expected =
            hex::decode("5df6e0e2761359d30a8275058e299fcc0381534545f55cf43e41983f5d4c9456")
                .unwrap();
        assert_eq!(&result[..], &expected[..]);
    }

    #[test]
    fn test_hash160_empty() {
        let result = hash160(b"");
        let expected = hex::decode("b472a266d0bd89c13706a4132ccfb16f7c3b9fcb").unwrap();
        assert_eq!(&result[..], &expected[..]);
    }

    #[test]
    fn test_sha256_abc() {
        let result = sha256_hash(b"abc");
        let expected =
            hex::decode("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
                .unwrap();
        assert_eq!(&result[..], &expected[..]);
    }
}
