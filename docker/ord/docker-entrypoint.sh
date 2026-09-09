#!/bin/bash
set -e

export _CHAIN=${CHAIN:-mainnet}
export _RPCUSER=${RPCUSER:-bitcoinrpc}
export _RPCPASSWORD=${RPCPASSWORD:-bitcoinrpc}
export _DAEMON_RPC_ADDR=${DAEMON_RPC_ADDR:-127.0.0.1:8332}
export _PORT=${PORT:-8090}

declare -a networks=("mainnet", "regtest", "signet", "testnet4", "testnet3", "")
for i in "${networks[@]}"
do
  mkdir -p /bitcoin/${i}
  echo "${_RPCUSER}:${_RPCPASSWORD}" > /bitcoin/${i}/.cookie
done

until curl -s -u "${_RPCUSER}:${_RPCPASSWORD}" --data-binary '{"jsonrpc": "1.0", "id": "curltest", "method": "getblockcount", "params": []}' -H 'content-type: text/plain;' "http://${_DAEMON_RPC_ADDR}/" | jq -e '.result' > /dev/null; do
  >&2 echo "Bitcoind is unavailable - sleeping"
  sleep 0.5
done

>&2 echo "Bitcoind is up - executing command"

ord --index-transactions --index-addresses --index-sats --index-runes --chain $_CHAIN --bitcoin-rpc-url ${_DAEMON_RPC_ADDR} --bitcoin-rpc-username $_RPCUSER --bitcoin-rpc-password $_RPCPASSWORD --bitcoin-data-dir /bitcoin server --http-port $_PORT
