// Test fixture: Test ExecutionContext loading and parsing
import { u128 } from "as-bignum/assembly";
import { AlkaneResponder } from "../../assembly";

/**
 * Test function: Load ExecutionContext and return its components
 * Returns:
 *   [myself_block(16)][myself_tx(16)]
 *   [caller_block(16)][caller_tx(16)]
 *   [vout(16)]
 *   [incoming_alkanes_count(16)]
 *   [input_count(16)]
 *   [input0(16)][input1(16)]...
 */
export function __execute(): i32 {
  const responder = new AlkaneResponder();
  
  // Load context
  const ctx = responder.loadContext();
  
  // Build response buffer - allocate enough for all fields
  // myself(32) + caller(32) + vout(16) + count(16) + input_count(16) + inputs(variable)
  const bufferSize = 32 + 32 + 16 + 16 + 16 + (ctx.inputs.length * 16);
  const buffer = new ArrayBuffer(bufferSize);
  const ptr = changetype<usize>(buffer);
  let offset: usize = 0;
  
  // Write myself (AlkaneId = 32 bytes)
  const myselfBytes = ctx.myself.toArrayBuffer();
  memory.copy(ptr + offset, changetype<usize>(myselfBytes), 32);
  offset += 32;
  
  // Write caller (AlkaneId = 32 bytes)
  const callerBytes = ctx.caller.toArrayBuffer();
  memory.copy(ptr + offset, changetype<usize>(callerBytes), 32);
  offset += 32;
  
  // Write vout (u128 = 16 bytes)
  store<u64>(ptr + offset, ctx.vout.lo);
  store<u64>(ptr + offset + 8, ctx.vout.hi);
  offset += 16;
  
  // Write incoming alkanes count (u128 = 16 bytes)
  store<u64>(ptr + offset, ctx.incomingAlkanesCount.lo);
  store<u64>(ptr + offset + 8, ctx.incomingAlkanesCount.hi);
  offset += 16;
  
  // Write input count (u128 = 16 bytes)
  const inputCount = ctx.inputs.length;
  store<u64>(ptr + offset, inputCount);
  store<u64>(ptr + offset + 8, 0);
  offset += 16;
  
  // Write each input (u128 = 16 bytes each)
  for (let i = 0; i < inputCount; i++) {
    const input = ctx.inputs[i];
    store<u64>(ptr + offset, input.lo);
    store<u64>(ptr + offset + 8, input.hi);
    offset += 16;
  }
  
  return changetype<i32>(ptr);
}
