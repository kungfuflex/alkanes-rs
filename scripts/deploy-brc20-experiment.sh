#!/bin/bash
# Deploy MockFrBTC contract to local Anvil and save address to .addresses.json

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SUBFROST_BRC20_DIR="/data/subfrost-brc20"
ADDRESSES_FILE="$SCRIPT_DIR/.addresses.json"

# Default Anvil private key (account 0)
PRIVATE_KEY="${PRIVATE_KEY:-0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80}"
RPC_URL="${RPC_URL:-http://localhost:8545}"

echo "Deploying MockFrBTC to $RPC_URL..."

# Deploy the contract
cd "$SUBFROST_BRC20_DIR"

# Run deployment and capture output
set +e  # Don't exit on error for the forge command
OUTPUT=$(PRIVATE_KEY="$PRIVATE_KEY" forge script script/Deploy.s.sol:Deploy --rpc-url "$RPC_URL" --broadcast 2>&1)
FORGE_EXIT_CODE=$?
set -e

# Extract the deployed address from the output
FRBTC_ADDRESS=$(echo "$OUTPUT" | grep "MockFrBTC deployed at:" | sed 's/.*deployed at: //' | tr -d ' ')
MOCKLIB_ADDRESS=$(echo "$OUTPUT" | grep "MockBRC20Lib deployed at:" | sed 's/.*deployed at: //' | tr -d ' ')

if [ -z "$FRBTC_ADDRESS" ]; then
    echo "Failed to deploy MockFrBTC."
    echo "Forge exit code: $FORGE_EXIT_CODE"
    echo "Output:"
    echo "$OUTPUT"
    exit 1
fi

echo "MockFrBTC deployed at: $FRBTC_ADDRESS"
echo "MockBRC20Lib deployed at: $MOCKLIB_ADDRESS"

# Add test payments
echo "Adding test payments..."
if ! cast send "$FRBTC_ADDRESS" "addTestPayments()" --rpc-url "$RPC_URL" --private-key "$PRIVATE_KEY" > /dev/null 2>&1; then
    echo "Warning: Failed to add test payments (may already exist)"
fi

# Write addresses to JSON file
cat > "$ADDRESSES_FILE" << EOF
{
  "frbtc": "$FRBTC_ADDRESS",
  "mockBrc20Lib": "$MOCKLIB_ADDRESS",
  "rpcUrl": "$RPC_URL",
  "deployedAt": "$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
}
EOF

echo "Addresses saved to $ADDRESSES_FILE"
echo "Done!"
