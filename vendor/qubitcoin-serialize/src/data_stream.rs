//! In-memory data stream for serialization/deserialization.
//!
//! Maps to: `src/streams.h` (`DataStream`) in Bitcoin Core.
//!
//! [`DataStream`] wraps a `Vec<u8>` with a read cursor, supporting both
//! sequential reading (consuming) and appending (writing) operations.

use crate::encode::{Decodable, Encodable, Error};
use std::io::{self, Read, Write};

/// In-memory data stream with read cursor.
///
/// Port of Bitcoin Core's `DataStream` (formerly `CDataStream`).
/// Supports sequential reading via cursor and appending via write.
#[derive(Debug, Clone)]
pub struct DataStream {
    data: Vec<u8>,
    read_pos: usize,
}

impl DataStream {
    /// Create an empty DataStream.
    pub fn new() -> Self {
        DataStream {
            data: Vec::new(),
            read_pos: 0,
        }
    }

    /// Create a DataStream from existing bytes.
    pub fn from_bytes(data: Vec<u8>) -> Self {
        DataStream { data, read_pos: 0 }
    }

    /// Create a DataStream from a byte slice.
    pub fn from_slice(data: &[u8]) -> Self {
        DataStream {
            data: data.to_vec(),
            read_pos: 0,
        }
    }

    /// Get remaining unread bytes.
    pub fn remaining(&self) -> &[u8] {
        &self.data[self.read_pos..]
    }

    /// Get number of unread bytes.
    pub fn remaining_len(&self) -> usize {
        self.data.len() - self.read_pos
    }

    /// Check if all data has been read.
    pub fn is_empty(&self) -> bool {
        self.read_pos >= self.data.len()
    }

    /// Get total size of the buffer.
    pub fn size(&self) -> usize {
        self.data.len()
    }

    /// Get the current read position.
    pub fn pos(&self) -> usize {
        self.read_pos
    }

    /// Reset read position to the beginning.
    pub fn rewind(&mut self) {
        self.read_pos = 0;
    }

    /// Clear all data and reset position.
    pub fn clear(&mut self) {
        self.data.clear();
        self.read_pos = 0;
    }

    /// Get all data (including already-read bytes).
    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }

    /// Take ownership of all data.
    pub fn into_bytes(self) -> Vec<u8> {
        self.data
    }

    /// Serializes an [`Encodable`] value by appending its bytes to this stream.
    ///
    /// Returns the number of bytes written.
    pub fn write_obj<T: Encodable>(&mut self, obj: &T) -> Result<usize, Error> {
        obj.encode(&mut self.data)
    }

    /// Deserializes a [`Decodable`] value from this stream, advancing the read cursor.
    pub fn read_obj<T: Decodable>(&mut self) -> Result<T, Error> {
        let mut cursor = io::Cursor::new(&self.data[self.read_pos..]);
        let obj = T::decode(&mut cursor)?;
        self.read_pos += cursor.position() as usize;
        Ok(obj)
    }

    /// Advances the read cursor by `n` bytes without reading them.
    ///
    /// Returns [`Error::EndOfData`] if fewer than `n` bytes remain.
    pub fn skip(&mut self, n: usize) -> Result<(), Error> {
        if self.read_pos + n > self.data.len() {
            return Err(Error::EndOfData);
        }
        self.read_pos += n;
        Ok(())
    }
}

impl Default for DataStream {
    fn default() -> Self {
        Self::new()
    }
}

impl Read for DataStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let remaining = &self.data[self.read_pos..];
        let to_read = std::cmp::min(buf.len(), remaining.len());
        buf[..to_read].copy_from_slice(&remaining[..to_read]);
        self.read_pos += to_read;
        Ok(to_read)
    }
}

impl Write for DataStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.data.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_write_read_u32() {
        let mut ds = DataStream::new();
        ds.write_obj(&42u32).unwrap();
        assert_eq!(ds.size(), 4);

        let val: u32 = ds.read_obj().unwrap();
        assert_eq!(val, 42);
        assert!(ds.is_empty());
    }

    #[test]
    fn test_multiple_writes() {
        let mut ds = DataStream::new();
        ds.write_obj(&1u8).unwrap();
        ds.write_obj(&2u16).unwrap();
        ds.write_obj(&3u32).unwrap();

        let a: u8 = ds.read_obj().unwrap();
        let b: u16 = ds.read_obj().unwrap();
        let c: u32 = ds.read_obj().unwrap();
        assert_eq!(a, 1);
        assert_eq!(b, 2);
        assert_eq!(c, 3);
        assert!(ds.is_empty());
    }

    #[test]
    fn test_from_bytes() {
        let ds = DataStream::from_bytes(vec![0x78, 0x56, 0x34, 0x12]);
        assert_eq!(ds.remaining_len(), 4);
    }

    #[test]
    fn test_rewind() {
        let mut ds = DataStream::from_bytes(vec![42]);
        let _: u8 = ds.read_obj().unwrap();
        assert!(ds.is_empty());
        ds.rewind();
        assert!(!ds.is_empty());
        let val: u8 = ds.read_obj().unwrap();
        assert_eq!(val, 42);
    }

    #[test]
    fn test_skip() {
        let mut ds = DataStream::from_bytes(vec![1, 2, 3, 4]);
        ds.skip(2).unwrap();
        let val: u8 = ds.read_obj().unwrap();
        assert_eq!(val, 3);
    }
}
