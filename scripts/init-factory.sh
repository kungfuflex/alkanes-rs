  export FACTORY_INIT_PROTOSTONE="[2:1:1:p1]:v0:v0,[4,65522,0,780993,4,65523]:v0:v0"

  ./target/release/alkanes-cli -p regtest \
    --wallet-file /home/ubuntu/.alkanes/wallet.json \
    --passphrase testtesttest \
    alkanes execute "$FACTORY_INIT_PROTOSTONE" \
    --from p2tr:0 \
    --inputs 2:1:1 \
    --fee-rate 1 \
    --mine \
    --trace \
    -y
