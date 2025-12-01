// Minimal test - just return empty response to verify inputs work
import { 
  AlkaneResponder, 
  ExtendedCallResponse,
} from "../../alkanes-asm-common/assembly";

export function __execute(): i32 {
  const responder = new AlkaneResponder();
  
  // Load context to get alkane ID from inputs
  const ctx = responder.loadContext();
  
  // Inputs: [0] = block (u128), [1] = tx (u128)
  const block = ctx.getInput(0);
  const tx = ctx.getInput(1);
  
  // Just return empty data with the inputs to prove we can read them
  const response = new ExtendedCallResponse();
  response.writeU128(block);
  response.writeU128(tx);
  
  // Finalize and return
  const finalBuf = response.finalize();
  return changetype<i32>(changetype<usize>(finalBuf));
}
