#!/bin/bash

# AMM-Only Deployment Script for Regtest
# This script deploys ONLY the OYL AMM system for debugging

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WASM_DIR="$SCRIPT_DIR/../prod_wasms"
WALLET_FILE="${WALLET_FILE:-$HOME/.alkanes/wallet.json}"
DEPLOY_PASSWORD="${DEPLOY_PASSWORD:-testtesttest}"
RPC_URL="http://localhost:18888"

# OYL AMM Constants (matching oyl-protocol deployment)
AMM_FACTORY_ID=65522          # 0xfff2
AUTH_TOKEN_FACTORY_ID=65517   # 0xffed
AMM_FACTORY_PROXY_TX=1
AMM_FACTORY_LOGIC_IMPL_TX=62463  # 0xf3ff
POOL_BEACON_PROXY_TX=781633      # 0xbeac1
POOL_UPGRADEABLE_BEACON_TX=781632 # 0xbeac0

# Helper functions
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

# Check if alkanes-cli exists
check_cli() {
    if [ -f "$SCRIPT_DIR/../target/release/alkanes-cli" ]; then
        ALKANES_CLI="$SCRIPT_DIR/../target/release/alkanes-cli"
        log_success "Found alkanes-cli: $ALKANES_CLI"
    elif command -v alkanes-cli &> /dev/null; then
        ALKANES_CLI="alkanes-cli"
        log_success "Found alkanes-cli in PATH: $(which alkanes-cli)"
    else
        log_error "alkanes-cli not found"
        exit 1
    fi
}

# Check if regtest node is running
check_regtest() {
    log_info "Checking if regtest node is running..."
    if ! curl -s "$RPC_URL" > /dev/null 2>&1; then
        log_error "Cannot connect to regtest node at $RPC_URL"
        exit 1
    fi
    log_success "Regtest node is running at $RPC_URL"
}

# Setup wallet if it doesn't exist
setup_wallet() {
    if [ ! -f "$WALLET_FILE" ]; then
        log_info "Creating new wallet..."
        mkdir -p "$(dirname "$WALLET_FILE")"
        
        "$ALKANES_CLI" -p regtest --wallet-file "$WALLET_FILE" --passphrase "$DEPLOY_PASSWORD" wallet create
        log_success "Wallet created at $WALLET_FILE"
    else
        log_success "Using existing wallet at $WALLET_FILE"
    fi
    
    WALLET_ADDRESS=$("$ALKANES_CLI" -p regtest --wallet-file "$WALLET_FILE" --passphrase "$DEPLOY_PASSWORD" wallet addresses p2tr:0-1 2>/dev/null | grep -oP 'bcrt1[a-zA-Z0-9]+' | head -1)
    log_info "Wallet address: $WALLET_ADDRESS"
}

# Fund wallet with regtest coins
fund_wallet() {
    log_info "Checking if wallet needs funding..."
    
    UTXO_CHECK=$("$ALKANES_CLI" -p regtest --wallet-file "$WALLET_FILE" --passphrase "$DEPLOY_PASSWORD" wallet utxos p2tr:0 2>&1 | grep -c "Outpoint:" || echo "0")
    
    if [ "$UTXO_CHECK" -gt "0" ]; then
        log_success "Wallet already funded with $UTXO_CHECK UTXOs at p2tr:0"
    else
        log_info "No UTXOs found, mining blocks to fund wallet..."
        log_info "Mining 400 blocks to $WALLET_ADDRESS (p2tr:0)..."
        "$ALKANES_CLI" -p regtest --wallet-file "$WALLET_FILE" --passphrase "$DEPLOY_PASSWORD" bitcoind generatetoaddress 400 "p2tr:0" > /dev/null 2>&1
        
        log_info "Waiting for sandshrew to index blocks (15 seconds)..."
        sleep 15
        
        log_success "Wallet funded! Ready for deployments"
    fi
}

# Ensure coinbase maturity by mining additional blocks
ensure_coinbase_maturity() {
    log_info "Ensuring coinbase maturity (mining 101 blocks to mature recent coinbases)..."
    "$ALKANES_CLI" -p regtest --wallet-file "$WALLET_FILE" --passphrase "$DEPLOY_PASSWORD" bitcoind generatetoaddress 101 "p2tr:0" > /dev/null 2>&1
    
    log_info "Waiting for sandshrew to index maturity blocks (10 seconds)..."
    sleep 10
    
    log_success "Coinbase outputs matured"
}

# Deploy a WASM contract using [3, tx] cellpack
deploy_contract() {
    local CONTRACT_NAME=$1
    local WASM_FILE=$2
    local TARGET_TX=$3
    shift 3
    local INIT_ARGS="$@"
    
    log_info "Deploying $CONTRACT_NAME using [3, $TARGET_TX] -> will create at [4, $TARGET_TX]..."
    
    if [ ! -f "$WASM_FILE" ]; then
        log_error "WASM file not found: $WASM_FILE"
        return 1
    fi
    
    # Build protostone: [3,tx,init_args...]:v0:v0 for deployment
    local PROTOSTONE="[3,$TARGET_TX$([ -n "$INIT_ARGS" ] && echo ",$INIT_ARGS" || echo "")]:v0:v0"
    
    log_info "  Protostone: $PROTOSTONE"
    
    # Deploy using alkanes-cli with envelope and protostone
    "$ALKANES_CLI" -p regtest \
        --wallet-file "$WALLET_FILE" \
        --passphrase "$DEPLOY_PASSWORD" \
        alkanes execute "$PROTOSTONE" \
        --envelope "$WASM_FILE" \
        --from p2tr:0 \
        --fee-rate 1 \
        --mine \
        -y
    
    if [ $? -eq 0 ]; then
        log_success "$CONTRACT_NAME deployed to [4, $TARGET_TX]"
        
        log_info "Waiting for metashrew to index (5 seconds)..."
        sleep 5
        
        log_info "Verifying $CONTRACT_NAME deployment at [4, $TARGET_TX]..."
        
        BYTECODE=""
        for i in 1 2 3; do
            BYTECODE=$("$ALKANES_CLI" -p regtest alkanes getbytecode "4:$TARGET_TX" 2>/dev/null)
            if [ -n "$BYTECODE" ] && [ "$BYTECODE" != "null" ] && [ "$BYTECODE" != '""' ]; then
                break
            fi
            if [ $i -lt 3 ]; then
                log_info "Bytecode not found yet, waiting 2 seconds..."
                sleep 2
            fi
        done
        
        if [ -n "$BYTECODE" ] && [ "$BYTECODE" != "null" ] && [ "$BYTECODE" != '""' ]; then
            BYTECODE_SIZE=$(echo "$BYTECODE" | wc -c)
            log_success "✓ Bytecode verified at [4, $TARGET_TX] (${BYTECODE_SIZE} bytes)"
        else
            log_error "✗ Bytecode verification failed for $CONTRACT_NAME at [4, $TARGET_TX]"
            log_error "Deployment failed - contract bytecode not found in metashrew"
            exit 1
        fi
    else
        log_error "Failed to deploy $CONTRACT_NAME"
        return 1
    fi
}

# Main deployment process
main() {
    echo ""
    log_info "=========================================="
    log_info "AMM-Only Deployment for Debugging"
    log_info "=========================================="
    echo ""
    
    # Pre-deployment checks
    check_cli
    check_regtest
    setup_wallet
    fund_wallet
    
    # Ensure we have mature coinbase outputs
    ensure_coinbase_maturity
    
    echo ""
    log_info "=========================================="
    log_info "OYL AMM System Deployment"
    log_info "=========================================="
    echo ""
    
    # Deploy pool logic implementation (for cloning)
    deploy_contract "OYL Pool Logic" "$WASM_DIR/pool.wasm" "$AMM_FACTORY_ID" "50"
    
    # Deploy auth token factory
    deploy_contract "OYL Auth Token Factory" "$WASM_DIR/alkanes_std_auth_token.wasm" "$AUTH_TOKEN_FACTORY_ID" "100"
    
    # Deploy AMM factory logic implementation
    deploy_contract "OYL Factory Logic" "$WASM_DIR/factory.wasm" "$AMM_FACTORY_LOGIC_IMPL_TX" "50"
    
    # Deploy beacon proxy for pools
    deploy_contract "OYL Beacon Proxy" "$WASM_DIR/alkanes_std_beacon_proxy.wasm" "$POOL_BEACON_PROXY_TX" "$((0x8fff))"
    
    # Deploy upgradeable beacon (points to pool logic)
    deploy_contract "OYL Upgradeable Beacon" "$WASM_DIR/alkanes_std_upgradeable_beacon.wasm" "$POOL_UPGRADEABLE_BEACON_TX" "$((0x7fff)),4,$AMM_FACTORY_ID,1"
    
    # Deploy factory proxy
    deploy_contract "OYL Factory Proxy" "$WASM_DIR/alkanes_std_upgradeable.wasm" "$AMM_FACTORY_PROXY_TX" "$((0x7fff)),4,$AMM_FACTORY_LOGIC_IMPL_TX,1"
    
    # Initialize factory proxy
    # Pattern: [3, proxy_tx, opcode=0, beacon_proxy_tx, beacon_block, beacon_tx]
    log_info "Initializing OYL Factory Proxy..."
    INIT_PROTOSTONE="[3,$AMM_FACTORY_PROXY_TX,0,$POOL_BEACON_PROXY_TX,4,$POOL_UPGRADEABLE_BEACON_TX]:v0:v0"
    log_info "  Protostone: $INIT_PROTOSTONE"
    
    "$ALKANES_CLI" -p regtest \
        --wallet-file "$WALLET_FILE" \
        --passphrase "$DEPLOY_PASSWORD" \
        alkanes execute "$INIT_PROTOSTONE" \
        --inputs B:10000 \
        --to p2tr:1 \
        --change p2tr:2 \
        --from p2tr:0 \
        --fee-rate 1 \
        --mine \
        --trace \
        -y
    
    if [ $? -eq 0 ]; then
        log_success "OYL Factory initialized"
        
        log_info "Waiting for metashrew to index factory initialization (5 seconds)..."
        sleep 5
    else
        log_error "Failed to initialize OYL Factory"
    fi
    
    echo ""
    log_info "=========================================="
    log_info "Deployment Summary"
    log_info "=========================================="
    echo ""
    
    log_success "AMM deployment complete!"
    echo ""
    log_info "Deployed Contracts:"
    echo "  - OYL Pool Logic:         [4, $AMM_FACTORY_ID]"
    echo "  - OYL Auth Token Factory: [4, $AUTH_TOKEN_FACTORY_ID]"
    echo "  - OYL Factory Logic:      [4, $AMM_FACTORY_LOGIC_IMPL_TX]"
    echo "  - OYL Beacon Proxy:       [4, $POOL_BEACON_PROXY_TX]"
    echo "  - OYL Upgradeable Beacon: [4, $POOL_UPGRADEABLE_BEACON_TX]"
    echo "  - OYL Factory Proxy:      [4, $AMM_FACTORY_PROXY_TX]"
    echo ""
}

# Run main
main
