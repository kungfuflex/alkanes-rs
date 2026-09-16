#!/bin/bash
# Helper script to mine blocks in regtest

set -e

BLOCKS=${1:-1}

echo "⛏️  Mining $BLOCKS blocks in regtest..."

# Create wallet if it doesn't exist
docker-compose exec -T bitcoind bitcoin-cli -regtest -rpcuser=bitcoinrpc -rpcpassword=bitcoinrpc createwallet "test" 2>/dev/null || true

# Get a new address
ADDRESS=$(docker-compose exec -T bitcoind bitcoin-cli -regtest -rpcuser=bitcoinrpc -rpcpassword=bitcoinrpc getnewaddress | tr -d '\r')

# Mine blocks
docker-compose exec -T bitcoind bitcoin-cli -regtest -rpcuser=bitcoinrpc -rpcpassword=bitcoinrpc generatetoaddress "$BLOCKS" "$ADDRESS"

echo "✅ Mined $BLOCKS blocks to $ADDRESS"

# Show current height
HEIGHT=$(docker-compose exec -T bitcoind bitcoin-cli -regtest -rpcuser=bitcoinrpc -rpcpassword=bitcoinrpc getblockcount | tr -d '\r')
echo "📊 Current block height: $HEIGHT"
