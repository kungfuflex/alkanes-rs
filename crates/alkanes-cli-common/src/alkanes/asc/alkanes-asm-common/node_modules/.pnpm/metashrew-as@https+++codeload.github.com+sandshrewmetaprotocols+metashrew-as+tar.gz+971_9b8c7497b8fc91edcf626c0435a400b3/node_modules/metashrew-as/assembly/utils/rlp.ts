import { Box } from "./box";
import { memcpy } from "./memcpy";
import { console } from "./logging";
import { nullptr } from "./pointer";

export const enum RLPItemType {
  NONE,
  ARRAYBUFFER,
  BOX,
  LIST,
  VALUE,
}

export class RLPItem {
  public typeFlag: RLPItemType;
  public data: usize;
  constructor(ptr: usize, typeFlag: RLPItemType) {
    this.data = ptr;
    this.typeFlag = typeFlag;
  }
  asList(): Array<RLPItem> {
    return changetype<Array<RLPItem>>(this.data);
  }
  asArrayBuffer(): ArrayBuffer {
    return changetype<ArrayBuffer>(this.data);
  }
  asBox(): Box {
    return changetype<Box>(this.data);
  }
  asValue(): usize {
    return this.data;
  }
  static fromList(v: Array<RLPItem>): RLPItem {
    return new RLPItem(changetype<usize>(v), RLPItemType.LIST);
  }
  static fromBox(v: Box): RLPItem {
    return new RLPItem(changetype<usize>(v), RLPItemType.BOX);
  }
  static fromArrayBuffer(v: ArrayBuffer): RLPItem {
    return new RLPItem(changetype<usize>(v), RLPItemType.ARRAYBUFFER);
  }
  static fromValue(v: usize): RLPItem {
    return new RLPItem(v, RLPItemType.VALUE);
  }
}

export function byteLengthForLength(v: i32): i32 {
  if (v >> 24 !== 0) return 4;
  if (v >> 16 !== 0) return 3;
  if (v >> 8 !== 0) return 2;
  return 1;
}

function fromRLPTrackPtr(Box: data, ptrToEnd: usize): RLPItem {
  const rlpType = data.start.deref<u8>();
  let result = changetype<RLPItem>(0);
  if (rlpType < 0x80) {
    result = RLPItem.fromBox(new Box(data.start, 1));
    store<usize>(ptrToEnd, load<usize>(ptrToEnd) + <usize>result.len + 1);
  } else if (rlpType < 0xb8) {
    const len = rlpType - 0x80;
    result = RLPItem.fromBox(
      new Box(toPointer<usize>(data.start.asUsize() + 1), len),
    );
    store<usize>(
      ptrToEnd,
      load<usize>(ptrToEnd) + <usize>result.len + <usize>2,
    );
  } else if (rlpType < 0xc0) {
    const byteLength = rlpType - 0xb7;
    if (byteLength == 1) {
      result = RLPItem.fromBox(
        new Box(
          toPointer<usize>(data.start.asUsize() + 2),
          <usize>load<u8>(data.start.asUsize() + 1),
        ),
      );
    } else if (byteLength == 2) {
      result = RLPItem.fromBox(
        new Box(
          toPointer<usize>(data.start.asUsize() + 3),
          <usize>bswap<u16>(load<u16>(data.start.asUsize() + 1)),
        ),
      );
    } else if (byteLength == 3) {
      result = RLPItem.fromBox(
        new Box(
          toPointer<usize>(data.start.asUsize() + 4),
          bswap<u32>(load<u32>(data.start.asUsize()) & (<u32>0xffffff)),
        ),
      );
    } else if (byteLength == 4) {
      result = RLPItem.fromBox(
        new Box(
          toPointer<usize>(data.start.asUsize() + 5),
          bswap<u32>(load<u32>(data.start.asUsize() + 1)),
        ),
      );
    } else throw Error("invalid byte-length for RLP segment");
    store<usize>(
      ptrToEnd,
      load<usize>(ptrToEnd) + <usize>result.len + <usize>1 + <usize>byteLength,
    );
  } else {
    let listLength: u32 = 0x0;
    let start = data.start.asUsize();
    if (rlpType < 0xf8) {
      listLength = <u32>rlpType - <u32>0xc0;
      start++;
    } else {
      const lengthBytes: u8 = data.start.deref<u8>() - <u8>0xf7;
      switch (lengthBytes) {
        case 1:
          listLength = <u32>load<u8>(data.start.asUsize() + <usize>1);
          break;
        case 2:
          listLength = <u32>(
            bswap<u16>(load<u16>(data.start.asUsize() + <usize>1))
          );
          break;
        case 3:
          listLength = <u32>(
            bswap<u32>(load<u32>(data.start.asUsize()) & (<u32>0xffffff))
          );
          break;
        case 4:
          listLength = <u32>bswap<u32>(load<u32>(data.start.asUsize()));
          break;
        default:
          throw Error("invalid byte-length for RLP segment");
          break;
      }
      start += listLength;
    }
    const list = new Array<RLPItem>();
    let end = load<usize>(ptrToEnd) + listLength;
    for (let i = start; load<usize>(ptrToEnd) < end; ) {
      list.push(
        fromRLPTrackPtr(
          new Box(
            toPointer<usize>(start),
            data.len + data.start.asUsize() - start,
          ),
          ptrToEnd,
        ),
      );
    }
    return RLPItem.fromList(list);
  }
}

export function fromRLP(data: Box): RLPItem {
  const end = new ArrayBuffer(sizeof<usize>());
  store<usize>(end, data.start.asUsize());
  return fromRLPTrackPtr(data, changetype<usize>(end));
}

export function toRLP(item: RLPItem): ArrayBuffer {
  if (item.typeFlag == RLPItemType.LIST) {
    const list = item
      .asList()
      .map<ArrayBuffer>(
        (v: RLPItem, i: i32, ary: Array<RLPItem>): ArrayBuffer => {
          const item = toRLP(v);

          return item;
        },
      );
    const length = list.reduce<i32>(
      (r: i32, v: ArrayBuffer, i: i32, ary: Array<ArrayBuffer>): i32 => {
        return r + v.byteLength;
      },
      0,
    );
    let result = nullptr<ArrayBuffer>();
    if (length <= 55) {
      result = new ArrayBuffer(length + 1);
      store<u8>(changetype<usize>(result), <u8>0xc0 + <u8>length);
      list.reduce<usize>(
        (r: usize, v: ArrayBuffer, i: i32, ary: Array<ArrayBuffer>) => {
          memcpy(r, changetype<usize>(v), v.byteLength);
          return r + <usize>v.byteLength;
        },
        changetype<usize>(result) + <usize>1,
      );
    } else {
      const byteLength = byteLengthForLength(length);
      result = new ArrayBuffer(<i32>length + <i32>byteLength + <i32>1);
      store<u8>(changetype<usize>(result), <u8>0xf7 + <u8>byteLength);
      if (byteLength === 1) {
        store<u8>(changetype<usize>(result) + 0x01, length);
      } else if (byteLength === 2) {
        store<u8>(
          changetype<usize>(result) + 0x01,
          <u8>((length & 0xff00) >> 8),
        );
        store<u8>(changetype<usize>(result) + 0x02, <u8>(length & 0xff));
      } else if (byteLength === 3) {
        store<u8>(
          changetype<usize>(result) + 0x01,
          <u8>((length & 0xff0000) >> 16),
        );
        store<u8>(
          changetype<usize>(result) + 0x02,
          <u8>((length & 0xff00) >> 8),
        );
        store<u8>(changetype<usize>(result) + 0x03, <u8>(length & 0xff));
      } else if (byteLength === 4) {
        store<u8>(
          changetype<usize>(result) + 0x01,
          <u8>((length & 0xff000000) >> 24),
        );
        store<u8>(
          changetype<usize>(result) + 0x02,
          <u8>((length & 0xff0000) >> 16),
        );
        store<u8>(
          changetype<usize>(result) + 0x03,
          <u8>((length & 0xff00) >> 8),
        );
        store<u8>(changetype<usize>(result) + 0x04, <u8>(length & 0xff));
      }
      list.reduce<usize>(
        (r: usize, v: ArrayBuffer, i: i32, ary: Array<ArrayBuffer>) => {
          memcpy(r, changetype<usize>(v), v.byteLength);
          return r + <usize>v.byteLength;
        },
        changetype<usize>(result) + <usize>1 + <usize>byteLength,
      );
    }
    return result;
  } else {
    let start: usize = 0;
    let len: i32 = 0;
    if (item.typeFlag === RLPItemType.ARRAYBUFFER) {
      const val = item.asArrayBuffer();
      start = changetype<usize>(val);
      len = val.byteLength;
    } else if (item.typeFlag === RLPItemType.BOX) {
      const val = item.asBox();
      start = val.start;
      len = <i32>val.len;
    }
    if (len === 1) {
      const byte = load<u8>(start);
      if (byte <= 0x7f) {
        const result = new ArrayBuffer(1);
        store<u8>(changetype<usize>(result), byte);
        return result;
      }
    }
    if (len >= 0 && len <= 55) {
      const result = new ArrayBuffer(len + 1);
      store<u8>(changetype<usize>(result), <u8>len + 0x80);
      memcpy(changetype<usize>(result) + 0x01, start, <usize>len);
      return result;
    } else {
      const length = byteLengthForLength(len);
      const result = new ArrayBuffer(len + length + 1);
      store<u8>(changetype<usize>(result), <u8>length + <u8>0xb7);
      if (length === 1) {
        store<u8>(changetype<usize>(result) + 0x01, len);
      } else if (length === 2) {
        store<u8>(changetype<usize>(result) + 0x01, <u8>((len & 0xff00) >> 8));
        store<u8>(changetype<usize>(result) + 0x02, <u8>(len & 0xff));
      } else if (length === 3) {
        store<u8>(
          changetype<usize>(result) + 0x01,
          <u8>((len & 0xff0000) >> 16),
        );
        store<u8>(changetype<usize>(result) + 0x02, <u8>((len & 0xff00) >> 8));
        store<u8>(changetype<usize>(result) + 0x03, <u8>(len & 0xff));
      } else if (length === 4) {
        store<u8>(
          changetype<usize>(result) + 0x01,
          <u8>((len & 0xff000000) >> 24),
        );
        store<u8>(
          changetype<usize>(result) + 0x02,
          <u8>((len & 0xff0000) >> 16),
        );
        store<u8>(changetype<usize>(result) + 0x03, <u8>((len & 0xff00) >> 8));
        store<u8>(changetype<usize>(result) + 0x04, <u8>(len & 0xff));
      }
      memcpy(
        changetype<usize>(result) + <usize>1 + <usize>length,
        start,
        <usize>len,
      );
      return result;
    }
  }
}
