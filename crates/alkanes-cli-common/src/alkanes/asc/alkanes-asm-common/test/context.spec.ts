// Test context loading functionality
import * as assert from 'assert';
import * as path from 'path';
import {
  AlkaneId,
  buildContext,
  createHostImports,
  loadWasm,
  readWasmData,
  deserializeU128,
} from './helpers';

describe('Context Loading', () => {
  const wasmPath = path.join(__dirname, 'fixtures', 'test-context.wasm');

  it('should load context and return inputs', async () => {
    // Create a test context
    const myself = new AlkaneId(1n, 0n);
    const caller = new AlkaneId(4n, 65522n);
    const vout = 0n;
    const incomingAlkanesCount = 0n;
    const inputs = [42n, 99n]; // Two test inputs
    
    const contextBuffer = buildContext(myself, caller, vout, incomingAlkanesCount, inputs);
    
    // Create host imports
    const imports = createHostImports(contextBuffer);
    
    // Load WASM
    const instance = await loadWasm(wasmPath, imports);
    const memory = instance.exports.memory as WebAssembly.Memory;
    const execute = instance.exports.__execute as () => number;
    
    // Execute
    const resultPtr = execute();
    
    // Read result
    const resultData = readWasmData(memory, resultPtr);
    
    // ExtendedCallResponse format: [alkanes_count(16)][storage_count(16)][data...]
    // So we expect: 16 + 16 + 16 + 16 = 64 bytes total
    assert.strictEqual(resultData.length, 64);
    
    // Skip the alkanes_count and storage_count headers (32 bytes)
    const dataOffset = 32;
    
    const output0 = deserializeU128(resultData, dataOffset);
    const output1 = deserializeU128(resultData, dataOffset + 16);
    
    assert.strictEqual(output0, 42n);
    assert.strictEqual(output1, 99n);
  });

  it('should handle empty inputs', async () => {
    const myself = new AlkaneId(1n, 0n);
    const caller = new AlkaneId(4n, 65522n);
    const vout = 0n;
    const incomingAlkanesCount = 0n;
    const inputs: bigint[] = []; // No inputs
    
    const contextBuffer = buildContext(myself, caller, vout, incomingAlkanesCount, inputs);
    const imports = createHostImports(contextBuffer);
    const instance = await loadWasm(wasmPath, imports);
    
    const memory = instance.exports.memory as WebAssembly.Memory;
    const execute = instance.exports.__execute as () => number;
    
    // This should either return zeros or handle gracefully
    const resultPtr = execute();
    const resultData = readWasmData(memory, resultPtr);
    
    // Should return 64 bytes: alkanes(16) + storage(16) + 2 inputs(32)
    assert.strictEqual(resultData.length, 64);
  });

  it('should handle large input values', async () => {
    const myself = new AlkaneId(1n, 0n);
    const caller = new AlkaneId(4n, 65522n);
    const vout = 0n;
    const incomingAlkanesCount = 0n;
    const inputs = [
      0xFFFFFFFFFFFFFFFFn, // Max u64
      (1n << 127n) - 1n,   // Near max u128
    ];
    
    const contextBuffer = buildContext(myself, caller, vout, incomingAlkanesCount, inputs);
    const imports = createHostImports(contextBuffer);
    const instance = await loadWasm(wasmPath, imports);
    
    const memory = instance.exports.memory as WebAssembly.Memory;
    const execute = instance.exports.__execute as () => number;
    
    const resultPtr = execute();
    const resultData = readWasmData(memory, resultPtr);
    
    // Skip ExtendedCallResponse headers (32 bytes)
    const dataOffset = 32;
    
    const output0 = deserializeU128(resultData, dataOffset);
    const output1 = deserializeU128(resultData, dataOffset + 16);
    
    assert.strictEqual(output0, inputs[0]);
    assert.strictEqual(output1, inputs[1]);
  });
});
