#!/usr/bin/env node

/**
 * Script to vendor prod WASM files into ts-sdk/build/contracts/
 * This makes ts-sdk self-contained and portable.
 */

const fs = require('fs');
const path = require('path');

const SOURCE_DIR = path.join(__dirname, '../../prod_wasms');
const TARGET_DIR = path.join(__dirname, '../build/contracts');

function ensureDir(dir) {
  if (!fs.existsSync(dir)) {
    fs.mkdirSync(dir, { recursive: true });
    console.log(`✓ Created directory: ${dir}`);
  }
}

function copyWasmFiles() {
  console.log('📦 Vendoring WASM files from prod_wasms...\n');
  
  if (!fs.existsSync(SOURCE_DIR)) {
    console.error(`❌ Source directory not found: ${SOURCE_DIR}`);
    process.exit(1);
  }

  ensureDir(TARGET_DIR);

  const files = fs.readdirSync(SOURCE_DIR);
  const wasmFiles = files.filter(f => f.endsWith('.wasm'));

  if (wasmFiles.length === 0) {
    console.warn('⚠️  No WASM files found in prod_wasms directory');
    return;
  }

  let copiedCount = 0;
  wasmFiles.forEach(file => {
    const source = path.join(SOURCE_DIR, file);
    const target = path.join(TARGET_DIR, file);
    
    fs.copyFileSync(source, target);
    copiedCount++;
    console.log(`  ✓ ${file}`);
  });

  console.log(`\n✅ Successfully vendored ${copiedCount} WASM files to build/contracts/\n`);
}

try {
  copyWasmFiles();
} catch (error) {
  console.error('❌ Error vendoring WASM files:', error.message);
  process.exit(1);
}
