//! Block storage using flat files (`blk*.dat` and `rev*.dat`).
//!
//! Maps to: `src/node/blockstorage.cpp` in Bitcoin Core.
//!
//! Blocks and their associated undo data are written to numbered flat files
//! in the `blocks/` subdirectory of the data directory.  Each block file is
//! capped at `MAX_BLOCKFILE_SIZE` bytes; when the current file is full a
//! new one is opened.
//!
//! File naming:
//! - Block data: `blk00000.dat`, `blk00001.dat`, ...
//! - Undo data:  `rev00000.dat`, `rev00001.dat`, ...

use crate::undo::BlockUndo;
use qubitcoin_consensus::block::Block;
use qubitcoin_serialize::Decodable;

use std::fs::{File, OpenOptions};
use std::io::{BufReader, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use parking_lot::RwLock;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum size of a single block file (128 MB, matching Bitcoin Core).
pub const MAX_BLOCKFILE_SIZE: u64 = 128 * 1024 * 1024;

// ---------------------------------------------------------------------------
// DiskBlockPos
// ---------------------------------------------------------------------------

/// Position of block or undo data on disk.
///
/// Combines a file number and a byte offset within that file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiskBlockPos {
    /// File number (`blk?????.dat` or `rev?????.dat`).
    pub file: i32,
    /// Byte offset within the file where the serialized data begins.
    pub pos: u32,
}

impl DiskBlockPos {
    /// A null (invalid) position.
    pub fn null() -> Self {
        DiskBlockPos { file: -1, pos: 0 }
    }

    /// Returns `true` if this position is null (no data available).
    pub fn is_null(&self) -> bool {
        self.file < 0
    }
}

impl Default for DiskBlockPos {
    fn default() -> Self {
        Self::null()
    }
}

// ---------------------------------------------------------------------------
// BlockFileManager
// ---------------------------------------------------------------------------

/// Manages block data files (`blk*.dat`) and undo files (`rev*.dat`).
///
/// Provides append-only write operations and random-access reads.
/// Thread-safe via `RwLock`-guarded interior state.
pub struct BlockFileManager {
    /// Root data directory (the `blocks/` subdir is created inside it).
    data_dir: PathBuf,
    /// Current block file number.
    current_file: RwLock<i32>,
    /// Current write offset in the active block file.
    current_pos: RwLock<u64>,
    /// Current write offset in the active undo file.
    current_undo_pos: RwLock<u64>,
}

impl BlockFileManager {
    /// Create a new `BlockFileManager` rooted at `data_dir`.
    ///
    /// Creates the `blocks/` subdirectory if it does not already exist.
    pub fn new(data_dir: &Path) -> Self {
        std::fs::create_dir_all(data_dir.join("blocks")).ok();
        BlockFileManager {
            data_dir: data_dir.to_path_buf(),
            current_file: RwLock::new(0),
            current_pos: RwLock::new(0),
            current_undo_pos: RwLock::new(0),
        }
    }

    /// Return the filesystem path for block data file `file_num`.
    fn block_file_path(&self, file_num: i32) -> PathBuf {
        self.data_dir
            .join("blocks")
            .join(format!("blk{:05}.dat", file_num))
    }

    /// Return the filesystem path for undo data file `file_num`.
    fn undo_file_path(&self, file_num: i32) -> PathBuf {
        self.data_dir
            .join("blocks")
            .join(format!("rev{:05}.dat", file_num))
    }

    // -- Block data ---------------------------------------------------------

    /// Write a block to disk and return its [`DiskBlockPos`].
    ///
    /// The on-disk format is:
    /// ```text
    /// [4 bytes] network magic
    /// [4 bytes] serialized block size (little-endian u32)
    /// [N bytes] serialized block data
    /// ```
    ///
    /// If the current file would exceed `MAX_BLOCKFILE_SIZE`, a new file is
    /// started automatically.
    pub fn write_block(
        &self,
        block: &Block,
        magic: [u8; 4],
    ) -> Result<DiskBlockPos, std::io::Error> {
        let serialized = qubitcoin_serialize::serialize(block)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

        let mut file_num = *self.current_file.read();
        let mut pos = *self.current_pos.read();

        // Total bytes we will append: magic(4) + size(4) + data(N).
        let total_size = 4 + 4 + serialized.len() as u64;
        if pos + total_size > MAX_BLOCKFILE_SIZE && pos > 0 {
            file_num += 1;
            pos = 0;
            *self.current_file.write() = file_num;
            *self.current_pos.write() = 0;
            // Reset undo position for the new file.
            *self.current_undo_pos.write() = 0;
        }

        let path = self.block_file_path(file_num);
        let mut file = OpenOptions::new().create(true).append(true).open(&path)?;

        // Write: magic + size + block data.
        file.write_all(&magic)?;
        file.write_all(&(serialized.len() as u32).to_le_bytes())?;
        let data_pos = pos + 8; // position where the actual block data starts
        file.write_all(&serialized)?;

        *self.current_pos.write() = pos + total_size;

        Ok(DiskBlockPos {
            file: file_num,
            pos: data_pos as u32,
        })
    }

    /// Read a block from disk at the given position.
    ///
    /// `pos.pos` should point to the start of the serialized block data
    /// (after the 8-byte magic+size header).
    pub fn read_block(&self, pos: &DiskBlockPos) -> Result<Block, std::io::Error> {
        if pos.is_null() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "null disk position",
            ));
        }
        let path = self.block_file_path(pos.file);
        eprintln!("[read_block] file={}, pos={}, path={}", pos.file, pos.pos, path.display());
        let mut file = File::open(&path)?;
        file.seek(SeekFrom::Start(pos.pos as u64))?;

        let mut reader = BufReader::new(file);
        Block::decode(&mut reader)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
    }

    // -- Undo data ----------------------------------------------------------

    /// Write undo data for a block and return its [`DiskBlockPos`].
    ///
    /// The on-disk format is:
    /// ```text
    /// [4 bytes] serialized undo data size (little-endian u32)
    /// [N bytes] serialized BlockUndo data
    /// ```
    pub fn write_undo(
        &self,
        file_num: i32,
        undo: &BlockUndo,
    ) -> Result<DiskBlockPos, std::io::Error> {
        let serialized = qubitcoin_serialize::serialize(undo)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

        let path = self.undo_file_path(file_num);
        let mut file = OpenOptions::new().create(true).append(true).open(&path)?;

        let pos = *self.current_undo_pos.read();
        file.write_all(&(serialized.len() as u32).to_le_bytes())?;
        let data_pos = pos + 4;
        file.write_all(&serialized)?;

        *self.current_undo_pos.write() = pos + 4 + serialized.len() as u64;

        Ok(DiskBlockPos {
            file: file_num,
            pos: data_pos as u32,
        })
    }

    /// Read undo data from disk at the given position.
    ///
    /// `pos.pos` should point to the start of the serialized `BlockUndo` data
    /// (after the 4-byte size header).
    pub fn read_undo(&self, pos: &DiskBlockPos) -> Result<BlockUndo, std::io::Error> {
        if pos.is_null() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "null disk position",
            ));
        }
        let path = self.undo_file_path(pos.file);
        let mut file = File::open(&path)?;
        file.seek(SeekFrom::Start(pos.pos as u64))?;

        let mut reader = BufReader::new(file);
        BlockUndo::decode(&mut reader)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
    }

    /// Return the current block file number.
    pub fn current_file_num(&self) -> i32 {
        *self.current_file.read()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::undo::{BlockUndo, TxUndo};
    use qubitcoin_common::coins::Coin;
    use qubitcoin_consensus::block::{Block, BlockHeader};
    use qubitcoin_consensus::transaction::{Transaction, TxIn, TxOut};
    use qubitcoin_primitives::{Amount, BlockHash, Uint256};
    use qubitcoin_script::Script;
    use std::sync::Arc;

    fn make_test_block(nonce: u32) -> Block {
        let coinbase = Transaction::new(
            1,
            vec![TxIn::coinbase(Script::from_bytes(vec![0x04, 0xff]))],
            vec![TxOut::new(
                Amount::from_btc(50),
                Script::from_bytes(vec![0x76, 0xa9]),
            )],
            0,
        );

        let header = BlockHeader {
            version: 1,
            prev_blockhash: BlockHash::ZERO,
            merkle_root: Uint256::ZERO,
            time: 1231006505,
            bits: 0x207fffff,
            nonce,
        };

        Block {
            header,
            vtx: vec![Arc::new(coinbase)],
        }
    }

    /// Create a temporary directory for tests that auto-cleans on drop.
    struct TestDir(PathBuf);

    impl TestDir {
        fn new() -> Self {
            use std::sync::atomic::{AtomicU64, Ordering};
            static COUNTER: AtomicU64 = AtomicU64::new(0);
            let id = COUNTER.fetch_add(1, Ordering::SeqCst);
            let pid = std::process::id();
            let path = std::env::temp_dir().join(format!("qubitcoin_test_{}_{}", pid, id));
            std::fs::create_dir_all(&path).unwrap();
            TestDir(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn test_disk_block_pos_null() {
        let pos = DiskBlockPos::null();
        assert!(pos.is_null());
        assert_eq!(pos.file, -1);

        let pos2 = DiskBlockPos { file: 0, pos: 100 };
        assert!(!pos2.is_null());
    }

    #[test]
    fn test_write_and_read_block() {
        let dir = TestDir::new();
        let mgr = BlockFileManager::new(dir.path());

        let block = make_test_block(42);
        let magic = [0xf9, 0xbe, 0xb4, 0xd9]; // mainnet magic

        let pos = mgr.write_block(&block, magic).unwrap();
        assert!(!pos.is_null());
        assert_eq!(pos.file, 0);

        let read_back = mgr.read_block(&pos).unwrap();
        assert_eq!(read_back.header, block.header);
        assert_eq!(read_back.vtx.len(), block.vtx.len());
        assert_eq!(read_back.block_hash(), block.block_hash());
    }

    #[test]
    fn test_write_multiple_blocks() {
        let dir = TestDir::new();
        let mgr = BlockFileManager::new(dir.path());
        let magic = [0xfa, 0xbf, 0xb5, 0xda];

        let mut positions = Vec::new();
        for i in 0..5 {
            let block = make_test_block(i);
            let pos = mgr.write_block(&block, magic).unwrap();
            positions.push((pos, block));
        }

        // Verify all blocks can be read back.
        for (pos, original) in &positions {
            let read_back = mgr.read_block(pos).unwrap();
            assert_eq!(read_back.block_hash(), original.block_hash());
        }
    }

    #[test]
    fn test_write_and_read_undo() {
        let dir = TestDir::new();
        let mgr = BlockFileManager::new(dir.path());

        let mut block_undo = BlockUndo::new();
        let mut tx_undo = TxUndo::new();
        tx_undo.prev_coins.push(Coin::new(
            TxOut::new(
                Amount::from_sat(100_000),
                Script::from_bytes(vec![0x76, 0xa9]),
            ),
            50,
            false,
        ));
        tx_undo.prev_coins.push(Coin::new(
            TxOut::new(
                Amount::from_sat(200_000),
                Script::from_bytes(vec![0x76, 0xa9]),
            ),
            60,
            true,
        ));
        block_undo.tx_undo.push(tx_undo);

        let pos = mgr.write_undo(0, &block_undo).unwrap();
        assert!(!pos.is_null());

        let read_back = mgr.read_undo(&pos).unwrap();
        assert_eq!(read_back.tx_undo.len(), 1);
        assert_eq!(read_back.tx_undo[0].prev_coins.len(), 2);
        assert_eq!(
            read_back.tx_undo[0].prev_coins[0].tx_out.value.to_sat(),
            100_000
        );
        assert_eq!(
            read_back.tx_undo[0].prev_coins[1].tx_out.value.to_sat(),
            200_000
        );
        assert!(read_back.tx_undo[0].prev_coins[1].coinbase);
    }

    #[test]
    fn test_write_multiple_undos() {
        let dir = TestDir::new();
        let mgr = BlockFileManager::new(dir.path());

        let mut positions = Vec::new();
        for i in 0..3u32 {
            let mut undo = BlockUndo::new();
            let mut tx_undo = TxUndo::new();
            tx_undo.prev_coins.push(Coin::new(
                TxOut::new(
                    Amount::from_sat((i as i64 + 1) * 50_000),
                    Script::from_bytes(vec![0x76, 0xa9]),
                ),
                i,
                false,
            ));
            undo.tx_undo.push(tx_undo);
            let pos = mgr.write_undo(0, &undo).unwrap();
            positions.push((pos, (i as i64 + 1) * 50_000));
        }

        for (pos, expected_value) in &positions {
            let read_back = mgr.read_undo(pos).unwrap();
            assert_eq!(read_back.tx_undo.len(), 1);
            assert_eq!(
                read_back.tx_undo[0].prev_coins[0].tx_out.value.to_sat(),
                *expected_value
            );
        }
    }

    #[test]
    fn test_read_null_position_fails() {
        let dir = TestDir::new();
        let mgr = BlockFileManager::new(dir.path());

        let null_pos = DiskBlockPos::null();
        assert!(mgr.read_block(&null_pos).is_err());
        assert!(mgr.read_undo(&null_pos).is_err());
    }

    #[test]
    fn test_block_and_undo_same_file_num() {
        let dir = TestDir::new();
        let mgr = BlockFileManager::new(dir.path());

        let block = make_test_block(99);
        let magic = [0xf9, 0xbe, 0xb4, 0xd9];
        let block_pos = mgr.write_block(&block, magic).unwrap();

        let mut undo = BlockUndo::new();
        let mut tx_undo = TxUndo::new();
        tx_undo.prev_coins.push(Coin::new(
            TxOut::new(
                Amount::from_sat(500_000),
                Script::from_bytes(vec![0x76, 0xa9]),
            ),
            10,
            false,
        ));
        undo.tx_undo.push(tx_undo);
        let undo_pos = mgr.write_undo(block_pos.file, &undo).unwrap();

        // Both should be on file 0.
        assert_eq!(block_pos.file, 0);
        assert_eq!(undo_pos.file, 0);

        // Both should be readable.
        let read_block = mgr.read_block(&block_pos).unwrap();
        assert_eq!(read_block.block_hash(), block.block_hash());

        let read_undo = mgr.read_undo(&undo_pos).unwrap();
        assert_eq!(read_undo.tx_undo.len(), 1);
    }
}
