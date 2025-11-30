// Minimal __staticcall test - directly call host function with manually serialized data
//
// This bypasses ALL helper methods to isolate the staticcall issue.
// We manually serialize:
// - Cellpack: target 4:65522, inputs [3] (GET_ALL_POOLS opcode)
// - Empty AlkaneTransferParcel
// - Empty StorageMap
//
// Then call __staticcall directly and return whatever we get

import { ExtendedCallResponse, CallResponse } from "../../alkanes-asm-common/assembly";
import { __staticcall, __returndatacopy } from "../../alkanes-asm-common/assembly/alkanes/runtime";

export function __execute(): i32 {
  const response = new ExtendedCallResponse();
  
  // Manually serialize Cellpack for factory 4:65522 with opcode 3
  // Cellpack format (matches Rust): [target_block(16)][target_tx(16)][input0(16)]...
  // NO INPUT COUNT! Just raw inputs appended after target.
  const cellpackSize = 16 + 16 + 16; // target (block+tx) + 1 input
  const cellpack = new ArrayBuffer(cellpackSize);
  const cellpackPtr = changetype<usize>(cellpack);
  
  // Target: block=4, tx=65522 (0xFFF2)
  store<u64>(cellpackPtr, 4);      // block low
  store<u64>(cellpackPtr + 8, 0);  // block high
  store<u64>(cellpackPtr + 16, 65522); // tx low (0xFFF2)
  store<u64>(cellpackPtr + 24, 0);     // tx high
  
  // Input 0: opcode 3 (GET_ALL_POOLS)
  store<u64>(cellpackPtr + 32, 3); // input low
  store<u64>(cellpackPtr + 40, 0); // input high
  
  // Empty AlkaneTransferParcel: just count=0
  const emptyParcel = new ArrayBuffer(16);
  const parcelPtr = changetype<usize>(emptyParcel);
  store<u64>(parcelPtr, 0);     // count low
  store<u64>(parcelPtr + 8, 0); // count high
  
  // Empty StorageMap: just count=0
  const emptyStorage = new ArrayBuffer(4);
  const storagePtr = changetype<usize>(emptyStorage);
  store<u32>(storagePtr, 0); // count
  
  // Call __staticcall directly
  // signature: __staticcall(cellpack: i32, incoming_alkanes: i32, checkpoint: i32, start_fuel: u64) -> i32
  // Returns: <0 for error (abs is returndata size), >=0 for success (value is returndata size)
  const resultCode = __staticcall(
    changetype<i32>(cellpackPtr),
    changetype<i32>(parcelPtr),
    changetype<i32>(storagePtr),
    0xFFFFFFFFFFFFFFFF // max fuel
  );
  
  // Check if call failed
  if (resultCode < 0) {
    // Failed - could get error message via __returndatacopy but just return empty for now
    response.setData(new ArrayBuffer(0));
  } else {
    // Success! resultCode is the size of returndata
    // Allocate buffer and call __returndatacopy to populate it
    const returndata = new ArrayBuffer(resultCode);
    __returndatacopy(changetype<i32>(changetype<usize>(returndata)));
    
    // Now returndata contains a CallResponse: [AlkaneTransferParcel][data]
    // Parse it to extract just the data field
    const callResponse = CallResponse.fromBytes(returndata);
    
    // Return just the data field (the actual pool list)
    response.setData(callResponse.data);
  }
  
  const finalResult = response.finalize();
  return changetype<i32>(changetype<usize>(finalResult));
}
