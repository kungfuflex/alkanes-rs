// Test fixture for ExtendedCallResponse serialization
import { u128 } from "as-bignum/assembly";
import { ExtendedCallResponse } from "../../assembly/alkanes/types";

// Helper to create buffer from bytes
function createBuffer(bytes: u8[]): ArrayBuffer {
  const buf = new ArrayBuffer(bytes.length);
  const ptr = changetype<usize>(buf);
  for (let i = 0; i < bytes.length; i++) {
    store<u8>(ptr + i, bytes[i]);
  }
  return buf;
}

/**
 * Test empty response
 * Expected: empty alkanes (16 bytes) + empty storage (4 bytes) + no data = 20 bytes
 */
export function testEmpty(): ArrayBuffer {
  const response = new ExtendedCallResponse();
  return response.finalize();
}

/**
 * Test with data only
 */
export function testDataOnly(): ArrayBuffer {
  const response = new ExtendedCallResponse();
  response.setData(createBuffer([1, 2, 3, 4]));
  return response.finalize();
}

/**
 * Test with alkane transfer
 */
export function testWithAlkane(): ArrayBuffer {
  const response = new ExtendedCallResponse();
  response.addAlkaneTransfer(u128.from(100), u128.from(200), u128.from(1000));
  return response.finalize();
}

/**
 * Test with storage
 */
export function testWithStorage(): ArrayBuffer {
  const response = new ExtendedCallResponse();
  response.setStorage(createBuffer([1, 2]), createBuffer([10, 20, 30]));
  return response.finalize();
}

/**
 * Test with all fields
 */
export function testComplete(): ArrayBuffer {
  const response = new ExtendedCallResponse();
  
  // Add alkane transfer
  response.addAlkaneTransfer(u128.from(5), u128.from(10), u128.from(500));
  
  // Add storage
  response.setStorage(createBuffer([1]), createBuffer([99]));
  
  // Add data
  response.setData(createBuffer([0xAA, 0xBB, 0xCC]));
  
  return response.finalize();
}

/**
 * Test with multiple alkanes and storage entries
 */
export function testMultiple(): ArrayBuffer {
  const response = new ExtendedCallResponse();
  
  // Multiple alkanes
  response.addAlkaneTransfer(u128.from(1), u128.from(2), u128.from(10));
  response.addAlkaneTransfer(u128.from(3), u128.from(4), u128.from(20));
  
  // Multiple storage entries
  response.setStorage(createBuffer([1]), createBuffer([10]));
  response.setStorage(createBuffer([2, 3]), createBuffer([20, 21]));
  
  // Data
  response.appendData(createBuffer([0x01, 0x02]));
  response.appendData(createBuffer([0x03, 0x04]));
  
  return response.finalize();
}
