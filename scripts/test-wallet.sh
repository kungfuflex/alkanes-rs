#!/bin/bash
./target/release/alkanes-cli --wallet-file /tmp/wallet-file.json --passphrase testtesttest -p regtest wallet create 
./target/release/alkanes-cli --wallet-file /tmp/wallet-file.json --passphrase testtesttest -p regtest bitcoind generatetoaddress 201 p2tr:0
sleep 10
./target/release/alkanes-cli --wallet-file /tmp/wallet-file.json --passphrase testtesttest -p regtest wallet utxos p2tr:0
