#!/bin/bash

# Simple script to perform a swap on regtest
# Mints DIESEL with cellpack [2,0,77] then swaps it for frBTC along path 2:0,32:0

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Configuration
WALLET_FILE="${HOME}/.alkanes/wallet.json"
DEPLOY_PASSWORD="${DEPLOY_PASSWORD:-testtesttest}"
ALKANES_CLI="${PROJECT_ROOT}/target/release/alkanes-cli"

# Check if alkanes-cli exists
if [ ! -f "$ALKANES_CLI" ]; then
    log_error "alkanes-cli not found at $ALKANES_CLI"
    log_info "Please build it first with: cargo build --release -p alkanes-cli"
    exit 1
fi

# Check if wallet exists
if [ ! -f "$WALLET_FILE" ]; then
    log_error "Wallet file not found at $WALLET_FILE"
    log_info "Please create a wallet first or set WALLET_FILE environment variable"
    exit 1
fi

log_info "=========================================="
log_info "Regtest Swap Test"
log_info "=========================================="
echo ""

# Step 1: Mine DIESEL tokens using cellpack [2,0,77]
log_info "Step 1: Mining DIESEL tokens with cellpack [2,0,77]..."
"$ALKANES_CLI" -p regtest \
    --wallet-file "$WALLET_FILE" \
    --passphrase "$DEPLOY_PASSWORD" \
    alkanes execute "[2,0,77]:v0:v0" \
    --to p2tr:0 \
    --from p2tr:0 \
    --change p2tr:0 \
    --auto-confirm

if [ $? -eq 0 ]; then
    log_success "DIESEL mined successfully"
else
    log_error "Failed to mine DIESEL"
    exit 1
fi

echo ""

# Step 2: Mine a block to confirm
log_info "Step 2: Mining a block to confirm the transaction..."
"$ALKANES_CLI" -p regtest \
    --wallet-file "$WALLET_FILE" \
    --passphrase "$DEPLOY_PASSWORD" \
    bitcoind generatetoaddress 1 p2tr:0 > /dev/null 2>&1

log_info "Waiting for metashrew to index (10 seconds)..."
sleep 10

echo ""

# Step 3: Check balance before swap
log_info "Step 3: Checking DIESEL balance..."
"$ALKANES_CLI" -p regtest \
    --wallet-file "$WALLET_FILE" \
    --passphrase "$DEPLOY_PASSWORD" \
    alkanes getbalance 2>&1 | grep -E "DIESEL|2:0" || true

echo ""

# Step 4: Perform the swap
log_info "Step 4: Swapping DIESEL for frBTC along path 2:0,32:0..."
log_info "  Swapping 1000000 DIESEL (minimum) for frBTC"
"$ALKANES_CLI" -p regtest \
    --wallet-file "$WALLET_FILE" \
    --passphrase "$DEPLOY_PASSWORD" \
    alkanes swap \
    --path "2:0,32:0" \
    --input 1000000 \
    --minimum-output 1 \
    --to p2tr:0 \
    --from p2tr:0 \
    --change p2tr:0 \
    --mine \
    --trace \
    --auto-confirm

if [ $? -eq 0 ]; then
    log_success "Swap executed successfully!"
else
    log_error "Failed to execute swap"
    exit 1
fi

echo ""

# Step 5: Mine a block to confirm the swap
log_info "Step 5: Mining a block to confirm the swap..."
"$ALKANES_CLI" -p regtest \
    --wallet-file "$WALLET_FILE" \
    --passphrase "$DEPLOY_PASSWORD" \
    bitcoind generatetoaddress 1 p2tr:0 > /dev/null 2>&1

log_info "Waiting for metashrew to index the swap (10 seconds)..."
sleep 10

echo ""

# Step 6: Check balance after swap
log_info "Step 6: Checking balances after swap..."
"$ALKANES_CLI" -p regtest \
    --wallet-file "$WALLET_FILE" \
    --passphrase "$DEPLOY_PASSWORD" \
    alkanes getbalance

echo ""
log_success "🎉 Swap test complete!"
log_info "Check the indexer logs to see trace transform processing"
