//! # base64 reader module
extern crate alloc;
use alloc::vec;

use base64::Engine;
#[cfg(feature = "std")]
use std::io::{BufRead, Read, Result, Error};

/// Reads base64 values from a given byte input, stops once it detects the first non base64 char.
#[derive(Debug)]
#[cfg(feature = "std")]
pub struct Base64Reader<R: BufRead> {
    inner: R,
}

#[cfg(feature = "std")]
impl<R: BufRead> Base64Reader<R> {
    /// Creates a new `Base64Reader`.
    pub fn new(input: R) -> Self {
        Base64Reader { inner: input }
    }

    /// Consume `self` and return the inner reader.
    pub fn into_inner(self) -> R {
        self.inner
    }
}

#[cfg(feature = "std")]
impl<R: BufRead> Read for Base64Reader<R> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let mut temp_buf = vec![0; buf.len() * 4 / 3 + 4];
        let available = self.inner.fill_buf()?;
        let mut end = 0;
        for (i, &b) in available.iter().enumerate() {
            if !is_base64_token(b) {
                end = i;
                break;
            }
            end = i + 1;
        }

        let len = base64::engine::general_purpose::STANDARD
            .decode_slice(&available[..end], &mut temp_buf)
            .map_err(|_e| Error::other("base64 decode error"))?;

        buf[..len].copy_from_slice(&temp_buf[..len]);
        self.inner.consume(end);

        Ok(len)
    }
}

#[inline]
fn is_base64_token(c: u8) -> bool {
    ((0x41..=0x5A).contains(&c) || (0x61..=0x7A).contains(&c))
        // alphabetic
        || (0x30..=0x39).contains(&c) //  digit
        || c == b'/'
        || c == b'+'
        || c == b'='
        || c == b'\n'
        || c == b'\r'
}
