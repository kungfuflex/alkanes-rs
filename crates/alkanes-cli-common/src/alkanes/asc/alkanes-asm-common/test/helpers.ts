// Test helpers for running WASM modules with mock alkanes runtime
import * as fs from 'fs';
import * as path from 'path';

/**
 * Serialize a u128 value to bytes (little-endian)
 */
export function serializeU128(value: bigint): Uint8Array {
  const buffer = new ArrayBuffer(16);
  const view = new DataView(buffer);
  
  // Split into two 64-bit parts
  const low = value & 0xFFFFFFFFFFFFFFFFn;
  const high = (value >> 64n) & 0xFFFFFFFFFFFFFFFFn;
  
  view.setBigUint64(0, low, true);   // little-endian
  view.setBigUint64(8, high, true);
  
  return new Uint8Array(buffer);
}

/**
 * Deserialize a u128 value from bytes (little-endian)
 */
export function deserializeU128(bytes: Uint8Array, offset: number = 0): bigint {
  const view = new DataView(bytes.buffer, bytes.byteOffset + offset, 16);
  const low = view.getBigUint64(0, true);
  const high = view.getBigUint64(8, true);
  return (high << 64n) | low;
}

/**
 * AlkaneId serialization
 */
export class AlkaneId {
  constructor(public block: bigint = 0n, public tx: bigint = 0n) {}

  serialize(): Uint8Array {
    const blockBytes = serializeU128(this.block);
    const txBytes = serializeU128(this.tx);
    const result = new Uint8Array(32);
    result.set(blockBytes, 0);
    result.set(txBytes, 16);
    return result;
  }

  static deserialize(bytes: Uint8Array, offset: number = 0): AlkaneId {
    const block = deserializeU128(bytes, offset);
    const tx = deserializeU128(bytes, offset + 16);
    return new AlkaneId(block, tx);
  }
}

/**
 * Build a context buffer for testing
 * Layout: [myself(32)][caller(32)][vout(16)][incoming_alkanes_count(16)][inputs...]
 */
export function buildContext(
  myself: AlkaneId,
  caller: AlkaneId,
  vout: bigint,
  incomingAlkanesCount: bigint,
  inputs: bigint[]
): Uint8Array {
  const myselfBytes = myself.serialize();
  const callerBytes = caller.serialize();
  const voutBytes = serializeU128(vout);
  const countBytes = serializeU128(incomingAlkanesCount);
  
  const inputsSize = inputs.length * 16;
  const totalSize = 32 + 32 + 16 + 16 + inputsSize;
  
  const buffer = new Uint8Array(totalSize);
  let offset = 0;
  
  buffer.set(myselfBytes, offset);
  offset += 32;
  
  buffer.set(callerBytes, offset);
  offset += 32;
  
  buffer.set(voutBytes, offset);
  offset += 16;
  
  buffer.set(countBytes, offset);
  offset += 16;
  
  for (const input of inputs) {
    buffer.set(serializeU128(input), offset);
    offset += 16;
  }
  
  return buffer;
}

/**
 * Mock staticcall responses
 */
export interface StaticCallResponse {
  success: boolean;
  data: Uint8Array;
}

export type StaticCallMock = (
  target: AlkaneId,
  opcode: bigint
) => StaticCallResponse;

/**
 * Create host imports for WASM testing
 */
export function createHostImports(
  contextBuffer: Uint8Array,
  staticcallMock?: StaticCallMock
): any {
  let memory: WebAssembly.Memory | null = null;
  let lastReturndataBuffer: Uint8Array = new Uint8Array(0);

  const env = {
    abort: () => {
      throw new Error("abort called");
    },
    
    __request_context: () => {
      return contextBuffer.length;
    },
    
    __load_context: (ptr: number) => {
      if (!memory) throw new Error("memory not set");
      const memView = new Uint8Array(memory.buffer);
      memView.set(contextBuffer, ptr);
    },
    
    __staticcall: (
      cellpackPtr: number,
      alkanesPtr: number,
      storagePtr: number,
      maxFuel: bigint
    ): number => {
      if (!staticcallMock) {
        // Default: return -1 (failure)
        return -1;
      }
      
      if (!memory) throw new Error("memory not set");
      const memView = new Uint8Array(memory.buffer);
      
      // Read cellpack length from ptr-4
      const cellpackLen = new DataView(memory.buffer).getUint32(cellpackPtr - 4, true);
      const cellpackBytes = memView.slice(cellpackPtr, cellpackPtr + cellpackLen);
      
      // Parse cellpack: [target(32)][opcode(16)]
      const target = AlkaneId.deserialize(cellpackBytes, 0);
      const opcode = deserializeU128(cellpackBytes, 32);
      
      // Call the mock
      const response = staticcallMock(target, opcode);
      
      if (!response.success) {
        return -1;
      }
      
      // Store response data for returndatacopy
      lastReturndataBuffer = response.data;
      
      return response.data.length;
    },
    
    __returndatacopy: (ptr: number) => {
      if (!memory) throw new Error("memory not set");
      const memView = new Uint8Array(memory.buffer);
      memView.set(lastReturndataBuffer, ptr);
    },
    
    __call: () => -1,
    __delegatecall: () => -1,
    __request_storage: () => 0,
    __load_storage: () => {},
    __log: (ptr: number) => {
      if (!memory) return;
      const memView = new Uint8Array(memory.buffer);
      const len = new DataView(memory.buffer).getUint32(ptr - 4, true);
      const bytes = memView.slice(ptr, ptr + len);
      const text = new TextDecoder().decode(bytes);
      console.log(`[WASM LOG] ${text}`);
    },
    __balance: () => {},
    __sequence: () => {},
    __fuel: () => {},
    __height: () => {},
    __request_transaction: () => 0,
    __load_transaction: () => {},
    __request_block: () => 0,
    __load_block: () => {},
  };

  return {
    env,
    // Capture memory when instantiated
    __internal: {
      setMemory: (mem: WebAssembly.Memory) => {
        memory = mem;
      },
    },
  };
}

/**
 * Load and instantiate a WASM module
 */
export async function loadWasm(
  wasmPath: string,
  imports: any
): Promise<WebAssembly.Instance> {
  const wasmBytes = fs.readFileSync(wasmPath);
  const { instance } = await WebAssembly.instantiate(wasmBytes, imports);
  
  // Set memory reference
  const memory = instance.exports.memory as WebAssembly.Memory;
  if (!memory) {
    throw new Error("WASM module does not export 'memory'");
  }
  
  if (imports.__internal && typeof imports.__internal.setMemory === 'function') {
    imports.__internal.setMemory(memory);
  }
  
  return instance;
}

/**
 * Read data from WASM memory at pointer (with length at ptr-4)
 */
export function readWasmData(memory: WebAssembly.Memory, ptr: number): Uint8Array {
  const view = new DataView(memory.buffer);
  const length = view.getUint32(ptr - 4, true);
  return new Uint8Array(memory.buffer, ptr, length);
}
