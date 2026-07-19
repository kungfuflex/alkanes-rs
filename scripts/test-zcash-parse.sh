#!/bin/bash
set -e

BLOCK_HEIGHT=1500000
BLOCK_HASH="00000000019e5b25a95c7607e7789eb326fddd69736970ebbe1c7d00247ef902"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

echo "Testing Zcash block parsing for block $BLOCK_HEIGHT"
echo "Block hash: $BLOCK_HASH"
echo ""

# Get the block bytes from the Zcash node
echo "Fetching block data from Zcash node..."
BLOCK_DATA=$(curl -s --data-binary "{\"jsonrpc\":\"1.0\",\"id\":\"test\",\"method\":\"getblock\",\"params\":[\"$BLOCK_HASH\", 0]}" \
  -H 'content-type: text/plain;' http://localhost:8232/ | jq -r '.result')

if [ -z "$BLOCK_DATA" ] || [ "$BLOCK_DATA" = "null" ]; then
  echo "Error: Failed to fetch block data"
  exit 1
fi

echo "Block data fetched successfully (${#BLOCK_DATA} hex characters)"
echo ""

# Create a test example within the project
TEST_DIR="$PROJECT_ROOT/examples"
mkdir -p "$TEST_DIR"

cat > "$TEST_DIR/test_zcash_parse.rs" << 'EOF'
use std::io::Cursor;
use bitcoin::Block;
use metashrew_support::block::AuxpowBlock;

fn main() {
    let block_hex = std::env::args().nth(1).expect("Expected block hex as argument");
    let block_bytes = hex::decode(&block_hex).expect("Failed to decode hex");
    
    println!("Attempting to parse Zcash block...");
    println!("Block size: {} bytes", block_bytes.len());
    
    match AuxpowBlock::parse(&mut Cursor::new(block_bytes.clone())) {
        Ok(auxpow_block) => {
            println!("✓ Successfully parsed AuxpowBlock!");
            
            let consensus_block: Block = auxpow_block.to_consensus();
            println!("✓ Successfully converted to consensus Block!");
            println!("");
            println!("Block details:");
            println!("  Version: {:?}", consensus_block.header.version);
            println!("  Previous block hash: {}", consensus_block.header.prev_blockhash);
            println!("  Merkle root: {}", consensus_block.header.merkle_root);
            println!("  Time: {}", consensus_block.header.time);
            println!("  Bits: {}", consensus_block.header.bits.to_consensus());
            println!("  Nonce: {}", consensus_block.header.nonce);
            println!("  Transaction count: {}", consensus_block.txdata.len());
            
            if !consensus_block.txdata.is_empty() {
                println!("");
                println!("First transaction (coinbase):");
                println!("  Txid: {}", consensus_block.txdata[0].compute_txid());
                println!("  Inputs: {}", consensus_block.txdata[0].input.len());
                println!("  Outputs: {}", consensus_block.txdata[0].output.len());
            }
            
            std::process::exit(0);
        }
        Err(e) => {
            eprintln!("✗ Failed to parse AuxpowBlock: {:?}", e);
            std::process::exit(1);
        }
    }
}
EOF

echo "Building test parser..."
cd "$PROJECT_ROOT"
cargo build --example test_zcash_parse --features zcash --quiet 2>&1 | grep -v "warning:" || true

echo ""
echo "Running parser test..."
cargo run --example test_zcash_parse --features zcash -- "$BLOCK_DATA" 2>&1 | grep -E "^(Attempting|✓|✗|Block details|  [A-Z]|  Txid)"
RESULT=${PIPESTATUS[0]}

# Cleanup
rm -f "$TEST_DIR/test_zcash_parse.rs"

if [ $RESULT -eq 0 ]; then
  echo ""
  echo "=========================================="
  echo "✓ Test PASSED: Zcash block parsed successfully!"
  echo "=========================================="
else
  echo ""
  echo "=========================================="
  echo "✗ Test FAILED: Could not parse Zcash block"
  echo "=========================================="
  exit 1
fi
