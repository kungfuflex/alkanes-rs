//! Memory management utilities for host-guest communication
//!
//! This module provides functions for handling the AssemblyScript ArrayBuffer
//! memory layout used for passing data between the WASM guest and the host.

pub fn to_ptr(v: &mut Vec<u8>) -> i32 {
    return v.as_mut_ptr() as usize as i32;
}

pub fn to_passback_ptr(v: &mut Vec<u8>) -> i32 {
    to_ptr(v) + 4
}

pub fn to_arraybuffer_layout<T: AsRef<[u8]>>(v: T) -> Vec<u8> {
    let mut buffer = Vec::<u8>::new();
    buffer.extend_from_slice(&(v.as_ref().len() as u32).to_le_bytes());
    buffer.extend_from_slice(v.as_ref());
    return buffer;
}

pub fn copy_to_wasm<T: AsRef<[u8]>>(v: T) -> i32 {
    let buffer = to_arraybuffer_layout(v);
    let leaked_buffer = Box::leak(buffer.into_boxed_slice());
    let ptr = leaked_buffer.as_mut_ptr();
    (ptr as usize + 4) as i32
}