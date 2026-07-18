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
   * Manual implementation to avoid Box.concat with callbacks (stub runtime compatibility)
   */
  serialize(): ArrayBuffer {
    // Calculate total size: count (4 bytes) + entries (4 + key.len + 4 + value.len each)
    let totalSize: usize = 4; // count
    for (let i = 0; i < this.entries.length; i++) {
      const entry = this.entries[i];
      totalSize += 4 + entry.key.byteLength + 4 + entry.value.byteLength;
    }
    
    const result = new ArrayBuffer(totalSize as i32);
    const resultPtr = changetype<usize>(result);
    
    // Write count
    store<u32>(resultPtr, this.entries.length as u32);
    
    // Write each entry
    let offset: usize = 4;
    for (let i = 0; i < this.entries.length; i++) {
      const entry = this.entries[i];
      
      // Write key length
      store<u32>(resultPtr + offset, entry.key.byteLength as u32);
      offset += 4;
      
      // Write key bytes
      const keyPtr = changetype<usize>(entry.key);
      const keyLen = entry.key.byteLength as usize;
      for (let j: usize = 0; j < keyLen; j++) {
        store<u8>(resultPtr + offset + j, load<u8>(keyPtr + j));
      }
      offset += keyLen;
      
      // Write value length
      store<u32>(resultPtr + offset, entry.value.byteLength as u32);
      offset += 4;
      
      // Write value bytes
      const valuePtr = changetype<usize>(entry.value);
      const valueLen = entry.value.byteLength as usize;
      for (let j: usize = 0; j < valueLen; j++) {
        store<u8>(resultPtr + offset + j, load<u8>(valuePtr + j));
      }
      offset += valueLen;
    }
    
    return result;
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
