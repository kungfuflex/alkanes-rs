import { loadU128, storeU128, u128 } from "./u128";

export class AlkaneId {
  constructor(
    public block: u128,
    public tx: u128
  ) {}

  /**
   * Serialize to ArrayBuffer [block(16)][tx(16)]
   */
  serialize(): ArrayBuffer {
    const result = new ArrayBuffer(32);
    const ptr = changetype<usize>(result);
    storeU128(ptr, this.block);
    storeU128(ptr + 16, this.tx);
    return result;
  }

  /**
   * Parse from ArrayBuffer
   */
  static parse(data: ArrayBuffer, offset: i32 = 0): AlkaneId {
    const ptr = changetype<usize>(data) + offset;
    const block = loadU128(ptr);
    const tx = loadU128(ptr + 16);
    return new AlkaneId(block, tx);
  }
}

export class AlkaneTransfer {
  constructor(
    public id: AlkaneId,
    public value: u128
  ) {}

  /**
   * Serialize to ArrayBuffer [id.block(16)][id.tx(16)][value(16)]
   */
  serialize(): ArrayBuffer {
    const result = new ArrayBuffer(48);
    const ptr = changetype<usize>(result);
    
    storeU128(ptr, this.id.block);
    storeU128(ptr + 16, this.id.tx);
    storeU128(ptr + 32, this.value);
    
    return result;
  }

  /**
   * Parse from ArrayBuffer
   */
  static parse(data: ArrayBuffer, offset: i32 = 0): AlkaneTransfer {
    const ptr = changetype<usize>(data) + offset;
    const id = AlkaneId.parse(data, offset);
    const value = loadU128(ptr + 32);
    return new AlkaneTransfer(id, value);
  }
}

export class AlkaneTransferParcel {
  transfers: AlkaneTransfer[];

  constructor() {
    this.transfers = [];
  }

  /**
   * Add a transfer to the parcel
   */
  add(transfer: AlkaneTransfer): void {
    this.transfers.push(transfer);
  }

  /**
   * Serialize to ArrayBuffer matching Rust format
   * Manual implementation to avoid Box.concat with callbacks (stub runtime compatibility)
   */
  serialize(): ArrayBuffer {
    // Calculate total size: count (16 bytes) + transfers (48 bytes each)
    const totalSize = 16 + (this.transfers.length * 48);
    const result = new ArrayBuffer(totalSize);
    const resultPtr = changetype<usize>(result);
    
    // Write count
    storeU128(resultPtr, u128.from(this.transfers.length));
    
    // Write each transfer
    let offset: usize = 16;
    for (let i = 0; i < this.transfers.length; i++) {
      const transfer = this.transfers[i];
      storeU128(resultPtr + offset, transfer.id.block);
      storeU128(resultPtr + offset + 16, transfer.id.tx);
      storeU128(resultPtr + offset + 32, transfer.value);
      offset += 48;
    }
    
    return result;
  }

  /**
   * Parse from ArrayBuffer (stub for compatibility)
   * Not used in tx-scripts - only serialization is needed
   */
  static parse(data: ArrayBuffer): AlkaneTransferParcel {
    return new AlkaneTransferParcel();
  }
}
