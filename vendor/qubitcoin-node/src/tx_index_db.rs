//! Transaction index database: maps `txid → (block_file, data_pos, tx_index)`.
//!
//! Enables `getrawtransaction` lookups for confirmed transactions by storing
//! the disk location of every transaction that enters the active chain.
//!
//! The database key is `[b't' | txid(32)]` (33 bytes) and the value is
//! `[file(i32 LE) | data_pos(u32 LE) | tx_index(u32 LE)]` (12 bytes).
//!
//! Follows the same pattern as [`crate::block_index_db::BlockIndexDB`].

use qubitcoin_primitives::Txid;
use qubitcoin_storage::traits::DbBatch;
use qubitcoin_storage::{Database, DbWrapper};

// ---------------------------------------------------------------------------
// Key constants
// ---------------------------------------------------------------------------

/// Prefix byte for transaction index entries: `b't'`.
const KEY_TX_INDEX: u8 = b't';

/// Value size: file(4) + data_pos(4) + tx_index(4) = 12 bytes.
const VALUE_SIZE: usize = 12;

// ---------------------------------------------------------------------------
// Key helper
// ---------------------------------------------------------------------------

/// Build the database key for a transaction index entry: `[b't' | txid(32)]`.
fn tx_index_key(txid: &Txid) -> Vec<u8> {
    let mut key = Vec::with_capacity(33);
    key.push(KEY_TX_INDEX);
    key.extend_from_slice(txid.data());
    key
}

// ---------------------------------------------------------------------------
// TxIndexDB
// ---------------------------------------------------------------------------

/// Persistent transaction index database.
///
/// Wraps a [`DbWrapper`] and provides typed read/write operations for
/// transaction position lookups.
pub struct TxIndexDB<D: Database> {
    db: DbWrapper<D>,
}

impl<D: Database> TxIndexDB<D> {
    /// Create a new `TxIndexDB` wrapping the given database.
    ///
    /// If `obfuscate` is true, values are XOR-obfuscated on disk (matching
    /// Bitcoin Core behaviour).
    pub fn new(db: D, obfuscate: bool) -> Self {
        let wrapper = if obfuscate {
            DbWrapper::new(db, true)
        } else {
            DbWrapper::new_unobfuscated(db)
        };
        TxIndexDB { db: wrapper }
    }

    /// Create without obfuscation (convenience for testing).
    pub fn new_unobfuscated(db: D) -> Self {
        TxIndexDB {
            db: DbWrapper::new_unobfuscated(db),
        }
    }

    /// Write transaction positions for an entire block in a single atomic batch.
    ///
    /// Each entry is `(txid, block_file, block_data_pos, tx_index_in_block)`.
    /// Returns `true` on success.
    pub fn write_tx_positions(&self, entries: &[(Txid, i32, u32, u32)]) -> bool {
        let db = self.db.inner();
        let mut batch = db.new_batch();
        for (txid, file, data_pos, tx_idx) in entries {
            let key = tx_index_key(txid);
            let mut value = Vec::with_capacity(VALUE_SIZE);
            value.extend_from_slice(&file.to_le_bytes());
            value.extend_from_slice(&data_pos.to_le_bytes());
            value.extend_from_slice(&tx_idx.to_le_bytes());
            batch.put(&key, &value);
        }
        db.write_batch(batch, false).is_ok()
    }

    /// Look up the disk position of a transaction by its txid.
    ///
    /// Returns `Some((block_file, block_data_pos, tx_index_in_block))` if found.
    pub fn read_tx_pos(&self, txid: &Txid) -> Option<(i32, u32, u32)> {
        let db = self.db.inner();
        let key = tx_index_key(txid);
        match db.read(&key) {
            Ok(Some(data)) if data.len() == VALUE_SIZE => {
                let file = i32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                let data_pos = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
                let tx_idx = u32::from_le_bytes([data[8], data[9], data[10], data[11]]);
                Some((file, data_pos, tx_idx))
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
    use qubitcoin_primitives::Txid;
    use qubitcoin_storage::MemoryDb;

    fn make_txid(byte: u8) -> Txid {
        let mut data = [0u8; 32];
        data[0] = byte;
        Txid::from_bytes(data)
    }

    #[test]
    fn roundtrip_single() {
        let db = TxIndexDB::new_unobfuscated(MemoryDb::new());
        let txid = make_txid(0x42);

        assert!(db.write_tx_positions(&[(txid, 3, 1024, 7)]));

        let result = db.read_tx_pos(&txid);
        assert_eq!(result, Some((3, 1024, 7)));
    }

    #[test]
    fn batch_write_multiple() {
        let db = TxIndexDB::new_unobfuscated(MemoryDb::new());
        let t1 = make_txid(0x01);
        let t2 = make_txid(0x02);
        let t3 = make_txid(0x03);

        let entries = vec![
            (t1, 0, 100, 0),
            (t2, 0, 100, 1),
            (t3, 1, 200, 0),
        ];
        assert!(db.write_tx_positions(&entries));

        assert_eq!(db.read_tx_pos(&t1), Some((0, 100, 0)));
        assert_eq!(db.read_tx_pos(&t2), Some((0, 100, 1)));
        assert_eq!(db.read_tx_pos(&t3), Some((1, 200, 0)));
    }

    #[test]
    fn missing_key_returns_none() {
        let db = TxIndexDB::new_unobfuscated(MemoryDb::new());
        let txid = make_txid(0xFF);
        assert_eq!(db.read_tx_pos(&txid), None);
    }

    #[test]
    fn overwrite_existing() {
        let db = TxIndexDB::new_unobfuscated(MemoryDb::new());
        let txid = make_txid(0xAA);

        assert!(db.write_tx_positions(&[(txid, 1, 500, 3)]));
        assert_eq!(db.read_tx_pos(&txid), Some((1, 500, 3)));

        // Overwrite with new position.
        assert!(db.write_tx_positions(&[(txid, 2, 800, 5)]));
        assert_eq!(db.read_tx_pos(&txid), Some((2, 800, 5)));
    }
}
