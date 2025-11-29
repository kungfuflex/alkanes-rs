import { Box } from "./utils/box";
/**
 * StorageMap - matches Rust alkanes-support/src/storage.rs
 * 
 * Serialization format:
 * - count (u32)
 * - For each entry:
 *   - key_length (u32)
 *   - key_bytes
 *   - value_length (u32)
 *   - value_bytes
 */

export class StorageEntry {
  constructor(
    public key: ArrayBuffer,
    public value: ArrayBuffer
  ) {}
}

export class StorageMap {
  entries: Array<StorageEntry>;

  constructor() {
    this.entries = new Array<StorageEntry>();
  }

  /**
   * Set a key-value pair
   */
  set(key: ArrayBuffer, value: ArrayBuffer): void {
    // Check if key already exists and update it
    for (let i = 0; i < this.entries.length; i++) {
      if (this.buffersEqual(this.entries[i].key, key)) {
        this.entries[i].value = value;
        return;
      }
    }
    // Key doesn't exist, add new entry
    this.entries.push(new StorageEntry(key, value));
  }

  /**
   * Get a value by key
   */
  get(key: ArrayBuffer): ArrayBuffer | null {
    for (let i = 0; i < this.entries.length; i++) {
      if (this.buffersEqual(this.entries[i].key, key)) {
        return this.entries[i].value;
      }
    }
    return null;
  }

  /**
   * Check if two ArrayBuffers are equal
   */
  private buffersEqual(a: ArrayBuffer, b: ArrayBuffer): bool {
    if (a.byteLength != b.byteLength) return false;
    
    const ptrA = changetype<usize>(a);
    const ptrB = changetype<usize>(b);
    
    for (let i = 0; i < a.byteLength; i++) {
      if (load<u8>(ptrA + i) != load<u8>(ptrB + i)) {
        return false;
      }
    }
    return true;
  }

  /**
   * Calculate serialized size in bytes
   */
  calculateSize(): i32 {
    let size: i32 = 4; // count (u32)
    
    for (let i = 0; i < this.entries.length; i++) {
      size += 4; // key_length
      size += this.entries[i].key.byteLength;
      size += 4; // value_length
      size += this.entries[i].value.byteLength;
    }
    
    return size;
  }

  /**
   * Serialize to ArrayBuffer matching Rust format
   */
  serialize(): ArrayBuffer {
    const sz = new ArrayBuffer(4);
    store<u32>(changetype<usize>(sz), <i32>this.entries.length);
    const mappingSerialized = Box.concat(this.entries.map<Box>((v: StorageEntry, i: i32, ary: Array<StorageEntry>) => {
      const ksz = new ArrayBuffer(4);
      store<u32>(changetype<usize>(ksz), <i32>v.key.byteLength);
      const vsz = new ArrayBuffer(4);
      store<u32>(changetype<usize>(vsz), <i32>v.value.byteLength);
      return Box.from(Box.concat([Box.from(ksz), Box.from(v.key), Box.from(vsz), Box.from(v.value)]));
    }));
    return Box.concat([Box.from(sz), Box.from(mappingSerialized)]);
  }

  /**
   * Parse from ArrayBuffer (for testing)
   */
  static parse(data: ArrayBuffer): StorageMap {
    const map = new StorageMap();
    let ptr = changetype<usize>(data);
    const endPtr = ptr + data.byteLength;
    
    // Read count
    const count = load<u32>(ptr);
    ptr += 4;
    
    // Read each entry
    for (let i: u32 = 0; i < count; i++) {
      // key_length
      const keyLen = load<u32>(ptr);
      ptr += 4;
      
      // key_bytes
      const key = new ArrayBuffer(keyLen as i32);
      memory.copy(changetype<usize>(key), ptr, keyLen);
      ptr += keyLen;
      
      // value_length
      const valueLen = load<u32>(ptr);
      ptr += 4;
      
      // value_bytes
      const value = new ArrayBuffer(valueLen as i32);
      memory.copy(changetype<usize>(value), ptr, valueLen);
      ptr += valueLen;
      
      map.entries.push(new StorageEntry(key, value));
    }
    
    return map;
  }
}
