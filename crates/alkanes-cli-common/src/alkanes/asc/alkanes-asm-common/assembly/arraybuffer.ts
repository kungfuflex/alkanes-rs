// ArrayBuffer utilities for interfacing with alkanes runtime
// The runtime expects pointers to have a 4-byte length prefix at ptr-4

/**
 * Allocate an ArrayBuffer-compatible region  
 * Layout: [length: u32][data: T[]]
 * @param size Size of data region in bytes
 * @returns Pointer to data (length is at ptr-4)
 */
export function allocArrayBuffer(size: i32): ArrayBuffer {
  // Allocate size + 4 bytes for length prefix
  const totalSize = size + 4;
  const buf = new ArrayBuffer(totalSize);
  
  // Write length at offset 0
  store<u32>(changetype<usize>(buf), size);
  
  // Return pointer to data (skip length prefix)
  return buf;
}

/**
 * Get pointer to data in ArrayBuffer (skips length prefix)
 */
export function getDataPtr(buf: ArrayBuffer): usize {
  return changetype<usize>(buf) + 4;
}

/**
 * Get pointer to length prefix in ArrayBuffer
 */
export function getLengthPtr(buf: ArrayBuffer): usize {
  return changetype<usize>(buf);
}

/**
 * Get the data length from ArrayBuffer
 */
export function getDataLength(buf: ArrayBuffer): u32 {
  return load<u32>(changetype<usize>(buf));
}

/**
 * Set the data length in ArrayBuffer
 */
export function setDataLength(buf: ArrayBuffer, len: u32): void {
  store<u32>(changetype<usize>(buf), len);
}

/**
 * Create ArrayBuffer from existing data pointer
 * Assumes data has length prefix at ptr-4
 */
export function fromDataPtr(ptr: usize): ArrayBuffer {
  const len = load<u32>(ptr - 4);
  const totalSize = len + 4;
  
  // Create new buffer and copy
  const buf = new ArrayBuffer(totalSize);
  memory.copy(changetype<usize>(buf), ptr - 4, totalSize);
  
  return buf;
}

/**
 * Write u128 value in little-endian format
 */
export function writeU128(ptr: usize, value: u128): void {
  store<u128>(ptr, value);
}

/**
 * Read u128 value in little-endian format
 */
export function readU128(ptr: usize): u128 {
  return load<u128>(ptr);
}

/**
 * Write u64 value in little-endian format
 */
export function writeU64(ptr: usize, value: u64): void {
  store<u64>(ptr, value);
}

/**
 * Read u64 value in little-endian format
 */
export function readU64(ptr: usize): u64 {
  return load<u64>(ptr);
}

/**
 * Write u32 value in little-endian format
 */
export function writeU32(ptr: usize, value: u32): void {
  store<u32>(ptr, value);
}

/**
 * Read u32 value in little-endian format
 */
export function readU32(ptr: usize): u32 {
  return load<u32>(ptr);
}
