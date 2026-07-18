import { u128 } from "./u128";
// Utility functions

/**
 * Minimum of two values
 */
export function min<T>(a: T, b: T): T {
  return a < b ? a : b;
}

/**
 * Maximum of two values
 */
export function max<T>(a: T, b: T): T {
  return a > b ? a : b;
}

/**
 * Clamp value between min and max
 */
export function clamp<T>(value: T, minVal: T, maxVal: T): T {
  return min(max(value, minVal), maxVal);
}

/**
 * Copy memory from source to destination
 */
export function memcpy(dest: usize, src: usize, len: usize): void {
  memory.copy(dest, src, len);
}

/**
 * Set memory to a value
 */
export function memset(dest: usize, value: u8, len: usize): void {
  for (let i: usize = 0; i < len; i++) {
    store<u8>(dest + i, value);
  }
}

/**
 * Compare two memory regions
 */
export function memcmp(a: usize, b: usize, len: usize): i32 {
  return memory.compare(a, b, len);
}

/**
 * Convert u128 to hex string (little-endian)
 */
export function u128ToHex(value: u128): string {
  let result = "";
  let remaining = value;
  
  // Extract bytes in little-endian order
  for (let i = 0; i < 16; i++) {
    const byte = (remaining as u64 & 0xFF) as u8;
    const hex = byte.toString(16);
    result += hex.length == 1 ? "0" + hex : hex;
    remaining >>= 8;
  }
  
  return result;
}

/**
 * Convert hex string to bytes
 */
export function hexToBytes(hex: string): Uint8Array {
  const len = hex.length / 2;
  const bytes = new Uint8Array(len);
  
  for (let i = 0; i < len; i++) {
    const byteString = hex.substr(i * 2, 2);
    bytes[i] = I32.parseInt(byteString, 16) as u8;
  }
  
  return bytes;
}

/**
 * Convert bytes to hex string
 */
export function bytesToHex(bytes: Uint8Array): string {
  let result = "";
  for (let i = 0; i < bytes.length; i++) {
    const hex = bytes[i].toString(16);
    result += hex.length == 1 ? "0" + hex : hex;
  }
  return result;
}

/**
 * Check if two ArrayBuffers are equal
 */
export function isEqualArrayBuffer(a: ArrayBuffer, b: ArrayBuffer): bool {
  if (a.byteLength !== b.byteLength) return false;
  return memory.compare(
    changetype<usize>(a),
    changetype<usize>(b),
    a.byteLength
  ) == 0;
}

/**
 * Concatenate multiple ArrayBuffers
 */
export function concatArrayBuffers(buffers: ArrayBuffer[]): ArrayBuffer {
  let totalLength = 0;
  for (let i = 0; i < buffers.length; i++) {
    totalLength += buffers[i].byteLength;
  }
  
  const result = new ArrayBuffer(totalLength);
  let offset = 0;
  
  for (let i = 0; i < buffers.length; i++) {
    memory.copy(
      changetype<usize>(result) + offset,
      changetype<usize>(buffers[i]),
      buffers[i].byteLength
    );
    offset += buffers[i].byteLength;
  }
  
  return result;
}

/**
 * Slice ArrayBuffer
 */
export function sliceArrayBuffer(buf: ArrayBuffer, start: i32, end: i32 = -1): ArrayBuffer {
  if (end < 0) end = buf.byteLength;
  const len = end - start;
  const result = new ArrayBuffer(len);
  memory.copy(
    changetype<usize>(result),
    changetype<usize>(buf) + start,
    len
  );
  return result;
}

export function u128ToArrayBuffer(data: u128): ArrayBuffer {
  const bytes = data.toBytes();
  return changetype<Uint8Array>(bytes).buffer;
}
