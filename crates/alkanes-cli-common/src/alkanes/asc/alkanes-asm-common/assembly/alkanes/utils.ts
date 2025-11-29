/**
 * Utility functions for alkanes contracts
 */

import { u128 } from "as-bignum/assembly";

/**
 * Store u128 value at pointer as two u64 values (little-endian)
 */
export function storeU128(ptr: usize, value: u128): void {
  // Get lower 64 bits by creating a mask
  const mask64 = u128.from(0xFFFFFFFFFFFFFFFF);
  const lo = u128.and(value, mask64);
  // Get upper 64 bits by right-shifting
  const hi = u128.shr(value, 64);
  
  // Convert to u64 for storage (truncates to lower 64 bits of each)
  store<u64>(ptr, lo.lo);
  store<u64>(ptr + 8, hi.lo);
}

/**
 * Load u128 value from pointer (two u64 values in little-endian)
 */
export function loadU128(ptr: usize): u128 {
  const lo = load<u64>(ptr);
  const hi = load<u64>(ptr + 8);
  return u128.or(u128.from(lo), u128.shl(u128.from(hi), 64));
}

/**
 * Convert u128 to little-endian bytes
 */
export function u128ToBytes(value: u128): ArrayBuffer {
  const buf = new ArrayBuffer(16);
  const ptr = changetype<usize>(buf);
  storeU128(ptr, value);
  return buf;
}

/**
 * Parse u128 from bytes (little-endian)
 */
export function bytesToU128(bytes: ArrayBuffer): u128 {
  const ptr = changetype<usize>(bytes);
  return loadU128(ptr);
}
