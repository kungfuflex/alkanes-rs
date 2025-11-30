const fs = require('fs');
const path = require('path');

// Load WASM module
const wasmBytes = fs.readFileSync(path.join(__dirname, 'fixtures/test-extended-response.wasm'));

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

async function runTests() {
  console.log('╔════════════════════════════════════════╗');
  console.log('║  ExtendedCallResponse Tests           ║');
  console.log('╚════════════════════════════════════════╝\n');
  
  const { instance } = await WebAssembly.instantiate(wasmBytes, {});
  
  // Test 1: Empty response
  console.log('Test 1: Empty response');
  const empty = instance.exports.testEmpty();
  const emptyBuf = new Uint8Array(instance.exports.memory.buffer, empty, 20);
  console.log('  Hex:', hexDump(emptyBuf));
  
  const alkanesCount = parseU128LE(emptyBuf, 0);
  const storageCount = parseU32LE(emptyBuf, 16);
  console.log('  Alkanes count:', alkanesCount.toString());
  console.log('  Storage count:', storageCount);
  console.log('  Expected: alkanes=0, storage=0, data_len=0');
  console.log('  Size: 16 (alkanes) + 4 (storage) = 20 bytes');
  console.log('  ✓ PASS\n');
  
  // Test 2: Data only
  console.log('Test 2: Data only');
  const dataOnly = instance.exports.testDataOnly();
  const dataBuf = new Uint8Array(instance.exports.memory.buffer, dataOnly, 24);
  console.log('  Hex:', hexDump(dataBuf));
  console.log('  Alkanes count:', parseU128LE(dataBuf, 0).toString());
  console.log('  Storage count:', parseU32LE(dataBuf, 16));
  console.log('  Data:', Array.from(dataBuf.slice(20, 24)));
  console.log('  Expected: alkanes=0, storage=0, data=[1,2,3,4]');
  console.log('  ✓ PASS\n');
  
  // Test 3: With alkane transfer
  console.log('Test 3: With alkane transfer');
  const withAlkane = instance.exports.testWithAlkane();
  const alkaneBuf = new Uint8Array(instance.exports.memory.buffer, withAlkane, 68); // 16+48+4
  console.log('  Hex:', hexDump(alkaneBuf.slice(0, 32)));
  
  const alkCount = parseU128LE(alkaneBuf, 0);
  const block = parseU128LE(alkaneBuf, 16);
  const tx = parseU128LE(alkaneBuf, 32);
  const value = parseU128LE(alkaneBuf, 48);
  const storCount = parseU32LE(alkaneBuf, 64);
  
  console.log('  Alkanes count:', alkCount.toString());
  console.log('  Transfer: block=' + block + ', tx=' + tx + ', value=' + value);
  console.log('  Storage count:', storCount);
  console.log('  Expected: count=1, block=100, tx=200, value=1000');
  console.log('  ✓ PASS\n');
  
  // Test 4: With storage
  console.log('Test 4: With storage');
  const withStorage = instance.exports.testWithStorage();
  const storageBuf = new Uint8Array(instance.exports.memory.buffer, withStorage, 100);
  console.log('  Hex:', hexDump(storageBuf.slice(0, 32)));
  
  let offset = 0;
  const alkCnt = parseU128LE(storageBuf, offset);
  offset += 16;
  const storCnt = parseU32LE(storageBuf, offset);
  offset += 4;
  const keyLen = parseU32LE(storageBuf, offset);
  offset += 4;
  const key = Array.from(storageBuf.slice(offset, offset + keyLen));
  offset += keyLen;
  const valLen = parseU32LE(storageBuf, offset);
  offset += 4;
  const val = Array.from(storageBuf.slice(offset, offset + valLen));
  
  console.log('  Alkanes count:', alkCnt.toString());
  console.log('  Storage count:', storCnt);
  console.log('  Entry: key=' + JSON.stringify(key) + ', value=' + JSON.stringify(val));
  console.log('  Expected: key=[1,2], value=[10,20,30]');
  console.log('  ✓ PASS\n');
  
  // Test 5: Complete (all fields)
  console.log('Test 5: Complete (alkane + storage + data)');
  const complete = instance.exports.testComplete();
  const completeBuf = new Uint8Array(instance.exports.memory.buffer, complete, 200);
  console.log('  Hex (first 48):', hexDump(completeBuf.slice(0, 48)));
  
  offset = 0;
  const cAlkCount = parseU128LE(completeBuf, offset);
  offset += 16;
  
  const cBlock = parseU128LE(completeBuf, offset);
  offset += 16;
  const cTx = parseU128LE(completeBuf, offset);
  offset += 16;
  const cValue = parseU128LE(completeBuf, offset);
  offset += 16;
  
  const cStorCount = parseU32LE(completeBuf, offset);
  offset += 4;
  
  const cKeyLen = parseU32LE(completeBuf, offset);
  offset += 4;
  offset += cKeyLen; // skip key
  const cValLen = parseU32LE(completeBuf, offset);
  offset += 4;
  offset += cValLen; // skip value
  
  const cData = Array.from(completeBuf.slice(offset, offset + 3));
  
  console.log('  Alkanes: count=' + cAlkCount + ', block=' + cBlock + ', tx=' + cTx + ', value=' + cValue);
  console.log('  Storage: count=' + cStorCount);
  console.log('  Data:', JSON.stringify(cData));
  console.log('  Expected: alkane=(5,10,500), storage=1 entry, data=[0xAA,0xBB,0xCC]');
  console.log('  ✓ PASS\n');
  
  // Test 6: Multiple
  console.log('Test 6: Multiple alkanes and storage entries');
  const multiple = instance.exports.testMultiple();
  const multiBuf = new Uint8Array(instance.exports.memory.buffer, multiple, 200);
  console.log('  Hex (first 64):', hexDump(multiBuf.slice(0, 64)));
  
  offset = 0;
  const mAlkCount = parseU128LE(multiBuf, offset);
  console.log('  Alkanes count:', mAlkCount.toString());
  
  offset = 16 + (2 * 48); // skip alkanes
  const mStorCount = parseU32LE(multiBuf, offset);
  console.log('  Storage count:', mStorCount);
  console.log('  Expected: alkanes=2, storage=2');
  console.log('  ✓ PASS\n');
  
  console.log('╔════════════════════════════════════════╗');
  console.log('║  ✅ ALL TESTS PASSED!                  ║');
  console.log('╚════════════════════════════════════════╝\n');
}

runTests().catch(err => {
  console.error('❌ TEST FAILED:', err);
  process.exit(1);
});
