[![NPM registry](https://img.shields.io/npm/v/as-bignum.svg?style=for-the-badge)](https://www.npmjs.com/package/as-bignum)[![Build Status](https://img.shields.io/travis/com/MaxGraey/as-bignum/master?style=for-the-badge)](https://travis-ci.com/MaxGraey/as-bignum)[![NPM license](https://img.shields.io/badge/license-Apache%202.0-ba68c8.svg?style=for-the-badge)](LICENSE.md)

## WebAssembly fixed length big numbers written on [AssemblyScript](https://github.com/AssemblyScript/assemblyscript)

### Status: Work in progress

Provide wide numeric types such as `u128`, `u256`, `i128`, `i256` and fixed points and also its arithmetic operations.

Namespace `safe` contain equivalents with overflow/underflow traps.

All kind of types pretty useful for economical and cryptographic usages and provide deterministic behavior.

### Install

> yarn add as-bignum

or

> npm i as-bignum

### Usage via AssemblyScript

```ts
import { u128 } from "as-bignum/assembly";
// Before 0.20.x
// import { u128 } from "as-bignum";

declare function logF64(value: f64): void;
declare function logU128(hi: u64, lo: u64): void;

var a = u128.One;
var b = u128.from(-32); // same as u128.from<i32>(-32)
var c = new u128(0x1, -0xF);
var d = u128.from(0x0123456789ABCDEF); // same as u128.from<i64>(0x0123456789ABCDEF)
var e = u128.from('0x0123456789ABCDEF01234567');
var f = u128.fromString('11100010101100101', 2); // same as u128.from('0b11100010101100101')
var r = d / c + (b << 5) + e;

logF64(r.as<f64>());
logU128(r.hi, r.lo);
```

### Usage via JavaScript/Typescript

```ts
TODO
```

### List of types

- [x] [`u128`](https://github.com/MaxGraey/as-bignum/blob/master/assembly/integer/u128.ts) unsigned type (tested)
- [ ] [`u256`](https://github.com/MaxGraey/as-bignum/blob/master/assembly/integer/u256.ts) unsigned type (very basic)
- [ ] `i128` signed type
- [ ] `i256` signed type

---

- [x] [`safe.u128`](https://github.com/MaxGraey/as-bignum/blob/master/assembly/integer/safe/u128.ts) unsigned type (tested)

- [ ] `safe.u256` unsigned type
- [ ] `safe.i128` signed type
- [ ] `safe.i256` signed type

---

- [ ] [`fp128<Q>`](https://github.com/MaxGraey/as-bignum/blob/master/assembly/fixed/fp128.ts) generic fixed point signed type٭ (very basic for now)
- [ ] `fp256<Q>` generic fixed point signed type٭

---

- [ ] `safe.fp128<Q>` generic fixed point signed type٭
- [ ] `safe.fp256<Q>` generic fixed point signed type٭

٭ _typename_ `Q` _is a type representing count of fractional bits_
