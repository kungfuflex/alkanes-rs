//! Variable-length integer encoding.
//!
//! Maps to: `serialize.h` (`WriteVarInt`/`ReadVarInt`) in Bitcoin Core.
//!
//! MSB base-128 encoding. High bit in each byte indicates continuation.
//! One is subtracted from all but the last digit for unique encoding.
//!
//! Properties:
//! - Very compact (0-127: 1 byte, 128-16511: 2 bytes)
//! - Every integer has exactly one encoding
//! - Encoding is independent of original integer size

use crate::encode::Error;
use std::io::{Read, Write};

/// Returns the number of bytes needed to encode `n` as a VarInt.
///
/// Results: 1 byte for 0..=127, 2 bytes for 128..=16511, and so on.
pub fn varint_len(mut n: u64) -> usize {
    let mut len = 0;
    loop {
        len += 1;
        if n <= 0x7f {
            break;
        }
        n = (n >> 7) - 1;
    }
    len
}

/// Writes an unsigned integer in VarInt (MSB base-128) encoding to `w`.
///
/// Returns the number of bytes written.
/// Port of Bitcoin Core's `WriteVarInt`.
pub fn write_varint<W: Write>(w: &mut W, mut n: u64) -> Result<usize, Error> {
    let mut tmp = [0u8; 10]; // max 10 bytes for 64-bit
    let mut len = 0;
    loop {
        tmp[len] = (n & 0x7f) as u8 | if len > 0 { 0x80 } else { 0x00 };
        if n <= 0x7f {
            break;
        }
        n = (n >> 7) - 1;
        len += 1;
    }
    // Write in reverse order
    let total = len + 1;
    for i in (0..total).rev() {
        w.write_all(&[tmp[i]])?;
    }
    Ok(total)
}

/// Reads a VarInt-encoded unsigned integer from `r`.
///
/// Returns [`Error::VarIntTooLarge`] if the
/// decoded value would overflow `u64`.
/// Port of Bitcoin Core's `ReadVarInt`.
pub fn read_varint<R: Read>(r: &mut R) -> Result<u64, Error> {
    let mut n: u64 = 0;
    loop {
        let mut buf = [0u8; 1];
        r.read_exact(&mut buf)?;
        let ch = buf[0];

        if n > (u64::MAX >> 7) {
            return Err(Error::VarIntTooLarge);
        }
        n = (n << 7) | (ch & 0x7f) as u64;
        if ch & 0x80 != 0 {
            if n == u64::MAX {
                return Err(Error::VarIntTooLarge);
            }
            n += 1;
        } else {
            return Ok(n);
        }
    }
}

/// Writes a non-negative `i64` value in VarInt encoding to `w`.
///
/// The value must be >= 0; negative values will produce incorrect results
/// because the sign bit is reinterpreted as magnitude.
pub fn write_varint_signed<W: Write>(w: &mut W, n: i64) -> Result<usize, Error> {
    write_varint(w, n as u64)
}

/// Reads a VarInt-encoded value and returns it as a non-negative `i64`.
pub fn read_varint_signed<R: Read>(r: &mut R) -> Result<i64, Error> {
    let n = read_varint(r)?;
    Ok(n as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn roundtrip(val: u64) {
        let mut buf = Vec::new();
        write_varint(&mut buf, val).unwrap();
        let mut cursor = Cursor::new(&buf);
        let decoded = read_varint(&mut cursor).unwrap();
        assert_eq!(val, decoded, "VarInt roundtrip failed for {}", val);
    }

    #[test]
    fn test_varint_zero() {
        let mut buf = Vec::new();
        write_varint(&mut buf, 0).unwrap();
        assert_eq!(buf, vec![0x00]);
        roundtrip(0);
    }

    #[test]
    fn test_varint_127() {
        let mut buf = Vec::new();
        write_varint(&mut buf, 127).unwrap();
        assert_eq!(buf, vec![0x7f]);
        roundtrip(127);
    }

    #[test]
    fn test_varint_128() {
        let mut buf = Vec::new();
        write_varint(&mut buf, 128).unwrap();
        assert_eq!(buf, vec![0x80, 0x00]);
        roundtrip(128);
    }

    #[test]
    fn test_varint_255() {
        let mut buf = Vec::new();
        write_varint(&mut buf, 255).unwrap();
        assert_eq!(buf, vec![0x80, 0x7f]);
        roundtrip(255);
    }

    #[test]
    fn test_varint_256() {
        let mut buf = Vec::new();
        write_varint(&mut buf, 256).unwrap();
        assert_eq!(buf, vec![0x81, 0x00]);
        roundtrip(256);
    }

    #[test]
    fn test_varint_16383() {
        roundtrip(16383);
    }

    #[test]
    fn test_varint_16384() {
        roundtrip(16384);
    }

    #[test]
    fn test_varint_large() {
        roundtrip(0xffffffff);
        roundtrip(0x100000000);
    }

    #[test]
    fn test_varint_len() {
        assert_eq!(varint_len(0), 1);
        assert_eq!(varint_len(127), 1);
        assert_eq!(varint_len(128), 2);
        assert_eq!(varint_len(16511), 2);
        assert_eq!(varint_len(16512), 3);
    }
}
