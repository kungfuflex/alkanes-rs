// AlkaneResponder - High-level abstraction for alkanes runtime
import { 
  __request_context, __load_context, __staticcall, __returndatacopy,
  __call, __delegatecall, __log, __sequence, __fuel, __height,
  __request_storage, __load_storage, __balance,
  __request_transaction, __load_transaction, __request_block, __load_block
} from "./runtime";
import { AlkaneId, Cellpack, AlkaneTransferParcel, CallResponse, ExtendedCallResponse } from "./types";
import { StorageMap } from "../storage-map";
import { u128 } from "../u128";

/**
 * ExecutionContext parsed from runtime
 * Layout: [myself(32)][caller(32)][vout(16)][incoming_alkanes_count(16)][inputs...]
 * 
 * NOTE: We store inputs as a raw buffer instead of Array<u128> for stub runtime compatibility.
 * Arrays don't work with stub runtime, so we use manual buffer access instead.
 */
export class ExecutionContext {
  myself: AlkaneId;
  caller: AlkaneId;
  vout: u128;
  incomingAlkanesCount: u128;
  
  // Store inputs as buffer instead of Array for stub runtime compatibility
  private inputsBuffer: ArrayBuffer;
  private inputsCount: i32;

  constructor(
    myself: AlkaneId,
    caller: AlkaneId,
    vout: u128,
    incomingAlkanesCount: u128,
    inputsBuffer: ArrayBuffer,
    inputsCount: i32
  ) {
    this.myself = myself;
    this.caller = caller;
    this.vout = vout;
    this.incomingAlkanesCount = incomingAlkanesCount;
    this.inputsBuffer = inputsBuffer;
    this.inputsCount = inputsCount;
  }

  /**
   * Load context from runtime
   */
  static load(): ExecutionContext {
    // Request context size
    const size = __request_context();
    
    // Allocate ArrayBuffer
    const buf = new ArrayBuffer(size);
    const ptr = changetype<usize>(buf);
    
    // Load context data
    __load_context(changetype<i32>(ptr));
    
    // Parse context fields
    const myself = AlkaneId.parse(buf, 0);
    const caller = AlkaneId.parse(buf, 32);
    const vout = u128.load(ptr + 64);
    const incomingAlkanesCount = u128.load(ptr + 80);
    
    // Extract inputs buffer (rest of buffer after offset 96)
    const inputsSize = size - 96;
    const inputsCount = inputsSize / 16;
    
    // Copy inputs to separate buffer
    const inputsBuffer = new ArrayBuffer(inputsSize);
    const inputsPtr = changetype<usize>(inputsBuffer);
    const srcPtr = ptr + 96;
    const inputsSizeUsize = inputsSize as usize;
    
    for (let i: usize = 0; i < inputsSizeUsize; i++) {
      store<u8>(inputsPtr + i, load<u8>(srcPtr + i));
    }
    
    return new ExecutionContext(myself, caller, vout, incomingAlkanesCount, inputsBuffer, inputsCount);
  }

  /**
   * Get number of inputs
   */
  getInputCount(): i32 {
    return this.inputsCount;
  }

  /**
   * Get input at index
   */
  getInput(index: i32): u128 {
    if (index < 0 || index >= this.inputsCount) {
      return u128.Zero;
    }
    const ptr = changetype<usize>(this.inputsBuffer);
    return u128.load(ptr + (index * 16));
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

  /**
   * Get all inputs as ArrayBuffer (for serialization)
   */
  getInputsBuffer(): ArrayBuffer {
    return this.inputsBuffer;
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
    opcode: u128,
    maxFuel: u64 = 0xFFFFFFFFFFFFFFFF
  ): CallResponse | null {
    // Build cellpack with single input (opcode)
    const cellpack = Cellpack.withSingleInput(target, opcode);
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
    return this.extcall(ExtcallType.STATICCALL, target, opcode, maxFuel);
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
