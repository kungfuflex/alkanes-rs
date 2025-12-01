/**
 * Test program for "safe_staticcall" pattern using CloneFuture (5:n) sandboxing
 * 
 * Strategy:
 * 1. Tx-script creates alkane at 2:n (read from context.myself)
 * 2. Outer frame: Use CloneFuture (5:n) to clone ourselves with magic opcode
 * 3. Inner frame (cloned): when we see magic opcode, staticcall the UNIMPLEMENTED opcode
 * 4. The clone creates a NEW alkane (2:m) which makes the risky call
 * 5. If inner frame reverts, maybe the separation from cloning lets us catch it?
 * 
 * This tests if CloneFuture provides enough isolation to catch reverts
 */

import { ExtendedCallResponse, CallResponse, u128 } from "../../alkanes-asm-common/assembly";
import { __staticcall, __call, __request_context, __load_context, __sequence, abort } from "../../alkanes-asm-common/assembly/alkanes/runtime";

const MAGIC_OPCODE: u64 = 0xFFFFFFFFFFFFFFFF; // ~0u64 - signals cloned call

/**
 * Main entry point
 */
export function __execute(): i32 {
  // Load context to get inputs
  const contextSize = __request_context();
  const contextBuf = new ArrayBuffer(contextSize);
  __load_context(changetype<i32>(changetype<usize>(contextBuf)));
  
  // Context format: [myself(32)][caller(32)][vout(16)][incoming_alkanes_count(16)][inputs...]
  const contextPtr = changetype<usize>(contextBuf);
  
  // Read myself (the tx-script alkane ID - this is 2:n)
  const myselfBlockLo = load<u64>(contextPtr);
  const myselfBlockHi = load<u64>(contextPtr + 8);
  const myselfTxLo = load<u64>(contextPtr + 16);
  const myselfTxHi = load<u64>(contextPtr + 24);
  
  // Read inputs to determine if this is outer or inner frame
  const inputsOffset: i32 = 96;
  const opcode = contextSize > inputsOffset ? load<u64>(contextPtr + inputsOffset as usize) : 0;
  
  const response = new ExtendedCallResponse();
  
  if (opcode == MAGIC_OPCODE) {
    // INNER FRAME: We were called via CloneFuture (5:n)
    // Now make the risky staticcall to an unimplemented opcode
    const targetBlockLo: u64 = 2;
    const targetBlockHi: u64 = 0;
    const targetTxLo: u64 = 0;
    const targetTxHi: u64 = 0;
    
    const cellpack = new ArrayBuffer(48);
    const cellpackPtr = changetype<usize>(cellpack);
    store<u64>(cellpackPtr, targetBlockLo);
    store<u64>(cellpackPtr + 8, targetBlockHi);
    store<u64>(cellpackPtr + 16, targetTxLo);
    store<u64>(cellpackPtr + 24, targetTxHi);
    store<u64>(cellpackPtr + 32, 102); // UNIMPLEMENTED opcode on 2:0
    store<u64>(cellpackPtr + 40, 0);
    
    const emptyParcel = new ArrayBuffer(16);
    store<u64>(changetype<usize>(emptyParcel), 0);
    store<u64>(changetype<usize>(emptyParcel) + 8, 0);
    
    const emptyStorage = new ArrayBuffer(4);
    store<u32>(changetype<usize>(emptyStorage), 0);
    
    // This SHOULD now return -1 instead of hard reverting
    const result = __staticcall(
      changetype<i32>(cellpackPtr),
      changetype<i32>(changetype<usize>(emptyParcel)),
      changetype<i32>(changetype<usize>(emptyStorage)),
      10000
    );
    
    // Pack the result - should be negative if call failed
    const resultBytes = new ArrayBuffer(8);
    store<u64>(changetype<usize>(resultBytes), result as u64);
    response.setData(resultBytes);
    
  } else {
    // OUTER FRAME: Use CloneFuture (5:n) to clone ourselves and sandbox the failing call
    // CloneFuture format: block=5, tx=<sequence_of_alkane_to_clone>
    // This will create a NEW alkane (2:m) that is a copy of us (2:n)
    
    const cellpack = new ArrayBuffer(48);
    const cellpackPtr = changetype<usize>(cellpack);
    store<u64>(cellpackPtr, 5); // CloneFuture block
    store<u64>(cellpackPtr + 8, 0);
    store<u64>(cellpackPtr + 16, myselfTxLo); // Our sequence number (n from 2:n)
    store<u64>(cellpackPtr + 24, myselfTxHi);
    store<u64>(cellpackPtr + 32, MAGIC_OPCODE); // Magic opcode - tells clone to make risky call
    store<u64>(cellpackPtr + 40, 0);
    
    const emptyParcel = new ArrayBuffer(16);
    store<u64>(changetype<usize>(emptyParcel), 0);
    store<u64>(changetype<usize>(emptyParcel) + 8, 0);
    
    const emptyStorage = new ArrayBuffer(4);
    store<u32>(changetype<usize>(emptyStorage), 0);
    
    // Use __call instead of __staticcall - maybe regular call handles errors differently?
    const result = __call(
      changetype<i32>(cellpackPtr),
      changetype<i32>(changetype<usize>(emptyParcel)),
      changetype<i32>(changetype<usize>(emptyStorage)),
      50000 // More fuel for recursive call
    );
    
    // Pack the result - should be negative if inner frame reverted
    const resultBytes = new ArrayBuffer(8);
    const resultAsU64 = result < 0 
      ? (0xFFFFFFFF00000000 | (result as u64))
      : (result as u64);
    store<u64>(changetype<usize>(resultBytes), resultAsU64);
    response.setData(resultBytes);
  }
  
  const finalBuf = response.finalize();
  return changetype<i32>(changetype<usize>(finalBuf));
}
