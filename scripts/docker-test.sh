#!/bin/bash
# Test script for alkanes-rs Docker environment

set -e

BASE_URL="http://localhost:18888"

echo "🧪 Testing Alkanes-RS Docker Environment"
echo "========================================"
echo ""

# Function to test a JSON-RPC call
test_call() {
    local name="$1"
    local method="$2"
    local params="$3"
    
    echo "Testing: $name"
    echo "  Method: $method"
    
    RESPONSE=$(curl -s -X POST "$BASE_URL" \
        -H "Content-Type: application/json" \
        -d "{\"jsonrpc\":\"2.0\",\"method\":\"$method\",\"params\":$params,\"id\":1}")
    
    if echo "$RESPONSE" | grep -q '"result"'; then
        RESULT=$(echo "$RESPONSE" | jq -r '.result')
        echo "  ✅ Success: $RESULT"
    else
        ERROR=$(echo "$RESPONSE" | jq -r '.error.message // "Unknown error"')
        echo "  ❌ Error: $ERROR"
    fi
    echo ""
}

# Test bitcoind
echo "=== Bitcoin Core RPC ==="
test_call "Get block count" "getblockcount" "[]"
test_call "Get blockchain info" "getblockchaininfo" "[]"

# Test metashrew
echo "=== Metashrew ==="
test_call "Metashrew height" "metashrew_height" "[]"

# Test ord
echo "=== Ord ==="
test_call "Ord block count" "ord_blockcount" "[]"

# Test esplora
echo "=== Esplora ==="
test_call "Esplora tip height" "esplora_blocks:tip:height" "[]"

echo "✅ All tests complete!"
echo ""
echo "Use alkanes-cli for more comprehensive testing:"
echo "  cargo build --release -p alkanes-cli"
echo "  ./target/release/alkanes-cli -p regtest --sandshrew-rpc-url $BASE_URL bitcoind getblockcount"
