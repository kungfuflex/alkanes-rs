//! CompactSize encoding/decoding.
//!
//! Maps to: `serialize.h` (`WriteCompactSize`/`ReadCompactSize`) in Bitcoin Core.
//!
//! Compact Size encoding:
//!   size <  253        -> 1 byte
//!   size <= 0xFFFF     -> 3 bytes (253 + 2 bytes LE)
//!   size <= 0xFFFFFFFF -> 5 bytes (254 + 4 bytes LE)
//!   size >  0xFFFFFFFF -> 9 bytes (255 + 8 bytes LE)

use crate::encode::Error;
use std::io::{Read, Write};

/// Returns the number of bytes needed to encode `size` as a CompactSize value.
///
/// Results: 1 byte for values < 253, 3 bytes for <= 0xFFFF, 5 bytes for <= 0xFFFFFFFF, 9 otherwise.
pub fn compact_size_len(size: u64) -> usize {
    if size < 253 {
        1
    } else if size <= 0xffff {
        3
    } else if size <= 0xffffffff {
        5
    } else {
        9
    }
}

/// Writes a CompactSize-encoded unsigned integer to `w`.
///
/// Returns the number of bytes written (1, 3, 5, or 9).
/// Port of Bitcoin Core's `WriteCompactSize`.
pub fn write_compact_size<W: Write>(w: &mut W, size: u64) -> Result<usize, Error> {
    if size < 253 {
        w.write_all(&[size as u8])?;
        Ok(1)
    } else if size <= 0xffff {
        w.write_all(&[253])?;
        w.write_all(&(size as u16).to_le_bytes())?;
        Ok(3)
    } else if size <= 0xffffffff {
        w.write_all(&[254])?;
        w.write_all(&(size as u32).to_le_bytes())?;
        Ok(5)
    } else {
        w.write_all(&[255])?;
        w.write_all(&size.to_le_bytes())?;
        Ok(9)
    }
}

/// Reads a CompactSize-encoded unsigned integer from `r`.
///
/// Validates canonical encoding (smallest possible representation) and
/// rejects values exceeding [`MAX_SIZE`](crate::encode::MAX_SIZE).
/// Port of Bitcoin Core's `ReadCompactSize`.
pub fn read_compact_size<R: Read>(r: &mut R) -> Result<u64, Error> {
    read_compact_size_with_range(r, true)
}

/// Reads a CompactSize-encoded unsigned integer with an optional range check.
///
/// When `range_check` is `true`, values exceeding [`MAX_SIZE`](crate::encode::MAX_SIZE)
/// are rejected. Non-canonical encodings are always rejected regardless of this flag.
pub fn read_compact_size_with_range<R: Read>(r: &mut R, range_check: bool) -> Result<u64, Error> {
    let mut ch_size = [0u8; 1];
    r.read_exact(&mut ch_size)?;
    let ch_size = ch_size[0];

    let size = if ch_size < 253 {
        ch_size as u64
    } else if ch_size == 253 {
        let mut buf = [0u8; 2];
        r.read_exact(&mut buf)?;
        let val = u16::from_le_bytes(buf) as u64;
        if val < 253 {
            return Err(Error::NonCanonicalCompactSize);
        }
        val
    } else if ch_size == 254 {
        let mut buf = [0u8; 4];
        r.read_exact(&mut buf)?;
        let val = u32::from_le_bytes(buf) as u64;
        if val < 0x10000 {
            return Err(Error::NonCanonicalCompactSize);
        }
        val
    } else {
        let mut buf = [0u8; 8];
        r.read_exact(&mut buf)?;
        let val = u64::from_le_bytes(buf);
        if val < 0x100000000 {
            return Err(Error::NonCanonicalCompactSize);
        }
        val
    };

    if range_check && size > crate::encode::MAX_SIZE {
        return Err(Error::CompactSizeTooLarge(size));
    }

    Ok(size)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_compact_size_single_byte() {
        let mut buf = Vec::new();
        let written = write_compact_size(&mut buf, 0).unwrap();
        assert_eq!(written, 1);
        assert_eq!(buf, vec![0]);

        let mut cursor = Cursor::new(&buf);
        assert_eq!(read_compact_size(&mut cursor).unwrap(), 0);
    }

    #[test]
    fn test_compact_size_252() {
        let mut buf = Vec::new();
        write_compact_size(&mut buf, 252).unwrap();
        assert_eq!(buf, vec![252]);

        let mut cursor = Cursor::new(&buf);
        assert_eq!(read_compact_size(&mut cursor).unwrap(), 252);
    }

    #[test]
    fn test_compact_size_253() {
        let mut buf = Vec::new();
        write_compact_size(&mut buf, 253).unwrap();
        assert_eq!(buf.len(), 3);
        assert_eq!(buf[0], 253);

        let mut cursor = Cursor::new(&buf);
        assert_eq!(read_compact_size(&mut cursor).unwrap(), 253);
    }

    #[test]
    fn test_compact_size_65535() {
        let mut buf = Vec::new();
        write_compact_size(&mut buf, 65535).unwrap();
        assert_eq!(buf.len(), 3);

        let mut cursor = Cursor::new(&buf);
        assert_eq!(read_compact_size(&mut cursor).unwrap(), 65535);
    }

    #[test]
    fn test_compact_size_65536() {
        let mut buf = Vec::new();
        write_compact_size(&mut buf, 65536).unwrap();
        assert_eq!(buf.len(), 5);
        assert_eq!(buf[0], 254);

        let mut cursor = Cursor::new(&buf);
        assert_eq!(read_compact_size(&mut cursor).unwrap(), 65536);
    }

    #[test]
    fn test_compact_size_large() {
        let val = 0x100000000u64;
        let mut buf = Vec::new();
        write_compact_size(&mut buf, val).unwrap();
        assert_eq!(buf.len(), 9);
        assert_eq!(buf[0], 255);

        let mut cursor = Cursor::new(&buf);
        assert_eq!(
            read_compact_size_with_range(&mut cursor, false).unwrap(),
            val
        );
    }

    #[test]
    fn test_non_canonical_rejected() {
        // 253 prefix but value < 253
        let buf = vec![253, 100, 0];
        let mut cursor = Cursor::new(&buf);
        assert!(read_compact_size(&mut cursor).is_err());
    }

    #[test]
    fn test_compact_size_len() {
        assert_eq!(compact_size_len(0), 1);
        assert_eq!(compact_size_len(252), 1);
        assert_eq!(compact_size_len(253), 3);
        assert_eq!(compact_size_len(0xffff), 3);
        assert_eq!(compact_size_len(0x10000), 5);
        assert_eq!(compact_size_len(0xffffffff), 5);
        assert_eq!(compact_size_len(0x100000000), 9);
    }
}
