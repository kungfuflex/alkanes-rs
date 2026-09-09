#!/bin/bash

echo "=========================================="
echo " Block 349330 Debug Setup Status"
echo "=========================================="
echo ""

# Check if block hex file exists
BLOCK_FILE="crates/alkanes/src/tests/blocks/zec_349330.hex"
echo "1. Block Data File:"
if [ -f "$BLOCK_FILE" ]; then
    SIZE=$(wc -c < "$BLOCK_FILE")
    echo "   ✓ EXISTS: $BLOCK_FILE"
    echo "   File size: $SIZE bytes"
    echo "   Hex length: $((SIZE / 2)) bytes of block data"
else
    echo "   ✗ NOT FOUND: $BLOCK_FILE"
    echo "   Run: ./scripts/fetch-block-349330.sh"
fi
echo ""

# Check if test module is enabled
MOD_FILE="crates/alkanes/src/tests/mod.rs"
echo "2. Test Module Status:"
if grep -q "^pub mod zcash_block_349330;" "$MOD_FILE" 2>/dev/null; then
    echo "   ✓ ENABLED in $MOD_FILE"
elif grep -q "^// pub mod zcash_block_349330;" "$MOD_FILE" 2>/dev/null; then
    echo "   ⚠ COMMENTED OUT in $MOD_FILE"
    echo "   Uncomment the line to enable tests"
else
    echo "   ✗ NOT FOUND in $MOD_FILE"
    echo "   Check the file manually"
fi
echo ""

# Check if test file exists
TEST_FILE="crates/alkanes/src/tests/zcash_block_349330.rs"
echo "3. Test File:"
if [ -f "$TEST_FILE" ]; then
    TESTS=$(grep -c "fn test_" "$TEST_FILE")
    echo "   ✓ EXISTS: $TEST_FILE"
    echo "   Contains $TESTS test functions"
else
    echo "   ✗ NOT FOUND: $TEST_FILE"
fi
echo ""

# Check documentation
echo "4. Documentation Files:"
for doc in "DEBUG_BLOCK_349330.md" "QUICKSTART_BLOCK_349330_DEBUG.md"; do
    if [ -f "$doc" ]; then
        echo "   ✓ $doc"
    else
        echo "   ✗ $doc"
    fi
done
echo ""

# Overall status
echo "=========================================="
if [ -f "$BLOCK_FILE" ] && grep -q "^pub mod zcash_block_349330;" "$MOD_FILE" 2>/dev/null; then
    echo " STATUS: ✓ READY TO TEST"
    echo ""
    echo " Run: cargo test --features zcash zcash_block_349330 --lib -- --nocapture"
elif [ -f "$BLOCK_FILE" ]; then
    echo " STATUS: ⚠ BLOCK FETCHED, MODULE DISABLED"
    echo ""
    echo " Uncomment the module in: $MOD_FILE"
    echo " Then run: cargo test --features zcash zcash_block_349330 --lib -- --nocapture"
elif grep -q "pub mod zcash_block_349330;" "$MOD_FILE" 2>/dev/null; then
    echo " STATUS: ⚠ MODULE ENABLED, MISSING BLOCK DATA"
    echo ""
    echo " Run: ./scripts/fetch-block-349330.sh"
else
    echo " STATUS: ⏳ SETUP INCOMPLETE"
    echo ""
    echo " Next steps:"
    echo "   1. ./scripts/fetch-block-349330.sh"
    echo "   2. Uncomment module in $MOD_FILE"
    echo "   3. cargo test --features zcash zcash_block_349330 --lib -- --nocapture"
fi
echo "=========================================="
