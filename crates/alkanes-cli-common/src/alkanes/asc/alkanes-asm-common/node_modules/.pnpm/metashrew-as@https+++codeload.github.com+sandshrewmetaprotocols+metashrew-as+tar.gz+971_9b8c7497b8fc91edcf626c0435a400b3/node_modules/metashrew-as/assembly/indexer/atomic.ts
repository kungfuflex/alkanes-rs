import { IndexPointer } from "./tables";
import { hash } from "./index";
import { console } from "../utils/logging";

export class AtomicTransaction {
  temp: Map<string, ArrayBuffer>;
  tempKeys: Map<string, ArrayBuffer>;
  saved: Map<string, ArrayBuffer>;
  savedKeys: Map<string, ArrayBuffer>;

  constructor() {
    this.temp = new Map<string, ArrayBuffer>();
    this.tempKeys = new Map<string, ArrayBuffer>();
    this.saved = new Map<string, ArrayBuffer>();
    this.savedKeys = new Map<string, ArrayBuffer>();
  }

  checkpoint(): void {
    const keys = this.temp.keys();
    for (let i = 0; i < keys.length; i++) {
      this.saved.set(keys[i], this.temp.get(keys[i]));
      this.savedKeys.set(keys[i], this.tempKeys.get(keys[i]));
    }
  }

  commit(): void {
    const keys = this.savedKeys.keys();
    console.log(keys.length.toString());
    for (let i = 0; i < keys.length; i++) {
      IndexPointer.wrap(this.savedKeys.get(keys[i])).set(
        this.saved.get(keys[i])
      );
    }

    this.saved.clear();
  }

  rollback(): void {
    this.temp.clear();
  }

  set(_key: ArrayBuffer, value: ArrayBuffer): void {
    const key = hash(_key);
    this.temp.set(key, value);
    this.tempKeys.set(key, _key);
  }

  get(_key: ArrayBuffer): ArrayBuffer {
    const key = hash(_key);
    if (this.temp.has(key)) {
      return this.temp.get(key);
    }
    return changetype<ArrayBuffer>(0);
  }

  setValue<T extends number>(key: ArrayBuffer, value: T): void {
    const container = new ArrayBuffer(sizeof<T>());
    store<T>(changetype<usize>(container), value);
    this.set(key, container);
  }

  getValue<T>(key: ArrayBuffer): T {
    const value = this.get(key);
    const container: ArrayBuffer = new ArrayBuffer(sizeof<T>());
    memory.copy(
      changetype<usize>(container),
      changetype<usize>(value),
      sizeof<T>()
    );
    return load<T>(changetype<usize>(container));
  }

  setKeyword(key: string, value: ArrayBuffer): void {
    const container = String.UTF8.encode(key);
    this.set(container, value);
  }

  getKeyword(key: string): ArrayBuffer {
    const container = String.UTF8.encode(key);
    return this.get(container);
  }
  nullify(key: ArrayBuffer): void {
    this.set(key, new ArrayBuffer(0));
  }
  rollbackKey(_key: ArrayBuffer): void {
    const key = hash(_key);
    if (this.temp.has(key)) {
      this.temp.delete(key);
      this.tempKeys.delete(key);
    }
  }
  has(key: ArrayBuffer): bool {
    return this.temp.has(hash(key));
  }
  nullifyIndexPointerList(ptr: IndexPointer): void {
    for (let i = 0; i < ptr.length(); i++) {
      this.nullify(ptr.selectIndex(i).unwrap());
    }
    this.nullify(ptr.lengthKey().unwrap());
  }
  extendIndexPointerList(ptr: IndexPointer): ArrayBuffer {
    let length: u32;
    const lengthKey = ptr.lengthKey();
    if (this.has(lengthKey.unwrap())) {
      length = this.getValue<u32>(lengthKey.unwrap());
    } else {
      length = lengthKey.getValue<u32>();
    }
    this.setValue<u32>(lengthKey.unwrap(), length + 1);
    return ptr.selectIndex(length).unwrap();
  }
  appendIndexPointerList(ptr: IndexPointer, value: ArrayBuffer): void {
    this.set(this.extendIndexPointerList(ptr), value);
  }
}
