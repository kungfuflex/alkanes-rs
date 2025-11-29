import { IndexPointer } from "./tables";
import { memcpy } from "../utils/memcpy";
import { console } from "../utils/logging";
import { Box } from "../utils/box";

export function maskLowerThan(v: ArrayBuffer, position: u8): void {
  const ary = new Array<u64>(4);
  ary[0] = bswap<u64>(load<u64>(changetype<usize>(v)));
  ary[1] = bswap<u64>(load<u64>(changetype<usize>(v) + sizeof<u64>()));
  ary[2] = bswap<u64>(load<u64>(changetype<usize>(v) + 2 * sizeof<u64>()));
  ary[3] = bswap<u64>(load<u64>(changetype<usize>(v) + 3 * sizeof<u64>()));
  const byteSelected = position / 64;
  const bitSelected = position % 64;
  ary[byteSelected] =
    ary[byteSelected] & (<u64>(((1 << bitSelected) - 1) << (64 - bitSelected)));
  for (let i = byteSelected + 1; i < 4; i++) {
    ary[i] = 0;
  }
  for (let i = 0; i < 4; i++) {
    store<u64>(changetype<usize>(v) + i * sizeof<u64>(), bswap<u64>(ary[i]));
  }
}

export function maskGreaterThan(v: ArrayBuffer, position: u8): void {
  const ary = new Array<u64>(4);
  ary[0] = bswap<u64>(load<u64>(changetype<usize>(v)));
  ary[1] = bswap<u64>(load<u64>(changetype<usize>(v) + sizeof<u64>()));
  ary[2] = bswap<u64>(load<u64>(changetype<usize>(v) + 2 * sizeof<u64>()));
  ary[3] = bswap<u64>(load<u64>(changetype<usize>(v) + 3 * sizeof<u64>()));
  const byteSelected = position / 64;
  const bitSelected = position % 64;
  ary[byteSelected] =
    ary[byteSelected] &
    ~(<u64>(((1 << (bitSelected + 1)) - 1) << (63 - bitSelected)));
  for (let i: u8 = 0; i < byteSelected; i++) {
    ary[i] = 0;
  }
  for (let i = 0; i < 4; i++) {
    store<u64>(changetype<usize>(v) + i * sizeof<u64>(), bswap<u64>(ary[i]));
  }
}

export function binarySearchU256(v: ArrayBuffer, forHighest: bool): i32 {
  const leftHigh = bswap<u64>(load<u64>(changetype<usize>(v)));
  const leftLow = bswap<u64>(load<u64>(changetype<usize>(v) + sizeof<u64>()));
  const rightHigh = bswap<u64>(load<u64>(changetype<usize>(v) + 16));
  const rightLow = bswap<u64>(load<u64>(changetype<usize>(v) + 24));
  const left = leftHigh | leftLow;
  const right = rightHigh | rightLow;
  if ((left | right) === 0) return -1;
  if ((forHighest || right === 0) && left !== 0) {
    return binarySearchU128(leftHigh, leftLow, forHighest);
  } else {
    return (
      sizeof<u64>() * 2 * 8 + binarySearchU128(rightHigh, rightLow, forHighest)
    );
  }
}

export function binarySearchU128(high: u64, low: u64, forHighest: bool): i32 {
  if ((forHighest || low === 0) && high !== 0) {
    return binarySearchU64(high, forHighest);
  } else {
    return sizeof<u64>() * 8 + binarySearchU64(low, forHighest);
  }
}

export function binarySearchU64(word: u64, forHighest: bool): i32 {
  const low = <u32>(word & U32.MAX_VALUE);
  const high = <u32>((word >> (<u64>sizeof<u32>() * 8)) & U32.MAX_VALUE);
  if ((forHighest || low === 0) && high !== 0)
    return binarySearchU32(high, forHighest);
  return sizeof<u32>() * 8 + binarySearchU32(low, forHighest);
}

export function binarySearchU32(word: u32, forHighest: bool): i32 {
  const low: u16 = <u16>(word & U16.MAX_VALUE);
  const high: u16 = <u16>((word >> (<u32>sizeof<u16>() * 8)) & U16.MAX_VALUE);
  if ((forHighest || low === 0) && high !== 0) {
    return binarySearchU16(high, forHighest);
  } else {
    return sizeof<u16>() * 8 + binarySearchU16(low, forHighest);
  }
}

export function binarySearchU16(word: u16, forHighest: bool): i32 {
  const low: u8 = <u8>(word & U8.MAX_VALUE);
  const high: u8 = <u8>((word >> (<u16>sizeof<u8>() * 8)) & U8.MAX_VALUE);
  if ((forHighest || low === 0) && high !== 0) {
    return binarySearchU8(high, forHighest);
  } else {
    return sizeof<u8>() * 8 + binarySearchU8(low, forHighest);
  }
}

export function binarySearchU8(word: u8, forHighest: bool): i32 {
  const high = (word >> 4) & 0x0f;
  const low = word & 0x0f;
  if ((forHighest || low === 0) && high !== 0) {
    return binarySearchU4(high, forHighest);
  }
  return 4 + binarySearchU4(low, forHighest);
}

export function binarySearchU4(word: u8, forHighest: bool): i32 {
  const high = (word >> 2) & 0x03;
  const low = word & 0x03;
  if ((forHighest || low === 0) && high !== 0) {
    return binarySearchU2(high, forHighest);
  }
  return 2 + binarySearchU2(low, forHighest);
}

export function binarySearchU2(word: u8, forHighest: bool): i32 {
  const high = (word >> 1) & 0x01;
  const low = word & 0x01;
  return (forHighest || low === 0) && high !== 0 ? 0 : 1;
}

export function setBitU256(mask: ArrayBuffer, position: i32): void {
  const bytePosition = position / 8;
  const bitPosition = position % 8;
  const existingByte: u8 = load<u8>(
    changetype<usize>(mask) + <usize>bytePosition,
  );
  const newBit: u8 = <u8>(1 << (<u8>7 - <u8>bitPosition));
  store<u8>(
    changetype<usize>(mask) + <usize>bytePosition,
    existingByte | newBit,
  );
}

export function unsetBitU256(mask: ArrayBuffer, position: i32): void {
  const bytePosition = position / 8;
  const bitPosition = position % 8;
  const existingByte = load<u8>(changetype<usize>(mask) + <usize>bytePosition);
  const bitMask = ~(<u8>(1 << (<u8>7 - <u8>bitPosition)));
  store<u8>(
    changetype<usize>(mask) + <usize>bytePosition,
    existingByte & bitMask,
  );
}

export function isSetU256(mask: ArrayBuffer, position: i32): boolean {
  const bytePosition: usize = position / 8;
  const bitPosition: i32 = position % 8;
  const existingByte = load<u8>(changetype<usize>(mask) + <usize>bytePosition);
  const bitMask = <u8>(1 << (7 - <u8>bitPosition));
  return (bitMask & existingByte) !== 0;
}

export function isZeroU256(mask: ArrayBuffer): boolean {
  for (var i = 0; i < 4; i++) {
    if (load<u64>(changetype<usize>(mask) + sizeof<u64>() * i) !== 0)
      return false;
  }
  return true;
}

export function byteAt<T>(v: ArrayBuffer, byte: T): u8 {
  return load<u8>(changetype<usize>(v) + <usize>byte);
}

export function toBuffer<T>(v: T): ArrayBuffer {
  const data = new ArrayBuffer(sizeof<T>());
  store<T>(changetype<usize>(data), bswap<T>(v));
  return data;
}

function coerceToZero(v: i32): u8 {
  if (v === -1) return <u8>0;
  return <u8>v;
}

export class BST<K> {
  public ptr: IndexPointer;
  constructor(ptr: IndexPointer) {
    this.ptr = ptr;
  }
  static at<K>(key: IndexPointer): BST<K> {
    return new BST<K>(key);
  }
  getMaskPointer(partialKey: ArrayBuffer): IndexPointer {
    return this.ptr.select(partialKey).keyword("/mask");
  }
  getMask(partialKey: ArrayBuffer): ArrayBuffer {
    const mask = this.getMaskPointer(partialKey).get();
    if (mask.byteLength === 0) return new ArrayBuffer(32);
    return mask;
  }
  markPath(key: K): void {
    const keyBytes = new ArrayBuffer(sizeof<K>());
    store<K>(changetype<usize>(keyBytes), bswap<K>(key));
    for (let i = 0; i < sizeof<K>(); i++) {
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
  unmarkPath(key: K): void {
    const keyBytes = new ArrayBuffer(sizeof<K>());
    store<K>(changetype<usize>(keyBytes), bswap<K>(key));
    for (let i: i32 = <i32>sizeof<K>() - 1; i >= 0; i--) {
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
  _findBoundaryFromPartial(keyBytes: ArrayBuffer, seekHigher: bool): K {
    let partialKey = keyBytes;
    while (partialKey.byteLength !== sizeof<K>()) {
      const newPartial = new ArrayBuffer(partialKey.byteLength + 1);
      memcpy(
        changetype<usize>(newPartial),
        changetype<usize>(partialKey),
        partialKey.byteLength,
      );
      const mask = this.getMask(partialKey);
      const symbol = binarySearchU256(mask.byteLength === 0 ? new ArrayBuffer(32) : mask, seekHigher);
      if (symbol === -1) break;
      store<u8>(
        changetype<usize>(newPartial) + <usize>partialKey.byteLength,
        <u8>symbol
      );
      partialKey = newPartial;
    }
    return bswap<K>(load<K>(changetype<usize>(partialKey)));
  }
  seekLower(start: K): K {
    let partialKey = new ArrayBuffer(sizeof<K>());
    store<K>(changetype<usize>(partialKey), bswap<K>(start));
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
    return ~(0 as K);
  }
  seekGreater(start: K): K {
    let partialKey = new ArrayBuffer(sizeof<K>());
    store<K>(changetype<usize>(partialKey), bswap<K>(start));
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
    return 0 as K;
  }
  set(k: K, v: ArrayBuffer): void {
    const key = bswap<K>(k);
    const keyBytes = new ArrayBuffer(sizeof<K>());
    store<K>(changetype<usize>(keyBytes), key);
    if (v.byteLength === 0) this.unmarkPath(k);
    else this.markPath(k);
    this.ptr.select(keyBytes).set(v);
  }
  get(k: K): ArrayBuffer {
    const key = bswap<K>(k);
    const keyBytes = new ArrayBuffer(sizeof<K>());
    store<K>(changetype<usize>(keyBytes), key);
    return Box.from(this.ptr.select(keyBytes).get()).toArrayBuffer();
  }
  nullify(k: K): void {
    const key = bswap<K>(k);
    const keyBytes = new ArrayBuffer(sizeof<K>());
    store<K>(changetype<usize>(keyBytes), key);
    this.ptr.select(keyBytes).set(new ArrayBuffer(0));
  }
}

/*
export class FixedBST {
  public ptr: IndexPointer;
  public keySize: usize;
  constructor(ptr: IndexPointer, keySize: usize) {
    this.ptr = ptr;
    this.keySize = keySize;
  }
  static at(key: IndexPointer, keySize: usize): FixedBST {
    return new FixedBST(key, keySize);
  }
  getMaskPointer(partialKey: ArrayBuffer): IndexPointer {
    return this.ptr.select(partialKey).keyword("/mask");
  }
  getMask(partialKey: ArrayBuffer): ArrayBuffer {
    const mask = this.getMaskPointer(partialKey).get();
    if (mask.byteLength === 0) return new ArrayBuffer(32);
    return mask;
  }
  markPath(keyBytes: ArrayBuffer): void {
    for (let i = 0; i < <i32>this.keySize; i++) {
      const partialKey = new ArrayBuffer(i);
      if (i !== 0)
        memcpy(changetype<usize>(partialKey), changetype<usize>(keyBytes), i);
      const ptr = this.getMaskPointer(partialKey);
      const mask = ptr.get();
      const newMask = mask.byteLength === 0 ? new ArrayBuffer(32) : mask;
      const byte = load<u8>(changetype<usize>(keyBytes) + i);
      if (!isSetU256(newMask, byte)) {
        setBitU256(newMask, byte);
        ptr.set(newMask);
      }
    }
  }
  unmarkPath(keyBytes: ArrayBuffer): void {
    for (let i: i32 = <i32>this.keySize - 1; i >= 0; i--) {
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
        break;
      } else {
        ptr.set(newMask);
      }
    }
  }
  _findBoundaryFromPartial(
    keyBytes: ArrayBuffer,
    seekHigher: bool,
  ): ArrayBuffer {
    let partialKey = keyBytes;
    while (<usize>partialKey.byteLength !== this.keySize) {
      const newPartial = new ArrayBuffer(partialKey.byteLength + 1);
      memcpy(
        changetype<usize>(newPartial),
        changetype<usize>(partialKey),
        partialKey.byteLength,
      );
      store<u8>(
        changetype<usize>(newPartial) + <usize>partialKey.byteLength,
        <u8>coerceToZero(binarySearchU256(this.getMask(partialKey), seekHigher)),
      );
      partialKey = newPartial;
    }
    return partialKey;
  }
  seekLower(start: ArrayBuffer): ArrayBuffer {
    let partialKey = start;
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
    return changetype<ArrayBuffer>(0);
  }
  seekGreater(start: ArrayBuffer): ArrayBuffer {
    let partialKey = start;
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
    } while (partialKey.byteLength != 0);
    return changetype<ArrayBuffer>(0);
  }
  set(keyBytes: ArrayBuffer, v: ArrayBuffer): void {
    if (v.byteLength === 0) this.unmarkPath(keyBytes);
    else this.markPath(keyBytes);
    this.ptr.select(keyBytes).set(v);
  }
  get(keyBytes: ArrayBuffer): ArrayBuffer {
    return this.ptr.select(keyBytes).get();
  }
  nullify(keyBytes: ArrayBuffer): void {
    this.ptr.select(keyBytes).set(new ArrayBuffer(0));
  }
}
*/
