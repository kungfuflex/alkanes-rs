//! 256-bit unsigned integer with arithmetic operations.
//! Maps to: src/arith_uint256.h/cpp (base_uint<256>, arith_uint256)
//!
//! Used for difficulty target calculations. Unlike Uint256 (opaque blob),
//! ArithUint256 supports full arithmetic: add, sub, mul, div, shift, compare.
//!
//! Internal representation: 8 x u32 limbs, least-significant first.

use crate::uint256::Uint256;
use std::cmp::Ordering;
use std::fmt;
use std::ops::{
    Add, AddAssign, BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Div, DivAssign,
    Mul, MulAssign, Neg, Not, Shl, ShlAssign, Shr, ShrAssign, Sub, SubAssign,
};

/// Number of 32-bit limbs in a 256-bit integer (256 / 32 = 8).
const WIDTH: usize = 8;

/// Error type for `ArithUint256` arithmetic operations.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ArithError {
    /// Attempted division by zero.
    #[error("Division by zero")]
    DivisionByZero,
}

/// A 256-bit unsigned integer with full arithmetic operations for difficulty calculations.
///
/// Represented as 8 x `u32` limbs in little-endian order (least significant first).
/// Supports addition, subtraction, multiplication, division, bitwise operations,
/// and shifts. Also provides compact encoding/decoding for the `nBits` field in
/// block headers.
///
/// Equivalent to `base_uint<256>` / `arith_uint256` in Bitcoin Core.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ArithUint256 {
    /// The 8 limbs of the 256-bit number, with `pn[0]` being the least significant.
    pn: [u32; WIDTH],
}

impl Default for ArithUint256 {
    fn default() -> Self {
        ArithUint256 { pn: [0u32; WIDTH] }
    }
}

impl ArithUint256 {
    /// Creates a new `ArithUint256` with value zero.
    pub const fn zero() -> Self {
        ArithUint256 { pn: [0u32; WIDTH] }
    }

    /// Creates a new `ArithUint256` from a `u64` value, zero-extending to 256 bits.
    pub const fn from_u64(b: u64) -> Self {
        let mut pn = [0u32; WIDTH];
        pn[0] = b as u32;
        pn[1] = (b >> 32) as u32;
        ArithUint256 { pn }
    }

    /// Returns the low 64 bits of this 256-bit integer.
    pub fn low64(&self) -> u64 {
        self.pn[0] as u64 | ((self.pn[1] as u64) << 32)
    }

    /// Returns an `f64` approximation of this value (for display or logging, not consensus).
    pub fn to_f64(&self) -> f64 {
        let mut ret = 0.0f64;
        let mut fact = 1.0f64;
        for i in 0..WIDTH {
            ret += fact * self.pn[i] as f64;
            fact *= 4294967296.0; // 2^32
        }
        ret
    }

    /// Returns the position of the highest set bit plus one, or zero if the value is zero.
    ///
    /// For example, `bits()` returns 1 for value 1, 8 for value 255, and 9 for value 256.
    /// Equivalent to `base_uint::bits()` in Bitcoin Core.
    pub fn bits(&self) -> u32 {
        for pos in (0..WIDTH).rev() {
            if self.pn[pos] != 0 {
                for nbits in (1..=31).rev() {
                    if self.pn[pos] & (1u32 << nbits) != 0 {
                        return 32 * pos as u32 + nbits + 1;
                    }
                }
                return 32 * pos as u32 + 1;
            }
        }
        0
    }

    /// Performs numeric comparison, checking limbs from most significant to least significant.
    pub fn compare_to(&self, other: &ArithUint256) -> Ordering {
        for i in (0..WIDTH).rev() {
            if self.pn[i] < other.pn[i] {
                return Ordering::Less;
            }
            if self.pn[i] > other.pn[i] {
                return Ordering::Greater;
            }
        }
        Ordering::Equal
    }

    /// Returns `true` if this 256-bit value equals the given `u64` (upper limbs must be zero).
    pub fn equal_to_u64(&self, b: u64) -> bool {
        for i in (2..WIDTH).rev() {
            if self.pn[i] != 0 {
                return false;
            }
        }
        self.pn[1] == (b >> 32) as u32 && self.pn[0] == (b & 0xffffffff) as u32
    }

    /// Returns the hexadecimal string representation (most significant digits first).
    pub fn to_hex(&self) -> String {
        let u256 = arith_to_uint256(self);
        u256.to_hex()
    }

    /// Increments this value by one in-place, with carry propagation.
    pub fn inc(&mut self) {
        let mut i = 0;
        while i < WIDTH {
            self.pn[i] = self.pn[i].wrapping_add(1);
            if self.pn[i] != 0 {
                break;
            }
            i += 1;
        }
    }

    /// Decrements this value by one in-place, with borrow propagation.
    pub fn dec(&mut self) {
        let mut i = 0;
        while i < WIDTH {
            let old = self.pn[i];
            self.pn[i] = self.pn[i].wrapping_sub(1);
            if old != 0 {
                break;
            }
            i += 1;
        }
    }

    /// Decodes a compact difficulty representation (`nBits` field in block headers) into
    /// this 256-bit integer.
    ///
    /// The compact format encodes a 256-bit number as a 32-bit value:
    /// - Top 8 bits: exponent (number of bytes in the mantissa)
    /// - Bit 23: sign bit (negative targets are invalid but representable)
    /// - Lower 23 bits: mantissa
    ///
    /// The decoded value is: `N = (-1^sign) * mantissa * 256^(exponent-3)`
    ///
    /// Returns `(negative, overflow)` where `negative` indicates the sign bit was set
    /// and `overflow` indicates the compact value overflows 256 bits.
    ///
    /// Equivalent to `arith_uint256::SetCompact()` in Bitcoin Core.
    pub fn set_compact(&mut self, compact: u32) -> (bool, bool) {
        let n_size = (compact >> 24) as i32;
        let mut n_word = compact & 0x007fffff;
        if n_size <= 3 {
            n_word >>= 8 * (3 - n_size) as u32;
            *self = ArithUint256::from_u64(n_word as u64);
        } else {
            *self = ArithUint256::from_u64(n_word as u64);
            *self <<= (8 * (n_size - 3)) as u32;
        }

        let negative = n_word != 0 && (compact & 0x00800000) != 0;
        let overflow = n_word != 0
            && ((n_size > 34)
                || (n_word > 0xff && n_size > 33)
                || (n_word > 0xffff && n_size > 32));

        (negative, overflow)
    }

    /// Encodes this 256-bit integer into compact difficulty representation (`nBits`).
    ///
    /// If `negative` is true, the sign bit is set in the result. This is the inverse
    /// of [`set_compact`](Self::set_compact).
    ///
    /// Equivalent to `arith_uint256::GetCompact()` in Bitcoin Core.
    pub fn get_compact(&self, negative: bool) -> u32 {
        let mut n_size = (self.bits() as i32 + 7) / 8;
        let mut n_compact: u32;

        if n_size <= 3 {
            n_compact = (self.low64() << (8 * (3 - n_size)) as u64) as u32;
        } else {
            let bn = *self >> (8 * (n_size - 3)) as u32;
            n_compact = bn.low64() as u32;
        }

        // The 0x00800000 bit denotes the sign.
        if n_compact & 0x00800000 != 0 {
            n_compact >>= 8;
            n_size += 1;
        }

        debug_assert!(n_compact & !0x007fffff == 0);
        debug_assert!(n_size < 256);

        n_compact |= (n_size as u32) << 24;
        if negative && (n_compact & 0x007fffff) != 0 {
            n_compact |= 0x00800000;
        }

        n_compact
    }
}

// --- Operator implementations ---

impl Ord for ArithUint256 {
    fn cmp(&self, other: &Self) -> Ordering {
        self.compare_to(other)
    }
}

impl PartialOrd for ArithUint256 {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Not for ArithUint256 {
    type Output = ArithUint256;
    fn not(self) -> ArithUint256 {
        let mut ret = ArithUint256::default();
        for i in 0..WIDTH {
            ret.pn[i] = !self.pn[i];
        }
        ret
    }
}

impl Neg for ArithUint256 {
    type Output = ArithUint256;
    fn neg(self) -> ArithUint256 {
        let mut ret = !self;
        ret.inc();
        ret
    }
}

impl AddAssign for ArithUint256 {
    fn add_assign(&mut self, rhs: ArithUint256) {
        let mut carry: u64 = 0;
        for i in 0..WIDTH {
            let n = carry + self.pn[i] as u64 + rhs.pn[i] as u64;
            self.pn[i] = (n & 0xffffffff) as u32;
            carry = n >> 32;
        }
    }
}

impl SubAssign for ArithUint256 {
    fn sub_assign(&mut self, rhs: ArithUint256) {
        *self += -rhs;
    }
}

impl AddAssign<u64> for ArithUint256 {
    fn add_assign(&mut self, rhs: u64) {
        *self += ArithUint256::from_u64(rhs);
    }
}

impl SubAssign<u64> for ArithUint256 {
    fn sub_assign(&mut self, rhs: u64) {
        *self += -ArithUint256::from_u64(rhs);
    }
}

impl MulAssign<u32> for ArithUint256 {
    fn mul_assign(&mut self, b32: u32) {
        let mut carry: u64 = 0;
        for i in 0..WIDTH {
            let n = carry + (b32 as u64) * (self.pn[i] as u64);
            self.pn[i] = (n & 0xffffffff) as u32;
            carry = n >> 32;
        }
    }
}

impl MulAssign for ArithUint256 {
    fn mul_assign(&mut self, rhs: ArithUint256) {
        let orig = *self;
        let mut a = ArithUint256::default();
        for j in 0..WIDTH {
            let mut carry: u64 = 0;
            for i in 0..(WIDTH - j) {
                let n = carry + a.pn[i + j] as u64 + (orig.pn[j] as u64) * (rhs.pn[i] as u64);
                a.pn[i + j] = (n & 0xffffffff) as u32;
                carry = n >> 32;
            }
        }
        *self = a;
    }
}

impl DivAssign for ArithUint256 {
    fn div_assign(&mut self, rhs: ArithUint256) {
        let mut div = rhs;
        let mut num = *self;
        *self = ArithUint256::default();

        let num_bits = num.bits() as i32;
        let div_bits = div.bits() as i32;

        if div_bits == 0 {
            panic!("Division by zero");
        }
        if div_bits > num_bits {
            return;
        }

        let mut shift = num_bits - div_bits;
        div <<= shift as u32;
        while shift >= 0 {
            if num >= div {
                num -= div;
                self.pn[(shift / 32) as usize] |= 1u32 << (shift & 31);
            }
            div >>= 1u32;
            shift -= 1;
        }
    }
}

impl BitXorAssign for ArithUint256 {
    fn bitxor_assign(&mut self, rhs: ArithUint256) {
        for i in 0..WIDTH {
            self.pn[i] ^= rhs.pn[i];
        }
    }
}

impl BitAndAssign for ArithUint256 {
    fn bitand_assign(&mut self, rhs: ArithUint256) {
        for i in 0..WIDTH {
            self.pn[i] &= rhs.pn[i];
        }
    }
}

impl BitOrAssign for ArithUint256 {
    fn bitor_assign(&mut self, rhs: ArithUint256) {
        for i in 0..WIDTH {
            self.pn[i] |= rhs.pn[i];
        }
    }
}

impl ShlAssign<u32> for ArithUint256 {
    fn shl_assign(&mut self, shift: u32) {
        let a = *self;
        for i in 0..WIDTH {
            self.pn[i] = 0;
        }
        let k = (shift / 32) as usize;
        let shift = shift % 32;
        for i in 0..WIDTH {
            if i + k + 1 < WIDTH && shift != 0 {
                self.pn[i + k + 1] |= a.pn[i] >> (32 - shift);
            }
            if i + k < WIDTH {
                self.pn[i + k] |= a.pn[i] << shift;
            }
        }
    }
}

impl ShrAssign<u32> for ArithUint256 {
    fn shr_assign(&mut self, shift: u32) {
        let a = *self;
        for i in 0..WIDTH {
            self.pn[i] = 0;
        }
        let k = (shift / 32) as usize;
        let shift = shift % 32;
        for i in 0..WIDTH {
            if i >= k + 1 && shift != 0 {
                self.pn[i - k - 1] |= a.pn[i] << (32 - shift);
            }
            if i >= k {
                self.pn[i - k] |= a.pn[i] >> shift;
            }
        }
    }
}

// Binary operators (derived from assign operators)
impl Add for ArithUint256 {
    type Output = ArithUint256;
    fn add(mut self, rhs: ArithUint256) -> ArithUint256 {
        self += rhs;
        self
    }
}

impl Sub for ArithUint256 {
    type Output = ArithUint256;
    fn sub(mut self, rhs: ArithUint256) -> ArithUint256 {
        self -= rhs;
        self
    }
}

impl Mul for ArithUint256 {
    type Output = ArithUint256;
    fn mul(mut self, rhs: ArithUint256) -> ArithUint256 {
        self *= rhs;
        self
    }
}

impl Mul<u32> for ArithUint256 {
    type Output = ArithUint256;
    fn mul(mut self, rhs: u32) -> ArithUint256 {
        self *= rhs;
        self
    }
}

impl Div for ArithUint256 {
    type Output = ArithUint256;
    fn div(mut self, rhs: ArithUint256) -> ArithUint256 {
        self /= rhs;
        self
    }
}

impl BitXor for ArithUint256 {
    type Output = ArithUint256;
    fn bitxor(mut self, rhs: ArithUint256) -> ArithUint256 {
        self ^= rhs;
        self
    }
}

impl BitAnd for ArithUint256 {
    type Output = ArithUint256;
    fn bitand(mut self, rhs: ArithUint256) -> ArithUint256 {
        self &= rhs;
        self
    }
}

impl BitOr for ArithUint256 {
    type Output = ArithUint256;
    fn bitor(mut self, rhs: ArithUint256) -> ArithUint256 {
        self |= rhs;
        self
    }
}

impl Shl<u32> for ArithUint256 {
    type Output = ArithUint256;
    fn shl(mut self, shift: u32) -> ArithUint256 {
        self <<= shift;
        self
    }
}

impl Shr<u32> for ArithUint256 {
    type Output = ArithUint256;
    fn shr(mut self, shift: u32) -> ArithUint256 {
        self >>= shift;
        self
    }
}

impl From<u64> for ArithUint256 {
    fn from(val: u64) -> Self {
        ArithUint256::from_u64(val)
    }
}

impl fmt::Debug for ArithUint256 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ArithUint256({})", self.to_hex())
    }
}

impl fmt::Display for ArithUint256 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

// --- Conversion functions ---

/// Converts an [`ArithUint256`] to a [`Uint256`] by writing limbs as little-endian bytes.
///
/// This is used when an arithmetic result (e.g., a difficulty target) needs to be
/// stored or compared as an opaque hash blob. Equivalent to `ArithToUint256()` in Bitcoin Core.
pub fn arith_to_uint256(a: &ArithUint256) -> Uint256 {
    let mut data = [0u8; 32];
    for x in 0..WIDTH {
        let bytes = a.pn[x].to_le_bytes();
        data[x * 4..x * 4 + 4].copy_from_slice(&bytes);
    }
    Uint256::from_bytes(data)
}

/// Converts a [`Uint256`] to an [`ArithUint256`] by reading limbs as little-endian bytes.
///
/// This is used when a hash blob (e.g., a block hash) needs to be compared against
/// a difficulty target. Equivalent to `UintToArith256()` in Bitcoin Core.
pub fn uint256_to_arith(a: &Uint256) -> ArithUint256 {
    let data = a.data();
    let mut result = ArithUint256::default();
    for x in 0..WIDTH {
        result.pn[x] = u32::from_le_bytes([
            data[x * 4],
            data[x * 4 + 1],
            data[x * 4 + 2],
            data[x * 4 + 3],
        ]);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zero() {
        let z = ArithUint256::zero();
        assert_eq!(z.bits(), 0);
        assert!(z.equal_to_u64(0));
    }

    #[test]
    fn test_from_u64() {
        let a = ArithUint256::from_u64(0x1234567890abcdef);
        assert_eq!(a.low64(), 0x1234567890abcdef);
        assert!(a.equal_to_u64(0x1234567890abcdef));
    }

    #[test]
    fn test_add() {
        let a = ArithUint256::from_u64(100);
        let b = ArithUint256::from_u64(200);
        let c = a + b;
        assert!(c.equal_to_u64(300));
    }

    #[test]
    fn test_sub() {
        let a = ArithUint256::from_u64(300);
        let b = ArithUint256::from_u64(100);
        let c = a - b;
        assert!(c.equal_to_u64(200));
    }

    #[test]
    fn test_mul() {
        let a = ArithUint256::from_u64(100);
        let b = ArithUint256::from_u64(200);
        let c = a * b;
        assert!(c.equal_to_u64(20000));
    }

    #[test]
    fn test_mul_u32() {
        let a = ArithUint256::from_u64(100);
        let c = a * 200u32;
        assert!(c.equal_to_u64(20000));
    }

    #[test]
    fn test_div() {
        let a = ArithUint256::from_u64(20000);
        let b = ArithUint256::from_u64(100);
        let c = a / b;
        assert!(c.equal_to_u64(200));
    }

    #[test]
    #[should_panic(expected = "Division by zero")]
    fn test_div_by_zero() {
        let a = ArithUint256::from_u64(1);
        let b = ArithUint256::zero();
        let _ = a / b;
    }

    #[test]
    fn test_shift_left() {
        let a = ArithUint256::from_u64(1);
        let b = a << 64;
        assert_eq!(b.pn[2], 1);
        for i in 0..WIDTH {
            if i != 2 {
                assert_eq!(b.pn[i], 0);
            }
        }
    }

    #[test]
    fn test_shift_right() {
        let mut a = ArithUint256::default();
        a.pn[2] = 1; // 2^64
        let b = a >> 64;
        assert!(b.equal_to_u64(1));
    }

    #[test]
    fn test_bits() {
        assert_eq!(ArithUint256::from_u64(0).bits(), 0);
        assert_eq!(ArithUint256::from_u64(1).bits(), 1);
        assert_eq!(ArithUint256::from_u64(2).bits(), 2);
        assert_eq!(ArithUint256::from_u64(255).bits(), 8);
        assert_eq!(ArithUint256::from_u64(256).bits(), 9);
    }

    #[test]
    fn test_comparison() {
        let a = ArithUint256::from_u64(100);
        let b = ArithUint256::from_u64(200);
        assert!(a < b);
        assert!(b > a);
        assert_eq!(a, ArithUint256::from_u64(100));
    }

    #[test]
    fn test_compact_roundtrip() {
        // Genesis block nBits
        let mut target = ArithUint256::default();
        let (neg, overflow) = target.set_compact(0x1d00ffff);
        assert!(!neg);
        assert!(!overflow);

        let compact = target.get_compact(false);
        assert_eq!(compact, 0x1d00ffff);
    }

    #[test]
    fn test_arith_uint256_roundtrip() {
        let a = ArithUint256::from_u64(0xdeadbeef12345678);
        let u = arith_to_uint256(&a);
        let b = uint256_to_arith(&u);
        assert_eq!(a, b);
    }

    #[test]
    fn test_neg() {
        let a = ArithUint256::from_u64(1);
        let b = -a;
        // -1 in 256-bit two's complement = all ones
        for i in 0..WIDTH {
            assert_eq!(b.pn[i], 0xffffffff);
        }
    }

    #[test]
    fn test_not() {
        let a = ArithUint256::zero();
        let b = !a;
        for i in 0..WIDTH {
            assert_eq!(b.pn[i], 0xffffffff);
        }
    }

    #[test]
    fn test_inc_dec() {
        let mut a = ArithUint256::from_u64(42);
        a.inc();
        assert!(a.equal_to_u64(43));
        a.dec();
        assert!(a.equal_to_u64(42));
    }

    #[test]
    fn test_to_f64() {
        let a = ArithUint256::from_u64(1_000_000);
        assert!((a.to_f64() - 1_000_000.0).abs() < 0.001);
    }
}
