import { describe, it, run } from "../node_modules/@as-pect/core/assembly/index";
import { StorageMap } from "../assembly/storage-map";

function createBuffer(bytes: u8[]): ArrayBuffer {
  const buf = new ArrayBuffer(bytes.length);
  const ptr = changetype<usize>(buf);
  for (let i = 0; i < bytes.length; i++) {
    store<u8>(ptr + i, bytes[i]);
  }
  return buf;
}

function buffersEqual(a: ArrayBuffer, b: ArrayBuffer): bool {
  if (a.byteLength != b.byteLength) return false;
  const ptrA = changetype<usize>(a);
  const ptrB = changetype<usize>(b);
  for (let i = 0; i < a.byteLength; i++) {
    if (load<u8>(ptrA + i) != load<u8>(ptrB + i)) {
      return false;
    }
  }
  return true;
}

describe("StorageMap", () => {
  it("serializes empty map correctly", () => {
    const map = new StorageMap();
    const serialized = map.serialize();
    
    // Should be 4 bytes for count = 0
    assert(serialized.byteLength == 4, "Empty map should be 4 bytes");
    
    const ptr = changetype<usize>(serialized);
    const count = load<u32>(ptr);
    assert(count == 0, "Count should be 0");
  });

  it("serializes single entry correctly", () => {
    const map = new StorageMap();
    
    const key = createBuffer([1, 2, 3]);
    const value = createBuffer([4, 5, 6, 7]);
    
    map.set(key, value);
    
    const serialized = map.serialize();
    
    // Expected size: 4 (count) + 4 (key_len) + 3 (key) + 4 (val_len) + 4 (val) = 19
    assert(serialized.byteLength == 19, "Serialized size should be 19 bytes");
    
    let ptr = changetype<usize>(serialized);
    
    // Check count
    assert(load<u32>(ptr) == 1, "Count should be 1");
    ptr += 4;
    
    // Check key_length
    assert(load<u32>(ptr) == 3, "Key length should be 3");
    ptr += 4;
    
    // Check key bytes
    assert(load<u8>(ptr) == 1, "Key[0] should be 1");
    assert(load<u8>(ptr + 1) == 2, "Key[1] should be 2");
    assert(load<u8>(ptr + 2) == 3, "Key[2] should be 3");
    ptr += 3;
    
    // Check value_length
    assert(load<u32>(ptr) == 4, "Value length should be 4");
    ptr += 4;
    
    // Check value bytes
    assert(load<u8>(ptr) == 4, "Value[0] should be 4");
    assert(load<u8>(ptr + 1) == 5, "Value[1] should be 5");
    assert(load<u8>(ptr + 2) == 6, "Value[2] should be 6");
    assert(load<u8>(ptr + 3) == 7, "Value[3] should be 7");
  });

  it("serializes multiple entries correctly", () => {
    const map = new StorageMap();
    
    map.set(createBuffer([1]), createBuffer([10]));
    map.set(createBuffer([2, 3]), createBuffer([20, 21]));
    
    const serialized = map.serialize();
    
    // Expected: 4 (count) + (4+1+4+1) + (4+2+4+2) = 4 + 10 + 12 = 26
    assert(serialized.byteLength == 26, "Serialized size should be 26 bytes");
    
    const ptr = changetype<usize>(serialized);
    assert(load<u32>(ptr) == 2, "Count should be 2");
  });

  it("round-trips serialize/parse correctly", () => {
    const map = new StorageMap();
    
    const key1 = createBuffer([1, 2, 3]);
    const val1 = createBuffer([10, 20, 30]);
    const key2 = createBuffer([4, 5]);
    const val2 = createBuffer([40, 50, 60, 70]);
    
    map.set(key1, val1);
    map.set(key2, val2);
    
    const serialized = map.serialize();
    const parsed = StorageMap.parse(serialized);
    
    assert(parsed.entries.length == 2, "Parsed map should have 2 entries");
    
    // Check first entry
    assert(buffersEqual(parsed.entries[0].key, key1), "First key should match");
    assert(buffersEqual(parsed.entries[0].value, val1), "First value should match");
    
    // Check second entry
    assert(buffersEqual(parsed.entries[1].key, key2), "Second key should match");
    assert(buffersEqual(parsed.entries[1].value, val2), "Second value should match");
  });

  it("get/set work correctly", () => {
    const map = new StorageMap();
    
    const key = createBuffer([1, 2, 3]);
    const value = createBuffer([10, 20, 30]);
    
    map.set(key, value);
    
    const retrieved = map.get(key);
    assert(retrieved != null, "Should retrieve value");
    assert(buffersEqual(retrieved!, value), "Retrieved value should match");
    
    const notFound = map.get(createBuffer([9, 9, 9]));
    assert(notFound == null, "Non-existent key should return null");
  });

  it("updates existing key", () => {
    const map = new StorageMap();
    
    const key = createBuffer([1, 2]);
    const value1 = createBuffer([10]);
    const value2 = createBuffer([20, 30]);
    
    map.set(key, value1);
    assert(map.entries.length == 1, "Should have 1 entry");
    
    map.set(key, value2);
    assert(map.entries.length == 1, "Should still have 1 entry");
    
    const retrieved = map.get(key);
    assert(buffersEqual(retrieved!, value2), "Should get updated value");
  });
});

run();
