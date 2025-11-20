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
WALLET_FILE="${HOME}/.alkanes/regtest-wallet.json"
PASSPHRASE="${ALKANES_PASSPHRASE:-password}"
CLI="./target/release/alkanes-cli -p regtest --wallet-file $WALLET_FILE --passphrase $PASSPHRASE"

# Step 0: Fund the wallet
echo ""
echo "💰 Step 0: Funding wallet with Bitcoin..."
./target/release/alkanes-cli -p regtest --wallet-file ~/.alkanes/regtest-wallet.json bitcoind generatetoaddress 201 bcrt1pldrufx0nklknemsdcjaelst9m24xh0lat9jsrxh45w47detp7xyqw3a70w
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
echo "⏳ Waiting for confirmations..."
sleep 5

# Step 3: Create the pool
echo ""
echo "🏊 Step 3: Creating DIESEL/frBTC pool..."
$CLI alkanes init-pool \
    --pair "$DIESEL_ID,$FRBTC_ID" \
    --liquidity "$DIESEL_AMOUNT:$FRBTC_AMOUNT" \
    --to $ADDR \
    --from $ADDR \
    --change $ADDR \
    --auto-confirm \
    --trace

# Note: --factory defaults to $FACTORY (4:65522)

echo ""
echo "✅ Pool created successfully!"
echo ""
echo "🎉 Deployment complete!"
echo ""
echo "You can now query the pool:"
echo "  $CLI dataapi get-pools"
echo "  (factory defaults to $FACTORY)"
