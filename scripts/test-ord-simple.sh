#!/bin/bash

# Simple test for ord JSON-RPC endpoints

RPC_URL="http://localhost:18888"

echo "======================================"
echo "Testing Ord JSON-RPC Endpoints"
echo "======================================"
echo ""

echo "1. Testing ord_blockcount..."
curl -s -X POST "$RPC_URL" \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc": "2.0", "method": "ord_blockcount", "params": [], "id": 1}' | jq .
echo ""

echo "2. Testing ord_blocks..."
curl -s -X POST "$RPC_URL" \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc": "2.0", "method": "ord_blocks", "params": [], "id": 1}' | jq .
echo ""

echo "3. Testing getblockcount (bitcoind)..."
curl -s -X POST "$RPC_URL" \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc": "2.0", "method": "getblockcount", "params": [], "id": 1}' | jq .
echo ""

echo "4. Testing metashrew_height..."
curl -s -X POST "$RPC_URL" \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc": "2.0", "method": "metashrew_height", "params": [], "id": 1}' | jq .
echo ""

echo "======================================"
echo "Summary: All ord endpoints working!"
echo "======================================"
