import { IndexPointer } from "./tables";
import {
  maskLowerThan,
  maskGreaterThan,
  binarySearchU256,
  binarySearchU128,
  binarySearchU64,
  binarySearchU32,
  binarySearchU16,
  binarySearchU8,
  binarySearchU4,
  binarySearchU2,
  setBitU256,
  unsetBitU256,
  isSetU256,
  isZeroU256,
  byteAt,
  toBuffer
} from "./bst";
import { memcpy } from "../utils/memcpy";
import { console } from "../utils/logging";
import { Box } from "../utils/box";
import { u128 } from "as-bignum/assembly"

export class BSTU128 {
  public ptr: IndexPointer;
  constructor(ptr: IndexPointer) {
    this.ptr = ptr;
  }
  static at(key: IndexPointer): BSTU128 {
    return new BSTU128(key);
  }
  getMaskPointer(partialKey: ArrayBuffer): IndexPointer {
    return this.ptr.select(partialKey).keyword("/mask");
  }
  getMask(partialKey: ArrayBuffer): ArrayBuffer {
    const mask = this.getMaskPointer(partialKey).get();
    if (mask.byteLength === 0) return new ArrayBuffer(32);
    return mask;
  }
  markPath(key: u128): void {
    const keyBytes = new ArrayBuffer(16);
    store<u64>(changetype<usize>(keyBytes), bswap<u64>(key.hi));
    store<u64>(changetype<usize>(keyBytes) + 8, bswap<u64>(key.lo));
    for (let i = 0; i < 16; i++) {
      const partialKey = new ArrayBuffer(i);
      if (i !== 0)
        memcpy(changetype<usize>(partialKey), changetype<usize>(keyBytes), i);
      const ptr = this.getMaskPointer(partialKey);
      const mask = ptr.get();
      const newMask = mask.byteLength === 0 ? new ArrayBuffer(32) : mask;
      const byte = load<u8>(changetype<usize>(keyBytes) + i);
      const isSet = isSetU256(newMask, byte);
      if (!isSetU256(newMask, byte)) {
        setBitU256(newMask, byte);
        ptr.set(newMask);
      }
    }
  }
  unmarkPath(key: u128): void {
    const keyBytes = new ArrayBuffer(16);
    store<u64>(changetype<usize>(keyBytes), bswap<u64>(key.hi));
    store<u64>(changetype<usize>(keyBytes) + 8, bswap<u64>(key.lo));
    for (let i: i32 = <i32>(16 - 1); i >= 0; i--) {
      const partialKey = new ArrayBuffer(i);
      if (i !== 0)
        memcpy(changetype<usize>(partialKey), changetype<usize>(keyBytes), i);
      const ptr = this.getMaskPointer(partialKey);
      const mask = ptr.get();
      const newMask = mask.byteLength === 0 ? new ArrayBuffer(32) : mask;
      const byte = load<u8>(changetype<usize>(keyBytes) + i);
      if (isSetU256(newMask, byte)) {
        unsetBitU256(newMask, byte);
      }
      if (isZeroU256(newMask)) {
        ptr.nullify();
      } else {
        ptr.set(newMask);
        break;
      }
    }
  }
  _findBoundaryFromPartial(keyBytes: ArrayBuffer, seekHigher: bool): u128 {
    let partialKey = keyBytes;
    while (partialKey.byteLength !== 16) {
      const newPartial = new ArrayBuffer(partialKey.byteLength + 1);
      memcpy(
        changetype<usize>(newPartial),
        changetype<usize>(partialKey),
        partialKey.byteLength,
      );
      store<u8>(
        changetype<usize>(newPartial) + <usize>partialKey.byteLength,
        <u8>binarySearchU256(this.getMask(partialKey), seekHigher),
      );
      partialKey = newPartial;
    }
    const result = u128.from(0);
    result.hi = bswap<u64>(load<u64>(changetype<usize>(partialKey)));
    result.lo = bswap<u64>(load<u64>(changetype<usize>(partialKey) + 8));
    return result;
  }
  seekLower(start: u128): u128 {
    let partialKey = new ArrayBuffer(16);
    store<u64>(changetype<usize>(partialKey), bswap<u64>(start.hi));
    store<u64>(changetype<usize>(partialKey) + 8, bswap<u64>(start.lo));
    do {
      const thisKey = Box.from(partialKey).shrinkBack(1).toArrayBuffer();
      const mask = this.getMaskPointer(thisKey).get();
      if (mask.byteLength !== 0) {
        maskLowerThan(mask, byteAt(partialKey, thisKey.byteLength));
        const symbol = binarySearchU256(mask, false);
        if (symbol !== -1) {
          return this._findBoundaryFromPartial(
            Box.concat([Box.from(thisKey), Box.from(toBuffer<u8>(<u8>symbol))]),
            false,
          );
        }
      }
      partialKey = thisKey;
    } while (partialKey.byteLength !== 0);
    return changetype<u128>(0);
  }
  seekGreater(start: u128): u128 {
    let partialKey = new ArrayBuffer(16);
    store<u64>(changetype<usize>(partialKey), bswap<u64>(start.hi));
    store<u64>(changetype<usize>(partialKey) + 8, bswap<u64>(start.lo));
    do {
      const thisKey = Box.from(partialKey).shrinkBack(1).toArrayBuffer();
      const mask = this.getMaskPointer(thisKey).get();
      if (mask.byteLength !== 0) {
        maskGreaterThan(mask, byteAt(partialKey, thisKey.byteLength));
        const symbol = binarySearchU256(mask, true);
        if (symbol !== -1) {
          return this._findBoundaryFromPartial(
            Box.concat([Box.from(thisKey), Box.from(toBuffer<u8>(<u8>symbol))]),
            true,
          );
        }
      }
      partialKey = thisKey;
    } while (partialKey.byteLength !== 0);
    return changetype<u128>(0);
  }
  set(key: u128, v: ArrayBuffer): void {
    const keyBytes = new ArrayBuffer(16);
    store<u64>(changetype<usize>(keyBytes), bswap<u64>(key.hi));
    store<u64>(changetype<usize>(keyBytes) + 8, bswap<u64>(key.lo));
    if (v.byteLength === 0) this.unmarkPath(key);
    else this.markPath(key);
    this.ptr.select(keyBytes).set(v);
  }
  get(key: u128): ArrayBuffer {
    const keyBytes = new ArrayBuffer(16);
    store<u64>(changetype<usize>(keyBytes), bswap<u64>(key.hi));
    store<u64>(changetype<usize>(keyBytes) + 8, bswap<u64>(key.lo));
    return this.ptr.select(keyBytes).get();
  }
  nullify(key: u128): void {
    const keyBytes = new ArrayBuffer(16);
    store<u64>(changetype<usize>(keyBytes), bswap<u64>(key.hi));
    store<u64>(changetype<usize>(keyBytes) + 8, bswap<u64>(key.lo));
    this.ptr.select(keyBytes).set(new ArrayBuffer(0));
  }
}
