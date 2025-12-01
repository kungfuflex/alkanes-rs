// Minimal test: just make direct staticcall to unimplemented opcode
// With the fix, should return -1 instead of reverting

import { ExtendedCallResponse } from "./crates/alkanes-cli-common/src/alkanes/asc/alkanes-asm-common/assembly";
import { __staticcall } from "./crates/alkanes-cli-common/src/alkanes/asc/alkanes-asm-common/assembly/alkanes/runtime";

export function __execute(): i32 {
  const response = new ExtendedCallResponse();
  
  // Build cellpack to call alkane 2:0, opcode 102 (unimplemented)
  const cellpack = new ArrayBuffer(48);
  const ptr = changetype<usize>(cellpack);
  store<u64>(ptr, 2);      // block
  store<u64>(ptr + 8, 0);
  store<u64>(ptr + 16, 0);  // tx
  store<u64>(ptr + 24, 0);
  store<u64>(ptr + 32, 102); // UNIMPLEMENTED opcode
  store<u64>(ptr + 40, 0);
  
  const emptyParcel = new ArrayBuffer(16);
  store<u64>(changetype<usize>(emptyParcel), 0);
  store<u64>(changetype<usize>(emptyParcel) + 8, 0);
  
  const emptyStorage = new ArrayBuffer(4);
  store<u32>(changetype<usize>(emptyStorage), 0);
  
  // This should return -1 with the fix
  const result = __staticcall(
    changetype<i32>(ptr),
    changetype<i32>(changetype<usize>(emptyParcel)),
    changetype<i32>(changetype<usize>(emptyStorage)),
    10000
  );
  
  // Return the result
  const resultBytes = new ArrayBuffer(8);
  store<u64>(changetype<usize>(resultBytes), result as u64);
  response.setData(resultBytes);
  
  const finalBuf = response.finalize();
  return changetype<i32>(changetype<usize>(finalBuf));
}
