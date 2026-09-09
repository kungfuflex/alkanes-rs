#!/usr/bin/env bash
set -e

# FROST Multisig Test Script
# Tests the subfrost-cli and alkanes-cli workflow for FROST multisig operations

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
SUBFROST_CLI="${SUBFROST_CLI:-/data/subfrost-master/target/release/subfrost-cli}"
ALKANES_CLI="${ALKANES_CLI:-alkanes-cli}"
NETWORK="regtest"
FROST_FILES="~/.subfrost-test"
PASSPHRASE="testtesttest"
THRESHOLD=6
SIGNERS=9
ALKANES_WALLET="~/.alkanes/test-wallet.json"
SEND_AMOUNT_BTC="0.1"
SEND_AMOUNT_SATS="10000000"  # 0.1 BTC = 10,000,000 satoshis
FEE_RATE="1"
# RPC Configuration for regtest
BITCOIN_RPC_URL="${BITCOIN_RPC_URL:-http://localhost:18888}"
JSONRPC_URL="${JSONRPC_URL:-http://localhost:18888}"

# Expand tilde
FROST_FILES="${FROST_FILES/#\~/$HOME}"
ALKANES_WALLET="${ALKANES_WALLET/#\~/$HOME}"

echo -e "${GREEN}=== FROST Multisig Test Script ===${NC}\n"

# Step 1: Clean up previous test files
echo -e "${YELLOW}Step 1: Cleaning up previous test files...${NC}"
rm -rf "$FROST_FILES"
rm -f "$ALKANES_WALLET"
echo -e "${GREEN}✓ Cleanup complete${NC}\n"

# Step 2: Create FROST multisig setup
echo -e "${YELLOW}Step 2: Creating FROST multisig (threshold=$THRESHOLD, signers=$SIGNERS)...${NC}"
$SUBFROST_CLI -p $NETWORK \
  --frost-files "$FROST_FILES" \
  --passphrase "$PASSPHRASE" \
  frost create \
  --threshold $THRESHOLD \
  --signers $SIGNERS

if [ $? -ne 0 ]; then
    echo -e "${RED}✗ Failed to create FROST multisig${NC}"
    exit 1
fi
echo -e "${GREEN}✓ FROST multisig created successfully${NC}\n"

# Step 3: Get multisig address
echo -e "${YELLOW}Step 3: Getting multisig address...${NC}"
MULTISIG_ADDRESS=$($SUBFROST_CLI -p $NETWORK \
  --frost-files "$FROST_FILES" \
  --jsonrpc-url "$JSONRPC_URL" \
  wallet addresses p2tr:0 | grep -oE 'bcrt1[a-zA-Z0-9]+' | head -1)

if [ -z "$MULTISIG_ADDRESS" ]; then
    echo -e "${RED}✗ Failed to get multisig address${NC}"
    exit 1
fi
echo -e "${GREEN}✓ Multisig address: $MULTISIG_ADDRESS${NC}\n"

# Step 4: Mine initial blocks, then blocks to FROST address
echo -e "${YELLOW}Step 4: Mining 101 blocks to FROST address (will be immediately spendable)...${NC}"
$ALKANES_CLI -p $NETWORK \
  bitcoind generatetoaddress 101 "$MULTISIG_ADDRESS"

if [ $? -ne 0 ]; then
    echo -e "${RED}✗ Failed to mine blocks${NC}"
    exit 1
fi
echo -e "${GREEN}✓ Initial blocks mined successfully${NC}\n"

# Step 4a: Mine 100 more blocks for coinbase maturity of first batch
echo -e "${YELLOW}Step 4a: Mining 100 blocks for coinbase maturity...${NC}"
$ALKANES_CLI -p $NETWORK \
  bitcoind generatetoaddress 100 "$MULTISIG_ADDRESS"

if [ $? -ne 0 ]; then
    echo -e "${RED}✗ Failed to mine maturity blocks${NC}"
    exit 1
fi
echo -e "${GREEN}✓ Maturity blocks mined successfully${NC}\n"

# Wait for indexer to start catching up
echo -e "${YELLOW}Waiting for indexer to catch up (30s)...${NC}"
sleep 30

# Step 4.5: Sync to ensure indexer catches up
echo -e "${YELLOW}Step 4.5: Syncing indexer with blockchain...${NC}"
$ALKANES_CLI -p $NETWORK \
  --jsonrpc-url "$JSONRPC_URL" \
  wallet sync

if [ $? -ne 0 ]; then
    echo -e "${RED}✗ Failed to sync${NC}"
    exit 1
fi
echo -e "${GREEN}✓ Indexer synced successfully${NC}\n"

# Step 5: Create regular wallet with alkanes-cli
echo -e "${YELLOW}Step 5: Creating regular wallet with alkanes-cli...${NC}"
$ALKANES_CLI -p $NETWORK \
  --wallet-file "$ALKANES_WALLET" \
  --passphrase "$PASSPHRASE" \
  wallet create

if [ $? -ne 0 ]; then
    echo -e "${RED}✗ Failed to create wallet${NC}"
    exit 1
fi
echo -e "${GREEN}✓ Wallet created successfully${NC}\n"

# Step 6: Get regular wallet address
echo -e "${YELLOW}Step 6: Getting regular wallet address...${NC}"
WALLET_ADDRESS=$($ALKANES_CLI -p $NETWORK \
  --wallet-file "$ALKANES_WALLET" \
  wallet addresses p2tr:0 | grep -oE 'bcrt1[a-zA-Z0-9]+' | head -1)

if [ -z "$WALLET_ADDRESS" ]; then
    echo -e "${RED}✗ Failed to get wallet address${NC}"
    exit 1
fi
echo -e "${GREEN}✓ Wallet address: $WALLET_ADDRESS${NC}\n"

# Step 7: Send Bitcoin from multisig to regular wallet
echo -e "${YELLOW}Step 7: Sending $SEND_AMOUNT_BTC BTC ($SEND_AMOUNT_SATS sats) from multisig to wallet...${NC}"
$SUBFROST_CLI -p $NETWORK \
  --frost-files "$FROST_FILES" \
  --passphrase "$PASSPHRASE" \
  --jsonrpc-url "$JSONRPC_URL" \
  wallet send "$WALLET_ADDRESS" $SEND_AMOUNT_SATS \
  --fee-rate $FEE_RATE

if [ $? -ne 0 ]; then
    echo -e "${RED}✗ Failed to send Bitcoin${NC}"
    exit 1
fi
echo -e "${GREEN}✓ Bitcoin sent successfully${NC}\n"

# Step 8: Verify the transaction
echo -e "${YELLOW}Step 8: Mining a block to confirm transaction...${NC}"
$ALKANES_CLI -p $NETWORK \
  bitcoind generatetoaddress 1 "$WALLET_ADDRESS"

if [ $? -ne 0 ]; then
    echo -e "${RED}✗ Failed to mine confirmation block${NC}"
    exit 1
fi
echo -e "${GREEN}✓ Transaction confirmed${NC}\n"

# Step 9: Check wallet balance
echo -e "${YELLOW}Step 9: Checking wallet balance...${NC}"
$ALKANES_CLI -p $NETWORK \
  --wallet-file "$ALKANES_WALLET" \
  --jsonrpc-url "$JSONRPC_URL" \
  wallet balance

if [ $? -ne 0 ]; then
    echo -e "${RED}✗ Failed to check balance${NC}"
    exit 1
fi
echo -e "${GREEN}✓ Balance check complete${NC}\n"

echo -e "${GREEN}=== Test completed successfully! ===${NC}"
