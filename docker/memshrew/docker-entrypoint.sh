#!/bin/bash -x
set -euo pipefail

# Default configuration
DAEMON_RPC_ADDR=${DAEMON_RPC_ADDR:-"http://127.0.0.1:8332"}
HOST=${HOST:-"0.0.0.0"}
PORT=${PORT:-"8080"}
AUTH=${AUTH:-"bitcoinrpc:bitcoinrpc"}
RUST_LOG=${LOG_FILTERS:-"none,memshrew=debug"}


# Configure logging
export RUST_LOG=${RUST_LOG:-"debug"}

# Execute memshrew with parameters
exec memshrew-p2p \
    --daemon-rpc-url "$DAEMON_RPC_ADDR" \
    --p2p-addr "$P2P_ADDR" \
    --auth "$AUTH" \
    --host "$HOST" \
    --port "$PORT"
