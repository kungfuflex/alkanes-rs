//! MuHash3072: Rolling hash for UTXO set commitment.
//!
//! MuHash3072 is a set hash function that supports efficient incremental
//! updates. It maps the UTXO set to a single hash value that can be
//! updated in O(1) per UTXO addition/removal, regardless of set size.
//!
//! This is an improvement over Bitcoin Core where UTXO set hashing was
//! added later (PR#19055). We make it a first-class feature.
//!
//! The algorithm:
//! - Uses a 3072-bit prime modulus (the largest 3072-bit safe prime)
//! - Hash each element with SHA256 then expand to 3072 bits via ChaCha20
//! - Multiply all element hashes together mod p
//! - The final hash is SHA256 of the reduced 3072-bit product
//!
//! Properties:
//! - Commutative: order of operations doesn't matter
//! - Supports removal via modular inverse
//! - Collision-resistant under the discrete log assumption

use crate::hash::sha256_hash;

/// The difference between `2^3072` and the modular prime `p`.
///
/// The prime is `p = 2^3072 - 1103717`, which is the largest 3072-bit safe prime.
/// Storing only the difference allows efficient modular reduction.
const PRIME_DIFF: u64 = 1103717;

/// Number of 32-bit limbs needed to represent a 3072-bit number (3072 / 32 = 96).
const LIMBS: usize = 96;

/// A 3072-bit unsigned integer stored as 96 x 32-bit limbs in little-endian order.
///
/// Used internally by [`MuHash`] for modular arithmetic over the prime field
/// `GF(p)` where `p = 2^3072 - 1103717`. Equivalent to `Num3072` in Bitcoin Core.
#[derive(Clone, Debug)]
pub struct Num3072 {
    /// The 96 limbs representing the number, with `limbs[0]` being the least significant.
    limbs: [u32; LIMBS],
}

impl Num3072 {
    /// Create the multiplicative identity (1).
    pub fn one() -> Self {
        let mut limbs = [0u32; LIMBS];
        limbs[0] = 1;
        Num3072 { limbs }
    }

    /// Create from 384 bytes (little-endian).
    pub fn from_bytes(bytes: &[u8; 384]) -> Self {
        let mut limbs = [0u32; LIMBS];
        for i in 0..LIMBS {
            limbs[i] = u32::from_le_bytes([
                bytes[i * 4],
                bytes[i * 4 + 1],
                bytes[i * 4 + 2],
                bytes[i * 4 + 3],
            ]);
        }
        Num3072 { limbs }
    }

    /// Convert to 384 bytes (little-endian).
    pub fn to_bytes(&self) -> [u8; 384] {
        let mut bytes = [0u8; 384];
        for i in 0..LIMBS {
            let b = self.limbs[i].to_le_bytes();
            bytes[i * 4] = b[0];
            bytes[i * 4 + 1] = b[1];
            bytes[i * 4 + 2] = b[2];
            bytes[i * 4 + 3] = b[3];
        }
        bytes
    }

    /// Check if this number is zero.
    pub fn is_zero(&self) -> bool {
        self.limbs.iter().all(|&l| l == 0)
    }

    /// Check if this number equals one.
    pub fn is_one(&self) -> bool {
        self.limbs[0] == 1 && self.limbs[1..].iter().all(|&l| l == 0)
    }

    /// Compare self against p = 2^3072 - PRIME_DIFF.
    /// Returns true if self >= p.
    fn is_overflow(&self) -> bool {
        // p = 2^3072 - PRIME_DIFF
        // p in limbs: all limbs are 0xFFFFFFFF except limb[0] which is
        // (2^32 - PRIME_DIFF) if PRIME_DIFF < 2^32, but PRIME_DIFF = 1103717
        // Actually, 2^3072 - PRIME_DIFF means:
        //   limb[0] = (2^32 - PRIME_DIFF) as u32 = 0xFFF10D4B (since 2^32 = 4294967296, 4294967296 - 1103717 = 4293863579 = 0xFFF10D4B)
        //   limb[1..95] = 0xFFFFFFFF
        // Wait, that's not right. 2^3072 in 96 limbs would be a carry beyond limb 95.
        // So 2^3072 - PRIME_DIFF = max_96_limb_value - PRIME_DIFF + 1
        // = (2^3072 - 1) - PRIME_DIFF + 1 = 2^3072 - PRIME_DIFF
        // In limbs (little-endian):
        //   All 0xFFFFFFFF minus borrowing PRIME_DIFF from limb[0]
        //   limb[0] = 0xFFFFFFFF - PRIME_DIFF + 1 = 0x100000000 - PRIME_DIFF = (4294967296 - 1103717) as u32
        //   limb[1..] = 0xFFFFFFFF
        let p_limb0: u32 = (0x1_0000_0000u64 - PRIME_DIFF) as u32;

        // Check from most significant limb down
        for i in (1..LIMBS).rev() {
            if self.limbs[i] < 0xFFFFFFFF {
                return false;
            }
            // If equal to 0xFFFFFFFF, continue checking lower limbs
        }
        // All upper limbs equal 0xFFFFFFFF, check limb[0]
        self.limbs[0] >= p_limb0
    }

    /// Reduce self modulo p if self >= p.
    /// Since self < 2*p at most, a single subtraction suffices.
    /// Subtracting p = 2^3072 - PRIME_DIFF means adding PRIME_DIFF and dropping the carry.
    fn reduce_once(&mut self) {
        if self.is_overflow() {
            let mut carry: u64 = PRIME_DIFF;
            for i in 0..LIMBS {
                carry += self.limbs[i] as u64;
                self.limbs[i] = carry as u32;
                carry >>= 32;
            }
            // The carry out corresponds to 2^3072 which we discard (mod p).
        }
    }

    /// Multiplies two 3072-bit numbers modulo `p`.
    ///
    /// Uses schoolbook O(n^2) multiplication followed by Barrett-style reduction.
    /// The result is fully reduced to the range `[0, p)`.
    pub fn mul_mod(&self, other: &Num3072) -> Num3072 {
        // Use 64-bit wide multiplication
        let mut result = [0u64; LIMBS * 2];

        for i in 0..LIMBS {
            let mut carry: u64 = 0;
            for j in 0..LIMBS {
                let prod = (self.limbs[i] as u64) * (other.limbs[j] as u64) + result[i + j] + carry;
                result[i + j] = prod & 0xFFFFFFFF;
                carry = prod >> 32;
            }
            result[i + LIMBS] += carry;
        }

        // Reduce modulo p = 2^3072 - PRIME_DIFF
        // result mod p = (result mod 2^3072) + (result / 2^3072) * PRIME_DIFF
        Self::reduce(&result)
    }

    /// Reduce a double-width number modulo p.
    fn reduce(wide: &[u64; LIMBS * 2]) -> Num3072 {
        let mut result = [0u64; LIMBS + 1];

        // Copy lower half
        for i in 0..LIMBS {
            result[i] = wide[i];
        }

        // Add upper half * PRIME_DIFF
        let mut carry: u64 = 0;
        for i in 0..LIMBS {
            let add = wide[i + LIMBS] * PRIME_DIFF + result[i] + carry;
            result[i] = add & 0xFFFFFFFF;
            carry = add >> 32;
        }
        result[LIMBS] = carry;

        // May need another round of reduction if there's overflow into result[LIMBS]
        // Since the high part is now at most a few limbs, we do one more reduction pass.
        let mut limbs = [0u32; LIMBS];
        for i in 0..LIMBS {
            limbs[i] = result[i] as u32;
        }

        // If there's overflow, fold it back in
        if result[LIMBS] > 0 {
            let mut c: u64 = result[LIMBS] * PRIME_DIFF;
            for i in 0..LIMBS {
                c += limbs[i] as u64;
                limbs[i] = c as u32;
                c >>= 32;
            }
            // c should be 0 now since result[LIMBS] was small
        }

        let mut num = Num3072 { limbs };
        num.reduce_once();
        num
    }

    /// Computes the modular inverse using Fermat's little theorem: `a^(-1) = a^(p-2) mod p`.
    ///
    /// Since `p` is prime, `a^(p-1) = 1 mod p` for any `a != 0`, so `a^(p-2)` is the
    /// multiplicative inverse. Uses square-and-multiply exponentiation from the most
    /// significant bit down.
    ///
    /// The exponent is `p - 2 = 2^3072 - 1103719`.
    pub fn mod_inverse(&self) -> Num3072 {
        // p - 2 = 2^3072 - (PRIME_DIFF + 2)
        // In binary, this is 3072 ones, then subtract (PRIME_DIFF + 2).
        // PRIME_DIFF + 2 = 1103719
        //
        // (2^3072 - 1) in binary is 3072 one-bits.
        // Subtracting (PRIME_DIFF + 1) from that:
        //   p - 2 = (2^3072 - 1) - (PRIME_DIFF + 1)
        //
        // The lowest 21 bits of (2^3072 - 1) are all 1s.
        // We need: all_ones - 1103719
        // 1103719 in binary = 0b100001101011101100111 (21 bits)
        // ~1103719 (in 21 bits) = 0b011110010100010011000
        // But we want (2^21 - 1) - 1103719 = 2097151 - 1103719 = 993432
        // 993432 in binary = 0b11110010100010011000 (20 bits)
        // So the lowest 21 bits of (p-2) are: 0b011110010100010011000
        // And bits 21..3071 are all 1.
        //
        // Actually let me just compute this directly.

        // Build the exponent p-2 as an array of 96 u32 limbs
        // Start with all 0xFFFFFFFF (= 2^3072 - 1), then subtract (sub)
        let mut exp = [0xFFFF_FFFFu32; LIMBS];
        // Subtract sub from limb[0], borrowing as needed
        // Since sub < 2^32, and exp[0] = 0xFFFFFFFF >= sub:
        //   exp[0] = 0xFFFFFFFF - sub + 1... wait, we want (2^3072 - 1) - (sub) not 2^3072 - sub
        // (2^3072 - 1) has all limbs 0xFFFFFFFF.
        // Subtract sub from it:
        //   exp[0] = 0xFFFFFFFF - sub (no borrow since 0xFFFFFFFF > sub)
        //   Actually 0xFFFFFFFF = 4294967295, sub = 1103719
        //   exp[0] = 4294967295 - 1103719 = 4293863576 = 0xFFF10D48... let me check:
        //   Wait: p - 2 = 2^3072 - PRIME_DIFF - 2 = (2^3072 - 1) - (PRIME_DIFF + 1)
        //   So sub for the exponent subtraction from all-ones is PRIME_DIFF + 1 = 1103718
        //   No wait: p - 2. p = 2^3072 - PRIME_DIFF. p - 2 = 2^3072 - PRIME_DIFF - 2.
        //   And (2^3072 - 1) is all ones. So p - 2 = (2^3072 - 1) - (PRIME_DIFF + 1).
        //   PRIME_DIFF + 1 = 1103718.
        let sub_from_ones: u64 = PRIME_DIFF + 1; // 1103718
        exp[0] = (0xFFFF_FFFFu64 - sub_from_ones) as u32;
        // exp[1..] remain 0xFFFFFFFF (no borrow needed since exp[0] didn't underflow)

        // Now do square-and-multiply from MSB (bit 3071) down to bit 0
        let mut result = self.clone(); // Start with a^1 (bit 3071 is 1)

        // Process bits 3070 down to 0
        for bit in (0..3071).rev() {
            result = result.mul_mod(&result); // square

            // Check if bit `bit` is set in exp
            let limb_idx = bit / 32;
            let bit_idx = bit % 32;
            if (exp[limb_idx] >> bit_idx) & 1 == 1 {
                result = result.mul_mod(self); // multiply
            }
        }

        result
    }
}

/// MuHash3072 accumulator for rolling UTXO set hashing.
///
/// Maintains a running product of hashed elements modulo p.
/// Supports O(1) addition and removal of elements.
#[derive(Clone, Debug)]
pub struct MuHash {
    /// The running product (numerator).
    numerator: Num3072,
    /// The running divisor (elements removed).
    denominator: Num3072,
}

impl MuHash {
    /// Create a new empty MuHash (identity element).
    pub fn new() -> Self {
        MuHash {
            numerator: Num3072::one(),
            denominator: Num3072::one(),
        }
    }

    /// Hash an element (arbitrary bytes) to a 3072-bit number.
    fn hash_to_num(data: &[u8]) -> Num3072 {
        // SHA256 the data, then expand to 384 bytes using SHA256 in counter mode
        let seed = sha256_hash(data);
        let mut expanded = [0u8; 384];

        // Use SHA256 in counter mode to expand to 384 bytes (12 * 32 = 384)
        for i in 0..12 {
            let mut input = Vec::with_capacity(36);
            input.extend_from_slice(&seed);
            input.extend_from_slice(&(i as u32).to_le_bytes());
            let hash = sha256_hash(&input);
            expanded[i * 32..(i + 1) * 32].copy_from_slice(&hash);
        }

        let mut num = Num3072::from_bytes(&expanded);

        // Ensure non-zero (extremely unlikely but handle it)
        if num.is_zero() {
            num.limbs[0] = 1;
        }

        // Reduce modulo p to ensure we're in the valid range
        num.reduce_once();

        num
    }

    /// Adds an element to the set by multiplying its hash into the numerator.
    ///
    /// The element `data` is hashed to a 3072-bit field element and multiplied
    /// into the running product. This operation is O(1) regardless of set size.
    pub fn insert(&mut self, data: &[u8]) {
        let h = Self::hash_to_num(data);
        self.numerator = self.numerator.mul_mod(&h);
    }

    /// Removes an element from the set by multiplying its hash into the denominator.
    ///
    /// The element `data` is hashed to a 3072-bit field element and accumulated
    /// in the denominator. On finalization, the denominator is inverted and
    /// multiplied with the numerator to cancel this element.
    pub fn remove(&mut self, data: &[u8]) {
        let h = Self::hash_to_num(data);
        self.denominator = self.denominator.mul_mod(&h);
    }

    /// Combines two `MuHash` accumulators by multiplying their numerators and denominators.
    ///
    /// This allows parallel computation of partial set hashes that are then merged.
    pub fn combine(&mut self, other: &MuHash) {
        self.numerator = self.numerator.mul_mod(&other.numerator);
        self.denominator = self.denominator.mul_mod(&other.denominator);
    }

    /// Finalize and produce the 32-byte hash.
    ///
    /// Computes: SHA256(numerator / denominator mod p)
    pub fn finalize(&self) -> [u8; 32] {
        // Compute numerator * denominator^(-1) mod p
        let inv = self.denominator.mod_inverse();
        let result = self.numerator.mul_mod(&inv);

        // Hash the 384-byte result to 32 bytes
        let bytes = result.to_bytes();
        sha256_hash(&bytes)
    }

    /// Serializes UTXO data into a canonical byte format for hashing.
    ///
    /// Format: `txid || vout || height || coinbase || amount || script_pubkey`
    ///
    /// This canonical serialization ensures that the UTXO set hash is deterministic
    /// across all nodes.
    ///
    /// # Parameters
    /// - `txid`: The 32-byte transaction ID of the output.
    /// - `vout`: The output index within the transaction.
    /// - `height`: The block height at which this UTXO was created.
    /// - `coinbase`: Whether this output comes from a coinbase transaction.
    /// - `amount`: The output value in satoshis.
    /// - `script_pubkey`: The output's locking script.
    pub fn serialize_utxo(
        txid: &[u8; 32],
        vout: u32,
        height: u32,
        coinbase: bool,
        amount: i64,
        script_pubkey: &[u8],
    ) -> Vec<u8> {
        let mut data = Vec::with_capacity(32 + 4 + 4 + 1 + 8 + script_pubkey.len());
        data.extend_from_slice(txid);
        data.extend_from_slice(&vout.to_le_bytes());
        data.extend_from_slice(&height.to_le_bytes());
        data.push(coinbase as u8);
        data.extend_from_slice(&amount.to_le_bytes());
        data.extend_from_slice(script_pubkey);
        data
    }
}

impl Default for MuHash {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_num3072_one() {
        let one = Num3072::one();
        assert!(one.is_one());
        assert!(!one.is_zero());
    }

    #[test]
    fn test_num3072_mul_identity() {
        let one = Num3072::one();
        let mut num = Num3072::one();
        num.limbs[0] = 42;
        let result = num.mul_mod(&one);
        assert_eq!(result.limbs[0], 42);
        for i in 1..LIMBS {
            assert_eq!(result.limbs[i], 0);
        }
    }

    #[test]
    fn test_muhash_empty() {
        let h = MuHash::new();
        let hash1 = h.finalize();
        // Empty set should produce a consistent hash
        let h2 = MuHash::new();
        assert_eq!(hash1, h2.finalize());
    }

    #[test]
    fn test_muhash_single_element() {
        let mut h = MuHash::new();
        h.insert(b"hello");
        let hash = h.finalize();
        // Should be deterministic
        let mut h2 = MuHash::new();
        h2.insert(b"hello");
        assert_eq!(hash, h2.finalize());
    }

    #[test]
    fn test_muhash_commutative() {
        let mut h1 = MuHash::new();
        h1.insert(b"alpha");
        h1.insert(b"beta");

        let mut h2 = MuHash::new();
        h2.insert(b"beta");
        h2.insert(b"alpha");

        assert_eq!(h1.finalize(), h2.finalize());
    }

    #[test]
    fn test_muhash_insert_remove() {
        let mut h = MuHash::new();
        let empty_hash = h.finalize();

        h.insert(b"element");
        let with_element = h.finalize();
        assert_ne!(empty_hash, with_element);

        h.remove(b"element");
        let after_remove = h.finalize();
        assert_eq!(empty_hash, after_remove);
    }

    #[test]
    fn test_muhash_combine() {
        let mut h1 = MuHash::new();
        h1.insert(b"a");

        let mut h2 = MuHash::new();
        h2.insert(b"b");

        let mut combined = MuHash::new();
        combined.insert(b"a");
        combined.insert(b"b");

        let mut h1_copy = h1.clone();
        h1_copy.combine(&h2);

        assert_eq!(h1_copy.finalize(), combined.finalize());
    }

    #[test]
    fn test_muhash_different_elements() {
        let mut h1 = MuHash::new();
        h1.insert(b"foo");

        let mut h2 = MuHash::new();
        h2.insert(b"bar");

        assert_ne!(h1.finalize(), h2.finalize());
    }

    #[test]
    fn test_serialize_utxo() {
        let txid = [0u8; 32];
        let data = MuHash::serialize_utxo(&txid, 0, 100, true, 5000000000, &[0x51]);
        assert_eq!(data.len(), 32 + 4 + 4 + 1 + 8 + 1); // 50 bytes
    }

    #[test]
    fn test_muhash_utxo_workflow() {
        let mut muhash = MuHash::new();

        // Add a UTXO
        let txid = [1u8; 32];
        let utxo_data = MuHash::serialize_utxo(&txid, 0, 1, true, 5000000000, &[0x76, 0xa9]);
        muhash.insert(&utxo_data);

        let _hash_with = muhash.finalize();

        // Remove same UTXO
        muhash.remove(&utxo_data);
        let hash_without = muhash.finalize();

        // Should be back to empty set hash
        let empty = MuHash::new();
        assert_eq!(hash_without, empty.finalize());
    }

    #[test]
    fn test_num3072_from_to_bytes_roundtrip() {
        let mut bytes = [0u8; 384];
        bytes[0] = 42;
        bytes[100] = 0xFF;
        bytes[383] = 0x01;

        let num = Num3072::from_bytes(&bytes);
        let back = num.to_bytes();
        assert_eq!(bytes, back);
    }

    #[test]
    fn test_muhash_multiple_insert_remove() {
        // Insert three elements, remove two, check consistency
        let mut h1 = MuHash::new();
        h1.insert(b"x");
        h1.insert(b"y");
        h1.insert(b"z");
        h1.remove(b"y");
        h1.remove(b"z");

        let mut h2 = MuHash::new();
        h2.insert(b"x");

        assert_eq!(h1.finalize(), h2.finalize());
    }

    #[test]
    fn test_num3072_mul_commutative() {
        let mut a = Num3072::one();
        a.limbs[0] = 12345;
        a.limbs[1] = 67890;

        let mut b = Num3072::one();
        b.limbs[0] = 54321;
        b.limbs[2] = 11111;

        let ab = a.mul_mod(&b);
        let ba = b.mul_mod(&a);

        for i in 0..LIMBS {
            assert_eq!(ab.limbs[i], ba.limbs[i]);
        }
    }
}
