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

# Start the indexer
echo "Starting alkanes-contract-indexer..."
exec /usr/local/bin/app "$@"
