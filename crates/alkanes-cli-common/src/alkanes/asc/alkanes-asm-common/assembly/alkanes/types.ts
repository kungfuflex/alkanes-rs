// Core Alkanes types using native ArrayBuffer layout
import { u128 } from "../u128";
import { AlkaneTransferParcel, AlkaneTransfer, AlkaneId } from "../parcel";
import { StorageMap } from "../storage-map";
import { storeU128, loadU128 } from "./utils";
import { Box } from "../utils/box";

// Re-export core types
export { AlkaneId, AlkaneTransfer, AlkaneTransferParcel };

/**
 * Cellpack for calling alkanes
 * Format: [target_block(16)][target_tx(16)][inputs...]
 * 
 * NOTE: Stores inputs as buffer instead of Array for stub runtime compatibility
 */
export class Cellpack {
  private inputsBuffer: ArrayBuffer;
  private inputsCount: i32;

  constructor(
    public target: AlkaneId,
    inputsBuffer: ArrayBuffer | null = null,
    inputsCount: i32 = 0
  ) {
    if (inputsBuffer == null) {
      this.inputsBuffer = new ArrayBuffer(0);
      this.inputsCount = 0;
    } else {
      this.inputsBuffer = inputsBuffer;
      this.inputsCount = inputsCount;
    }
  }

  /**
   * Create Cellpack with single input (common case for opcodes)
   */
  static withSingleInput(target: AlkaneId, input: u128): Cellpack {
    const inputsBuffer = new ArrayBuffer(16);
    storeU128(changetype<usize>(inputsBuffer), input);
    return new Cellpack(target, inputsBuffer, 1);
  }

  toArrayBuffer(): ArrayBuffer {
    const size = 32 + this.inputsBuffer.byteLength;
    const buf = new ArrayBuffer(size);
    const ptr = changetype<usize>(buf);
    
    // Write target
    storeU128(ptr, this.target.block);
    storeU128(ptr + 16, this.target.tx);
    
    // Copy inputs buffer
    const inputsPtr = changetype<usize>(this.inputsBuffer);
    const inputsSize = this.inputsBuffer.byteLength as usize;
    for (let i: usize = 0; i < inputsSize; i++) {
      store<u8>(ptr + 32 + i, load<u8>(inputsPtr + i));
    }
    
    return buf;
  }
}

/**
 * CallResponse from a staticcall
 * Format: [AlkaneTransferParcel][data...]
 */
export class CallResponse {
  constructor(
    public alkaneTransfers: AlkaneTransferParcel,
    public data: ArrayBuffer
  ) {}

  static fromBytes(data: ArrayBuffer): CallResponse {
    const ptr = changetype<usize>(data);
    
    // Read AlkaneTransferParcel count
    const count = loadU128(ptr);
    
    // Calculate offset to skip AlkaneTransferParcel (count + transfers)
    const countU64 = count.toU64();
    const parcelSize = 16 + (countU64 as i32 * 48);
    
    // Extract the response data after the parcel using manual copy (stub runtime compatible)
    const responseSize = data.byteLength - parcelSize;
    const responseData = new ArrayBuffer(responseSize);
    const responsePtr = changetype<usize>(responseData);
    const srcPtr = ptr + parcelSize;
    const responseSizeUsize = responseSize as usize;
    
    for (let i: usize = 0; i < responseSizeUsize; i++) {
      store<u8>(responsePtr + i, load<u8>(srcPtr + i));
    }
    
    // Parse the parcel (stub implementation, returns empty)
    const parcel = AlkaneTransferParcel.parse(data);
    
    return new CallResponse(parcel, responseData);
  }
}

/**
 * ExtendedCallResponse - for tx-script output
 * 
 * Matches Rust alkanes-support/src/response.rs ExtendedCallResponse::serialize()
 * 
 * Serialization format:
 * 1. Alkanes section (AlkaneTransferParcel):
 *    - count (u128 = 16 bytes)
 *    - For each transfer: block (u128), tx (u128), value (u128) = 48 bytes each
 * 2. Storage section (StorageMap):
 *    - count (u32 = 4 bytes)
 *    - For each entry: key_len (u32), key_bytes, value_len (u32), value_bytes
 * 3. Data section:
 *    - Arbitrary bytes
 * 
 * Uses our proven StorageMap and AlkaneTransferParcel classes.
 */
export class ExtendedCallResponse {
  alkanes: AlkaneTransferParcel;
  storage: StorageMap;
  data: ArrayBuffer;

  constructor() {
    this.alkanes = new AlkaneTransferParcel();
    this.storage = new StorageMap();
    this.data = new ArrayBuffer(0);
  }

  /**
   * Add an alkane transfer
   */
  addAlkaneTransfer(block: u128, tx: u128, value: u128): void {
    const id = new AlkaneId(block, tx);
    const transfer = new AlkaneTransfer(id, value);
    this.alkanes.pay(transfer);
  }

  /**
   * Set a storage entry
   */
  setStorage(key: ArrayBuffer, value: ArrayBuffer): void {
    this.storage.set(key, value);
  }

  /**
   * Set the data section
   */
  setData(data: ArrayBuffer): void {
    this.data = data;
  }

  /**
   * Append to data section
   */
  appendData(moreData: ArrayBuffer): void {
    if (this.data.byteLength == 0) {
      this.data = moreData;
    } else {
      // Need to concatenate
      const oldLen = this.data.byteLength;
      const newLen = oldLen + moreData.byteLength;
      const combined = new ArrayBuffer(newLen);
      
      memory.copy(
        changetype<usize>(combined),
        changetype<usize>(this.data),
        oldLen
      );
      memory.copy(
        changetype<usize>(combined) + oldLen,
        changetype<usize>(moreData),
        moreData.byteLength
      );
      
      this.data = combined;
    }
  }

  /**
   * Write u128 to data section
   */
  writeU128(value: u128): void {
    const buf = new ArrayBuffer(16);
    storeU128(changetype<usize>(buf), value);
    this.appendData(buf);
  }

  /**
   * Finalize and serialize to ArrayBuffer
   * 
   * Matches Rust ExtendedCallResponse::serialize() exactly:
   * ```rust
   * result.extend(alkanes.serialize())
   * result.extend(storage.serialize())
   * result.extend(data)
   * ```
   * 
   * Returns: ArrayBuffer ready to return from tx-script
   */
  finalize(): ArrayBuffer {
    // Serialize each section using our proven classes
    const alkanesBytes = this.alkanes.serialize();
    const storageBytes = this.storage.serialize();
    
    // Manual concat without Box.concat to avoid function table issues with stub runtime
    const totalLen = alkanesBytes.byteLength + storageBytes.byteLength + this.data.byteLength;
    const result = new ArrayBuffer(totalLen);
    const resultPtr = changetype<usize>(result);
    
    // Copy alkanes bytes
    const alkanesPtr = changetype<usize>(alkanesBytes);
    const alkanesLen = alkanesBytes.byteLength as usize;
    for (let i: usize = 0; i < alkanesLen; i++) {
      store<u8>(resultPtr + i, load<u8>(alkanesPtr + i));
    }
    
    // Copy storage bytes
    const storagePtr = changetype<usize>(storageBytes);
    const storageOffset = alkanesLen;
    const storageLen = storageBytes.byteLength as usize;
    for (let i: usize = 0; i < storageLen; i++) {
      store<u8>(resultPtr + storageOffset + i, load<u8>(storagePtr + i));
    }
    
    // Copy data bytes
    if (this.data.byteLength > 0) {
      const dataPtr = changetype<usize>(this.data);
      const dataOffset = alkanesLen + storageLen;
      const dataLen = this.data.byteLength as usize;
      for (let i: usize = 0; i < dataLen; i++) {
        store<u8>(resultPtr + dataOffset + i, load<u8>(dataPtr + i));
      }
    }
    
    return result;
  }
}
