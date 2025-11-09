//! Traits for abstracting over different blockchain types (Bitcoin, Zcash, etc.)
//!
//! These traits define the minimal interface needed for indexing, allowing
//! the same code to work with Bitcoin and Zcash blocks/transactions.

use bitcoin::{BlockHash, OutPoint, ScriptBuf, TxIn, TxOut, Txid};

/// A transaction-like type that provides access to inputs, outputs, and txid
pub trait TransactionLike {
    /// Compute the transaction ID
    fn txid(&self) -> Txid;
    
    /// Get the transaction inputs
    fn inputs(&self) -> &[TxIn];
    
    /// Get the transaction outputs
    fn outputs(&self) -> &[TxOut];
    
    /// Check if this is a coinbase transaction
    fn is_coinbase(&self) -> bool {
        self.inputs().len() == 1 && self.inputs()[0].previous_output.is_null()
    }
    
    /// Get the transaction version
    fn version(&self) -> i32;
    
    /// Convert to a Bitcoin transaction for compatibility with libraries that need it
    /// This creates a new transaction with only the transparent parts
    fn to_bitcoin_tx(&self) -> bitcoin::Transaction {
        bitcoin::Transaction {
            version: bitcoin::transaction::Version(self.version()),
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: self.inputs().to_vec(),
            output: self.outputs().to_vec(),
        }
    }
}

/// A block-like type that provides access to header and transactions
pub trait BlockLike {
    /// The transaction type for this block
    type Transaction: TransactionLike;
    
    /// Get the block hash
    fn block_hash(&self) -> BlockHash;
    
    /// Get the transactions in this block
    fn transactions(&self) -> &[Self::Transaction];
    
    /// Get the block header (Bitcoin-compatible)
    fn header(&self) -> bitcoin::block::Header;
}

// Implement for Bitcoin types
impl TransactionLike for bitcoin::Transaction {
    fn txid(&self) -> Txid {
        self.compute_txid()
    }
    
    fn inputs(&self) -> &[TxIn] {
        &self.input
    }
    
    fn outputs(&self) -> &[TxOut] {
        &self.output
    }
    
    fn is_coinbase(&self) -> bool {
        bitcoin::Transaction::is_coinbase(self)
    }
    
    fn version(&self) -> i32 {
        self.version.0
    }
    
    fn to_bitcoin_tx(&self) -> bitcoin::Transaction {
        self.clone()
    }
}

impl BlockLike for bitcoin::Block {
    type Transaction = bitcoin::Transaction;
    
    fn block_hash(&self) -> BlockHash {
        bitcoin::Block::block_hash(self)
    }
    
    fn transactions(&self) -> &[Self::Transaction] {
        &self.txdata
    }
    
    fn header(&self) -> bitcoin::block::Header {
        self.header
    }
}
