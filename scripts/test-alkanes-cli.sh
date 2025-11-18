./target/release/alkanes-cli --wallet-file ~/.alkanes/wallet.json -p regtest bitcoind generatetoaddress 201 p2tr:0
sleep 10
./target/release/alkanes-cli --wallet-file ~/.alkanes/wallet.json --passphrase testtesttest -p regtest alkanes execute --envelope ./prod_wasms/dx_btc.wasm "[3,$((0x1f00)),0,32,0,4,$((0x1f01)),4,$((0x1f22)),4,$((0x1f14))]" --to p2wsh:0 --change p2tr:0 --from p2tr:0 --fee-rate 1 -y --mine --trace
sleep 10
./target/release/alkanes-cli --wallet-file ~/.alkanes/wallet.json -p regtest esplora address-txs p2tr:0 --exclude-coinbase
