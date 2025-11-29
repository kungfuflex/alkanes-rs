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
 * - Return value is a pointer to the DATA (not the ArrayBuffer object)
 * - At (return_value - 4), there must be a u32 length field
 */
export function __execute(): i32 {
  // Create ExtendedCallResponse
  const response = new ExtendedCallResponse();
  
  // Create test data: [0x01, 0x02, 0x03, 0x04]
  const testData = new ArrayBuffer(4);
  const dataPtr = changetype<usize>(testData);
  store<u8>(dataPtr + 0, 0x01);
  store<u8>(dataPtr + 1, 0x02);
  store<u8>(dataPtr + 2, 0x03);
  store<u8>(dataPtr + 3, 0x04);
  
  // Set the data in response
  response.setData(testData);
  
  // Expected ExtendedCallResponse format:
  // [alkanes_count(16 bytes = 0)][storage_count(4 bytes = 0)][data(4 bytes = 01020304)]
  // Total: 24 bytes
  
  // Finalize to get serialized ExtendedCallResponse
  const result = response.finalize();
  
  // Return pointer to the result ArrayBuffer
  // (alkanes runtime will read length from result ptr - 4)
  return changetype<i32>(changetype<usize>(result));
}
