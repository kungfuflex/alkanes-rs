// RunestoneProtostoneUpgrade - Encode Runestones with embedded Protostones
// Forked from @magiceden-oss/runestone-lib/src/runestone with protostone extensions
// Uses bitcoinjs-lib for script compilation (already a ts-sdk dependency)

import * as bitcoin from 'bitcoinjs-lib';
import { ProtoruneRuneId } from './protoruneruneid';
import { ProtoruneEdict } from './protoruneedict';
import { Tag as ProtoTag } from './tag';
import { u128, u32, u64, u8 } from './integer';
import { Option, Some, None } from './monads';
import { ProtoStone } from './protostone';
import { unpack, encipher } from './bytes';

// Runestone constants
const OP_RETURN = 0x6a;
const MAGIC_NUMBER = 0x5d; // OP_13
const MAX_SCRIPT_ELEMENT_SIZE = 520;

// Standard Rune tags (from Runes protocol, not Protostone tags)
const RuneTag = {
  BODY: 0n,
  FLAGS: 2n,
  RUNE: 4n,
  PREMINE: 6n,
  CAP: 8n,
  AMOUNT: 10n,
  HEIGHT_START: 12n,
  HEIGHT_END: 14n,
  OFFSET_START: 16n,
  OFFSET_END: 18n,
  MINT: 20n,
  POINTER: 22n,
  CENOTAPH: 126n,
  DIVISIBILITY: 1n,
  SPACERS: 3n,
  SYMBOL: 5n,
  NOP: 127n,
} as const;

// Flag constants for etching
const Flag = {
  ETCHING: 0n,
  TERMS: 1n,
  TURBO: 2n,
  CENOTAPH: 3n,
};

function setFlag(flags: bigint, flag: bigint): bigint {
  return flags | (1n << flag);
}

function encodeOptionInt(payloads: bigint[], tag: bigint, opt: Option<any>) {
  if (opt.isSome()) {
    payloads.push(tag);
    payloads.push(u128(opt.unwrap()));
  }
}

// --- Etching types (simplified, no external deps) ---

export interface EtchingTermsSpec {
  amount?: bigint;
  cap?: bigint;
  height?: { start?: bigint; end?: bigint };
  offset?: { start?: bigint; end?: bigint };
}

export interface RuneEtchingSpec {
  runeName?: string;
  symbol?: string;
  divisibility?: number;
  premine?: bigint;
  terms?: EtchingTermsSpec;
  turbo?: boolean;
}

export interface RunestoneProtostoneSpec {
  mint?: { block: bigint; tx: bigint };
  pointer?: number;
  etching?: RuneEtchingSpec;
  edicts?: ProtoruneEdict[];
  protostones?: ProtoStone[];
}

// Simple Rune name encoder (A=0, B=1, ..., Z=25)
function encodeRuneName(name: string): bigint {
  // Strip spacers (dots/bullets)
  const stripped = name.replace(/[.\u2022\u00B7]/g, '');
  let value = 0n;
  for (let i = 0; i < stripped.length; i++) {
    const c = stripped.charCodeAt(i);
    if (c < 65 || c > 90) throw new Error(`Invalid rune character: ${stripped[i]}`);
    if (i > 0) value += 1n;
    value *= 26n;
    value += BigInt(c - 65);
  }
  return value;
}

function parseSpacers(name: string): number {
  let spacers = 0;
  let charIndex = 0;
  for (let i = 0; i < name.length; i++) {
    const c = name[i];
    if (c === '.' || c === '\u2022' || c === '\u00B7') {
      if (charIndex > 0) {
        spacers |= 1 << (charIndex - 1);
      }
    } else {
      charIndex++;
    }
  }
  return spacers;
}

// Compute the commitment for a Rune name (for etching)
function runeCommitment(runeValue: bigint): Buffer {
  const bytes: number[] = [];
  let v = runeValue;
  while (v > 0n) {
    bytes.push(Number(v & 0xffn));
    v >>= 8n;
  }
  return Buffer.from(bytes);
}

export class RunestoneProtostoneUpgrade {
  constructor(
    readonly mint: Option<ProtoruneRuneId>,
    readonly pointer: Option<u32>,
    readonly edicts: ProtoruneEdict[],
    readonly etching: Option<{
      flags: bigint;
      rune: Option<bigint>;
      divisibility: Option<number>;
      spacers: Option<number>;
      symbol: Option<string>;
      premine: Option<bigint>;
      terms: Option<{
        amount: Option<bigint>;
        cap: Option<bigint>;
        height: [Option<bigint>, Option<bigint>];
        offset: [Option<bigint>, Option<bigint>];
      }>;
    }>,
    readonly protostones: ProtoStone[],
  ) {}

  encipher(): Buffer {
    const payloads: bigint[] = [];

    if (this.etching.isSome()) {
      const etching = this.etching.unwrap();
      payloads.push(RuneTag.FLAGS);
      payloads.push(etching.flags);

      encodeOptionInt(payloads, RuneTag.RUNE, etching.rune);
      encodeOptionInt(payloads, RuneTag.DIVISIBILITY, etching.divisibility);
      encodeOptionInt(payloads, RuneTag.SPACERS, etching.spacers);
      encodeOptionInt(
        payloads,
        RuneTag.SYMBOL,
        etching.symbol.map((s: string) => BigInt(s.codePointAt(0)!)),
      );
      encodeOptionInt(payloads, RuneTag.PREMINE, etching.premine);

      if (etching.terms.isSome()) {
        const terms = etching.terms.unwrap();
        encodeOptionInt(payloads, RuneTag.AMOUNT, terms.amount);
        encodeOptionInt(payloads, RuneTag.CAP, terms.cap);
        encodeOptionInt(payloads, RuneTag.HEIGHT_START, terms.height[0]);
        encodeOptionInt(payloads, RuneTag.HEIGHT_END, terms.height[1]);
        encodeOptionInt(payloads, RuneTag.OFFSET_START, terms.offset[0]);
        encodeOptionInt(payloads, RuneTag.OFFSET_END, terms.offset[1]);
      }
    }

    if (this.mint.isSome()) {
      const claim = this.mint.unwrap();
      payloads.push(RuneTag.MINT);
      payloads.push(u128(claim.block));
      payloads.push(RuneTag.MINT);
      payloads.push(u128(claim.tx));
    }

    encodeOptionInt(payloads, RuneTag.POINTER, this.pointer.map(u128));

    // Encode protostones
    if (this.protostones.length) {
      let all_protostone_payloads: bigint[] = [];
      this.protostones.forEach((protostone: ProtoStone) => {
        protostone
          .encipher_payloads()
          .forEach((v) => all_protostone_payloads.push(v));
      });
      unpack(encipher(all_protostone_payloads)).forEach((v) => {
        payloads.push(u128(ProtoTag.PROTOCOL));
        payloads.push(u128(v));
      });
    }

    // Encode edicts
    if (this.edicts.length) {
      payloads.push(u128(RuneTag.BODY));

      const edicts = [...this.edicts].sort((x, y) =>
        Number(x.id.block - y.id.block || x.id.tx - y.id.tx),
      );

      let previous = new ProtoruneRuneId(u128(0), u128(0));
      for (const edict of edicts) {
        const [block, tx] = previous.delta(edict.id).unwrap();
        payloads.push(block);
        payloads.push(tx);
        payloads.push(edict.amount);
        payloads.push(u128(edict.output));
        previous = edict.id;
      }
    }

    // Compile to Bitcoin script
    const stack: (Buffer | number)[] = [];
    stack.push(OP_RETURN);
    stack.push(MAGIC_NUMBER);

    const payload = encipher(payloads);
    for (let i = 0; i < payload.length; i += MAX_SCRIPT_ELEMENT_SIZE) {
      stack.push(payload.subarray(i, i + MAX_SCRIPT_ELEMENT_SIZE));
    }

    return bitcoin.script.compile(stack);
  }
}

/**
 * Encode a Runestone with Protostone extensions into an OP_RETURN script buffer.
 *
 * @example
 * ```typescript
 * import { encodeRunestoneProtostone, ProtoStone, Cellpack } from '@alkanes/ts-sdk';
 *
 * const cellpack = new Cellpack(2n, 1n, [77n]);
 * const { encodedRunestone } = encodeRunestoneProtostone({
 *   protostones: [
 *     ProtoStone.message({
 *       protocolTag: 1n,
 *       calldata: cellpack.serialize(),
 *       pointer: 0,
 *       refundPointer: 1,
 *     }),
 *   ],
 * });
 * // Use encodedRunestone as an OP_RETURN output in a PSBT
 * ```
 */
export function encodeRunestoneProtostone(runestone: RunestoneProtostoneSpec): {
  encodedRunestone: Buffer;
  etchingCommitment?: Buffer;
} {
  const mint = runestone.mint
    ? Some(
        new ProtoruneRuneId(
          u128(runestone.mint.block),
          u128(runestone.mint.tx),
        ),
      )
    : None;

  const pointer =
    runestone.pointer !== undefined
      ? Some(u32(runestone.pointer))
      : None;

  const edicts: ProtoruneEdict[] = (runestone.edicts ?? []).map((edict) => ({
    id: new ProtoruneRuneId(u128(edict.id.block), u128(edict.id.tx)),
    amount: u128(edict.amount),
    output: edict.output,
  }));

  const protostones = runestone.protostones ?? [];

  let etching: Option<any> = None;
  let etchingCommitment: Buffer | undefined = undefined;

  if (runestone.etching) {
    const spec = runestone.etching;
    let flags = 0n;
    flags = setFlag(flags, Flag.ETCHING);
    if (spec.terms) flags = setFlag(flags, Flag.TERMS);
    if (spec.turbo) flags = setFlag(flags, Flag.TURBO);

    let runeValue: Option<bigint> = None;
    let spacers: Option<number> = None;
    if (spec.runeName) {
      const spacerVal = parseSpacers(spec.runeName);
      runeValue = Some(encodeRuneName(spec.runeName));
      if (spacerVal !== 0) spacers = Some(spacerVal);
      etchingCommitment = runeCommitment(runeValue.unwrap());
    }

    const divisibility = spec.divisibility !== undefined ? Some(spec.divisibility) : None;
    const premine = spec.premine !== undefined ? Some(spec.premine) : None;
    const symbol = spec.symbol ? Some(spec.symbol) : None;

    let terms: Option<any> = None;
    if (spec.terms) {
      const t = spec.terms;
      terms = Some({
        amount: t.amount !== undefined ? Some(t.amount) : None,
        cap: t.cap !== undefined ? Some(t.cap) : None,
        height: [
          t.height?.start !== undefined ? Some(t.height.start) : None,
          t.height?.end !== undefined ? Some(t.height.end) : None,
        ],
        offset: [
          t.offset?.start !== undefined ? Some(t.offset.start) : None,
          t.offset?.end !== undefined ? Some(t.offset.end) : None,
        ],
      });
    }

    etching = Some({ flags, rune: runeValue, divisibility, spacers, symbol, premine, terms });
  }

  return {
    encodedRunestone: new RunestoneProtostoneUpgrade(
      mint,
      pointer,
      edicts,
      etching,
      protostones,
    ).encipher(),
    etchingCommitment,
  };
}
