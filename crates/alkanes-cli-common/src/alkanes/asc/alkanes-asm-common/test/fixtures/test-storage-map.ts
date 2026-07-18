// Test fixture for StorageMap serialization
import { StorageMap } from "../../assembly/storage-map";

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
 * Test empty map
 * Expected: 4 bytes with count=0
 */
export function testEmpty(): ArrayBuffer {
  const map = new StorageMap();
  return map.serialize();
}

/**
 * Test single entry
 * key: [1, 2, 3], value: [4, 5, 6, 7]
 */
export function testSingle(): ArrayBuffer {
  const map = new StorageMap();
  map.set(createBuffer([1, 2, 3]), createBuffer([4, 5, 6, 7]));
  return map.serialize();
}

/**
 * Test multiple entries
 * entry1: key=[1], value=[10]
 * entry2: key=[2, 3], value=[20, 21]
 */
export function testMultiple(): ArrayBuffer {
  const map = new StorageMap();
  map.set(createBuffer([1]), createBuffer([10]));
  map.set(createBuffer([2, 3]), createBuffer([20, 21]));
  return map.serialize();
}

/**
 * Test round-trip: serialize then parse
 */
export function testRoundTrip(): ArrayBuffer {
  const map = new StorageMap();
  map.set(createBuffer([1, 2, 3]), createBuffer([10, 20, 30]));
  map.set(createBuffer([4, 5]), createBuffer([40, 50, 60, 70]));
  
  const serialized = map.serialize();
  const parsed = StorageMap.parse(serialized);
  
  // Re-serialize to verify
  return parsed.serialize();
}
