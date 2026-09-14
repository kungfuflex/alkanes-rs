#!/bin/bash
set -euo pipefail

# Default configuration
DAEMON_RPC_ADDR=${DAEMON_RPC_ADDR:-"127.0.0.1:8332"}
HOST=${HOST:-"0.0.0.0"}
PORT=${PORT:-"8080"}
INDEXER=${INDEXER:-"/metashrew/indexer.wasm"}
AUTH=${AUTH:-"bitcoinrpc:bitcoinrpc"}
DB_PATH=${DB_PATH:-"/data"}
LOG_FILTERS=${LOG_FILTERS:-"debug"}
START_BLOCK=${START_BLOCK:-""}
EXIT_AT=${EXIT_AT:-""}
LABEL=${LABEL:-""}

# Validate required files
if [ ! -f "$INDEXER" ]; then
    echo "Error: Indexer WASM file not found at $INDEXER"
    exit 1
fi

# Parse host/port from DAEMON_RPC_ADDR if not explicitly set
if [ -z "$HOST" ] || [ -z "$PORT" ]; then
    IFS=':' read -r addr_host addr_port <<< "$DAEMON_RPC_ADDR"
    HOST=${HOST:-$addr_host}
    PORT=${PORT:-$addr_port}
fi

# Configure logging
export RUST_LOG=DEBUG
# Build command with optional parameters
CMD="/usr/local/bin/rockshrew-mono --host $HOST --port $PORT --indexer $INDEXER --db-path $DB_PATH --auth $AUTH --daemon-rpc-url $DAEMON_RPC_ADDR"

if [ -n "$START_BLOCK" ]; then
    CMD="$CMD --start-block $START_BLOCK"
fi

if [ -n "$EXIT_AT" ]; then
    CMD="$CMD --exit-at $EXIT_AT"
fi

if [ -n "$LABEL" ]; then
    CMD="$CMD --label $LABEL"
fi
# Execute rockshrew
exec $CMD
