//! In-memory LRU cache, scope-aware.
//!
//! Three independent buckets so a churning tip can't evict immutable
//! bytecode and historical traces stay around as long as their block stays
//! canonical. Each bucket is its own `HashMap` guarded by `tokio::sync::RwLock`,
//! with a simple FIFO-based eviction (we cap insert count per bucket and
//! drop the oldest key when full).
//!
//! This isn't a perfect LRU — implementing true LRU under tokio requires
//! either an external crate or unsafe linked-list bookkeeping. FIFO is
//! good enough for the workload (immutable entries dominate, churn is low).

use core::time::Duration;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;
use tokio::time::Instant;

use super::{AlkanesCache, BlockHash, Bytes, CacheError, CacheKey, CacheResult, CacheScope};

#[derive(Clone, Debug)]
struct Entry {
    value: Bytes,
    inserted: Instant,
    ttl: Option<Duration>,
}

impl Entry {
    fn expired(&self) -> bool {
        match self.ttl {
            None => false,
            Some(ttl) => self.inserted.elapsed() >= ttl,
        }
    }
}

#[derive(Debug)]
struct Bucket {
    entries: HashMap<(CacheKey, Option<BlockHash>), Entry>,
    order: VecDeque<(CacheKey, Option<BlockHash>)>,
    cap: usize,
}

impl Bucket {
    fn new(cap: usize) -> Self {
        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
            cap,
        }
    }

    fn get(&self, key: &(CacheKey, Option<BlockHash>)) -> Option<Bytes> {
        let e = self.entries.get(key)?;
        if e.expired() {
            return None;
        }
        Some(e.value.clone())
    }

    fn put(&mut self, key: (CacheKey, Option<BlockHash>), entry: Entry) {
        if !self.entries.contains_key(&key) {
            self.order.push_back(key.clone());
        }
        self.entries.insert(key, entry);
        while self.order.len() > self.cap {
            if let Some(old) = self.order.pop_front() {
                self.entries.remove(&old);
            }
        }
    }

    fn retain_tip(&mut self, keep: BlockHash) {
        let drop_keys: Vec<_> = self
            .entries
            .keys()
            .filter(|(_, sk)| sk.as_ref().map(|h| *h != keep).unwrap_or(false))
            .cloned()
            .collect();
        for k in &drop_keys {
            self.entries.remove(k);
        }
        self.order.retain(|k| !drop_keys.contains(k));
    }

    fn purge_expired(&mut self) {
        let drop_keys: Vec<_> = self
            .entries
            .iter()
            .filter_map(|(k, v)| if v.expired() { Some(k.clone()) } else { None })
            .collect();
        for k in &drop_keys {
            self.entries.remove(k);
        }
        self.order.retain(|k| !drop_keys.contains(k));
    }

    fn len(&self) -> usize {
        self.entries.len()
    }
}

/// Tunables for [`InMemoryCache`].
#[derive(Debug, Clone)]
pub struct InMemoryCacheConfig {
    pub immutable_cap: usize,
    pub history_cap: usize,
    pub tip_cap: usize,
}

impl Default for InMemoryCacheConfig {
    fn default() -> Self {
        Self {
            immutable_cap: 4096,
            history_cap: 2048,
            tip_cap: 512,
        }
    }
}

/// In-memory async cache. Cheap to clone (internal `Arc`).
#[derive(Clone)]
pub struct InMemoryCache {
    immutable: Arc<RwLock<Bucket>>,
    history: Arc<RwLock<Bucket>>,
    tip: Arc<RwLock<Bucket>>,
}

impl core::fmt::Debug for InMemoryCache {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("InMemoryCache").finish()
    }
}

impl InMemoryCache {
    pub fn new() -> Self {
        Self::with_config(InMemoryCacheConfig::default())
    }

    pub fn with_config(cfg: InMemoryCacheConfig) -> Self {
        Self {
            immutable: Arc::new(RwLock::new(Bucket::new(cfg.immutable_cap))),
            history: Arc::new(RwLock::new(Bucket::new(cfg.history_cap))),
            tip: Arc::new(RwLock::new(Bucket::new(cfg.tip_cap))),
        }
    }

    /// Diagnostic only — count of live entries per bucket.
    pub async fn sizes(&self) -> (usize, usize, usize) {
        let i = self.immutable.read().await.len();
        let h = self.history.read().await.len();
        let t = self.tip.read().await.len();
        (i, h, t)
    }
}

impl Default for InMemoryCache {
    fn default() -> Self {
        Self::new()
    }
}

fn map_key(key: &CacheKey, scope: &CacheScope) -> (CacheKey, Option<BlockHash>) {
    (key.clone(), scope.scope_bytes().map(|b| {
        let mut out = [0u8; 32];
        let n = b.len().min(32);
        out[..n].copy_from_slice(&b[..n]);
        out
    }))
}

#[async_trait]
impl AlkanesCache for InMemoryCache {
    async fn get(&self, key: &CacheKey, scope: &CacheScope) -> CacheResult<Option<Bytes>> {
        let mk = map_key(key, scope);
        let bucket = match scope {
            CacheScope::Immutable => &self.immutable,
            CacheScope::History(_) => &self.history,
            CacheScope::Tip(_) => &self.tip,
        };
        let guard = bucket.read().await;
        Ok(guard.get(&mk))
    }

    async fn put(
        &self,
        key: &CacheKey,
        scope: &CacheScope,
        value: Bytes,
        ttl: Option<Duration>,
    ) -> CacheResult<()> {
        let mk = map_key(key, scope);
        let entry = Entry {
            value,
            inserted: Instant::now(),
            ttl,
        };
        let bucket = match scope {
            CacheScope::Immutable => &self.immutable,
            CacheScope::History(_) => &self.history,
            CacheScope::Tip(_) => &self.tip,
        };
        let mut guard = bucket.write().await;
        guard.put(mk, entry);
        Ok(())
    }

    async fn on_reorg(&self, new_tip: BlockHash) -> CacheResult<()> {
        // Tip bucket: keep only entries scoped to new_tip. History/Immutable
        // untouched — the caller is responsible for History invalidation
        // when it knows which block actually orphaned.
        let mut guard = self.tip.write().await;
        guard.retain_tip(new_tip);
        guard.purge_expired();
        Ok(())
    }

    fn backend_name(&self) -> &'static str {
        "in-memory"
    }
}

// Workspace doesn't pull in `mockall` here yet; using AlkanesError manually if
// needed by the in-memory backend.
impl From<CacheError> for crate::AlkanesError {
    fn from(e: CacheError) -> Self {
        crate::AlkanesError::Storage(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::time::Duration;

    fn k(ns: &'static str, suffix: &[u8]) -> CacheKey {
        CacheKey::new(ns, "mainnet", suffix.to_vec())
    }

    fn hash(b: u8) -> BlockHash {
        [b; 32]
    }

    #[tokio::test]
    async fn put_then_get_immutable_hits() {
        let c = InMemoryCache::new();
        let key = k("getbytecode", b"4:512");
        c.put(&key, &CacheScope::Immutable, b"wasm".to_vec(), None)
            .await
            .unwrap();
        let got = c.get(&key, &CacheScope::Immutable).await.unwrap();
        assert_eq!(got.as_deref(), Some(b"wasm".as_ref()));
    }

    #[tokio::test]
    async fn miss_returns_none() {
        let c = InMemoryCache::new();
        let got = c
            .get(&k("getbytecode", b"4:999"), &CacheScope::Immutable)
            .await
            .unwrap();
        assert!(got.is_none());
    }

    #[tokio::test]
    async fn tip_scope_isolated_from_immutable() {
        let c = InMemoryCache::new();
        let key = k("simulate", b"4:512:get_name");
        c.put(&key, &CacheScope::Tip(hash(1)), b"FIRE".to_vec(), None)
            .await
            .unwrap();
        // Same key under Immutable should miss.
        let imm = c.get(&key, &CacheScope::Immutable).await.unwrap();
        assert!(imm.is_none());
        let tip = c.get(&key, &CacheScope::Tip(hash(1))).await.unwrap();
        assert_eq!(tip.as_deref(), Some(b"FIRE".as_ref()));
    }

    #[tokio::test]
    async fn different_tips_dont_collide() {
        let c = InMemoryCache::new();
        let key = k("simulate", b"4:512:get_name");
        c.put(&key, &CacheScope::Tip(hash(1)), b"A".to_vec(), None)
            .await
            .unwrap();
        c.put(&key, &CacheScope::Tip(hash(2)), b"B".to_vec(), None)
            .await
            .unwrap();
        let a = c.get(&key, &CacheScope::Tip(hash(1))).await.unwrap();
        let b = c.get(&key, &CacheScope::Tip(hash(2))).await.unwrap();
        assert_eq!(a.as_deref(), Some(b"A".as_ref()));
        assert_eq!(b.as_deref(), Some(b"B".as_ref()));
    }

    #[tokio::test]
    async fn on_reorg_drops_stale_tip_entries() {
        let c = InMemoryCache::new();
        let key1 = k("simulate", b"a");
        let key2 = k("simulate", b"b");
        c.put(&key1, &CacheScope::Tip(hash(1)), b"old".to_vec(), None)
            .await
            .unwrap();
        c.put(&key2, &CacheScope::Tip(hash(2)), b"new".to_vec(), None)
            .await
            .unwrap();
        c.on_reorg(hash(2)).await.unwrap();
        assert!(c.get(&key1, &CacheScope::Tip(hash(1))).await.unwrap().is_none());
        assert_eq!(
            c.get(&key2, &CacheScope::Tip(hash(2))).await.unwrap().as_deref(),
            Some(b"new".as_ref())
        );
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn ttl_expiry_observed() {
        let c = InMemoryCache::new();
        let key = k("getinventory", b"x");
        c.put(
            &key,
            &CacheScope::Tip(hash(1)),
            b"v".to_vec(),
            Some(Duration::from_secs(60)),
        )
        .await
        .unwrap();
        assert!(c.get(&key, &CacheScope::Tip(hash(1))).await.unwrap().is_some());
        tokio::time::advance(Duration::from_secs(61)).await;
        assert!(c.get(&key, &CacheScope::Tip(hash(1))).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn lru_cap_enforced() {
        let c = InMemoryCache::with_config(InMemoryCacheConfig {
            immutable_cap: 3,
            history_cap: 16,
            tip_cap: 16,
        });
        for i in 0..6u8 {
            c.put(
                &k("getbytecode", &[i]),
                &CacheScope::Immutable,
                vec![i],
                None,
            )
            .await
            .unwrap();
        }
        // Oldest 3 should have been evicted.
        let (imm, _, _) = c.sizes().await;
        assert_eq!(imm, 3);
        assert!(c.get(&k("getbytecode", &[0]), &CacheScope::Immutable).await.unwrap().is_none());
        assert!(c.get(&k("getbytecode", &[5]), &CacheScope::Immutable).await.unwrap().is_some());
    }
}
