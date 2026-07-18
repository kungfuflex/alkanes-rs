// Test fixture: Make a staticcall and return the response
import { u128 } from "as-bignum/assembly";
import { AlkaneResponder, AlkaneId, ExtendedCallResponse } from "../../assembly";

/**
 * Test function: Make a staticcall to a target and return the response
 * Expects inputs:
 *   [0]: target_block
 *   [1]: target_tx
 *   [2]: opcode
 */
export function __execute(): i32 {
  const responder = new AlkaneResponder();
  
  // Load context
  const ctx = responder.loadContext();
  
  // Get target and opcode from inputs
  const targetBlock = ctx.getInput(0);
  const targetTx = ctx.getInput(1);
  const opcode = ctx.getInput(2);
  
  // Create target AlkaneId
  const target = new AlkaneId(targetBlock, targetTx);
  
  // Make staticcall
  const callResult = responder.staticcall(target, opcode);
  
  // Build response
  const response = new ExtendedCallResponse();
  
  if (callResult) {
    // Write success flag
    response.writeU128(u128.One);
    // Write the response data length
    response.writeU128(u128.from(callResult.data.byteLength));
    // Write the response data
    response.writeBytes(callResult.data);
  } else {
    // Write failure flag
    response.writeU128(u128.Zero);
  }
  
  const finalBuf = response.finalize();
  return changetype<i32>(changetype<usize>(finalBuf));
}
