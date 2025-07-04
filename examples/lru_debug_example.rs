//! Example demonstrating LRU cache debugging functionality in alkanes-rs
//! 
//! This example shows how the lru-debug feature integrates with block processing
//! to provide detailed cache analysis and key prefix insights.
//! 
//! To run with LRU debugging enabled:
//! ```bash
//! cargo run --example lru_debug_example --features lru-debug
//! ```

use alkanes::logging::{self, BlockStats, CacheStats};
use bitcoin::{Block, Transaction, TxOut, ScriptBuf};
use bitcoin::blockdata::block::Header;
use bitcoin::blockdata::transaction::{TxIn, OutPoint};
use bitcoin::hashes::Hash;
use bitcoin::pow::CompactTarget;
use std::str::FromStr;

fn main() {
    println!("🧪 ALKANES-RS LRU DEBUG EXAMPLE");
    println!("================================");
    println!();

    // Initialize block statistics
    logging::init_block_stats();

    // Enable LRU debug mode (only available with lru-debug feature)
    #[cfg(feature = "lru-debug")]
    {
        println!("🔍 Enabling LRU debug mode...");
        logging::enable_lru_debug_mode();
        println!("✅ LRU debug mode enabled");
        println!();
    }

    #[cfg(not(feature = "lru-debug"))]
    {
        println!("⚠️  LRU debug mode not available (compile with --features lru-debug)");
        println!();
    }

    // Create a mock block for demonstration
    let mock_block = create_mock_block();
    let height = 850000;

    println!("📦 Processing mock block {} with {} transactions", height, mock_block.txdata.len());
    
    // Record some mock statistics
    logging::record_transactions(mock_block.txdata.len() as u32);
    logging::record_outpoints(mock_block.txdata.iter().map(|tx| tx.output.len() as u32).sum());
    logging::record_protostone_run();
    logging::record_protostone_run();
    logging::record_protostone_with_cellpack();
    logging::record_fuel_consumed(150000);
    logging::record_excess_fuel_unused(25000);

    // Simulate some cache activity by getting cache stats
    // (In real usage, this would happen during actual block processing)
    let cache_stats = logging::get_cache_stats();
    logging::update_cache_stats(cache_stats);

    println!("✅ Block processing simulation complete");
    println!();

    // Generate the block summary with LRU debug information
    println!("📊 GENERATING BLOCK SUMMARY WITH LRU DEBUG INFO:");
    println!();
    logging::log_block_summary_with_size(&mock_block, height, 1024 * 1024); // 1MB mock size

    // Show additional debug information if available
    #[cfg(feature = "lru-debug")]
    {
        println!();
        println!("🔍 DETAILED LRU CACHE ANALYSIS:");
        println!("================================");
        
        let debug_report = logging::generate_lru_debug_report();
        println!("{}", debug_report);
        
        println!();
        println!("💡 TIP: The cache analysis above shows:");
        println!("   • Key prefix patterns and their access frequency");
        println!("   • Memory usage breakdown by prefix");
        println!("   • Cache hit/miss patterns for different data types");
        println!("   • Readable key formats instead of raw hex");
    }

    #[cfg(not(feature = "lru-debug"))]
    {
        println!();
        println!("💡 To see detailed LRU cache analysis, recompile with:");
        println!("   cargo run --example lru_debug_example --features lru-debug");
    }

    println!();
    println!("🎯 Example complete!");
}

/// Create a mock block for demonstration purposes
fn create_mock_block() -> Block {
    // Create mock transactions
    let mut transactions = Vec::new();
    
    // Coinbase transaction
    let coinbase_tx = Transaction {
        version: bitcoin::transaction::Version::TWO,
        lock_time: bitcoin::locktime::absolute::LockTime::ZERO,
        input: vec![TxIn {
            previous_output: OutPoint::null(),
            script_sig: ScriptBuf::from_hex("03d0490c").unwrap(),
            sequence: bitcoin::Sequence::MAX,
            witness: bitcoin::Witness::new(),
        }],
        output: vec![TxOut {
            value: bitcoin::Amount::from_sat(625000000),
            script_pubkey: ScriptBuf::from_hex("76a914389ffce9cd9ae88dcc0631e88a821ffdbe9bfe2615").unwrap(),
        }],
    };
    transactions.push(coinbase_tx);

    // Add a few mock regular transactions
    for i in 1..=3 {
        let tx = Transaction {
            version: bitcoin::transaction::Version::TWO,
            lock_time: bitcoin::locktime::absolute::LockTime::ZERO,
            input: vec![TxIn {
                previous_output: OutPoint {
                    txid: bitcoin::Txid::from_str(&format!("{}000000000000000000000000000000000000000000000000000000000000000", i)).unwrap(),
                    vout: 0,
                },
                script_sig: ScriptBuf::from_hex("483045022100c2c4a6e553e81c7240d1c64a5b2c1b9c8d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b102207f8e9d0c1b2a3948576869a8b7c6d5e4f3a2b1c0d9e8f7a6b5c4d3e2f1a0b9c8d701").unwrap(),
                sequence: bitcoin::Sequence::MAX,
                witness: bitcoin::Witness::new(),
            }],
            output: vec![
                TxOut {
                    value: bitcoin::Amount::from_sat(100000000),
                    script_pubkey: ScriptBuf::from_hex("76a914389ffce9cd9ae88dcc0631e88a821ffdbe9bfe2615").unwrap(),
                },
                TxOut {
                    value: bitcoin::Amount::from_sat(50000000),
                    script_pubkey: ScriptBuf::from_hex("a914b7fcce0648a8b8b5b8c8d8e8f8a8b8c8d8e8f8a8b887").unwrap(),
                },
            ],
        };
        transactions.push(tx);
    }

    // Create mock header
    let header = Header {
        version: bitcoin::block::Version::TWO,
        prev_blockhash: bitcoin::BlockHash::from_str("0000000000000000000308a8f1097a8f1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8").unwrap(),
        merkle_root: bitcoin::TxMerkleNode::from_str("4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b").unwrap(),
        time: 1640995200, // Mock timestamp
        bits: CompactTarget::from_consensus(0x1d00ffff),
        nonce: 2083236893,
    };

    Block {
        header,
        txdata: transactions,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_block_creation() {
        let block = create_mock_block();
        assert_eq!(block.txdata.len(), 4); // 1 coinbase + 3 regular transactions
        assert!(!block.txdata.is_empty());
    }

    #[test]
    fn test_logging_functions() {
        logging::init_block_stats();
        logging::record_transaction();
        logging::record_outpoints(5);
        
        let stats = logging::get_block_stats();
        assert!(stats.is_some());
    }

    #[cfg(feature = "lru-debug")]
    #[test]
    fn test_lru_debug_functions() {
        logging::enable_lru_debug_mode();
        let _report = logging::generate_lru_debug_report();
        logging::disable_lru_debug_mode();
    }
}