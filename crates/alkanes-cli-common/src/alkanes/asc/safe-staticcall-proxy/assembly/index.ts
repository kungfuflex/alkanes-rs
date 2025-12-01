/**
 * Safe Staticcall Proxy Alkane
 * 
 * This alkane acts as a proxy for making staticcalls that might revert.
 * 
 * When called with inputs [target_block_lo, target_block_hi, target_tx_lo, target_tx_hi, opcode_lo, opcode_hi],
 * it will staticcall the target alkane with the specified opcode.
 * 
 * If the target reverts, this alkane returns an error response with data = [0xFF...] (all 0xFF bytes).
 * If the target succeeds, this alkane returns the target's response data.
 * 
 * The caller can deploy this alkane using the CloneFuture pattern (5:n) to create instances as needed.
 */

import { ExtendedCallResponse, CallResponse, u128 } from "../../alkanes-asm-common/assembly";
import { __staticcall, __returndatacopy, __request_context, __load_context } from "../../alkanes-asm-common/assembly/alkanes/runtime";

/**
 * Main entry point
 * 
 * Inputs: [target_block_lo, target_block_hi, target_tx_lo, target_tx_hi, opcode_lo, opcode_hi]
 * Each input is a u128 (16 bytes), so total 96 bytes
 */
export function __execute(): i32 {
  // Load context to get inputs
  const contextSize = __request_context();
  const contextBuf = new ArrayBuffer(contextSize);
  __load_context(changetype<i32>(changetype<usize>(contextBuf)));
  
  // Context format: [myself(32)][caller(32)][vout(16)][incoming_alkanes_count(16)][inputs...]
  const contextPtr = changetype<usize>(contextBuf);
  const inputsOffset: i32 = 96;
  
  const response = new ExtendedCallResponse();
  
  // Check if we have enough inputs (6 u128s = 96 bytes)
  if (contextSize < (inputsOffset + 96)) {
    // Not enough inputs - return error
    const errorData = new ArrayBuffer(1);
    store<u8>(changetype<usize>(errorData), 0xFF);
    response.setData(errorData);
    const finalBuf = response.finalize();
    return changetype<i32>(changetype<usize>(finalBuf));
  }
  
  // Read inputs: target alkane ID and opcode
  const inputBase = contextPtr + (inputsOffset as usize);
  const targetBlockLo = load<u64>(inputBase);
  const targetBlockHi = load<u64>(inputBase + 8);
  const targetTxLo = load<u64>(inputBase + 16);
  const targetTxHi = load<u64>(inputBase + 24);
  const opcodeLo = load<u64>(inputBase + 32);
  const opcodeHi = load<u64>(inputBase + 40);
  
  // Build cellpack for staticcall
  const cellpack = new ArrayBuffer(48);
  const cellpackPtr = changetype<usize>(cellpack);
  store<u64>(cellpackPtr, targetBlockLo);
  store<u64>(cellpackPtr + 8, targetBlockHi);
  store<u64>(cellpackPtr + 16, targetTxLo);
  store<u64>(cellpackPtr + 24, targetTxHi);
  store<u64>(cellpackPtr + 32, opcodeLo);
  store<u64>(cellpackPtr + 40, opcodeHi);
  
  // Empty parcel and storage
  const emptyParcel = new ArrayBuffer(16);
  store<u64>(changetype<usize>(emptyParcel), 0);
  store<u64>(changetype<usize>(emptyParcel) + 8, 0);
  
  const emptyStorage = new ArrayBuffer(4);
  store<u32>(changetype<usize>(emptyStorage), 0);
  
  // Make the staticcall
  const result = __staticcall(
    changetype<i32>(cellpackPtr),
    changetype<i32>(changetype<usize>(emptyParcel)),
    changetype<i32>(changetype<usize>(emptyStorage)),
    100000 // Reasonable fuel limit
  );
  
  // If result < 0, the call failed - return error marker
  if (result < 0) {
    const errorData = new ArrayBuffer(1);
    store<u8>(changetype<usize>(errorData), 0xFF);
    response.setData(errorData);
  } else {
    // Call succeeded - copy return data
    const returnData = new ArrayBuffer(result);
    __returndatacopy(changetype<i32>(changetype<usize>(returnData)));
    response.setData(returnData);
  }
  
  const finalBuf = response.finalize();
  return changetype<i32>(changetype<usize>(finalBuf));
}
