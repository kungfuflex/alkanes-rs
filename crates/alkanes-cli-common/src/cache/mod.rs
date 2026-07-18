//! Async cache layer for alkanes RPC results.
//!
//! The cache sits between [`crate::traits::JsonRpcProvider`] /
//! [`crate::traits::AlkanesProvider`] callers and the upstream metashrew /
//! subfrost endpoint. It exists to:
//!
//! * survive rate limits and indexer lag (subfrost mainnet returns
//!   `IP_RATE_LIMIT` aggressively),
//! * cut latency on immutable data (`getbytecode`, `meta`),
//! * stay reorg-safe by scoping mutable data to the current chain tip.
//!
//! The trait is intentionally generic over the value type ([`Bytes`]) so any
//! call site can serialize whatever it needs. Three backends live here:
//!
//! * [`InMemoryCache`] — LRU-bounded, in-process. The default for tests and
//!   for clients that don't want to touch disk.
//! * [`NoopCache`] — caching disabled; passes through.
//! * `SqliteCache` — persistent, opened by the `alkanes-cli` binary at
//!   `~/.alkanes/cache.sqlite3`. Lives in [`sqlite`] behind the `cache-sqlite`
//!   feature.
//! * `GrpcCache` — forwards to a remote cache server (used by
//!   `subfrost-mobile` to share Redis state). Behind `cache-grpc`.
//!
//! See [`AlkanesCache`] for the trait contract.

#[cfg(feature = "std")]
use async_trait::async_trait;
#[cfg(feature = "std")]
use core::time::Duration;

use crate::{String, Vec};

pub mod retry;

#[cfg(feature = "std")]
pub mod in_memory;
#[cfg(feature = "std")]
pub mod integration;
#[cfg(feature = "std")]
pub mod noop;

#[cfg(all(test, feature = "std"))]
mod e2e_provider_test;

#[cfg(all(feature = "std", feature = "cache-sqlite"))]
pub mod sqlite;

#[cfg(all(feature = "std", feature = "cache-grpc"))]
pub mod grpc;

#[cfg(feature = "std")]
pub use in_memory::InMemoryCache;
#[cfg(feature = "std")]
pub use noop::NoopCache;
pub use retry::{retry_transient, RetryClassifier, RetryPolicy, TransientErrorClass};

/// Opaque cache value. Each cached method serializes its return type into
/// bytes so the cache stays method-agnostic.
pub type Bytes = Vec<u8>;

/// 32-byte block hash (binary). Caller-supplied; we don't validate hex
/// formatting here.
pub type BlockHash = [u8; 32];

/// Logical "what indexer view are we asking about" axis. Keys are uniquely
/// identified by `(namespace, network, scope, suffix)`.
///
/// * [`CacheScope::Immutable`] — content can never change once the indexer
///   confirms it (contract bytecode, `meta`, fully confirmed historical
///   traces). Eligible for long TTLs.
/// * [`CacheScope::History`] — pinned to a specific block hash. Becomes a
///   miss on reorg of that block; otherwise valid forever.
/// * [`CacheScope::Tip`] — current-tip view; the value is whatever the
///   indexer thinks "right now". Invalidated when the tip hash changes
///   (see [`AlkanesCache::on_reorg`]).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CacheScope {
    Immutable,
    History(BlockHash),
    Tip(BlockHash),
}

impl CacheScope {
    pub fn tag(&self) -> &'static str {
        match self {
            CacheScope::Immutable => "imm",
            CacheScope::History(_) => "hist",
            CacheScope::Tip(_) => "tip",
        }
    }

    pub fn scope_bytes(&self) -> Option<&[u8]> {
        match self {
            CacheScope::Immutable => None,
            CacheScope::History(h) | CacheScope::Tip(h) => Some(h),
        }
    }
}

/// Structured cache key.
///
/// `namespace` is a static label per RPC method ("getbytecode", "meta",
/// "protorunesbyoutpoint", …). `network` is the chain ("mainnet", "signet",
/// "regtest"). `suffix` is the per-call discriminator — alkane id bytes for
/// getbytecode, txid:vout for protorunesbyoutpoint, etc.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CacheKey {
    pub namespace: &'static str,
    pub network: String,
    pub suffix: Vec<u8>,
}

impl CacheKey {
    pub fn new(namespace: &'static str, network: impl Into<String>, suffix: Vec<u8>) -> Self {
        Self {
            namespace,
            network: network.into(),
            suffix,
        }
    }
}

/// Cache layer errors. Backends use this for storage problems; "cache miss"
/// is `Ok(None)`, never an error.
#[derive(Debug)]
pub enum CacheError {
    Backend(String),
}

impl core::fmt::Display for CacheError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            CacheError::Backend(s) => write!(f, "cache backend error: {s}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for CacheError {}

pub type CacheResult<T> = core::result::Result<T, CacheError>;

/// Async cache trait shared by every backend.
///
/// All backends must be `Send + Sync` so callers can stash them in
/// `Arc<dyn AlkanesCache>` and share across spawn boundaries.
///
/// Backends should treat misses as `Ok(None)` and only return `Err` for
/// real I/O failures — a broken cache must never poison a query.
#[cfg(feature = "std")]
#[async_trait]
pub trait AlkanesCache: Send + Sync {
    /// Return a previously stored value for `key` at the given `scope`, or
    /// `None` for a miss. Note that scope is part of the lookup: a value
    /// stored as `Tip(A)` is invisible to a lookup at `Tip(B)`.
    async fn get(&self, key: &CacheKey, scope: &CacheScope) -> CacheResult<Option<Bytes>>;

    /// Store `value` under `(key, scope)`. `ttl` is advisory; backends that
    /// don't support timed expiry should ignore it. Pass `None` for the
    /// canonical scope-driven lifetime (Immutable: forever; History:
    /// forever-unless-reorged; Tip: until next tip change).
    async fn put(
        &self,
        key: &CacheKey,
        scope: &CacheScope,
        value: Bytes,
        ttl: Option<Duration>,
    ) -> CacheResult<()>;

    /// Drop every entry whose scope is `Tip(old_tip)` where `old_tip !=
    /// new_tip`. Backends may also drop `History(h)` entries when `h` is
    /// orphaned, but that requires knowing the new canonical chain — most
    /// backends just leave History alone and let TTL/LRU take care of it.
    ///
    /// Default implementation is a no-op; in-memory and sqlite override.
    async fn on_reorg(&self, _new_tip: BlockHash) -> CacheResult<()> {
        Ok(())
    }

    /// Hint for diagnostic logging. Backends pick a short name.
    fn backend_name(&self) -> &'static str {
        "anonymous"
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;

    #[test]
    fn cache_key_uniqueness() {
        let a = CacheKey::new("getbytecode", "mainnet", vec![1, 2, 3]);
        let b = CacheKey::new("getbytecode", "mainnet", vec![1, 2, 3]);
        let c = CacheKey::new("getbytecode", "mainnet", vec![1, 2, 4]);
        let d = CacheKey::new("getbytecode", "signet", vec![1, 2, 3]);
        let e = CacheKey::new("meta", "mainnet", vec![1, 2, 3]);
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_ne!(a, d);
        assert_ne!(a, e);
    }

    #[test]
    fn scope_distinguishes_tip_and_history() {
        let h1 = [1u8; 32];
        let h2 = [2u8; 32];
        assert_ne!(CacheScope::Tip(h1), CacheScope::Tip(h2));
        assert_ne!(CacheScope::Tip(h1), CacheScope::History(h1));
        assert_ne!(CacheScope::Immutable, CacheScope::Tip(h1));
    }
}
