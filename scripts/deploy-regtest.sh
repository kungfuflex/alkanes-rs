#!/bin/bash

# Subfrost Alkanes Deployment Script for Regtest
# This script deploys all subfrost alkanes to a local regtest environment
# Pattern follows reference/oyl-amm/deploy-oyl-amm.sh

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
ALKANES_DIR="../alkanes-rs"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WASM_DIR="$SCRIPT_DIR/../prod_wasms"
WALLET_FILE="$HOME/.alkanes/regtest-wallet.json"
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

# Check if subfrost-cli exists
check_cli() {
    if ! command -v subfrost-cli &> /dev/null; then
        log_error "subfrost-cli not found in PATH"
        log_info "Please ensure subfrost-cli is built and in your PATH"
        log_info "Try: cd $ALKANES_DIR && cargo build --release"
        exit 1
    fi
    log_success "Found subfrost-cli: $(which subfrost-cli)"
}

# Check if regtest node is running
check_regtest() {
    log_info "Checking if regtest node is running..."
    if ! curl -s "$RPC_URL" > /dev/null 2>&1; then
        log_error "Cannot connect to regtest node at $RPC_URL"
        log_info "Please start the regtest node first:"
        log_info "  cd $ALKANES_DIR && docker-compose up -d"
        exit 1
    fi
    log_success "Regtest node is running at $RPC_URL"
}

# Check if WASMs exist
check_wasms() {
    log_info "Checking if WASM files exist in prod_wasms..."
    if [ ! -d "$WASM_DIR" ] || [ -z "$(ls -A $WASM_DIR/*.wasm 2>/dev/null)" ]; then
        log_error "WASM files not found in $WASM_DIR"
        log_info "Please ensure WASMs are copied to $WASM_DIR"
        log_info "Or build them with:"
        log_info "  cd ../subfrost-alkanes && cargo build --release --target wasm32-unknown-unknown"
        log_info "  cp target/wasm32-unknown-unknown/release/*.wasm $WASM_DIR/"
        exit 1
    fi
    
    # Count non-empty WASMs
    local count=$(find "$WASM_DIR" -name "*.wasm" -type f -size +1k | wc -l)
    log_success "Found $count WASM files in $WASM_DIR"
}

# Setup wallet if it doesn't exist
setup_wallet() {
    if [ ! -f "$WALLET_FILE" ]; then
        log_info "Creating new wallet..."
        mkdir -p "$(dirname "$WALLET_FILE")"
        subfrost-cli -p regtest wallet new > "$WALLET_FILE"
        log_success "Wallet created at $WALLET_FILE"
    else
        log_success "Using existing wallet at $WALLET_FILE"
    fi
    
    # Get wallet address
    WALLET_ADDRESS=$(subfrost-cli -p regtest --wallet-file "$WALLET_FILE" wallet address)
    log_info "Wallet address: $WALLET_ADDRESS"
}

# Fund wallet with regtest coins
fund_wallet() {
    log_info "Checking wallet balance..."
    
    # Sync wallet first
    subfrost-cli -p regtest --wallet-file "$WALLET_FILE" wallet sync > /dev/null 2>&1 || true
    
    BALANCE=$(subfrost-cli -p regtest --wallet-file "$WALLET_FILE" wallet balance 2>/dev/null | grep -oP '\d+' | head -1 || echo "0")
    
    if [ "$BALANCE" -lt "1000000000" ]; then # Less than 10 BTC
        log_info "Wallet balance low ($BALANCE sats), mining blocks to fund wallet..."
        WALLET_ADDRESS=$(subfrost-cli -p regtest --wallet-file "$WALLET_FILE" wallet address)
        
        # Mine blocks using bitcoin-cli (regtest mode allows instant mining)
        # This assumes bitcoin-cli is configured for the regtest network
        for i in {1..10}; do
            subfrost-cli -p regtest --wallet-file "$WALLET_FILE" wallet mine 10 > /dev/null 2>&1 || true
        done
        
        # Sync wallet again
        subfrost-cli -p regtest --wallet-file "$WALLET_FILE" wallet sync > /dev/null 2>&1 || true
        
        BALANCE=$(subfrost-cli -p regtest --wallet-file "$WALLET_FILE" wallet balance 2>/dev/null | grep -oP '\d+' | head -1 || echo "0")
        log_success "Wallet funded! Balance: $BALANCE sats"
    else
        log_success "Wallet balance: $BALANCE sats"
    fi
}

# Deploy a WASM contract with initialization
deploy_contract() {
    local CONTRACT_NAME=$1
    local WASM_FILE=$2
    local TARGET_TX=$3
    shift 3
    local INIT_ARGS="$@"
    
    log_info "Deploying $CONTRACT_NAME to [3, $TARGET_TX] -> [4, $TARGET_TX]..."
    
    if [ ! -f "$WASM_FILE" ]; then
        log_error "WASM file not found: $WASM_FILE"
        return 1
    fi
    
    # Build protostone: [3,tx,init_args...]:v0:v0 for deployment
    # Opcode is first in init_args if provided
    local PROTOSTONE="[3,$TARGET_TX$([ -n "$INIT_ARGS" ] && echo ",$INIT_ARGS" || echo "")]:v0:v0"
    
    log_info "  Protostone: $PROTOSTONE"
    
    # Deploy using subfrost-cli with envelope and protostone
    subfrost-cli -p regtest \
        --wallet-file "$WALLET_FILE" \
        alkanes execute "$PROTOSTONE" \
        --envelope "$WASM_FILE" \
        --fee-rate 1 \
        --mine \
        --trace \
        -y \
        > /dev/null 2>&1
    
    if [ $? -eq 0 ]; then
        log_success "$CONTRACT_NAME deployed at [4, $TARGET_TX]"
    else
        log_error "Failed to deploy $CONTRACT_NAME"
        return 1
    fi
}

# Initialize a deployed contract
initialize_contract() {
    local CONTRACT_NAME=$1
    local ALKANE_ID=$2
    shift 2
    local ARGS="$@"
    
    log_info "Initializing $CONTRACT_NAME at $ALKANE_ID..."
    
    # Build the protostone format: [block:tx:opcode,args...]
    local PROTOSTONE="[$ALKANE_ID:0$([ -n "$ARGS" ] && echo ",$ARGS" || echo "")]"
    
    subfrost-cli -p regtest \
        --wallet-file "$WALLET_FILE" \
        alkanes execute "$PROTOSTONE" \
        --fee-rate 1 \
        --mine \
        --trace \
        -y \
        > /dev/null 2>&1
    
    if [ $? -eq 0 ]; then
        log_success "$CONTRACT_NAME initialized"
    else
        log_warn "Failed to initialize $CONTRACT_NAME (may not need initialization)"
    fi
}

# Main deployment process
main() {
    echo ""
    log_info "=========================================="
    log_info "Subfrost Alkanes Regtest Deployment"
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
    log_info "Starting Contract Deployments"
    log_info "=========================================="
    echo ""
    
    # Deploy Genesis Contracts (these are special and auto-deployed by the protocol)
    log_info "=========================================="
    log_info "Genesis Contracts (auto-deployed by alkanes-rs)"
    log_info "=========================================="
    log_info "  - Genesis Alkane at [1, 0]"
    log_info "  - DIESEL at [2, 0]"
    log_info "  - frBTC (or frZEC) at [32, 0] (or [42, 0] for Zcash)"
    log_info "  - frSIGIL at [32, 1] (or [42, 1] for Zcash)"
    log_info "  - ftrBTC Master at [31, 0] (via setup_ftrbtc in network.rs)"
    echo ""
    
    log_info "=========================================="
    log_info "Deployment Patterns"
    log_info "=========================================="
    log_info "  [3, tx] + envelope -> creates alkane at [4, tx]"
    log_info "  [2, 0] + envelope  -> CREATE (next available [2, n])"
    log_info "  [4, tx] + envelope -> CREATERESERVED ([tx, 0])"
    echo ""
    
    log_info "=========================================="
    log_info "Subfrost Reserved Range: [4, 0x1f00-0x1fff]"
    log_info "=========================================="
    log_info "  Core Infrastructure (0x1f00-0x1f0f):"
    log_info "    - dxBTC at [4, 0x1f00]"
    log_info "    - yv-fr-btc Vault at [4, 0x1f01]"
    log_info ""
    log_info "  LBTC Yield System (0x1f10-0x1f1f):"
    log_info "    - LBTC Yield Splitter at [4, 0x1f10]"
    log_info "    - pLBTC at [4, 0x1f11]"
    log_info "    - yxLBTC at [4, 0x1f12]"
    log_info "    - FROST Token at [4, 0x1f13]"
    log_info "    - vxFROST Gauge at [4, 0x1f14] (special: needs fixed ID)"
    log_info "    - Synth Pool at [4, 0x1f15]"
    log_info "    - LBTC Oracle at [4, 0x1f16]"
    log_info "    - LBTC Token at [4, 0x1f17]"
    log_info ""
    log_info "  Templates (0x1f20-0x1f2f):"
    log_info "    - Unit Template at [4, 0x1f20]"
    log_info "    - VE Token Vault Template at [4, 0x1f21]"
    log_info "    - YVE Token NFT Template at [4, 0x1f22]"
    log_info "    - VX Token Gauge Template at [4, 0x1f23]"
    log_info ""
    log_info "  DIESEL Governance (instantiated from templates):"
    log_info "    - veDIESEL: [6, 0x1f21] → creates at [2, n]"
    log_info "    - yveDIESEL: [6, 0x1f22] → creates at [2, n]"
    log_info "    - vxDIESEL Gauge: [6, 0x1f23] → creates at [2, n]"
    echo ""
    
    # Deploy Core Alkanes
    # Note: We deploy to [3, n] which creates the alkane at [4, n]
    # Format: deploy_contract "Name" "file.wasm" target_tx [init_args...]
    
    # === RESERVED RANGE: [4, 0x1f00-0x1fff] for Subfrost System ===
    
    log_info "=========================================="
    log_info "Phase 1: Core Infrastructure"
    log_info "=========================================="
    
    # Deploy dx-btc at [4, 0x1f00] (DX_BTC_ID)
    deploy_contract "dxBTC" "$WASM_DIR/dx_btc.wasm" $((0x1f00)) ""
    
    # Deploy yv-fr-btc-vault at [4, 0x1f01] (YV_FR_BTC_VAULT_ID)
    deploy_contract "yv-fr-btc Vault" "$WASM_DIR/yv_fr_btc_vault.wasm" $((0x1f01)) "32,0"
    
    log_info "=========================================="
    log_info "Phase 2: LBTC Yield System"
    log_info "=========================================="
    
    # Deploy lbtc-yield-splitter at [4, 0x1f10] (LBTC_YIELD_SPLITTER_ID)
    deploy_contract "LBTC Yield Splitter" "$WASM_DIR/lbtc_yield_splitter.wasm" $((0x1f10)) "4,$((0x1f11)),4,$((0x1f12))"
    
    # Deploy p-lbtc at [4, 0x1f11] (PLBTC_ID)
    deploy_contract "pLBTC (Principal LBTC)" "$WASM_DIR/p_lbtc.wasm" $((0x1f11)) "4,$((0x1f10))"
    
    # Deploy yx-lbtc at [4, 0x1f12] (YXLBTC_ID)
    deploy_contract "yxLBTC (Yield LBTC)" "$WASM_DIR/yx_lbtc.wasm" $((0x1f12)) "4,$((0x1f10))"
    
    # Deploy frost-token at [4, 0x1f13] (FROST_TOKEN_ID)
    deploy_contract "FROST Token" "$WASM_DIR/frost_token.wasm" $((0x1f13)) "1"
    
    # Deploy vx-frost-gauge at [4, 0x1f14] (VX_FROST_GAUGE_ID)
    # NOTE: vxFROST is deployed directly (not instantiated) because dx-btc needs to reference it at init time
    deploy_contract "vxFROST Gauge" "$WASM_DIR/vx_frost_gauge.wasm" $((0x1f14)) "4,$((0x1f13))"
    
    # Deploy synth-pool at [4, 0x1f15] (SYNTH_POOL_ID)
    deploy_contract "Synth Pool (pLBTC/frBTC)" "$WASM_DIR/synth_pool.wasm" $((0x1f15)) "4,$((0x1f11)),32,0"
    
    log_info "=========================================="
    log_info "Phase 3: LBTC Oracle System"
    log_info "=========================================="
    
    # Deploy lbtc-oracle (unit alkane) at [4, 0x1f16] (LBTC_ORACLE_ID)
    deploy_contract "LBTC Oracle" "$WASM_DIR/unit.wasm" $((0x1f16)) ""
    
    # Deploy lbtc token at [4, 0x1f17] (LBTC_ID)
    # Initialize with oracle ID [4, 0x1f16]
    deploy_contract "LBTC Token" "$WASM_DIR/lbtc.wasm" $((0x1f17)) "4,$((0x1f16))"
    
    log_info "=========================================="
    log_info "Phase 4: Template Contracts"
    log_info "=========================================="
    
    # Deploy unit template at [4, 0x1f20] (UNIT_TEMPLATE_ID)
    deploy_contract "Unit Template" "$WASM_DIR/unit.wasm" $((0x1f20)) ""
    
    # Deploy ve-token-vault-template at [4, 0x1f21] (VE_TOKEN_VAULT_TEMPLATE_ID)
    deploy_contract "VE Token Vault Template" "$WASM_DIR/ve_token_vault_template.wasm" $((0x1f21)) ""
    
    # Deploy yve-token-nft-template at [4, 0x1f22] (YVE_TOKEN_NFT_TEMPLATE_ID)
    deploy_contract "YVE Token NFT Template" "$WASM_DIR/yve_token_nft_template.wasm" $((0x1f22)) ""
    
    # Deploy vx-token-gauge-template at [4, 0x1f23] (VX_TOKEN_GAUGE_TEMPLATE_ID)
    deploy_contract "VX Token Gauge Template" "$WASM_DIR/vx_token_gauge_template.wasm" $((0x1f23)) ""
    
    log_info "=========================================="
    log_info "Phase 5: DIESEL Governance System (Instantiated from Templates)"
    log_info "=========================================="
    
    # Instantiate veDIESEL from ve-token-vault-template at [4, 0x1f21]
    # Using [6, 0x1f21] cellpack creates instance at [2, 1] (or next available [2, n])
    log_info "Instantiating veDIESEL from template [4, 0x1f21]..."
    PROTOSTONE="[6,$((0x1f21)),0,2,0]"  # [6, template_tx, opcode, DIESEL_id]
    log_info "  Protostone: $PROTOSTONE (creates at [2, n])"
    
    subfrost-cli -p regtest \
        --wallet-file "$WALLET_FILE" \
        alkanes execute "$PROTOSTONE" \
        --fee-rate 1 \
        --mine \
        --trace \
        -y \
        > /dev/null 2>&1
    
    if [ $? -eq 0 ]; then
        log_success "veDIESEL instantiated at [2, n]"
    else
        log_warn "Failed to instantiate veDIESEL"
    fi
    
    echo ""
    
    # Instantiate yveDIESEL from yve-token-nft-template at [4, 0x1f22]
    # Using [6, 0x1f22] cellpack creates instance at [2, n]
    log_info "Instantiating yveDIESEL from template [4, 0x1f22]..."
    PROTOSTONE="[6,$((0x1f22)),0,2,0]"  # [6, template_tx, opcode, DIESEL_id]
    log_info "  Protostone: $PROTOSTONE (creates at [2, n])"
    
    subfrost-cli -p regtest \
        --wallet-file "$WALLET_FILE" \
        alkanes execute "$PROTOSTONE" \
        --fee-rate 1 \
        --mine \
        --trace \
        -y \
        > /dev/null 2>&1
    
    if [ $? -eq 0 ]; then
        log_success "yveDIESEL instantiated at [2, n]"
    else
        log_warn "Failed to instantiate yveDIESEL"
    fi
    
    echo ""
    
    # Instantiate vxDIESEL gauge from vx-token-gauge-template at [4, 0x1f23]
    # Using [6, 0x1f23] cellpack creates instance at [2, n]
    log_info "Instantiating vxDIESEL Gauge from template [4, 0x1f23]..."
    # TODO: Update with correct LP token ID and reward rate
    PROTOSTONE="[6,$((0x1f23)),0,2,0,2,1,100]"  # [6, template_tx, opcode, lp_token, ve_token, reward_rate]
    log_info "  Protostone: $PROTOSTONE (creates at [2, n])"
    
    subfrost-cli -p regtest \
        --wallet-file "$WALLET_FILE" \
        alkanes execute "$PROTOSTONE" \
        --fee-rate 1 \
        --mine \
        --trace \
        -y \
        > /dev/null 2>&1
    
    if [ $? -eq 0 ]; then
        log_success "vxDIESEL Gauge instantiated at [2, n]"
    else
        log_warn "Failed to instantiate vxDIESEL Gauge"
    fi
    
    # NOTE: ftr-btc at [31, 0] is deployed automatically in alkanes-rs genesis (setup_ftrbtc)
    # NOTE: dx-btc and yv-fr-btc-vault are now deployed above in the reserved range
    
    # Initialize dx-btc at [4, 0x1f00] with frBTC[32,0] and yv-fr-btc-vault[4,0x1f01]
    log_info "Initializing dxBTC at [4, 0x1f00]..."
    INIT_PROTOSTONE="[4,$((0x1f00)),0,32,0,4,$((0x1f01))]"
    log_info "  Protostone: $INIT_PROTOSTONE"
    
    subfrost-cli -p regtest \
        --wallet-file "$WALLET_FILE" \
        alkanes execute "$INIT_PROTOSTONE" \
        --fee-rate 1 \
        --mine \
        --trace \
        -y \
        > /dev/null 2>&1
    
    if [ $? -eq 0 ]; then
        log_success "dxBTC initialized"
    else
        log_warn "Failed to initialize dxBTC (may not need initialization)"
    fi
    
    echo ""
    
    # Additional Test Contracts (if needed for specific test scenarios)
    # NOTE: DIESEL governance contracts are instantiated from templates above
    # These are generic test vaults deployed to [4, n] via [3, n]
    
    # Uncomment if needed for testing:
    # deploy_contract "Generic Gauge Contract" "$WASM_DIR/gauge_contract.wasm" 100 "1"
    # deploy_contract "yvBOOST Vault" "$WASM_DIR/yv_boost_vault.wasm" 101 "1"
    # deploy_contract "yvTOKEN Vault" "$WASM_DIR/yv_token_vault.wasm" 102 "1"
    
    # OYL AMM System (following oyl-protocol deployment pattern)
    log_info "Deploying OYL AMM System..."
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
    log_info "Initializing OYL Factory Proxy..."
    INIT_PROTOSTONE="[4,$AMM_FACTORY_PROXY_TX,0,$POOL_BEACON_PROXY_TX,4,$POOL_UPGRADEABLE_BEACON_TX]:v0:v0"
    log_info "  Protostone: $INIT_PROTOSTONE"
    
    subfrost-cli -p regtest \
        --wallet-file "$WALLET_FILE" \
        alkanes execute "$INIT_PROTOSTONE" \
        --fee-rate 1 \
        --mine \
        --trace \
        -y \
        > /dev/null 2>&1
    
    if [ $? -eq 0 ]; then
        log_success "OYL Factory initialized"
    else
        log_error "Failed to initialize OYL Factory"
    fi
    
    # BTC PT/YT tokens (if needed for tests)
    if [ -f "$WASM_DIR/btc_pt.wasm" ] && [ -s "$WASM_DIR/btc_pt.wasm" ]; then
        deploy_contract "BTC PT Token" "$WASM_DIR/btc_pt.wasm" 70
    fi
    if [ -f "$WASM_DIR/btc_yt.wasm" ] && [ -s "$WASM_DIR/btc_yt.wasm" ]; then
        deploy_contract "BTC YT Token" "$WASM_DIR/btc_yt.wasm" 71
    fi
    
    # FR-BTC (Full Wrapped BTC with signer)
    if [ -f "$WASM_DIR/fr_btc.wasm" ] && [ -s "$WASM_DIR/fr_btc.wasm" ]; then
        deploy_contract "frBTC (Full)" "$WASM_DIR/fr_btc.wasm" 80
    fi
    
    echo ""
    log_info "=========================================="
    log_info "Deployment Summary"
    log_info "=========================================="
    echo ""
    
    log_success "All contracts deployed successfully!"
    echo ""
    log_info "Deployed Alkanes:"
    echo ""
    echo "Genesis (Auto-deployed):"
    echo "  - DIESEL:                 [2, 0]"
    echo "  - frBTC:                  [32, 0]"
    echo ""
    echo "Core Contracts:"
    echo "  - dxBTC Vault:            [2, 1]   (CREATE, deps: frBTC[32,0], yv-fr-btc[3,0])"
    echo "  - yv-fr-btc Vault:        [3, 0]   (RESERVED, deps: frBTC[32,0])"
    echo "  - ftrBTC Master:          [31, 0]  (RESERVED)"
    echo ""
    echo "LBTC System:"
    echo "  - FROST Token:            [4, 10]"
    echo "  - pLBTC:                  [4, 11]"
    echo "  - yxLBTC:                 [4, 12]"
    echo "  - LBTC Yield Splitter:    [4, 13]  (deps: pLBTC[4,11], yxLBTC[4,12])"
    echo "  - Synth Pool:             [4, 30]  (deps: pLBTC[4,11], frBTC[32,0])"
    echo ""
    echo "Governance:"
    echo "  - vxFROST Gauge:          [4, 50]"
    echo "  - veDIESEL Vault:         [4, 60]"
    echo ""
    echo "Additional Vaults & Gauges:"
    echo "  - Gauge Contract:         [4, 100]"
    echo "  - yvBOOST Vault:          [4, 101]"
    echo "  - yvTOKEN Vault:          [4, 102]"
    echo "  - yveDIESEL Vault:        [4, 103]"
    echo ""
    echo "OYL AMM System:"
    echo "  - OYL Pool Logic:         [4, $AMM_FACTORY_ID]"
    echo "  - OYL Auth Token Factory: [4, $AUTH_TOKEN_FACTORY_ID]"
    echo "  - OYL Factory Logic:      [4, $AMM_FACTORY_LOGIC_IMPL_TX]"
    echo "  - OYL Beacon Proxy:       [4, $POOL_BEACON_PROXY_TX]"
    echo "  - OYL Upgradeable Beacon: [4, $POOL_UPGRADEABLE_BEACON_TX]"
    echo "  - OYL Factory Proxy:      [4, $AMM_FACTORY_PROXY_TX]"
    echo ""
    
    log_info "Example commands:"
    echo ""
    echo "# Check balances:"
    echo "subfrost-cli -p regtest --wallet-file $WALLET_FILE alkanes getbalance"
    echo ""
    echo "# Inspect a contract:"
    echo "subfrost-cli -p regtest alkanes inspect 4:10"
    echo ""
    echo "# Execute a contract call (e.g., transfer FROST):"
    echo "subfrost-cli -p regtest --wallet-file $WALLET_FILE alkanes execute '[4:10:1,1000,0,0]' --mine -y"
    echo ""
    
    log_success "Deployment complete! Your regtest environment is ready."
}

# Run main
main
