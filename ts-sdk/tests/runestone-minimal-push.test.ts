/**
 * The runestone script must carry its payload as DATA pushes only.
 *
 * bitcoinjs `script.compile` applies BIP62 minimal pushes: a ONE-byte chunk holding 1..16 (or 0x81)
 * becomes OP_1..OP_16 (OP_1NEGATE). A runestone payload is split into 520-byte pushes, so a
 * 520·k + 1 byte payload ending in such a byte used to end in an OPCODE, and the runestone
 * decoder calls any opcode after the magic a cenotaph (ordinals runestone.rs: Flaw::Opcode): the
 * indexer runs nothing. Found by the subfrost wallet-core golden replay (2026-10-10).
 */
import { describe, expect, it } from 'vitest';
import * as bitcoin from 'bitcoinjs-lib';
import { encodeRunestoneProtostone } from '../src/protostone/runestone';

/** A runestone of `n` rune edicts; the last edict's output (one byte) ends the payload. */
function encode(n: number, firstAmount: bigint, lastOutput: number): Buffer {
  const edicts = Array.from({ length: n }, (_, i) => ({
    id: { block: 840000n + BigInt(i), tx: 1n },
    amount: i === 0 ? firstAmount : 1n,
    output: i === n - 1 ? lastOutput : 0,
  }));
  return encodeRunestoneProtostone({ edicts }).encodedRunestone;
}

/**
 * The data chunks after `OP_RETURN OP_13`, read straight off the bytes. Not bitcoinjs
 * `script.decompile`: it ALSO normalises a one-byte push of 1..16 into an opcode number, so it
 * cannot tell the fixed script from the broken one.
 */
function chunks(script: Buffer): Array<Buffer | number> {
  const out: Array<Buffer | number> = [];
  let i = 2;
  while (i < script.length) {
    const op = script[i++];
    let n: number;
    if (op >= 1 && op <= 75) n = op;
    else if (op === 0x4c) n = script[i++];
    else if (op === 0x4d) { n = script[i] | (script[i + 1] << 8); i += 2; }
    else { out.push(op); continue; }
    out.push(script.subarray(i, i + n));
    i += n;
  }
  return out;
}
/** What the pre-fix encoder produced: the same chunks through `bitcoin.script.compile`. */
const viaBitcoinjs = (script: Buffer) => Buffer.from(bitcoin.script.compile([0x6a, 0x5d, ...chunks(script)]));
const payloadLength = (script: Buffer) =>
  chunks(script).reduce<number>((sum, c) => sum + (Buffer.isBuffer(c) ? c.length : 1), 0);

describe('runestone script pushes', () => {
  it('🔴 a 521-byte payload ending in 1..16 stays a data push (was OP_n → cenotaph)', () => {
    let found: Buffer | undefined;
    search: for (let n = 60; n < 140; n++) {
      for (let bytes = 1; bytes <= 18; bytes++) {
        const script = encode(n, 1n << BigInt(7 * (bytes - 1)), 7);
        if (payloadLength(script) === 521) {
          found = script;
          break search;
        }
      }
    }
    expect(found, 'the search must reach a 521-byte payload').toBeDefined();
    const script = found!;
    // Last chunk: one byte of DATA (push length 0x01, then 0x07), never OP_7 (0x57).
    expect(script.subarray(-2)).toEqual(Buffer.from([0x01, 0x07]));
    expect(chunks(script).every((c) => Buffer.isBuffer(c))).toBe(true);
    // Red: the old bitcoinjs compile of the same chunks ends in the opcode.
    expect(viaBitcoinjs(script).subarray(-1)).toEqual(Buffer.from([0x57]));
  });

  it('every other shape is byte-identical to the previous encoder', () => {
    let compared = 0;
    for (let n = 1; n < 140; n += 3) {
      for (const amount of [1n, 300n, 1n << 40n, (1n << 100n) - 1n]) {
        for (const out of [0, 5, 200]) {
          const script = encode(n, amount, out);
          const len = payloadLength(script);
          if (len % 520 === 1 && ((out >= 1 && out <= 16) || out === 0x81)) continue;
          expect(script.equals(viaBitcoinjs(script)), `n=${n} amount=${amount} out=${out}`).toBe(true);
          compared++;
        }
      }
    }
    expect(compared).toBeGreaterThan(500);
  });
});
