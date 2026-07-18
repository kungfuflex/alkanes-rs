/**
 * Utility functions for alkanes operations
 */

import { u128 } from "../u128";

/**
 * Store u128 value at pointer (16 bytes, little-endian)
 */
export function storeU128(ptr: usize, value: u128): void {
  value.store(ptr);
}

/**
 * Load u128 value from pointer (16 bytes, little-endian)
 */
export function loadU128(ptr: usize): u128 {
  return u128.load(ptr);
}

/**
 * Convert u128 to ArrayBuffer (16 bytes, little-endian)
 */
export function u128ToArrayBuffer(value: u128): ArrayBuffer {
  const buf = new ArrayBuffer(16);
  value.store(changetype<usize>(buf));
  return buf;
}
