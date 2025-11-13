//! Block builder utilities for creating test Bitcoin blocks
//!
//! This module provides a fluent API for building Bitcoin blocks for testing purposes.

use bitcoin::blockdata::block::Header as BlockHeader;
use bitcoin::hashes::Hash;
use bitcoin::{Block, BlockHash, OutPoint, ScriptBuf, Transaction, TxIn, TxOut, Txid};
use hex;
use std::collections::VecDeque;

/// A builder for creating test Bitcoin blocks
pub struct BlockBuilder {
    height: u32,
    prev_hash: BlockHash,
    timestamp: u32,
    transactions: Vec<Transaction>,
    nonce: u32,
}

impl BlockBuilder {
    /// Create a new block builder
    pub fn new() -> Self {
        Self {
            height: 0,
            prev_hash: BlockHash::all_zeros(),
            timestamp: 1231006505, // Bitcoin genesis timestamp
            transactions: Vec::new(),
            nonce: 2083236893,
        }
    }

    /// Set the block height
    pub fn height(mut self, height: u32) -> Self {
        self.height = height;
        self.timestamp = 1231006505 + (height * 600) as u32; // 10 minutes per block
        self.nonce = 2083236893 + height;
        self
    }

    /// Set the previous block hash
    pub fn prev_hash(mut self, prev_hash: BlockHash) -> Self {
        self.prev_hash = prev_hash;
        self
    }

    /// Set the timestamp
    pub fn timestamp(mut self, timestamp: u32) -> Self {
        self.timestamp = timestamp;
        self
    }

    /// Set the nonce
    pub fn nonce(mut self, nonce: u32) -> Self {
        self.nonce = nonce;
        self
    }

    /// Add a transaction to the block
    pub fn add_transaction(mut self, tx: Transaction) -> Self {
        self.transactions.push(tx);
        self
    }

    /// Add a coinbase transaction
    pub fn add_coinbase(self, value_sats: u64, script_sig_data: Option<&str>) -> Self {
        let script_sig = match script_sig_data {
            Some(data) => ScriptBuf::from_hex(data).unwrap_or_else(|_| {
                ScriptBuf::from_hex(&format!("03{:06x}", self.height)).unwrap()
            }),
            None => ScriptBuf::from_hex(&format!("03{:06x}", self.height)).unwrap(),
        };

        let coinbase_tx = Transaction {
            version: bitcoin::transaction::Version::ONE,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: vec![TxIn {
                previous_output: OutPoint::null(),
                script_sig,
                sequence: bitcoin::Sequence::MAX,
                witness: bitcoin::Witness::new(),
            }],
            output: vec![TxOut {
                value: bitcoin::Amount::from_sat(value_sats),
                script_pubkey: ScriptBuf::from_hex(
                    "76a914389ffce9cd9ae88dcc0631e88a821ffdbe9bfe26159988ac",
                )
                .unwrap(),
            }],
        };

        self.add_transaction(coinbase_tx)
    }

    /// Build the block
    pub fn build(self) -> Block {
        let mut transactions = self.transactions;

        // If no transactions were added, add a default coinbase
        if transactions.is_empty() {
            let coinbase_tx = Transaction {
                version: bitcoin::transaction::Version::ONE,
                lock_time: bitcoin::absolute::LockTime::ZERO,
                input: vec![TxIn {
                    previous_output: OutPoint::null(),
                    script_sig: ScriptBuf::from_hex(&format!("03{:06x}", self.height)).unwrap(),
                    sequence: bitcoin::Sequence::MAX,
                    witness: bitcoin::Witness::new(),
                }],
                output: vec![TxOut {
                    value: bitcoin::Amount::from_sat(5000000000), // 50 BTC
                    script_pubkey: ScriptBuf::from_hex(
                        "76a914389ffce9cd9ae88dcc0631e88a821ffdbe9bfe26159988ac",
                    )
                    .unwrap(),
                }],
            };
            transactions.push(coinbase_tx);
        }

        // Calculate merkle root
        let txids: Vec<Txid> = transactions.iter().map(|tx| tx.compute_txid()).collect();
        let merkle_root = bitcoin::merkle_tree::calculate_root(txids.into_iter())
            .unwrap_or_else(|| Txid::all_zeros());

        let header = BlockHeader {
            version: bitcoin::blockdata::block::Version::ONE,
            prev_blockhash: self.prev_hash,
            merkle_root: merkle_root.into(),
            time: self.timestamp,
            bits: bitcoin::CompactTarget::from_consensus(0x1d00ffff),
            nonce: self.nonce,
        };

        Block {
            header,
            txdata: transactions,
        }
    }
}

/// A chain builder for creating sequences of test blocks
#[derive(Clone)]
pub struct ChainBuilder {
    blocks: VecDeque<Block>,
    current_height: u32,
    current_hash: BlockHash,
    salt: u32,
}

impl ChainBuilder {
    /// Create a new chain builder starting with genesis
    pub fn new() -> Self {
        let genesis = BlockBuilder::new()
            .height(0)
            .add_coinbase(5000000000, Some("04ffff001d0104455468652054696d65732030332f4a616e2f32303039204368616e63656c6c6f72206f6e206272696e6b206f66207365636f6e64206261696c6f757420666f722062616e6b73"))
            .build();

        let genesis_hash = genesis.block_hash();
        let mut blocks = VecDeque::new();
        blocks.push_back(genesis);

        Self {
            blocks,
            current_height: 0,
            current_hash: genesis_hash,
            salt: 0,
        }
    }

    /// Add a block to the chain
    pub fn add_block(mut self) -> Self {
        let next_height = self.current_height + 1;
        let block = BlockBuilder::new()
            .height(next_height)
            .prev_hash(self.current_hash)
            .timestamp(1231006505 + (next_height * 600) + self.salt)
            .nonce(2083236893 + next_height + self.salt)
            .build();

        self.current_hash = block.block_hash();
        self.current_height = next_height;
        self.blocks.push_back(block);

        self
    }

    /// Add multiple blocks to the chain
    pub fn add_blocks(mut self, count: u32) -> Self {
        for _ in 0..count {
            self = self.add_block();
        }
        self
    }

    /// Add a custom block to the chain
    pub fn add_custom_block<F>(mut self, builder_fn: F) -> Self
    where
        F: FnOnce(BlockBuilder) -> BlockBuilder,
    {
        let next_height = self.current_height + 1;
        let block = builder_fn(
            BlockBuilder::new()
                .height(next_height)
                .prev_hash(self.current_hash),
        )
        .build();

        self.current_hash = block.block_hash();
        self.current_height = next_height;
        self.blocks.push_back(block);

        self
    }

    /// Get all blocks in the chain
    pub fn blocks(self) -> Vec<Block> {
        self.blocks.into()
    }

    /// Get the current tip hash
    pub fn tip_hash(&self) -> BlockHash {
        self.current_hash
    }

    /// Get the current height
    pub fn height(&self) -> u32 {
        self.current_height
    }

    /// Get a specific block by height
    pub fn get_block(&self, height: u32) -> Option<&Block> {
        self.blocks.get(height as usize)
    }

    /// Fork the chain at a specific height
    pub fn fork(mut self, height: u32) -> Self {
        self.blocks.truncate(height as usize + 1);
        self.current_height = height;
        self.current_hash = self.blocks[height as usize].block_hash();
        self
    }

    /// Set a salt to differentiate chains
    pub fn with_salt(mut self, salt: u32) -> Self {
        self.salt = salt;
        self
    }
}

/// Simple function to create a test block with given parameters
pub fn create_test_block(height: u32, prev_hash: BlockHash, extra_data: &[u8]) -> Block {
    let extra_hex = hex::encode(extra_data);
    BlockBuilder::new()
        .height(height)
        .prev_hash(prev_hash)
        .add_coinbase(5000000000, Some(&extra_hex))
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_block_builder() {
        let block = BlockBuilder::new()
            .height(1)
            .prev_hash(BlockHash::all_zeros())
            .build();

        assert_eq!(block.txdata.len(), 1); // Should have coinbase
        assert_eq!(block.header.prev_blockhash, BlockHash::all_zeros());
    }

    #[test]
    fn test_block_builder_with_custom_coinbase() {
        let block = BlockBuilder::new()
            .height(1)
            .add_coinbase(1000000000, Some("deadbeef"))
            .build();

        assert_eq!(block.txdata.len(), 1);
        assert_eq!(block.txdata[0].output[0].value.to_sat(), 1000000000);
    }

    #[test]
    fn test_chain_builder() {
        let chain = ChainBuilder::new().add_blocks(5).blocks();

        assert_eq!(chain.len(), 6); // Genesis + 5 blocks

        // Verify chain integrity
        for i in 1..chain.len() {
            assert_eq!(chain[i].header.prev_blockhash, chain[i - 1].block_hash());
        }
    }

    #[test]
    fn test_chain_builder_custom_block() {
        let chain = ChainBuilder::new()
            .add_custom_block(|builder| builder.add_coinbase(2000000000, Some("custom")))
            .blocks();

        assert_eq!(chain.len(), 2); // Genesis + custom block
        assert_eq!(chain[1].txdata[0].output[0].value.to_sat(), 2000000000);
    }
}
