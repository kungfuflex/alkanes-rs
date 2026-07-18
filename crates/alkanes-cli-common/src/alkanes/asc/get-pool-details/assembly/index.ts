// Get Pool Details Tx-Script
// Fetches AMM pool details for a specified range of pools using AlkaneResponder
//
// Inputs (via context):
//   [0]: start_index - Starting pool index (0-based)
//   [1]: batch_size - Number of pools to fetch
//
// Output format:
//   [alkanes_count(16)][storage_count(16)][pool_count(16)]
//   [pool0_block(16)][pool0_tx(16)][pool0_details]
//   [pool1_block(16)][pool1_tx(16)][pool1_details]
//   ...

import { u128 } from "as-bignum/assembly";
import { 
  AlkaneResponder, 
  AlkaneId, 
  ExtendedCallResponse 
} from "../../alkanes-asm-common/assembly";

// Factory contract constants
const FACTORY = new AlkaneId(u128.from(4), u128.from(65522));
const GET_ALL_POOLS_OPCODE = u128.from(3);
const GET_POOL_DETAILS_OPCODE = u128.from(999);

/**
 * Main entry point for tx-script execution
 * @returns Pointer to response data (ArrayBuffer with length at ptr-4)
 */
export function __execute(): i32 {
  // DEBUG: Return hardcoded response
  const response = new ExtendedCallResponse(8192);
  response.writeU128(u128.from(2)); // pool_count = 2
  const finalBuf = response.finalize();
  return changetype<i32>(changetype<usize>(finalBuf));
  
  /* DISABLED - TESTING STEP BY STEP
  const responder = new AlkaneResponder();
  
  // Step 1: Load context to get inputs
  const ctx = responder.loadContext();
  const startIndex = ctx.getInputU32(0);
  const batchSize = ctx.getInputU32(1);
  
  // DEBUG: Just return the inputs to verify context loading works
  const response = new ExtendedCallResponse(256);
  response.writeU128(u128.from(startIndex));
  response.writeU128(u128.from(batchSize));
  const finalBuf = response.finalize();
  return changetype<i32>(changetype<usize>(finalBuf));
  
  /* DISABLED FOR NOW - DEBUGGING
  // Step 2: Call factory to get all pools
  const factoryResponse = responder.staticcall(FACTORY, GET_ALL_POOLS_OPCODE);
  if (!factoryResponse) {
    return buildEmptyResponse();
  }
  
  // Step 3: Parse pool list from factory response
  // Format: [pool_count(16)][pool0_block(16)][pool0_tx(16)]...
  const poolData = factoryResponse.data;
  const ptr = changetype<usize>(poolData);
  const totalPools = load<u64>(ptr) as u32;
  
  // Calculate range
  const endIndex = min(startIndex + batchSize, totalPools);
  const poolCount = endIndex - startIndex;
  
  // Step 4: Build response
  const response = new ExtendedCallResponse(8192);
  
  // Write pool count
  response.writeU128(u128.from(poolCount));
  
  // Step 5: Loop through pools and fetch details
  for (let i: u32 = 0; i < poolCount; i++) {
    const poolIdx = startIndex + i;
    
    // Read pool ID from factory response
    // Pool list starts at offset 16 (skip count), each pool is 32 bytes
    const poolIdOffset = 16 + (poolIdx * 32);
    const poolBlock = new u128(
      load<u64>(ptr + poolIdOffset),
      load<u64>(ptr + poolIdOffset + 8)
    );
    const poolTx = new u128(
      load<u64>(ptr + poolIdOffset + 16),
      load<u64>(ptr + poolIdOffset + 24)
    );
    
    // Write pool ID to response
    response.writeU128(poolBlock);
    response.writeU128(poolTx);
    
    // Call pool to get details
    const poolId = new AlkaneId(poolBlock, poolTx);
    const poolResponse = responder.staticcall(poolId, GET_POOL_DETAILS_OPCODE);
    
    if (!poolResponse) {
      // Pool call failed - write detail_len = 0
      response.writeU128(u128.Zero);
      continue;
    }
    
    // Write detail length and details
    const detailLen = poolResponse.data.byteLength;
    response.writeU128(u128.from(detailLen));
    response.writeBytes(poolResponse.data);
  }
  
  // Step 6: Finalize and return
  const finalBuf = response.finalize();
  return changetype<i32>(changetype<usize>(finalBuf));
  */
}

/**
 * Build empty response for error cases
 */
function buildEmptyResponse(): i32 {
  const response = new ExtendedCallResponse(128);
  response.writeU128(u128.Zero); // pool_count = 0
  const finalBuf = response.finalize();
  return changetype<i32>(changetype<usize>(finalBuf));
}

/**
 * Minimum of two values
 */
function min(a: u32, b: u32): u32 {
  return a < b ? a : b;
}
