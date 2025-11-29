// Test ExecutionContext with redesigned buffer-based inputs

import { AlkaneResponder, ExtendedCallResponse } from "../../alkanes-asm-common/assembly";

export function __execute(): i32 {
  const responder = new AlkaneResponder();
  const response = new ExtendedCallResponse();
  
  // Load context using redesigned ExecutionContext
  const context = responder.loadContext();
  
  // Get inputs buffer directly and return it
  response.setData(context.getInputsBuffer());
  
  const result = response.finalize();
  return changetype<i32>(changetype<usize>(result));
}
