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
  // Create ExtendedCallResponse with NO data to test if serialization works
  const response = new ExtendedCallResponse();
  
  // Don't set any data - just test empty response serialization
  // Expected format: [alkanes_count(16 bytes = 0)][storage_count(4 bytes = 0)][empty data]
  // Total: 20 bytes of zeros
  
  // Finalize to get serialized ExtendedCallResponse
  const result = response.finalize();
  
  // Return pointer to the result ArrayBuffer
  // (alkanes runtime will read length from result ptr - 4)
  return changetype<i32>(changetype<usize>(result));
}
