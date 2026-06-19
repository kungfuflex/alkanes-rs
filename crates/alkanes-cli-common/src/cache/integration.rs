//! Glue between [`AlkanesCache`] and the existing metashrew RPC surface.
//!
//! Call sites (in `provider.rs::ConcreteProvider::metashrew_view_call` and
//! similar) consult [`cached_view_call`] instead of going straight to HTTP.
//! The helper:
//!
//! 1. Picks a cache scope based on the metashrew method name.
//! 2. Builds a [`CacheKey`] from `(method, params_hex)`.
//! 3. Returns the cached bytes on hit, skipping the network call.
//! 4. Otherwise invokes the supplied fetcher (with retry) and stores the
//!    result before returning.
//!
//! Designed to be additive — call sites that don't pass a cache see no
//! behavior change.

use core::future::Future;
use core::time::Duration;
use std::sync::Arc;

use crate::AlkanesError;

use super::{
    retry::{retry_transient, DefaultClassifier, RetryPolicy},
    AlkanesCache, BlockHash, Bytes, CacheKey, CacheScope,
};

/// Metashrew view methods we know to be content-addressed (bytecode never
/// changes for a given alkane id, ditto for `meta`).
const IMMUTABLE_VIEWS: &[&str] = &["getbytecode", "meta"];

/// View methods that are pinned to current chain state. Cached only if the
/// caller supplies a `tip_hash`.
const TIP_BOUND_VIEWS: &[&str] = &[
    "simulate",
    "protorunesbyaddress",
    "protorunesbyoutpoint",
    "getbalance",
    "getinventory",
    "getstorageat",
    "txscript",
    "unwrap",
    "trace",       // safe default; confirmed-tx traces could move to History
    "traceblock",  // ditto
];

/// Classify a metashrew_view method into a cache scope. `tip_hash` is only
/// used for [`CacheScope::Tip`]; pass `None` to skip tip-bound caching.
pub fn scope_for_method(method: &str, tip_hash: Option<BlockHash>) -> Option<CacheScope> {
    if IMMUTABLE_VIEWS.contains(&method) {
        return Some(CacheScope::Immutable);
    }
    if TIP_BOUND_VIEWS.contains(&method) {
        return tip_hash.map(CacheScope::Tip);
    }
    None
}

/// Recommended TTL for a given scope. Backends are free to ignore.
pub fn ttl_for_scope(scope: &CacheScope) -> Option<Duration> {
    match scope {
        CacheScope::Immutable => Some(Duration::from_secs(30 * 24 * 60 * 60)), // 30 days
        CacheScope::History(_) => None,                                        // forever
        CacheScope::Tip(_) => Some(Duration::from_secs(60)),                   // soft TTL
    }
}

/// Build the cache key for a metashrew_view call. `params_hex` is the hex
/// blob the indexer receives — we hash nothing, we just use it verbatim as
/// the key suffix. This stays human-readable in sqlite and keeps lookup
/// O(1) without crypto-grade hashing.
pub fn make_key(method: &'static str, network: &str, params_hex: &str) -> CacheKey {
    CacheKey::new(method, network.to_string(), params_hex.as_bytes().to_vec())
}

/// Run a metashrew_view-style call, consulting `cache` if present.
///
/// `network` is used as part of the cache key (mainnet vs signet vs regtest
/// never share entries). `tip_hash` is the current chain tip — required for
/// tip-bound methods to be cacheable; pass `None` to disable tip caching.
///
/// `method` must be a 'static str so the [`CacheKey`] namespace field stays
/// cheap. Pass it as a literal from the call site (matches what the existing
/// code does anyway).
///
/// `fetch` is the closure that performs the actual HTTP call when we miss.
/// It's invoked through [`retry_transient`] so transient upstream failures
/// (rate-limit, 502, EAGAIN) get a second chance.
pub async fn cached_view_call<F, Fut>(
    cache: Option<&Arc<dyn AlkanesCache>>,
    network: &str,
    tip_hash: Option<BlockHash>,
    method: &'static str,
    params_hex: &str,
    fetch: F,
) -> Result<Bytes, AlkanesError>
where
    // The fetcher's future doesn't have to be Send — `JsonRpcProvider::call`
    // (the production source) uses `#[async_trait(?Send)]`. The cache trait
    // itself is `Send + Sync` so its own await points are Send; we just don't
    // propagate that requirement to the closure.
    F: Fn() -> Fut,
    Fut: Future<Output = Result<Bytes, AlkanesError>>,
{
    let scope = scope_for_method(method, tip_hash);
    let key = make_key(method, network, params_hex);

    // Cache hit?
    if let (Some(cache), Some(scope)) = (cache, scope.as_ref()) {
        match cache.get(&key, scope).await {
            Ok(Some(bytes)) => {
                log::debug!(
                    "cache HIT  [{}] backend={} method={} params_len={}",
                    cache.backend_name(),
                    cache.backend_name(),
                    method,
                    params_hex.len()
                );
                return Ok(bytes);
            }
            Ok(None) => {}
            Err(e) => {
                log::warn!(
                    "cache READ error (continuing without cache): {e} method={method}"
                );
            }
        }
    }

    // Miss → fetch with retry.
    let fetched =
        retry_transient::<_, _, _, _, DefaultClassifier>(method, RetryPolicy::default(), || {
            fetch()
        })
        .await?;

    // Store on success.
    if let (Some(cache), Some(scope)) = (cache, scope) {
        let ttl = ttl_for_scope(&scope);
        if let Err(e) = cache.put(&key, &scope, fetched.clone(), ttl).await {
            log::warn!(
                "cache WRITE error (ignored): {e} method={method} backend={}",
                cache.backend_name()
            );
        }
    }

    Ok(fetched)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::in_memory::InMemoryCache;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[tokio::test]
    async fn second_call_hits_cache_for_immutable() {
        let cache: Arc<dyn AlkanesCache> = Arc::new(InMemoryCache::new());
        let calls = AtomicU32::new(0);
        let go = || {
            let calls = &calls;
            async move {
                calls.fetch_add(1, Ordering::SeqCst);
                Ok::<Bytes, AlkanesError>(b"wasm".to_vec())
            }
        };
        let a = cached_view_call(Some(&cache), "mainnet", None, "getbytecode", "0a01", go)
            .await
            .unwrap();
        let b = cached_view_call(Some(&cache), "mainnet", None, "getbytecode", "0a01", go)
            .await
            .unwrap();
        assert_eq!(a, b"wasm");
        assert_eq!(b, b"wasm");
        assert_eq!(calls.load(Ordering::SeqCst), 1, "second call should have hit cache");
    }

    #[tokio::test]
    async fn tip_bound_method_skips_cache_without_tip_hash() {
        let cache: Arc<dyn AlkanesCache> = Arc::new(InMemoryCache::new());
        let calls = AtomicU32::new(0);
        let go = || {
            let calls = &calls;
            async move {
                calls.fetch_add(1, Ordering::SeqCst);
                Ok::<Bytes, AlkanesError>(b"x".to_vec())
            }
        };
        // No tip_hash → simulate is uncacheable.
        cached_view_call(Some(&cache), "mainnet", None, "simulate", "0a01", go)
            .await
            .unwrap();
        cached_view_call(Some(&cache), "mainnet", None, "simulate", "0a01", go)
            .await
            .unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn tip_bound_method_caches_when_tip_present() {
        let cache: Arc<dyn AlkanesCache> = Arc::new(InMemoryCache::new());
        let calls = AtomicU32::new(0);
        let tip = Some([7u8; 32]);
        let go = || {
            let calls = &calls;
            async move {
                calls.fetch_add(1, Ordering::SeqCst);
                Ok::<Bytes, AlkanesError>(b"y".to_vec())
            }
        };
        cached_view_call(Some(&cache), "mainnet", tip, "simulate", "0a01", go)
            .await
            .unwrap();
        cached_view_call(Some(&cache), "mainnet", tip, "simulate", "0a01", go)
            .await
            .unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn no_cache_passes_through() {
        let calls = AtomicU32::new(0);
        let go = || {
            let calls = &calls;
            async move {
                calls.fetch_add(1, Ordering::SeqCst);
                Ok::<Bytes, AlkanesError>(b"x".to_vec())
            }
        };
        for _ in 0..3 {
            cached_view_call(None, "mainnet", None, "getbytecode", "0a01", go)
                .await
                .unwrap();
        }
        assert_eq!(calls.load(Ordering::SeqCst), 3);
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn retries_transient_error_then_succeeds() {
        let calls = AtomicU32::new(0);
        let go = || {
            let calls = &calls;
            async move {
                let n = calls.fetch_add(1, Ordering::SeqCst);
                if n < 1 {
                    Err(AlkanesError::JsonRpc("IP_RATE_LIMIT".into()))
                } else {
                    Ok(b"ok".to_vec())
                }
            }
        };
        let out = cached_view_call(None, "mainnet", None, "getbytecode", "0a01", go)
            .await
            .unwrap();
        assert_eq!(out, b"ok");
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn scope_picker() {
        assert_eq!(
            scope_for_method("getbytecode", None),
            Some(CacheScope::Immutable)
        );
        assert_eq!(scope_for_method("simulate", None), None);
        assert_eq!(
            scope_for_method("simulate", Some([1u8; 32])),
            Some(CacheScope::Tip([1u8; 32]))
        );
        assert_eq!(scope_for_method("not_a_view", Some([1u8; 32])), None);
    }
}
