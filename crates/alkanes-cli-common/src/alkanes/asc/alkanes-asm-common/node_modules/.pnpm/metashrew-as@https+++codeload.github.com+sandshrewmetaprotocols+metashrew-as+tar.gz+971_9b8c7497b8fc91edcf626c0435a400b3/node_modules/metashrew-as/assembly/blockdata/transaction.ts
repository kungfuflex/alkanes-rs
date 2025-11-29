import { console } from "../utils/logging";
import {
  reverse,
  parsePrimitive,
  parseVarInt,
  parseLenThenBytes,
  parseBytes,
  isPushOp,
  decodeTag,
  concat,
  primitiveToBuffer,
} from "../utils/utils";
import { Box } from "../utils/box";
import { encodeHex, encodeHexFromBuffer } from "../utils/hex";
import { sha256d, sha256 } from "../utils/sha256";
import { Script } from "../utils/yabsp";
import { Inscription, Field } from "./inscription";
import { Pointer, toPointer, nullptr } from "../utils/pointer";
import { Address } from "./address";
import { Witness } from "./witness";

// A reference to a transaction output.
export class OutPoint {
  public bytes: Box;
  public txid: Box;
  public index: u32;
  constructor(bytes: Box) {
    this.bytes = nullptr<Box>();
    this.txid = nullptr<Box>();
    this.index = 0;
    if (bytes.len == 36) {
      this.bytes = bytes;
      this.txid = bytes.sliceFrom(0).shrinkBack(<usize>4);
      this.index = parsePrimitive<u32>(bytes.sliceFrom(32));
    }
  }
  toArrayBuffer(): ArrayBuffer {
    return this.bytes.toArrayBuffer();
  }
  isNull(): boolean {
    return (
      encodeHexFromBuffer(this.txid.toArrayBuffer()) ==
      "0x0000000000000000000000000000000000000000000000000000000000000000"
    );
  }

  static from(txid: ArrayBuffer, offset: u32): OutPoint {
    let index = primitiveToBuffer<u32>(offset);
    return new OutPoint(Box.from(concat([txid, index])));
  }

  /**
   * Returns the outpoint as a buffer
   * @returns {ArrayBuffer} - The outpoint as a buffer
   */
  toBuffer(): ArrayBuffer {
    return this.bytes.toArrayBuffer();
  }
}

export class Input {
  public bytes: Box;
  public hash: Box;
  public index: u32;
  public script: Box;
  public sequence: u32;
  public witness: Witness;
  constructor(data: Box) {
    const head = data.start;
    this.hash = parseBytes(data, 32);
    this.index = parsePrimitive<u32>(data);
    this.script = parseLenThenBytes(data);
    this.sequence = parsePrimitive<u32>(data);
    this.witness = nullptr<Witness>();
    const tail = data.start;
    this.bytes = toPointer(head).toBox(tail - head);
  }

  /**
   * Returns the outpoint of the input
   * @returns {ArrayBuffer} - The outpoint of the input
   */
  previousOutput(): OutPoint {
    let txid = toPointer(this.hash.start).toBox(<usize>32);
    // let bytes = toPointer(this.hash.start).toBox(<usize>36);
    return OutPoint.from(
      reverse(txid.toArrayBuffer()),
      this.index
    );
  }

  inscription(): Inscription | null {
    if (changetype<usize>(this.witness) === 0) return null;
    let script = this.witness.tapscript();
    if (script == nullptr<Box>() || !Witness.isInscribed(script)) return null;
    return new Inscription(script);
  }
}

export class Output {
  public bytes: Box;
  public value: u64;
  public script: Box;
  public segwit: boolean;
  constructor(data: Box, segwit: boolean = false) {
    const head = data.start;
    this.value = parsePrimitive<u64>(data);
    this.script = parseLenThenBytes(data);
    const tail = data.start;
    this.bytes = toPointer(head).toBox(tail - head);
    this.segwit = segwit;
  }

  /**
   * Returns the address of the output, returns null if the output is not a known or valid address
   * @returns {ArrayBuffer | null} - The address of the output or null if the output is not a known or valid address
   */
  intoAddress(): ArrayBuffer | null {
    let scr = Script.from(this.script);
    return Address.from(scr);
  }
}

export class Transaction {
  public bytes: Box;
  public version: u32;
  public flag: boolean;
  public ins: Array<Input>;
  public outs: Array<Output>;
  public locktime: u32;
  public witnessDataBytes: Box;
  public legacyInputOutputBytes: Box;
  private _txid: ArrayBuffer;
  constructor(data: Box) {
    this.bytes = nullptr<Box>();
    this.ins = new Array<Input>();
    this.outs = new Array<Output>();
    this.witnessDataBytes = nullptr<Box>();
    this.legacyInputOutputBytes = nullptr<Box>();
    this.flag = false;
    this._txid = new ArrayBuffer(0);

    const head = data.start;
    this.version = parsePrimitive<u32>(data);
    const saved = data.sliceFrom(0);
    const marker = parsePrimitive<u8>(data);
    const flag = parsePrimitive<u8>(data);
    if (marker === <u8>0 && flag === <u8>1) {
      this.flag = true;
    } else {
      data.start = saved.start;
      data.len = saved.len;
    }

    let inputLegacyHead = data.start;
    const vinLen = <i32>parseVarInt(data);
    for (let i = 0; i < vinLen; i++) {
      this.ins[i] = new Input(data);
    }

    const voutLen = <i32>parseVarInt(data);
    for (let i = 0; i < voutLen; i++) {
      if (this.flag) this.outs.push(new Output(data, true));
      else this.outs.push(new Output(data));
    }
    let outputLegacyTail = data.start;
    this.legacyInputOutputBytes = toPointer(inputLegacyHead).toBox(
      outputLegacyTail - inputLegacyHead,
    );

    if (this.flag) {
      let witnessHead = data.start;
      for (let i = 0; i < vinLen; i++) {
        this.ins[i].witness = new Witness(data);
      }
      let witnessTail = data.start;
      this.witnessDataBytes = toPointer(witnessHead).toBox(
        witnessTail - witnessHead,
      );
    }

    this.locktime = parsePrimitive<u32>(data);
    let tail = data.start;
    this.bytes = toPointer(head).toBox(tail - head);

    // let testbytes = toPointer(head).toBox((tail - head) + 4)
    // console.log("\n\n 4 bytes larger then end => " + encodeHexFromBuffer( testbytes.toArrayBuffer() ));
  }

  legacyBytes(): Array<Box> {
    if (this.flag) {
      let version = toPointer(this.bytes.start).toBox(4);
      let locktime = toPointer(this.bytes.start + this.bytes.len - 4).toBox(4);
      let result = new Array<Box>();
      result.push(version);
      result.push(this.legacyInputOutputBytes);
      result.push(locktime);
      return result;
    } else {
      let result = new Array<Box>();
      result.push(this.bytes);
      return result;
    }
  }

  txid(): ArrayBuffer {
    if (this._txid.byteLength > 0) {
      return this._txid;
    }
    this._txid = sha256d(Box.concat(this.legacyBytes()));
    return this._txid;
  }

  parseInscriptions(): Array<Inscription> {
    return Transaction.parseInscriptionsFromTransaction(this);
  }

  static parseInscriptionsFromTransaction(tx: Transaction): Array<Inscription> {
    let insc = new Array<Inscription>();
    let vins = tx.ins;
    let vinLen = tx.ins.length;

    for (let j = 0; j < vinLen; j++) {
      let script = vins[j].witness.tapscript();
      if (script == nullptr<Box>()) continue;
      let inscription = new Inscription(script);
      insc.push(inscription);
    }

    return insc;
  }

  static parseTransactionsFromBlock(data: Box): Array<Transaction> {
    const txsLen = parseVarInt(data);
    let result = new Array<Transaction>(<i32>txsLen);
    for (let i = 0; i < <i32>txsLen; i++) {
      result[i] = new Transaction(data);
    }
    return result;
  }
}
