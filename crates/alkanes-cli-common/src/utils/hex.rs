//! Hex encoding utility trait.

use crate::Result;
#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};
#[cfg(feature = "std")]
use std::{string::String, vec::Vec};
use bitcoin::hashes::hex::FromHex;
use bitcoin::ScriptBuf;
use hex;

pub trait ToHexString {
    fn to_hex_string(&self) -> String;
}

impl ToHexString for ScriptBuf {
    fn to_hex_string(&self) -> String {
        hex::encode(self.as_bytes())
    }
}

/// Reverse the bytes of a txid for trace calls
/// Bitcoin txids are displayed in reverse byte order compared to their internal representation
pub fn reverse_txid_bytes(txid: &str) -> Result<String> {
    let mut txid_bytes = Vec::from_hex(txid)?;
    txid_bytes.reverse();
    Ok(hex::encode(txid_bytes))
}