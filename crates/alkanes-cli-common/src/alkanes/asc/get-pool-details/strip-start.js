#!/usr/bin/env node
// Strip the start function from WASM binary
// Usage: node strip-start.js input.wasm output.wasm

const fs = require('fs');

if (process.argv.length < 4) {
  console.error('Usage: node strip-start.js input.wasm output.wasm');
  process.exit(1);
}

const inputPath = process.argv[2];
const outputPath = process.argv[3];

// Read WASM binary
const buffer = fs.readFileSync(inputPath);

// WASM sections:
// 0 = custom, 1 = type, 2 = import, 3 = function, 4 = table, 5 = memory, 
// 6 = global, 7 = export, 8 = start, 9 = element, 10 = code, 11 = data, 12 = datacount

let offset = 8; // Skip magic number and version
const output = [buffer.slice(0, offset)]; // Keep header

while (offset < buffer.length) {
  const sectionId = buffer[offset];
  offset++;
  
  // Read LEB128 section size
  let size = 0;
  let shift = 0;
  let sizeStart = offset;
  while (true) {
    const byte = buffer[offset++];
    size |= (byte & 0x7f) << shift;
    if ((byte & 0x80) === 0) break;
    shift += 7;
  }
  
  const sizeBytes = buffer.slice(sizeStart, offset);
  const sectionData = buffer.slice(offset, offset + size);
  
  // Skip start section (id = 8)
  if (sectionId !== 8) {
    output.push(Buffer.from([sectionId]));
    output.push(sizeBytes);
    output.push(sectionData);
  } else {
    console.log('Stripped start section');
  }
  
  offset += size;
}

// Write output
fs.writeFileSync(outputPath, Buffer.concat(output));
console.log(`Wrote ${outputPath}`);
