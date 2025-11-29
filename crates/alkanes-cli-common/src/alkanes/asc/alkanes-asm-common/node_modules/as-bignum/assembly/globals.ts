import { u256 } from './integer';
import { u128 } from './integer/u128';

// used for returning quotient and reminder from __divmod128
@lazy export var __divmod_quot_hi: u64 = 0;
@lazy export var __divmod_rem_lo:  u64 = 0;
@lazy export var __divmod_rem_hi:  u64 = 0;

// used for returning low and high part of __mulq64, __multi3 etc
@lazy export var __res128_hi: u64 = 0;
// used for returning 0 or 1
@lazy export var __carry: u64 = 0;

/**
 * Convert 128-bit unsigned integer to 64-bit float
 * @param  lo lower  64-bit part of unsigned 128-bit integer
 * @param  hi higher 64-bit part of unsigned 128-bit integer
 * @return    64-bit float result
 */
// @ts-ignore: decorator
@global
export function __floatuntidf(lo: u64, hi: u64): f64 {
  // __floatuntidf ported from LLVM sources
  if (!(lo | hi)) return 0.0;

  var v  = new u128(lo, hi);
  var sd = 128 - __clz128(lo, hi);
  var e  = sd - 1;

  if (sd > 53) {
    if (sd != 55) {
      if (sd == 54) {
        v = u128.shl(v, 1);
      } else {
        v = u128.or(
          u128.shr(v, sd - 55),
          u128.fromBool(u128.and(v, u128.shr(u128.Max, 128 + 55 - sd)).toBool())
        );
      }
    }

    v.lo |= (v.lo & 4) >> 2;
    v.preInc();

    v = u128.shr(v, 2);

    if (v.lo & (1 << 53)) {
      v = u128.shr(v, 1);
      ++e;
    }

  } else {
    v = u128.shl(v, 53 - sd);
  }

  var w: u64 = u128.shr(v, 32).lo & 0x000FFFFF;
  var u: u64 = <u64>(((e + 1023) << 20) | w) << 32;
  return reinterpret<f64>(u | (v.lo & 0xFFFFFFFF));
}

// @ts-ignore: decorator
@global
export function __umulh64(a: u64, b: u64): u64 {
  var u = a & 0xFFFFFFFF; a >>= 32;
  var v = b & 0xFFFFFFFF; b >>= 32;

  var uv = u * v;
  var uv = a * v + (uv >> 32);
  var w0 = u * b + (uv & 0xFFFFFFFF);
  return a * b + (uv >> 32) + (w0 >> 32);
}

// @ts-ignore: decorator
@global
export function __umulq64(a: u64, b: u64): u64 {
  var u = a & 0xFFFFFFFF; a >>= 32;
  var v = b & 0xFFFFFFFF; b >>= 32;

  var uv = u * v;
  var w0 = uv & 0xFFFFFFFF;
  uv = a * v + (uv >> 32);
  var w1 = uv >> 32;
  uv = u * b + (uv & 0xFFFFFFFF);

  __res128_hi = a * b + w1 + (uv >> 32);
  return (uv << 32) | w0;
}

// __umul64Hop computes (hi * 2^64 + lo) = z + (x * y)
// @ts-ignore: decorator
@inline
export function __umul64Hop(z: u64, x: u64, y: u64): u64 {
  var lo = __umulq64(x, y);
  lo = __uadd64(lo, z);
  var hi = __res128_hi +__carry;
  __res128_hi = hi;
  return lo
}

// __umul64Step computes (hi * 2^64 + lo) = z + (x * y) + carry.
// @ts-ignore: decorator
@inline
export function __umul64Step(z: u64, x: u64, y: u64, carry: u64): u64 {
  var lo = __umulq64(x, y)
  lo = __uadd64(lo, carry);
  var hi = __uadd64(__res128_hi, 0, __carry);
  lo = __uadd64(lo, z);
  hi += __carry;
  __res128_hi = hi;
  return lo
}

// __uadd64 returns the sum with carry of x, y and carry: sum = x + y + carry.
// The carry input must be 0 or 1; otherwise the behavior is undefined.
// The carryOut output is guaranteed to be 0 or 1.
// @ts-ignore: decorator
@inline
export function __uadd64(x: u64, y: u64, carry: u64 = 0): u64 {
  var sum = x + y + carry
  // // The sum will overflow if both top bits are set (x & y) or if one of them
  // // is (x | y), and a carry from the lower place happened. If such a carry
  // // happens, the top bit will be 1 + 0 + 1 = 0 (& ~sum).
  __carry = ((x & y) | ((x | y) & ~sum)) >>> 63
  return sum;

}

// u256 * u256 => u256 implemented from https://github.com/holiman/uint256
// @ts-ignore: decorator
@global
export function __mul256(x0: u64, x1: u64, x2: u64, x3: u64, y0: u64, y1: u64, y2: u64, y3: u64): u256 {
  var lo1 = __umulq64(x0, y0);
  var res1 = __umul64Hop(__res128_hi, x1, y0);
  var res2 = __umul64Hop(__res128_hi, x2, y0);
  var res3 = x3 * y0 + __res128_hi;

  var lo2 = __umul64Hop(res1, x0, y1);
  res2 = __umul64Step(res2, x1, y1, __res128_hi);
  res3 += x2 * y1 + __res128_hi;

  var hi1 = __umul64Hop(res2, x0, y2);
  res3 += x1 * y2 + __res128_hi

  var hi2 = __umul64Hop(res3, x0, y3);

  return new u256(lo1, lo2, hi1, hi2);
}

// @ts-ignore: decorator
@global
export function __multi3(al: u64, ah: u64, bl: u64, bh: u64): u64 {
  var u = al, v = bl;
  var w: u64, k: u64;

  var u1 = u & 0xFFFFFFFF; u >>= 32;
  var v1 = v & 0xFFFFFFFF; v >>= 32;
  var t  = u1 * v1;
  var w1 = t & 0xFFFFFFFF;

  t = u * v1 + (t >> 32);
  k = t & 0xFFFFFFFF;
  w = t >> 32;
  t = u1 * v + k;

  var lo  = (t << 32) | w1;
  var hi  = u  * v + w;
      hi += ah * bl;
      hi += al * bh;
      hi += t >> 32;

  __res128_hi = hi;
  return lo;
}

// @ts-ignore: decorator
@global
export function __floatuntfdi(value: f64): u64 {
  var u = reinterpret<u64>(value);

  // if (value < -1.7014118346046e38) { // -(2^127-1)
  if (value < reinterpret<f64>(0xC7F0000000000000)) { // -(2^128-1)
    // __float_u128_hi = <u64>-1; // for i128
    __res128_hi = 0;
    // overflow negative
    return 0;
    // } else if (value < -9.2233720368547e18) { // -2^63-1 // for i128
  } else if (value < reinterpret<f64>(0xC3F0000000000000)) { // // -(2^64-1)
    let lo: u64, hi: u64, m: u64;

    m = (u & 0x000FFFFFFFFFFFFF) | (1 << 52);
    u = (u & 0x7FFFFFFFFFFFFFFF) >> 52;

    u -= 1075;
    if (u > 64) {
      lo = 0;
      hi = m << (u - 64);
    } else {
      lo = m << u;
      hi = m >> (64 - u);
    }
    __res128_hi = ~hi;
    return ~lo;
    // } else if (value < 9.2233720368547e18) { // 2^63-1 // for i128
  } else if (value < reinterpret<f64>(0x43F0000000000000)) { // 2^64-1
    // __float_u128_hi = (value < 0) ? -1 : 0; // for int
    __res128_hi = 0;
    // fit in a u64
    return <u64>value;
    // } else if (value < 1.7014118346046e38) {
  } else if (value < reinterpret<f64>(0x47F0000000000000)) { // 2^128-1
    let lo: u64, hi: u64, m: u64;

    m = (u & 0x000FFFFFFFFFFFFF) | (1 << 52);
    u = (u & 0x7FFFFFFFFFFFFFFF) >> 52;
    u -= 1075;
    if (u > 64) {
      lo = 0;
      hi = m << (u - 64);
    } else {
      lo = m << u;
      hi = m >> (64 - u);
    }
    __res128_hi = hi;
    return lo;
  } else {
    // overflow positive
    __res128_hi = <u64>-1; // 0x7FFFFFFFFFFFFFFF for i128
    return <u64>-1;
  }
}

// @ts-ignore: decorator
@global @inline
export function __clz128(lo: u64, hi: u64): i32 {
  var mask: u64 = <i64>(hi ^ (hi - 1)) >> 63;
  return <i32>clz((hi & ~mask) | (lo & mask)) + (<i32>mask & 64);
}

// @ts-ignore: decorator
@global @inline
export function __ctz128(lo: u64, hi: u64): i32 {
  var mask: u64 = <i64>(lo ^ (lo - 1)) >> 63;
  return <i32>ctz((hi & mask) | (lo & ~mask)) + (<i32>mask & 64);
}

// @ts-ignore: decorator
@global
export function __udivmod128(alo: u64, ahi: u64, blo: u64, bhi: u64): u64 {
  var bzn = __clz128(blo, bhi); // N

  // b == 0
  if (bzn == 128) {
    throw new RangeError("Division by zero"); // division by zero
  }

  // var azn = __clz128(alo, ahi); // M
  var btz = __ctz128(blo, bhi); // N

  // a == 0
  if (!(alo | ahi)) {
    __divmod_quot_hi = 0;
    __divmod_rem_lo  = 0;
    __divmod_rem_hi  = 0;
    return 0;
  }

  // a / 1
  if (bzn == 127) {
    __divmod_quot_hi = ahi;
    __divmod_rem_lo  = 0;
    __divmod_rem_hi  = 0;
    return alo;
  }

  // a == b
  if (alo == blo && ahi == bhi) {
    __divmod_quot_hi = 0;
    __divmod_rem_lo  = 0;
    __divmod_rem_hi  = 0;
    return 1;
  }

  // if (btz + bzn == 127) {
  //   // TODO
  //   // __divmod_quot = a >> btz
  //   // b++
  //   // __divmod_rem = a & b
  //   return;
  // }

  if (!(ahi | bhi)) {
    __divmod_quot_hi = 0;
    __divmod_rem_hi  = 0;
    // if `b.lo` is power of two
    if (!(blo & (blo - 1))) {
      __divmod_rem_lo = alo & (blo - 1);
      return alo >> btz;
    } else {
      let dlo = alo / blo;
      __divmod_rem_lo = alo - dlo * blo;
      return dlo;
    }
  }

  // if b.lo == 0 and `b.hi` is power of two
  // if (!blo && !(bhi & (bhi - 1))) {
  //   __divmod_rem = 0;

  //   // TODO

  //   return 0;
  // }

  // var diff: i64 = ahi - bhi;
  // var cmp = <i32>(diff != 0 ? diff : alo - blo); // TODO optimize this

  // if (cmp <= 0) {
  //   __divmod_quot_hi = 0;
  //   __divmod_rem     = 0;
  //   return u64(cmp == 0);
  // }

  // if (bzn - azn <= 5) {
  //   // TODO
  //   // fast path
  //   return __udivmod128core(alo, ahi, blo, bhi);
  // }
  return __udivmod128core(alo, ahi, blo, bhi);
}

function __udivmod128core(alo: u64, ahi: u64, blo: u64, bhi: u64): u64 {
  var a = new u128(alo, ahi);
  var b = new u128(blo, bhi);
  // get leading zeros for left alignment
  var alz = __clz128(alo, ahi);
  var blz = __clz128(blo, bhi);
  var off = blz - alz;
  var nb  = b << off;
  var q = u128.Zero;
  var n = a.clone();

  // create a mask with the length of b
  var mask = u128.One;
  mask <<= 128 - blz;
  --mask;
  mask <<= off;

  var i = 0;
  while (n >= b) {
    ++i;
    q <<= 1;
    if ((n & mask) >= nb) {
      ++q;
      n -= nb;
    }

    mask |= mask >> 1;
    nb >>= 1;
  }
  q <<= (blz - alz - i + 1);

  __divmod_quot_hi = q.hi;
  __divmod_rem_lo  = n.lo;
  __divmod_rem_hi  = n.hi;
  return q.lo;
}

// @ts-ignore: decorator
@global
export function __udivmod128_10(lo: u64, hi: u64): u64 {
  if (!hi) {
    __divmod_quot_hi = 0;
    if (lo < 10) {
      __divmod_rem_lo = 0;
      __divmod_rem_hi = 0;
      return 0;
    } else {
      let qlo = lo / 10;
      __divmod_rem_lo = lo - qlo * 10;
      __divmod_rem_hi = 0;
      return qlo;
    }
  }

  var q: u128, r: u128;
  var n = new u128(lo, hi);

  q  = n >> 1;
  q += n >> 2;
  q += q >> 4;
  q += q >> 8;
  q += q >> 16;
  q += q >> 32;
  q += u128.fromU64(q.hi); // q >> 64
  q >>= 3;
  r = n - (((q << 2) + q) << 1);
  n = q + u128.fromBool(r.lo > 9);

  __divmod_quot_hi = n.hi;
  __divmod_rem_lo  = r.lo;
  __divmod_rem_hi  = r.hi;
  return n.lo;
}
