#!/bin/bash
set -e

echo "🚀 Deploying Regtest DIESEL/frBTC Pool"
echo "========================================"

# Configuration
FACTORY="4:65522"  # Factory proxy (upgradeable)
DIESEL_ID="2:0"
FRBTC_ID="32:0"
DIESEL_AMOUNT="300000000"  # 300M DIESEL
FRBTC_AMOUNT="50000"       # 0.0005 BTC in sats
ADDR="p2tr:0"
WALLET_FILE="${WALLET_FILE:-$HOME/.alkanes/wallet.json}"
PASSPHRASE="${DEPLOY_PASSWORD:-testtesttest}"
CLI="./target/release/alkanes-cli -p regtest --wallet-file $WALLET_FILE --passphrase $PASSPHRASE"

# Step 0: Fund the wallet
echo ""
echo "💰 Step 0: Funding wallet with Bitcoin..."
$CLI bitcoind generatetoaddress 201 p2tr:0
echo "⏳ Waiting for esplora/metashrew to index blocks (10 seconds)..."
sleep 10
echo "✅ Wallet funded with 201 blocks"

# Step 1: Mine DIESEL
echo ""
echo "📦 Step 1: Mining DIESEL tokens..."
$CLI alkanes execute "[2,0,77]:v0:v0" \
    --to $ADDR \
    --from $ADDR \
    --change $ADDR \
    --auto-confirm

echo "✅ DIESEL mined"

# Step 2: Wrap BTC for frBTC
echo ""
echo "🔄 Step 2: Wrapping BTC to frBTC..."
$CLI alkanes wrap-btc \
    100000000 \
    --to $ADDR \
    --from $ADDR \
    --change $ADDR \
    --auto-confirm

echo "✅ frBTC wrapped"

# Wait for confirmations
echo ""
echo "⏳ Mining a block to confirm transactions..."
$CLI bitcoind generatetoaddress 1 p2tr:0
echo "⏳ Waiting for metashrew to index transactions (15 seconds)..."
sleep 15

# Step 3: Create the pool
echo ""
echo "🏊 Step 3: Creating DIESEL/frBTC pool..."
$CLI alkanes init-pool \
    --pair "$DIESEL_ID,$FRBTC_ID" \
    --liquidity "$DIESEL_AMOUNT:$FRBTC_AMOUNT" \
    --to $ADDR \
    --from $ADDR \
    --change $ADDR \
    --factory "$FACTORY" \
    --auto-confirm \
    --trace

echo ""
echo "✅ Pool created successfully!"
echo ""
echo "🎉 Deployment complete!"
echo ""
echo "You can now query the pool:"
echo "  $CLI dataapi get-pools"
echo "  (factory is at $FACTORY)"
