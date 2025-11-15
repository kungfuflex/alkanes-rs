#!/bin/bash

# Build script for alkanes-rs ts-sdk and integration with @oyl/sdk

set -e

echo "📦 Building @alkanes/ts-sdk..."

# Step 1: Build WASM (already completed)
echo "✅ WASM module already built at wasm-pkg/"

# Step 2: Build TypeScript SDK
echo "🔨 Building TypeScript SDK..."
npx tsup src/index.ts --format cjs,esm --dts --clean

echo "✅ Build completed!"
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
