#!/bin/bash
set -e

# Set default values
DB_PATH=${DB_PATH:-/rocksdb}
HOST=${HOST:-0.0.0.0}
PORT=${PORT:-8080}
START_BLOCK=${START_BLOCK:-880000}
INDEXER_PATH=${INDEXER_PATH:-/usr/local/bin/alkanes.wasm}
CORS=${CORS:-"*"}

# Increase file descriptor limit
ulimit -n $(ulimit -n -H)

# Set log level if not already set
export RUST_LOG=${RUST_LOG:-debug}

# Execute rockshrew-mono with parameters
exec rockshrew-mono \
  --db-path "$DB_PATH" \
  --indexer "$INDEXER_PATH" \
  --host "$HOST" \
  --port "$PORT" \
  --start-block "$START_BLOCK" \
  --cors "$CORS" \
  "$@"
