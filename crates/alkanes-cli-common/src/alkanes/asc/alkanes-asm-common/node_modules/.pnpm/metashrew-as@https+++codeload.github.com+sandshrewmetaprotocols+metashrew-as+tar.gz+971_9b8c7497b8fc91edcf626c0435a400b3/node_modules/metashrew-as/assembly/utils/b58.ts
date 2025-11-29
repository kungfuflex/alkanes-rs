"use strict";
import { concat } from "./utils";

const ALPHABET: StaticArray<u8> = [
  49, 50, 51, 52, 53, 54, 55, 56, 57, 65, 66, 67, 68, 69, 70, 71, 72, 74, 75,
  76, 77, 78, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89, 90, 97, 98, 99, 100, 101,
  102, 103, 104, 105, 106, 107, 109, 110, 111, 112, 113, 114, 115, 116, 117,
  118, 119, 120, 121, 122,
];


@inline const BASE = 58;
const LEADER = ALPHABET[0];

// log(256) / log(58) = 1.365658237309761 ~= 554 / 405
@inline const FACTOR_NUM = 406;


@inline const FACTOR_DEN = 554;


@inline const INV_FACTOR_NUM = FACTOR_DEN;


@inline const INV_FACTOR_DEN = FACTOR_NUM - 1;


@inline
function INV_FACTOR(length: i32): i32 {
  return (length * INV_FACTOR_NUM) / INV_FACTOR_DEN + 1; // log(256) / log(58), rounded up
}

/**
 * Encode Uint8Array as a base58 string.
 * @param bytes Byte array of type Uint8Array.
 */
export function base58(source: Uint8Array): ArrayBuffer {
  // Skip & count leading zeroes.
  let pend = source.length;
  let pbegin = 0;
  while (pbegin != pend && source[pbegin] == 0) ++pbegin;
  let zeroes = pbegin;

  // Allocate enough space in big-endian base58 representation.
  let size = INV_FACTOR(pend - pbegin);
  let b58 = new Uint8Array(size);
  let length = 0;

  // Process the bytes.
  while (pbegin != pend) {
    let carry = u32(source[pbegin]);
    // Apply "b58 = b58 * 256 + ch".
    let i = 0;
    for (let it = size - 1; it != -1 && (carry != 0 || i < length); --it, ++i) {
      carry += u32(b58[it]) << 8;
      b58[it] = carry % BASE;
      carry = carry / BASE;
    }
    if (ASC_OPTIMIZE_LEVEL == 0) {
      assert(!carry, "Non-zero carry");
    }
    length = i;
    pbegin++;
  }

  // Skip leading zeroes in base58 result.
  let it = size - length;
  while (it != size && b58[it] == 0) ++it;

  let result = new Uint8Array(zeroes + b58.length);
  result.fill(LEADER, 0, zeroes);
  for (; it < size; ++it) {
    result[zeroes++] = ALPHABET[b58[it]];
  }
  return result.buffer;
}
