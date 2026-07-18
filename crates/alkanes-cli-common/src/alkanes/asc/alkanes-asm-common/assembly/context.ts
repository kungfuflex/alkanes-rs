// Context loading and parsing utilities

import { __request_context, __load_context } from "./runtime";
import { readU128 } from "./arraybuffer";
import { AlkaneId } from "./types";

/**
 * Execution context from alkanes runtime
 */
export class ExecutionContext {
  myself: AlkaneId;
  caller: AlkaneId;
  vout: u128;
  incoming_alkanes_count: u128;
  inputs: u128[];

  constructor() {
    this.myself = new AlkaneId(0, 0);
    this.caller = new AlkaneId(0, 0);
    this.vout = 0;
    incoming_alkanes_count = 0;
    this.inputs = [];
  }

  /**
   * Load context from runtime
   * Context layout: [myself(32)][caller(32)][vout(16)][incoming_alkanes_count(16)][inputs...]
   */
  static load(): ExecutionContext {
    const ctx = new ExecutionContext();
    
    // Request context size
    const size = __request_context();
    
    // Allocate buffer with length prefix
    const buf = new ArrayBuffer(size + 4);
    const bufPtr = changetype<usize>(buf);
    
    // Write length
    store<u32>(bufPtr, size);
    
    // Load context data
    __load_context(bufPtr + 4);
    
    // Parse context
    let offset: u32 = 4; // Skip length prefix
    
    // Read myself (32 bytes)
    ctx.myself = AlkaneId.fromBytes(buf, offset);
    offset += 32;
    
    // Read caller (32 bytes)
    ctx.caller = AlkaneId.fromBytes(buf, offset);
    offset += 32;
    
    // Read vout (16 bytes)
    ctx.vout = readU128(bufPtr + offset);
    offset += 16;
    
    // Read incoming_alkanes_count (16 bytes)
    ctx.incoming_alkanes_count = readU128(bufPtr + offset);
    offset += 16;
    
    // Read inputs (rest of buffer, each 16 bytes)
    const inputCount = (size - offset + 4) / 16;
    ctx.inputs = new Array<u128>(inputCount as i32);
    
    for (let i = 0; i < inputCount; i++) {
      ctx.inputs[i] = readU128(bufPtr + offset);
      offset += 16;
    }
    
    return ctx;
  }

  /**
   * Get input at index (0-based)
   */
  getInput(index: i32): u128 {
    if (index < 0 || index >= this.inputs.length) {
      return 0;
    }
    return this.inputs[index];
  }

  /**
   * Get input as u64
   */
  getInputU64(index: i32): u64 {
    return this.getInput(index) as u64;
  }

  /**
   * Get input as u32
   */
  getInputU32(index: i32): u32 {
    return this.getInput(index) as u32;
  }
}
