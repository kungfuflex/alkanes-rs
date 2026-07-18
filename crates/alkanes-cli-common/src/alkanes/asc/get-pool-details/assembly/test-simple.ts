// Simplest possible tx-script - just return empty response
// This tests if the basic infrastructure works

/**
 * Main entry point
 * @returns Pointer to response data (length at ptr-4)
 */
export function __execute(): i32 {
  const RESPONSE_PTR: usize = 16384;
  let offset = RESPONSE_PTR + 4;
  
  // Write empty alkanes count (16 bytes)
  store<u64>(offset, 0);
  store<u64>(offset + 8, 0);
  offset += 16;
  
  // Write empty storage count (16 bytes)
  store<u64>(offset, 0);
  store<u64>(offset + 8, 0);
  offset += 16;
  
  // Write pool count = 0 (16 bytes)
  store<u64>(offset, 0);
  store<u64>(offset + 8, 0);
  offset += 16;
  
  // Write total length
  store<u32>(RESPONSE_PTR, 48);
  
  return (RESPONSE_PTR + 4) as i32;
}
