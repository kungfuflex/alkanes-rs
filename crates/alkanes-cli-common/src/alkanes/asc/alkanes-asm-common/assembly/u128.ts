/**
 * Simple u128 implementation for stub runtime
 * 
 * This is a minimal implementation that avoids the complexity of as-bignum
 * which doesn't work well with stub runtime. We only implement what's needed
 * for alkanes tx-scripts: storing, loading, and basic operations.
 */

/**
 * Simple u128 wrapper - just stores 16 bytes
 */
export class u128 {
  // Store as two u64 values (low and high)
  lo: u64;
  hi: u64;

  constructor(lo: u64 = 0, hi: u64 = 0) {
    this.lo = lo;
    this.hi = hi;
  }

  /**
   * Create u128 from a small number
   */
  static from(value: i32): u128 {
    return new u128(value as u64, 0);
  }

  /**
   * Zero value
   */
  static get Zero(): u128 {
    return new u128(0, 0);
  }

  /**
   * One value
   */
  static get One(): u128 {
    return new u128(1, 0);
  }

  /**
   * Check if zero
   */
  isZero(): bool {
    return this.lo == 0 && this.hi == 0;
  }

  /**
   * Check equality
   */
  eq(other: u128): bool {
    return this.lo == other.lo && this.hi == other.hi;
  }

  /**
   * Convert to u64 (truncate high bits)
   */
  toU64(): u64 {
    return this.lo;
  }

  /**
   * Convert to string (decimal)
   * Simple implementation for small values only
   */
  toString(): string {
    // For simplicity, only handle values that fit in u64
    if (this.hi == 0) {
      return this.lo.toString();
    }
    return "[large u128]";
  }

  /**
   * Load u128 from memory at pointer
   */
  static load(ptr: usize): u128 {
    return new u128(
      load<u64>(ptr),
      load<u64>(ptr + 8)
    );
  }

  /**
   * Store u128 to memory at pointer
   */
  store(ptr: usize): void {
    store<u64>(ptr, this.lo);
    store<u64>(ptr + 8, this.hi);
  }
}

/**
 * Load u128 from memory pointer
 */
export function loadU128(ptr: usize): u128 {
  return u128.load(ptr);
}

/**
 * Store u128 to memory pointer
 */
export function storeU128(ptr: usize, value: u128): void {
  value.store(ptr);
}

/**
 * Convert u128 to ArrayBuffer (16 bytes, little-endian)
 */
export function u128ToArrayBuffer(value: u128): ArrayBuffer {
  const buf = new ArrayBuffer(16);
  value.store(changetype<usize>(buf));
  return buf;
}
