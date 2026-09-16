//! In-memory [`FrozenStore`] — the harness driver.
//!
//! Backed by `Arc<Mutex<BTreeMap<outpoint, FrozenRecord>>>`, so cloning is
//! cheap and every clone sees the same data. That is the property that lets
//! a test hand the store to a provider AND keep a handle for direct
//! assertions.
//!
//! This exists so the freeze behaviour can be exercised end-to-end with no
//! side effects on the developer's own `~/.alkanes/frozen.sqlite3` — a test
//! that froze a real outpoint in the real store would be a test that changes
//! which coins the operator's next send is allowed to spend.

#[cfg(feature = "std")]
use std::collections::BTreeMap;
#[cfg(not(feature = "std"))]
use alloc::collections::BTreeMap;

#[cfg(feature = "std")]
use std::sync::{Arc, Mutex};
#[cfg(not(feature = "std"))]
use alloc::sync::Arc;
#[cfg(not(feature = "std"))]
use spin::Mutex;

#[cfg(not(feature = "std"))]
use alloc::{string::{String, ToString}, vec::Vec};

use async_trait::async_trait;
use anyhow::Result;

use super::{normalize_outpoint, now_secs, FrozenRecord, FrozenStore};

/// Process-local frozen set. Cheap to clone; clones share state.
#[derive(Clone, Default)]
pub struct MemoryFrozenStore {
    inner: Arc<Mutex<BTreeMap<String, FrozenRecord>>>,
}

impl MemoryFrozenStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[cfg(feature = "std")]
fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    // A poisoned lock here would mean a panic while holding the frozen set.
    // Recovering the guard is right: the map is a plain BTreeMap with no
    // cross-entry invariant to corrupt, and refusing to read it would turn
    // an unrelated panic into "nothing is frozen".
    m.lock().unwrap_or_else(|e| e.into_inner())
}

#[cfg(not(feature = "std"))]
fn lock<'a, T>(m: &'a Mutex<T>) -> spin::MutexGuard<'a, T> {
    m.lock()
}

#[async_trait(?Send)]
impl FrozenStore for MemoryFrozenStore {
    async fn freeze(&self, outpoint: &str, reason: Option<&str>) -> Result<()> {
        let key = normalize_outpoint(outpoint)?;
        let record = FrozenRecord {
            outpoint: key.clone(),
            reason: reason.map(|r| r.to_string()),
            frozen_at: now_secs(),
        };
        lock(&self.inner).insert(key, record);
        Ok(())
    }

    async fn unfreeze(&self, outpoint: &str) -> Result<bool> {
        let key = normalize_outpoint(outpoint)?;
        Ok(lock(&self.inner).remove(&key).is_some())
    }

    async fn get(&self, outpoint: &str) -> Result<Option<FrozenRecord>> {
        let key = normalize_outpoint(outpoint)?;
        Ok(lock(&self.inner).get(&key).cloned())
    }

    async fn list(&self) -> Result<Vec<FrozenRecord>> {
        Ok(lock(&self.inner).values().cloned().collect())
    }

    async fn clear(&self) -> Result<()> {
        lock(&self.inner).clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: &str = "c5520bb64d1a742a6bd62999267f683e1f0756481220ff2155d2be841a3d7b92:0";
    const B: &str = "c5520bb64d1a742a6bd62999267f683e1f0756481220ff2155d2be841a3d7b92:1";

    #[tokio::test]
    async fn a_fresh_store_is_empty() {
        let store = MemoryFrozenStore::new();
        assert!(store.list().await.unwrap().is_empty());
        assert!(store.get(A).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn freeze_then_get_returns_the_reason() {
        let store = MemoryFrozenStore::new();
        store.freeze(A, Some("holds inscription #123")).await.unwrap();

        let rec = store.get(A).await.unwrap().expect("frozen");
        assert_eq!(rec.outpoint, A);
        assert_eq!(rec.reason.as_deref(), Some("holds inscription #123"));
    }

    #[tokio::test]
    async fn freeze_without_a_reason_is_still_frozen() {
        let store = MemoryFrozenStore::new();
        store.freeze(A, None).await.unwrap();

        let rec = store.get(A).await.unwrap().expect("frozen");
        assert!(rec.reason.is_none());
    }

    #[tokio::test]
    async fn refreezing_updates_the_reason_without_duplicating() {
        let store = MemoryFrozenStore::new();
        store.freeze(A, Some("first")).await.unwrap();
        store.freeze(A, Some("second")).await.unwrap();

        assert_eq!(store.list().await.unwrap().len(), 1);
        assert_eq!(
            store.get(A).await.unwrap().unwrap().reason.as_deref(),
            Some("second")
        );
    }

    #[tokio::test]
    async fn unfreeze_reports_whether_it_did_anything() {
        // The whole point of the bool: the CLI previously printed success
        // unconditionally, so "unfroze" and "that wasn't frozen" looked
        // identical to the user.
        let store = MemoryFrozenStore::new();
        store.freeze(A, None).await.unwrap();

        assert!(store.unfreeze(A).await.unwrap(), "was frozen");
        assert!(!store.unfreeze(A).await.unwrap(), "no longer frozen");
        assert!(store.get(A).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn entries_are_independent() {
        let store = MemoryFrozenStore::new();
        store.freeze(A, Some("a")).await.unwrap();
        store.freeze(B, Some("b")).await.unwrap();
        assert_eq!(store.list().await.unwrap().len(), 2);

        store.unfreeze(A).await.unwrap();
        let left = store.list().await.unwrap();
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].outpoint, B);
    }

    #[tokio::test]
    async fn keys_are_normalized_so_the_same_coin_matches_either_spelling() {
        let store = MemoryFrozenStore::new();
        let upper = A.to_uppercase();
        store.freeze(&upper, Some("typed from an explorer")).await.unwrap();

        // Frozen via the uppercase spelling, found via the canonical one.
        assert!(store.get(A).await.unwrap().is_some());
        assert!(store.unfreeze(A).await.unwrap());
    }

    #[tokio::test]
    async fn a_malformed_outpoint_is_rejected_rather_than_stored() {
        let store = MemoryFrozenStore::new();
        assert!(store.freeze("not-an-outpoint", None).await.is_err());
        assert!(store.list().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn clones_share_state() {
        let store = MemoryFrozenStore::new();
        let handle = store.clone();
        store.freeze(A, None).await.unwrap();

        assert_eq!(handle.list().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn clear_empties_the_store() {
        let store = MemoryFrozenStore::new();
        store.freeze(A, None).await.unwrap();
        store.freeze(B, None).await.unwrap();

        store.clear().await.unwrap();
        assert!(store.list().await.unwrap().is_empty());
    }
}
