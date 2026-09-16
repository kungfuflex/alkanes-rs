//! Zcash-specific block and transaction types
//!
//! These types preserve Zcash-specific fields while providing
//! compatibility with Bitcoin-style indexing for transparent components.

use crate::block_traits::{BlockLike, TransactionLike};
use anyhow::Result;
use bitcoin::hashes::Hash as HashTrait;
use bitcoin::{BlockHash, ScriptBuf, TxIn, TxOut, Txid};
use std::io::Read;

/// A Zcash transaction that preserves all Zcash-specific fields
#[derive(Clone, Debug)]
pub struct ZcashTransaction {
    /// Transaction version (can be negative for overwinter flag)
    pub version: i32,
    
    /// Version group ID (Overwinter+)
    pub version_group_id: Option<u32>,
    
    /// Transparent inputs (standard Bitcoin format)
    pub inputs: Vec<TxIn>,
    
    /// Transparent outputs (standard Bitcoin format)
    pub outputs: Vec<TxOut>,
    
    /// Lock time
    pub lock_time: u32,
    
    /// Expiry height (Overwinter+)
    pub expiry_height: Option<u32>,
    
    /// Value balance for shielded components (Sapling+)
    pub value_balance: Option<i64>,
    
    /// Number of shielded spends (for indexing purposes)
    pub shielded_spend_count: u64,
    
    /// Number of shielded outputs (for indexing purposes)
    pub shielded_output_count: u64,
    
    /// Number of JoinSplits (for indexing purposes)
    pub joinsplit_count: u64,
    
    /// Cached transaction ID
    cached_txid: Option<Txid>,
}

impl ZcashTransaction {
    /// Parse a Zcash transaction from a cursor
    pub fn parse(cursor: &mut std::io::Cursor<Vec<u8>>) -> Result<Self> {
        use bitcoin::consensus::Decodable;
        
        // Read version (4 bytes)
        let mut version_bytes = [0u8; 4];
        cursor.read_exact(&mut version_bytes)?;
        let version = i32::from_le_bytes(version_bytes);
        
        // Check if this is Overwinter or later
        let is_overwinter = version >= 3 || (version < 0 && version as u32 >= 0x80000003);
        
        // Read version group ID if Overwinter+
        let version_group_id = if is_overwinter {
            let mut vg_bytes = [0u8; 4];
            cursor.read_exact(&mut vg_bytes)?;
            Some(u32::from_le_bytes(vg_bytes))
        } else {
            None
        };
        
        // Read inputs (standard Bitcoin format)
        let input_count = read_varint(cursor)?;
        let mut inputs = Vec::with_capacity(input_count as usize);
        for _ in 0..input_count {
            let input = TxIn::consensus_decode(cursor)?;
            inputs.push(input);
        }
        
        // Read outputs (standard Bitcoin format)
        let output_count = read_varint(cursor)?;
        let mut outputs = Vec::with_capacity(output_count as usize);
        for _ in 0..output_count {
            let output = TxOut::consensus_decode(cursor)?;
            outputs.push(output);
        }
        
        // Read lock time
        let mut lock_time_bytes = [0u8; 4];
        cursor.read_exact(&mut lock_time_bytes)?;
        let lock_time = u32::from_le_bytes(lock_time_bytes);
        
        // Read expiry height if Overwinter+
        let expiry_height = if is_overwinter {
            let mut eh_bytes = [0u8; 4];
            cursor.read_exact(&mut eh_bytes)?;
            Some(u32::from_le_bytes(eh_bytes))
        } else {
            None
        };
        
        // For Sapling (v4+), handle shielded components
        let (value_balance, shielded_spend_count, shielded_output_count) = if version >= 4 {
            // Read value balance
            let mut vb_bytes = [0u8; 8];
            cursor.read_exact(&mut vb_bytes)?;
            let value_balance = i64::from_le_bytes(vb_bytes);
            
            // Read and skip shielded spends
            let spend_count = read_varint(cursor)?;
            for _ in 0..spend_count {
                let mut _spend_data = [0u8; 384];
                cursor.read_exact(&mut _spend_data)?;
            }
            
            // Read and skip shielded outputs
            let output_count = read_varint(cursor)?;
            for _ in 0..output_count {
                let mut _output_data = [0u8; 948];
                cursor.read_exact(&mut _output_data)?;
            }
            
            (Some(value_balance), spend_count, output_count)
        } else {
            (None, 0, 0)
        };
        
        // For v2+, handle JoinSplits
        let joinsplit_count = if version >= 2 {
            let js_count = read_varint(cursor)?;
            
            if js_count > 0 {
                // Skip JoinSplit data
                let joinsplit_size = if version >= 4 { 1698 } else { 1802 };
                for _ in 0..js_count {
                    let mut _js_data = vec![0u8; joinsplit_size];
                    cursor.read_exact(&mut _js_data)?;
                }
                
                // Skip JoinSplit pubkey (32 bytes)
                let mut _js_pubkey = [0u8; 32];
                cursor.read_exact(&mut _js_pubkey)?;
                
                // Skip JoinSplit sig (64 bytes)
                let mut _js_sig = [0u8; 64];
                cursor.read_exact(&mut _js_sig)?;
            }
            
            js_count
        } else {
            0
        };
        
        // For Sapling (v4+), read binding signature if shielded components exist
        if version >= 4 && (shielded_spend_count > 0 || shielded_output_count > 0) {
            let mut _binding_sig = [0u8; 64];
            cursor.read_exact(&mut _binding_sig)?;
        }
        
        Ok(Self {
            version,
            version_group_id,
            inputs,
            outputs,
            lock_time,
            expiry_height,
            value_balance,
            shielded_spend_count,
            shielded_output_count,
            joinsplit_count,
            cached_txid: None,
        })
    }
    
    /// Compute the transaction ID
    /// 
    /// For Zcash, we compute the txid from the transparent parts only
    /// (similar to how Bitcoin does it)
    pub fn compute_txid(&self) -> Txid {
        if let Some(txid) = self.cached_txid {
            return txid;
        }
        
        // For now, compute a Bitcoin-compatible txid from the transparent parts
        // This is a simplified implementation - real Zcash txid calculation
        // includes all fields including shielded components
        use bitcoin::hashes::{sha256d, Hash};
        use std::io::Write;
        
        let mut engine = sha256d::Hash::engine();
        
        // Version
        engine.write_all(&self.version.to_le_bytes()).unwrap();
        
        // Inputs
        engine.write_all(&encode_varint(self.inputs.len() as u64)).unwrap();
        for input in &self.inputs {
            use bitcoin::consensus::Encodable;
            input.consensus_encode(&mut engine).unwrap();
        }
        
        // Outputs  
        engine.write_all(&encode_varint(self.outputs.len() as u64)).unwrap();
        for output in &self.outputs {
            use bitcoin::consensus::Encodable;
            output.consensus_encode(&mut engine).unwrap();
        }
        
        // Lock time
        engine.write_all(&self.lock_time.to_le_bytes()).unwrap();
        
        let hash = sha256d::Hash::from_engine(engine);
        Txid::from_raw_hash(bitcoin::hashes::sha256d::Hash::from_byte_array(hash.to_byte_array()))
    }
    
    /// Check if this is a coinbase transaction
    pub fn is_coinbase(&self) -> bool {
        self.inputs.len() == 1 && self.inputs[0].previous_output.is_null()
    }
}

/// A Zcash block that preserves all Zcash-specific header fields
#[derive(Clone, Debug)]
pub struct ZcashBlock {
    /// Block version
    pub version: i32,
    
    /// Previous block hash
    pub prev_blockhash: BlockHash,
    
    /// Merkle root
    pub merkle_root: bitcoin::TxMerkleNode,
    
    /// Reserved / Final Sapling root (32 bytes)
    pub reserved: [u8; 32],
    
    /// Block timestamp
    pub time: u32,
    
    /// Difficulty bits
    pub bits: u32,
    
    /// Nonce (32 bytes for Zcash, not 4 like Bitcoin)
    pub nonce: [u8; 32],
    
    /// Equihash solution size
    pub solution_size: usize,
    
    /// Equihash solution
    pub solution: Vec<u8>,
    
    /// Transactions
    pub transactions: Vec<ZcashTransaction>,
}

impl ZcashBlock {
    /// Parse a Zcash block from raw bytes
    pub fn parse(cursor: &mut std::io::Cursor<Vec<u8>>) -> Result<Self> {
        // Read version
        let mut version_bytes = [0u8; 4];
        cursor.read_exact(&mut version_bytes)?;
        let version = i32::from_le_bytes(version_bytes);
        
        // Read previous block hash
        let mut prev_blockhash = [0u8; 32];
        cursor.read_exact(&mut prev_blockhash)?;
        
        // Read merkle root
        let mut merkle_root = [0u8; 32];
        cursor.read_exact(&mut merkle_root)?;
        
        // Read reserved field
        let mut reserved = [0u8; 32];
        cursor.read_exact(&mut reserved)?;
        
        // Read time
        let mut time_bytes = [0u8; 4];
        cursor.read_exact(&mut time_bytes)?;
        let time = u32::from_le_bytes(time_bytes);
        
        // Read bits
        let mut bits_bytes = [0u8; 4];
        cursor.read_exact(&mut bits_bytes)?;
        let bits = u32::from_le_bytes(bits_bytes);
        
        // Read nonce (32 bytes)
        let mut nonce = [0u8; 32];
        cursor.read_exact(&mut nonce)?;
        
        // Read solution size
        let solution_size = read_varint(cursor)? as usize;
        
        // Read solution
        let mut solution = vec![0u8; solution_size];
        cursor.read_exact(&mut solution)?;
        
        // Read transaction count
        let tx_count = read_varint(cursor)? as usize;
        
        // Parse transactions
        let mut transactions = Vec::with_capacity(tx_count);
        for _ in 0..tx_count {
            let tx = ZcashTransaction::parse(cursor)?;
            transactions.push(tx);
        }
        
        Ok(Self {
            version,
            prev_blockhash: BlockHash::from_byte_array(prev_blockhash),
            merkle_root: bitcoin::TxMerkleNode::from_byte_array(merkle_root),
            reserved,
            time,
            bits,
            nonce,
            solution_size,
            solution,
            transactions,
        })
    }
    
    /// Get the block hash
    pub fn block_hash(&self) -> BlockHash {
        use bitcoin::hashes::{sha256d, Hash};
        use std::io::Write;
        
        let mut engine = sha256d::Hash::engine();
        
        // Write header fields
        engine.write_all(&self.version.to_le_bytes()).unwrap();
        engine.write_all(self.prev_blockhash.as_byte_array()).unwrap();
        engine.write_all(self.merkle_root.as_byte_array()).unwrap();
        engine.write_all(&self.reserved).unwrap();
        engine.write_all(&self.time.to_le_bytes()).unwrap();
        engine.write_all(&self.bits.to_le_bytes()).unwrap();
        engine.write_all(&self.nonce).unwrap();
        engine.write_all(&encode_varint(self.solution_size as u64)).unwrap();
        engine.write_all(&self.solution).unwrap();
        
        let hash = sha256d::Hash::from_engine(engine);
        BlockHash::from_raw_hash(bitcoin::hashes::sha256d::Hash::from_byte_array(hash.to_byte_array()))
    }
    
    /// Get a Bitcoin-compatible header (for compatibility)
    pub fn header(&self) -> bitcoin::block::Header {
        // Convert 32-byte nonce to 4-byte for Bitcoin compatibility
        let nonce_u32 = u32::from_le_bytes([
            self.nonce[0],
            self.nonce[1],
            self.nonce[2],
            self.nonce[3],
        ]);
        
        bitcoin::block::Header {
            version: bitcoin::block::Version::from_consensus(self.version),
            prev_blockhash: self.prev_blockhash,
            merkle_root: self.merkle_root,
            time: self.time,
            bits: bitcoin::CompactTarget::from_consensus(self.bits),
            nonce: nonce_u32,
        }
    }
}

/// Read a Bitcoin-style varint
fn read_varint(cursor: &mut std::io::Cursor<Vec<u8>>) -> Result<u64> {
    let mut first = [0u8; 1];
    cursor.read_exact(&mut first)?;
    
    let value = match first[0] {
        0..=0xfc => first[0] as u64,
        0xfd => {
            let mut bytes = [0u8; 2];
            cursor.read_exact(&mut bytes)?;
            u16::from_le_bytes(bytes) as u64
        }
        0xfe => {
            let mut bytes = [0u8; 4];
            cursor.read_exact(&mut bytes)?;
            u32::from_le_bytes(bytes) as u64
        }
        0xff => {
            let mut bytes = [0u8; 8];
            cursor.read_exact(&mut bytes)?;
            u64::from_le_bytes(bytes)
        }
    };
    
    Ok(value)
}

/// Encode a value as a Bitcoin-style varint
fn encode_varint(value: u64) -> Vec<u8> {
    if value < 0xfd {
        vec![value as u8]
    } else if value <= 0xffff {
        let mut result = vec![0xfd];
        result.extend_from_slice(&(value as u16).to_le_bytes());
        result
    } else if value <= 0xffffffff {
        let mut result = vec![0xfe];
        result.extend_from_slice(&(value as u32).to_le_bytes());
        result
    } else {
        let mut result = vec![0xff];
        result.extend_from_slice(&value.to_le_bytes());
        result
    }
}

// Implement BlockLike and TransactionLike traits for Zcash types
impl TransactionLike for ZcashTransaction {
    fn txid(&self) -> Txid {
        self.compute_txid()
    }
    
    fn inputs(&self) -> &[TxIn] {
        &self.inputs
    }
    
    fn outputs(&self) -> &[TxOut] {
        &self.outputs
    }
    
    fn is_coinbase(&self) -> bool {
        ZcashTransaction::is_coinbase(self)
    }
    
    fn version(&self) -> i32 {
        self.version
    }
}

impl BlockLike for ZcashBlock {
    type Transaction = ZcashTransaction;
    
    fn block_hash(&self) -> BlockHash {
        ZcashBlock::block_hash(self)
    }
    
    fn transactions(&self) -> &[Self::Transaction] {
        &self.transactions
    }
    
    fn header(&self) -> bitcoin::block::Header {
        ZcashBlock::header(self)
    }
}
