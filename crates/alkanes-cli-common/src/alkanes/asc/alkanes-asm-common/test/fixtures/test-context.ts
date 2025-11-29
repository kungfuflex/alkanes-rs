// Test fixture: Load context and return the inputs
import { u128 } from "as-bignum/assembly";
import { AlkaneResponder, ExtendedCallResponse } from "../../assembly";

/**
 * Test function: Load context and echo back the first two inputs
 */
export function __execute(): i32 {
  const responder = new AlkaneResponder();
  
  // Load context
  const ctx = responder.loadContext();
  
  // Get first two inputs
  const input0 = ctx.getInput(0);
  const input1 = ctx.getInput(1);
  
  // Build response with the inputs
  const response = new ExtendedCallResponse();
  response.writeU128(input0);
  response.writeU128(input1);
  
  const finalPtr = response.finalize();
  return changetype<i32>(finalPtr);
}
