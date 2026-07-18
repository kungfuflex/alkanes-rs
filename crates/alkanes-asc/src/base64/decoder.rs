//! # base64 decoder module
extern crate alloc;

use std::io::BufReader;

#[cfg(feature = "std")]
use std::io::Read;


/// Decodes Base64 from the supplied reader.
#[derive(Debug)]
#[cfg(feature = "std")]
pub struct Base64Decoder<R: Read> {
    /// The inner Read instance we are reading bytes from.
    inner: BufReader<R>,
}

#[cfg(feature = "std")]
impl<R: Read> Base64Decoder<R> {
    /// Creates a new `Base64Decoder`.
    pub fn new(input: R) -> Self {
        Base64Decoder {
            inner: BufReader::new(input),
        }
    }

}

#[cfg(feature = "std")]
impl<R: Read> Read for Base64Decoder<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.inner.read(buf)
    }
}
