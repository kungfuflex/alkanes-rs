import { u128Safe as u128 } from './u128';
import { u128 as U128 } from '../u128';
import { u256 as U256 } from '../u256';

class u256 extends U256 {
  @inline static get Zero(): u256 {
    return new u256();
  }
  @inline static get One(): u256 {
    return new u256(1);
  }
  @inline static get Min(): u256 {
    return new u256();
  }
  @inline static get Max(): u256 {
    return new u256(-1, -1, -1, -1);
  }

  @inline
  static fromU256(value: u256): u256 {
    return changetype<u256>(U256.fromU256(value));
  }

  @inline
  static fromU128(value: u128): u256 {
    return changetype<u256>(U256.fromU128(value));
  }

  @inline
  static fromI64(value: i64): u256 {
    return changetype<u256>(U256.fromI64(value));
  }

  @inline
  static fromU64(value: u64): u256 {
    return changetype<u256>(U256.fromU64(value));
  }

  @inline
  static fromF64(value: f64): u256 {
    return changetype<u256>(U256.fromF64(value));
  }

  @inline
  static fromF32(value: f32): u256 {
    return changetype<u256>(U256.fromF32(value));
  }

  @inline
  static fromI32(value: i32): u256 {
    return changetype<u256>(U256.fromI32(value));
  }

  @inline
  static fromU32(value: u32): u256 {
    return changetype<u256>(U256.fromU32(value));
  }

  @inline
  static fromBytes<T>(array: T, bigEndian: bool = false): u256 {
    return changetype<u256>(U256.fromBytes<T>(array, bigEndian));
  }

  @inline
  static fromBytesLE(array: u8[]): u256 {
    return changetype<u256>(U256.fromBytesLE(array));
  }

  @inline
  static fromBytesBE(array: u8[]): u256 {
    return changetype<u256>(U256.fromBytesBE(array));
  }

  @inline
  static fromUint8ArrayLE(array: Uint8Array): u256 {
    return changetype<u256>(U256.fromUint8ArrayLE(array));
  }

  @inline
  static fromUint8ArrayBE(array: Uint8Array): u256 {
    return changetype<u256>(U256.fromUint8ArrayBE(array));
  }

  @inline
  static from<T>(value: T): u256 {
         if (value instanceof bool)       return u256.fromU64(<u64>value);
    else if (value instanceof i8)         return u256.fromI64(<i64>value);
    else if (value instanceof u8)         return u256.fromU64(<u64>value);
    else if (value instanceof i16)        return u256.fromI64(<i64>value);
    else if (value instanceof u16)        return u256.fromU64(<u64>value);
    else if (value instanceof i32)        return u256.fromI64(<i64>value);
    else if (value instanceof u32)        return u256.fromU64(<u64>value);
    else if (value instanceof i64)        return u256.fromI64(<i64>value);
    else if (value instanceof u64)        return u256.fromU64(<u64>value);
    else if (value instanceof f32)        return u256.fromF64(<f64>value);
    else if (value instanceof f64)        return u256.fromF64(<f64>value);
    else if (value instanceof u128)       return u256.fromU128(<u128>value);
    else if (value instanceof U128)       return u256.fromU128(<U128>value);
    else if (value instanceof U256)       return u256.fromU256(<U256>value);
    else if (value instanceof u256)       return u256.fromU256(<u256>value);
    else if (value instanceof u8[])       return u256.fromBytes(<u8[]>value);
    else if (value instanceof Uint8Array) return u256.fromBytes(<Uint8Array>value);
    else throw new TypeError("Unsupported generic type");
  }

  @operator("+")
  static add(a: u256, b: u256): u256 {
    var lo1a = a.lo1,
        lo2a = a.lo2,
        hi1a = a.hi1,
        hi2a = a.hi2;

    var lo1b = b.lo1,
        lo2b = b.lo2,
        hi1b = b.hi1,
        hi2b = b.hi2;

    // Addition for the lowest segment
    var lo1 = lo1a + lo1b;
    var cy = u64(lo1 < lo1a); // Detect carry

    // Addition for the second lowest segment with carry
    var lo2 = lo2a + lo2b + cy;
    cy = u64(lo2 < lo2a || (cy == 1 && lo2 == lo2a)); // Update carry

    // Addition for the second highest segment with carry
    var hi1 = hi1a + hi1b + cy;
    cy = u64(hi1 < hi1a || (cy == 1 && hi1 == hi1a)); // Update carry

    // Addition for the highest segment with carry
    var hi2 = hi2a + hi2b + cy;

    // Overflow detection after adding carry to the highest segment
    // In a 'safe' implementation, an overflow can be detected if the final carry would exceed the bounds of u256,
    // which means an addition that causes the highest segment to overflow.
    // However, standard unsigned integer behavior would wrap around, so this step depends on the intended behavior:
    if (hi2 < hi2a || (cy == 1 && hi2 == hi2a)) {
        throw new RangeError("Overflow during addition");
    }

    return new u256(lo1, lo2, hi1, hi2);
  }

  @operator("-")
  static sub(a: u256, b: u256): u256 {
    if (a < b) throw new RangeError("Underflow during subtraction");
    return changetype<u256>(U256.sub(changetype<U256>(a), changetype<U256>(b)));
  }
}

export { u256 as u256Safe };
