#!/bin/bash

# Test script to verify alkanes-cli addresses match oyl-sdk
# Uses the standard test mnemonic

MNEMONIC="abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
WALLET_FILE="/tmp/test-wallet-$$.json"

echo "Testing address derivation with test mnemonic..."
echo "Mnemonic: $MNEMONIC"
echo ""

# Create wallet with alkanes-cli
echo "Creating wallet with alkanes-cli..."
./target/release/alkanes-cli --passphrase testtest --wallet-file "$WALLET_FILE" wallet create "$MNEMONIC"

echo ""
echo "Getting addresses from alkanes-cli:"
echo "  p2tr:0 (Taproot):"
./target/release/alkanes-cli --wallet-file "$WALLET_FILE" wallet addresses p2tr:0

echo ""
echo "  p2wpkh:0 (Native SegWit):"
./target/release/alkanes-cli --wallet-file "$WALLET_FILE" wallet addresses p2wpkh:0

echo ""
echo "Expected addresses from oyl-sdk (using same mnemonic, index 0, regtest):"
echo "  Taproot:       bcrt1p5cyxnuxmeuwuvkwfem96lqzszd02n6xdcjrs20cac6yqjjwudpxqkedrcr"
echo "  Native SegWit: bcrt1qcr8te4kr609gcawutmrza0j4xv80jy8z306fyu"

echo ""
echo "Cleaning up..."
rm -f "$WALLET_FILE"

echo ""
echo "NOTE: For signet, use -p signet flag and compare with oyl-sdk signet addresses"
