import { _flush, input, getImmutable, get, set } from "./index";
import { memcpy } from "../utils/memcpy";
import { Box } from "../utils/box";
import { console } from "../utils/logging";

export class Node {
  public key: ArrayBuffer;
  public value: ArrayBuffer;
  constructor(key: ArrayBuffer, value: ArrayBuffer) {
    this.key = key;
    this.value = value;
  }


  @inline
  static from(key: ArrayBuffer, value: ArrayBuffer): Node {
    return new Node(key, value);
  }
}


@final
@unmanaged
export class IndexPointer {
  [key: number]: number;
  constructor(keyPrefix: ArrayBuffer) {
    return IndexPointer.wrap(keyPrefix);
  }
  static wrap(pointer: ArrayBuffer): IndexPointer {
    return changetype<IndexPointer>(pointer);
  }
  static for(keyword: string): IndexPointer {
    return IndexPointer.wrap(String.UTF8.encode(keyword));
  }
  unwrap(): ArrayBuffer {
    return changetype<ArrayBuffer>(this);
  }
  select(key: ArrayBuffer): IndexPointer {
    const res = Box.concat([Box.from(this.unwrap()), Box.from(key)]);
    return IndexPointer.wrap(res);
  }
  selectValue<T>(key: T): IndexPointer {
    const keyBytes = new ArrayBuffer(sizeof<T>());
    store<T>(changetype<usize>(keyBytes), bswap<T>(key));
    return this.select(keyBytes);
  }
  keyword(key: string): IndexPointer {
    return this.select(String.UTF8.encode(key));
  }
  getValue<T>(): T {
    const value = this.get();
    if (value.byteLength === 0) return <T>0;
    const container = new ArrayBuffer(sizeof<T>());
    memcpy(
      changetype<usize>(container),
      changetype<usize>(value),
      value.byteLength,
    );
    return load<T>(changetype<usize>(container));
  }
  getImmutableValue<T>(): T {
    const value = this.getImmutable();
    if (value.byteLength === 0) return <T>0;
    const container = new ArrayBuffer(sizeof<T>());
    memcpy(
      changetype<usize>(container),
      changetype<usize>(value),
      value.byteLength,
    );
    return load<T>(changetype<usize>(container));
  }
  setValue<T>(v: T): void {
    const value = new ArrayBuffer(sizeof<T>());
    store<T>(changetype<usize>(value), v);
    this.set(value);
  }
  set(v: ArrayBuffer): void {
    set(this.unwrap(), v);
  }
  get(): ArrayBuffer {
    return Box.from(get(this.unwrap())).toArrayBuffer();
  }
  getImmutable(): ArrayBuffer {
    return getImmutable(this.unwrap());
  }
  lengthKey(): IndexPointer {
    return this.keyword("/length");
  }
  length(): u32 {
    return this.lengthKey().getValue<u32>();
  }
  getList(): Array<ArrayBuffer> {
    const result = new Array<ArrayBuffer>(<i32>this.length());
    for (let i: i32 = 0; i < result.length; i++) {
      result[i] = this.selectIndex(i).get();
    }
    return result;
  }
  getListValues<T>(): Array<T> {
    const result = new Array<T>(<i32>this.length());
    for (let i: i32 = 0; i < result.length; i++) {
      result[i] = this.selectIndex(i).getValue<T>();
    }
    return result;
  }
  extend(): IndexPointer {
    const lengthKey = this.lengthKey();
    const length = lengthKey.getValue<u32>();
    lengthKey.setValue<u32>(length + 1);
    return this.selectIndex(length);
  }
  selectIndex(index: u32): IndexPointer {
    return this.keyword("/" + index.toString(10));
  }
  nullify(): void {
    this.set(new ArrayBuffer(0));
  }
  pop(): ArrayBuffer {
    const lengthKey = this.lengthKey();
    const length = lengthKey.getValue<u32>();
    if (length === 0) return new ArrayBuffer(0);
    const newLength = length - 1;
    lengthKey.setValue<u32>(newLength);
    return this.selectIndex(newLength).get();
  }
  popValue<T>(): ArrayBuffer {
    const lengthKey = this.lengthKey();
    const length = lengthKey.getValue<u32>();
    if (length === 0) return new ArrayBuffer(0);
    const newLength = length - 1;
    lengthKey.setValue<u32>(newLength);
    return this.selectIndex(newLength).getValue<T>();
  }
  append(v: ArrayBuffer): void {
    this.extend().set(v);
  }
  appendValue<T>(v: T): void {
    this.extend().setValue<T>(v);
  }
}
/*

export class ListPointer {
  public keyPrefix: ArrayBuffer;
  constructor(key: ArrayBuffer) {
    this.keyPrefix = prefix;

  }
  static lengthKey(key: ArrayBuffer): ArrayBuffer {
    return Box.concat([
      Box.from(key),
      Box.from(String.UTF8.encode("/length"))
    ]);
  }
  static tipKey(key: ArrayBuffer): ArrayBuffer {
    return Box.concat([
      Box.from(key),
      Box.from(String.UTF8.encode("/tip"))
    ]);
  }
  static nextKey(key: ArrayBuffer): ArrayBuffer {
    return Box.concat([
      Box.from(key),
      Box.from(String.UTF8.encode("/next"))
    ]);
  }
  static subKey(key: ArrayBuffer, subKey: ArrayBuffer): ArrayBuffer {
    return Box.concat([
      Box.from(key),
      Box.from(String.UTF8.encode("/")),
      Box.from(subKey)
    ]);
  }
  static previousKey(key: ArrayBuffer): ArrayBuffer {
    return Box.concat([
      Box.from(key),
      Box.from(String.UTF8.encode("/previous"))
    ]);
  }
  static appendNode(key: ArrayBuffer, subKey: ArrayBuffer, value: ArrayBuffer): void {
    const tableKey = Table.keyFor(key);
    let nextKey = Table.tipKey(tableKey);
    let tip = get(tipKey);
    let previous = new ArrayBuffer(0);
    let i = 0;
    while (tip.byteLength !== 0) {
      previous = tip;
      nextKey = Table.nextKey(Table.subKey(tableKey, tip));
      tip = get(nextKey);
      i++;
    }
    if (i > 0) {
      set(Table.previousKey(Table.subKey(tableKey, subKey)), previous);
      set(Table.nextKey(Table.subKey(tableKey, previous)), subKey);
    }
    set(Table.subKey(tableKey, subKey), value);
  }
  static listKey(key: ArrayBuffer, i: u32): ArrayBuffer {
    return Box.concat([
      Box.from(key),
      Box.from(String.UTF8.encode("/")),
      Box.from(String.UTF8.encode(i.toString(10)))
    ]);
  }
  static keyFor(table: ArrayBuffer, key: ArrayBuffer): ArrayBuffer {
    return Box.concat([Box.from(table), Box.from(key)]);
  }
  public tableKey(key: ArrayBuffer): ArrayBuffer {
    return Table.keyFor(this.keyPrefix, key);
  }
  static open(prefix: ArrayBuffer): Table {
    return new Table(prefix);
  }
  fromLinkedList(key: ArrayBuffer): Array<Node> {
    let tip = this.get(Table.tipKey(key));
    const result: Array<Node> = [];
    if (tip.byteLength === 0) return [];
    while (tip.byteLength !== 0) {
      result.push(Node.from(tip, this.get(Table.subKey(key, tip))));
      tip = this.get(Table.nextKey(Table.subKey(key, tip)));
    }
    return result;
  }
}
export const HEIGHT_TO_BLOCKHASH = Table.open(String.UTF8.encode("/block/byheight"));
export const BLOCKHASH_TO_HEIGHT = Table.open(String.UTF8.encode("/height/byhash"));
export const SAT_TO_SATPOINT = Table.open(String.UTF8.encode("/satpoint/byordinal"));
export const SAT_TO_INSCRIPTION_ID = Table.open(String.UTF8.encode("/inscription/byordinal"));
export const INSCRIPTION_TO_SATPOINT = Table.open(String.UTF8.encode("/satpoint/byinscription"));
export const SATPOINT_TO_INSCRIPTION = Table.open(String.UTF8.encode("/inscription/bysatpoint"));
export const OUTPOINT_TO_SATRANGES = Table.open(String.UTF8.encode("/satranges/byoutpoint"));
export const OUTPOINT_TO_VALUE = Table.open(String.UTF8.encode("/outpoint/tovalue"));
export const SEQUENCE_TO_INSCRIPTION_ID = Table.open(String.UTF8.encode("/inscription/bysequence"));
export const SEQUENCE_NUMBER_TO_SATPOINT = Table.open(String.UTF8.encode("/satpoint/bysequencenumber"));
export const SEQUENCE_NUMBER_TO_ENTRY = Table.open(String.UTF8.encode("/entry/bysequencenumber"));
export const SEQUENCE_NUMBER_TO_CHILDREN = Table.open(String.UTF8.encode("/children/bysequencenumber"));
export const SAT_TO_SEQUENCE_NUMBER = Table.open(String.UTF8.encode("/sequencenumber/bysat"));
export const INSCRIPTION_NUMBER_TO_SEQUENCE_NUMBER = Table.open(String.UTF8.encode("/sequencenumber/byinscriptionnumber"));
export const ID_TO_SEQUENCE_NUMBER = Table.open(String.UTF8.encode("/sequencenumber/byid"));
export const HOME_INSCRIPTIONS = Table.open(String.UTF8.encode("/homeinscriptions"));
export const SATPOINT_TO_SEQUENCE_NUMBER = Table.open(String.UTF8.encode("/sequencenumber/bysatpoint"));
export const CONTENT_TYPE_TO_COUNT = Table.open(String.UTF8.encode("/count/bycontenttype"));
export const VALUE_CACHE = Table.open(String.UTF8.encode("/valuecache"));
export const TRANSACTION_ID_TO_TRANSACTION = Table.open(String.UTF8.encode("/transaction/byid"));
*/
