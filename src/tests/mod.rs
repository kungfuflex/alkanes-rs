//! Test suite for Metashrew runtime with in-memory backend
//!
//! This module provides comprehensive testing for the Metashrew indexer using
//! the memshrew in-memory adapter and the metashrew-minimal WASM module.

use anyhow::Result;
use bitcoin::blockdata::block::Header as BlockHeader;
use bitcoin::hashes::Hash;
use bitcoin::{Block, BlockHash, OutPoint, ScriptBuf, Transaction, TxIn, TxOut, Txid};
use memshrew_runtime::{MemStoreAdapter, MemStoreRuntime};
use metashrew_support::utils;
use std::path::PathBuf;

#[cfg(test)]
pub mod atomic_block_processing_test;
#[cfg(test)]
pub mod atomic_processing_test;
pub mod block_builder;
#[cfg(test)]
pub mod bst_verification_test;
#[cfg(test)]
pub mod comprehensive_e2e_test;
#[cfg(test)]
pub mod historical_view_test;
#[cfg(test)]
pub mod integration_tests;
#[cfg(test)]
pub mod jsonrpc_height_test;
#[cfg(test)]
pub mod jsonrpc_preview_test;
#[cfg(test)]
pub mod production_snapshot_test;
#[cfg(test)]
pub mod reorg_focused_test;
#[cfg(test)]
pub mod runtime_tests;
#[cfg(test)]
pub mod simple_snapshot_test;
#[cfg(test)]
pub mod smt_key_debug_test;
#[cfg(test)]
pub mod snapshot_repo_test;
#[cfg(test)]
pub mod state_root_debug_test;
#[cfg(test)]
pub mod state_root_gap_test;
#[cfg(test)]
pub mod state_root_production_test;
#[cfg(test)]
pub mod stateroot_jsonrpc_test;
#[cfg(test)]
pub mod surface_api_test;
#[cfg(test)]
pub mod snapshot_interval_test;
#[cfg(test)]
pub mod snapshot_interval_fix_test;
#[cfg(test)]
pub mod snapshot_directory_creation_test;
#[cfg(test)]
pub mod snapshot_initialization_fix_test;

/// Test configuration and utilities
pub struct TestConfig {
    pub wasm_path: PathBuf,
}

impl TestConfig {
    pub fn new() -> Self {
        let wasm_path = PathBuf::from(env!("METASHREW_MINIMAL_WASM"));
        Self { wasm_path }
    }

    /// Create a new runtime instance for testing
    pub fn create_runtime(&self) -> Result<MemStoreRuntime> {
        let adapter = MemStoreAdapter::new();
        MemStoreRuntime::load(self.wasm_path.clone(), adapter)
    }
}

/// Test utilities for creating Bitcoin blocks and transactions
pub struct TestUtils;

impl TestUtils {
    /// Create a genesis block for testing
    pub fn create_genesis_block() -> Block {
        let prev_blockhash = BlockHash::all_zeros();
        let merkle_root = Txid::all_zeros();

        let header = BlockHeader {
            version: bitcoin::blockdata::block::Version::ONE,
            prev_blockhash,
            merkle_root: merkle_root.into(),
            time: 1231006505, // Bitcoin genesis timestamp
            bits: bitcoin::CompactTarget::from_consensus(0x1d00ffff),
            nonce: 2083236893,
        };

        // Create a simple coinbase transaction
        let coinbase_tx = Transaction {
            version: bitcoin::transaction::Version::ONE,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: vec![TxIn {
                previous_output: OutPoint::null(),
                script_sig: ScriptBuf::from_hex("04ffff001d0104455468652054696d65732030332f4a616e2f32303039204368616e63656c6c6f72206f6e206272696e6b206f66207365636f6e64206261696c6f757420666f722062616e6b73").unwrap(),
                sequence: bitcoin::Sequence::MAX,
                witness: bitcoin::Witness::new(),
            }],
            output: vec![TxOut {
                value: bitcoin::Amount::from_sat(5000000000), // 50 BTC
                script_pubkey: ScriptBuf::from_hex("4104678afdb0fe5548271967f1a67130b7105cd6a828e03909a67962e0ea1f61deb649f6bc3f4cef38c4f35504e51ec112de5c384df7ba0b8d578a4c702b6bf11d5f").unwrap(),
            }],
        };

        Block {
            header,
            txdata: vec![coinbase_tx],
        }
    }

    /// Create a test block with specified height and previous hash
    pub fn create_test_block(height: u32, prev_hash: BlockHash) -> Block {
        let merkle_root = Txid::all_zeros();

        let header = BlockHeader {
            version: bitcoin::blockdata::block::Version::ONE,
            prev_blockhash: prev_hash,
            merkle_root: merkle_root.into(),
            time: 1231006505 + (height * 600) as u32, // 10 minutes per block
            bits: bitcoin::CompactTarget::from_consensus(0x1d00ffff),
            nonce: 2083236893 + height,
        };

        // Create a simple transaction for this block
        let tx = Transaction {
            version: bitcoin::transaction::Version::ONE,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: vec![TxIn {
                previous_output: OutPoint::null(),
                script_sig: ScriptBuf::from_hex(&format!("03{:06x}", height)).unwrap(),
                sequence: bitcoin::Sequence::MAX,
                witness: bitcoin::Witness::new(),
            }],
            output: vec![TxOut {
                value: bitcoin::Amount::from_sat(5000000000),
                script_pubkey: ScriptBuf::from_hex(
                    "76a914389ffce9cd9ae88dcc0631e88a821ffdbe9bfe26159988ac",
                )
                .unwrap(),
            }],
        };

        Block {
            header,
            txdata: vec![tx],
        }
    }

    /// Serialize a block to bytes for passing to the runtime
    pub fn serialize_block(block: &Block) -> Vec<u8> {
        utils::consensus_encode(block).expect("Failed to encode block")
    }

    /// Create input data for the runtime (height + block bytes)
    pub fn create_runtime_input(height: u32, block: &Block) -> Vec<u8> {
        let mut input = Vec::new();
        input.extend_from_slice(&height.to_le_bytes());
        input.extend_from_slice(&Self::serialize_block(block));
        input
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_creation() {
        let config = TestConfig::new();
        assert!(
            config.wasm_path.exists(),
            "WASM file should exist at: {:?}",
            config.wasm_path
        );
    }

    #[test]
    fn test_genesis_block_creation() {
        let genesis = TestUtils::create_genesis_block();
        assert_eq!(genesis.txdata.len(), 1);
        assert_eq!(genesis.header.prev_blockhash, BlockHash::all_zeros());
    }

    #[test]
    fn test_block_serialization() {
        let genesis = TestUtils::create_genesis_block();
        let serialized = TestUtils::serialize_block(&genesis);
        assert!(!serialized.is_empty());

        // Verify we can deserialize it back using metashrew's consensus_decode
        let mut cursor = std::io::Cursor::new(serialized);
        let deserialized: Block = utils::consensus_decode(&mut cursor).unwrap();
        assert_eq!(
            deserialized.header.prev_blockhash,
            genesis.header.prev_blockhash
        );
    }

    #[test]
    fn test_runtime_input_creation() {
        let genesis = TestUtils::create_genesis_block();
        let input = TestUtils::create_runtime_input(0, &genesis);

        // Should have 4 bytes for height + block data
        assert!(input.len() > 4);

        // First 4 bytes should be height (0)
        let height_bytes = &input[0..4];
        let height = u32::from_le_bytes(height_bytes.try_into().unwrap());
        assert_eq!(height, 0);
    }
}
