// Test ExecutionContext loading and parsing
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

describe('ExecutionContext', () => {
  const wasmPath = path.join(__dirname, 'fixtures', 'test-execution-context.wasm');

  it('should correctly load and parse context with inputs', async () => {
    // Create test context
    const myself = new AlkaneId(1n, 0n);
    const caller = new AlkaneId(4n, 65522n);
    const vout = 0n;
    const incomingAlkanesCount = 0n;
    const inputs = [42n, 99n, 123n];
    
    const contextBuffer = buildContext(myself, caller, vout, incomingAlkanesCount, inputs);
    
    console.log('Context buffer length:', contextBuffer.length);
    console.log('Context buffer (hex):', Buffer.from(contextBuffer).toString('hex'));
    
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
    
    console.log('Result data length:', resultData.length);
    console.log('Result data (hex):', Buffer.from(resultData).toString('hex'));
    
    // Parse result
    let offset = 0;
    
    // Read myself
    const myselfBlock = deserializeU128(resultData, offset);
    offset += 16;
    const myselfTx = deserializeU128(resultData, offset);
    offset += 16;
    
    console.log('Parsed myself:', myselfBlock, ':', myselfTx);
    assert.strictEqual(myselfBlock, myself.block);
    assert.strictEqual(myselfTx, myself.tx);
    
    // Read caller
    const callerBlock = deserializeU128(resultData, offset);
    offset += 16;
    const callerTx = deserializeU128(resultData, offset);
    offset += 16;
    
    console.log('Parsed caller:', callerBlock, ':', callerTx);
    assert.strictEqual(callerBlock, caller.block);
    assert.strictEqual(callerTx, caller.tx);
    
    // Read vout
    const parsedVout = deserializeU128(resultData, offset);
    offset += 16;
    
    console.log('Parsed vout:', parsedVout);
    assert.strictEqual(parsedVout, vout);
    
    // Read incoming alkanes count
    const parsedCount = deserializeU128(resultData, offset);
    offset += 16;
    
    console.log('Parsed incoming_alkanes_count:', parsedCount);
    assert.strictEqual(parsedCount, incomingAlkanesCount);
    
    // Read input count
    const inputCount = deserializeU128(resultData, offset);
    offset += 16;
    
    console.log('Parsed input_count:', inputCount);
    assert.strictEqual(inputCount, BigInt(inputs.length));
    
    // Read inputs
    for (let i = 0; i < inputs.length; i++) {
      const input = deserializeU128(resultData, offset);
      offset += 16;
      console.log(`Parsed input[${i}]:`, input);
      assert.strictEqual(input, inputs[i]);
    }
  });

  it('should handle context with no inputs', async () => {
    const myself = new AlkaneId(1n, 0n);
    const caller = new AlkaneId(2n, 0n);
    const vout = 0n;
    const incomingAlkanesCount = 0n;
    const inputs: bigint[] = [];
    
    const contextBuffer = buildContext(myself, caller, vout, incomingAlkanesCount, inputs);
    const imports = createHostImports(contextBuffer);
    const instance = await loadWasm(wasmPath, imports);
    
    const memory = instance.exports.memory as WebAssembly.Memory;
    const execute = instance.exports.__execute as () => number;
    
    const resultPtr = execute();
    const resultData = readWasmData(memory, resultPtr);
    
    // Should have: myself(32) + caller(32) + vout(16) + count(16) + input_count(16) = 112 bytes
    assert.strictEqual(resultData.length, 112);
    
    // Check input count is 0
    const inputCount = deserializeU128(resultData, 96); // offset to input_count field
    assert.strictEqual(inputCount, 0n);
  });

  it('should handle context with many inputs', async () => {
    const myself = new AlkaneId(1n, 0n);
    const caller = new AlkaneId(2n, 0n);
    const vout = 5n;
    const incomingAlkanesCount = 3n;
    const inputs = [1n, 2n, 3n, 4n, 5n, 6n, 7n, 8n, 9n, 10n];
    
    const contextBuffer = buildContext(myself, caller, vout, incomingAlkanesCount, inputs);
    const imports = createHostImports(contextBuffer);
    const instance = await loadWasm(wasmPath, imports);
    
    const memory = instance.exports.memory as WebAssembly.Memory;
    const execute = instance.exports.__execute as () => number;
    
    const resultPtr = execute();
    const resultData = readWasmData(memory, resultPtr);
    
    // Parse and verify
    let offset = 64; // Skip myself and caller
    const parsedVout = deserializeU128(resultData, offset);
    assert.strictEqual(parsedVout, vout);
    
    offset += 16;
    const parsedIncomingCount = deserializeU128(resultData, offset);
    assert.strictEqual(parsedIncomingCount, incomingAlkanesCount);
    
    offset += 16;
    const inputCount = deserializeU128(resultData, offset);
    assert.strictEqual(inputCount, BigInt(inputs.length));
    
    offset += 16;
    for (let i = 0; i < inputs.length; i++) {
      const input = deserializeU128(resultData, offset);
      assert.strictEqual(input, inputs[i], `Input ${i} should match`);
      offset += 16;
    }
  });

  it('should handle large u128 values', async () => {
    const myself = new AlkaneId((1n << 64n) - 1n, (1n << 64n) - 1n); // Max u64 in both parts
    const caller = new AlkaneId(0n, 1n << 127n); // High bit set
    const vout = (1n << 127n) - 1n; // Near max u128
    const incomingAlkanesCount = 0n;
    const inputs = [
      0xFFFFFFFFFFFFFFFFn, // Max u64
      (1n << 64n) - 1n,     // Max u64
      (1n << 127n) - 1n,    // Near max u128
    ];
    
    const contextBuffer = buildContext(myself, caller, vout, incomingAlkanesCount, inputs);
    const imports = createHostImports(contextBuffer);
    const instance = await loadWasm(wasmPath, imports);
    
    const memory = instance.exports.memory as WebAssembly.Memory;
    const execute = instance.exports.__execute as () => number;
    
    const resultPtr = execute();
    const resultData = readWasmData(memory, resultPtr);
    
    // Verify large values are preserved
    let offset = 0;
    const myselfBlock = deserializeU128(resultData, offset);
    offset += 16;
    const myselfTx = deserializeU128(resultData, offset);
    offset += 16;
    
    assert.strictEqual(myselfBlock, myself.block);
    assert.strictEqual(myselfTx, myself.tx);
    
    const callerBlock = deserializeU128(resultData, offset);
    offset += 16;
    const callerTx = deserializeU128(resultData, offset);
    offset += 16;
    
    assert.strictEqual(callerBlock, caller.block);
    assert.strictEqual(callerTx, caller.tx);
    
    const parsedVout = deserializeU128(resultData, offset);
    offset += 16;
    assert.strictEqual(parsedVout, vout);
    
    // Skip incoming count and input count
    offset += 32;
    
    // Verify inputs
    for (let i = 0; i < inputs.length; i++) {
      const input = deserializeU128(resultData, offset);
      assert.strictEqual(input, inputs[i], `Large input ${i} should match`);
      offset += 16;
    }
  });
});
