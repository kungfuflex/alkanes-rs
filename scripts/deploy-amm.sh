#!/bin/bash

# Minimal OYL AMM Deployment Script
# This script deploys ONLY the OYL AMM and requisite template wasms

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

# OYL AMM Constants (matching oyl-sdk deployment pattern)
AUTH_TOKEN_FACTORY_ID=65517      # 0xffed
POOL_BEACON_PROXY_TX=780993      # 0xbeac1
AMM_FACTORY_LOGIC_IMPL_TX=65524  # 0xfff4
POOL_LOGIC_TX=65520              # 0xfff0
AMM_FACTORY_PROXY_TX=65522       # 0xfff2 (upgradeable proxy)
POOL_UPGRADEABLE_BEACON_TX=65523 # 0xfff3

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
        log_info "Please build alkanes-cli first:"
        log_info "  cd $SCRIPT_DIR/.. && cargo build --release"
        exit 1
    fi
}

# Check if regtest node is running
check_regtest() {
    log_info "Checking if regtest node is running..."
    if ! curl -s "$RPC_URL" > /dev/null 2>&1; then
        log_error "Cannot connect to regtest node at $RPC_URL"
        log_info "Please start the regtest node first"
        exit 1
    fi
    log_success "Regtest node is running at $RPC_URL"
}

# Check if WASMs exist
check_wasms() {
    log_info "Checking if WASM files exist in prod_wasms..."
    if [ ! -d "$WASM_DIR" ] || [ -z "$(ls -A $WASM_DIR/*.wasm 2>/dev/null)" ]; then
        log_error "WASM files not found in $WASM_DIR"
        exit 1
    fi
    
    local count=$(find "$WASM_DIR" -name "*.wasm" -type f -size +1k | wc -l)
    log_success "Found $count WASM files in $WASM_DIR"
}

# Setup wallet if it doesn't exist
setup_wallet() {
    if [ ! -f "$WALLET_FILE" ]; then
        log_info "Creating new wallet..."
        mkdir -p "$(dirname "$WALLET_FILE")"
        
        DEPLOY_PASSWORD="${DEPLOY_PASSWORD:-password}"
        
        "$ALKANES_CLI" -p regtest --wallet-file "$WALLET_FILE" --passphrase "$DEPLOY_PASSWORD" wallet create
        log_success "Wallet created at $WALLET_FILE"
    else
        log_success "Using existing wallet at $WALLET_FILE"
    fi
    
    DEPLOY_PASSWORD="${DEPLOY_PASSWORD:-password}"
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
        
        log_info "Waiting for indexer to sync blocks (15 seconds)..."
        sleep 15
        
        log_success "Wallet funded! Ready for deployments"
    fi
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
    DEPLOY_PASSWORD="${DEPLOY_PASSWORD:-password}"
    "$ALKANES_CLI" -p regtest \
        --wallet-file "$WALLET_FILE" \
        --passphrase "$DEPLOY_PASSWORD" \
        alkanes execute "$PROTOSTONE" \
        --envelope "$WASM_FILE" \
        --from p2tr:0 \
        --fee-rate 1 \
        --mine \
        --trace \
        -y
    
    if [ $? -eq 0 ]; then
        log_success "$CONTRACT_NAME deployed to [4, $TARGET_TX]"
        
        # Wait for metashrew to index the deployment
        log_info "Waiting for metashrew to index (5 seconds)..."
        sleep 5
        
        # Verify deployment by checking bytecode
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
    log_info "Minimal OYL AMM Deployment"
    log_info "=========================================="
    echo ""
    
    # Pre-deployment checks
    check_cli
    check_regtest
    check_wasms
    setup_wallet
    fund_wallet
    
    echo ""
    log_info "=========================================="
    log_info "Starting OYL AMM Contract Deployments"
    log_info "=========================================="
    echo ""
    
    log_info "OYL AMM System - Required Components Only"
    echo ""
    
    # Step 1: Deploy Auth Token Factory
    # This creates authentication tokens at [2, 1] that are needed to call the factory
    deploy_contract "OYL Auth Token Factory" "$WASM_DIR/alkanes_std_auth_token.wasm" "$AUTH_TOKEN_FACTORY_ID" "100"
    
    # Step 2: Deploy Beacon Proxy Template
    # This is the template used for all pool instances
    deploy_contract "OYL Beacon Proxy" "$WASM_DIR/alkanes_std_beacon_proxy.wasm" "$POOL_BEACON_PROXY_TX" "36863"
    
    # Step 3: Deploy Factory Logic Implementation
    # This is the actual factory logic that will be proxied
    deploy_contract "OYL Factory Logic" "$WASM_DIR/factory.wasm" "$AMM_FACTORY_LOGIC_IMPL_TX" "50"
    
    # Step 4: Deploy Pool Logic Implementation
    # This is the actual pool logic that will be used by all pools via the beacon
    deploy_contract "OYL Pool Logic" "$WASM_DIR/pool.wasm" "$POOL_LOGIC_TX" "50"
    
    # Step 5: Deploy Upgradeable Proxy (Factory Proxy)
    # This wraps the factory logic and makes it upgradeable
    # Init args: 32767 (0x7fff), block 4, tx 65524 (factory logic), opcode 5 (setImplementation)
    deploy_contract "OYL Factory Proxy (Upgradeable)" "$WASM_DIR/alkanes_std_upgradeable.wasm" "$AMM_FACTORY_PROXY_TX" "$((0x7fff)),4,$AMM_FACTORY_LOGIC_IMPL_TX,5"
    
    # Step 6: Deploy Upgradeable Beacon
    # This allows all pools to be upgraded by changing what the beacon points to
    # Init args: 32767 (0x7fff), block 4, tx 65520 (pool logic), opcode 5 (setImplementation)
    deploy_contract "OYL Upgradeable Beacon" "$WASM_DIR/alkanes_std_upgradeable_beacon.wasm" "$POOL_UPGRADEABLE_BEACON_TX" "$((0x7fff)),4,$POOL_LOGIC_TX,5"
    
    echo ""
    log_info "=========================================="
    log_info "Checking Auth Token Information"
    log_info "=========================================="
    echo ""
    
    # Check what alkane ID was created for auth tokens
    log_info "Checking auth token factory bytecode at [4, $AUTH_TOKEN_FACTORY_ID]..."
    "$ALKANES_CLI" -p regtest alkanes getbytecode "4:$AUTH_TOKEN_FACTORY_ID" | head -c 100
    echo ""
    echo ""
    
    log_info "Checking for auth token alkane at [2, 1]..."
    "$ALKANES_CLI" -p regtest alkanes getbytecode "2:1" | head -c 100
    echo ""
    echo ""
    
    log_info "Checking wallet balances..."
    "$ALKANES_CLI" -p regtest --wallet-file "$WALLET_FILE" --passphrase "$DEPLOY_PASSWORD" wallet utxos p2tr:0 | head -50
    echo ""
    
    # Step 7: Initialize Factory
    log_info "Initializing OYL Factory with InitFactory opcode..."
    log_info "This requires spending auth token [2:1] to authenticate the call..."
    
    FACTORY_INIT_PROTOSTONE="[4,$AMM_FACTORY_PROXY_TX,0,$POOL_BEACON_PROXY_TX,4,$POOL_UPGRADEABLE_BEACON_TX]:v0:v0"
    log_info "  Protostone: $FACTORY_INIT_PROTOSTONE"
    log_info "  Opcode 0 = InitFactory(pool_beacon_proxy_id, pool_beacon_id)"
    echo ""
    
    DEPLOY_PASSWORD="${DEPLOY_PASSWORD:-password}"
    
    "$ALKANES_CLI" -p regtest \
        --wallet-file "$WALLET_FILE" \
        --passphrase "$DEPLOY_PASSWORD" \
        alkanes execute "$FACTORY_INIT_PROTOSTONE" \
        --from p2tr:0 \
        --inputs 2:1:1 \
        --fee-rate 1 \
        --mine \
        --trace \
        -y
    
    if [ $? -eq 0 ]; then
        log_success "OYL Factory initialized successfully!"
        
        # Wait for metashrew to index
        log_info "Waiting for metashrew to index factory initialization (5 seconds)..."
        sleep 5
    else
        log_error "Failed to initialize OYL Factory"
        exit 1
    fi
    echo ""
    
    # Step 8: Create test tokens and pool
    log_info "=========================================="
    log_info "Creating Test Pool (DIESEL/frBTC)"
    log_info "=========================================="
    echo ""
    
    # Configuration for test pool
    DIESEL_ID="2:0"
    FRBTC_ID="32:0"
    DIESEL_AMOUNT="300000000"  # 300M DIESEL
    FRBTC_AMOUNT="50000"       # 0.0005 BTC in sats
    
    # Step 8a: Mine DIESEL
    log_info "Mining DIESEL tokens..."
    "$ALKANES_CLI" -p regtest \
        --wallet-file "$WALLET_FILE" \
        --passphrase "$DEPLOY_PASSWORD" \
        alkanes execute "[2,0,77]:v0:v0" \
        --to p2tr:0 \
        --from p2tr:0 \
        --change p2tr:0 \
        --auto-confirm
    
    if [ $? -eq 0 ]; then
        log_success "DIESEL mined"
    else
        log_error "Failed to mine DIESEL"
        exit 1
    fi
    
    # Step 8b: Wrap BTC for frBTC
    log_info "Wrapping BTC to frBTC..."
    "$ALKANES_CLI" -p regtest \
        --wallet-file "$WALLET_FILE" \
        --passphrase "$DEPLOY_PASSWORD" \
        alkanes wrap-btc \
        100000000 \
        --to p2tr:0 \
        --from p2tr:0 \
        --change p2tr:0 \
        --auto-confirm
    
    if [ $? -eq 0 ]; then
        log_success "frBTC wrapped"
    else
        log_error "Failed to wrap frBTC"
        exit 1
    fi
    
    # Wait for confirmations
    log_info "Mining a block to confirm transactions..."
    "$ALKANES_CLI" -p regtest \
        --wallet-file "$WALLET_FILE" \
        --passphrase "$DEPLOY_PASSWORD" \
        bitcoind generatetoaddress 1 p2tr:0 > /dev/null 2>&1
    
    log_info "Waiting for metashrew to index transactions (15 seconds)..."
    sleep 15
    
    # Step 8c: Create the pool
    log_info "Creating DIESEL/frBTC pool..."
    "$ALKANES_CLI" -p regtest \
        --wallet-file "$WALLET_FILE" \
        --passphrase "$DEPLOY_PASSWORD" \
        alkanes init-pool \
        --pair "$DIESEL_ID,$FRBTC_ID" \
        --liquidity "$DIESEL_AMOUNT:$FRBTC_AMOUNT" \
        --to p2tr:0 \
        --from p2tr:0 \
        --change p2tr:0 \
        --factory "4:$AMM_FACTORY_PROXY_TX" \
        --auto-confirm \
        --trace
    
    if [ $? -eq 0 ]; then
        log_success "Pool created successfully!"
        
        # Wait for metashrew to index
        log_info "Waiting for metashrew to index pool creation (5 seconds)..."
        sleep 5
        
        echo ""
        log_success "🎉 OYL AMM deployment and pool creation complete!"
    else
        log_error "Failed to create pool"
        exit 1
    fi
    
    echo ""
    log_info "=========================================="
    log_info "Deployment Summary"
    log_info "=========================================="
    echo ""
    
    log_info "Deployed Contracts:"
    echo ""
    echo "  OYL Auth Token Factory:   [4, $AUTH_TOKEN_FACTORY_ID] (0x$(printf '%x' $AUTH_TOKEN_FACTORY_ID))"
    echo "  OYL Beacon Proxy:         [4, $POOL_BEACON_PROXY_TX] (0x$(printf '%x' $POOL_BEACON_PROXY_TX))"
    echo "  OYL Factory Logic:        [4, $AMM_FACTORY_LOGIC_IMPL_TX] (0x$(printf '%x' $AMM_FACTORY_LOGIC_IMPL_TX))"
    echo "  OYL Pool Logic:           [4, $POOL_LOGIC_TX] (0x$(printf '%x' $POOL_LOGIC_TX))"
    echo "  OYL Factory Proxy:        [4, $AMM_FACTORY_PROXY_TX] (0x$(printf '%x' $AMM_FACTORY_PROXY_TX)) ← Main entry point"
    echo "  OYL Upgradeable Beacon:   [4, $POOL_UPGRADEABLE_BEACON_TX] (0x$(printf '%x' $POOL_UPGRADEABLE_BEACON_TX))"
    echo ""
    log_info "Auth Tokens:"
    echo "  Factory Auth Token:       [2, 1] (created by auth token factory)"
    echo ""
    log_info "Test Pool:"
    echo "  DIESEL/frBTC Pool:        Created with 300M DIESEL / 50K frBTC"
    echo ""
    
    log_info "Next steps:"
    echo ""
    echo "  1. Check pools:"
    echo "     $ALKANES_CLI -p regtest dataapi get-pools"
    echo ""
    echo "  2. Check alkanes balances:"
    echo "     $ALKANES_CLI -p regtest --wallet-file $WALLET_FILE --passphrase $DEPLOY_PASSWORD alkanes getbalance"
    echo ""
    echo "  3. Create another pool:"
    echo "     $ALKANES_CLI -p regtest --wallet-file $WALLET_FILE --passphrase $DEPLOY_PASSWORD \\"
    echo "       alkanes init-pool \\"
    echo "       --pair TOKEN0_BLOCK:TOKEN0_TX,TOKEN1_BLOCK:TOKEN1_TX \\"
    echo "       --liquidity AMOUNT0:AMOUNT1 \\"
    echo "       --to p2tr:0 --from p2tr:0 --change p2tr:0 \\"
    echo "       --factory 4:$AMM_FACTORY_PROXY_TX \\"
    echo "       --auto-confirm"
    echo ""
    
    log_success "Deployment script completed successfully!"
}

# Run main
main
