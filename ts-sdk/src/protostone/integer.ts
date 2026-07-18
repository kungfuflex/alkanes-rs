// Lightweight integer type wrappers compatible with @magiceden-oss/runestone-lib interface
// Provides u128, u32, u64, u8 with checked arithmetic and LEB128 encoding

import { Option, Some, None } from './monads';

type BigTypedNumber<T> = bigint & { readonly __kind__: T };

export type u128 = BigTypedNumber<'u128'>;
export type u64 = BigTypedNumber<'u64'>;
export type u32 = BigTypedNumber<'u32'>;
export type u8 = BigTypedNumber<'u8'>;

export type FixedArray<T, N extends number> = T[] & { length: N };

// u128 constructor + namespace
export function u128(value: bigint | number): u128 {
  return BigInt(value) as u128;
}

export namespace u128 {
  export const MAX = (1n << 128n) - 1n as u128;

  export function checkedAdd(a: u128, b: u128): Option<u128> {
    const result = BigInt(a) + BigInt(b);
    if (result > BigInt(MAX)) return None;
    return Some(result as u128);
  }

  export function checkedSub(a: u128, b: u128): Option<u128> {
    const result = BigInt(a) - BigInt(b);
    if (result < 0n) return None;
    return Some(result as u128);
  }

  export function tryIntoU32(value: u128): Option<u32> {
    if (BigInt(value) > BigInt(u32.MAX)) return None;
    return Some(Number(value) as unknown as u32);
  }

  export function encodeVarInt(value: u128): Buffer {
    const v: number[] = [];
    let val = BigInt(value);
    while (val >> 7n > 0n) {
      v.push(Number(val & 0xffn) | 0b1000_0000);
      val = val >> 7n;
    }
    v.push(Number(val & 0xffn));
    return Buffer.from(v);
  }
}

// u64 constructor + namespace
export function u64(value: bigint | number): u64 {
  return BigInt(value) as u64;
}

export namespace u64 {
  export const MAX = (1n << 64n) - 1n as u64;
}

// u32 constructor + namespace
export function u32(value: bigint | number): u32 {
  return Number(value) as unknown as u32;
}

export namespace u32 {
  export const MAX = 0xFFFFFFFF as unknown as u32;
}

// u8 constructor + namespace
export function u8(value: bigint | number): u8 {
  return Number(value) as unknown as u8;
}

export namespace u8 {
  export const MAX = 0xFF as unknown as u8;
}
