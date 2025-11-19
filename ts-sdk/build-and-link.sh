#!/bin/bash

# Build script for alkanes-rs ts-sdk and integration with @oyl/sdk

set -e

echo "📦 Building @alkanes/ts-sdk..."

# Step 1: Build WASM module
echo "🔨 Building WASM module..."
npm run build:wasm

# Step 2: Vendor prod WASM files
echo "📦 Vendoring prod WASM files..."
npm run build:vendor

# Step 3: Build TypeScript SDK
echo "🔨 Building TypeScript SDK..."
npm run build:ts

echo "✅ Build completed!"
echo ""
echo "📦 All WASM files are now vendored in build/ directory"
echo "   - build/wasm/ - alkanes-web-sys WASM module"
echo "   - build/contracts/ - production contract WASMs"
echo ""
echo "📋 Next steps:"
echo "1. Install @oyl/sdk in your project:"
echo "   npm install @oyl/sdk"
echo ""
echo "2. Link this SDK locally:"
echo "   npm link (from this directory)"
echo "   npm link @alkanes/ts-sdk (from your project directory)"
echo ""
echo "3. Or install from npm once published:"
echo "   npm install @alkanes/ts-sdk"
