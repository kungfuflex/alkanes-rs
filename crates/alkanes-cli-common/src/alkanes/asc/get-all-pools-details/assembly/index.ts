// Get All Pools with Details
// 
// This tx-script:
// 1. Calls factory GET_ALL_POOLS (opcode 3) to get pool list
// 2. Reads range from context inputs: inputs[0] = start_index, inputs[1] = end_index
// 3. For each pool in the range, calls GET_POOL_DETAILS (opcode 999) on that pool
// 4. Returns concatenated response: [pool_count(u128)][pool_details_0][pool_details_1]...
//
// Usage: --inputs <start>,<end>
// Example: --inputs 0,5 gets details for pools 0-5 (inclusive)

import { ExtendedCallResponse, CallResponse, u128 } from "../../alkanes-asm-common/assembly";
import { __staticcall, __returndatacopy, __request_context, __load_context } from "../../alkanes-asm-common/assembly/alkanes/runtime";

export function __execute(): i32 {
  const response = new ExtendedCallResponse();
  
  // Step 1: Get pool list from factory
  const factoryBlock = u128.from(4);
  const factoryTx = u128.from(65522);
  const GET_ALL_POOLS_OPCODE = u128.from(3);
  
  // Serialize Cellpack for factory call: [block(16)][tx(16)][opcode(16)]
  const factoryCellpack = new ArrayBuffer(48);
  let ptr = changetype<usize>(factoryCellpack);
  
  // Target: 4:65522
  store<u64>(ptr, 4);
  store<u64>(ptr + 8, 0);
  store<u64>(ptr + 16, 65522);
  store<u64>(ptr + 24, 0);
  
  // Opcode: 3 (GET_ALL_POOLS)
  store<u64>(ptr + 32, 3);
  store<u64>(ptr + 40, 0);
  
  // Empty parcel and storage
  const emptyParcel = new ArrayBuffer(16);
  store<u64>(changetype<usize>(emptyParcel), 0);
  store<u64>(changetype<usize>(emptyParcel) + 8, 0);
  
  const emptyStorage = new ArrayBuffer(4);
  store<u32>(changetype<usize>(emptyStorage), 0);
  
  // Call factory to get pool list
  const factoryResult = __staticcall(
    changetype<i32>(changetype<usize>(factoryCellpack)),
    changetype<i32>(changetype<usize>(emptyParcel)),
    changetype<i32>(changetype<usize>(emptyStorage)),
    0xFFFFFFFFFFFFFFFF
  );
  
  if (factoryResult < 0) {
    // Factory call failed
    response.setData(new ArrayBuffer(0));
    const finalResult = response.finalize();
    return changetype<i32>(changetype<usize>(finalResult));
  }
  
  // Get factory response data
  const factoryReturndata = new ArrayBuffer(factoryResult);
  __returndatacopy(changetype<i32>(changetype<usize>(factoryReturndata)));
  
  // Parse CallResponse to extract pool list
  const factoryResponse = CallResponse.fromBytes(factoryReturndata);
  const poolsData = factoryResponse.data;
  
  // Parse pool count from first 16 bytes
  const poolsPtr = changetype<usize>(poolsData);
  const poolCount = load<u64>(poolsPtr); // Low 64 bits of u128
  
  // Step 2: Load context to get range from inputs
  const contextSize = __request_context();
  const contextBuf = new ArrayBuffer(contextSize);
  __load_context(changetype<i32>(changetype<usize>(contextBuf)));
  
  // Context format: [myself(32)][caller(32)][vout(16)][incoming_alkanes_count(16)][inputs...]
  // Inputs start at offset 96
  const contextPtr = changetype<usize>(contextBuf);
  const inputsOffset: usize = 96;
  const inputsSize = contextSize - 96;
  
  // Read start and end indices from inputs[0] and inputs[1]
  // Default: start=0, end=2^32-1 (fetch all pools)
  let startIdx: u64 = 0;
  let endIdx: u64 = 0xFFFFFFFF; // Default to 2^32 - 1
  
  if (inputsSize >= 16) {
    // Read inputs[0] (start index)
    startIdx = load<u64>(contextPtr + inputsOffset);
  }
  
  if (inputsSize >= 32) {
    // Read inputs[1] (end index)
    endIdx = load<u64>(contextPtr + inputsOffset + 16);
  }
  
  // Clamp end index to pool count - 1
  if (endIdx >= poolCount) {
    endIdx = poolCount - 1;
  }
  
  // Validate range
  if (startIdx > endIdx) {
    // Invalid range - return empty
    response.setData(new ArrayBuffer(0));
    const finalResult = response.finalize();
    return changetype<i32>(changetype<usize>(finalResult));
  }
  
  // Calculate number of pools to fetch
  const numPools = (endIdx - startIdx + 1) as i32;
  
  // Step 3: For each pool in range, call GET_POOL_DETAILS (opcode 999)
  // We'll build a response buffer with all the pool details concatenated
  // Format: [count(u128)][details_0_size(u64)][details_0_data][details_1_size(u64)][details_1_data]...
  
  // First pass: calculate total size needed
  const detailsSizes = new Array<i32>(numPools);
  let totalSize: i32 = 16; // Start with count (u128)
  
  for (let i: i32 = 0; i < numPools; i++) {
    const poolIdx = (startIdx as i32) + i;
    const poolOffset = 16 + (poolIdx * 32); // Skip count (16 bytes) + pool entries (32 bytes each)
    
    // Read pool AlkaneId
    const poolBlock = load<u64>(poolsPtr + poolOffset);
    const poolBlockHi = load<u64>(poolsPtr + poolOffset + 8);
    const poolTx = load<u64>(poolsPtr + poolOffset + 16);
    const poolTxHi = load<u64>(poolsPtr + poolOffset + 24);
    
    // Build Cellpack for pool GET_POOL_DETAILS call
    const poolCellpack = new ArrayBuffer(48);
    const poolCellpackPtr = changetype<usize>(poolCellpack);
    
    store<u64>(poolCellpackPtr, poolBlock);
    store<u64>(poolCellpackPtr + 8, poolBlockHi);
    store<u64>(poolCellpackPtr + 16, poolTx);
    store<u64>(poolCellpackPtr + 24, poolTxHi);
    store<u64>(poolCellpackPtr + 32, 999); // GET_POOL_DETAILS opcode
    store<u64>(poolCellpackPtr + 40, 0);
    
    // Call pool to get details
    const poolResult = __staticcall(
      changetype<i32>(poolCellpackPtr),
      changetype<i32>(changetype<usize>(emptyParcel)),
      changetype<i32>(changetype<usize>(emptyStorage)),
      0xFFFFFFFFFFFFFFFF
    );
    
    if (poolResult < 0) {
      // Pool call failed - store 0 size
      detailsSizes[i] = 0;
    } else {
      detailsSizes[i] = poolResult;
      totalSize += 8 + poolResult; // size(u64) + data
    }
  }
  
  // Second pass: build response buffer
  const resultData = new ArrayBuffer(totalSize);
  const resultPtr = changetype<usize>(resultData);
  
  // Write count
  store<u64>(resultPtr, numPools as u64);
  store<u64>(resultPtr + 8, 0);
  
  let writeOffset: usize = 16;
  
  for (let i: i32 = 0; i < numPools; i++) {
    const poolIdx = (startIdx as i32) + i;
    const poolOffset = 16 + (poolIdx * 32);
    
    // Read pool AlkaneId again
    const poolBlock = load<u64>(poolsPtr + poolOffset);
    const poolBlockHi = load<u64>(poolsPtr + poolOffset + 8);
    const poolTx = load<u64>(poolsPtr + poolOffset + 16);
    const poolTxHi = load<u64>(poolsPtr + poolOffset + 24);
    
    // Build Cellpack again
    const poolCellpack = new ArrayBuffer(48);
    const poolCellpackPtr = changetype<usize>(poolCellpack);
    
    store<u64>(poolCellpackPtr, poolBlock);
    store<u64>(poolCellpackPtr + 8, poolBlockHi);
    store<u64>(poolCellpackPtr + 16, poolTx);
    store<u64>(poolCellpackPtr + 24, poolTxHi);
    store<u64>(poolCellpackPtr + 32, 999);
    store<u64>(poolCellpackPtr + 40, 0);
    
    // Call pool
    const poolResult = __staticcall(
      changetype<i32>(poolCellpackPtr),
      changetype<i32>(changetype<usize>(emptyParcel)),
      changetype<i32>(changetype<usize>(emptyStorage)),
      0xFFFFFFFFFFFFFFFF
    );
    
    if (poolResult < 0) {
      // Failed - write 0 size
      store<u64>(resultPtr + writeOffset, 0);
      writeOffset += 8;
    } else {
      // Success - get data and write it
      const poolReturndata = new ArrayBuffer(poolResult);
      __returndatacopy(changetype<i32>(changetype<usize>(poolReturndata)));
      
      // Parse to get just the data field
      const poolResponse = CallResponse.fromBytes(poolReturndata);
      const detailsData = poolResponse.data;
      const detailsSize = detailsData.byteLength;
      
      // Write size
      store<u64>(resultPtr + writeOffset, detailsSize as u64);
      writeOffset += 8;
      
      // Write data
      const detailsPtr = changetype<usize>(detailsData);
      const detailsSizeUsize = detailsSize as usize;
      for (let j: usize = 0; j < detailsSizeUsize; j++) {
        store<u8>(resultPtr + writeOffset + j, load<u8>(detailsPtr + j));
      }
      writeOffset += detailsSizeUsize;
    }
  }
  
  response.setData(resultData);
  const finalResult = response.finalize();
  return changetype<i32>(changetype<usize>(finalResult));
}
