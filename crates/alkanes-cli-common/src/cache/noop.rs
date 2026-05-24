//! Pass-through cache. Every `get` is a miss; every `put` succeeds silently.
//!
//! Used when the user passes `--no-cache` or when running in a context that
//! can't or shouldn't persist (single-shot scripts, tests that want to
//! bypass caching).

use core::time::Duration;

use async_trait::async_trait;

use super::{AlkanesCache, BlockHash, Bytes, CacheKey, CacheResult, CacheScope};

#[derive(Debug, Default, Clone, Copy)]
pub struct NoopCache;

#[async_trait]
impl AlkanesCache for NoopCache {
    async fn get(&self, _key: &CacheKey, _scope: &CacheScope) -> CacheResult<Option<Bytes>> {
        Ok(None)
    }

    async fn put(
        &self,
        _key: &CacheKey,
        _scope: &CacheScope,
        _value: Bytes,
        _ttl: Option<Duration>,
    ) -> CacheResult<()> {
        Ok(())
    }

    async fn on_reorg(&self, _new_tip: BlockHash) -> CacheResult<()> {
        Ok(())
    }

    fn backend_name(&self) -> &'static str {
        "noop"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn always_misses() {
        let c = NoopCache;
        let k = CacheKey::new("getbytecode", "mainnet", vec![1, 2, 3]);
        c.put(&k, &CacheScope::Immutable, b"x".to_vec(), None).await.unwrap();
        assert!(c.get(&k, &CacheScope::Immutable).await.unwrap().is_none());
    }
}
