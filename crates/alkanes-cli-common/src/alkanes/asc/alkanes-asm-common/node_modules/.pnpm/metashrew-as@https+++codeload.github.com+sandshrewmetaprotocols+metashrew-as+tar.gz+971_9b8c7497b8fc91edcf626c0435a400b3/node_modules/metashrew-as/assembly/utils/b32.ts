"use strict";
import { memcpy } from "./memcpy";
import { Box } from "./box";
import { arrayBufferToArray } from "../indexer/index";

const ENCODING_CONST_BECH32: u32 = 1;
const ENCODING_CONST_BECH32M: u32 = 0x2bc830a3;

// const limit: i32 = 90;
const ONE = String.UTF8.encode("1");
const ALPHABET_MAP: StaticArray<u8> = [
  0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
  0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 15, 0, 10,
  17, 21, 20, 26, 30, 7, 5, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
  0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 29, 0, 24,
  13, 25, 9, 8, 23, 0, 18, 22, 31, 27, 19, 0, 1, 0, 3, 16, 11, 28, 12, 14, 6, 4,
  2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
  0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
  0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
  0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
  0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
  0, 0, 0, 0,
];
const ALPHABET: StaticArray<u8> = [
  113, 112, 122, 114, 121, 57, 120, 56, 103, 102, 50, 116, 118, 100, 119, 48,
  115, 51, 106, 110, 53, 52, 107, 104, 99, 101, 54, 109, 117, 97, 55, 108,
];


@inline
function polymodStep(pre: u32): u32 {
  const b = pre >> 25;
  return (
    ((pre & 0x1ffffff) << 5) ^
    (-((b >> 0) & 1) & 0x3b6a57b2) ^
    (-((b >> 1) & 1) & 0x26508e6d) ^
    (-((b >> 2) & 1) & 0x1ea119fa) ^
    (-((b >> 3) & 1) & 0x3d4233dd) ^
    (-((b >> 4) & 1) & 0x2a1462b3)
  );
}


@inline
function lookupByte(n: u8): u8 {
  return load<u8>(changetype<usize>(ALPHABET_MAP) + <usize>n);
}


@inline
function encodeByte(n: u8): u8 {
  return load<u8>(changetype<usize>(ALPHABET) + <usize>n);
}

function prefixChk(prefix: ArrayBuffer): u32 {
  let chk: u32 = 1;
  for (let i: i32 = 0; i < prefix.byteLength; ++i) {
    const c = load<u8>(changetype<usize>(prefix) + <usize>i);
    if (c < <u8>33 || c > <u8>126)
      throw Error("Invalid prefix (" + prefix + ")");

    chk = polymodStep(chk) ^ (c >> 5);
  }
  chk = polymodStep(chk);

  for (let i: i32 = 0; i < prefix.byteLength; ++i) {
    const v = load<u8>(changetype<usize>(prefix) + <usize>i);
    chk = polymodStep(chk) ^ (<u32>(v & (<u8>0x1f)));
  }
  return chk;
}

// convert function uses Uint8Array instead of Array<u8>
function convert(
  data: Uint8Array,
  inBits: u32,
  outBits: u32,
  pad: boolean,
): Array<u8> {
  let value = 0;
  let bits: u32 = 0;
  const maxV = (1 << outBits) - 1;

  const result: Array<u8> = new Array(0);

  for (let i = 0; i < data.length; i++) {
    value = (value << inBits) | data[i];
    bits += inBits;

    while (bits >= outBits) {
      bits -= outBits;
      result.push(<u8>((value >> bits) & maxV));
    }
  }

  if (pad) {
    if (bits > 0) {
      result.push(<u8>((value << (outBits - bits)) & maxV));
    }
  } else {
    if (bits >= inBits) throw Error("Excess padding");
    if ((value << (outBits - bits)) & maxV) throw Error("Non-zero padding");
  }

  return result;
}

export function toWords(bytes: ArrayBuffer): ArrayBuffer {
  let data = Uint8Array.wrap(bytes);
  return fromDataStart(convert(data, 8, 5, true));
}

export function encode(
  prefix: ArrayBuffer,
  words: Array<u8>,
  encoding: u32,
  limit: i32 = 90,
): ArrayBuffer {
  if (prefix.byteLength + 7 + words.length > limit)
    throw new Error("Exceeds length limit");

  let chk = prefixChk(prefix);

  const result = new ArrayBuffer(prefix.byteLength + 7 + words.length);
  memcpy(
    changetype<usize>(result),
    changetype<usize>(prefix),
    prefix.byteLength,
  );
  let ptr: usize = changetype<usize>(result) + prefix.byteLength;
  store<u8>(ptr, load<u8>(changetype<usize>(ONE)));
  ptr++;
  for (let i = 0; i < words.length; ++i) {
    const x = words[i];
    if (x >> 5 !== 0) throw new Error("Non 5-bit word");
    chk = polymodStep(chk) ^ x;
    store<u8>(ptr + i, encodeByte(x));
  }
  ptr += <usize>words.length;

  for (let i: u32 = 0; i < 6; ++i) {
    chk = polymodStep(chk);
  }
  chk ^= encoding;

  for (let i = 0; i < 6; ++i) {
    const v = (chk >> ((5 - i) * 5)) & 0x1f;
    store<u8>(ptr + i, encodeByte(<u8>v));
  }

  return result;
}

export class Decoded {
  public prefix: ArrayBuffer;
  public words: ArrayBuffer;
  constructor(prefix: ArrayBuffer, words: ArrayBuffer) {
    this.prefix = prefix;
    this.words = words;
  }
  static from(prefix: ArrayBuffer, words: ArrayBuffer):  Decoded {
    return new Decoded(prefix, words);
  }
}

const DEFAULT_LIMIT: u32 = 90;

function __decodeDefault(str: string): Decoded {
  return __decode(str, DEFAULT_LIMIT);
}

function __decode(str: string, limit: u32): Decoded {
    if (str.length < 8) return changetype<Decoded>(0);
    if (str.length > <i32>limit) return changetype<Decoded>(0);

    // don't allow mixed case
    const lowered = str.toLowerCase();
    const uppered = str.toUpperCase();
    if (str !== lowered && str !== uppered) return changetype<Decoded>(0);
    str = lowered;

    const split = str.lastIndexOf('1');
    if (split === -1) return changetype<Decoded>(0);
    if (split === 0) return changetype<Decoded>(0);

    const buffer = String.UTF8.encode(str);
    const prefix = Box.from(buffer).setLength(split).toArrayBuffer();
    const wordChars = Box.from(buffer).shrinkFront(split +1).toArrayBuffer();
    if (wordChars.byteLength < 6) return changetype<Decoded>(0);

    let chk = prefixChk(prefix);

    const words = new Array<u8>(0);
    for (let i = 0; i < wordChars.byteLength; ++i) {
      const c = load<u8>(changetype<usize>(wordChars) + <usize>i);
      const v = lookupByte(c);
      chk = polymodStep(chk) ^ v;

      // not in the checksum?
      if (i + 6 >= wordChars.byteLength) continue;
      words.push(v);
    }

//    if (chk !== ENCODING_CONST) return changetype<Decoded>(0);
    return Decoded.from(prefix, fromDataStart(words));
  }


export function fromDataStart<T>(v:  T): ArrayBuffer {
  return Box.from(changetype<ArrayBuffer>(v.dataStart)).setLength(v.length).toArrayBuffer();
}
export function fromWords(words: ArrayBuffer): ArrayBuffer {
  const converted = convert(Uint8Array.wrap(words), 5, 8, false);
  return converted.buffer;
}

  export function b32decode(str: string): Decoded {
    return __decodeDefault(str);
  }



export function bech32m(prefix: ArrayBuffer, words: ArrayBuffer): ArrayBuffer {
  return encode(prefix, arrayBufferToArray(words), ENCODING_CONST_BECH32M);
}

export function bech32(prefix: ArrayBuffer, words: ArrayBuffer): ArrayBuffer {
  return encode(prefix, arrayBufferToArray(words), ENCODING_CONST_BECH32);
}
