#!/bin/bash
# Script to query Rebar Shield API and understand request/response format
# This will help us implement --use-rebar support in alkanes-cli

set -e

echo "================================================"
echo "Rebar Shield API Query Script"
echo "================================================"
echo ""

REBAR_INFO_URL="https://shield.rebarlabs.io/v1/info"
REBAR_RPC_URL="https://shield.rebarlabs.io/v1/rpc"

# Step 1: Get current info (payment address, fees, block height)
echo "Step 1: Querying Rebar Shield info..."
echo "GET $REBAR_INFO_URL"
echo ""

INFO_RESPONSE=$(curl -s "$REBAR_INFO_URL")
echo "$INFO_RESPONSE" | python3 -m json.tool

echo ""
echo "================================================"
echo ""

# Extract key information
PAYMENT_ADDR=$(echo "$INFO_RESPONSE" | python3 -c "import sys, json; print(json.load(sys.stdin)['payment']['p2wpkh'])")
BLOCK_HEIGHT=$(echo "$INFO_RESPONSE" | python3 -c "import sys, json; print(json.load(sys.stdin)['height'])")
FEE_TIER_1=$(echo "$INFO_RESPONSE" | python3 -c "import sys, json; data=json.load(sys.stdin); print(f\"{data['fees'][0]['feerate']} sat/vB @ {data['fees'][0]['estimated_hashrate']*100:.0f}% hashrate\")")
FEE_TIER_2=$(echo "$INFO_RESPONSE" | python3 -c "import sys, json; data=json.load(sys.stdin); print(f\"{data['fees'][1]['feerate']} sat/vB @ {data['fees'][1]['estimated_hashrate']*100:.0f}% hashrate\")" 2>/dev/null || echo "N/A")

echo "Extracted Information:"
echo "  Payment Address: $PAYMENT_ADDR"
echo "  Block Height: $BLOCK_HEIGHT"
echo "  Fee Tier 1: $FEE_TIER_1"
echo "  Fee Tier 2: $FEE_TIER_2"
echo ""

# Step 2: Calculate payment for a sample transaction
echo "================================================"
echo "Step 2: Calculate Rebar payment for sample transaction"
echo "================================================"
echo ""

# Example: 9,158 input transaction @ 2.1 sat/vB
SAMPLE_INPUTS=9158
SAMPLE_TX_VSIZE=$((53 + SAMPLE_INPUTS * 107))
BASE_FEE_RATE=2.1

echo "Sample transaction:"
echo "  Inputs: $SAMPLE_INPUTS"
echo "  Estimated vSize: $SAMPLE_TX_VSIZE vbytes"
echo "  Base fee rate: $BASE_FEE_RATE sat/vB"
echo ""

# Extract fee rates from info
TIER_1_RATE=$(echo "$INFO_RESPONSE" | python3 -c "import sys, json; print(json.load(sys.stdin)['fees'][0]['feerate'])")
TIER_2_RATE=$(echo "$INFO_RESPONSE" | python3 -c "import sys, json; print(json.load(sys.stdin)['fees'][1]['feerate'])" 2>/dev/null || echo "0")

echo "Rebar fee tiers:"
echo "  Tier 1: $TIER_1_RATE sat/vB"
echo "  Tier 2: $TIER_2_RATE sat/vB"
echo ""

# Calculate payment amount (Rebar payment = vsize * feerate)
PAYMENT_TIER_1=$(python3 -c "print(int($SAMPLE_TX_VSIZE * $TIER_1_RATE))")
PAYMENT_TIER_2=$(python3 -c "print(int($SAMPLE_TX_VSIZE * $TIER_2_RATE))" 2>/dev/null || echo "0")

echo "Rebar payment required:"
echo "  Tier 1 ($TIER_1_RATE sat/vB): $PAYMENT_TIER_1 sats"
if [ "$PAYMENT_TIER_2" != "0" ]; then
    echo "  Tier 2 ($TIER_2_RATE sat/vB): $PAYMENT_TIER_2 sats"
fi
echo ""

echo "Transaction structure with Rebar payment:"
echo "  Output 0: Main destination (consolidation output)"
echo "  Output 1: Rebar payment address ($PAYMENT_ADDR)"
echo "            Amount: $PAYMENT_TIER_1 sats (using Tier 1)"
echo ""

# Step 3: Test sendrawtransaction RPC format (with dummy tx)
echo "================================================"
echo "Step 3: Test sendrawtransaction RPC format"
echo "================================================"
echo ""

# Use a dummy transaction hex for testing (will fail but shows format)
DUMMY_TX="0200000001..."

echo "POST $REBAR_RPC_URL"
echo ""
echo "Request body format:"
cat << EOF_JSON
{
  "jsonrpc": "2.0",
  "id": "1",
  "method": "sendrawtransaction",
  "params": ["<transaction_hex>"]
}
EOF_JSON

echo ""
echo "Example curl command:"
echo ""
cat << 'EOF_CURL'
curl -XPOST 'https://shield.rebarlabs.io/v1/rpc' \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "id": "1",
    "method": "sendrawtransaction",
    "params": ["YOUR_SIGNED_TRANSACTION_HEX"]
  }'
EOF_CURL

echo ""
echo ""

# Step 4: Summary for implementation
echo "================================================"
echo "Implementation Summary for --use-rebar"
echo "================================================"
echo ""

echo "Key Requirements:"
echo "  1. Query /v1/info to get payment address and fee rates"
echo "  2. Choose fee tier based on desired hashrate coverage"
echo "  3. Add payment output to transaction:"
echo "     Address: $PAYMENT_ADDR (from /v1/info)"
echo "     Amount: vsize × feerate (e.g., $PAYMENT_TIER_1 sats for tier 1)"
echo "  4. Build transaction with 2 outputs:"
echo "     - Output 0: Your consolidation destination"
echo "     - Output 1: Rebar payment"
echo "  5. Sign transaction (both outputs)"
echo "  6. Submit via JSON-RPC POST to $REBAR_RPC_URL"
echo ""

echo "Differences from standard broadcast:"
echo "  ✓ Requires extra output for Rebar payment"
echo "  ✓ Payment amount = transaction_vsize × fee_tier_rate"
echo "  ✓ Uses JSON-RPC (like Bitcoin Core), not REST"
echo "  ✓ Transaction goes to mining pools privately"
echo "  ✓ Higher fees than Slipstream (16-28 sat/vB vs 2 sat/vB)"
echo ""

echo "For our consolidation transaction:"
TOTAL_BASE_FEE=$(python3 -c "print(int($SAMPLE_TX_VSIZE * $BASE_FEE_RATE))")
TOTAL_WITH_REBAR=$(python3 -c "print($TOTAL_BASE_FEE + $PAYMENT_TIER_1)")

echo "  Base fee (2.1 sat/vB): $TOTAL_BASE_FEE sats"
echo "  Rebar payment (tier 1): $PAYMENT_TIER_1 sats"
echo "  Total fees: $TOTAL_WITH_REBAR sats"
echo "  Effective rate: $(python3 -c "print(f'{$TOTAL_WITH_REBAR / $SAMPLE_TX_VSIZE:.2f}')") sat/vB"
echo ""

echo "================================================"
echo "Query complete!"
echo ""
echo "Next: Implement --use-rebar in alkanes-cli with:"
echo "  1. Query /v1/info before building transaction"
echo "  2. Add second output for Rebar payment"
echo "  3. Adjust main output to account for Rebar fee"
echo "  4. Use JSON-RPC to submit"
echo "================================================"
