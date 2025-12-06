#!/bin/bash
set -e

echo "Alkanes Contract Indexer - Starting"

# Wait for database to be ready
echo "Waiting for database to be ready..."
until pg_isready -h "$(echo $DATABASE_URL | sed -n 's/.*@\([^:]*\).*/\1/p')" -U "$(echo $DATABASE_URL | sed -n 's/.*:\/\/\([^:]*\).*/\1/p')" > /dev/null 2>&1; do
  echo "Database is unavailable - sleeping"
  sleep 2
done

echo "Database is ready!"

# Initialize database schema if not exists
echo "Checking database schema..."
TABLE_COUNT=$(psql "$DATABASE_URL" -t -c "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = 'public' AND table_name IN ('Pool', 'PoolState', 'PoolSwap', 'PoolMint', 'PoolBurn', 'PoolCreation')" 2>/dev/null || echo "0")

if [ "$TABLE_COUNT" -lt 6 ]; then
  echo "Database schema not initialized. Running dbctl push..."

  # Build and run dbctl to initialize schema
  if [ -f "/usr/local/bin/dbctl" ]; then
    /usr/local/bin/dbctl push
  else
    echo "Warning: dbctl not found. Schema may need manual initialization."
    echo "Run: cargo run --bin dbctl -- push"
  fi

  # Run migrations if they exist
  if [ -f "/usr/local/bin/dbctl" ]; then
    echo "Running migrations..."
    /usr/local/bin/dbctl migrate || echo "No migrations to run or already applied"
  fi
else
  echo "Database schema already exists (found $TABLE_COUNT core tables)"
fi

# Check if we need to reindex due to chain reset (regtest redeployment)
echo "Checking for chain reset..."
STORED_HASH=$(psql "$DATABASE_URL" -t -c "SELECT block_hash FROM indexer_position WHERE id = 1" 2>/dev/null | tr -d ' ' || echo "")

if [ -n "$STORED_HASH" ] && [ "$STORED_HASH" != "" ]; then
  echo "Found stored block hash: $STORED_HASH"

  # Wait for jsonrpc to be ready
  echo "Waiting for jsonrpc to be ready..."
  JSONRPC_READY=0
  for i in $(seq 1 30); do
    if curl -s "$JSONRPC_URL" > /dev/null 2>&1; then
      JSONRPC_READY=1
      break
    fi
    echo "jsonrpc is unavailable - sleeping ($i/30)"
    sleep 2
  done

  if [ "$JSONRPC_READY" -eq 1 ]; then
    # Get the stored height
    STORED_HEIGHT=$(psql "$DATABASE_URL" -t -c "SELECT height FROM indexer_position WHERE id = 1" 2>/dev/null | tr -d ' ' || echo "0")

    if [ "$STORED_HEIGHT" -gt 0 ]; then
      # Get the block hash at stored height from the chain using btc_getblockhash
      CHAIN_HASH=$(curl -s -X POST "$JSONRPC_URL" \
        -H "Content-Type: application/json" \
        -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"btc_getblockhash\",\"params\":[$STORED_HEIGHT]}" \
        2>/dev/null | jq -r '.result // empty' | tr -d '"' || echo "")

      echo "Chain hash at height $STORED_HEIGHT: $CHAIN_HASH"

      if [ -n "$CHAIN_HASH" ] && [ "$CHAIN_HASH" != "$STORED_HASH" ]; then
        echo "⚠️  Chain reset detected! Stored hash doesn't match chain hash."
        echo "   Stored: $STORED_HASH"
        echo "   Chain:  $CHAIN_HASH"
        echo "Resetting indexer to reindex from block 0..."

        if [ -f "/usr/local/bin/dbctl" ]; then
          /usr/local/bin/dbctl reset
          echo "✓ Database reset complete. Will reindex from START_HEIGHT=${START_HEIGHT:-0}"
        fi
      else
        echo "✓ Chain hashes match - no reset needed"
      fi
    fi
  else
    echo "Warning: Could not connect to jsonrpc to verify chain state"
  fi
else
  echo "No stored position found - fresh start"
fi

# Start the indexer
echo "Starting alkanes-contract-indexer..."
exec /usr/local/bin/app "$@"
