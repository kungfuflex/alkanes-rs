#!/bin/bash

# Test if WASM can be loaded and executed
cd /data/alkanes-rs-zcash

# Get block 0 hex
BLOCK_HEX=$(curl -s -X POST http://localhost:8232 -H "Content-Type: application/json" -d '{"jsonrpc":"2.0","method":"getblock","params":["00040fe8ec8471911baa1db1266ea15dd06b4a8a5c453883c000b031973dce08",0],"id":1}' | jq -r '.result')

# Convert to binary file
echo "Block hex length: ${#BLOCK_HEX}"

# Create a test input: 4 bytes height (0) + block data
echo -n -e '\x00\x00\x00\x00' > /tmp/test_input.bin
echo -n "$BLOCK_HEX" | xxd -r -p >> /tmp/test_input.bin

echo "Input file size: $(stat -c%s /tmp/test_input.bin)"

# Try to run with wasmtime if available
if command -v wasmtime &> /dev/null; then
    echo "Testing WASM with wasmtime..."
    wasmtime --invoke _start target/wasm32-unknown-unknown/release/alkanes.wasm < /tmp/test_input.bin
else
    echo "wasmtime not found, cannot test WASM directly"
fi
