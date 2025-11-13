import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

// ESM-compatible way to get __dirname
const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

function mapToObject(map) {
  const obj = {};
  for (let [key, value] of map) {
    if (value instanceof Map) {
      obj[key] = mapToObject(value);
    } else if (Array.isArray(value)) {
      obj[key] = value.map(item => item instanceof Map ? mapToObject(item) : item);
    }
    else {
      obj[key] = value;
    }
  }
  return obj;
}

import('deezel-web').then(deezel_web => {
  // Read the block hex from the file
  const blockHexPath = path.join(__dirname, 'block.hex');
  const block_hex = fs.readFileSync(blockHexPath, 'utf8').trim();

  try {
    const block_data = deezel_web.parse_block(block_hex);
    const block_obj = mapToObject(block_data);
    console.log(JSON.stringify(block_obj, null, 2));
  } catch (e) {
    console.error("Error parsing block:", e);
  }
});