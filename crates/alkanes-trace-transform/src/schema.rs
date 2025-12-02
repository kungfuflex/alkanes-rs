/// Optimized Postgres schema for trace transform with proper indexes
/// 
/// This schema is optimized for:
/// 1. Fast lookups by address
/// 2. Fast lookups by alkane ID
/// 3. Fast range scans for trades/candles
/// 4. Efficient joins where needed

#[cfg(feature = "postgres")]
use anyhow::Result;
#[cfg(feature = "postgres")]
use sqlx::PgPool;

/// DDL for trace transform tables with optimized indexes
#[cfg(feature = "postgres")]
pub const TRACE_TRANSFORM_SCHEMA: &str = r#"
-- Address-level aggregate balances
-- Stores cumulative balance per (address, alkane_id)
CREATE TABLE IF NOT EXISTS "TraceBalanceAggregate" (
    address TEXT NOT NULL,
    alkane_block INTEGER NOT NULL,
    alkane_tx BIGINT NOT NULL,
    total_amount NUMERIC NOT NULL DEFAULT 0,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (address, alkane_block, alkane_tx)
);

CREATE INDEX IF NOT EXISTS idx_balance_agg_address ON "TraceBalanceAggregate"(address);
CREATE INDEX IF NOT EXISTS idx_balance_agg_alkane ON "TraceBalanceAggregate"(alkane_block, alkane_tx);
CREATE INDEX IF NOT EXISTS idx_balance_agg_amount ON "TraceBalanceAggregate"(alkane_block, alkane_tx, total_amount DESC);

-- UTXO-level balances
-- Stores balance at each UTXO for precise tracking
CREATE TABLE IF NOT EXISTS "TraceBalanceUtxo" (
    outpoint_txid TEXT NOT NULL,
    outpoint_vout INTEGER NOT NULL,
    address TEXT NOT NULL,
    alkane_block INTEGER NOT NULL,
    alkane_tx BIGINT NOT NULL,
    amount NUMERIC NOT NULL,
    block_height INTEGER NOT NULL,
    spent BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (outpoint_txid, outpoint_vout, alkane_block, alkane_tx)
);

CREATE INDEX IF NOT EXISTS idx_balance_utxo_address ON "TraceBalanceUtxo"(address) WHERE NOT spent;
CREATE INDEX IF NOT EXISTS idx_balance_utxo_alkane ON "TraceBalanceUtxo"(alkane_block, alkane_tx);
CREATE INDEX IF NOT EXISTS idx_balance_utxo_outpoint ON "TraceBalanceUtxo"(outpoint_txid, outpoint_vout);
CREATE INDEX IF NOT EXISTS idx_balance_utxo_spent ON "TraceBalanceUtxo"(spent, block_height);

-- Holder enumeration for efficient holder queries
CREATE TABLE IF NOT EXISTS "TraceHolder" (
    alkane_block INTEGER NOT NULL,
    alkane_tx BIGINT NOT NULL,
    address TEXT NOT NULL,
    total_amount NUMERIC NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (alkane_block, alkane_tx, address)
);

CREATE INDEX IF NOT EXISTS idx_holder_alkane_amount ON "TraceHolder"(alkane_block, alkane_tx, total_amount DESC);
CREATE INDEX IF NOT EXISTS idx_holder_address ON "TraceHolder"(address);

-- Holder count cache for fast pagination
CREATE TABLE IF NOT EXISTS "TraceHolderCount" (
    alkane_block INTEGER NOT NULL,
    alkane_tx BIGINT NOT NULL,
    count BIGINT NOT NULL DEFAULT 0,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (alkane_block, alkane_tx)
);

-- AMM Trade events
CREATE TABLE IF NOT EXISTS "TraceTrade" (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    txid TEXT NOT NULL,
    vout INTEGER NOT NULL,
    pool_block INTEGER NOT NULL,
    pool_tx BIGINT NOT NULL,
    token0_block INTEGER NOT NULL,
    token0_tx BIGINT NOT NULL,
    token1_block INTEGER NOT NULL,
    token1_tx BIGINT NOT NULL,
    amount0_in NUMERIC NOT NULL,
    amount1_in NUMERIC NOT NULL,
    amount0_out NUMERIC NOT NULL,
    amount1_out NUMERIC NOT NULL,
    reserve0_after NUMERIC NOT NULL,
    reserve1_after NUMERIC NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL,
    block_height INTEGER NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(txid, vout, pool_block, pool_tx)
);

CREATE INDEX IF NOT EXISTS idx_trade_pool ON "TraceTrade"(pool_block, pool_tx, timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_trade_timestamp ON "TraceTrade"(timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_trade_block ON "TraceTrade"(block_height);
CREATE INDEX IF NOT EXISTS idx_trade_tokens ON "TraceTrade"(token0_block, token0_tx, token1_block, token1_tx);

-- Reserve snapshots for pool state at points in time
CREATE TABLE IF NOT EXISTS "TraceReserveSnapshot" (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    pool_block INTEGER NOT NULL,
    pool_tx BIGINT NOT NULL,
    reserve0 NUMERIC NOT NULL,
    reserve1 NUMERIC NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL,
    block_height INTEGER NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_reserve_pool_time ON "TraceReserveSnapshot"(pool_block, pool_tx, timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_reserve_timestamp ON "TraceReserveSnapshot"(timestamp DESC);

-- OHLCV Candles for AMM price charts
CREATE TABLE IF NOT EXISTS "TraceCandle" (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    pool_block INTEGER NOT NULL,
    pool_tx BIGINT NOT NULL,
    interval TEXT NOT NULL, -- '1m', '5m', '1h', '1d', etc.
    open_time TIMESTAMPTZ NOT NULL,
    close_time TIMESTAMPTZ NOT NULL,
    open DOUBLE PRECISION NOT NULL,
    high DOUBLE PRECISION NOT NULL,
    low DOUBLE PRECISION NOT NULL,
    close DOUBLE PRECISION NOT NULL,
    volume0 NUMERIC NOT NULL,
    volume1 NUMERIC NOT NULL,
    trade_count INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(pool_block, pool_tx, interval, open_time)
);

CREATE INDEX IF NOT EXISTS idx_candle_pool_interval ON "TraceCandle"(pool_block, pool_tx, interval, open_time DESC);
CREATE INDEX IF NOT EXISTS idx_candle_time ON "TraceCandle"(open_time DESC);

-- Storage key-value pairs per alkane (for contract storage tracking)
CREATE TABLE IF NOT EXISTS "TraceStorage" (
    alkane_block INTEGER NOT NULL,
    alkane_tx BIGINT NOT NULL,
    key TEXT NOT NULL,
    value BYTEA NOT NULL,
    last_txid TEXT NOT NULL,
    last_vout INTEGER NOT NULL,
    block_height INTEGER NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (alkane_block, alkane_tx, key)
);

CREATE INDEX IF NOT EXISTS idx_storage_alkane ON "TraceStorage"(alkane_block, alkane_tx);
CREATE INDEX IF NOT EXISTS idx_storage_block ON "TraceStorage"(block_height);
"#;

/// Apply the trace transform schema
#[cfg(feature = "postgres")]
pub async fn apply_schema(pool: &PgPool) -> Result<()> {
    // Execute schema statements one by one without transaction
    // This allows IF NOT EXISTS to work properly for indexes
    
    // Split by semicolons and execute each statement
    for (idx, statement) in TRACE_TRANSFORM_SCHEMA.split(';').enumerate() {
        let trimmed = statement.trim();
        
        // Skip empty statements
        if trimmed.is_empty() {
            continue;
        }
        
        // Remove comment lines
        let cleaned: String = trimmed
            .lines()
            .filter(|line| !line.trim().starts_with("--"))
            .collect::<Vec<_>>()
            .join("\n");
        
        // Skip if only comments (nothing left after removing comment lines)
        if !cleaned.trim().is_empty() {
            // Log what we're about to execute
            let preview = cleaned.lines().next().unwrap_or("").trim();
            eprintln!("Executing statement {}: {}...", idx, &preview[..preview.len().min(60)]);
            
            match sqlx::query(&cleaned).execute(pool).await {
                Ok(_) => eprintln!("  ✓ Success"),
                Err(e) => {
                    eprintln!("  ✗ Failed: {}", e);
                    eprintln!("  Statement was:\n{}", cleaned);
                    return Err(e.into());
                }
            }
        }
    }
    
    // Apply alkane registry schema
    for (idx, statement) in ALKANE_REGISTRY_SCHEMA.split(';').enumerate() {
        let trimmed = statement.trim();
        
        if trimmed.is_empty() {
            continue;
        }
        
        let cleaned: String = trimmed
            .lines()
            .filter(|line| !line.trim().starts_with("--"))
            .collect::<Vec<_>>()
            .join("\n");
        
        if !cleaned.trim().is_empty() {
            let preview = cleaned.lines().next().unwrap_or("").trim();
            eprintln!("Executing alkane registry statement {}: {}...", idx, &preview[..preview.len().min(60)]);
            
            match sqlx::query(&cleaned).execute(pool).await {
                Ok(_) => eprintln!("  ✓ Success"),
                Err(e) => {
                    eprintln!("  ✗ Failed: {}", e);
                    return Err(e.into());
                }
            }
        }
    }
    
    Ok(())
}

/// Drop all trace transform tables (for testing)
#[cfg(feature = "postgres")]
pub async fn drop_schema(pool: &PgPool) -> Result<()> {
    let drop_sql = r#"
        DROP TABLE IF EXISTS "TraceAlkaneBalance" CASCADE;
        DROP TABLE IF EXISTS "TraceAlkane" CASCADE;
        DROP TABLE IF EXISTS "TraceCandle" CASCADE;
        DROP TABLE IF EXISTS "TraceReserveSnapshot" CASCADE;
        DROP TABLE IF EXISTS "TraceTrade" CASCADE;
        DROP TABLE IF EXISTS "TraceHolderCount" CASCADE;
        DROP TABLE IF EXISTS "TraceHolder" CASCADE;
        DROP TABLE IF EXISTS "TraceBalanceUtxo" CASCADE;
        DROP TABLE IF EXISTS "TraceBalanceAggregate" CASCADE;
        DROP TABLE IF EXISTS "TraceStorage" CASCADE;
    "#;
    
    sqlx::query(drop_sql).execute(pool).await?;
    
    Ok(())
}

#[cfg(all(test, feature = "postgres"))]
mod tests {
    use super::*;
    
    #[tokio::test]
    #[ignore] // Requires database connection
    async fn test_apply_schema() {
        // This test requires a real Postgres connection
        // Run with: cargo test --features postgres -- --ignored
        
        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://localhost/alkanes_test".to_string());
        
        let pool = PgPool::connect(&database_url).await.unwrap();
        
        // Apply schema
        apply_schema(&pool).await.unwrap();
        
        // Verify tables exist
        let tables: Vec<(String,)> = sqlx::query_as(
            r#"SELECT table_name FROM information_schema.tables 
               WHERE table_schema = 'public' 
               AND table_name LIKE 'Trace%'
               ORDER BY table_name"#
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        
        assert!(tables.len() >= 7, "Should have created trace transform tables");
        
        // Clean up
        drop_schema(&pool).await.unwrap();
    }
}

/// Schema for tracking all created alkanes
#[cfg(feature = "postgres")]
pub const ALKANE_REGISTRY_SCHEMA: &str = r#"
-- Registry of all created alkanes
CREATE TABLE IF NOT EXISTS "TraceAlkane" (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    alkane_block INTEGER NOT NULL,
    alkane_tx BIGINT NOT NULL,
    created_at_block INTEGER NOT NULL,
    created_at_tx TEXT NOT NULL,
    created_at_height INTEGER,
    created_at_timestamp TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(alkane_block, alkane_tx)
);

CREATE INDEX IF NOT EXISTS idx_trace_alkane_id 
    ON "TraceAlkane"(alkane_block, alkane_tx);
    
CREATE INDEX IF NOT EXISTS idx_trace_alkane_created 
    ON "TraceAlkane"(created_at_height DESC);

-- Address -> Alkane balances from traces
CREATE TABLE IF NOT EXISTS "TraceAlkaneBalance" (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    address TEXT NOT NULL,
    alkane_block INTEGER NOT NULL,
    alkane_tx BIGINT NOT NULL,
    balance NUMERIC NOT NULL DEFAULT 0,
    last_updated_block INTEGER NOT NULL,
    last_updated_tx TEXT NOT NULL,
    last_updated_timestamp TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(address, alkane_block, alkane_tx)
);

CREATE INDEX IF NOT EXISTS idx_trace_alkane_balance_address 
    ON "TraceAlkaneBalance"(address);
    
CREATE INDEX IF NOT EXISTS idx_trace_alkane_balance_alkane 
    ON "TraceAlkaneBalance"(alkane_block, alkane_tx);
    
CREATE INDEX IF NOT EXISTS idx_trace_alkane_balance_amount
    ON "TraceAlkaneBalance"(alkane_block, alkane_tx, balance DESC) WHERE balance > 0
"#;
