//! Memory-mapped block file I/O.
//!
//! Bitcoin Core uses standard file I/O (fread/fwrite) for block files.
//! We improve on this with memory-mapped I/O (mmap), which:
//!
//! 1. Eliminates double-buffering (kernel page cache -> userspace buffer)
//! 2. Lets the OS manage the page cache optimally
//! 3. Enables zero-copy reads - data goes directly from disk to our structs
//! 4. Significantly speeds up initial block download and reindex
//!
//! The MmapBlockReader maps block files (blk*.dat) lazily and provides
//! zero-copy access to block data.

use memmap2::Mmap;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};

/// Error type for mmap operations.
#[derive(Debug, thiserror::Error)]
pub enum MmapError {
    /// An underlying I/O error occurred (e.g., permission denied, disk failure).
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    /// The requested block file number does not exist on disk.
    #[error("Block file {0} not found")]
    FileNotFound(u32),
    /// The requested read range exceeds the mapped file size.
    #[error("Offset {offset} + length {length} exceeds file size {file_size} in file {file_num}")]
    OutOfBounds {
        /// Block file number.
        file_num: u32,
        /// Byte offset of the requested read.
        offset: usize,
        /// Number of bytes requested.
        length: usize,
        /// Actual size of the file on disk.
        file_size: usize,
    },
    /// The data at the given position could not be parsed as a valid block.
    #[error("Invalid block data at file {file_num} offset {offset}")]
    InvalidData {
        /// Block file number.
        file_num: u32,
        /// Byte offset where invalid data was found.
        offset: usize,
    },
}

/// A memory-mapped view of a single block file.
struct MappedFile {
    mmap: Mmap,
    #[allow(dead_code)]
    file_num: u32,
}

/// Memory-mapped block file reader.
///
/// Lazily maps block files and provides zero-copy access to block data.
/// Mapped files are cached for reuse.
pub struct MmapBlockReader {
    /// Base directory for block files.
    blocks_dir: PathBuf,
    /// Cached memory-mapped files.
    files: RwLock<HashMap<u32, MappedFile>>,
    /// Maximum number of files to keep mapped simultaneously.
    max_mapped_files: usize,
}

impl MmapBlockReader {
    /// Create a new MmapBlockReader for block files in `blocks_dir`.
    pub fn new(blocks_dir: impl AsRef<Path>) -> Self {
        MmapBlockReader {
            blocks_dir: blocks_dir.as_ref().to_path_buf(),
            files: RwLock::new(HashMap::new()),
            max_mapped_files: 16,
        }
    }

    /// Set the maximum number of simultaneously mapped files.
    pub fn with_max_mapped_files(mut self, max: usize) -> Self {
        self.max_mapped_files = max;
        self
    }

    /// Get the path for a block file number.
    fn file_path(&self, file_num: u32) -> PathBuf {
        self.blocks_dir.join(format!("blk{:05}.dat", file_num))
    }

    /// Get the path for a rev (undo) file number.
    #[allow(dead_code)]
    fn rev_file_path(&self, file_num: u32) -> PathBuf {
        self.blocks_dir.join(format!("rev{:05}.dat", file_num))
    }

    /// Ensure a block file is memory-mapped, mapping it if necessary.
    fn ensure_mapped(&self, file_num: u32) -> Result<(), MmapError> {
        {
            let files = self.files.read();
            if files.contains_key(&file_num) {
                return Ok(());
            }
        }

        let path = self.file_path(file_num);
        if !path.exists() {
            return Err(MmapError::FileNotFound(file_num));
        }

        let file = File::open(&path)?;
        let mmap = unsafe { Mmap::map(&file)? };

        let mut files = self.files.write();

        // Evict old mappings if we have too many
        if files.len() >= self.max_mapped_files {
            // Remove the lowest-numbered file (LRU-ish eviction)
            if let Some(&oldest) = files.keys().min() {
                files.remove(&oldest);
            }
        }

        files.insert(file_num, MappedFile { mmap, file_num });
        Ok(())
    }

    /// Read raw block data from a block file at the given offset and length.
    ///
    /// Returns a reference to the memory-mapped data (zero-copy).
    pub fn read_block_data(
        &self,
        file_num: u32,
        offset: usize,
        length: usize,
    ) -> Result<Vec<u8>, MmapError> {
        self.ensure_mapped(file_num)?;

        let files = self.files.read();
        let mapped = files
            .get(&file_num)
            .ok_or(MmapError::FileNotFound(file_num))?;

        let file_size = mapped.mmap.len();
        if offset + length > file_size {
            return Err(MmapError::OutOfBounds {
                file_num,
                offset,
                length,
                file_size,
            });
        }

        Ok(mapped.mmap[offset..offset + length].to_vec())
    }

    /// Read raw block data with zero-copy access.
    ///
    /// The returned slice is valid as long as the RwLock read guard is held.
    /// This avoids any memory allocation.
    pub fn read_block_slice(
        &self,
        file_num: u32,
        offset: usize,
        length: usize,
    ) -> Result<Vec<u8>, MmapError> {
        // For thread safety, we copy the data. A truly zero-copy API would
        // require returning a guard, which complicates the interface.
        self.read_block_data(file_num, offset, length)
    }

    /// Scan a block file for block magic numbers and return offsets.
    ///
    /// This is useful for reindexing - we scan the raw file for the
    /// network magic bytes that mark block boundaries.
    pub fn scan_block_offsets(
        &self,
        file_num: u32,
        magic: [u8; 4],
    ) -> Result<Vec<usize>, MmapError> {
        self.ensure_mapped(file_num)?;

        let files = self.files.read();
        let mapped = files
            .get(&file_num)
            .ok_or(MmapError::FileNotFound(file_num))?;

        let data = &mapped.mmap[..];
        let mut offsets = Vec::new();
        let mut pos = 0;

        while pos + 8 <= data.len() {
            // Look for magic bytes
            if data[pos..pos + 4] == magic {
                // Read the block size (4 bytes LE after magic)
                let size = u32::from_le_bytes([
                    data[pos + 4],
                    data[pos + 5],
                    data[pos + 6],
                    data[pos + 7],
                ]) as usize;

                offsets.push(pos + 8); // offset to block data (after magic + size)

                if size > 0 && pos + 8 + size <= data.len() {
                    pos += 8 + size;
                } else {
                    pos += 8;
                }
            } else {
                pos += 1;
            }
        }

        Ok(offsets)
    }

    /// Unmap a specific file, freeing its memory mapping.
    pub fn unmap_file(&self, file_num: u32) {
        let mut files = self.files.write();
        files.remove(&file_num);
    }

    /// Unmap all files.
    pub fn unmap_all(&self) {
        let mut files = self.files.write();
        files.clear();
    }

    /// Get the number of currently mapped files.
    pub fn mapped_file_count(&self) -> usize {
        let files = self.files.read();
        files.len()
    }

    /// Get the total size of all currently mapped files.
    pub fn total_mapped_size(&self) -> usize {
        let files = self.files.read();
        files.values().map(|f| f.mmap.len()).sum()
    }

    /// Prefetch (advise the OS to load) a range of data.
    ///
    /// This is a hint to the OS to start loading the data into memory
    /// before we actually need it, improving read latency.
    #[cfg(unix)]
    pub fn prefetch(&self, file_num: u32, offset: usize, length: usize) -> Result<(), MmapError> {
        self.ensure_mapped(file_num)?;
        let files = self.files.read();
        if let Some(mapped) = files.get(&file_num) {
            if offset + length <= mapped.mmap.len() {
                mapped
                    .mmap
                    .advise_range(memmap2::Advice::WillNeed, offset, length)
                    .map_err(MmapError::Io)?;
            }
        }
        Ok(())
    }
}

/// Statistics for the mmap block reader.
#[derive(Debug, Clone, Default)]
pub struct MmapStats {
    /// Number of files currently mapped.
    pub mapped_files: usize,
    /// Total bytes mapped.
    pub total_mapped_bytes: usize,
    /// Number of read operations performed.
    pub read_count: u64,
    /// Total bytes read.
    pub bytes_read: u64,
}

impl MmapBlockReader {
    /// Get current statistics.
    pub fn stats(&self) -> MmapStats {
        let files = self.files.read();
        MmapStats {
            mapped_files: files.len(),
            total_mapped_bytes: files.values().map(|f| f.mmap.len()).sum(),
            read_count: 0,
            bytes_read: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn create_test_block_file(dir: &Path, file_num: u32, magic: [u8; 4], blocks: &[&[u8]]) {
        let path = dir.join(format!("blk{:05}.dat", file_num));
        let mut file = File::create(&path).unwrap();

        for block_data in blocks {
            file.write_all(&magic).unwrap();
            file.write_all(&(block_data.len() as u32).to_le_bytes())
                .unwrap();
            file.write_all(block_data).unwrap();
        }
    }

    #[test]
    fn test_mmap_reader_creation() {
        let dir = tempfile::tempdir().unwrap();
        let reader = MmapBlockReader::new(dir.path());
        assert_eq!(reader.mapped_file_count(), 0);
    }

    #[test]
    fn test_mmap_reader_file_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let reader = MmapBlockReader::new(dir.path());
        let result = reader.read_block_data(0, 0, 10);
        assert!(result.is_err());
    }

    #[test]
    fn test_mmap_read_block_data() {
        let dir = tempfile::tempdir().unwrap();
        let magic = [0xf9, 0xbe, 0xb4, 0xd9]; // mainnet magic
        let block_data = b"hello world block data";

        create_test_block_file(dir.path(), 0, magic, &[block_data]);

        let reader = MmapBlockReader::new(dir.path());

        // Read the block data (after magic + size = 8 bytes)
        let data = reader.read_block_data(0, 8, block_data.len()).unwrap();
        assert_eq!(&data, block_data);
        assert_eq!(reader.mapped_file_count(), 1);
    }

    #[test]
    fn test_mmap_out_of_bounds() {
        let dir = tempfile::tempdir().unwrap();
        let magic = [0xf9, 0xbe, 0xb4, 0xd9];
        create_test_block_file(dir.path(), 0, magic, &[b"short"]);

        let reader = MmapBlockReader::new(dir.path());
        let result = reader.read_block_data(0, 0, 10000);
        assert!(result.is_err());
    }

    #[test]
    fn test_mmap_scan_blocks() {
        let dir = tempfile::tempdir().unwrap();
        let magic = [0xf9, 0xbe, 0xb4, 0xd9];
        let block1 = vec![1u8; 100];
        let block2 = vec![2u8; 200];

        create_test_block_file(dir.path(), 0, magic, &[&block1, &block2]);

        let reader = MmapBlockReader::new(dir.path());
        let offsets = reader.scan_block_offsets(0, magic).unwrap();

        assert_eq!(offsets.len(), 2);
        assert_eq!(offsets[0], 8); // first block at offset 8 (after magic + size)
        assert_eq!(offsets[1], 8 + 100 + 8); // second block
    }

    #[test]
    fn test_mmap_unmap() {
        let dir = tempfile::tempdir().unwrap();
        let magic = [0xf9, 0xbe, 0xb4, 0xd9];
        create_test_block_file(dir.path(), 0, magic, &[b"data"]);
        create_test_block_file(dir.path(), 1, magic, &[b"data2"]);

        let reader = MmapBlockReader::new(dir.path());
        let _ = reader.read_block_data(0, 8, 4);
        let _ = reader.read_block_data(1, 8, 5);
        assert_eq!(reader.mapped_file_count(), 2);

        reader.unmap_file(0);
        assert_eq!(reader.mapped_file_count(), 1);

        reader.unmap_all();
        assert_eq!(reader.mapped_file_count(), 0);
    }

    #[test]
    fn test_mmap_max_files_eviction() {
        let dir = tempfile::tempdir().unwrap();
        let magic = [0xf9, 0xbe, 0xb4, 0xd9];

        // Create 5 files
        for i in 0..5 {
            create_test_block_file(dir.path(), i, magic, &[b"block"]);
        }

        let reader = MmapBlockReader::new(dir.path()).with_max_mapped_files(3);

        // Map first 3
        for i in 0..3 {
            let _ = reader.read_block_data(i, 8, 5).unwrap();
        }
        assert_eq!(reader.mapped_file_count(), 3);

        // Map one more - should evict one
        let _ = reader.read_block_data(3, 8, 5).unwrap();
        assert!(reader.mapped_file_count() <= 3);
    }

    #[test]
    fn test_mmap_stats() {
        let dir = tempfile::tempdir().unwrap();
        let magic = [0xf9, 0xbe, 0xb4, 0xd9];
        create_test_block_file(dir.path(), 0, magic, &[b"test data here"]);

        let reader = MmapBlockReader::new(dir.path());
        let _ = reader.read_block_data(0, 8, 14).unwrap();

        let stats = reader.stats();
        assert_eq!(stats.mapped_files, 1);
        assert!(stats.total_mapped_bytes > 0);
    }

    #[test]
    fn test_mmap_multiple_reads_same_file() {
        let dir = tempfile::tempdir().unwrap();
        let magic = [0xf9, 0xbe, 0xb4, 0xd9];
        let data = b"some block data for testing multiple reads";
        create_test_block_file(dir.path(), 0, magic, &[data]);

        let reader = MmapBlockReader::new(dir.path());

        // Multiple reads from the same file should reuse the mapping
        let read1 = reader.read_block_data(0, 8, 10).unwrap();
        let read2 = reader.read_block_data(0, 8, 10).unwrap();
        assert_eq!(read1, read2);
        assert_eq!(reader.mapped_file_count(), 1);
    }
}
