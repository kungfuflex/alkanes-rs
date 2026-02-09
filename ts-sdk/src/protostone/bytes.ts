// Binary encoding utilities for Protostone/Runestone encoding
// Provides LEB128 VarInt encoding, pack/unpack for 15-byte segments, and encipher/decipher

import { SeekBuffer } from './seekbuffer';

export type AlkaneId = {
  block: bigint;
  tx: bigint;
};

export type AlkaneTransfer = {
  id: AlkaneId;
  value: bigint;
};

// --- LEB128 VarInt encoding/decoding ---

export function encodeVarInt(value: bigint): Buffer {
  const v: number[] = [];
  while (value >> 7n > 0n) {
    v.push(Number(value & 0xffn) | 0b1000_0000);
    value = BigInt(value >> 7n);
  }
  v.push(Number(value & 0xffn));
  return Buffer.from(v);
}

export function decodeVarInt(seekBuffer: SeekBuffer): bigint {
  try {
    return tryDecodeVarInt(seekBuffer);
  } catch (e) {
    return BigInt(-1);
  }
}

export function tryDecodeVarInt(seekBuffer: SeekBuffer): bigint {
  let result = BigInt(0);
  for (let i = 0; i <= 18; i++) {
    const byte = seekBuffer.readUInt8();
    if (byte === undefined) {
      throw new Error('Unterminated');
    }

    const value = BigInt(byte) & 0b0111_1111n;

    if (i === 18 && (value & 0b0111_1100n) !== 0n) {
      throw new Error('Overflow');
    }

    result = BigInt(result | (value << BigInt(7 * i)));

    if ((byte & 0b1000_0000) === 0) {
      return result;
    }
  }

  throw new Error('Overlong');
}

// --- Encipher/decipher (LEB128 array <-> Buffer) ---

export function encipher(values: bigint[]): Buffer {
  return Buffer.concat(values.map((v) => encodeVarInt(v))) as any;
}

export function decipher(values: Buffer): bigint[] {
  const seekBuffer = new SeekBuffer(values);
  let v = null;
  const result: bigint[] = [];
  while ((v = decodeVarInt(seekBuffer)) !== BigInt(-1)) {
    result.push(v);
  }
  return result;
}

// --- Pack/unpack (15-byte segment encoding for Runes protocol compat) ---
// uint128s -> leb128 max needs 19 bytes (128/7 = 18.3)
// Runes cenotaphs if >18 bytes per leb128, so we use 15-byte segments
// to keep upper 2 bits clear and avoid cenotaph

export function leftPad15(v: string): string {
  if (v.length > 30) throw Error('varint in encoding cannot exceed 15 bytes');
  return '0'.repeat(30 - v.length) + v;
}

export function leftPadByte(v: string): string {
  if (v.length % 2) {
    return '0' + v;
  }
  return v;
}

export function rightPadByte(v: string): string {
  if (v.length % 2) {
    return v + '0';
  }
  return v;
}

export function leftPad16(v: string): string {
  if (v.length > 32) throw Error('value exceeds 16 bytes');
  return '0'.repeat(32 - v.length) + v;
}

export function pack(v: bigint[]): Buffer {
  return Buffer.concat(
    v.map((segment) => {
      return Buffer.from(
        leftPad15(
          Buffer.from(
            Array.from(
              Buffer.from(leftPadByte(segment.toString(16)), 'hex')
            ).reverse()
          ).toString('hex')
        ),
        'hex'
      );
    })
  ) as any;
}

export function unpack(v: Buffer): bigint[] {
  return Array.from(v)
    .reduce((r: number[][], v: number, i) => {
      if (i % 15 === 0) {
        r.push([]);
      }
      r[r.length - 1].push(v);
      return r;
    }, [])
    .map((v) => BigInt('0x' + Buffer.from(v.reverse()).toString('hex')));
}

export function decipherPacked(v: bigint[]): bigint[] {
  return decipher(
    Buffer.concat(
      v.map((v) =>
        Buffer.from(
          Array.from(
            Buffer.from(leftPadByte(v.toString(16)), 'hex')
          ).reverse()
        )
      )
    ) as any
  );
}

// --- Buffer conversion helpers ---

export const toBuffer = (v: number | bigint): Buffer => {
  return Buffer.from(
    Array.from(Buffer.from(leftPad16(v.toString(16)), 'hex')).reverse(),
  );
};

export const fromBuffer = (v: Buffer): bigint => {
  return BigInt('0x' + Buffer.from(Array.from(v).reverse()).toString('hex'));
};
