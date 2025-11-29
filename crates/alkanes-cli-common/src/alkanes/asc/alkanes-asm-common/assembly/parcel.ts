/**
 * AlkaneTransferParcel - matches Rust alkanes-support/src/parcel.rs
 * 
 * Serialization format:
 * - count (u128 = 16 bytes)
 * - For each transfer:
 *   - id.block (u128 = 16 bytes)
 *   - id.tx (u128 = 16 bytes)
 *   - value (u128 = 16 bytes)
 */

import { u128 } from "as-bignum/assembly";
import { storeU128, loadU128 } from "./alkanes/utils";
import { u128ToArrayBuffer } from "./utils";
import { Box } from "./utils/box";

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
  transfers: Array<AlkaneTransfer>;

  constructor() {
    this.transfers = new Array<AlkaneTransfer>();
  }

  /**
   * Add a transfer to the parcel
   */
  pay(transfer: AlkaneTransfer): void {
    this.transfers.push(transfer);
  }

  /**
   * Calculate serialized size in bytes
   */
  calculateSize(): i32 {
    return 16 + (this.transfers.length * 48); // count(16) + transfers(48 each)
  }

  /**
   * Serialize to ArrayBuffer matching Rust format
   */
  serialize(): ArrayBuffer {
    const transfers = Box.concat(this.transfers.map<Box>((v: AlkaneTransfer, i: i32, ary: Array<AlkaneTransfer>) => {
      return Box.from(v.serialize());
    }));
    return Box.concat([Box.from(u128ToArrayBuffer(u128.from(this.transfers.length))), Box.from(transfers)])
    
    return result;
  }

  /**
   * Parse from ArrayBuffer (for testing)
   */
  static parse(data: ArrayBuffer): AlkaneTransferParcel {
    const parcel = new AlkaneTransferParcel();
    let ptr = changetype<usize>(data);
    
    // Read count
    const count = loadU128(ptr);
    ptr += 16;
    
    // Read each transfer
    for (let i = u128.Zero; u128.lt(i, count); i = u128.add(i, u128.One)) {
      const block = loadU128(ptr);
      ptr += 16;
      
      const tx = loadU128(ptr);
      ptr += 16;
      
      const value = loadU128(ptr);
      ptr += 16;
      
      parcel.transfers.push(new AlkaneTransfer(new AlkaneId(block, tx), value));
    }
    
    return parcel;
  }
}
