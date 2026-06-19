//! Flat-file block storage manager.
//! Maps to: src/node/blockstorage.h / src/flatfile.h (FlatFilePos + FlatFileSeq)
//!
//! Provides `BlockFileManager` which manages blk?????.dat files, matching
//! Bitcoin Core's design where each file has a maximum size of 128 MiB
//! and block data is prefixed with network magic (4 bytes) + size (4 bytes LE).

use parking_lot::Mutex;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Constants (matching Bitcoin Core: src/node/blockstorage.h)
// ---------------------------------------------------------------------------

/// Maximum size of a single blk?????.dat file (128 MiB).
pub const MAX_BLOCKFILE_SIZE: u32 = 0x0800_0000; // 128 MiB

/// Size of the header written before each serialized block:
///   4 bytes network magic  +  4 bytes block-data length (LE).
pub const STORAGE_HEADER_BYTES: u32 = 8;

/// Default mainnet network magic bytes.
pub const MAINNET_MAGIC: [u8; 4] = [0xf9, 0xbe, 0xb4, 0xd9];

// ---------------------------------------------------------------------------
// BlockFilePos  (maps to Bitcoin Core's FlatFilePos)
// ---------------------------------------------------------------------------

/// Position of a block within a flat-file sequence.
///
/// `file_no` is the numeric suffix of the blk?????.dat filename.
/// `pos` is the byte offset *after* the 8-byte header (magic + size),
/// pointing directly at the raw block data, matching Bitcoin Core convention.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockFilePos {
    /// File number (blk00000.dat = 0, blk00001.dat = 1, ...).
    pub file_no: u32,
    /// Byte offset within the file where the raw block data begins
    /// (i.e. right after the 8-byte storage header).
    pub pos: u32,
}

impl BlockFilePos {
    /// Create a new `BlockFilePos` with the given file number and byte offset.
    pub fn new(file_no: u32, pos: u32) -> Self {
        Self { file_no, pos }
    }

    /// A null/invalid position.
    pub fn null() -> Self {
        Self {
            file_no: u32::MAX,
            pos: 0,
        }
    }

    /// Returns `true` if this position is the null sentinel value.
    pub fn is_null(&self) -> bool {
        self.file_no == u32::MAX
    }
}

impl fmt::Debug for BlockFilePos {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BlockFilePos(file={}, pos={})", self.file_no, self.pos)
    }
}

impl fmt::Display for BlockFilePos {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BlockFilePos(file={}, pos={})", self.file_no, self.pos)
    }
}

// ---------------------------------------------------------------------------
// BlockFileInfo  (maps to Bitcoin Core's CBlockFileInfo, simplified)
// ---------------------------------------------------------------------------

/// Metadata about a single blk?????.dat file.
#[derive(Debug, Clone)]
struct BlockFileInfo {
    /// Number of blocks stored in this file.
    n_blocks: u32,
    /// Number of bytes used in this file (including headers).
    n_size: u32,
}

impl BlockFileInfo {
    fn new() -> Self {
        Self {
            n_blocks: 0,
            n_size: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// Inner state (behind Mutex)
// ---------------------------------------------------------------------------

struct Inner {
    /// Info for each block file we know about. Index = file number.
    file_info: Vec<BlockFileInfo>,
    /// The current block file being written to.
    current_file: u32,
}

impl Inner {
    fn new() -> Self {
        Self {
            file_info: vec![BlockFileInfo::new()],
            current_file: 0,
        }
    }

    /// Ensure `file_info` has an entry for `file_no`.
    fn ensure_file_info(&mut self, file_no: u32) {
        let idx = file_no as usize;
        if self.file_info.len() <= idx {
            self.file_info.resize(idx + 1, BlockFileInfo::new());
        }
    }

    /// Get the current used size in the given file.
    fn file_size(&self, file_no: u32) -> u32 {
        self.file_info
            .get(file_no as usize)
            .map(|fi| fi.n_size)
            .unwrap_or(0)
    }
}

// ---------------------------------------------------------------------------
// BlockFileManager
// ---------------------------------------------------------------------------

/// Manages a sequence of flat files (`blk?????.dat`) for persisting raw block
/// data, mirroring Bitcoin Core's `BlockManager` flat-file logic.
///
/// Thread-safe: all mutable state is protected by a `parking_lot::Mutex`.
pub struct BlockFileManager {
    /// Root directory where blk?????.dat files are created.
    data_dir: PathBuf,
    /// Network magic prepended to every block entry.
    magic: [u8; 4],
    /// Maximum size of a single blk file.
    max_file_size: u32,
    /// Mutable inner state.
    inner: Mutex<Inner>,
}

impl BlockFileManager {
    /// Create a new `BlockFileManager` that stores files under `data_dir`.
    ///
    /// Uses mainnet magic and the default 128 MiB file size cap.
    pub fn new(data_dir: PathBuf) -> Self {
        Self::with_params(data_dir, MAINNET_MAGIC, MAX_BLOCKFILE_SIZE)
    }

    /// Create a `BlockFileManager` with custom network magic and max file size.
    ///
    /// Useful for testing (small max file size) and for non-mainnet networks.
    pub fn with_params(data_dir: PathBuf, magic: [u8; 4], max_file_size: u32) -> Self {
        // Ensure the data directory exists.
        let _ = fs::create_dir_all(&data_dir);
        Self {
            data_dir,
            magic,
            max_file_size,
            inner: Mutex::new(Inner::new()),
        }
    }

    // -- public API ---------------------------------------------------------

    /// Append raw block data to the current file and return its position.
    ///
    /// The on-disk layout for each entry is:
    ///   [4-byte magic] [4-byte LE block size] [raw block data ...]
    ///
    /// The returned `BlockFilePos::pos` points to the start of the raw block
    /// data (i.e. after the 8-byte header), matching Bitcoin Core's convention
    /// where `FlatFilePos::nPos` is advanced past the header in `WriteBlock`.
    pub fn write_block(&self, data: &[u8]) -> Result<BlockFilePos, io::Error> {
        let block_size = data.len() as u32;
        let total_entry_size = STORAGE_HEADER_BYTES + block_size;

        // Guard: a single block must fit inside a fresh file.
        if total_entry_size > self.max_file_size {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "block data ({} bytes) exceeds maximum file size ({} bytes)",
                    data.len(),
                    self.max_file_size
                ),
            ));
        }

        let mut inner = self.inner.lock();

        // Roll over to the next file if the current one would exceed the cap.
        let current_size = inner.file_size(inner.current_file);
        if current_size + total_entry_size > self.max_file_size {
            inner.current_file += 1;
            let cf = inner.current_file;
            inner.ensure_file_info(cf);
        }

        let file_no = inner.current_file;
        let write_offset = inner.file_size(file_no);

        // Open (or create) the file and seek to the write position.
        let path = self.block_file_path(file_no);
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .read(true)
            .open(&path)?;
        file.seek(SeekFrom::Start(write_offset as u64))?;

        // Write: [magic][size LE][block data]
        file.write_all(&self.magic)?;
        file.write_all(&block_size.to_le_bytes())?;
        file.write_all(data)?;
        file.sync_data()?;

        // Update bookkeeping.
        inner.ensure_file_info(file_no);
        let fi = &mut inner.file_info[file_no as usize];
        fi.n_blocks += 1;
        fi.n_size += total_entry_size;

        // pos points past the 8-byte header, at the raw block data.
        Ok(BlockFilePos::new(
            file_no,
            write_offset + STORAGE_HEADER_BYTES,
        ))
    }

    /// Read block data previously written at `pos`.
    ///
    /// `pos` must have been returned by a prior `write_block` call (or
    /// constructed equivalently). The position points to the start of raw block
    /// data; this method seeks back 8 bytes to read the header first.
    pub fn read_block(&self, pos: &BlockFilePos) -> Result<Vec<u8>, io::Error> {
        if pos.is_null() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "null BlockFilePos",
            ));
        }

        if pos.pos < STORAGE_HEADER_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "BlockFilePos offset {} is less than STORAGE_HEADER_BYTES ({}); cannot read header",
                    pos.pos, STORAGE_HEADER_BYTES
                ),
            ));
        }

        let path = self.block_file_path(pos.file_no);
        let mut file = File::open(&path).map_err(|e| {
            io::Error::new(
                e.kind(),
                format!("failed to open {}: {}", path.display(), e),
            )
        })?;

        // Seek to the start of the 8-byte header preceding the raw block data.
        let header_offset = (pos.pos - STORAGE_HEADER_BYTES) as u64;
        file.seek(SeekFrom::Start(header_offset))?;

        // Read magic.
        let mut magic_buf = [0u8; 4];
        file.read_exact(&mut magic_buf)?;
        if magic_buf != self.magic {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "block magic mismatch at {}: expected {:02x?}, got {:02x?}",
                    pos, self.magic, magic_buf
                ),
            ));
        }

        // Read size.
        let mut size_buf = [0u8; 4];
        file.read_exact(&mut size_buf)?;
        let block_size = u32::from_le_bytes(size_buf) as usize;

        // Read block data.
        let mut block_data = vec![0u8; block_size];
        file.read_exact(&mut block_data)?;

        Ok(block_data)
    }

    // -- helpers ------------------------------------------------------------

    /// Return the filesystem path for blk?????.dat with the given number.
    pub fn block_file_path(&self, file_no: u32) -> PathBuf {
        self.data_dir.join(format!("blk{:05}.dat", file_no))
    }

    /// Return the current block file number.
    pub fn current_file_no(&self) -> u32 {
        self.inner.lock().current_file
    }

    /// Return (n_blocks, n_size) for the given file number.
    pub fn file_info(&self, file_no: u32) -> Option<(u32, u32)> {
        let inner = self.inner.lock();
        inner
            .file_info
            .get(file_no as usize)
            .map(|fi| (fi.n_blocks, fi.n_size))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Helper: create a BlockFileManager with a temporary directory and the
    /// specified maximum file size.
    fn make_manager(max_file_size: u32) -> (BlockFileManager, TempDir) {
        let tmp = TempDir::new().expect("failed to create temp dir");
        let mgr =
            BlockFileManager::with_params(tmp.path().to_path_buf(), MAINNET_MAGIC, max_file_size);
        (mgr, tmp)
    }

    #[test]
    fn test_write_and_read_single_block() {
        let (mgr, _tmp) = make_manager(MAX_BLOCKFILE_SIZE);

        let block_data = b"this is a fake block";
        let pos = mgr.write_block(block_data).unwrap();

        assert_eq!(pos.file_no, 0);
        assert_eq!(pos.pos, STORAGE_HEADER_BYTES); // first block starts right after the 8-byte header

        let read_back = mgr.read_block(&pos).unwrap();
        assert_eq!(read_back, block_data);
    }

    #[test]
    fn test_write_and_read_multiple_blocks() {
        let (mgr, _tmp) = make_manager(MAX_BLOCKFILE_SIZE);

        let blocks: Vec<Vec<u8>> = (0..10)
            .map(|i| format!("block-data-{:04}", i).into_bytes())
            .collect();

        let mut positions = Vec::new();
        for block in &blocks {
            positions.push(mgr.write_block(block).unwrap());
        }

        // All should be in file 0.
        for pos in &positions {
            assert_eq!(pos.file_no, 0);
        }

        // Positions should be strictly increasing.
        for i in 1..positions.len() {
            assert!(positions[i].pos > positions[i - 1].pos);
        }

        // Read them all back.
        for (block, pos) in blocks.iter().zip(positions.iter()) {
            let read_back = mgr.read_block(pos).unwrap();
            assert_eq!(read_back, *block);
        }
    }

    #[test]
    fn test_file_rollover() {
        // Use a small max file size to trigger rollover quickly.
        // Each entry = 8 (header) + block_data.len() bytes.
        // With block_data = 50 bytes => 58 bytes per entry.
        // max_file_size = 100 => only 1 block per file (58 + 58 = 116 > 100).
        let (mgr, _tmp) = make_manager(100);

        let block_a = vec![0xAA; 50];
        let block_b = vec![0xBB; 50];
        let block_c = vec![0xCC; 50];

        let pos_a = mgr.write_block(&block_a).unwrap();
        let pos_b = mgr.write_block(&block_b).unwrap();
        let pos_c = mgr.write_block(&block_c).unwrap();

        // First block in file 0.
        assert_eq!(pos_a.file_no, 0);
        // Second block should roll over to file 1.
        assert_eq!(pos_b.file_no, 1);
        // Third block should roll over to file 2.
        assert_eq!(pos_c.file_no, 2);

        // Verify all three files exist on disk.
        assert!(mgr.block_file_path(0).exists());
        assert!(mgr.block_file_path(1).exists());
        assert!(mgr.block_file_path(2).exists());

        // Read them all back.
        assert_eq!(mgr.read_block(&pos_a).unwrap(), block_a);
        assert_eq!(mgr.read_block(&pos_b).unwrap(), block_b);
        assert_eq!(mgr.read_block(&pos_c).unwrap(), block_c);
    }

    #[test]
    fn test_read_invalid_position_no_file() {
        let (mgr, _tmp) = make_manager(MAX_BLOCKFILE_SIZE);

        // File 99 does not exist.
        let pos = BlockFilePos::new(99, STORAGE_HEADER_BYTES);
        let result = mgr.read_block(&pos);
        assert!(result.is_err());
    }

    #[test]
    fn test_read_null_position() {
        let (mgr, _tmp) = make_manager(MAX_BLOCKFILE_SIZE);

        let pos = BlockFilePos::null();
        let result = mgr.read_block(&pos);
        assert!(result.is_err());
    }

    #[test]
    fn test_read_invalid_offset_too_small() {
        let (mgr, _tmp) = make_manager(MAX_BLOCKFILE_SIZE);

        // Write one block so the file exists.
        mgr.write_block(b"hello").unwrap();

        // pos < STORAGE_HEADER_BYTES is invalid.
        let pos = BlockFilePos::new(0, 2);
        let result = mgr.read_block(&pos);
        assert!(result.is_err());
    }

    #[test]
    fn test_read_invalid_offset_past_eof() {
        let (mgr, _tmp) = make_manager(MAX_BLOCKFILE_SIZE);

        mgr.write_block(b"hello").unwrap();

        // Point well beyond the file's actual content.
        let pos = BlockFilePos::new(0, 9999);
        let result = mgr.read_block(&pos);
        assert!(result.is_err());
    }

    #[test]
    fn test_block_file_path_format() {
        let (mgr, _tmp) = make_manager(MAX_BLOCKFILE_SIZE);

        let path = mgr.block_file_path(0);
        assert!(path.to_string_lossy().ends_with("blk00000.dat"));

        let path = mgr.block_file_path(42);
        assert!(path.to_string_lossy().ends_with("blk00042.dat"));

        let path = mgr.block_file_path(99999);
        assert!(path.to_string_lossy().ends_with("blk99999.dat"));
    }

    #[test]
    fn test_file_info_tracking() {
        let (mgr, _tmp) = make_manager(MAX_BLOCKFILE_SIZE);

        assert_eq!(mgr.file_info(0), Some((0, 0))); // empty initially

        let block = b"some block data";
        mgr.write_block(block).unwrap();

        let (n_blocks, n_size) = mgr.file_info(0).unwrap();
        assert_eq!(n_blocks, 1);
        assert_eq!(n_size, STORAGE_HEADER_BYTES + block.len() as u32);
    }

    #[test]
    fn test_block_too_large_for_file_size() {
        // max_file_size = 20 bytes; a block of 20 bytes needs 28 total (20 + 8 header).
        let (mgr, _tmp) = make_manager(20);

        let result = mgr.write_block(&[0u8; 20]);
        assert!(result.is_err());
    }

    #[test]
    fn test_multiple_blocks_same_file() {
        // max = 200, each entry = 8 + 20 = 28. We can fit 7 blocks (28*7=196).
        let (mgr, _tmp) = make_manager(200);

        let mut positions = Vec::new();
        for i in 0u8..7 {
            let data = vec![i; 20];
            let pos = mgr.write_block(&data).unwrap();
            assert_eq!(pos.file_no, 0, "block {} should be in file 0", i);
            positions.push(pos);
        }

        // 8th block should trigger rollover (196 + 28 = 224 > 200).
        let pos = mgr.write_block(&vec![0xFF; 20]).unwrap();
        assert_eq!(pos.file_no, 1);

        // Verify we can still read all blocks from file 0.
        for (i, pos) in positions.iter().enumerate() {
            let data = mgr.read_block(pos).unwrap();
            assert_eq!(data, vec![i as u8; 20]);
        }
    }

    #[test]
    fn test_magic_mismatch_on_read() {
        // Write with one magic, try to read with another.
        let tmp = TempDir::new().unwrap();
        let magic_a = [0x01, 0x02, 0x03, 0x04];
        let magic_b = [0xAA, 0xBB, 0xCC, 0xDD];

        let mgr_a =
            BlockFileManager::with_params(tmp.path().to_path_buf(), magic_a, MAX_BLOCKFILE_SIZE);
        let pos = mgr_a.write_block(b"data").unwrap();

        let mgr_b =
            BlockFileManager::with_params(tmp.path().to_path_buf(), magic_b, MAX_BLOCKFILE_SIZE);
        let result = mgr_b.read_block(&pos);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("magic mismatch"),
            "unexpected error: {}",
            err_msg
        );
    }
}
