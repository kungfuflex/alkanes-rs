import { console } from "../utils/logging";
import { Box } from "../utils/box";
import { Version } from "./version";
import {
  parsePrimitive,
  parseBytes,
  concat,
  primitiveToBuffer,
  reverse,
} from "../utils/utils";
import { nullptr, Pointer, toPointer } from "../utils/pointer";
import { Transaction } from "./transaction";
import { AuxPow } from "./auxpow";
import { sha256d } from "../utils/sha256";
import { encodeHexFromBuffer } from "../utils/hex";

export class Header {
  public version: Version;
  public prevBlock: Box;
  public merkleRoot: Box;
  public time: u32;
  public bits: u32;
  public nonce: u32;
  public bytes: Box;
  public auxpow: AuxPow;
  constructor(data: Box) {
    let head = data.start;
    this.auxpow = nullptr<AuxPow>();
    this.version = new Version(parsePrimitive<u32>(data));
    this.prevBlock = parseBytes(data, 32);
    this.merkleRoot = parseBytes(data, 32);
    this.time = parsePrimitive<u32>(data);
    this.bits = parsePrimitive<u32>(data);
    this.nonce = parsePrimitive<u32>(data);
    let tail = data.start;
    this.bytes = toPointer(head).toBox(tail - head);
    if (this.version.isAuxPow() && data.len !== 0) this.auxpow = new AuxPow(data);
  }
  inspect(): string {
    return "Header {\n" +
      "  version: " + this.version.unwrap().toString(10) + ",\n" +
      "  prevBlock: " + this.prevBlock.toHexString() + ",\n" +
      "  merkleRoot: " + this.merkleRoot.toHexString() + ",\n" +
      "  time: " + this.time.toString(10) + ",\n" +
      "  bits: " + this.bits.toString(2) + ",\n" +
      "  nonce: " + this.nonce.toString(16) + "\n" +
    "}";
  }
  toLeBytes(): ArrayBuffer {
    let vers = primitiveToBuffer<i32>(this.version.unwrap());
    let prevBlock = this.prevBlock.toArrayBuffer();
    let merkleRoot = this.merkleRoot.toArrayBuffer();
    let time = primitiveToBuffer<u32>(this.time);
    let bits = primitiveToBuffer<u32>(this.bits);
    let nonce = primitiveToBuffer<u32>(this.nonce);
    return concat([vers, prevBlock, merkleRoot, time, bits, nonce]);
  }
}

export class Block {
  public bytes: Box;
  public header: Header;
  public transactions: Array<Transaction>;
  public prevBlock: Box;
  public merkleRoot: Box;
  constructor(data: Box) {
    this.transactions = nullptr<Array<Transaction>>();
    this.prevBlock = nullptr<Box>();
    this.merkleRoot = nullptr<Box>();
    let head = data.start;
    this.header = new Header(data);
    this.transactions = Transaction.parseTransactionsFromBlock(data);
    let tail = data.start;
    this.bytes = toPointer(head).toBox(tail - head);
  }

  // returns the coinbase transaction if there is one
  coinbase(): Transaction {
    return this.transactions[0];
  }

  blockhash(): ArrayBuffer {
    // console.log(encodeHexFromBuffer(this.header.toLeBytes()) + "\n");
    return sha256d(this.header.toLeBytes());
  }

  // returns list of txids in the block transaction(s) body including the coinbase txid
  txids(): Array<ArrayBuffer> {
    let txids = new Array<ArrayBuffer>();
    for (let i = 0; i < this.transactions.length; i++) {
      txids.push(this.transactions[i].txid());
    }
    return txids;
  }
}
