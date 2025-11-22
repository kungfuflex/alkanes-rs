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
        
        log_info "Waiting for sandshrew to index blocks (15 seconds)..."
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
    
    log_warn "Factory initialization COMMENTED OUT for troubleshooting"
    log_info "To initialize factory manually, run:"
    echo ""
    echo "  FACTORY_INIT_PROTOSTONE=\"[2:1:1:p1]:v0:v0,[4,$AMM_FACTORY_PROXY_TX,0,$POOL_BEACON_PROXY_TX,4,$POOL_UPGRADEABLE_BEACON_TX]:v0:v0\""
    echo ""
    echo "  $ALKANES_CLI -p regtest \\"
    echo "    --wallet-file $WALLET_FILE \\"
    echo "    --passphrase $DEPLOY_PASSWORD \\"
    echo "    alkanes execute \"\$FACTORY_INIT_PROTOSTONE\" \\"
    echo "    --from p2tr:0 \\"
    echo "    --inputs 2:1:1 \\"
    echo "    --fee-rate 1 \\"
    echo "    --mine \\"
    echo "    --trace \\"
    echo "    -y"
    echo ""
    
    # COMMENTED OUT: Factory initialization that depends on auth token [2:1]
    # 
    # # Step 7: Initialize Factory
    # # This is the tricky part that needs troubleshooting
    # # 
    # # The factory's InitFactory opcode (opcode 0) requires authentication via auth token [2, 1]
    # # 
    # # Pattern explanation:
    # #   - First protostone: [2:1:1:p1]:v0:v0
    # #     This sends 1 unit of auth token [2:1] to the NEXT protostone (p1 means "physical output 1")
    # #     Change goes back to v0 (virtual output 0, which becomes physical output 0)
    # #     
    # #   - Second protostone: [4,$AMM_FACTORY_PROXY_TX,0,$POOL_BEACON_PROXY_TX,4,$POOL_UPGRADEABLE_BEACON_TX]:v0:v0
    # #     This receives the auth token from p1, uses it to authenticate the call to factory
    # #     Opcode 0 = InitFactory(pool_beacon_proxy_id, pool_beacon_id)
    # #     After the call, outputs the auth token back to v0
    # #
    # # The --inputs flag tells the wallet to specifically use a UTXO containing auth token [2:1]
    # 
    # log_info "Initializing OYL Factory with InitFactory opcode..."
    # log_info "This requires spending auth token [2:1] to authenticate the call..."
    # 
    # FACTORY_INIT_PROTOSTONE="[2:1:1:p1]:v0:v0,[4,$AMM_FACTORY_PROXY_TX,0,$POOL_BEACON_PROXY_TX,4,$POOL_UPGRADEABLE_BEACON_TX]:v0:v0"
    # log_info "  Protostone: $FACTORY_INIT_PROTOSTONE"
    # log_info ""
    # log_info "  Explanation:"
    # log_info "    First protostone:  [2:1:1:p1]:v0:v0"
    # log_info "      - Send 1 unit of auth token [2:1] to physical output 1 (p1)"
    # log_info "      - Change goes to v0 (becomes physical output 0)"
    # log_info ""
    # log_info "    Second protostone: [4,$AMM_FACTORY_PROXY_TX,0,...]:v0:v0"
    # log_info "      - Receives auth token from p1 (physical output 1 from TX)"
    # log_info "      - Uses it to authenticate call to factory"
    # log_info "      - Returns auth token to v0"
    # echo ""
    # 
    # DEPLOY_PASSWORD="${DEPLOY_PASSWORD:-password}"
    # 
    # # The key here is:
    # # 1. --inputs 2:1:1 - tells wallet to use a UTXO with auth token [2:1]
    # # 2. First protostone sends auth token to p1 (next protostone)
    # # 3. Second protostone receives it and uses it for authentication
    # # 4. --trace flag helps debug what's happening
    # # 5. --mine ensures the transaction gets mined immediately
    # 
    # "$ALKANES_CLI" -p regtest \
    #     --wallet-file "$WALLET_FILE" \
    #     --passphrase "$DEPLOY_PASSWORD" \
    #     alkanes execute "$FACTORY_INIT_PROTOSTONE" \
    #     --from p2tr:0 \
    #     --inputs 2:1:1 \
    #     --fee-rate 1 \
    #     --mine \
    #     --trace \
    #     -y
    # 
    # if [ $? -eq 0 ]; then
    #     log_success "OYL Factory initialized successfully!"
    #     
    #     # Wait for metashrew to index
    #     log_info "Waiting for metashrew to index factory initialization (5 seconds)..."
    #     sleep 5
    #     
    #     echo ""
    #     log_success "🎉 OYL AMM deployment complete!"
    # else
    #     log_error "Failed to initialize OYL Factory"
    #     log_error ""
    #     log_error "Troubleshooting tips:"
    #     log_error "  1. Check that auth token [2:1] exists in wallet:"
    #     log_error "     $ALKANES_CLI -p regtest --wallet-file $WALLET_FILE --passphrase $DEPLOY_PASSWORD alkanes getbalance"
    #     log_error ""
    #     log_error "  2. Verify auth token factory created tokens at [2, 1]:"
    #     log_error "     $ALKANES_CLI -p regtest alkanes getbytecode 2:1"
    #     log_error ""
    #     log_error "  3. Check wallet UTXOs for auth tokens:"
    #     log_error "     $ALKANES_CLI -p regtest --wallet-file $WALLET_FILE --passphrase $DEPLOY_PASSWORD wallet utxos p2tr:0"
    #     log_error ""
    #     log_error "  4. The issue might be:"
    #     log_error "     - Auth token not being properly received by second protostone"
    #     log_error "     - UTXO selection not finding auth token"
    #     log_error "     - Auth token not being accepted by factory"
    #     log_error ""
    #     exit 1
    # fi
    
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
    
    log_info "Next steps:"
    echo ""
    echo "  1. Create a liquidity pool:"
    echo "     $ALKANES_CLI -p regtest --wallet-file $WALLET_FILE --passphrase $DEPLOY_PASSWORD \\"
    echo "       alkanes execute '[2:1:1:p1]:v0:v0,[4,$AMM_FACTORY_PROXY_TX,1,TOKEN0_BLOCK,TOKEN0_TX,TOKEN1_BLOCK,TOKEN1_TX,AMOUNT0,AMOUNT1]:v0:v0' \\"
    echo "       --from p2tr:0 --inputs 2:1:1 --fee-rate 1 --mine -y"
    echo ""
    echo "  2. Check alkanes balances:"
    echo "     $ALKANES_CLI -p regtest --wallet-file $WALLET_FILE --passphrase $DEPLOY_PASSWORD alkanes getbalance"
    echo ""
    
    log_success "Deployment script completed successfully!"
}

# Run main
main
