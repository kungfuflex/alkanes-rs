#!/bin/bash
set -e

echo "🚀 Deploying Regtest DIESEL/frBTC Pool"
echo "========================================"

# Configuration
FACTORY="4:65522"
DIESEL_ID="2:0"
FRBTC_ID="32:0"
DIESEL_AMOUNT="300000000"  # 300M DIESEL
FRBTC_AMOUNT="50000"       # 0.0005 BTC in sats
ADDR="p2tr:0"

# Step 1: Mine DIESEL
echo ""
echo "📦 Step 1: Mining DIESEL tokens..."
alkanes-cli alkanes execute "[2,0,77]:v0:v0" \
    --to $ADDR \
    --from $ADDR \
    --change $ADDR \
    --auto-confirm

echo "✅ DIESEL mined"

# Step 2: Wrap BTC for frBTC
echo ""
echo "🔄 Step 2: Wrapping BTC to frBTC..."
alkanes-cli alkanes wrap-btc \
    100000000 \
    --from $ADDR \
    --change $ADDR \
    --auto-confirm

echo "✅ frBTC wrapped"

# Wait for confirmations
echo ""
echo "⏳ Waiting for confirmations..."
sleep 5

# Step 3: Create the pool
echo ""
echo "🏊 Step 3: Creating DIESEL/frBTC pool..."
alkanes-cli alkanes init-pool \
    --pair "$DIESEL_ID,$FRBTC_ID" \
    --liquidity "$DIESEL_AMOUNT:$FRBTC_AMOUNT" \
    --to $ADDR \
    --from $ADDR \
    --change $ADDR \
    --trace

# Note: --factory defaults to $FACTORY (4:65522)

echo ""
echo "✅ Pool created successfully!"
echo ""
echo "🎉 Deployment complete!"
echo ""
echo "You can now query the pool:"
echo "  alkanes-cli alkanes dataapi get-pools"
echo "  (factory defaults to $FACTORY)"
