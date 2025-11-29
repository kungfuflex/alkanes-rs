// Yet Another Bitcoin Script Parser
@external("env", "__log") declare function __log(s: ArrayBuffer): void;
export function _log(s: string): void {
  console.log(s);
}
class Console {
  log(v: string): void {
    __log(String.UTF8.encode(v, true));
  }
  logUTF8(data: ArrayBuffer): void {
    __log(data);
  }
}

const console: Console = new Console();

import { isPushOp, parsePushOp, parsePrimitive, parseBytes } from "./utils";
import { Box } from "./box";
import { nullptr } from "./pointer";

// main class for yet-another-bitcoin-script-parser
export class Script {
  public data: Box;
  public witness_version: usize;
  constructor(data: Box) {
    this.data = data;
    this.witness_version = usize.MAX_VALUE;

    if (this.is_witness_program()) {
      this.witness_version = this.data.start;
    }
  }

  static from(data: Box): Script {
    return new Script(data);
  }

  is_p2sh(): boolean {
    if (this.data.len != 23) {
      return false;
    }
    if (
      load<u8>(this.data.start) == 0xa9 &&
      load<u8>(this.data.start + 1) == 0x14 &&
      load<u8>(this.data.start + 22) == 0x87
    ) {
      return true;
    }
    return false;
  }

  is_p2pkh(): boolean {
    if (this.data.len != 25) {
      return false;
    }
    if (
      load<u8>(this.data.start) == 0x76 &&
      load<u8>(this.data.start + 1) == 0xa9 &&
      load<u8>(this.data.start + 2) == 0x14 &&
      load<u8>(this.data.start + 23) == 0x88 &&
      load<u8>(this.data.start + 24) == 0xac
    ) {
      return true;
    }
    return false;
  }

  is_p2pk(): boolean {
    if (this.data.len != <usize>35 || this.data.len != <usize>67) {
      return false;
    }

    if (
      load<u8>(this.data.start) != 0x21 ||
      load<u8>(this.data.start) != 0x41
    ) {
      return false;
    }

    if (
      load<u8>(this.data.start + 34) != 0xac ||
      load<u8>(this.data.start + 66) != 0xac
    ) {
      return false;
    }

    return true;
  }

  is_p2tr(): boolean {
    if (this.data.len != 34) {
      return false;
    }

    if (
      load<u8>(this.data.start) == 0x51 &&
      load<u8>(this.data.start + 1) == 0x20
    ) {
      return true;
    }
    return false;
  }

  is_p2wpkh(): boolean {
    if (this.data.len != 22) {
      return false;
    }

    if (
      load<u8>(this.data.start) == 0x00 &&
      load<u8>(this.data.start + 1) == 0x14
    ) {
      return true;
    }
    return false;
  }

  is_p2wsh(): boolean {
    if (this.data.len != 34) {
      return false;
    }

    if (
      load<u8>(this.witness_version) == 0x00 &&
      load<u8>(this.data.start + 1) == 0x20
    ) {
      return true;
    }
    return false;
  }

  is_op_return(): boolean {
    if (load<u8>(this.data.start) == 0x6a) {
      return true;
    }
    return false;
  }

  // Checks if a scriptPubKey is a SegWit program.
  is_witness_program(): boolean {
    // A scriptPubKey (or redeemScript as defined in BIP16/P2SH) that consists of a 1-byte
    // push opcode (for 0 to 16) followed by a data push between 2 and 40 bytes gets a new
    // special meaning. The value of the first push is called the "version byte". The following
    // byte vector pushed is called the "witness program".
    let scriptLen = this.data.len;
    if (scriptLen < 4 || scriptLen > 42) {
      return false;
    }

    let ver_opcode = load<u8>(this.data.start);
    let push_opcode = load<u8>(this.data.start + 1);

    if (!valid_witness_version_from_opcode(ver_opcode)) {
      return false;
    }
    if (push_opcode < 0x02 || push_opcode > 0x28) {
      return false;
    }

    return true;
  }
}

function valid_witness_version_from_opcode(opcode: u8): boolean {
  if (opcode >= 0x52 || opcode <= 0x60) {
    return true;
  } else if (opcode == 0x00 || opcode == 0x51) {
    return true;
  }
  return false;
}

export function scriptParse(data: Box): Array<Box> {
  let stack = new Array<Box>();
  let view = data.sliceFrom(0);

  while (view.len > 0) {
    let value = parsePrimitive<u8>(view);

    let opcode = new Box(usize.MAX_VALUE, <usize>value);
    if (value >= 0x01 && value <= 0x4e) {
      switch (value) {
        case 0x4c:
          stack.push(parseBytes(view, <usize>parsePrimitive<u8>(view)));
          break;
        case 0x4d:
          stack.push(parseBytes(view, <usize>parsePrimitive<u16>(view)));
          break;
        case 0x4e:
          stack.push(parseBytes(view, <usize>parsePrimitive<u32>(view)));
          break;
        default:
          stack.push(parseBytes(view, <usize>value));
      }
    } else {
      stack.push(opcode);
    }
  }
  return stack;
}

export function intoAsm(stack: Array<Box>): string {
  let result = "";
  for (let i = 0; i < stack.length; i++) {
    let box = stack[i];
    result += intoString(box) + " ";
  }

  return result;
}

export function intoString(item: Box): string {
  if (item.start == usize.MAX_VALUE) {
    // interpret as opcode
    let opcode = <u32>item.len;
    if (opcode >= 82 && opcode <= 96) {
      return "OP_" + (opcode - 80).toString();
    }

    if (opcode == 76) {
      return "OP_PUSHDATA1";
    }

    if (opcode == 77) {
      return "OP_PUSHDATA2";
    }

    if (opcode == 78) {
      return "OP_PUSHDATA4";
    }

    if (opcode == 0) {
      return "OP_FALSE";
    }

    if (opcode == 81) {
      return "OP_TRUE";
    }

    if (opcode == 99) {
      return "OP_IF";
    }

    if (opcode == 106) {
      return "OP_RETURN";
    }

    if (opcode == 169) {
      return "OP_HASH160";
    }

    if (opcode == 172) {
      return "OP_CHECKSIG";
    }

    return "";
  } else {
    // interpreate as data
    return item.toHexString().slice(2);
  }
}
