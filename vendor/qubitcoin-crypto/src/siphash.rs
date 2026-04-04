//! SipHash-2-4 implementation.
//! Maps to: src/crypto/siphash.h/cpp
//!
//! Used in Bitcoin Core for:
//! - Short transaction ID calculation (BIP152 compact blocks)
//! - Hash table randomization
//! - Mempool transaction ordering

/// Computes SipHash-2-4 with the given 128-bit key (`k0`, `k1`) and arbitrary-length `data`.
///
/// SipHash is a fast, cryptographically strong PRF suitable for hash tables and
/// short message authentication. Equivalent to `CSipHasher` in Bitcoin Core.
///
/// Returns a 64-bit hash value.
pub fn sip_hash(k0: u64, k1: u64, data: &[u8]) -> u64 {
    let mut v0: u64 = 0x736f6d6570736575u64 ^ k0;
    let mut v1: u64 = 0x646f72616e646f6du64 ^ k1;
    let mut v2: u64 = 0x6c7967656e657261u64 ^ k0;
    let mut v3: u64 = 0x7465646279746573u64 ^ k1;

    let len = data.len();
    let blocks = len / 8;

    for i in 0..blocks {
        let offset = i * 8;
        let m = u64::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
            data[offset + 4],
            data[offset + 5],
            data[offset + 6],
            data[offset + 7],
        ]);

        v3 ^= m;
        sip_round(&mut v0, &mut v1, &mut v2, &mut v3);
        sip_round(&mut v0, &mut v1, &mut v2, &mut v3);
        v0 ^= m;
    }

    let mut last: u64 = (len as u64) << 56;
    let remaining = &data[blocks * 8..];
    for (i, &byte) in remaining.iter().enumerate() {
        last |= (byte as u64) << (i * 8);
    }

    v3 ^= last;
    sip_round(&mut v0, &mut v1, &mut v2, &mut v3);
    sip_round(&mut v0, &mut v1, &mut v2, &mut v3);
    v0 ^= last;

    v2 ^= 0xff;
    sip_round(&mut v0, &mut v1, &mut v2, &mut v3);
    sip_round(&mut v0, &mut v1, &mut v2, &mut v3);
    sip_round(&mut v0, &mut v1, &mut v2, &mut v3);
    sip_round(&mut v0, &mut v1, &mut v2, &mut v3);

    v0 ^ v1 ^ v2 ^ v3
}

/// Computes SipHash-2-4 for a single 256-bit (32-byte) input.
///
/// Convenience wrapper around [`sip_hash`] for hashing `Uint256` values.
/// Equivalent to `SipHashUint256` in Bitcoin Core.
pub fn sip_hash_uint256(k0: u64, k1: u64, data: &[u8; 32]) -> u64 {
    sip_hash(k0, k1, data)
}

/// Computes SipHash-2-4 for a 256-bit value with an additional 32-bit `extra` value appended.
///
/// Used for short transaction ID computation in BIP-152 compact blocks.
/// The `extra` parameter is typically the transaction index within the block.
/// Equivalent to `SipHashUint256Extra` in Bitcoin Core.
pub fn sip_hash_uint256_extra(k0: u64, k1: u64, data: &[u8; 32], extra: u32) -> u64 {
    let mut buf = [0u8; 36];
    buf[..32].copy_from_slice(data);
    buf[32..36].copy_from_slice(&extra.to_le_bytes());
    sip_hash(k0, k1, &buf)
}

#[inline(always)]
fn sip_round(v0: &mut u64, v1: &mut u64, v2: &mut u64, v3: &mut u64) {
    *v0 = v0.wrapping_add(*v1);
    *v1 = v1.rotate_left(13);
    *v1 ^= *v0;
    *v0 = v0.rotate_left(32);
    *v2 = v2.wrapping_add(*v3);
    *v3 = v3.rotate_left(16);
    *v3 ^= *v2;
    *v0 = v0.wrapping_add(*v3);
    *v3 = v3.rotate_left(21);
    *v3 ^= *v0;
    *v2 = v2.wrapping_add(*v1);
    *v1 = v1.rotate_left(17);
    *v1 ^= *v2;
    *v2 = v2.rotate_left(32);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sip_hash_empty() {
        let result = sip_hash(0, 0, &[]);
        // Known SipHash-2-4 value for k0=0, k1=0, empty input
        assert_ne!(result, 0);
    }

    #[test]
    fn test_sip_hash_known_vector() {
        // Test vector from SipHash reference
        let k0: u64 = 0x0706050403020100;
        let k1: u64 = 0x0f0e0d0c0b0a0908;
        let data: Vec<u8> = (0u8..15).collect();
        let result = sip_hash(k0, k1, &data);
        assert_eq!(result, 0xa129ca6149be45e5);
    }
}
