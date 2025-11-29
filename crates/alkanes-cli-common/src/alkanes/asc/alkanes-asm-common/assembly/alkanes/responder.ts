// AlkaneResponder - High-level abstraction for alkanes runtime
import { 
  __request_context, __load_context, __staticcall, __returndatacopy,
  __call, __delegatecall, __log, __sequence, __fuel, __height,
  __request_storage, __load_storage, __balance,
  __request_transaction, __load_transaction, __request_block, __load_block
} from "./runtime";
import { AlkaneId, Cellpack, AlkaneTransferParcel, CallResponse, ExtendedCallResponse } from "./types";
import { StorageMap } from "../storage-map";
import { u128 } from "as-bignum/assembly";

/**
 * ExecutionContext parsed from runtime
 * Layout: [myself(32)][caller(32)][vout(16)][incoming_alkanes_count(16)][inputs...]
 */
export class ExecutionContext {
  myself: AlkaneId;
  caller: AlkaneId;
  vout: u128;
  incomingAlkanesCount: u128;
  inputs: u128[];

  constructor(
    myself: AlkaneId,
    caller: AlkaneId,
    vout: u128,
    incomingAlkanesCount: u128,
    inputs: u128[]
  ) {
    this.myself = myself;
    this.caller = caller;
    this.vout = vout;
    this.incomingAlkanesCount = incomingAlkanesCount;
    this.inputs = inputs;
  }

  /**
   * Load context from runtime
   */
  static load(): ExecutionContext {
    // Request context size
    const size = __request_context();
    
    // Allocate ArrayBuffer (automatically has length prefix at -4)
    const buf = new ArrayBuffer(size);
    const ptr = changetype<usize>(buf);
    
    // Load context data (runtime will read length from ptr-4)
    __load_context(changetype<i32>(ptr));
    
    // Parse context
    const myself = AlkaneId.fromBytes(buf, 0);
    const caller = AlkaneId.fromBytes(buf, 32);
    const vout = new u128(load<u64>(ptr + 64), load<u64>(ptr + 72));
    const incomingAlkanesCount = new u128(load<u64>(ptr + 80), load<u64>(ptr + 88));
    
    // Parse inputs (rest of buffer)
    const inputCount = (size - 96) / 16;
    const inputs: u128[] = [];
    for (let i = 0; i < inputCount; i++) {
      const offset = 96 + (i * 16);
      inputs.push(new u128(load<u64>(ptr + offset), load<u64>(ptr + offset + 8)));
    }
    
    return new ExecutionContext(myself, caller, vout, incomingAlkanesCount, inputs);
  }

  /**
   * Get input at index
   */
  getInput(index: i32): u128 {
    if (index < 0 || index >= this.inputs.length) {
      return u128.Zero;
    }
    return this.inputs[index];
  }

  /**
   * Get input as u64
   */
  getInputU64(index: i32): u64 {
    return this.getInput(index).lo;
  }

  /**
   * Get input as u32
   */
  getInputU32(index: i32): u32 {
    return this.getInput(index).lo as u32;
  }
}

/**
 * ExtcallType enum for different call types
 */
export enum ExtcallType {
  CALL = 0,
  STATICCALL = 1,
  DELEGATECALL = 2
}

/**
 * AlkaneResponder - Main abstraction for interacting with alkanes runtime
 */
export class AlkaneResponder {
  private context: ExecutionContext | null = null;

  /**
   * Load the execution context (lazy)
   */
  loadContext(): ExecutionContext {
    if (!this.context) {
      this.context = ExecutionContext.load();
    }
    return this.context!;
  }

  /**
   * Generic extcall implementation - all call types use this under the hood
   * @param callType Type of call (CALL, STATICCALL, DELEGATECALL)
   * @param target Target alkane ID
   * @param inputs Input parameters (opcodes, args, etc)
   * @param maxFuel Maximum fuel to use
   * @returns CallResponse or null if failed
   */
  private extcall(
    callType: ExtcallType,
    target: AlkaneId,
    inputs: u128[],
    maxFuel: u64 = 0xFFFFFFFFFFFFFFFF
  ): CallResponse | null {
    // Build cellpack
    const cellpack = new Cellpack(target, inputs);
    const cellpackBuf = cellpack.toArrayBuffer();
    
    // Empty parcels
    const emptyAlkanes = new AlkaneTransferParcel().serialize();
    const emptyStorage = new StorageMap().serialize();
    
    // Convert to pointers
    const cellpackPtr = changetype<i32>(changetype<usize>(cellpackBuf));
    const alkanesPtr = changetype<i32>(changetype<usize>(emptyAlkanes));
    const storagePtr = changetype<i32>(changetype<usize>(emptyStorage));
    
    // Dispatch to appropriate host function based on call type
    let result: i32;
    if (callType == ExtcallType.STATICCALL) {
      result = __staticcall(cellpackPtr, alkanesPtr, storagePtr, maxFuel);
    } else if (callType == ExtcallType.CALL) {
      result = __call(cellpackPtr, alkanesPtr, storagePtr, maxFuel);
    } else { // DELEGATECALL
      result = __delegatecall(cellpackPtr, alkanesPtr, storagePtr, maxFuel);
    }
    
    // Check for error
    if (result < 0) {
      return null;
    }
    
    // Allocate buffer for return data
    const returnBuf = new ArrayBuffer(result);
    
    // Copy return data (ArrayBuffer automatically has length at ptr-4)
    __returndatacopy(changetype<i32>(changetype<usize>(returnBuf)));
    
    return CallResponse.fromBytes(returnBuf);
  }

  /**
   * Make a staticcall to another alkane (read-only)
   * @param target Target alkane ID
   * @param opcode Opcode to call
   * @param maxFuel Maximum fuel (default: max u64)
   * @returns CallResponse or null if failed
   */
  staticcall(
    target: AlkaneId,
    opcode: u128,
    maxFuel: u64 = 0xFFFFFFFFFFFFFFFF
  ): CallResponse | null {
    return this.extcall(ExtcallType.STATICCALL, target, [opcode], maxFuel);
  }

  /**
   * Make a staticcall with multiple inputs
   */
  staticcallWithInputs(
    target: AlkaneId,
    inputs: u128[],
    maxFuel: u64 = 0xFFFFFFFFFFFFFFFF
  ): CallResponse | null {
    return this.extcall(ExtcallType.STATICCALL, target, inputs, maxFuel);
  }

  /**
   * Make a regular call (can modify state)
   */
  call(
    target: AlkaneId,
    opcode: u128,
    maxFuel: u64 = 0xFFFFFFFFFFFFFFFF
  ): CallResponse | null {
    return this.extcall(ExtcallType.CALL, target, [opcode], maxFuel);
  }

  /**
   * Make a call with multiple inputs
   */
  callWithInputs(
    target: AlkaneId,
    inputs: u128[],
    maxFuel: u64 = 0xFFFFFFFFFFFFFFFF
  ): CallResponse | null {
    return this.extcall(ExtcallType.CALL, target, inputs, maxFuel);
  }

  /**
   * Make a delegatecall (runs in current context)
   */
  delegatecall(
    target: AlkaneId,
    opcode: u128,
    maxFuel: u64 = 0xFFFFFFFFFFFFFFFF
  ): CallResponse | null {
    return this.extcall(ExtcallType.DELEGATECALL, target, [opcode], maxFuel);
  }

  /**
   * Make a delegatecall with multiple inputs
   */
  delegatecallWithInputs(
    target: AlkaneId,
    inputs: u128[],
    maxFuel: u64 = 0xFFFFFFFFFFFFFFFF
  ): CallResponse | null {
    return this.extcall(ExtcallType.DELEGATECALL, target, inputs, maxFuel);
  }

  /**
   * Log a message for debugging
   */
  log(message: string): void {
    const buf = String.UTF8.encode(message);
    __log(changetype<i32>(changetype<usize>(buf)));
  }

  /**
   * Get current sequence number
   */
  getSequence(): u128 {
    const buf = new ArrayBuffer(16);
    __sequence(changetype<i32>(changetype<usize>(buf)));
    const ptr = changetype<usize>(buf);
    return new u128(load<u64>(ptr), load<u64>(ptr + 8));
  }

  /**
   * Get remaining fuel
   */
  getFuel(): u64 {
    const buf = new ArrayBuffer(8);
    __fuel(changetype<i32>(changetype<usize>(buf)));
    return load<u64>(changetype<usize>(buf));
  }

  /**
   * Get current block height
   */
  getHeight(): u64 {
    const buf = new ArrayBuffer(8);
    __height(changetype<i32>(changetype<usize>(buf)));
    return load<u64>(changetype<usize>(buf));
  }

  /**
   * Load storage value
   */
  loadStorage(key: ArrayBuffer): ArrayBuffer | null {
    const size = __request_storage(changetype<i32>(changetype<usize>(key)));
    if (size <= 0) return null;
    
    const value = new ArrayBuffer(size);
    __load_storage(
      changetype<i32>(changetype<usize>(key)),
      changetype<i32>(changetype<usize>(value))
    );
    return value;
  }

  /**
   * Get balance of an alkane
   */
  getBalance(who: AlkaneId, what: AlkaneId): u128 {
    const whoBuf = who.toArrayBuffer();
    const whatBuf = what.toArrayBuffer();
    const output = new ArrayBuffer(16);
    
    __balance(
      changetype<i32>(changetype<usize>(whoBuf)),
      changetype<i32>(changetype<usize>(whatBuf)),
      changetype<i32>(changetype<usize>(output))
    );
    
    const ptr = changetype<usize>(output);
    return new u128(load<u64>(ptr), load<u64>(ptr + 8));
  }

  /**
   * Load current transaction
   */
  loadTransaction(): ArrayBuffer {
    const size = __request_transaction();
    const buf = new ArrayBuffer(size);
    __load_transaction(changetype<i32>(changetype<usize>(buf)));
    return buf;
  }

  /**
   * Load current block
   */
  loadBlock(): ArrayBuffer {
    const size = __request_block();
    const buf = new ArrayBuffer(size);
    __load_block(changetype<i32>(changetype<usize>(buf)));
    return buf;
  }
}
