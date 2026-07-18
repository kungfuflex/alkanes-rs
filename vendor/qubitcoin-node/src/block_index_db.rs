//! Block index database: persists `BlockIndex` entries to a key-value store.
//!
//! Maps to: `src/txdb.h` / `src/txdb.cpp` (`CBlockTreeDB`) in Bitcoin Core.
//!
//! The database stores block index entries keyed by `[b'b' | block_hash(32)]`
//! and the last known block file number under key `[b'F']`.
//!
//! Because `BlockIndex` uses arena-style parent/skip indices that are
//! only meaningful at runtime, loading produces `BlockIndexRecord` values
//! instead. The caller is responsible for rebuilding the in-memory arena
//! from those records.

use qubitcoin_common::chain::BlockIndex;
use qubitcoin_primitives::{
    arith_to_uint256, uint256_to_arith, ArithUint256, BlockHash, Uint256,
};
use qubitcoin_storage::traits::{DbBatch, DbIterator};
use qubitcoin_storage::{Database, DbWrapper};
use std::io::{self, Read, Write};

// ---------------------------------------------------------------------------
// Key constants
// ---------------------------------------------------------------------------

/// Prefix byte for block-index entries: `b'b'`.
const KEY_BLOCK_INDEX: u8 = b'b';

/// Key for the last-known block file number: `b'F'`.
const KEY_LAST_BLOCK_FILE: &[u8] = &[b'F'];

// ---------------------------------------------------------------------------
// BlockIndexRecord
// ---------------------------------------------------------------------------

/// A flat record containing all persistable fields of a `BlockIndex`.
///
/// Returned by [`BlockIndexDB::load_all`] so that callers can rebuild the
/// arena-based in-memory block index without depending on runtime-only fields
/// like `prev` (arena index), `skip`, `sequence_id`, or `time_max`.
#[derive(Clone, Debug)]
pub struct BlockIndexRecord {
    /// The double-SHA256 hash of the block header.
    pub block_hash: BlockHash,
    /// Block header version field.
    pub version: i32,
    /// Hash of the previous block header.
    pub prev_blockhash: BlockHash,
    /// Merkle root of the block's transactions.
    pub merkle_root: Uint256,
    /// Block timestamp (Unix epoch seconds).
    pub time: u32,
    /// Compact representation of the proof-of-work target (`nBits`).
    pub bits: u32,
    /// Nonce used to satisfy the proof-of-work.
    pub nonce: u32,
    /// Height of this block in the chain (0 = genesis).
    pub height: i32,
    /// Raw `BlockStatus` bitfield (validity and data-availability flags).
    pub status_bits: u32,
    /// Block file number where this block's data is stored.
    pub file: i32,
    /// Byte offset of block data within the block file.
    pub data_pos: u32,
    /// Byte offset of undo data within the undo (rev) file.
    pub undo_pos: u32,
    /// Number of transactions in this block.
    pub tx_count: u32,
    /// Total number of transactions in the chain up to and including this block.
    pub chain_tx_count: u64,
    /// Cumulative proof-of-work as a 32-byte big-endian [`ArithUint256`].
    pub chain_work_bytes: [u8; 32],
}

impl BlockIndexRecord {
    /// Reconstruct the [`ArithUint256`] chain-work value from the stored bytes.
    pub fn chain_work(&self) -> ArithUint256 {
        uint256_to_arith(&Uint256::from_bytes(self.chain_work_bytes))
    }
}

// ---------------------------------------------------------------------------
// Serialization helpers (little-endian, matching Bitcoin Core wire format)
// ---------------------------------------------------------------------------

/// Serialize a `BlockIndex` to bytes.
///
/// Field order:
///   height (i32) | version (i32) | prev_blockhash (32) | merkle_root (32)
///   | time (u32) | bits (u32) | nonce (u32) | status_bits (u32)
///   | file (i32) | data_pos (u32) | undo_pos (u32) | tx_count (u32)
///   | chain_tx_count (u64) | chain_work (32)
fn serialize_block_index(entry: &BlockIndex) -> Vec<u8> {
    // 4+4+32+32+4+4+4+4+4+4+4+4+8+32 = 144 bytes
    let mut buf = Vec::with_capacity(144);
    let _ = buf.write_all(&entry.height.to_le_bytes());
    let _ = buf.write_all(&entry.version.to_le_bytes());
    let _ = buf.write_all(entry.prev_blockhash.data());
    let _ = buf.write_all(entry.merkle_root.data());
    let _ = buf.write_all(&entry.time.to_le_bytes());
    let _ = buf.write_all(&entry.bits.to_le_bytes());
    let _ = buf.write_all(&entry.nonce.to_le_bytes());
    let _ = buf.write_all(&entry.status.bits().to_le_bytes());
    let _ = buf.write_all(&entry.file.to_le_bytes());
    let _ = buf.write_all(&entry.data_pos.to_le_bytes());
    let _ = buf.write_all(&entry.undo_pos.to_le_bytes());
    let _ = buf.write_all(&entry.tx_count.to_le_bytes());
    let _ = buf.write_all(&entry.chain_tx_count.to_le_bytes());
    let cw_uint = arith_to_uint256(&entry.chain_work);
    let _ = buf.write_all(cw_uint.data());
    buf
}

/// Deserialize a `BlockIndexRecord` from bytes.
///
/// `block_hash` is provided separately because it is encoded in the key, not
/// the value.
fn deserialize_block_index_record(
    block_hash: BlockHash,
    data: &[u8],
) -> io::Result<BlockIndexRecord> {
    if data.len() < 144 {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            format!(
                "block index record too short: expected 144 bytes, got {}",
                data.len()
            ),
        ));
    }
    let mut cursor = io::Cursor::new(data);

    let mut buf4 = [0u8; 4];
    let mut buf8 = [0u8; 8];
    let mut buf32 = [0u8; 32];

    cursor.read_exact(&mut buf4)?;
    let height = i32::from_le_bytes(buf4);

    cursor.read_exact(&mut buf4)?;
    let version = i32::from_le_bytes(buf4);

    cursor.read_exact(&mut buf32)?;
    let prev_blockhash = BlockHash::from_bytes(buf32);

    cursor.read_exact(&mut buf32)?;
    let merkle_root = Uint256::from_bytes(buf32);

    cursor.read_exact(&mut buf4)?;
    let time = u32::from_le_bytes(buf4);

    cursor.read_exact(&mut buf4)?;
    let bits = u32::from_le_bytes(buf4);

    cursor.read_exact(&mut buf4)?;
    let nonce = u32::from_le_bytes(buf4);

    cursor.read_exact(&mut buf4)?;
    let status_bits = u32::from_le_bytes(buf4);

    cursor.read_exact(&mut buf4)?;
    let file = i32::from_le_bytes(buf4);

    cursor.read_exact(&mut buf4)?;
    let data_pos = u32::from_le_bytes(buf4);

    cursor.read_exact(&mut buf4)?;
    let undo_pos = u32::from_le_bytes(buf4);

    cursor.read_exact(&mut buf4)?;
    let tx_count = u32::from_le_bytes(buf4);

    cursor.read_exact(&mut buf8)?;
    let chain_tx_count = u64::from_le_bytes(buf8);

    cursor.read_exact(&mut buf32)?;
    let chain_work_bytes = buf32;

    Ok(BlockIndexRecord {
        block_hash,
        version,
        prev_blockhash,
        merkle_root,
        time,
        bits,
        nonce,
        height,
        status_bits,
        file,
        data_pos,
        undo_pos,
        tx_count,
        chain_tx_count,
        chain_work_bytes,
    })
}

/// Build the database key for a block index entry: `[b'b' | hash(32)]`.
fn block_index_key(hash: &BlockHash) -> Vec<u8> {
    let mut key = Vec::with_capacity(33);
    key.push(KEY_BLOCK_INDEX);
    key.extend_from_slice(hash.data());
    key
}

// ---------------------------------------------------------------------------
// BlockIndexDB
// ---------------------------------------------------------------------------

/// Persistent block-index database.
///
/// Wraps a [`DbWrapper`] and provides typed read/write operations for
/// `BlockIndex` entries and block-file bookkeeping.
///
/// Maps to `CBlockTreeDB` in Bitcoin Core.
pub struct BlockIndexDB<D: Database> {
    db: DbWrapper<D>,
}

impl<D: Database> BlockIndexDB<D> {
    /// Create a new `BlockIndexDB` wrapping the given database.
    ///
    /// If `obfuscate` is true, values are XOR-obfuscated on disk (matching
    /// Bitcoin Core behaviour).
    pub fn new(db: D, obfuscate: bool) -> Self {
        let wrapper = if obfuscate {
            DbWrapper::new(db, true)
        } else {
            DbWrapper::new_unobfuscated(db)
        };
        BlockIndexDB { db: wrapper }
    }

    /// Create without obfuscation (convenience for testing).
    pub fn new_unobfuscated(db: D) -> Self {
        BlockIndexDB {
            db: DbWrapper::new_unobfuscated(db),
        }
    }

    /// Get a reference to the underlying [`DbWrapper`].
    pub fn db(&self) -> &DbWrapper<D> {
        &self.db
    }

    // -- Block index operations ---------------------------------------------

    /// Write a single block index entry. Returns `true` on success.
    pub fn write_block_index(&self, entry: &BlockIndex) -> bool {
        let key = block_index_key(&entry.block_hash);
        let value = serialize_block_index(entry);
        let db = self.db.inner();
        let mut batch = db.new_batch();
        batch.put(&key, &value);
        db.write_batch(batch, false).is_ok()
    }

    /// Write multiple block index entries in a single atomic batch.
    /// Returns `true` on success.
    pub fn write_block_indices(&self, entries: &[&BlockIndex]) -> bool {
        let db = self.db.inner();
        let mut batch = db.new_batch();
        for entry in entries {
            let key = block_index_key(&entry.block_hash);
            let value = serialize_block_index(entry);
            batch.put(&key, &value);
        }
        db.write_batch(batch, false).is_ok()
    }

    /// Load all block index entries from the database.
    ///
    /// Returns `BlockIndexRecord` values (not full `BlockIndex`) because
    /// arena-based parent/skip links must be rebuilt by the caller.
    ///
    /// Entries whose key does not start with the block-index prefix byte or
    /// whose value fails to deserialize are silently skipped.
    pub fn load_all(&self) -> Vec<BlockIndexRecord> {
        let db = self.db.inner();
        let mut iter = db.new_iterator();
        let mut records = Vec::new();

        // Seek to the first key starting with the block-index prefix.
        iter.seek(&[KEY_BLOCK_INDEX]);

        while iter.valid() {
            let key = iter.key();

            // Stop once we've moved past the block-index prefix range.
            if key.is_empty() || key[0] != KEY_BLOCK_INDEX {
                break;
            }

            // Key must be exactly 1 (prefix) + 32 (hash) = 33 bytes.
            if key.len() == 33 {
                let mut hash_bytes = [0u8; 32];
                hash_bytes.copy_from_slice(&key[1..33]);
                let block_hash = BlockHash::from_bytes(hash_bytes);

                let value = iter.value();
                if let Ok(record) = deserialize_block_index_record(block_hash, value) {
                    records.push(record);
                }
            }

            iter.next();
        }

        records
    }

    // -- Block file bookkeeping ---------------------------------------------

    /// Persist the last-known block file number. Returns `true` on success.
    pub fn write_last_block_file(&self, file_no: u32) -> bool {
        let db = self.db.inner();
        let mut batch = db.new_batch();
        batch.put(KEY_LAST_BLOCK_FILE, &file_no.to_le_bytes());
        db.write_batch(batch, false).is_ok()
    }

    /// Read the last-known block file number, if stored.
    pub fn read_last_block_file(&self) -> Option<u32> {
        let db = self.db.inner();
        match db.read(KEY_LAST_BLOCK_FILE) {
            Ok(Some(data)) if data.len() == 4 => {
                Some(u32::from_le_bytes([data[0], data[1], data[2], data[3]]))
            }
            _ => None,
        }
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use qubitcoin_common::chain::BlockStatus;
    use qubitcoin_primitives::{ArithUint256, BlockHash, Uint256};
    use qubitcoin_storage::MemoryDb;

    /// Helper: create a `BlockIndex` with recognisable field values.
    fn make_test_block_index(height: i32, hash_byte: u8) -> BlockIndex {
        let mut hash_data = [0u8; 32];
        hash_data[0] = hash_byte;

        let mut prev_data = [0u8; 32];
        prev_data[0] = hash_byte.wrapping_sub(1);

        let mut merkle_data = [0u8; 32];
        merkle_data[0] = 0xAA;
        merkle_data[31] = hash_byte;

        let mut bi = BlockIndex::new();
        bi.block_hash = BlockHash::from_bytes(hash_data);
        bi.version = 0x2000_0000;
        bi.prev_blockhash = BlockHash::from_bytes(prev_data);
        bi.merkle_root = Uint256::from_bytes(merkle_data);
        bi.time = 1_700_000_000 + height as u32;
        bi.bits = 0x1d00_ffff;
        bi.nonce = 42 + height as u32;
        bi.height = height;
        bi.status = BlockStatus::new(
            BlockStatus::VALID_SCRIPTS | BlockStatus::HAVE_DATA | BlockStatus::HAVE_UNDO,
        );
        bi.file = 1;
        bi.data_pos = 1024 * height as u32;
        bi.undo_pos = 2048 * height as u32;
        bi.tx_count = 10 + height as u32;
        bi.chain_tx_count = 100 + height as u64;
        bi.chain_work = ArithUint256::from_u64(0xdead_beef_1234) + ArithUint256::from_u64(height as u64);
        bi
    }

    #[test]
    fn roundtrip_single_block_index() {
        let db = BlockIndexDB::new_unobfuscated(MemoryDb::new());
        let bi = make_test_block_index(42, 0x01);

        assert!(db.write_block_index(&bi));

        let records = db.load_all();
        assert_eq!(records.len(), 1);

        let rec = &records[0];
        assert_eq!(rec.block_hash, bi.block_hash);
        assert_eq!(rec.version, bi.version);
        assert_eq!(rec.prev_blockhash, bi.prev_blockhash);
        assert_eq!(rec.merkle_root, bi.merkle_root);
        assert_eq!(rec.time, bi.time);
        assert_eq!(rec.bits, bi.bits);
        assert_eq!(rec.nonce, bi.nonce);
        assert_eq!(rec.height, bi.height);
        assert_eq!(rec.status_bits, bi.status.bits());
        assert_eq!(rec.file, bi.file);
        assert_eq!(rec.data_pos, bi.data_pos);
        assert_eq!(rec.undo_pos, bi.undo_pos);
        assert_eq!(rec.tx_count, bi.tx_count);
        assert_eq!(rec.chain_tx_count, bi.chain_tx_count);
        assert_eq!(rec.chain_work(), bi.chain_work);
    }

    #[test]
    fn batch_write_multiple() {
        let db = BlockIndexDB::new_unobfuscated(MemoryDb::new());
        let b1 = make_test_block_index(1, 0x11);
        let b2 = make_test_block_index(2, 0x22);
        let b3 = make_test_block_index(3, 0x33);

        let entries: Vec<&BlockIndex> = vec![&b1, &b2, &b3];
        assert!(db.write_block_indices(&entries));

        let records = db.load_all();
        assert_eq!(records.len(), 3);

        // Records are ordered by key (BTreeMap iterator), and the key is
        // [b'b' | hash]. The hash bytes start with 0x11, 0x22, 0x33 so they
        // should be in ascending order.
        assert_eq!(records[0].block_hash, b1.block_hash);
        assert_eq!(records[1].block_hash, b2.block_hash);
        assert_eq!(records[2].block_hash, b3.block_hash);
    }

    #[test]
    fn last_block_file_roundtrip() {
        let db = BlockIndexDB::new_unobfuscated(MemoryDb::new());

        // Initially missing.
        assert_eq!(db.read_last_block_file(), None);

        // Write and read back.
        assert!(db.write_last_block_file(7));
        assert_eq!(db.read_last_block_file(), Some(7));

        // Overwrite.
        assert!(db.write_last_block_file(42));
        assert_eq!(db.read_last_block_file(), Some(42));
    }

    #[test]
    fn load_all_empty_database() {
        let db = BlockIndexDB::new_unobfuscated(MemoryDb::new());
        let records = db.load_all();
        assert!(records.is_empty());
    }

    #[test]
    fn chain_work_high_bits_roundtrip() {
        let db = BlockIndexDB::new_unobfuscated(MemoryDb::new());

        let mut bi = make_test_block_index(99, 0xFF);
        // Set a chain_work that uses high limbs (above u64 range).
        let mut big_work = ArithUint256::from_u64(u64::MAX);
        big_work += ArithUint256::from_u64(1); // 2^64
        bi.chain_work = big_work;

        assert!(db.write_block_index(&bi));

        let records = db.load_all();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].chain_work(), big_work);
    }

    #[test]
    fn serialization_size_is_144_bytes() {
        let bi = make_test_block_index(0, 0x00);
        let serialized = serialize_block_index(&bi);
        assert_eq!(serialized.len(), 144);
    }

    #[test]
    fn overwrite_existing_entry() {
        let db = BlockIndexDB::new_unobfuscated(MemoryDb::new());
        let mut bi = make_test_block_index(10, 0x42);

        assert!(db.write_block_index(&bi));

        // Update a field and re-write.
        bi.tx_count = 999;
        bi.chain_tx_count = 9999;
        assert!(db.write_block_index(&bi));

        let records = db.load_all();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].tx_count, 999);
        assert_eq!(records[0].chain_tx_count, 9999);
    }

    #[test]
    fn block_file_key_does_not_collide_with_index_prefix() {
        // Ensure the 'F' key is not in the 'b' prefix range, so load_all
        // does not accidentally pick it up.
        let db = BlockIndexDB::new_unobfuscated(MemoryDb::new());

        assert!(db.write_last_block_file(123));

        let bi = make_test_block_index(1, 0x01);
        assert!(db.write_block_index(&bi));

        let records = db.load_all();
        assert_eq!(records.len(), 1); // only the block index entry, not the file number
    }
}
