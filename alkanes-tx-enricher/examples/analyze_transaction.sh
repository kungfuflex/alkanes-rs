#!/bin/bash

# Example script to analyze a transaction with alkanes-tx-enricher
# This script demonstrates how to use the CLI tool to analyze a Bitcoin transaction
# with protorune data enrichment

# Set these variables
TX_HEX="02000000000101868b4d911d79a8f13612bb74df3f0b12766a5cf9b9ea30c04dd6050d18b2d95a1900000000ffffffff044a010000000000001600143369178e4af536e1dfd06ad8075048f59b7923fc0000000000000000506a5d4c4cff7f8194ec82d08bc0a886f5ea848890a4faed01ff7fc3d3af848891acfdc1e38688e5bbabc0b501ff7f89e182bceefbe8c7b6d9d7b6befbd9a88001ff7ffafddfdfdfdffffdfb9f8eefdf07b80b00000000000016001485fa2f8a4ebb17225ea647901effb3a90318e239e6f30700000000001600143369178e4af536e1dfd06ad8075048f59b7923fc02483045022100df6ca8750ce8d3af5e9458c341d1f388580f3965542e765e064e1c7c8c9ab41c022076201617dde7ae7de5fc9e8b3ec1198da703ee9f27c38c29238ed90d3d9ad0230121034015870024aac759c6089e4d2cc00e338608f4ff819f4433b42040e5a4cadce700000000"
BLOCK_HEIGHT=880001

# Set log level for more detailed output
export RUST_LOG=info

# Run the tool (make sure to build it first with 'cargo build --release')
echo "Analyzing transaction at block height $BLOCK_HEIGHT..."
# Run using cargo from the project root
cargo run --bin alkanes-tx-enricher -- \
  --tx-hex "$TX_HEX" \
  --block-height "$BLOCK_HEIGHT" \
  --verbose

# If you want to save the output to a file
# cargo run --bin alkanes-tx-enricher -- \
#   --tx-hex "$TX_HEX" \
#   --block-height "$BLOCK_HEIGHT" \
#   --verbose > transaction_analysis.txt