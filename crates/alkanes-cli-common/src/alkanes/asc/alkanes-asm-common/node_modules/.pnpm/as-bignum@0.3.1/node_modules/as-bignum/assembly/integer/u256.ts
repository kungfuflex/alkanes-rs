import { i128 } from './i128';
import { u128 } from './u128';
import { u256toDecimalString } from "../utils";
import { __mul256 } from '../globals';

@lazy const HEX_CHARS = '0123456789abcdef';

export class u256 {

  @inline static get Zero(): u256 { return new u256(); }
  @inline static get One():  u256 { return new u256(1); }
  @inline static get Min():  u256 { return new u256(); }
  @inline static get Max():  u256 { return new u256(-1, -1, -1, -1); }

  // TODO: fromString

  @inline
  static fromU256(value: u256): u256 {
    return new u256(value.lo1, value.lo2, value.hi1, value.hi2);
  }

  @inline
  static fromU128(value: u128): u256 {
    return new u256(value.lo, value.hi);
  }

  @inline
  static fromU64(value: u64): u256 {
    return new u256(value);
  }

  @inline
  static fromI64(value: i64): u256 {
    var mask = value >> 63;
    return new u256(value, mask, mask, mask);
  }

  @inline
  static fromU32(value: u32): u256 {
    return new u256(value);
  }

  @inline
  static fromI32(value: i32): u256 {
    var mask: u64 = value >> 63;
    return new u256(value, mask, mask, mask);
  }

  @inline
  static fromBits(
    l0: u32, l1: u32, l2: u32, l3: u32,
    h0: u32, h1: u32, h2: u32, h3: u32,
  ): u256 {
    return new u256(
      <u64>l0 | ((<u64>l1) << 32),
      <u64>l2 | ((<u64>l3) << 32),
      <u64>h0 | ((<u64>h1) << 32),
      <u64>h2 | ((<u64>h3) << 32),
    );
  }

  @inline
  static fromBytes<T>(array: T, bigEndian: bool = false): u256 {
    // @ts-ignore
    if (array instanceof u8[]) {
      return bigEndian
        // @ts-ignore
        ? u256.fromBytesBE(<u8[]>array)
        // @ts-ignore
        : u256.fromBytesLE(<u8[]>array);
    } else if (array instanceof Uint8Array) {
      return bigEndian
        ? u256.fromUint8ArrayBE(<Uint8Array>array)
        : u256.fromUint8ArrayLE(<Uint8Array>array);
    } else {
      throw new TypeError("Unsupported generic type");
    }
  }

  @inline
  static fromBytesLE(array: u8[]): u256 {
    assert(array.length && (array.length & 31) == 0);
    // @ts-ignore
    var buffer = array.dataStart
    return new u256(
      load<u64>(buffer, 0 * sizeof<u64>()),
      load<u64>(buffer, 1 * sizeof<u64>()),
      load<u64>(buffer, 2 * sizeof<u64>()),
      load<u64>(buffer, 3 * sizeof<u64>()),
    );
  }

  @inline
  static fromBytesBE(array: u8[]): u256 {
    assert(array.length && (array.length & 31) == 0);
    var buffer = array.dataStart;
    return new u256(
      bswap<u64>(load<u64>(buffer, 3 * sizeof<u64>())),
      bswap<u64>(load<u64>(buffer, 2 * sizeof<u64>())),
      bswap<u64>(load<u64>(buffer, 1 * sizeof<u64>())),
      bswap<u64>(load<u64>(buffer, 0 * sizeof<u64>()))
    );
  }

  @inline
  static fromUint8ArrayLE(array: Uint8Array): u256 {
    assert(array.length && (array.length & 31) == 0);
    var buffer = array.dataStart;
    return new u256(
        load<u64>(buffer, 0 * sizeof<u64>()),
        load<u64>(buffer, 1 * sizeof<u64>()),
        load<u64>(buffer, 2 * sizeof<u64>()),
        load<u64>(buffer, 3 * sizeof<u64>())
    );
  }

  @inline
  static fromUint8ArrayBE(array: Uint8Array): u256 {
    assert(array.length && (array.length & 31) == 0);
    var buffer = array.dataStart;
    return new u256(
        bswap<u64>(load<u64>(buffer, 3 * sizeof<u64>())),
        bswap<u64>(load<u64>(buffer, 2 * sizeof<u64>())),
        bswap<u64>(load<u64>(buffer, 1 * sizeof<u64>())),
        bswap<u64>(load<u64>(buffer, 0 * sizeof<u64>()))
    );
  }

  // TODO need improvement
  // max safe uint for f64 actually 52-bits
  @inline
  static fromF64(value: f64): u256 {
    var mask = u64(reinterpret<i64>(value) >> 63);
    return new u256(<u64>value, mask, mask, mask);
  }

  // TODO need improvement
  // max safe int for f32 actually 23-bits
  @inline
  static fromF32(value: f32): u256 {
    var mask = u64(reinterpret<i32>(value) >> 31);
    return new u256(<u64>value, mask, mask, mask);
  }

  /**
 * Create 256-bit unsigned integer from generic type T
 * @param  value
 * @returns 256-bit unsigned integer
 */
  @inline
  static from<T>(value: T): u256 {
    if (value instanceof bool) return u256.fromU64(<u64>value);
    else if (value instanceof i8) return u256.fromI64(<i64>value);
    else if (value instanceof u8) return u256.fromU64(<u64>value);
    else if (value instanceof i16) return u256.fromI64(<i64>value);
    else if (value instanceof u16) return u256.fromU64(<u64>value);
    else if (value instanceof i32) return u256.fromI64(<i64>value);
    else if (value instanceof u32) return u256.fromU64(<u64>value);
    else if (value instanceof i64) return u256.fromI64(<i64>value);
    else if (value instanceof u64) return u256.fromU64(<u64>value);
    else if (value instanceof f32) return u256.fromF64(<f64>value);
    else if (value instanceof f64) return u256.fromF64(<f64>value);
    else if (value instanceof u128) return u256.fromU128(<u128>value);
    else if (value instanceof u256) return u256.fromU256(<u256>value);
    else if (value instanceof u8[]) return u256.fromBytes(<u8[]>value);
    else if (value instanceof Uint8Array) return u256.fromBytes(<Uint8Array>value);
    else throw new TypeError("Unsupported generic type");
  }

  // TODO
  // static fromString(str: string): u256

  constructor(
    public lo1: u64 = 0,
    public lo2: u64 = 0,
    public hi1: u64 = 0,
    public hi2: u64 = 0,
  ) {}

  @inline
  set(value: u256): this {
    this.lo1 = value.lo1;
    this.lo2 = value.lo2;
    this.hi1 = value.hi1;
    this.hi2 = value.hi2;
    return this;
  }

  @inline
  setU128(value: u128): this {
    this.lo1 = value.lo;
    this.lo2 = value.hi;
    this.hi1 = 0;
    this.hi2 = 0;
    return this;
  }

  @inline
  setI64(value: i64): this {
    var mask: u64 = value >> 63;
    this.lo1 = value;
    this.lo2 = mask;
    this.hi1 = mask;
    this.hi2 = mask;
    return this;
  }

  @inline
  setU64(value: u64): this {
    this.lo1 = value;
    this.lo2 = 0;
    this.hi1 = 0;
    this.hi2 = 0;
    return this;
  }

  @inline
  setI32(value: i32): this {
    var mask: u64 = value >> 63;
    this.lo1 = value;
    this.lo2 = mask;
    this.hi1 = mask;
    this.hi2 = mask;
    return this;
  }

  @inline
  setU32(value: u32): this {
    this.lo1 = value;
    this.lo2 = 0;
    this.hi1 = 0;
    this.hi2 = 0;
    return this;
  }

  @inline
  isZero(): bool {
    return !(this.lo1 | this.lo2 | this.hi1 | this.hi2);
  }

  @inline @operator.prefix('!')
  static isEmpty(value: u256): bool {
    return value.isZero();
  }

  @inline @operator.prefix('~')
  not(): u256 {
    return new u256(~this.lo1, ~this.lo2, ~this.hi1, ~this.hi2);
  }

  @inline @operator.prefix('+')
  pos(): u256 {
    return this;
  }

  @operator.prefix('-')
  neg(): u256 {
    var
      lo1 = ~this.lo1,
      lo2 = ~this.lo2,
      hi1 = ~this.hi1,
      hi2 = ~this.hi2;

    var lo1p = lo1 + 1;
    var lo2p = lo2 + u64(lo1p < lo1);
    var hi1p = hi1 + ((lo2 & ~lo2p) >> 63);
    var hi2p = hi2 + ((hi1 & ~hi1p) >> 63);

    return new u256(lo1p, lo2p, hi1p, hi2p);
  }

  @operator.prefix('++')
  preInc(): this {
    var
      lo1 = this.lo1,
      lo2 = this.lo2,
      hi1 = this.hi1,
      hi2 = this.hi2;

    var lo1p = lo1 + 1;
    var lo2p = lo2 + u64(lo1p < lo1);
    var hi1p = hi1 + ((lo2 & ~lo2p) >> 63);
    var hi2p = hi2 + ((hi1 & ~hi1p) >> 63);

    this.lo1 = lo1p;
    this.lo2 = lo2p;
    this.hi1 = hi1p;
    this.hi2 = hi2p;

    return this;
  }

  @operator.prefix('--')
  preDec(): this {
    var
      lo1 = this.lo1,
      lo2 = this.lo2,
      hi1 = this.hi1,
      hi2 = this.hi2;

    var lo1p = lo1 - 1;
    var lo2p = lo2 - u64(lo1p > lo1);
    var hi1p = hi1 - ((~lo2 & lo2p) >> 63);
    var hi2p = hi2 - ((~hi1 & hi1p) >> 63);

    this.lo1 = lo1p;
    this.lo2 = lo2p;
    this.hi1 = hi1p;
    this.hi2 = hi2p;

    return this;
  }

  @inline @operator.postfix('++')
  postInc(): u256 {
    return this.clone().preInc();
  }

  @inline @operator.postfix('--')
  postDec(): u256 {
    return this.clone().preDec();
  }

  @operator('+')
  static add(a: u256, b: u256): u256 {
    var
      lo1a = a.lo1,
      lo2a = a.lo2,
      hi1a = a.hi1,
      hi2a = a.hi2;

    var
      lo1b = b.lo1,
      lo2b = b.lo2,
      hi1b = b.hi1,
      hi2b = b.hi2;

    var lo1 = lo1a + lo1b;
    var cy  = u64(lo1 < lo1a);
    var lo2 = lo2a + lo2b + cy;
        // for a + b + c case we should calculate carry bit differently
        cy  = ((lo2a & lo2b) | ((lo2a | lo2b) & ~lo2)) >> 63;
    var hi1 = hi1a + hi1b + cy;
        cy  = ((hi1a & hi1b) | ((hi1a | hi1b) & ~hi1)) >> 63;
    var hi2 = hi2a + hi2b + cy;
    return new u256(lo1, lo2, hi1, hi2);
  }

  @operator('-')
  static sub(a: u256, b: u256): u256 {
    var
      lo1a = a.lo1,
      lo2a = a.lo2,
      hi1a = a.hi1,
      hi2a = a.hi2;

    var
      lo1b = b.lo1,
      lo2b = b.lo2,
      hi1b = b.hi1,
      hi2b = b.hi2;

    var lo1 = lo1a - lo1b;
    var cy  = u64(lo1 > lo1a);
    var lo2 = lo2a - lo2b - cy;
        // for a - b - c case we should calculate carry bit differently
        cy  = ((~lo2a & lo2b) | ((~lo2a | lo2b) & lo2)) >> 63;
    var hi1 = hi1a - hi1b - cy;
        cy  = ((~hi1a & hi1b) | ((~hi1a | hi1b) & hi1)) >> 63;
    var hi2 = hi2a - hi2b - cy;
    return new u256(lo1, lo2, hi1, hi2);
  }

  @inline @operator('|')
  static or(a: u256, b: u256): u256 {
    return new u256(a.lo1 | b.lo1, a.lo2 | b.lo2, a.hi1 | b.hi1, a.hi2 | b.hi2);
  }

  @inline @operator('^')
  static xor(a: u256, b: u256): u256 {
    return new u256(a.lo1 ^ b.lo1, a.lo2 ^ b.lo2, a.hi1 ^ b.hi1, a.hi2 ^ b.hi2);
  }

  @inline @operator('&')
  static and(a: u256, b: u256): u256 {
    return new u256(a.lo1 & b.lo1, a.lo2 & b.lo2, a.hi1 & b.hi1, a.hi2 & b.hi2);
  }

  @operator('>>')
  static shr(value: u256, shift: i32): u256 {
    shift &= 255;
    var off = shift as u64;
    if (shift <= 64) {
      if (shift == 0) return value;
      let hi2 =  value.hi2 >> off;
      let hi1 = (value.hi1 >> off) | (value.hi2 << 64 - off);
      let lo2 = (value.lo2 >> off) | (value.hi1 << 64 - off);
      let lo1 = (value.lo1 >> off) | (value.lo2 << 64 - off);
      return new u256(lo1, lo2, hi1, hi2);
    } else if (shift > 64 && shift <= 128) {
      let hi1 = value.hi2 >> 128 - off;
      return new u256(value.lo2, value.hi1, hi1);
    } else if (shift > 128 && shift <= 192) {
      let lo2 = value.hi2 >> 192 - off;
      return new u256(value.hi1, lo2);
    } else {
      return new u256(value.hi2 >> 256 - off);
    }
  }

  @inline @operator('>>>')
  static shr_u(value: u256, shift: i32): u256 {
    return u256.shr(value, shift);
  }

  @inline @operator('==')
  static eq(a: u256, b: u256): bool {
    return (
      a.lo1 == b.lo1 && a.lo2 == b.lo2 &&
      a.hi1 == b.hi1 && a.hi2 == b.hi2
    );
  }

  @inline @operator('!=')
  static ne(a: u256, b: u256): bool {
    return !u256.eq(a, b);
  }

  @operator('<')
  static lt(a: u256, b: u256): bool {
    var ah2 = a.hi2, ah1 = a.hi1,
        bh2 = b.hi2, bh1 = b.hi1,
        al2 = a.lo2, bl2 = b.lo2;
    if (ah2 == bh2) {
      if (ah1 == bh1) {
        return al2 == bl2 ? a.lo1 < b.lo1 : al2 < bl2
      } else {
        return ah1 < bh1;
      }
    } else {
      return ah2 < bh2;
    }
  }

  @inline @operator('>')
  static gt(a: u256, b: u256): bool {
    return b < a;
  }

  @inline @operator('<=')
  static le(a: u256, b: u256): bool {
    return !u256.gt(a, b);
  }

  @inline @operator('>=')
  static ge(a: u256, b: u256): bool {
    return !u256.lt(a, b);
  }

  // mul: u256 x u256 = u256
  @inline @operator('*')
  static mul(a: u256, b: u256): u256 {
    return __mul256(a.lo1, a.lo2, a.hi1, a.hi2, b.lo1, b.lo2, b.hi1, b.hi2)
  }

  @inline
  static popcnt(value: u256): i32 {
    var count = popcnt(value.lo1);
    if (value.lo2) count += popcnt(value.lo2);
    if (value.hi1) count += popcnt(value.hi1);
    if (value.hi2) count += popcnt(value.hi2);
    return <i32>count;
  }

  @inline
  static clz(value: u256): i32 {
         if (value.hi2) return <i32>(clz(value.hi2) + 0);
    else if (value.hi1) return <i32>(clz(value.hi1) + 64);
    else if (value.lo2) return <i32>(clz(value.lo2) + 128);
    else                return <i32>(clz(value.lo1) + 192);
  }

  @inline
  static ctz(value: u256): i32 {
         if (value.lo1) return <i32>(ctz(value.lo1) + 0);
    else if (value.lo2) return <i32>(ctz(value.lo2) + 64);
    else if (value.hi1) return <i32>(ctz(value.hi1) + 128);
    else                return <i32>(ctz(value.hi2) + 192);
  }

  /**
   * Convert to 128-bit signed integer
   * @return 256-bit signed integer
   */
  @inline
  toI128(): i128 {
    return new i128(
      this.lo1,
      (this.lo2 & 0x7FFFFFFFFFFFFFFF) |
      (this.hi2 & 0x8000000000000000)
    );
  }

  /**
   * Convert to 128-bit unsigned integer
   * @return 128-bit unsigned integer
   */
  @inline
  toU128(): u128 {
    return new u128(this.lo1, this.lo2);
  }

  /**
  * Convert to 256-bit unsigned integer
  * @returns 256-bit unsigned integer
  */
  @inline
  toU256(): this {
    return this;
  }

  /**
   * Convert to 64-bit signed integer
   * @return 64-bit signed integer
   */
  @inline
  toI64(): i64 {
    return <i64>(
      (this.lo1 & 0x7FFFFFFFFFFFFFFF) |
      (this.hi2 & 0x8000000000000000)
    );
  }

  /**
   * Convert to 64-bit unsigned integer
   * @return 64-bit unsigned integer
   */
  @inline
  toU64(): u64 {
    return this.lo1;
  }

  /**
   * Convert to 32-bit signed integer
   * @return 32-bit signed integer
   */
  @inline
  toI32(): i32 {
    return <i32>this.toI64();
  }

  /**
   * Convert to 32-bit unsigned integer
   * @return 32-bit unsigned integer
   */
  @inline
  toU32(): u32 {
    return <u32>this.lo1;
  }

  /**
   * Convert to 1-bit boolean
   * @return 1-bit boolean
   */
  @inline
  toBool(): bool {
    return <bool>(this.lo1 | this.lo2 | this.hi1 | this.hi2);
  }

  @inline
  private toArrayBufferLE(buffer: usize): void {
    store<u64>(buffer, this.lo1, 0 * sizeof<u64>());
    store<u64>(buffer, this.lo2, 1 * sizeof<u64>());
    store<u64>(buffer, this.hi1, 2 * sizeof<u64>());
    store<u64>(buffer, this.hi2, 3 * sizeof<u64>());
  }

  @inline
  private toArrayBufferBE(buffer: usize): void {
    store<u64>(buffer, bswap(this.hi2), 0 * sizeof<u64>());
    store<u64>(buffer, bswap(this.hi1), 1 * sizeof<u64>());
    store<u64>(buffer, bswap(this.lo2), 2 * sizeof<u64>());
    store<u64>(buffer, bswap(this.lo1), 3 * sizeof<u64>());
  }

  @inline
  private toArrayBuffer(buffer: usize, bigEndian: bool = false): void {
    if (bigEndian) {
      this.toArrayBufferBE(buffer);
    } else {
      this.toArrayBufferLE(buffer);
    }
  }

  /**
   * Convert to generic type `T`. Useful inside other generics methods
   * @param  T  is <bool | i8 | u8 | i16 | u16 | i32 | u32 | i64 | u64 | f32 | f64 | i128 | u128 | u256 | u8[] | Uint8Array | `StaticArray<u8>` | string>
   * @returns   type of `T`
   */
  @inline
  as<T>(): T {
    var dummy!: T;
         if (dummy instanceof bool)       return <T>this.toBool();
    else if (dummy instanceof i8)         return <T>this.toI64();
    else if (dummy instanceof u8)         return <T>this.toU64();
    else if (dummy instanceof i16)        return <T>this.toI64();
    else if (dummy instanceof u16)        return <T>this.toU64();
    else if (dummy instanceof i32)        return <T>this.toI64();
    else if (dummy instanceof i64)        return <T>this.toI64();
    else if (dummy instanceof u32)        return <T>this.toU64();
    else if (dummy instanceof u64)        return <T>this.toU64();
    // else if (dummy instanceof f32)        return <T>this.toF64();
    // else if (dummy instanceof f64)        return <T>this.toF64();
    else if (dummy instanceof i128)       return <T>this.toI128();
    else if (dummy instanceof u128)       return <T>this.toU128();
    else if (dummy instanceof u256)       return <T>this.toU256();
    else if (dummy instanceof u8[])       return <T>this.toBytes();
    else if (dummy instanceof Uint8Array) return <T>this.toUint8Array();
    else if (dummy instanceof StaticArray<u8>) return <T>this.toStaticBytes();
    else if (dummy instanceof String)     return <T>this.toString();
    else throw new TypeError('Unsupported generic type');
  }

  /**
   * Convert to byte array
   * @param bigEndian Little or Big Endian? Default: false
   * @returns  Array of bytes
   */
  @inline
  toBytes(bigEndian: bool = false): u8[] {
    var result = new Array<u8>(32);
    this.toArrayBuffer(result.dataStart, bigEndian);
    return result;
  }

  /**
   * Convert to byte static array
   * @param bigEndian Little or Big Endian? Default: false
   * @returns  StaticArray of bytes
   */
   @inline
   toStaticBytes(bigEndian: bool = false): StaticArray<u8> {
     var result = new StaticArray<u8>(32);
     this.toArrayBuffer(changetype<usize>(result), bigEndian);
     return result;
   }

  /**
   * Convert to Uint8Array
   * @param bigEndian Little or Big Endian? Default: false
   * @returns  Uint8Array
   */
  @inline
  toUint8Array(bigEndian: bool = false): Uint8Array {
    var result = new Uint8Array(32);
    this.toArrayBuffer(result.dataStart, bigEndian);
    return result;
  }

  clone(): u256 {
    return new u256(this.lo1, this.lo2, this.hi1, this.hi2);
  }

  toString(radix: i32 = 10): string {
    assert(radix == 10 || radix == 16, 'radix argument must be between 10 or 16');
    if (this.isZero()) return '0';

    var result = '';
    if (radix == 16) {
      let shift: i32 = 252 - (u256.clz(this) & ~3);
      while (shift >= 0) {
        // @ts-ignore
        result += HEX_CHARS.charAt(<i32>((this >> shift).lo1 & 15));
        shift -= 4;
      }
      return result;
    }
    return u256toDecimalString(this);
  }
}
