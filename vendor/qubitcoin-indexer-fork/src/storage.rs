//! In-memory storage backend with read-through upstream fallback.
//!
//! `MemStorage` is the fork-mode replacement for the RocksDB-backed
//! `IndexerStorage` in the slim runtime. It holds three layers:
//!
//!   1. `overlay` — synthetic-projection writes (also serves as the
//!      write-back cache for upstream-fetched values, so a second
//!      `__get` for the same key short-circuits without another RPC).
//!   2. `appends` — per-key versioned append history mirroring the
//!      slim runtime's append-only model.
//!   3. `upstream` — an async [`crate::upstream::ForkUpstream`] that
//!      fetches values from a confirmed-state indexer (typically via
//!      `metashrew_view "getstorageat"` JSON-RPC).
//!
//! Reads consult the overlay first; on miss the host functions in
//! `runtime.rs` invoke the upstream and write the result back into
//! the overlay before returning to WASM. Writes never touch upstream.

use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

use crate::upstream::ForkUpstream;

/// In-memory KV storage with optional read-through upstream.
///
/// Implements [`qubitcoin_indexer_core::traits::IndexerStorage`] so the
/// shared rollback / state-root machinery can drive it. The host
/// functions in `runtime.rs` use the inherent methods to manage the
/// upstream cache directly (the trait surface is sync-only and can't
/// express the upstream's async-ness).
#[derive(Clone)]
pub struct MemStorage {
    /// Upstream KV source. `None` => storage is purely in-memory; misses
    /// return `None` instead of calling upstream.
    upstream: Option<Arc<dyn ForkUpstream>>,
    /// Synthetic / write-back overlay. Mutations + cached upstream hits.
    overlay: Arc<RwLock<HashMap<Vec<u8>, Vec<u8>>>>,
    /// Append history per logical key (height-tagged).
    appends: Arc<RwLock<HashMap<Vec<u8>, Vec<(u32, Vec<u8>)>>>>,
    /// Projected tip height, typically `upstream.tip_height + 1`.
    projected_height: Arc<RwLock<u32>>,
    /// Negative-cache for upstream misses, so a missing key doesn't
    /// trigger a fresh RPC on every same-block re-read.
    negative_cache: Arc<RwLock<HashMap<Vec<u8>, ()>>>,
}

impl MemStorage {
    /// Build a fork-mode storage backed by `upstream` with the given
    /// initial projected tip height.
    pub fn new(upstream: Arc<dyn ForkUpstream>, projected_height: u32) -> Self {
        Self {
            upstream: Some(upstream),
            overlay: Arc::new(RwLock::new(HashMap::new())),
            appends: Arc::new(RwLock::new(HashMap::new())),
            projected_height: Arc::new(RwLock::new(projected_height)),
            negative_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Build a pure in-memory storage with no upstream fallback. Misses
    /// return `None`. Useful for testing host-function wiring without
    /// network plumbing.
    pub fn new_local(projected_height: u32) -> Self {
        Self {
            upstream: None,
            overlay: Arc::new(RwLock::new(HashMap::new())),
            appends: Arc::new(RwLock::new(HashMap::new())),
            projected_height: Arc::new(RwLock::new(projected_height)),
            negative_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Reference to the upstream callback, if any.
    pub fn upstream(&self) -> Option<Arc<dyn ForkUpstream>> {
        self.upstream.clone()
    }

    /// Drop all overlay/append/negative-cache state. Useful when a
    /// real block lands and the projected state must restart against
    /// a fresh confirmed tip.
    pub fn reset(&self, new_projected_height: u32) {
        self.overlay.write().clear();
        self.appends.write().clear();
        self.negative_cache.write().clear();
        *self.projected_height.write() = new_projected_height;
    }

    /// Pre-seed a key in the overlay (bypasses upstream for that key).
    pub fn seed(&self, key: &[u8], value: Vec<u8>) {
        self.overlay.write().insert(key.to_vec(), value);
    }

    /// Lookup a key strictly in the overlay (no upstream fallback).
    pub fn local_get(&self, key: &[u8]) -> Option<Vec<u8>> {
        self.overlay.read().get(key).cloned()
    }

    /// Has this key been recorded as upstream-missing in the negative cache?
    pub fn is_negative_cached(&self, key: &[u8]) -> bool {
        self.negative_cache.read().contains_key(key)
    }

    /// Record an upstream hit in the write-back cache.
    pub fn cache_upstream_hit(&self, key: &[u8], value: &[u8]) {
        self.overlay.write().insert(key.to_vec(), value.to_vec());
    }

    /// Record an upstream miss so subsequent reads short-circuit.
    pub fn cache_upstream_miss(&self, key: &[u8]) {
        self.negative_cache.write().insert(key.to_vec(), ());
    }

    /// Number of keys currently in the overlay.
    pub fn overlay_len(&self) -> usize {
        self.overlay.read().len()
    }

    /// Number of keys with append history.
    pub fn appends_len(&self) -> usize {
        self.appends.read().len()
    }
}

// --- IndexerStorageReader / Writer --------------------------------------------
//
// The trait surface is sync-only. For the sync host fn we need to reach
// upstream from inside `__get`/`__get_len`, but the trait can't express
// that — so the trait impl returns overlay-only and the runtime's host
// functions short-circuit overlay first, then fall through to upstream
// via `block_in_place + Handle::current().block_on(...)` (sync linker)
// or `.await` (async linker).

impl qubitcoin_indexer_core::traits::IndexerStorageReader for MemStorage {
    fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        self.local_get(key)
    }

    fn get_latest(&self, key: &[u8]) -> Option<Vec<u8>> {
        // Check the append history's last entry first (mirrors slim
        // runtime's get_latest semantics), falling back to the
        // overlay. The runtime's host fn handles upstream miss separately.
        if let Some(history) = self.appends.read().get(key) {
            if let Some((_, v)) = history.last() {
                return Some(v.clone());
            }
        }
        self.local_get(key)
    }

    fn get_length(&self, key: &[u8]) -> u32 {
        if let Some(history) = self.appends.read().get(key) {
            return history.len() as u32;
        }
        0
    }

    fn tip_height(&self) -> u32 {
        *self.projected_height.read()
    }
}

impl qubitcoin_indexer_core::traits::IndexerStorageWriter for MemStorage {
    fn put(&self, key: &[u8], value: &[u8]) -> Result<(), String> {
        self.overlay.write().insert(key.to_vec(), value.to_vec());
        Ok(())
    }

    fn append(&self, key: &[u8], value: &[u8], height: u32) -> Result<(), String> {
        self.appends
            .write()
            .entry(key.to_vec())
            .or_insert_with(Vec::new)
            .push((height, value.to_vec()));
        // Mirror the slim runtime: appending also makes the latest value
        // visible to a flat get.
        self.overlay.write().insert(key.to_vec(), value.to_vec());
        Ok(())
    }

    fn set_tip_height(&self, height: u32) -> Result<(), String> {
        *self.projected_height.write() = height;
        Ok(())
    }

    fn delete_batch(&self, keys: &[Vec<u8>]) -> Result<(), String> {
        let mut ov = self.overlay.write();
        for k in keys {
            ov.remove(k);
        }
        Ok(())
    }
}

impl qubitcoin_indexer_core::traits::IndexerStorage for MemStorage {
    fn export_bytes(&self) -> Vec<u8> {
        // Export as a flat [count_u32_le, [keylen_u32_le, key, vlen_u32_le, val]*]
        let g = self.overlay.read();
        let mut total = 4usize;
        for (k, v) in g.iter() {
            total += 4 + k.len() + 4 + v.len();
        }
        let mut buf = Vec::with_capacity(total);
        buf.extend_from_slice(&(g.len() as u32).to_le_bytes());
        for (k, v) in g.iter() {
            buf.extend_from_slice(&(k.len() as u32).to_le_bytes());
            buf.extend_from_slice(k);
            buf.extend_from_slice(&(v.len() as u32).to_le_bytes());
            buf.extend_from_slice(v);
        }
        buf
    }

    fn import_bytes(&self, data: &[u8]) -> Result<usize, String> {
        if data.len() < 4 {
            return Err("import_bytes: truncated header".into());
        }
        let count = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
        let mut pos = 4usize;
        let mut imported = 0usize;
        let mut new_overlay: HashMap<Vec<u8>, Vec<u8>> = HashMap::with_capacity(count);
        for _ in 0..count {
            if pos + 4 > data.len() {
                return Err("import_bytes: truncated key length".into());
            }
            let klen =
                u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                    as usize;
            pos += 4;
            if pos + klen > data.len() {
                return Err("import_bytes: truncated key".into());
            }
            let k = data[pos..pos + klen].to_vec();
            pos += klen;
            if pos + 4 > data.len() {
                return Err("import_bytes: truncated value length".into());
            }
            let vlen =
                u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                    as usize;
            pos += 4;
            if pos + vlen > data.len() {
                return Err("import_bytes: truncated value".into());
            }
            let v = data[pos..pos + vlen].to_vec();
            pos += vlen;
            new_overlay.insert(k, v);
            imported += 1;
        }
        *self.overlay.write() = new_overlay;
        Ok(imported)
    }

    fn keys_with_lengths(&self) -> Vec<(Vec<u8>, u32)> {
        // Combines append-history keys (with their lengths) and overlay
        // keys (length 1 if absent from history).
        let mut out: HashMap<Vec<u8>, u32> = HashMap::new();
        for (k, h) in self.appends.read().iter() {
            out.insert(k.clone(), h.len() as u32);
        }
        for k in self.overlay.read().keys() {
            out.entry(k.clone()).or_insert(1);
        }
        out.into_iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::upstream::testing::StubUpstream;
    use qubitcoin_indexer_core::traits::{
        IndexerStorage, IndexerStorageReader, IndexerStorageWriter,
    };

    fn upstream_with(pairs: &[(&[u8], &[u8])]) -> Arc<dyn ForkUpstream> {
        let map: HashMap<Vec<u8>, Vec<u8>> =
            pairs.iter().map(|(k, v)| (k.to_vec(), v.to_vec())).collect();
        Arc::new(StubUpstream::new(map))
    }

    #[test]
    fn local_get_returns_overlay_when_present() {
        let s = MemStorage::new(upstream_with(&[]), 100);
        s.put(b"k", b"v").unwrap();
        assert_eq!(s.local_get(b"k"), Some(b"v".to_vec()));
    }

    #[test]
    fn local_get_returns_none_for_upstream_only_keys() {
        // local_get is overlay-only; the runtime's host fn handles
        // the upstream fall-through.
        let s = MemStorage::new(upstream_with(&[(b"k", b"upstream")]), 100);
        assert_eq!(s.local_get(b"k"), None);
    }

    #[test]
    fn cache_upstream_hit_makes_local_get_succeed() {
        let s = MemStorage::new(upstream_with(&[]), 100);
        s.cache_upstream_hit(b"x", b"y");
        assert_eq!(s.local_get(b"x"), Some(b"y".to_vec()));
    }

    #[test]
    fn cache_upstream_miss_records_in_negative_cache() {
        let s = MemStorage::new(upstream_with(&[]), 100);
        assert!(!s.is_negative_cached(b"absent"));
        s.cache_upstream_miss(b"absent");
        assert!(s.is_negative_cached(b"absent"));
    }

    #[test]
    fn reset_clears_overlay_and_appends() {
        let s = MemStorage::new(upstream_with(&[]), 10);
        s.put(b"a", b"1").unwrap();
        s.append(b"b", b"v1", 5).unwrap();
        s.cache_upstream_miss(b"miss");
        assert_eq!(s.overlay_len(), 2); // a + b (append also writes overlay)
        assert_eq!(s.appends_len(), 1);
        assert!(s.is_negative_cached(b"miss"));
        s.reset(20);
        assert_eq!(s.overlay_len(), 0);
        assert_eq!(s.appends_len(), 0);
        assert!(!s.is_negative_cached(b"miss"));
        assert_eq!(s.tip_height(), 20);
    }

    #[test]
    fn overlay_write_shadows_upstream() {
        let s = MemStorage::new(upstream_with(&[(b"k", b"upstream")]), 100);
        s.put(b"k", b"overlay").unwrap();
        assert_eq!(s.local_get(b"k"), Some(b"overlay".to_vec()));
    }

    #[test]
    fn delete_batch_removes_overlay_keys() {
        let s = MemStorage::new(upstream_with(&[]), 100);
        s.put(b"a", b"1").unwrap();
        s.put(b"b", b"2").unwrap();
        s.delete_batch(&[b"a".to_vec()]).unwrap();
        assert_eq!(s.local_get(b"a"), None);
        assert_eq!(s.local_get(b"b"), Some(b"2".to_vec()));
    }

    #[test]
    fn append_grows_length_and_get_latest() {
        let s = MemStorage::new(upstream_with(&[]), 1);
        s.append(b"k", b"v1", 1).unwrap();
        s.append(b"k", b"v2", 2).unwrap();
        s.append(b"k", b"v3", 3).unwrap();
        assert_eq!(s.get_length(b"k"), 3);
        assert_eq!(s.get_latest(b"k"), Some(b"v3".to_vec()));
    }

    #[test]
    fn tip_height_from_constructor_is_returned() {
        let s = MemStorage::new(upstream_with(&[]), 12345);
        assert_eq!(s.tip_height(), 12345);
    }

    #[test]
    fn set_tip_height_overrides_initial() {
        let s = MemStorage::new(upstream_with(&[]), 100);
        s.set_tip_height(150).unwrap();
        assert_eq!(s.tip_height(), 150);
    }

    #[test]
    fn export_import_roundtrip() {
        let s = MemStorage::new(upstream_with(&[]), 1);
        s.put(b"alpha", b"a").unwrap();
        s.put(b"beta", b"b").unwrap();
        let bytes = s.export_bytes();

        let s2 = MemStorage::new(upstream_with(&[]), 99);
        let n = s2.import_bytes(&bytes).unwrap();
        assert_eq!(n, 2);
        assert_eq!(s2.local_get(b"alpha"), Some(b"a".to_vec()));
        assert_eq!(s2.local_get(b"beta"), Some(b"b".to_vec()));
    }

    #[test]
    fn import_bytes_rejects_truncated_data() {
        let s = MemStorage::new(upstream_with(&[]), 1);
        // Header says 1 entry but body is empty.
        let bad = vec![1u8, 0, 0, 0];
        assert!(s.import_bytes(&bad).is_err());
    }

    #[test]
    fn keys_with_lengths_includes_overlay_and_appends() {
        let s = MemStorage::new(upstream_with(&[]), 1);
        s.put(b"flat", b"x").unwrap();
        s.append(b"hist", b"v1", 1).unwrap();
        s.append(b"hist", b"v2", 2).unwrap();
        let kw: HashMap<Vec<u8>, u32> = s.keys_with_lengths().into_iter().collect();
        assert_eq!(kw.get(b"flat".as_slice()), Some(&1));
        assert_eq!(kw.get(b"hist".as_slice()), Some(&2));
    }

    #[test]
    fn new_local_has_no_upstream() {
        let s = MemStorage::new_local(42);
        assert!(s.upstream().is_none());
        assert_eq!(s.tip_height(), 42);
        assert_eq!(s.local_get(b"any"), None);
    }
}
