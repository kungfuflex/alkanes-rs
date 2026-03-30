//! Compilation cache: WASM bytecode hash -> compiled SPIR-V
//!
//! Avoids recompiling contracts that have already been lowered to SPIR-V.
//! Contracts are keyed by SHA-256 of the WASM bytecode, so identical
//! bytecode always hits the cache regardless of alkane_id.

use sha2::{Sha256, Digest};
use std::collections::HashMap;
use std::path::PathBuf;

/// Two-tier cache: in-memory HashMap + optional disk persistence.
pub struct CompilationCache {
    /// In-memory cache: hash -> SPIR-V bytes
    memory: HashMap<[u8; 32], Vec<u8>>,
    /// Optional disk cache directory for persistence across restarts
    disk_path: Option<PathBuf>,
}

impl CompilationCache {
    /// Create a new cache, optionally backed by a directory on disk.
    /// If `disk_path` is Some, previously cached SPIR-V files will be
    /// loaded on first access (lazy).
    pub fn new(disk_path: Option<PathBuf>) -> Self {
        if let Some(ref path) = disk_path {
            let _ = std::fs::create_dir_all(path);
        }
        Self {
            memory: HashMap::new(),
            disk_path,
        }
    }

    /// Look up cached SPIR-V by WASM bytecode hash.
    /// Checks memory first, then falls back to disk.
    pub fn get(&mut self, wasm_hash: &[u8; 32]) -> Option<&Vec<u8>> {
        // If in memory, return it
        if self.memory.contains_key(wasm_hash) {
            return self.memory.get(wasm_hash);
        }
        // Try loading from disk
        if let Some(ref path) = self.disk_path {
            let filename = hex::encode(wasm_hash);
            let file_path = path.join(&filename);
            if let Ok(data) = std::fs::read(&file_path) {
                log::debug!("cache hit (disk): {}", filename);
                self.memory.insert(*wasm_hash, data);
                return self.memory.get(wasm_hash);
            }
        }
        None
    }

    /// Insert compiled SPIR-V into the cache, keyed by the WASM bytecode.
    /// Writes to disk if a disk path is configured.
    pub fn insert(&mut self, wasm_bytes: &[u8], spirv: Vec<u8>) {
        let hash = Self::hash_wasm(wasm_bytes);
        // Persist to disk
        if let Some(ref path) = self.disk_path {
            let filename = hex::encode(&hash);
            if let Err(e) = std::fs::write(path.join(&filename), &spirv) {
                log::warn!("failed to write SPIR-V cache file: {}", e);
            }
        }
        self.memory.insert(hash, spirv);
    }

    /// Insert compiled SPIR-V keyed by a pre-computed hash.
    pub fn insert_with_hash(&mut self, hash: [u8; 32], spirv: Vec<u8>) {
        if let Some(ref path) = self.disk_path {
            let filename = hex::encode(&hash);
            if let Err(e) = std::fs::write(path.join(&filename), &spirv) {
                log::warn!("failed to write SPIR-V cache file: {}", e);
            }
        }
        self.memory.insert(hash, spirv);
    }

    /// Check if a hash is already cached (memory or disk).
    pub fn contains(&mut self, wasm_hash: &[u8; 32]) -> bool {
        self.get(wasm_hash).is_some()
    }

    /// Number of entries currently in memory.
    pub fn len(&self) -> usize {
        self.memory.len()
    }

    /// Whether the in-memory cache is empty.
    pub fn is_empty(&self) -> bool {
        self.memory.is_empty()
    }

    /// Compute SHA-256 of WASM bytecode.
    pub fn hash_wasm(wasm_bytes: &[u8]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(wasm_bytes);
        hasher.finalize().into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_deterministic() {
        let data = b"hello wasm";
        let h1 = CompilationCache::hash_wasm(data);
        let h2 = CompilationCache::hash_wasm(data);
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_hash_different_inputs() {
        let h1 = CompilationCache::hash_wasm(b"contract_a");
        let h2 = CompilationCache::hash_wasm(b"contract_b");
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_memory_cache_roundtrip() {
        let mut cache = CompilationCache::new(None);
        let wasm = b"fake wasm bytecode";
        let spirv = vec![0x03, 0x02, 0x23, 0x07]; // SPIR-V magic

        assert!(cache.is_empty());
        cache.insert(wasm, spirv.clone());
        assert_eq!(cache.len(), 1);

        let hash = CompilationCache::hash_wasm(wasm);
        let got = cache.get(&hash).unwrap();
        assert_eq!(got, &spirv);
    }

    #[test]
    fn test_cache_miss() {
        let mut cache = CompilationCache::new(None);
        let hash = [0u8; 32];
        assert!(cache.get(&hash).is_none());
    }

    #[test]
    fn test_disk_cache_roundtrip() {
        let dir = std::env::temp_dir().join("alkanes_kiln_test_cache");
        let _ = std::fs::remove_dir_all(&dir);

        let wasm = b"disk cache test wasm";
        let spirv = vec![0x03, 0x02, 0x23, 0x07, 0xAA, 0xBB];
        let hash = CompilationCache::hash_wasm(wasm);

        // Write to cache
        {
            let mut cache = CompilationCache::new(Some(dir.clone()));
            cache.insert(wasm, spirv.clone());
        }

        // New cache instance should find it on disk
        {
            let mut cache = CompilationCache::new(Some(dir.clone()));
            assert!(cache.is_empty()); // not in memory yet
            let got = cache.get(&hash).unwrap();
            assert_eq!(got, &spirv);
            assert_eq!(cache.len(), 1); // now loaded into memory
        }

        let _ = std::fs::remove_dir_all(&dir);
    }
}
