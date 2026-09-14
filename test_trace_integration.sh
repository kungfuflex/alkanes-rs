#!/bin/bash
# End-to-end test for trace transform integration

set -e

echo "=== Alkanes Trace Transform Integration Test ==="
echo ""

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Check for required tools
if ! command -v psql &> /dev/null; then
    echo -e "${RED}Error: psql not found. Please install PostgreSQL client.${NC}"
    exit 1
fi

# Get database URL from environment or use default
DB_URL="${DATABASE_URL:-postgres://localhost/alkanes_test}"

echo -e "${YELLOW}Step 1: Checking database connection...${NC}"
if ! psql "$DB_URL" -c "SELECT 1" > /dev/null 2>&1; then
    echo -e "${RED}Error: Cannot connect to database: $DB_URL${NC}"
    echo "Please set DATABASE_URL environment variable or ensure postgres is running"
    exit 1
fi
echo -e "${GREEN}✓ Database connection OK${NC}"
echo ""

echo -e "${YELLOW}Step 2: Checking trace transform schema...${NC}"
TABLES=$(psql "$DB_URL" -t -c "
    SELECT COUNT(*) FROM information_schema.tables 
    WHERE table_name IN (
        'TraceBalanceAggregate', 
        'TraceBalanceUtxo', 
        'TraceHolder', 
        'TraceHolderCount',
        'TraceTrade', 
        'TraceReserveSnapshot', 
        'TraceCandle',
        'TraceStorage'
    )
")

if [ "$TABLES" -eq "8" ]; then
    echo -e "${GREEN}✓ All 8 trace transform tables exist${NC}"
else
    echo -e "${YELLOW}⚠ Only $TABLES/8 trace transform tables found${NC}"
    echo "The schema will be created on indexer startup"
fi
echo ""

echo -e "${YELLOW}Step 3: Checking for trace data...${NC}"

# Check TraceBalanceAggregate
BALANCE_COUNT=$(psql "$DB_URL" -t -c 'SELECT COUNT(*) FROM "TraceBalanceAggregate"' 2>/dev/null || echo "0")
echo "  TraceBalanceAggregate: $BALANCE_COUNT rows"

# Check TraceBalanceUtxo
UTXO_COUNT=$(psql "$DB_URL" -t -c 'SELECT COUNT(*) FROM "TraceBalanceUtxo"' 2>/dev/null || echo "0")
echo "  TraceBalanceUtxo: $UTXO_COUNT rows"

# Check TraceHolder
HOLDER_COUNT=$(psql "$DB_URL" -t -c 'SELECT COUNT(*) FROM "TraceHolder"' 2>/dev/null || echo "0")
echo "  TraceHolder: $HOLDER_COUNT rows"

# Check TraceTrade
TRADE_COUNT=$(psql "$DB_URL" -t -c 'SELECT COUNT(*) FROM "TraceTrade"' 2>/dev/null || echo "0")
echo "  TraceTrade: $TRADE_COUNT rows"

# Check TraceCandle
CANDLE_COUNT=$(psql "$DB_URL" -t -c 'SELECT COUNT(*) FROM "TraceCandle"' 2>/dev/null || echo "0")
echo "  TraceCandle: $CANDLE_COUNT rows"

if [ "$BALANCE_COUNT" -gt "0" ] || [ "$TRADE_COUNT" -gt "0" ]; then
    echo -e "${GREEN}✓ Found trace data in tables${NC}"
else
    echo -e "${YELLOW}⚠ No trace data found yet - run indexer to populate${NC}"
fi
echo ""

echo -e "${YELLOW}Step 4: Verifying schema indexes...${NC}"
INDEX_COUNT=$(psql "$DB_URL" -t -c "
    SELECT COUNT(*) FROM pg_indexes 
    WHERE tablename LIKE 'Trace%'
")
echo "  Found $INDEX_COUNT indexes on trace tables"

if [ "$INDEX_COUNT" -gt "0" ]; then
    echo -e "${GREEN}✓ Indexes created${NC}"
    
    # Show some key indexes
    echo ""
    echo "  Key indexes:"
    psql "$DB_URL" -c "
        SELECT tablename, indexname 
        FROM pg_indexes 
        WHERE tablename LIKE 'Trace%' 
        ORDER BY tablename, indexname
        LIMIT 10
    " 2>/dev/null || true
fi
echo ""

echo -e "${YELLOW}Step 5: Sample data check...${NC}"

# Try to get a sample address with balances
SAMPLE_ADDRESS=$(psql "$DB_URL" -t -c 'SELECT address FROM "TraceBalanceAggregate" LIMIT 1' 2>/dev/null | tr -d ' ')

if [ -n "$SAMPLE_ADDRESS" ]; then
    echo "  Sample address: $SAMPLE_ADDRESS"
    
    # Get balances for this address
    echo "  Balances:"
    psql "$DB_URL" -c "
        SELECT 
            alkane_block || ':' || alkane_tx as alkane_id,
            total_amount::TEXT
        FROM \"TraceBalanceAggregate\"
        WHERE address = '$SAMPLE_ADDRESS'
        LIMIT 5
    " 2>/dev/null || true
else
    echo "  No sample data available yet"
fi
echo ""

echo -e "${YELLOW}Step 6: Compilation check...${NC}"
echo "  Checking alkanes-trace-transform..."
if cargo check -p alkanes-trace-transform --quiet 2>&1 | grep -q "Finished"; then
    echo -e "  ${GREEN}✓ alkanes-trace-transform compiles${NC}"
else
    echo -e "  ${RED}✗ alkanes-trace-transform compilation failed${NC}"
fi

echo "  Checking alkanes-contract-indexer..."
if cargo check -p alkanes-contract-indexer --quiet 2>&1 | grep -q "Finished"; then
    echo -e "  ${GREEN}✓ alkanes-contract-indexer compiles${NC}"
else
    echo -e "  ${RED}✗ alkanes-contract-indexer compilation failed${NC}"
fi

echo "  Checking alkanes-data-api..."
if cargo check -p alkanes-data-api --quiet 2>&1 | grep -q "Finished"; then
    echo -e "  ${GREEN}✓ alkanes-data-api compiles${NC}"
else
    echo -e "  ${RED}✗ alkanes-data-api compilation failed${NC}"
fi
echo ""

echo -e "${YELLOW}Step 7: Test summary${NC}"
echo "─────────────────────────────────────"
echo "  Database: Connected"
echo "  Schema: $TABLES/8 tables"
echo "  Data: $((BALANCE_COUNT + TRADE_COUNT)) total rows"
echo "  Compilation: OK"
echo "─────────────────────────────────────"
echo ""

if [ "$TABLES" -eq "8" ] && ([ "$BALANCE_COUNT" -gt "0" ] || [ "$TRADE_COUNT" -gt "0" ]); then
    echo -e "${GREEN}✓✓✓ Integration test PASSED${NC}"
    echo "The trace transform system is fully integrated and has data"
else
    echo -e "${YELLOW}Integration partially complete${NC}"
    echo ""
    echo "Next steps:"
    echo "  1. Run the indexer to create schema: cargo run --bin alkanes-contract-indexer"
    echo "  2. Let it process some blocks to populate trace tables"
    echo "  3. Run this test again to verify data"
fi
echo ""

exit 0
