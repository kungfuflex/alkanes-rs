#!/bin/bash
# Test the experimental ASM bytecode generator for BRC20-Prog unwraps

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ADDRESSES_FILE="$SCRIPT_DIR/.addresses.json"

# Check if addresses file exists
if [ ! -f "$ADDRESSES_FILE" ]; then
    echo "Error: $ADDRESSES_FILE not found."
    echo "Run ./scripts/deploy-brc20-experiment.sh first to deploy the contract."
    exit 1
fi

# Read addresses from JSON
FRBTC_ADDRESS=$(jq -r '.frbtc' "$ADDRESSES_FILE")
RPC_URL=$(jq -r '.rpcUrl' "$ADDRESSES_FILE")

if [ -z "$FRBTC_ADDRESS" ] || [ "$FRBTC_ADDRESS" = "null" ]; then
    echo "Error: Could not read frbtc address from $ADDRESSES_FILE"
    exit 1
fi

echo "Using FrBTC contract at: $FRBTC_ADDRESS"
echo "Using RPC URL: $RPC_URL"
echo ""

# Run the test
"$SCRIPT_DIR/../target/release/alkanes-cli" \
    -p signet \
    --jsonrpc-url https://signet.subfrost.io/v4/subfrost \
    --data-api https://signet.subfrost.io/v4/subfrost \
    --brc20-prog-rpc-url "$RPC_URL" \
    --frbtc-address "$FRBTC_ADDRESS" \
    brc20-prog unwrap --experimental-asm
