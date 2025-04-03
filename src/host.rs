//! Host functions for interacting with the Metashrew runtime.
//!
//! This module provides safe wrappers around the raw host functions provided by the Metashrew runtime.

use anyhow::{anyhow, Result};
use protobuf::{Message};
use std::sync::Arc;

/// External host functions provided by the Metashrew runtime
extern "C" {
    /// Get the length of the input data
    pub fn __host_len() -> i32;
    
    /// Load input data into memory
    pub fn __load_input(ptr: i32);
    
    /// Log a message to the host
    pub fn __log(ptr: i32);
    
    /// Flush key-value pairs to the database
    pub fn __flush(ptr: i32);
    
    /// Get a value from the database
    pub fn __get(key_ptr: i32, value_ptr: i32);
    
    /// Get the length of a value in the database
    pub fn __get_len(ptr: i32) -> i32;
}

/// Load the input data from the host
///
/// Returns a tuple containing the block height and the serialized block data
pub fn load_input() -> Result<(u32, Vec<u8>)> {
    let len = unsafe { __host_len() };
    if len <= 4 {
        return Err(anyhow!("Invalid input length"));
    }
    
    let mut buffer = vec![0u8; len as usize];
    unsafe { __load_input(buffer.as_mut_ptr() as i32) };
    
    let height = u32::from_le_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]);
    let block = buffer[4..].to_vec();
    
    Ok((height, block))
}

/// Log a message to the host
pub fn log(message: &str) {
    let bytes = message.as_bytes();
    let len = bytes.len() as u32;
    
    let mut buffer = Vec::with_capacity(4 + bytes.len());
    buffer.extend_from_slice(&len.to_le_bytes());
    buffer.extend_from_slice(bytes);
    
    unsafe { __log(buffer.as_ptr() as i32) };
}

/// Get a value from the database
pub fn get(key: &[u8]) -> Result<Vec<u8>> {
    let key_len = key.len() as u32;
    let mut key_buffer = Vec::with_capacity(4 + key.len());
    key_buffer.extend_from_slice(&key_len.to_le_bytes());
    key_buffer.extend_from_slice(key);
    
    let value_len = unsafe { __get_len(key_buffer.as_ptr() as i32) };
    if value_len == i32::MAX {
        return Err(anyhow!("Failed to get value length"));
    }
    
    let mut value_buffer = vec![0u8; (value_len as usize) + 4];
    unsafe { __get(key_buffer.as_ptr() as i32, value_buffer.as_mut_ptr() as i32) };
    
    Ok(value_buffer[0..value_len as usize].to_vec())
}

/// Flush key-value pairs to the database
pub fn flush(pairs: &[(Vec<u8>, Vec<u8>)]) -> Result<()> {
    use metashrew_support::proto::metashrew::KeyValueFlush;
    
    let mut flush = KeyValueFlush::new();
    for (key, value) in pairs {
        flush.list.push(key.clone());
        flush.list.push(value.clone());
    }
    
    let bytes = flush.write_to_bytes()?;
    let len = bytes.len() as u32;
    
    let mut buffer = Vec::with_capacity(4 + bytes.len());
    buffer.extend_from_slice(&len.to_le_bytes());
    buffer.extend_from_slice(&bytes);
    
    unsafe { __flush(buffer.as_ptr() as i32) };
    Ok(())
}

/// Set a value in the database
pub fn set(key: Arc<Vec<u8>>, value: Arc<Vec<u8>>) {
    let pairs = vec![(key.as_ref().clone(), value.as_ref().clone())];
    if let Err(e) = flush(&pairs) {
        log(&format!("Error setting value: {}", e));
    }
}
