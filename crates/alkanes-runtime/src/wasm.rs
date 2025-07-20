// Journal:
// - 2025-07-20: Chadson. Re-created this wasm helper module inside alkanes-runtime
//   to resolve dependency issues. The original implementation was in metashrew-core,
//   but alkanes-runtime cannot depend on it. These functions are for data layout
//   and passing between the WASM guest and the host.

/// Prepends the length of the byte array as a little-endian u32.
/// This is a common convention for passing dynamically sized data to a WASM host.
pub fn to_arraybuffer_layout<T: Into<Vec<u8>>>(data: T) -> Vec<u8> {
    let mut vec: Vec<u8> = data.into();
    let len = vec.len() as u32;
    let mut result = len.to_le_bytes().to_vec();
    result.append(&mut vec);
    result
}

/// Returns the raw pointer to the buffer as an i32.
/// Used for passing a buffer to a host function.
pub fn to_passback_ptr(buffer: &mut Vec<u8>) -> i32 {
    buffer.as_mut_ptr() as i32
}

/// Returns the raw pointer to the buffer as an i32.
/// This is an alias for `to_passback_ptr`.
pub fn to_ptr(buffer: &mut Vec<u8>) -> i32 {
    buffer.as_mut_ptr() as i32
}