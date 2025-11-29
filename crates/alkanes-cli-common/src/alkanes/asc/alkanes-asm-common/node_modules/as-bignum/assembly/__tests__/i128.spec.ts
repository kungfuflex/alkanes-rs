import { i128 } from '../integer/i128';
import { arrayToUint8Array } from "./utils";

// describe("String Conversion", () => {
//   // TODO: tests
// });

describe("Buffer Conversion", () => {
  it("Should convert from bytes Little Endian 1", () => {
    var arr: u8[] = [
      0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88,
      0x99, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x12
    ];
    expect(i128.fromBytes(arr))
      .toStrictEqual(new i128(0x8877665544332211, 0x12FFEEDDCCBBAA99));
  });

  it("Should convert from bytes Little Endian 2", () => {
    var arr = arrayToUint8Array([
      0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x00,
      0x00, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11
    ]);
    expect(i128.fromBytes(arr))
      .toStrictEqual(new i128(0x0077665544332211, 0x1122334455667700));
  });

  it("Should convert from bytes Big Endian 1", () => {
    var arr: u8[] = [
      0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88,
      0x99, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x12
    ];
    expect(i128.fromBytes(arr, true))
      .toStrictEqual(new i128(0x99AABBCCDDEEFF12, 0x1122334455667788));
  });

  it("Should convert from bytes Big Endian 2", () => {
    var arr = arrayToUint8Array([
      0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88,
      0x99, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x12
    ]);
    expect(i128.fromBytes(arr, true))
      .toStrictEqual(new i128(0x99AABBCCDDEEFF12, 0x1122334455667788));
  });
});

describe("Basic Type Conversion", () => {
  // TODO: tests
});

describe("Basic Operations", () => {
  it("Should number is zero 1", () => {
    let a = i128.Zero;
    expect(a.isZero()).toBe(true);
  });

  it("Should number is zero 2", () => {
    let a = i128.One;
    expect(!a.isZero()).toBe(true);
  });

  it("Should number binary not", () => {
    let a = new i128(1, 2);
    expect(~a).toStrictEqual(new i128(~1, ~2));
  });

  it("Should number unary positive", () => {
    let a = new i128(1, 2);
    expect(+a).toStrictEqual(a);
  });

  it("Should number non empty", () => {
    let a = i128.One;
    expect(!!a).toBe(true);
  });

  it("Should number is empty 1", () => {
    let a = i128.Zero;
    expect(!a).toBe(true);
  });

  it("Should neg(1) == -1", () => {
    let one = i128.One;
    let neg = one.neg();
    expect(<u64>neg.lo).toBe(<u64>-1);
    expect(<u64>neg.hi).toBe(<u64>-1);
  });

  it("Should abs(-1) == 1", () => {
    let one = i128.One;
    let neg = -one;
    expect(i128.abs(neg)).toStrictEqual(one);
  });
});

// describe("Exponention Operation", () => {
//   // TODO: tests
// });

// describe("Binary Operation Count", () => {
//   // TODO: tests
// });

// describe("Division And Mod Operation", () => {
//   // TODO: tests
// });

// describe("Throwable", () => {
//   // TODO: tests
// });
