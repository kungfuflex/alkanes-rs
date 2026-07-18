extern crate alloc;
use alloc::string::ToString;
use core::hash::Hasher;

use base64::engine::{general_purpose, Engine as _};
use crc24::Crc24Hasher;

use crate::{
    armor::reader::{BlockType, Headers},
    errors::Result,
};

#[cfg(feature = "std")]
use std::io::Write;


#[cfg(feature = "std")]
pub struct Base64Encoder<'a, W: Write> {
    inner: &'a mut W,
    buffer: alloc::vec::Vec<u8>,
}

#[cfg(feature = "std")]
impl<'a, W: Write> Base64Encoder<'a, W> {
    pub fn new(inner: &'a mut W) -> Self {
        Self {
            inner,
            buffer: alloc::vec::Vec::new(),
        }
    }
}

#[cfg(feature = "std")]
impl<'a, W: Write> Write for Base64Encoder<'a, W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.buffer.extend_from_slice(buf);
        Ok(buf.len())
    }
    
    fn write_all(&mut self, buf: &[u8]) -> std::io::Result<()> {
        self.buffer.extend_from_slice(buf);
        Ok(())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let encoded = general_purpose::STANDARD.encode(&self.buffer);
        self.inner.write_all(encoded.as_bytes())?;
        self.buffer.clear();
        self.inner.flush()
    }
}

#[cfg(feature = "std")]
pub fn write(
    source: &[u8],
    typ: BlockType,
    writer: &mut impl Write,
    headers: Option<&Headers>,
    include_checksum: bool,
) -> Result<()> {
    write_header(writer, typ, headers)?;

    // write body
    let mut crc_hasher = include_checksum.then(Crc24Hasher::new);

    write_body(writer, source, crc_hasher.as_mut())?;

    write_footer(writer, typ, crc_hasher)?;

    Ok(())
}

#[cfg(feature = "std")]
pub(crate) fn write_header(
    writer: &mut impl Write,
    typ: BlockType,
    headers: Option<&Headers>,
) -> Result<()> {
    // write armor header
    writer.write_all(&b"-----BEGIN "[..])?;
    writer.write_all(typ.to_string().as_bytes())?;
    writer.write_all(&b"-----\n"[..])?;

    // write armor headers
    if let Some(headers) = headers {
        for (key, values) in headers.iter() {
            for value in values {
                writer.write_all(key.as_bytes())?;
                writer.write_all(&b": "[..])?;
                writer.write_all(value.as_bytes())?;
                writer.write_all(&b"\n"[..])?;
            }
        }
    }

    writer.write_all(&b"\n"[..])?;
    writer.flush()?;

    Ok(())
}

#[cfg(feature = "std")]
fn write_body(
    writer: &mut impl Write,
    source: &[u8],
    crc_hasher: Option<&mut Crc24Hasher>,
) -> Result<()> {
    // Update CRC if needed
    if let Some(hasher) = crc_hasher {
        hasher.write(source);
    }
    
    // Encode to base64 and write in chunks
    let encoded = base64::engine::general_purpose::STANDARD.encode(source);
    
    // Write in 64-character lines as per RFC
    for chunk in encoded.as_bytes().chunks(64) {
        writer.write_all(chunk)?;
        writer.write_all(b"\n")?;
    }
    
    Ok(())
}

#[cfg(feature = "std")]
pub(crate) fn write_footer(
    writer: &mut impl Write,
    typ: BlockType,
    crc_hasher: Option<Crc24Hasher>,
) -> Result<()> {
    // write crc
    if let Some(crc_hasher) = crc_hasher {
        writer.write_all(b"=")?;

        let crc = crc_hasher.finish() as u32;
        let crc_buf = [
            // (crc >> 24) as u8,
            (crc >> 16) as u8,
            (crc >> 8) as u8,
            crc as u8,
        ];
        let crc_enc = general_purpose::STANDARD.encode(crc_buf);

        writer.write_all(crc_enc.as_bytes())?;
        writer.write_all(&b"\n"[..])?;
    }

    // write footer
    writer.write_all(&b"-----END "[..])?;
    writer.write_all(typ.to_string().as_bytes())?;
    writer.write_all(&b"-----\n"[..])?;
    Ok(())
}
