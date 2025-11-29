import { Box } from "../utils/box";
import { toPointer, nullptr, Pointer } from "../utils/pointer";
import {
  isPushOp,
  isOrdTag,
  parsePushOp,
  decodeTag,
  fromPushBox,
  concat,
  parsePrimitive,
  parseBytes,
} from "../utils/utils";
import { console } from "../utils/logging";
import { logArrayBuffer } from "..";

export class Field {
  public tag: u32;
  public data: ArrayBuffer;
  constructor(tag: u32, data: ArrayBuffer) {
    this.tag = tag;
    this.data = data;
  }
}

export function parseEnvelope(view: Box): Box {
  let head = view.start;
  let len = view.len;

  while (head < view.start + view.len - 1) {
    if (load<u8>(head) == 0x00 && load<u8>(head + 1) == 0x63) {
      head += 2;
      len -= 2;
      break;
    }
    const next = toPointer(head).toBox(len);
    if (isPushOp(load<u8>(head))) {
      parsePushOp(next);
      head = next.start;
      len = next.len;
    } else {
      head++;
      len--;
    }
  }

  let subview = toPointer(head).toBox(len);
  // console.log("subview " + subview.toHexString())
  while (subview.len > 0) {
    if (load<u8>(subview.start) == 0x68) {
      // console.log("errror finding closing tag")
      let distance = <i32>subview.start - <i32>head;
      // console.log("pointer distance " + distance.toString())
      let envelopeBox = toPointer(head).toBox(distance);
      return envelopeBox;
    }
    parsePushOp(subview);
  }

  // console.log("error after parsing envelope")
  // return pointer to zero as fail case
  return nullptr<Box>();
}

export class Inscription {
  bytes: Box;
  public fields: Array<Field>;
  public malformed: boolean;
  constructor(data: Box) {
    this.bytes = nullptr<Box>();
    this.fields = new Array<Field>();
    this.malformed = false;
    const view = data.sliceFrom(0);
    // console.log("entire view " + view.toHexString())

    let inscBox: Box = parseEnvelope(view);
    this.bytes = inscBox.sliceFrom(0);

    // console.log("inscription Box" + inscBox.toHexString());

    let ordTag = parsePushOp(inscBox);
    if (!isOrdTag(ordTag)) {
      throw Error("invalid inscription");
    }
    let content = new Array<ArrayBuffer>();
    for (
      let i = parsePushOp(inscBox);
      inscBox.len > 0;
      i = parsePushOp(inscBox)
    ) {
      const decodedTag = decodeTag(i);
      // console.log(`got decoded tag ${decodedTag} and len is ${inscBox.len}`);
      if (decodedTag >= 0x01 && decodedTag <= 0x0b) {
        this.fields.push(
          new Field(<u32>decodedTag, fromPushBox(parsePushOp(inscBox))),
        );
      } else if (decodedTag == 0x00) {
        break;
      }
    }

    // console.log("parsed Inscription Box " + inscBox.toHexString())

    // handle content body pushes if data remains
    if (inscBox != nullptr<Box>()) {
      // console.log("error handling content body")
      while (inscBox.len > 0) {
        let i = parsePushOp(inscBox);
        // console.log("content body" + i.toHexString());
        content.push(fromPushBox(i));
      }
    }

    // console.log("error after handling content body");

    // fill out content body
    let contentBody: ArrayBuffer = concat(content);
    this.fields.push(new Field(<u32>0x00, contentBody));
    // console.log("inscription success")
  }
  field(tag: i32): ArrayBuffer | null {
    for (let i = 0; i < this.fields.length; i++) {
      if (this.fields[i].tag === <u32>tag) return this.fields[i].data;
    }
    return null;
  }
  toArrayBuffer(): ArrayBuffer {
    return this.bytes.toArrayBuffer();
  }
  body(): ArrayBuffer | null {
    return this.field(0);
  }
}
