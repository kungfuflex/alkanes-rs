//! Utility functions for Metashrew.
//!
//! This module provides utility functions for working with Metashrew.

use anyhow::{anyhow, Result};

/// Convert a u32 to a little-endian byte array
pub fn u32_to_le_bytes(value: u32) -> [u8; 4] {
    value.to_le_bytes()
}

/// Convert a little-endian byte array to a u32
pub fn le_bytes_to_u32(bytes: &[u8]) -> Result<u32> {
    if bytes.len() < 4 {
        return Err(anyhow!("Byte array too short for u32"));
    }
    
    let mut array = [0u8; 4];
    array.copy_from_slice(&bytes[0..4]);
    
    Ok(u32::from_le_bytes(array))
}

/// Create a key for a list item
pub fn make_list_key(key: &[u8], index: u32) -> Vec<u8> {
    let mut result = key.to_vec();
    result.extend_from_slice(&u32_to_le_bytes(index));
    result
}

/// Create a length key for a list
pub fn make_length_key(key: &[u8]) -> Vec<u8> {
    make_list_key(key, u32::MAX)
}

/// Annotate a value with a block height
pub fn annotate_value(value: &[u8], height: u32) -> Vec<u8> {
    let mut result = value.to_vec();
    result.extend_from_slice(&u32_to_le_bytes(height));
    result
}

/// Extract the value and height from an annotated value
pub fn extract_annotated_value(annotated: &[u8]) -> Result<(Vec<u8>, u32)> {
    if annotated.len() < 4 {
        return Err(anyhow!("Annotated value too short"));
    }
    
    let value = annotated[0..annotated.len() - 4].to_vec();
    let height = le_bytes_to_u32(&annotated[annotated.len() - 4..])?;
    
    Ok((value, height))
}

/// Encode a string as bytes with length prefix
pub fn encode_string(s: &str) -> Vec<u8> {
    let bytes = s.as_bytes();
    let len = bytes.len() as u32;
    
    let mut result = Vec::with_capacity(4 + bytes.len());
    result.extend_from_slice(&u32_to_le_bytes(len));
    result.extend_from_slice(bytes);
    
    result
}

/// Decode bytes with length prefix to a string
pub fn decode_string(bytes: &[u8]) -> Result<String> {
    if bytes.len() < 4 {
        return Err(anyhow!("Byte array too short for string"));
    }
    
    let len = le_bytes_to_u32(&bytes[0..4])? as usize;
    if bytes.len() < 4 + len {
        return Err(anyhow!("Byte array too short for string of length {}", len));
    }
    
    let s = String::from_utf8(bytes[4..4 + len].to_vec())
        .map_err(|e| anyhow!("Invalid UTF-8: {}", e))?;
    
    Ok(s)
}

/// Encode a byte array with length prefix
pub fn encode_bytes(bytes: &[u8]) -> Vec<u8> {
    let len = bytes.len() as u32;
    
    let mut result = Vec::with_capacity(4 + bytes.len());
    result.extend_from_slice(&u32_to_le_bytes(len));
    result.extend_from_slice(bytes);
    
    result
}

/// Decode bytes with length prefix
pub fn decode_bytes(bytes: &[u8]) -> Result<Vec<u8>> {
    if bytes.len() < 4 {
        return Err(anyhow!("Byte array too short for bytes"));
    }
    
    let len = le_bytes_to_u32(&bytes[0..4])? as usize;
    if bytes.len() < 4 + len {
        return Err(anyhow!("Byte array too short for bytes of length {}", len));
    }
    
    Ok(bytes[4..4 + len].to_vec())
}