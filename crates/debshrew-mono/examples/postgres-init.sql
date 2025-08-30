-- PostgreSQL initialization script for debshrew-mono CDC
-- Creates tables that match the CDC configuration examples

-- Enable required extensions
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- Bitcoin addresses table
CREATE TABLE IF NOT EXISTS addresses (
    address VARCHAR(62) PRIMARY KEY,
    balance BIGINT NOT NULL DEFAULT 0,
    tx_count INTEGER NOT NULL DEFAULT 0,
    first_seen_height INTEGER,
    last_seen_height INTEGER,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Bitcoin transactions table
CREATE TABLE IF NOT EXISTS transactions (
    txid VARCHAR(64) PRIMARY KEY,
    block_height INTEGER NOT NULL,
    block_hash VARCHAR(64) NOT NULL,
    fee BIGINT NOT NULL DEFAULT 0,
    size INTEGER NOT NULL DEFAULT 0,
    vsize INTEGER NOT NULL DEFAULT 0,
    input_count INTEGER NOT NULL DEFAULT 0,
    output_count INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Bitcoin UTXOs table
CREATE TABLE IF NOT EXISTS utxos (
    txid VARCHAR(64) NOT NULL,
    vout INTEGER NOT NULL,
    value BIGINT NOT NULL,
    script_pubkey TEXT NOT NULL,
    address VARCHAR(62),
    spent_height INTEGER,
    spent_txid VARCHAR(64),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (txid, vout)
);

-- Bitcoin blocks table
CREATE TABLE IF NOT EXISTS blocks (
    height INTEGER PRIMARY KEY,
    hash VARCHAR(64) UNIQUE NOT NULL,
    prev_hash VARCHAR(64),
    merkle_root VARCHAR(64) NOT NULL,
    timestamp TIMESTAMP NOT NULL,
    bits VARCHAR(8) NOT NULL,
    nonce BIGINT NOT NULL,
    tx_count INTEGER NOT NULL DEFAULT 0,
    size INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Ordinals inscriptions table
CREATE TABLE IF NOT EXISTS inscriptions (
    txid VARCHAR(64) NOT NULL,
    vout INTEGER NOT NULL,
    inscription_id VARCHAR(128) UNIQUE NOT NULL,
    content_type VARCHAR(255),
    content_length INTEGER,
    content_hash VARCHAR(64),
    inscription_number BIGINT,
    genesis_height INTEGER NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (txid, vout)
);

-- BRC-20 token balances table
CREATE TABLE IF NOT EXISTS brc20_balances (
    token VARCHAR(255) NOT NULL,
    address VARCHAR(62) NOT NULL,
    balance DECIMAL(36, 18) NOT NULL DEFAULT 0,
    available DECIMAL(36, 18) NOT NULL DEFAULT 0,
    transferable DECIMAL(36, 18) NOT NULL DEFAULT 0,
    last_updated_height INTEGER NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (token, address)
);

-- Debug data table for raw key-value pairs
CREATE TABLE IF NOT EXISTS debug_data (
    key TEXT PRIMARY KEY,
    raw_value TEXT NOT NULL,
    raw_length INTEGER NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Create indexes for better query performance
CREATE INDEX IF NOT EXISTS idx_addresses_balance ON addresses(balance DESC);
CREATE INDEX IF NOT EXISTS idx_addresses_first_seen ON addresses(first_seen_height);
CREATE INDEX IF NOT EXISTS idx_addresses_last_seen ON addresses(last_seen_height);

CREATE INDEX IF NOT EXISTS idx_transactions_block_height ON transactions(block_height);
CREATE INDEX IF NOT EXISTS idx_transactions_block_hash ON transactions(block_hash);
CREATE INDEX IF NOT EXISTS idx_transactions_fee ON transactions(fee DESC);

CREATE INDEX IF NOT EXISTS idx_utxos_address ON utxos(address);
CREATE INDEX IF NOT EXISTS idx_utxos_value ON utxos(value DESC);
CREATE INDEX IF NOT EXISTS idx_utxos_spent_height ON utxos(spent_height);

CREATE INDEX IF NOT EXISTS idx_blocks_timestamp ON blocks(timestamp);
CREATE INDEX IF NOT EXISTS idx_blocks_hash ON blocks(hash);

CREATE INDEX IF NOT EXISTS idx_inscriptions_inscription_id ON inscriptions(inscription_id);
CREATE INDEX IF NOT EXISTS idx_inscriptions_genesis_height ON inscriptions(genesis_height);
CREATE INDEX IF NOT EXISTS idx_inscriptions_content_type ON inscriptions(content_type);

CREATE INDEX IF NOT EXISTS idx_brc20_token ON brc20_balances(token);
CREATE INDEX IF NOT EXISTS idx_brc20_address ON brc20_balances(address);
CREATE INDEX IF NOT EXISTS idx_brc20_balance ON brc20_balances(balance DESC);

-- Create triggers to automatically update the updated_at timestamp
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = CURRENT_TIMESTAMP;
    RETURN NEW;
END;
$$ language 'plpgsql';

-- Apply the trigger to tables with updated_at columns
CREATE TRIGGER update_addresses_updated_at BEFORE UPDATE ON addresses
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_utxos_updated_at BEFORE UPDATE ON utxos
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_inscriptions_updated_at BEFORE UPDATE ON inscriptions
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_brc20_balances_updated_at BEFORE UPDATE ON brc20_balances
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_debug_data_updated_at BEFORE UPDATE ON debug_data
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- Create a view for address statistics
CREATE OR REPLACE VIEW address_stats AS
SELECT 
    COUNT(*) as total_addresses,
    SUM(balance) as total_balance,
    AVG(balance) as avg_balance,
    MAX(balance) as max_balance,
    MIN(balance) as min_balance,
    COUNT(CASE WHEN balance > 0 THEN 1 END) as funded_addresses,
    COUNT(CASE WHEN balance = 0 THEN 1 END) as empty_addresses
FROM addresses;

-- Create a view for block statistics
CREATE OR REPLACE VIEW block_stats AS
SELECT 
    COUNT(*) as total_blocks,
    MAX(height) as latest_height,
    MIN(height) as earliest_height,
    SUM(tx_count) as total_transactions,
    AVG(tx_count) as avg_tx_per_block,
    SUM(size) as total_size_bytes,
    AVG(size) as avg_block_size
FROM blocks;

-- Create a view for transaction statistics
CREATE OR REPLACE VIEW transaction_stats AS
SELECT 
    COUNT(*) as total_transactions,
    SUM(fee) as total_fees,
    AVG(fee) as avg_fee,
    MAX(fee) as max_fee,
    MIN(fee) as min_fee,
    AVG(size) as avg_size,
    AVG(vsize) as avg_vsize
FROM transactions;

-- Insert some sample data for testing
INSERT INTO addresses (address, balance, tx_count, first_seen_height, last_seen_height) VALUES
('1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa', 5000000000, 1, 0, 0),
('1BvBMSEYstWetqTFn5Au4m4GFg7xJaNVN2', 2500000000, 5, 100, 500)
ON CONFLICT (address) DO NOTHING;

INSERT INTO blocks (height, hash, prev_hash, merkle_root, timestamp, bits, nonce, tx_count, size) VALUES
(0, '000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f', NULL, '4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b', '2009-01-03 18:15:05', '1d00ffff', 2083236893, 1, 285),
(1, '00000000839a8e6886ab5951d76f411475428afc90947ee320161bbf18eb6048', '000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f', '0e3e2357e806b6cdb1f70b54c3a3a17b6714ee1f0e68bebb44a74b1efd512098', '2009-01-09 02:54:25', '1d00ffff', 2573394689, 1, 215)
ON CONFLICT (height) DO NOTHING;

-- Grant permissions for the application user
-- Note: In production, create a dedicated user with limited permissions
GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA public TO postgres;
GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA public TO postgres;
GRANT SELECT ON ALL TABLES IN SCHEMA public TO postgres;

-- Create a monitoring view for CDC health
CREATE OR REPLACE VIEW cdc_health AS
SELECT 
    'addresses' as table_name,
    COUNT(*) as record_count,
    MAX(updated_at) as last_updated
FROM addresses
UNION ALL
SELECT 
    'transactions' as table_name,
    COUNT(*) as record_count,
    MAX(created_at) as last_updated
FROM transactions
UNION ALL
SELECT 
    'utxos' as table_name,
    COUNT(*) as record_count,
    MAX(updated_at) as last_updated
FROM utxos
UNION ALL
SELECT 
    'blocks' as table_name,
    COUNT(*) as record_count,
    MAX(created_at) as last_updated
FROM blocks
UNION ALL
SELECT 
    'inscriptions' as table_name,
    COUNT(*) as record_count,
    MAX(updated_at) as last_updated
FROM inscriptions
UNION ALL
SELECT 
    'brc20_balances' as table_name,
    COUNT(*) as record_count,
    MAX(updated_at) as last_updated
FROM brc20_balances;

COMMIT;