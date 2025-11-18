#!/bin/bash

set -e

echo "🧪 Testing alkanes backtest functionality"
echo ""

# Configuration
RPC_URL="http://localhost:18888"
TXID="2d95f568908349fd00f88c2f5801e5bf7bac084bc561c7c0a6acc1940fc0de57"

echo "📋 Configuration:"
echo "   RPC URL: $RPC_URL"
echo "   TXID: $TXID"
echo ""

# Step 1: Get the transaction hex
echo "📥 Step 1: Fetching transaction hex..."
TX_HEX=$(curl -s -X POST $RPC_URL \
  -H "Content-Type: application/json" \
  -d "{\"jsonrpc\":\"2.0\",\"method\":\"esplora_tx::hex\",\"params\":[\"$TXID\"],\"id\":1}" \
  | jq -r '.result')

if [ "$TX_HEX" == "null" ] || [ -z "$TX_HEX" ]; then
  echo "❌ Failed to fetch transaction hex"
  exit 1
fi

echo "   ✅ Transaction hex fetched (${#TX_HEX} bytes)"
echo ""

# Step 2: Get current block height
echo "📊 Step 2: Getting current block height..."
CURRENT_HEIGHT=$(curl -s -X POST $RPC_URL \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"getblockcount","params":[],"id":1}' \
  | jq -r '.result')

BLOCK_TAG_BEFORE=$((CURRENT_HEIGHT - 1))

echo "   Current height: $CURRENT_HEIGHT"
echo "   Querying state at height: $BLOCK_TAG_BEFORE"
echo ""

# Step 3: Get previous block hash
echo "🔗 Step 3: Getting previous block hash..."
PREV_BLOCK_HASH=$(curl -s -X POST $RPC_URL \
  -H "Content-Type: application/json" \
  -d "{\"jsonrpc\":\"2.0\",\"method\":\"getblockhash\",\"params\":[$BLOCK_TAG_BEFORE],\"id\":1}" \
  | jq -r '.result')

echo "   Previous block hash: $PREV_BLOCK_HASH"
echo ""

# Step 4: Build simulated block
echo "🏗️  Step 4: Building simulated block..."

# Block header (80 bytes)
# - Version: 2 (4 bytes, little-endian) = 02000000
# - Previous block hash: 32 bytes (reversed)
# - Merkle root: 32 bytes (all zeros for now)
# - Timestamp: 4 bytes (current unix time, little-endian)
# - Bits: 4 bytes = ffff001d (0x1d00ffff in little-endian)
# - Nonce: 4 bytes = 00000000

# Reverse the block hash for block header
PREV_HASH_REVERSED=$(echo $PREV_BLOCK_HASH | fold -w2 | tac | tr -d '\n')

# Get current timestamp
TIMESTAMP=$(date +%s)
TIMESTAMP_HEX=$(printf "%08x" $TIMESTAMP | fold -w2 | tac | tr -d '\n')

# Merkle root (all zeros)
MERKLE_ROOT="0000000000000000000000000000000000000000000000000000000000000000"

# Construct block header
BLOCK_HEADER="02000000${PREV_HASH_REVERSED}${MERKLE_ROOT}${TIMESTAMP_HEX}ffff001d00000000"

# Coinbase transaction (simple, minimal)
# Version: 2
# Input count: 1
# Input: null outpoint (all zeros + ffffffff)
# ScriptSig: empty (just 0x00 for length)
# Sequence: 0xffffffff
# Output count: 1
# Output: 50 BTC to OP_TRUE (or empty script)
# Locktime: 0

COINBASE_TX="020000000001010000000000000000000000000000000000000000000000000000000000000000ffffffff00ffffffff0100f2052a010000000001200000000000000000000000000000000000000000000000000000000000000000000000"

# Transaction count (varint) - we have 2 transactions
TX_COUNT="02"

# Combine into full block
BLOCK_HEX="${BLOCK_HEADER}${TX_COUNT}${COINBASE_TX}${TX_HEX}"

echo "   Block header: ${BLOCK_HEADER}"
echo "   Transaction count: 2 (coinbase + target)"
echo "   Total block size: ${#BLOCK_HEX} bytes"
echo ""

# Step 5: Build trace input data
echo "🎯 Step 5: Preparing trace input data..."
# The trace view function expects:
# 1. Height (u32) - 4 bytes little-endian
# 2. Protobuf-encoded Outpoint message

VOUT=0

# Convert height to 4-byte little-endian hex
HEIGHT_HEX=$(printf "%08x" $BLOCK_TAG_BEFORE | fold -w2 | tac | tr -d '\n')

# Create protobuf Outpoint message
# Protobuf format for message Outpoint { bytes txid = 1; uint32 vout = 2; }
# Field 1 (txid): tag=0x0a (field 1, wire type 2=length-delimited), length=32 (0x20), data=32 bytes
# Field 2 (vout): tag=0x10 (field 2, wire type 0=varint), value=0
PROTOBUF_OUTPOINT="0a20${TXID}1000"

# Combine: height (4 bytes LE) + protobuf outpoint
INPUT_DATA_HEX="${HEIGHT_HEX}${PROTOBUF_OUTPOINT}"

echo "   Trace outpoint: ${TXID}:${VOUT}"
echo "   Height: $BLOCK_TAG_BEFORE (hex: $HEIGHT_HEX)"
echo "   Protobuf outpoint: $PROTOBUF_OUTPOINT"
echo "   Input data: $INPUT_DATA_HEX"
echo "   Input data length: $((${#INPUT_DATA_HEX} / 2)) bytes"
echo ""

# Step 6: Call metashrew_preview
echo "🔍 Step 6: Calling metashrew_preview..."
echo ""

RESPONSE=$(curl -s -X POST $RPC_URL \
  -H "Content-Type: application/json" \
  -d "{
    \"jsonrpc\": \"2.0\",
    \"method\": \"metashrew_preview\",
    \"params\": [
      \"$BLOCK_HEX\",
      \"trace\",
      \"$INPUT_DATA_HEX\",
      \"$BLOCK_TAG_BEFORE\"
    ],
    \"id\": 1
  }")

echo "📤 Response:"
echo "$RESPONSE" | jq '.'
echo ""

# Check if there's an error
ERROR=$(echo "$RESPONSE" | jq -r '.error // empty')
if [ -n "$ERROR" ]; then
  echo "❌ metashrew_preview returned an error:"
  echo "$RESPONSE" | jq '.error'
  echo ""
  echo "This might be expected if:"
  echo "  - The preview function is not implemented"
  echo "  - The transaction doesn't have alkanes operations"
  echo "  - The block format is incorrect"
else
  echo "✅ metashrew_preview call succeeded!"
  echo ""
  TRACE_DATA=$(echo "$RESPONSE" | jq -r '.result.trace // empty')
  if [ -n "$TRACE_DATA" ] && [ "$TRACE_DATA" != "null" ]; then
    echo "📊 Trace data received:"
    echo "$TRACE_DATA"
  fi
fi

echo ""
echo "🧪 Test complete!"
echo ""
echo "To run the full CLI command, use:"
echo "  ./target/release/alkanes-cli --metashrew-rpc-url $RPC_URL --sandshrew-rpc-url $RPC_URL alkanes backtest $TXID"
