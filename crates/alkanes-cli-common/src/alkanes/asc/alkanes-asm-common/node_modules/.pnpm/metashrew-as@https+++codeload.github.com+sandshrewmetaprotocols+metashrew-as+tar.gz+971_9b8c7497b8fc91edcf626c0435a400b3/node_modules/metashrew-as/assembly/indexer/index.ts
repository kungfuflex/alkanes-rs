
@external("env", "__set") declare function __set(
  k: ArrayBuffer,
  v: ArrayBuffer,
): void;


@external("env", "__get") declare function __get(
  k: ArrayBuffer,
  v: ArrayBuffer,
): void;


@external("env", "__get_len") declare function __get_len(k: ArrayBuffer): i32;


@external("env", "__flush") declare function __flush(values: ArrayBuffer): void;


@external("env", "__load_input") declare function __load_input(
  block: ArrayBuffer,
): void;


@external("env", "__host_len") declare function __host_len(): i32;

import { xxh32 } from "../utils/xxh32";
import { sha256 } from "../utils/sha256";
import { memcpy } from "../utils/memcpy";
import { Box } from "../utils/box";
import { console } from "../utils/logging";
import { IndexPointer } from "./tables";
import { metashrew } from "../proto/metashrew";
const _updates = new Map<string, ArrayBuffer>();

const _updateKeys = new Map<string, ArrayBuffer>();

const BUFFER_SIZE = <u32>0x100000;
let _filled: u32 = 0;

export function input(): ArrayBuffer {
  const data = new ArrayBuffer(__host_len());
  __load_input(data);
  return data;
}
export function hash(k: ArrayBuffer): string {
  return Box.from(k).toHexString();
}
export function set(k: ArrayBuffer, v: ArrayBuffer): void {
  const h = hash(k);
  _updates.set(h, v);
  _updateKeys.set(h, k);
}
export function get(k: ArrayBuffer): ArrayBuffer {
  const h = hash(k);
  let result = changetype<ArrayBuffer>(0);
  if (!_updates.has(h)) {
    result = new ArrayBuffer(__get_len(k));
    __get(k, result);
    _updates.set(h, result);
  } else result = _updates.get(h);
  return result;
}

export function getImmutable(k: ArrayBuffer): ArrayBuffer {
  const result = new ArrayBuffer(__get_len(k));
  __get(k, result);
  return result;
}

export function arrayBufferToArray(data: ArrayBuffer): Array<u8> {
  const result = new Array<u8>(data.byteLength);
  store<usize>(changetype<usize>(result), changetype<usize>(data));
  store<usize>(
    changetype<usize>(result) + sizeof<usize>(),
    changetype<usize>(data),
  );
  return result;
}

export function _flush(): void {
  const hashKeys = _updateKeys.keys();
  const protobufInput = new Array<Array<u8>>();
  hashKeys.reduce(
    (r: Array<Array<u8>>, v: string, i: i32, ary: Array<string>) => {
      r.push(arrayBufferToArray(_updateKeys.get(v)));
      r.push(arrayBufferToArray(_updates.get(v)));
      return r;
    },
    protobufInput,
  );
  const message = new metashrew.KeyValueFlush();
  message.list = protobufInput;
  _updateKeys.clear();
  __flush(message.encode());
}

/**
 * Class - Index
 *
 * Description:
 * This class is used to encode data and call the __set host function to store an index,
 * this class abstracts database updates as well as rlp encoding & hashing of data and key respectively
 *
 */
export class Index {
  static keyFor(table: ArrayBuffer, key: ArrayBuffer): ArrayBuffer {
    return Box.concat([Box.from(table), Box.from(key)]);
  }

  static indexTransactionById(txid: ArrayBuffer, data: ArrayBuffer): void {
    set(Index.keyFor(TRANSACTION_ID_TO_TRANSACTION, txid), data);
  }

  // Index: "/insc/byid"
  static indexInscriptionById(inscID: ArrayBuffer, data: ArrayBuffer): void {
    set(Index.keyFor(INSCRIPTION_ID_TO_INSCRIPTION, inscID), data);
  }

  // static indexInscriptionsByHeight(height: ArrayBuffer, data: ArrayBuffer): void {
  //   set(Index.keyFor(HEIGHT_TO_INSCRIPTIONS, height), data);
  // }
}
