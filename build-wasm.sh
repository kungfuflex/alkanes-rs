#!/bin/bash

# Build alkanes.wasm indexer with proper getrandom configuration

set -e

echo "Building WASM indexer for alkanes..."
echo ""

echo "Config: .cargo/config.toml has rustflags for getrandom"
echo ""

# Clean the wasm32 target to force a fresh build
echo "Cleaning wasm32 target..."
rm -rf target/wasm32-unknown-unknown

# Build ONLY the alkanes crate (not the entire workspace)
# This avoids building CLI and other non-WASM compatible crates
echo "Building alkanes crate only..."
cargo build -p alkanes --release --target wasm32-unknown-unknown

echo ""
echo "✅ Build complete!"
echo ""
echo "WASM file location:"
ls -lh target/wasm32-unknown-unknown/release/alkanes.wasm
