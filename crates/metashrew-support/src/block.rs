//! # Extended Bitcoin Block Parsing with AuxPoW Support
//!
//! This module provides comprehensive support for parsing Bitcoin blocks with extended
//! functionality including Auxiliary Proof of Work (AuxPoW) support. AuxPoW enables
//! merged mining, allowing cryptocurrencies to be mined simultaneously with Bitcoin
//! or other parent blockchains.
//!
//! ## Core Concepts
//!
//! ### Auxiliary Proof of Work (AuxPoW)
//! AuxPoW is a mechanism that allows a blockchain to accept work done on a parent
//! blockchain as valid proof of work. This enables:
//! - **Merged mining**: Mining multiple cryptocurrencies simultaneously
//! - **Increased security**: Leveraging the hash power of larger networks
//! - **Resource efficiency**: Reducing energy waste in cryptocurrency mining
//!
//! ### Version Encoding
//! The block version field encodes multiple pieces of information:
//! - **Base version**: Core protocol version
//! - **AuxPoW flag**: Indicates presence of auxiliary proof of work
//! - **Chain ID**: Identifies the specific blockchain in merged mining
//! - **Proof of Stake flag**: Indicates hybrid PoW/PoS consensus
//!
//! ## Usage Examples
//!
//! ```rust,ignore
//! use metashrew_support::block::*;
//! use std::io::Cursor;
//!
//! // Parse an AuxPoW block from raw bytes
//! let block_data = vec![/* raw block bytes */];
//! let mut cursor = Cursor::new(block_data);
//! let auxpow_block = AuxpowBlock::parse(&mut cursor).unwrap();
//!
//! // Convert to standard Bitcoin block format
//! let bitcoin_block = auxpow_block.to_consensus();
//!
//! // Check version flags
//! let version = AuxpowVersion::new(0x100); // AuxPoW enabled
//! assert!(version.is_auxpow());
//! assert_eq!(version.base_version(), 0);
//! ```
//!
//! ## Integration with Metashrew
//!
//! This module enables Metashrew to index blockchains that use AuxPoW:
//! - **Dogecoin**: Popular cryptocurrency using AuxPoW
//! - **Namecoin**: Decentralized naming system with AuxPoW
//! - **Other merged-mined chains**: Various cryptocurrencies leveraging Bitcoin's security

use crate::utils::{consensus_decode, consume_exact, consume_sized_int, consume_varint};
use anyhow::Result;
use bitcoin::blockdata::block::Header;
use bitcoin::blockdata::block::{BlockHash, TxMerkleNode, Version};
use bitcoin::hashes::Hash;
use bitcoin::pow::CompactTarget;
use bitcoin::{Block, Transaction};

/// Version flag indicating AuxPoW is enabled in the block version.
pub const VERSION_AUXPOW: u32 = 0x100;

/// Version flag indicating Proof of Stake consensus is active.
pub const VERSION_POS_START: u32 = 0x200;

/// Version multiplier for encoding chain ID in the version field.
pub const VERSION_CHAIN_START: u32 = 0x10000;

/// Default chain ID used for AuxPoW validation.
pub const VERSION_CHAIN_ID: u32 = 20;

/// Extended version information for AuxPoW-enabled blocks.
///
/// This wrapper around the standard version field provides methods to extract
/// AuxPoW-specific information encoded in the version bits. The version field
/// encodes multiple pieces of information using bit flags and arithmetic encoding.
///
/// # Version Encoding Format
/// - Bits 0-7: Base protocol version
/// - Bit 8: AuxPoW flag (VERSION_AUXPOW)
/// - Bit 9: Proof of Stake flag (VERSION_POS_START)  
/// - Bits 16+: Chain ID (version / VERSION_CHAIN_START)
#[derive(Default, Clone, Debug)]
pub struct AuxpowVersion(u32);

impl AuxpowVersion {
    /// Create a new AuxPoW version from a raw version value.
    ///
    /// # Parameters
    /// - `v`: Raw 32-bit version value from block header
    ///
    /// # Returns
    /// New AuxpowVersion instance wrapping the raw value
    pub fn new(v: u32) -> Self {
        Self(v)
    }

    /// Extract the raw version value.
    ///
    /// # Returns
    /// Raw 32-bit version value
    pub fn unwrap(&self) -> u32 {
        self.0
    }

    /// Extract the base protocol version (without AuxPoW flags).
    ///
    /// This removes the AuxPoW and chain ID encoding to reveal the underlying
    /// protocol version that would be used in a non-AuxPoW block.
    ///
    /// # Returns
    /// Base protocol version as 32-bit unsigned integer
    pub fn base_version(&self) -> u32 {
        self.unwrap() % VERSION_AUXPOW
    }

    /// Extract the chain ID from the version field.
    ///
    /// The chain ID identifies which specific blockchain this block belongs to
    /// in a merged mining setup. Different cryptocurrencies use different chain IDs
    /// to prevent cross-chain replay attacks.
    ///
    /// # Returns
    /// Chain ID as 32-bit unsigned integer
    pub fn chain_id(&self) -> u32 {
        self.unwrap() / VERSION_CHAIN_START
    }

    /// Check if this block uses Auxiliary Proof of Work.
    ///
    /// # Returns
    /// `true` if AuxPoW is enabled, `false` for standard Bitcoin blocks
    pub fn is_auxpow(&self) -> bool {
        self.unwrap() & VERSION_AUXPOW != 0
    }

    /// Check if this block uses Proof of Stake consensus.
    ///
    /// Some blockchains combine Proof of Work with Proof of Stake for hybrid
    /// consensus mechanisms.
    ///
    /// # Returns
    /// `true` if PoS is active, `false` for pure PoW consensus
    pub fn is_proof_of_stake(&self) -> bool {
        self.unwrap() & VERSION_POS_START != 0
    }

    /// Check if this is a legacy (pre-AuxPoW) block.
    ///
    /// Legacy blocks use version values below the AuxPoW threshold and follow
    /// standard Bitcoin block format without auxiliary proof of work.
    ///
    /// # Returns
    /// `true` if this is a legacy block, `false` if AuxPoW-capable
    pub fn is_legacy(&self) -> bool {
        self.unwrap() < VERSION_AUXPOW
    }
}

/// Auxiliary Proof of Work data structure.
///
/// Contains the complete auxiliary proof of work that demonstrates this block
/// was mined as part of a merged mining operation. The AuxPoW proves that
/// work was done on a parent blockchain that also validates this block.
///
/// # Structure
/// The AuxPoW contains:
/// - **Coinbase transaction**: Parent chain transaction including this block's hash
/// - **Block hash**: Hash of this block embedded in the parent coinbase
/// - **Merkle branches**: Proof paths linking coinbase to parent block
/// - **Parent block**: Header of the parent blockchain block
#[derive(Clone, Debug)]
pub struct Auxpow {
    /// Coinbase transaction from the parent blockchain containing this block's hash.
    pub coinbase_txn: Transaction,
    
    /// Hash of this block that was embedded in the parent coinbase transaction.
    pub block_hash: BlockHash,
    
    /// Merkle branch proving coinbase transaction inclusion in parent block.
    pub coinbase_branch: AuxpowMerkleBranch,
    
    /// Merkle branch proving this blockchain's inclusion in parent blockchain.
    pub blockchain_branch: AuxpowMerkleBranch,
    
    /// Header of the parent blockchain block that includes the proof of work.
    pub parent_block: AuxpowHeader,
}

impl Auxpow {
    /// Parse AuxPoW data from a binary cursor.
    ///
    /// This function deserializes the complete auxiliary proof of work structure
    /// from binary data, including all merkle branches and the parent block header.
    ///
    /// # Parameters
    /// - `cursor`: Mutable cursor positioned at AuxPoW data
    ///
    /// # Returns
    /// Parsed Auxpow structure containing all proof components
    ///
    /// # Errors
    /// Returns error if:
    /// - Binary data is malformed or truncated
    /// - Consensus decoding fails for embedded transactions
    /// - Merkle branch parsing encounters invalid data
    pub fn parse(cursor: &mut std::io::Cursor<Vec<u8>>) -> Result<Auxpow> {
        let coinbase_txn: Transaction = consensus_decode::<Transaction>(cursor)?;
        let block_hash: BlockHash =
            BlockHash::from_byte_array(to_ref(&consume_exact(cursor, 0x20)?).try_into().unwrap());
        let coinbase_branch: AuxpowMerkleBranch = AuxpowMerkleBranch::parse(cursor)?;
        let blockchain_branch: AuxpowMerkleBranch = AuxpowMerkleBranch::parse(cursor)?;
        let parent_block = AuxpowHeader::parse_without_auxpow(cursor)?;
        Ok(Auxpow {
            coinbase_txn,
            block_hash,
            coinbase_branch,
            blockchain_branch,
            parent_block,
        })
    }
}

/// Extended block header with AuxPoW support.
///
/// This structure extends the standard Bitcoin block header to include optional
/// auxiliary proof of work data. When AuxPoW is not used, this behaves identically
/// to a standard Bitcoin header.
///
/// # Compatibility
/// The header maintains full compatibility with standard Bitcoin headers while
/// adding the capability to include auxiliary proof of work when needed.
#[derive(Clone, Debug)]
pub struct AuxpowHeader {
    /// Extended version information with AuxPoW flags.
    pub version: AuxpowVersion,
    
    /// Hash of the previous block in the blockchain.
    pub prev_blockhash: BlockHash,
    
    /// Merkle root of all transactions in this block.
    pub merkle_root: TxMerkleNode,
    
    /// Block timestamp as Unix epoch time.
    pub time: u32,
    
    /// Difficulty target in compact format.
    pub bits: CompactTarget,
    
    /// Nonce value used for proof of work.
    pub nonce: u32,
    
    /// Optional auxiliary proof of work data (present when version.is_auxpow() is true).
    pub auxpow: Option<Box<Auxpow>>,
}

/// Conversion from AuxpowVersion to standard Bitcoin Version.
///
/// This conversion extracts the base version information and creates a standard
/// Bitcoin version value compatible with the bitcoin crate.
impl Into<Version> for AuxpowVersion {
    fn into(self) -> Version {
        Version::from_consensus(self.0 as i32)
    }
}

/// Conversion from AuxpowHeader to standard Bitcoin Header.
///
/// This conversion creates a standard Bitcoin header by extracting the core
/// header fields and discarding AuxPoW-specific information. This enables
/// compatibility with standard Bitcoin processing tools.
impl Into<Header> for AuxpowHeader {
    fn into(self) -> Header {
        Header {
            version: self.version.into(),
            prev_blockhash: self.prev_blockhash,
            merkle_root: self.merkle_root,
            time: self.time,
            bits: self.bits,
            nonce: self.nonce,
        }
    }
}

/// Complete AuxPoW-enabled block including header and transactions.
///
/// This structure represents a complete block that may include auxiliary proof
/// of work. It contains both the extended header and all transaction data.
#[derive(Clone, Debug)]
pub struct AuxpowBlock {
    /// Extended block header with optional AuxPoW data.
    pub header: AuxpowHeader,
    
    /// Vector of all transactions included in this block.
    pub txdata: Vec<Transaction>,
}

/// Merkle branch for AuxPoW proof verification.
///
/// This structure contains a merkle branch (proof path) used to verify inclusion
/// of specific data in a merkle tree. AuxPoW uses merkle branches to prove:
/// 1. Coinbase transaction inclusion in parent block
/// 2. Blockchain inclusion in merged mining setup
///
/// # Merkle Proof Format
/// The branch contains:
/// - **Length**: Number of hash values in the proof path
/// - **Hashes**: Array of hash values forming the proof path
/// - **Side mask**: Bit mask indicating left/right positions in tree
#[derive(Clone, Debug)]
pub struct AuxpowMerkleBranch {
    /// Number of hash values in the merkle branch.
    pub branch_length: u64,
    
    /// Array of hash values forming the merkle proof path.
    pub branch_hash: Vec<BlockHash>,
    
    /// Bit mask indicating whether each hash is on left or right side of tree.
    pub branch_side_mask: i32,
}

impl AuxpowMerkleBranch {
    /// Parse a merkle branch from binary data.
    ///
    /// This function deserializes a merkle branch structure used in AuxPoW
    /// verification. The branch contains the hash values and positioning
    /// information needed to verify merkle tree inclusion.
    ///
    /// # Parameters
    /// - `cursor`: Mutable cursor positioned at merkle branch data
    ///
    /// # Returns
    /// Parsed AuxpowMerkleBranch structure
    ///
    /// # Format
    /// - Varint: branch_length (number of hashes)
    /// - Array: branch_length × 32-byte hash values
    /// - 4 bytes: branch_side_mask (little-endian)
    pub fn parse(cursor: &mut std::io::Cursor<Vec<u8>>) -> Result<AuxpowMerkleBranch> {
        let branch_length = consume_varint(cursor)?;
        let mut branch_hash: Vec<BlockHash> = vec![];
        for _ in 0..branch_length {
            branch_hash.push(BlockHash::from_byte_array(
                to_ref(&consume_exact(cursor, 0x20)?).try_into()?,
            ));
        }
        let branch_side_mask = consume_sized_int::<u32>(cursor)? as i32;
        Ok(AuxpowMerkleBranch {
            branch_length,
            branch_hash,
            branch_side_mask,
        })
    }
}

impl AuxpowBlock {
    /// Convert AuxPoW block to standard Bitcoin block format.
    ///
    /// This conversion creates a standard Bitcoin block by extracting the core
    /// block data and discarding AuxPoW-specific information. The resulting
    /// block is compatible with standard Bitcoin processing tools.
    ///
    /// # Returns
    /// Standard Bitcoin Block with AuxPoW data removed
    pub fn to_consensus(&self) -> Block {
        Block {
            header: self.header.clone().into(),
            txdata: self.txdata.clone(),
        }
    }

    /// Parse a complete AuxPoW block from binary data.
    ///
    /// This function deserializes a complete block including header, optional
    /// AuxPoW data, and all transactions. It handles both legacy Bitcoin blocks
    /// and AuxPoW-enabled blocks transparently.
    ///
    /// # Parameters
    /// - `cursor`: Mutable cursor positioned at block data
    ///
    /// # Returns
    /// Parsed AuxpowBlock structure containing header and transactions
    ///
    /// # Format
    /// - Header: Extended header with optional AuxPoW
    /// - Varint: Transaction count
    /// - Array: Transaction data (consensus encoded)
    pub fn parse(cursor: &mut std::io::Cursor<Vec<u8>>) -> Result<AuxpowBlock> {
        let header = AuxpowHeader::parse(cursor)?;
        let mut txdata: Vec<Transaction> = vec![];
        let len = consume_varint(cursor)?;
        for _ in 0..len {
            let tx = consensus_decode::<Transaction>(cursor)?;
            txdata.push(tx);
        }
        Ok(AuxpowBlock { header, txdata })
    }
}

/// Helper function to convert Vec<u8> reference to byte slice.
///
/// This utility function provides a clean conversion from vector reference
/// to byte slice for use in hash construction and other operations.
fn to_ref(v: &Vec<u8>) -> &[u8] {
    v.as_ref()
}

impl AuxpowHeader {
    /// Parse block header without AuxPoW data.
    ///
    /// This function parses the core block header fields without attempting
    /// to read auxiliary proof of work data. It's used when parsing parent
    /// block headers within AuxPoW structures.
    ///
    /// # Parameters
    /// - `cursor`: Mutable cursor positioned at header data
    ///
    /// # Returns
    /// AuxpowHeader with core fields populated and auxpow set to None
    pub fn parse_without_auxpow(cursor: &mut std::io::Cursor<Vec<u8>>) -> Result<AuxpowHeader> {
        let version = AuxpowVersion(consume_sized_int::<u32>(cursor)?.into());
        let prev_blockhash: BlockHash =
            BlockHash::from_byte_array(to_ref(&consume_exact(cursor, 0x20)?).try_into().unwrap());
        let merkle_root: TxMerkleNode = consensus_decode::<TxMerkleNode>(cursor)?;
        let time: u32 = consume_sized_int::<u32>(cursor)?;
        let bits: CompactTarget = CompactTarget::from_consensus(consume_sized_int::<u32>(cursor)?);
        let nonce: u32 = consume_sized_int::<u32>(cursor)?;
        Ok(AuxpowHeader {
            version,
            prev_blockhash,
            merkle_root,
            time,
            bits,
            nonce,
            auxpow: None,
        })
    }

    /// Parse complete block header with optional AuxPoW data.
    ///
    /// This function parses a complete block header, automatically detecting
    /// whether AuxPoW data is present based on the version flags. If AuxPoW
    /// is enabled, it parses the auxiliary proof of work structure.
    ///
    /// # Parameters
    /// - `cursor`: Mutable cursor positioned at header data
    ///
    /// # Returns
    /// Complete AuxpowHeader with optional AuxPoW data
    ///
    /// # Behavior
    /// - If version.is_auxpow() is false: Returns header with auxpow = None
    /// - If version.is_auxpow() is true: Parses and includes AuxPoW data
    pub fn parse(cursor: &mut std::io::Cursor<Vec<u8>>) -> Result<AuxpowHeader> {
        let mut result = Self::parse_without_auxpow(cursor)?;
        result.auxpow = if result.version.is_auxpow() {
            Some(Box::new(Auxpow::parse(cursor)?))
        } else {
            None
        };
        Ok(result)
    }
}
