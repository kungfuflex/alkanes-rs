# metashrew-as

The indexer standard library. Sources in this repository can be imported into indexer programs built with metashrew, and may have some utilty in other contexts.

## Install

This library will be included by default in projects scaffolded with metashrew-cli, guides for which are available here:

[https://github.com/sandshrewmetaprotocols/metashrew-cli](https://github.com/sandshrewmetaprotocols/metashrew-cli)

For projects not scaffolded with metashrew-cli, installation of this library is done with npm or yarn as one would for a typical JavaScript package.

```sh
yarn add https://github.com/sandshrewmetaprotocols/metashrew-as
```

## Usage

### Getting and setting key-value pairs

### assembly/indexer/tables

#### IndexPointer

IndexPointer is the abstraction in metashrew over the key in a key-value pair. It is a wrapper over ArrayBuffer but has methods to add more segments to the key to form a new IndexPointer, as well as methods to set an ArrayBuffer value or any primitive value.

```js
abstract class IndexPointer {
  static wrap(pointer: ArrayBuffer): IndexPointer;
  unwrap(): ArrayBuffer;
  static for(keyword: string): 
  select(key: ArrayBuffer): IndexPointer;
  selectValue<T>(key: T): IndexPointer;
  keyword(key: string): IndexPointer;
  getValue<T>(): T;
  setValue<T>(v: T): void;
  set(v: ArrayBuffer): void;
  get(): ArrayBuffer: ArrayBuffer;
  lengthKey(): IndexPointer;
  length(): u32;
  getList(): Array<ArrayBuffer>;
  getListValues<T>(): Array<T>;
  extend(): IndexPointer;
  selectIndex(index: u32): IndexPointer;
  nullify(): void;
  pop(): ArrayBuffer;
  popValue<T>: ArrayBuffer;
  append(v: ArrayBuffer);
  appendValue<T>(v: T);
}
```

##### IndexPointer.wrap(pointer: ArrayBuffer)

Wraps a value as an IndexPointer. Under the hood this will just change the type of the ArrayBuffer to an IndexPointer.

```js
import { primitiveToBuffer } from "metashrew-as/assembly/utils/utils";
import { IndexPointer } from "metashrew-as/assembly/indexer/tables";
import { _flush } from "metashrew-as/assembly/indexer";

export function _start(): void {
  const pointer = IndexPointer.wrap(primitiveToBuffer(<u32>0x01010101)); // creates an IndexPointer to 0x01010101
  pointer.set(primitiveToBuffer<u32>(0x20202020)); // sets 0x01010101 -> 0x20202020
  _flush();
}
```

##### IndexPointer#unwrap()

Unwraps an IndexPointer back into an ArrayBuffer representation.

```js
import { primitiveToBuffer } from "metashrew-as/assembly/utils/utils";
import { IndexPointer } from "metashrew-as/assembly/indexer/tables";
import { Box } from "metashrew-as/assembly/utils/box";
import { console } from "metashrew-as/assembly/utils/logging";

export function _start(): void {
  const pointer = IndexPointer.wrap(primitiveToBuffer(<u32>0x0101010101));
  console.log(Box.from(pointer.unwrap()).toHexString()); // logs 0x0101010101
}
```

##### IndexPointer.for(keyword: string)

Converts `keyword` to an ArrayBuffer then wraps the ArrayBuffer as an IndexPointer. Useful for creating tables.

```js
import { primitiveToBuffer } from "metashrew-as/assembly/utils/utils";
import { IndexPointer } from "metashrew-as/assembly/indexer/tables";
import { console } from "metashrew-as/assembly/utils/logging";

export function _start(): void {
  const pointer = IndexPointer.for("/some-table/");
  pointer.set(primitiveToBuffer(<u32>0x0101010101));
}
```

##### IndexPointer#select(key: ArrayBuffer): IndexPointer

Grows the IndexPointer by appending another byte slice to the end of the ArrayBuffer it wraps.

```js
import { primitiveToBuffer } from "metashrew-as/assembly/utils/utils";
import { IndexPointer } from "metashrew-as/assembly/indexer/tables";
import { console } from "metashrew-as/assembly/utils/logging";
import { sha256 } from "metashrew-as/assembly/utils/sha256";

export function _start(): void {
  const pointer = IndexPointer.for("/value/by-txid/").select(sha256(String.UTF8.encode("test")));
  pointer.set(primitiveToBuffer(<u32>0x0101010101));
}
```

##### IndexPointer#selectValue<T>(v: T): IndexPointer

Converts argument to an ArrayBuffer and grows the key by that ArrayBuffer, returning a new IndexPointer.

```js
import { IndexPointer } from "metashrew-as/assembly/indexer/tables";
import { sha256 } from "metashrew-as/assembly/utils/sha256";

export function _start(): void {
  IndexPointer.for("/txid/number/").selectValue<u32>(1).set((sha256(String.UTF8.encode("test")));
}
```

##### IndexPointer#keyword(word: string): IndexPointer

Encodes `word` as UTF8 bytes and uses the ArrayBuffer to grow the key represented by the IndexPointer, returning a new IndexPointer.

```js
import { IndexPointer } from "metashrew-as/assembly/indexer/tables";
import { sha256 } from "metashrew-as/assembly/utils/sha256";

export function _start(): void {
  IndexPointer.for("/txid/number/").selectValue<u32>(1).keyword("/tag").set(sha256(String.UTF8.encode("test")));
}
```

##### IndexPointer#getValue<T>(): T

Reads the IndexPointer at the key it represents, then converts the result to type T. T must be a primitive type.

```js
import { IndexPointer } from "metashrew-as/assembly/indexer/tables";
import { console } from "metashrew-as/assembly/utils/logging";

export function _start(): void {
  const count = IndexPointer.for("/txid/count").getValue<u32>();
  console.log(count.toString(10));
}
```

##### IndexPointer#setValue<T>(v: T): void 

Sets a value of type T at the key represented by the IndexPointer.

```js
import { IndexPointer } from "metashrew-as/assembly/indexer/tables";

export function _start(): void {
  const pointer = IndexPointer.for("/txid/count");
  pointer.setValue<u32>(pointer.getValue<u32>() + 1);
}
```

##### IndexPointer#get(): ArrayBuffer

Gets the value stored at the key represented by the IndexPointer. The result is an ArrayBuffer type. Values that are set with IndexPointer are cached in-memory, but when reading out of the cache, the value is copied, so it can be mutated without affecting the call to `_flush()` at the end of the program run.

```js
import { IndexPointer } from "metashrew-as/assembly/indexer/tables";
import { Box } from "metashrew-as/assembly/utils/box";
import { console } from "metashrew-as/assembly/utils/logging";

export function _start(): void {
  const pointer = IndexPointer.for("/txid/last");
  console.log(Box.from(pointer.get()).toHexString());
}
```

##### IndexPointer#set(v: ArrayBuffer): void

Sets the value in the key-value database corresponding to the key represented by the IndexPointer. When inserting into the cache with `set` it will copy the bytes of its argument, so the value can potentially be mutated afterwards.

```js
import { IndexPointer } from "metashrew-as/assembly/indexer/tables";
import { Box } from "metashrew-as/assembly/utils/box";
import { console } from "metashrew-as/assembly/utils/logging";
import { sha256 } from "metashrew-as/assembly/utils/sha256";

export function _start(): void {
  IndexPointer.for("/txid/last").set(sha256(IndexPointer.for("/txid/first").get()));
}
```

##### IndexPointer#lengthKey(): IndexPointer

List operation:

Appends `"/length"` to the end of the key and returns a new IndexPointer. Useful for doing operations on the value representing the length of a list, to be used with `append` or `appendValue`.

```js
import { IndexPointer } from "metashrew-as/assembly/indexer/tables";
import { console } from "metashrew-as/assembly/utils/logging";

export function _start(): void {
  const length = IndexPointer.for("/txid/list").lengthKey().getValue<u32>();
  console.log(length.toString(10));
}
```

##### IndexPointer#length(): u32

List operation:

Calls `IndexPointer#lengthKey()` then `IndexPointer#getValue<u32>()` on the key it returns. Ultimately will produce the length of the list stored at the desired key.

```js
// same program as above, in effect
import { IndexPointer } from "metashrew-as/assembly/indexer/tables";
import { console } from "metashrew-as/assembly/utils/logging";

export function _start(): void {
  const length = IndexPointer.for("/txid/list").length();
  console.log(length.toString(10));
}
```


##### IndexPointer#getList(): Array<ArrayBuffer>

List operation:

Gets the list of values at the desired key. First it fetches the length of the list then called `IndexPointer#selectIndex(v: u32)` for each key, then gets the ArrayBuffer stored at that key.

```js
import { IndexPointer } from "metashrew-as/assembly/indexer/tables";
import { console } from "metashrew-as/assembly/utils/logging";

export function _start(): void {
  const values = IndexPointer.for("/txid/list").getList();
  values.forEach((v: ArrayBuffer, i: i32, ary: Array<ArrayBuffer>) => {
    console.log(Box.from(v).toHexString());
  });
}
```

##### IndexPointer#getListValues<T>(): Array<T>

List operation:

Gets the list of values at the desired key then converts to type `T`, which must be a primitive type.

```js
import { IndexPointer } from "metashrew-as/assembly/indexer/tables";
import { console } from "metashrew-as/assembly/utils/logging";

export function _start(): void {
  const values = IndexPointer.for("/txid/values").getListValues<u64>();
  values.forEach((v: u64, i: i32, ary: Array<u64>) => {
    console.log(v.toString(10));
  });
}
```

##### IndexPointer#extend(): IndexPointer

List operation:

Increases the length of the list by 1. Returns the IndexPointer at the newly created slot in the list.

```js
import { IndexPointer } from "metashrew-as/assembly/indexer/tables";
import { console } from "metashrew-as/assembly/utils/logging";

export function _start(): void {
  const pointer = IndexPointer.for("/txid/values");
  pointer.extend().setValue<u64>(1);
}
```

##### IndexPointer#selectIndex(index: u32): IndexPointer

List operation:

Selects the value in a list at `index`. Returns the IndexPointer at that slot.

```js
import { IndexPointer } from "metashrew-as/assembly/indexer/tables";
import { console } from "metashrew-as/assembly/utils/logging";

export function _start(): void {
  const pointer = IndexPointer.for("/txid/values");
  const first = IndexPointer.for("/txid/values").selectIndex(0);
  console.log(first.getValue<u64>().toString(10));
}
```

##### IndexPointer#nullify(): void

Deletes the value at the key represented by the IndexPointer.

```js
import { IndexPointer } from "metashrew-as/assembly/indexer/tables";

export function _start(): void {
  const pointer = IndexPointer.for("/some-key");
  pointer.nullify() // deletes the value at "/some-key"
}
```

##### IndexPointer#pop(): ArrayBuffer

List operation:

Pops the last value off of the list at the IndexPointer.

```js
import { IndexPointer } from "metashrew-as/assembly/indexer/tables";

export function _start(): void {
  const pointer = IndexPointer.for("/some-list");
  pointer.appendValue<u64>(10);
  pointer.appendValue<u64>(5);
  pointer.appendValue<u64>(2);
  const length = pointer.length(); // 3
  const last = pointer.pop(); // 0x0200000000000000
  console.log(pointer.length().toString(10)) // logs "2"
}
```

##### IndexPointer#popValue<T>(): T

List operation:

Pops the last value off of the list and converts it to type `T`, where `T` is a primitive.

```js
import { IndexPointer } from "metashrew-as/assembly/indexer/tables";

export function _start(): void {
  const pointer = IndexPointer.for("/some-list");
  pointer.appendValue<u64>(10);
  pointer.appendValue<u64>(5);
  pointer.appendValue<u64>(15);
  const length = pointer.length(); // 3
  const last = pointer.popValue<u64>(); // 15
  console.log(last.toString(10)) // logs "15"
}
```

##### IndexPointer#append(v: ArrayBuffer): void

List operation:

Appends an ArrayBuffer to the list of values at the key represented by the IndexPointer.

```js
import { IndexPointer } from "metashrew-as/assembly/indexer/tables";

export function _start(): void {
  const pointer = IndexPointer.for("/some-list");
  pointer.append(String.UTF8.encode("some value"));
  pointer.append(String.UTF8.encode("other value"));
  console.log(String.UTF8.decode(pointer.pop())) // logs "other value";
}
```

##### IndexPointer#appendValue<T>(v: T): void

List operation:

Appends a value of type `T` to the list of values at the key represented by the IndexPointer.

```js
import { IndexPointer } from "metashrew-as/assembly/indexer/tables";

export function _start(): void {
  const pointer = IndexPointer.for("/some-list");
  pointer.appendValue<u64>(10);
  pointer.appendValue<u64>(20);
  console.log(pointer.popValue<u64>().toString(10));
}
```


### assembly/indexer

#### get/set values

The following functions are abstracted by IndexPointer and should not be used directly, but are described here for context on how the underlying runtime works.

#### get(key: ArrayBuffer): ArrayBuffer

Gets a value from the index by its key. Multiple calls to the same key are cached, and calling the `set` function will update the cache as well, so it is acceptable to write programs that `set` then later `get` the value at the same key within the program run for the block.

```js
import { get } from "metashrew-as/assembly/indexer"

const KEY = String.UTF8.encode("/mykey");

export function lookup(): ArrayBuffer {
  return get(KEY);
}
```

#### set(key: ArrayBuffer, value: ArrayBuffer): void

Caches a value to be flushed to the write batch at the end of the program run. Even though no state changes are saved until the end of the program run, a call to `get` at any key will query the cache to lookup the value before it makes a call to the k/v store in the host environment.

```js
import { set, get, _flush } from "metashrew-as/assembly/indexer";

const KEY = String.UTF8.encode("/mykey");

export function setvalue(): void {
  set(KEY, String.UTF8.encode("hello");
}

export function lookup(): ArrayBuffer {
  return get(KEY);
}

export function _start(): void {
  setvalue();
  _flush();
}

// can call the metashrew-view JSONRPC with the "lookup" symbol to retrive the value, 
```

#### input(): ArrayBuffer

Reads the bytevector from the host environment containing a u32 value for the block height, followed by the bytes of the serialized block.


#### _flush(): void

It is required to call this function, only once, at the end of your `_start` function. Otherwise, no state changes will be saved to the index. Always put as the last line of your `_start` function.

```js

import { get, set, _flush, input } from "metashrew-as/assembly/indexer";
import { Box } from "metashrew-as/assembly/utils/box";
import { parsePrimitive } from "metashrew-as/assembly/utils/utils";

export function _start(): void {
  const data = input();
  const box = Box.from(data);
  const height = parsePrimitive<u32>(box);
  set(Box.concat([ Box.from(String.UTF8.encode("/block/")), Box.from(String.UTF8.encode(height.toString(10))); ]), box.toArrayBuffer());
  _flush();
  // trivial index sets UTF8 /block/n to the serialized block bytes, where n is 0..tip as
}
```

### assembly/indexer/bst

Binary Search Tree (BST) functions by inserts keys starting at the index pointer. BST efficiently supports operations like seekGreater and seekLower, which allow for searching the keyspace both up and down to find nearest neighbors.

#### BST

```js
abstract class BST {
  static at<K>(key: IndexPointer): BST<K>;
  getMaskPointer(partialKey: ArrayBuffer): IndexPointer;
  getMask(partialKey: ArrayBuffer): ArrayBuffer;
  markPath(key: K): void;
  unmarkPath(key: K): void;
  seekLower(start: K): K;
  seekGreater(start: K): K;
  set(k: K, v: ArrayBuffer): void;
  get(k: K): ArrayBuffer;
  nullify(k: K): void;
```

### assembly/indexer/widebst

#### BSTU128

```js
abstract class BSTU128 {
  static at(key: IndexPointer): BSTU128;
  getMaskPointer(partialKey: ArrayBuffer): IndexPointer;
  getMask(partialKey: ArrayBuffer): ArrayBuffer;
  markPath(key: u128): void;
  unmarkPath(key: u128): void;
  _findBoundaryFromPartial(keyBytes: ArrayBuffer, seekHigher: bool): u128;
  seekLower(start: u128): u128;
  seekGreater(start: u128): u128;
  set(key: u128, v: ArrayBuffer): void;
  get(key: u128): ArrayBuffer;
  nullify(key: u128): void;
```

## Utils

### assembly/utils/b32

#### bech32m

```js
export function bech32m(prefix: ArrayBuffer, words: Array<u8>): ArrayBuffer {
  return encode(prefix, words, ENCODING_CONST_BECH32M);
}
```

#### bech32

```js
export function bech32(prefix: ArrayBuffer, words: Array<u8>): ArrayBuffer {
  return encode(prefix, words, ENCODING_CONST_BECH32);
}
````

### assembly/utils/box

#### Box

```js
abstract class Box {
  toHexString(): string;
  toHexUTF8(): ArrayBuffer;
  static concat(data: Array<Box>): ArrayBuffer;
  shift(): Box;
  sliceFrom(start: usize): Box;
  sliceTo(ptr: usize): Box;
  shrinkFront(distance: usize): Box;
  growFront(distance: usize): Box;
  shrinkBack(distance: usize): Box;
  growBack(distance: usize): Box;
  setLength(len: usize): Box;
  toArrayBuffer(): ArrayBuffer;
  isEmpty(): boolean;
  static from(data: ArrayBuffer): Box;
  static copy(data: ArrayBuffer): Box;
  static freeCopy(v: Box): void;
  static fromTyped<T>(v: T): Box;
```

### assembly/utils/network

```js
abstract class Network {
  messagePrefix: string;
  bech32: string;
  bip32: Bip32;
  pubKeyHash: number;
  scriptHash: number;
  wif: number;
```

### assembly/utils/sha256

```js
function sha256d(data: ArrayBuffer): ArrayBuffer
```

### Logging

#### assembly/utils/logging

```js
import { console } from "metashrew-as/assembly/utils/logging";

export function _start(): void {
  console.log("visible from tests or metashrew itself");
  console.logUTF8(String.UTF8.encode("sometimes we have UTF8 too and we can avoid decoding it needlessly"));
}
```

### Network

#### network

The `network` namespace is exported from `metashrew-as/assembly/blockdata/network` and exposes the following API:

```js

namespace network {
  interface Network {
    static fromTriple(p2pkhPrefix: u8, p2shPrefix: u8, bech32Prefix: string): Network;
    static get MAINNET(): Network;
    static get REGTEST(): Network;
  }
  setNetwork(network: Network): void;
  getNetwork(): Network;
}
```


## Author

Sandshrew Inc


## License

MIT
