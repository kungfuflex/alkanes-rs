//! Block and BlockHeader types.
//! Maps to: src/primitives/block.h

use crate::transaction::{
    deserialize_transaction, serialize_transaction_to, Transaction, TransactionRef,
};
use qubitcoin_crypto::hash::hash256;
use qubitcoin_primitives::{BlockHash, Uint256};
use qubitcoin_serialize::{
    read_compact_size, write_compact_size, Decodable, Encodable, Error as SerError,
};
use std::io::{Read, Write};
use std::sync::Arc;

/// Block header (80 bytes).
///
/// Port of Bitcoin Core's CBlockHeader.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct BlockHeader {
    /// Block version (signals soft fork support).
    pub version: i32,
    /// Hash of the previous block header.
    pub prev_blockhash: BlockHash,
    /// Merkle root of the transaction tree.
    pub merkle_root: Uint256,
    /// Timestamp (seconds since Unix epoch).
    pub time: u32,
    /// Compact difficulty target (nBits).
    pub bits: u32,
    /// Nonce for proof-of-work.
    pub nonce: u32,
}

impl BlockHeader {
    /// Create a new block header with all fields set to zero/default.
    pub fn new() -> Self {
        BlockHeader {
            version: 0,
            prev_blockhash: BlockHash::ZERO,
            merkle_root: Uint256::ZERO,
            time: 0,
            bits: 0,
            nonce: 0,
        }
    }

    /// Returns `true` if this header is uninitialized (bits == 0).
    pub fn is_null(&self) -> bool {
        self.bits == 0
    }

    /// Compute the block hash (double SHA256 of the 80-byte header).
    pub fn block_hash(&self) -> BlockHash {
        let data = qubitcoin_serialize::serialize(self).unwrap();
        BlockHash::from_bytes(hash256(&data))
    }

    /// Get the block timestamp as i64.
    pub fn get_block_time(&self) -> i64 {
        self.time as i64
    }
}

impl Default for BlockHeader {
    fn default() -> Self {
        Self::new()
    }
}

impl Encodable for BlockHeader {
    fn encode<W: Write>(&self, w: &mut W) -> Result<usize, SerError> {
        let mut size = self.version.encode(w)?;
        size += self.prev_blockhash.encode(w)?;
        size += Encodable::encode(&self.merkle_root, w)?;
        size += self.time.encode(w)?;
        size += self.bits.encode(w)?;
        size += self.nonce.encode(w)?;
        Ok(size)
    }
}

impl Decodable for BlockHeader {
    fn decode<R: Read>(r: &mut R) -> Result<Self, SerError> {
        Ok(BlockHeader {
            version: i32::decode(r)?,
            prev_blockhash: BlockHash::decode(r)?,
            merkle_root: Uint256::decode(r)?,
            time: u32::decode(r)?,
            bits: u32::decode(r)?,
            nonce: u32::decode(r)?,
        })
    }
}

/// A full block: header + transactions.
///
/// Port of Bitcoin Core's CBlock.
#[derive(Clone, Debug)]
pub struct Block {
    /// Block header.
    pub header: BlockHeader,
    /// Transactions in the block.
    pub vtx: Vec<TransactionRef>,
}

impl Block {
    /// Create a new empty block with a default header and no transactions.
    pub fn new() -> Self {
        Block {
            header: BlockHeader::new(),
            vtx: Vec::new(),
        }
    }

    /// Create a new block with the given header and no transactions.
    pub fn with_header(header: BlockHeader) -> Self {
        Block {
            header,
            vtx: Vec::new(),
        }
    }

    /// Compute the block hash from the header.
    pub fn block_hash(&self) -> BlockHash {
        self.header.block_hash()
    }
}

impl Default for Block {
    fn default() -> Self {
        Self::new()
    }
}

impl Encodable for Block {
    fn encode<W: Write>(&self, w: &mut W) -> Result<usize, SerError> {
        let mut size = self.header.encode(w)?;
        size += write_compact_size(w, self.vtx.len() as u64)?;
        for tx in &self.vtx {
            size += serialize_transaction_to(tx, w, true)?;
        }
        Ok(size)
    }
}

impl Decodable for Block {
    fn decode<R: Read>(r: &mut R) -> Result<Self, SerError> {
        let header = BlockHeader::decode(r)?;
        let tx_count = read_compact_size(r)? as usize;
        let mut vtx = Vec::with_capacity(tx_count);
        for _ in 0..tx_count {
            let tx = deserialize_transaction(r, true)?;
            vtx.push(Arc::new(tx));
        }
        Ok(Block { header, vtx })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_block_header_size() {
        let header = BlockHeader::new();
        let data = qubitcoin_serialize::serialize(&header).unwrap();
        assert_eq!(data.len(), 80);
    }

    #[test]
    fn test_block_header_hash() {
        // Genesis block header
        let mut header = BlockHeader::new();
        header.version = 1;
        header.time = 1231006505;
        header.bits = 0x1d00ffff;
        header.nonce = 2083236893;
        // merkle_root for genesis block
        header.merkle_root =
            Uint256::from_hex("4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b")
                .unwrap();

        let hash = header.block_hash();
        assert_eq!(
            hash.to_hex(),
            "000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f"
        );
    }

    #[test]
    fn test_block_header_roundtrip() {
        let mut header = BlockHeader::new();
        header.version = 0x20000000;
        header.time = 1700000000;
        header.bits = 0x17034567;
        header.nonce = 12345;

        let encoded = qubitcoin_serialize::serialize(&header).unwrap();
        let decoded: BlockHeader = qubitcoin_serialize::deserialize(&encoded).unwrap();
        assert_eq!(header, decoded);
    }

    #[test]
    fn test_empty_block_roundtrip() {
        let block = Block::with_header(BlockHeader::new());
        let encoded = qubitcoin_serialize::serialize(&block).unwrap();
        let decoded: Block = qubitcoin_serialize::deserialize(&encoded).unwrap();
        assert_eq!(block.header, decoded.header);
        assert_eq!(decoded.vtx.len(), 0);
    }
}
