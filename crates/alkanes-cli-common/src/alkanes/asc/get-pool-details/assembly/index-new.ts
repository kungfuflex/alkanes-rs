// Get Pool Details Tx-Script using AlkaneResponder
import { u128 } from "as-bignum/assembly";
import { AlkaneResponder, ExecutionContext, AlkaneId, ExtendedCallResponse } from "../../../alkanes-asm-common/assembly";

// Factory contract
const FACTORY = new AlkaneId(u128.from(4), u128.from(65522));
const GET_ALL_POOLS_OPCODE = u128.from(3);
const GET_POOL_DETAILS_OPCODE = u128.from(999);

/**
 * Main entry point
 * Inputs: [start_index, batch_size]
 * Output: [alkanes(16)][storage(16)][pool_count(16)][pool0_id(32)][pool0_details][...]
 */
export function __execute(): i32 {
  const responder = new AlkaneResponder();
  
  // Load context to get inputs
  const ctx = responder.loadContext();
  const startIndex = ctx.getInputU32(0);
  const batchSize = ctx.getInputU32(1);
  
  // Step 1: Call factory to get all pools
  const factoryResponse = responder.staticcall(FACTORY, GET_ALL_POOLS_OPCODE);
  if (!factoryResponse) {
    return buildEmptyResponse();
  }
  
  // Parse pool list from factory response
  // Format: [pool_count(16)][pool0_block(16)][pool0_tx(16)][pool1...]
  const poolData = factoryResponse.data;
  const ptr = changetype<usize>(poolData);
  const totalPools = load<u64>(ptr) as u32;
  
  // Calculate range
  const endIndex = min(startIndex + batchSize, totalPools);
  const poolCount = endIndex - startIndex;
  
  // Step 2: Build response
  const response = new ExtendedCallResponse(8192);
  
  // Write pool count
  response.writeU128(u128.from(poolCount));
  
  // Step 3: Loop through pools and fetch details
  for (let i: u32 = 0; i < poolCount; i++) {
    const poolIdx = startIndex + i;
    
    // Read pool ID from factory response
    // Pool list starts at offset 16, each pool is 32 bytes
    const poolOffset = 16 + (poolIdx * 32);
    const poolBlock = new u128(
      load<u64>(ptr + poolOffset),
      load<u64>(ptr + poolOffset + 8)
    );
    const poolTx = new u128(
      load<u64>(ptr + poolOffset + 16),
      load<u64>(ptr + poolOffset + 24)
    );
    
    // Write pool ID to response
    response.writeU128(poolBlock);
    response.writeU128(poolTx);
    
    // Call pool to get details
    const poolId = new AlkaneId(poolBlock, poolTx);
    const poolResponse = responder.staticcall(poolId, GET_POOL_DETAILS_OPCODE);
    
    if (!poolResponse) {
      // Pool call failed - write empty placeholder
      response.writeU128(u128.Zero);
      continue;
    }
    
    // Write pool details
    response.writeBytes(poolResponse.data);
  }
  
  // Finalize and return
  const finalBuf = response.finalize();
  return changetype<i32>(changetype<usize>(finalBuf));
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
