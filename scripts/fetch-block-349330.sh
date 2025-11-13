#!/bin/bash
set -e

BLOCK_HEIGHT=349330
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

echo "Fetching Zcash block $BLOCK_HEIGHT"
echo ""

# First get the block hash for this height
echo "Getting block hash for height $BLOCK_HEIGHT..."
BLOCK_HASH=$(curl -s --data-binary "{\"jsonrpc\":\"1.0\",\"id\":\"test\",\"method\":\"getblockhash\",\"params\":[$BLOCK_HEIGHT]}" \
  -H 'content-type: text/plain;' http://localhost:8232/ | jq -r '.result')

if [ -z "$BLOCK_HASH" ] || [ "$BLOCK_HASH" = "null" ]; then
  echo "Error: Failed to fetch block hash for height $BLOCK_HEIGHT"
  echo "Make sure your Zcash node is running and synced to at least block $BLOCK_HEIGHT"
  exit 1
fi

echo "Block hash: $BLOCK_HASH"
echo ""

# Get the block bytes from the Zcash node
echo "Fetching block data..."
BLOCK_DATA=$(curl -s --data-binary "{\"jsonrpc\":\"1.0\",\"id\":\"test\",\"method\":\"getblock\",\"params\":[\"$BLOCK_HASH\", 0]}" \
  -H 'content-type: text/plain;' http://localhost:8232/ | jq -r '.result')

if [ -z "$BLOCK_DATA" ] || [ "$BLOCK_DATA" = "null" ]; then
  echo "Error: Failed to fetch block data"
  exit 1
fi

echo "Block data fetched successfully (${#BLOCK_DATA} hex characters, $((${#BLOCK_DATA}/2)) bytes)"
echo ""

# Save to the test blocks directory
BLOCKS_DIR="$PROJECT_ROOT/crates/alkanes/src/tests/blocks"
mkdir -p "$BLOCKS_DIR"
OUTPUT_FILE="$BLOCKS_DIR/zec_349330.hex"

echo "$BLOCK_DATA" > "$OUTPUT_FILE"

echo "Block saved to: $OUTPUT_FILE"
echo ""
echo "You can now create a test file for this block in:"
echo "  $PROJECT_ROOT/crates/alkanes/src/tests/zcash_block_349330.rs"
