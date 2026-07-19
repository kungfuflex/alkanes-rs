#!/bin/bash

echo "=== Testing esplora address-txs with and without --exclude-coinbase ==="
echo ""

echo "📊 With coinbase transactions:"
./target/release/alkanes-cli --metashrew-rpc-url http://localhost:18888 --jsonrpc-url http://localhost:18888 esplora address-txs p2tr:0 2>&1 | grep "📄.*transactions"

echo ""
echo "📊 Without coinbase transactions (--exclude-coinbase):"
./target/release/alkanes-cli --metashrew-rpc-url http://localhost:18888 --jsonrpc-url http://localhost:18888 esplora address-txs p2tr:0 --exclude-coinbase 2>&1 | grep "📄.*transactions"

echo ""
echo "✅ Test complete!"
