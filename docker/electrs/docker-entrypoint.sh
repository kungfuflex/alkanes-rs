#!/bin/bash -x

# Increase file descriptor limit if possible
ulimit -n $(ulimit -n -H) || true

# Default values
: ${ELECTRS_NETWORK:=mainnet}
: ${ELECTRS_DAEMON_RPC_ADDR:=127.0.0.1:8332}
: ${ELECTRS_HTTP_ADDR:=0.0.0.0:50010}
: ${ELECTRS_ELECTRUM_RPC_ADDR:=0.0.0.0:50001}
: ${ELECTRS_DB_DIR:=/data/electrs}
: ${ELECTRS_DAEMON_DIR:=/data/bitcoin}
: ${ELECTRS_UTXOS_LIMIT:=4294967296}



# Write auth cookie
mkdir -p "${ELECTRS_DAEMON_DIR}/regtest"

exec /usr/local/bin/flextrs -vvv \
    --db-dir "${ELECTRS_DB_DIR}" \
    --daemon-dir "${ELECTRS_DAEMON_DIR}" \
    --network "${ELECTRS_NETWORK}" \
    --daemon-rpc-addr "${ELECTRS_DAEMON_RPC_ADDR}" \
    --http-addr "${ELECTRS_HTTP_ADDR}" \
    --electrum-rpc-addr "${ELECTRS_ELECTRUM_RPC_ADDR}" \
    --utxos-limit "${ELECTRS_UTXOS_LIMIT}" \
    --auth ${ELECTRS_AUTH} \
    "$@"
