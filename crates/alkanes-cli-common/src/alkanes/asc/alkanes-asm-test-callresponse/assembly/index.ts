// Test CallResponse - Minimal test to verify ExtendedCallResponse serialization
//
// This is a minimal test that just creates an ExtendedCallResponse with
// a simple data payload (0x01020304) to verify the serialization works

import { 
  ExtendedCallResponse 
} from "../../alkanes-asm-common/assembly";

/**
 * Main entry point for tx-script execution
 * @returns Pointer to response data
 * 
 * The alkanes runtime expects:
 * - Return value is a pointer to data
 * - At ptr-4, there's a u32 length field
 * 
 * ArrayBuffer in AssemblyScript has this exact layout!
 */
export function __execute(): i32 {
  // Create simple test data: [0x01, 0x02, 0x03, 0x04]
  const testData = new ArrayBuffer(4);
  const dataPtr = changetype<usize>(testData);
  store<u8>(dataPtr + 0, 0x01);
  store<u8>(dataPtr + 1, 0x02);
  store<u8>(dataPtr + 2, 0x03);
  store<u8>(dataPtr + 3, 0x04);
  let response = new ExtendedCallResponse();
  const finalized = response.finalize();
  const finalPtr = changetype<usize>(finalized);
  return changetype<i32>(finalPtr);
}
