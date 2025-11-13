//! Unified block and transaction types that support both Bitcoin and Zcash
//!
//! This module provides wrapper types that can contain either Bitcoin or Zcash
//! blocks and transactions, avoiding lossy conversions and preserving chain-specific data.

use bitcoin::{BlockHash, OutPoint, ScriptBuf, TxIn, TxOut, Txid};
use std::fmt;

#[cfg(feature = "zcash")]
use crate::zcash::{ZcashBlock as RawZcashBlock, ZcashTransaction as RawZcashTransaction};

/// A unified transaction that can represent either Bitcoin or Zcash transactions
#[derive(Clone, Debug)]
pub enum UnifiedTransaction {
    /// Standard Bitcoin transaction
    Bitcoin(bitcoin::Transaction),
    
    #[cfg(feature = "zcash")]
    /// Zcash transaction (preserves Zcash-specific fields)
    Zcash(RawZcashTransaction),
}

impl UnifiedTransaction {
    /// Create from a Bitcoin transaction
    pub fn from_bitcoin(tx: bitcoin::Transaction) -> Self {
        Self::Bitcoin(tx)
    }
    
    #[cfg(feature = "zcash")]
    /// Create from a Zcash transaction
    pub fn from_zcash(tx: RawZcashTransaction) -> Self {
        Self::Zcash(tx)
    }
    
    /// Compute the transaction ID
    pub fn compute_txid(&self) -> Txid {
        match self {
            Self::Bitcoin(tx) => tx.compute_txid(),
            #[cfg(feature = "zcash")]
            Self::Zcash(tx) => tx.compute_txid(),
        }
    }
    
    /// Get the transaction inputs
    pub fn inputs(&self) -> &[TxIn] {
        match self {
            Self::Bitcoin(tx) => &tx.input,
            #[cfg(feature = "zcash")]
            Self::Zcash(tx) => &tx.inputs,
        }
    }
    
    /// Get the transaction outputs
    pub fn outputs(&self) -> &[TxOut] {
        match self {
            Self::Bitcoin(tx) => &tx.output,
            #[cfg(feature = "zcash")]
            Self::Zcash(tx) => &tx.outputs,
        }
    }
    
    /// Check if this is a coinbase transaction
    pub fn is_coinbase(&self) -> bool {
        match self {
            Self::Bitcoin(tx) => tx.is_coinbase(),
            #[cfg(feature = "zcash")]
            Self::Zcash(tx) => tx.is_coinbase(),
        }
    }
    
    /// Get the transaction version
    pub fn version(&self) -> i32 {
        match self {
            Self::Bitcoin(tx) => tx.version.0,
            #[cfg(feature = "zcash")]
            Self::Zcash(tx) => tx.version,
        }
    }
    
    /// Get the underlying Bitcoin transaction if this is a Bitcoin transaction
    pub fn as_bitcoin(&self) -> Option<&bitcoin::Transaction> {
        match self {
            Self::Bitcoin(tx) => Some(tx),
            #[cfg(feature = "zcash")]
            Self::Zcash(_) => None,
        }
    }
    
    #[cfg(feature = "zcash")]
    /// Get the underlying Zcash transaction if this is a Zcash transaction
    pub fn as_zcash(&self) -> Option<&RawZcashTransaction> {
        match self {
            Self::Bitcoin(_) => None,
            Self::Zcash(tx) => Some(tx),
        }
    }
    
    /// Get a reference to the raw Bitcoin transaction (for Bitcoin-only code paths)
    /// 
    /// # Panics
    /// Panics if called on a Zcash transaction when zcash feature is enabled
    pub fn as_bitcoin_unchecked(&self) -> &bitcoin::Transaction {
        match self {
            Self::Bitcoin(tx) => tx,
            #[cfg(feature = "zcash")]
            Self::Zcash(_) => panic!("Called as_bitcoin_unchecked on Zcash transaction"),
        }
    }
}

/// A unified block that can represent either Bitcoin or Zcash blocks
#[derive(Clone, Debug)]
pub enum UnifiedBlock {
    /// Standard Bitcoin block
    Bitcoin(bitcoin::Block),
    
    #[cfg(feature = "zcash")]
    /// Zcash block (preserves Zcash-specific fields)
    Zcash(RawZcashBlock),
}

impl UnifiedBlock {
    /// Create from a Bitcoin block
    pub fn from_bitcoin(block: bitcoin::Block) -> Self {
        Self::Bitcoin(block)
    }
    
    #[cfg(feature = "zcash")]
    /// Create from a Zcash block
    pub fn from_zcash(block: RawZcashBlock) -> Self {
        Self::Zcash(block)
    }
    
    /// Get the block hash
    pub fn block_hash(&self) -> BlockHash {
        match self {
            Self::Bitcoin(block) => block.block_hash(),
            #[cfg(feature = "zcash")]
            Self::Zcash(block) => block.block_hash(),
        }
    }
    
    /// Get the block header (for Bitcoin blocks only)
    /// 
    /// For Zcash blocks, this returns a Bitcoin-compatible header
    pub fn header(&self) -> bitcoin::block::Header {
        match self {
            Self::Bitcoin(block) => block.header,
            #[cfg(feature = "zcash")]
            Self::Zcash(block) => block.header(),
        }
    }
    
    /// Get the transactions in this block
    pub fn transactions(&self) -> Vec<UnifiedTransaction> {
        match self {
            Self::Bitcoin(block) => {
                block.txdata.iter()
                    .map(|tx| UnifiedTransaction::Bitcoin(tx.clone()))
                    .collect()
            }
            #[cfg(feature = "zcash")]
            Self::Zcash(block) => {
                block.transactions.iter()
                    .map(|tx| UnifiedTransaction::Zcash(tx.clone()))
                    .collect()
            }
        }
    }
    
    /// Get the number of transactions in this block
    pub fn transaction_count(&self) -> usize {
        match self {
            Self::Bitcoin(block) => block.txdata.len(),
            #[cfg(feature = "zcash")]
            Self::Zcash(block) => block.transactions.len(),
        }
    }
    
    /// Get the underlying Bitcoin block if this is a Bitcoin block
    pub fn as_bitcoin(&self) -> Option<&bitcoin::Block> {
        match self {
            Self::Bitcoin(block) => Some(block),
            #[cfg(feature = "zcash")]
            Self::Zcash(_) => None,
        }
    }
    
    #[cfg(feature = "zcash")]
    /// Get the underlying Zcash block if this is a Zcash block
    pub fn as_zcash(&self) -> Option<&RawZcashBlock> {
        match self {
            Self::Bitcoin(_) => None,
            Self::Zcash(block) => Some(block),
        }
    }
}

impl fmt::Display for UnifiedBlock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bitcoin(block) => write!(f, "Bitcoin Block {}", block.block_hash()),
            #[cfg(feature = "zcash")]
            Self::Zcash(block) => write!(f, "Zcash Block {}", block.block_hash()),
        }
    }
}

impl fmt::Display for UnifiedTransaction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bitcoin(tx) => write!(f, "Bitcoin TX {}", tx.compute_txid()),
            #[cfg(feature = "zcash")]
            Self::Zcash(tx) => write!(f, "Zcash TX {}", tx.compute_txid()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_unified_transaction_bitcoin() {
        let tx = bitcoin::Transaction {
            version: bitcoin::transaction::Version::ONE,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: vec![],
            output: vec![],
        };
        
        let unified = UnifiedTransaction::from_bitcoin(tx.clone());
        assert!(unified.as_bitcoin().is_some());
        assert_eq!(unified.inputs().len(), 0);
        assert_eq!(unified.outputs().len(), 0);
    }
}
