// Core Alkanes types

import { writeU128, writeU64, writeU32, readU128, readU64, getDataPtr } from "./arraybuffer";

/**
 * Alkane identifier (block:tx pair)
 */
export class AlkaneId {
  constructor(
    public block: u128,
    public tx: u128
  ) {}

  /**
   * Serialize to bytes: [block(16)][tx(16)]
   */
  toBytes(): ArrayBuffer {
    const buf = new ArrayBuffer(32 + 4); // 32 bytes data + 4 byte length prefix
    const ptr = changetype<usize>(buf) + 4;
    
    writeU128(ptr, this.block);
    writeU128(ptr + 16, this.tx);
    
    // Write length prefix
    store<u32>(changetype<usize>(buf), 32);
    
    return buf;
  }

  /**
   * Deserialize from bytes
   */
  static fromBytes(data: ArrayBuffer, offset: u32 = 0): AlkaneId {
    const ptr = changetype<usize>(data) + offset;
    return new AlkaneId(
      readU128(ptr),
      readU128(ptr + 16)
    );
  }
}

/**
 * Cellpack for calling alkanes
 * Layout: [target_block(16)][target_tx(16)][inputs...]
 */
export class Cellpack {
  constructor(
    public target: AlkaneId,
    public inputs: u128[]
  ) {}

  /**
   * Serialize cellpack with ArrayBuffer layout
   * @returns ArrayBuffer with [length(4)][data...] layout
   */
  toArrayBuffer(): ArrayBuffer {
    const dataSize = 32 + (this.inputs.length * 16); // target + inputs
    const buf = new ArrayBuffer(dataSize + 4);
    const dataPtr = changetype<usize>(buf) + 4;
    
    // Write length prefix
    store<u32>(changetype<usize>(buf), dataSize);
    
    // Write target
    writeU128(dataPtr, this.target.block);
    writeU128(dataPtr + 16, this.target.tx);
    
    // Write inputs
    for (let i = 0; i < this.inputs.length; i++) {
      writeU128(dataPtr + 32 + (i * 16), this.inputs[i]);
    }
    
    return buf;
  }

  /**
   * Get data pointer (for passing to __staticcall)
   */
  getDataPtr(): usize {
    return getDataPtr(this.toArrayBuffer());
  }
}

/**
 * Empty AlkaneTransferParcel
 * Format: [count(16)]
 */
export class EmptyAlkaneParcel {
  static toArrayBuffer(): ArrayBuffer {
    const buf = new ArrayBuffer(16 + 4); // 16 bytes data + 4 length
    const ptr = changetype<usize>(buf);
    
    // Write length = 16
    store<u32>(ptr, 16);
    
    // Write count = 0
    writeU128(ptr + 4, 0);
    
    return buf;
  }

  static getDataPtr(): usize {
    return getDataPtr(EmptyAlkaneParcel.toArrayBuffer());
  }
}

/**
 * Empty StorageMap
 * Format: [count(4)] - Note: u32 not u128!
 */
export class EmptyStorageMap {
  static toArrayBuffer(): ArrayBuffer {
    const buf = new ArrayBuffer(4 + 4); // 4 bytes data + 4 length
    const ptr = changetype<usize>(buf);
    
    // Write length = 4
    store<u32>(ptr, 4);
    
    // Write count = 0 as u32
    store<u32>(ptr + 4, 0);
    
    return buf;
  }

  static getDataPtr(): usize {
    return getDataPtr(EmptyStorageMap.toArrayBuffer());
  }
}

/**
 * CallResponse from staticcall
 * Format: [AlkaneTransferParcel][data...]
 */
export class CallResponse {
  constructor(
    public data: ArrayBuffer
  ) {}

  /**
   * Skip the AlkaneTransferParcel header to get to actual data
   * @returns Offset where data starts
   */
  skipAlkaneParcel(): u32 {
    const ptr = changetype<usize>(this.data);
    
    // Read transfer count (first u128)
    const count = readU128(ptr);
    
    // Skip: count(16) + transfers(count * 48)
    return 16 + (count as u32) * 48;
  }

  /**
   * Get pointer to data after AlkaneTransferParcel
   */
  getDataPtr(): usize {
    return changetype<usize>(this.data) + this.skipAlkaneParcel();
  }

  /**
   * Get data as ArrayBuffer (after skipping AlkaneTransferParcel)
   */
  getData(): ArrayBuffer {
    const offset = this.skipAlkaneParcel();
    const dataSize = this.data.byteLength - offset;
    
    const result = new ArrayBuffer(dataSize);
    memory.copy(
      changetype<usize>(result),
      changetype<usize>(this.data) + offset,
      dataSize
    );
    
    return result;
  }
}

/**
 * Extended CallResponse for tx-script output
 * Format: [alkanes_count(16)][storage_count(16)][data...]
 */
export class ExtendedCallResponse {
  data: ArrayBuffer;
  offset: u32;

  constructor(initialSize: i32 = 1024) {
    // Allocate with length prefix
    this.data = new ArrayBuffer(initialSize + 4);
    
    // Write initial length (will update later)
    store<u32>(changetype<usize>(this.data), 0);
    
    // Start offset after length prefix
    this.offset = 4;
    
    // Write empty alkanes count
    this.writeU128(0);
    
    // Write empty storage count
    this.writeU128(0);
  }

  /**
   * Write u128 at current offset and advance
   */
  writeU128(value: u128): void {
    this.ensureCapacity(16);
    writeU128(changetype<usize>(this.data) + this.offset, value);
    this.offset += 16;
  }

  /**
   * Write u64 at current offset and advance
   */
  writeU64(value: u64): void {
    this.ensureCapacity(8);
    writeU64(changetype<usize>(this.data) + this.offset, value);
    this.offset += 8;
  }

  /**
   * Write u32 at current offset and advance
   */
  writeU32(value: u32): void {
    this.ensureCapacity(4);
    writeU32(changetype<usize>(this.data) + this.offset, value);
    this.offset += 4;
  }

  /**
   * Write bytes at current offset and advance
   */
  writeBytes(src: ArrayBuffer): void {
    const len = src.byteLength;
    this.ensureCapacity(len);
    
    memory.copy(
      changetype<usize>(this.data) + this.offset,
      changetype<usize>(src),
      len
    );
    
    this.offset += len;
  }

  /**
   * Ensure buffer has enough capacity
   */
  private ensureCapacity(needed: u32): void {
    const available = this.data.byteLength - this.offset;
    if (available < needed) {
      // Grow buffer
      const newSize = this.data.byteLength * 2;
      const newData = new ArrayBuffer(newSize);
      memory.copy(
        changetype<usize>(newData),
        changetype<usize>(this.data),
        this.offset
      );
      this.data = newData;
    }
  }

  /**
   * Finalize response and get pointer to data
   */
  finalize(): usize {
    // Write final length (offset - 4 for length prefix)
    store<u32>(changetype<usize>(this.data), this.offset - 4);
    
    // Return pointer to data (skip length prefix)
    return changetype<usize>(this.data) + 4;
  }
}
