// Test staticcall functionality
import * as assert from 'assert';
import * as path from 'path';
import {
  AlkaneId,
  buildContext,
  createHostImports,
  loadWasm,
  readWasmData,
  deserializeU128,
  serializeU128,
  StaticCallMock,
} from './helpers';

describe('Staticcall', () => {
  const wasmPath = path.join(__dirname, 'fixtures', 'test-staticcall.wasm');

  it('should make a staticcall and return success response', async () => {
    // Setup: target is 4:65522, opcode is 3
    const targetBlock = 4n;
    const targetTx = 65522n;
    const opcode = 3n;
    
    const myself = new AlkaneId(1n, 0n);
    const caller = new AlkaneId(2n, 0n);
    const inputs = [targetBlock, targetTx, opcode];
    
    const contextBuffer = buildContext(myself, caller, 0n, 0n, inputs);
    
    // Create a mock staticcall that returns test data
    const mockStaticCall: StaticCallMock = (target, op) => {
      assert.strictEqual(target.block, targetBlock);
      assert.strictEqual(target.tx, targetTx);
      assert.strictEqual(op, opcode);
      
      // Return mock response: just echo back the opcode
      const responseData = serializeU128(op);
      
      return {
        success: true,
        data: responseData,
      };
    };
    
    const imports = createHostImports(contextBuffer, mockStaticCall);
    const instance = await loadWasm(wasmPath, imports);
    
    const memory = instance.exports.memory as WebAssembly.Memory;
    const execute = instance.exports.__execute as () => number;
    
    const resultPtr = execute();
    const resultData = readWasmData(memory, resultPtr);
    
    // Result format: [success_flag(16)][data_length(16)][data...]
    const successFlag = deserializeU128(resultData, 0);
    assert.strictEqual(successFlag, 1n); // Success
    
    const dataLength = deserializeU128(resultData, 16);
    assert.strictEqual(dataLength, 16n); // u128 is 16 bytes
    
    const returnedOpcode = deserializeU128(resultData, 32);
    assert.strictEqual(returnedOpcode, opcode);
  });

  it('should handle staticcall failure', async () => {
    const targetBlock = 4n;
    const targetTx = 65522n;
    const opcode = 999n;
    
    const myself = new AlkaneId(1n, 0n);
    const caller = new AlkaneId(2n, 0n);
    const inputs = [targetBlock, targetTx, opcode];
    
    const contextBuffer = buildContext(myself, caller, 0n, 0n, inputs);
    
    // Mock that always fails
    const mockStaticCall: StaticCallMock = () => {
      return {
        success: false,
        data: new Uint8Array(0),
      };
    };
    
    const imports = createHostImports(contextBuffer, mockStaticCall);
    const instance = await loadWasm(wasmPath, imports);
    
    const memory = instance.exports.memory as WebAssembly.Memory;
    const execute = instance.exports.__execute as () => number;
    
    const resultPtr = execute();
    const resultData = readWasmData(memory, resultPtr);
    
    // Result should just be a failure flag
    const successFlag = deserializeU128(resultData, 0);
    assert.strictEqual(successFlag, 0n); // Failure
  });

  it('should handle staticcall with large response data', async () => {
    const targetBlock = 4n;
    const targetTx = 65522n;
    const opcode = 3n;
    
    const myself = new AlkaneId(1n, 0n);
    const caller = new AlkaneId(2n, 0n);
    const inputs = [targetBlock, targetTx, opcode];
    
    const contextBuffer = buildContext(myself, caller, 0n, 0n, inputs);
    
    // Mock that returns a large response
    const largeData = new Uint8Array(1024);
    for (let i = 0; i < largeData.length; i++) {
      largeData[i] = i % 256;
    }
    
    const mockStaticCall: StaticCallMock = () => {
      return {
        success: true,
        data: largeData,
      };
    };
    
    const imports = createHostImports(contextBuffer, mockStaticCall);
    const instance = await loadWasm(wasmPath, imports);
    
    const memory = instance.exports.memory as WebAssembly.Memory;
    const execute = instance.exports.__execute as () => number;
    
    const resultPtr = execute();
    const resultData = readWasmData(memory, resultPtr);
    
    const successFlag = deserializeU128(resultData, 0);
    assert.strictEqual(successFlag, 1n);
    
    const dataLength = deserializeU128(resultData, 16);
    assert.strictEqual(dataLength, BigInt(largeData.length));
    
    // Verify the data
    const returnedData = resultData.slice(32, 32 + largeData.length);
    assert.deepStrictEqual(returnedData, largeData);
  });
});
