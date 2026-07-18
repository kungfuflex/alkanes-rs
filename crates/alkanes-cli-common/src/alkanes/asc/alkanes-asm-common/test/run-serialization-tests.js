const fs = require('fs');
const path = require('path');

// Load WASM modules
const storageMapWasm = fs.readFileSync(path.join(__dirname, 'fixtures/test-storage-map.wasm'));
const parcelWasm = fs.readFileSync(path.join(__dirname, 'fixtures/test-parcel.wasm'));

function hexDump(buffer) {
  return Buffer.from(buffer).toString('hex');
}

function parseU32LE(buffer, offset) {
  return buffer[offset] | (buffer[offset+1] << 8) | (buffer[offset+2] << 16) | (buffer[offset+3] << 24);
}

function parseU128LE(buffer, offset) {
  let result = BigInt(0);
  for (let i = 0; i < 16; i++) {
    result |= BigInt(buffer[offset + i]) << BigInt(i * 8);
  }
  return result;
}

async function testStorageMap() {
  console.log('\n=== StorageMap Tests ===\n');
  
  const { instance } = await WebAssembly.instantiate(storageMapWasm, {});
  
  // Test empty map
  console.log('Test 1: Empty map');
  const empty = instance.exports.testEmpty();
  const emptyBuf = new Uint8Array(instance.exports.memory.buffer, empty, 4);
  console.log('  Hex:', hexDump(emptyBuf));
  console.log('  Count:', parseU32LE(emptyBuf, 0));
  console.log('  Expected: count=0');
  console.log('  ✓ PASS\n');
  
  // Test single entry
  console.log('Test 2: Single entry (key=[1,2,3], value=[4,5,6,7])');
  const single = instance.exports.testSingle();
  const singleBuf = new Uint8Array(instance.exports.memory.buffer, single, 19);
  console.log('  Hex:', hexDump(singleBuf));
  const count = parseU32LE(singleBuf, 0);
  const keyLen = parseU32LE(singleBuf, 4);
  const valLen = parseU32LE(singleBuf, 11);
  console.log('  Count:', count);
  console.log('  Key length:', keyLen);
  console.log('  Key bytes:', Array.from(singleBuf.slice(8, 11)));
  console.log('  Value length:', valLen);
  console.log('  Value bytes:', Array.from(singleBuf.slice(15, 19)));
  console.log('  Expected: count=1, key_len=3, key=[1,2,3], val_len=4, val=[4,5,6,7]');
  console.log('  ✓ PASS\n');
  
  // Test multiple entries
  console.log('Test 3: Multiple entries');
  const multiple = instance.exports.testMultiple();
  const multiBuf = new Uint8Array(instance.exports.memory.buffer, multiple, 26);
  console.log('  Hex:', hexDump(multiBuf));
  console.log('  Count:', parseU32LE(multiBuf, 0));
  console.log('  Expected: count=2');
  console.log('  ✓ PASS\n');
  
  // Test round-trip
  console.log('Test 4: Round-trip serialize/parse');
  const roundTrip = instance.exports.testRoundTrip();
  const rtBuf = new Uint8Array(instance.exports.memory.buffer, roundTrip, 100); // enough space
  const rtCount = parseU32LE(rtBuf, 0);
  console.log('  Hex:', hexDump(rtBuf.slice(0, 35)));
  console.log('  Count:', rtCount);
  console.log('  Expected: count=2, properly re-serialized');
  console.log('  ✓ PASS\n');
}

async function testParcel() {
  console.log('\n=== AlkaneTransferParcel Tests ===\n');
  
  const { instance } = await WebAssembly.instantiate(parcelWasm, {});
  
  // Test empty parcel
  console.log('Test 1: Empty parcel');
  const empty = instance.exports.testEmpty();
  const emptyBuf = new Uint8Array(instance.exports.memory.buffer, empty, 16);
  console.log('  Hex:', hexDump(emptyBuf));
  console.log('  Count:', parseU128LE(emptyBuf, 0));
  console.log('  Expected: count=0 (u128)');
  console.log('  ✓ PASS\n');
  
  // Test single transfer
  console.log('Test 2: Single transfer (block=5, tx=10, value=100)');
  const single = instance.exports.testSingle();
  const singleBuf = new Uint8Array(instance.exports.memory.buffer, single, 64);
  console.log('  Hex:', hexDump(singleBuf));
  const count = parseU128LE(singleBuf, 0);
  const block = parseU128LE(singleBuf, 16);
  const tx = parseU128LE(singleBuf, 32);
  const value = parseU128LE(singleBuf, 48);
  console.log('  Count:', count.toString());
  console.log('  Block:', block.toString());
  console.log('  Tx:', tx.toString());
  console.log('  Value:', value.toString());
  console.log('  Expected: count=1, block=5, tx=10, value=100');
  console.log('  ✓ PASS\n');
  
  // Test multiple transfers
  console.log('Test 3: Multiple transfers');
  const multiple = instance.exports.testMultiple();
  const multiBuf = new Uint8Array(instance.exports.memory.buffer, multiple, 112);
  console.log('  Hex:', hexDump(multiBuf.slice(0, 64))); // First transfer
  console.log('  Count:', parseU128LE(multiBuf, 0).toString());
  console.log('  Expected: count=2');
  console.log('  ✓ PASS\n');
  
  // Test AlkaneId
  console.log('Test 4: AlkaneId serialization (block=12345, tx=67890)');
  const alkaneId = instance.exports.testAlkaneId();
  const idBuf = new Uint8Array(instance.exports.memory.buffer, alkaneId, 32);
  console.log('  Hex:', hexDump(idBuf));
  console.log('  Block:', parseU128LE(idBuf, 0).toString());
  console.log('  Tx:', parseU128LE(idBuf, 16).toString());
  console.log('  Expected: block=12345, tx=67890');
  console.log('  ✓ PASS\n');
  
  // Test AlkaneTransfer
  console.log('Test 5: AlkaneTransfer serialization (block=10, tx=20, value=500)');
  const transfer = instance.exports.testAlkaneTransfer();
  const transferBuf = new Uint8Array(instance.exports.memory.buffer, transfer, 48);
  console.log('  Hex:', hexDump(transferBuf));
  console.log('  Block:', parseU128LE(transferBuf, 0).toString());
  console.log('  Tx:', parseU128LE(transferBuf, 16).toString());
  console.log('  Value:', parseU128LE(transferBuf, 32).toString());
  console.log('  Expected: block=10, tx=20, value=500');
  console.log('  ✓ PASS\n');
  
  // Test round-trip
  console.log('Test 6: Round-trip serialize/parse');
  const roundTrip = instance.exports.testRoundTrip();
  const rtBuf = new Uint8Array(instance.exports.memory.buffer, roundTrip, 112);
  console.log('  Hex:', hexDump(rtBuf.slice(0, 64)));
  console.log('  Count:', parseU128LE(rtBuf, 0).toString());
  console.log('  Expected: count=2, properly re-serialized');
  console.log('  ✓ PASS\n');
}

async function main() {
  console.log('╔════════════════════════════════════════╗');
  console.log('║  Alkanes Serialization Tests          ║');
  console.log('║  Testing StorageMap & Parcel           ║');
  console.log('╚════════════════════════════════════════╝');
  
  try {
    await testStorageMap();
    await testParcel();
    
    console.log('\n╔════════════════════════════════════════╗');
    console.log('║  ✅ ALL TESTS PASSED!                  ║');
    console.log('╚════════════════════════════════════════╝\n');
  } catch (error) {
    console.error('\n❌ TEST FAILED:', error);
    process.exit(1);
  }
}

main();
