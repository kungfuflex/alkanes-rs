#!/bin/bash

# Mint auth token [2:1] by calling the auth token factory at [4, 65517]
# The factory will create the token and send it to the caller (v0)

export AUTH_TOKEN_FACTORY_TX=65517

./target/release/alkanes-cli -p regtest \
  --wallet-file /home/ubuntu/.alkanes/wallet.json \
  --passphrase testtesttest \
  alkanes execute "[4,${AUTH_TOKEN_FACTORY_TX}]:v0:v0" \
  --from p2tr:0 \
  --fee-rate 1 \
  --mine \
  --trace \
  -y

echo ""
echo "Auth token [2:1] should now be minted and in your wallet"
echo "Verify with: ./target/release/alkanes-cli -p regtest --wallet-file ~/.alkanes/wallet.json --passphrase testtesttest wallet utxos"
