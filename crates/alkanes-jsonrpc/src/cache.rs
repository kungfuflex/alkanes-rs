//! Shared Redis cache for `metashrew_view` responses.
//!
//! Cache key: `v1:mv:` + blake3(method || params[0] || params[1] || block_hash_hex).
//! Block_tag is resolved to a stable block_hash BEFORE keying — so "latest" /
//! missing / numeric all converge on the same key once they pin to the same
//! chain state. This makes the cache reorg-safe by construction:
//!
//!   * Reorgs produce a new block_hash for the affected height(s).
//!   * Cache lookups for the new chain miss → upstream → store fresh entries.
//!   * Orphaned-chain entries are still keyed by the OLD hash, which no
//!     subsequent request will ever ask for, so they age out via LRU.
//!
//! No active invalidation is required. The only required mutation is the
//! local `(height → hash)` map, which the background watermark refresher
//! drops if it detects a hash change at the current served_height.
//!
//! Failure modes: Redis errors, block-hash resolution errors, or oversized
//! responses (>1 MiB) all degrade silently to passthrough — the request
//! still completes via the existing upstream path, just without caching.

use crate::config::Config;
use crate::jsonrpc::{JsonRpcRequest, JsonRpcResponse};
use anyhow::{anyhow, Result};
use log::{debug, info, warn};
use lru::LruCache;
use redis::aio::ConnectionManager;
use redis::AsyncCommands;
use serde_json::Value;
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock};

const KEY_PREFIX: &str = "v1:mv:";
const MAX_VALUE_BYTES: usize = 1024 * 1024; // 1 MiB
const HEIGHT_HASH_LRU_CAP: usize = 10_000;
// Refresh the (served_height, hash) watermark every 250ms. Under burst load
// this lets concurrent "latest" requests share one upstream call instead of
// each issuing its own pair of metashrew_height + metashrew_getblockhash.
const WATERMARK_REFRESH_INTERVAL_MS: u64 = 250;
// A "latest" request will use the cached watermark if it's <= STALENESS_MS
// old; otherwise it falls back to a fresh fetch. With 250ms refresh and
// 500ms staleness, the refresher has 2 ticks to land before the staleness
// gate trips — accommodates one missed tick under load.
const WATERMARK_STALENESS_MS: u64 = 500;
const LOG_STATS_EVERY: u64 = 1000;

/// The monotonic served_height watermark + its current block_hash.
/// Refreshed every WATERMARK_REFRESH_INTERVAL_MS by the background task.
#[derive(Debug, Clone)]
struct Watermark {
    served_height: u64,
    served_hash: [u8; 32],
    refreshed_at: Instant,
}

/// Aggregated counters logged every LOG_STATS_EVERY total requests.
#[derive(Default)]
struct Stats {
    hits: AtomicU64,
    misses: AtomicU64,
    errors: AtomicU64,
    skipped: AtomicU64, // cache skipped (oversized, unresolved tag, etc.)
}

pub struct MetashrewViewCache {
    conn: ConnectionManager,
    watermark: Arc<RwLock<Option<Watermark>>>,
    height_to_hash: Arc<Mutex<LruCache<u64, [u8; 32]>>>,
    metashrew_url: String,
    http: reqwest::Client,
    stats: Arc<Stats>,
}

impl MetashrewViewCache {
    /// Build a cache from config. Returns Err if the Redis URL can't be
    /// parsed or the initial connection fails — caller should log and
    /// proceed without the cache (passthrough mode).
    pub async fn from_config(config: &Config) -> Result<Self> {
        let redis_url = config
            .redis_url
            .as_deref()
            .ok_or_else(|| anyhow!("REDIS_URL not configured"))?;
        let client = redis::Client::open(redis_url)?;
        let conn = ConnectionManager::new(client).await?;
        info!("metashrew_view cache: connected to {}", redis_url);
        Ok(Self {
            conn,
            watermark: Arc::new(RwLock::new(None)),
            height_to_hash: Arc::new(Mutex::new(LruCache::new(
                NonZeroUsize::new(HEIGHT_HASH_LRU_CAP).unwrap(),
            ))),
            metashrew_url: config.metashrew_url.clone(),
            http: reqwest::Client::new(),
            stats: Arc::new(Stats::default()),
        })
    }

    /// Resolve a JSON-RPC `block_tag` to (height, block_hash). Handles:
    ///   * `Value::Null` or missing → cached watermark (refreshed every
    ///     250ms); falls back to fresh fetch on cold-start.
    ///   * `"latest"` → same as missing
    ///   * `"<N>"` (decimal string) → numeric height, local LRU then
    ///     upstream metashrew_getblockhash on miss
    ///   * `Number(N)` → same
    ///
    /// REORG SAFETY: the watermark is refreshed every 250ms by a
    /// background task that also detects reorgs (block_hash mismatch at
    /// current served_height) and drops the local (height → hash) LRU
    /// when one is detected. Worst case: a request that lands in the
    /// ~250ms window between a reorg landing on metashrew and the
    /// refresher noticing serves a `block_hash` from the orphaned chain.
    /// Cache lookups keyed by that stale hash do return data correct
    /// FOR THAT CHAIN VERSION (the data was committed when that hash
    /// was tip) — so the user sees a snapshot that's at most 250ms
    /// stale, not arbitrary stale-empty state.
    ///
    /// This is a tradeoff with the "verify-on-every-lookup" design that
    /// added 2 upstream RPCs (metashrew_height + metashrew_getblockhash)
    /// per "latest" request. Under burst load those calls queue behind
    /// view-call semaphore traffic on metashrew, ballooning latency to
    /// 1-3s per request and degrading the whole service. The cached
    /// watermark cuts the per-request upstream cost to ~0 RPCs and
    /// preserves correctness within a bounded window.
    pub async fn resolve_block_hash(&self, block_tag: Option<&Value>) -> Result<(u64, [u8; 32])> {
        let want_latest = match block_tag {
            None => true,
            Some(Value::Null) => true,
            Some(Value::String(s)) if s == "latest" => true,
            _ => false,
        };
        if want_latest {
            // Fast path: cached watermark within freshness window.
            if let Ok((h, hash)) = self.read_watermark().await {
                return Ok((h, hash));
            }
            // Cold-start / refresher hasn't run yet → fresh fetch + seed
            // the watermark so subsequent requests hit the cache.
            let height = self.fetch_served_height().await?;
            let hash = self.fetch_block_hash_at(height).await?;
            *self.watermark.write().await = Some(Watermark {
                served_height: height,
                served_hash: hash,
                refreshed_at: Instant::now(),
            });
            self.height_to_hash.lock().await.put(height, hash);
            return Ok((height, hash));
        }

        // Numeric (string or number) — explicit heights are immutable
        // (until reorg, which the refresher detects + handles).
        let height: u64 = match block_tag {
            Some(Value::String(s)) => s
                .parse::<u64>()
                .map_err(|e| anyhow!("non-numeric block_tag {:?}: {}", s, e))?,
            Some(Value::Number(n)) => n
                .as_u64()
                .ok_or_else(|| anyhow!("block_tag number out of u64 range: {}", n))?,
            other => return Err(anyhow!("unsupported block_tag shape: {:?}", other)),
        };

        // Local LRU hit?
        {
            let mut cache = self.height_to_hash.lock().await;
            if let Some(hash) = cache.get(&height) {
                return Ok((height, *hash));
            }
        }

        // Upstream lookup.
        let hash = self.fetch_block_hash_at(height).await?;
        {
            let mut cache = self.height_to_hash.lock().await;
            cache.put(height, hash);
        }
        Ok((height, hash))
    }

    /// Lookup a metashrew_view response in Redis. Returns Ok(Some(..)) on
    /// hit, Ok(None) on miss, Err on Redis failure.
    pub async fn lookup(
        &self,
        request: &JsonRpcRequest,
        block_hash: &[u8; 32],
    ) -> Result<Option<JsonRpcResponse>> {
        let key = build_key(request, block_hash);
        let mut conn = self.conn.clone();
        let bytes: Option<Vec<u8>> = conn.get(&key).await?;
        let Some(bytes) = bytes else {
            self.stats.misses.fetch_add(1, Ordering::Relaxed);
            self.maybe_log_stats();
            return Ok(None);
        };
        // Stored payload is the JSON-encoded `result` Value. Wrap it back
        // into a Success with the request's current id (id changes per
        // request, can't be part of the cached value).
        let result: Value = serde_json::from_slice(&bytes)?;
        self.stats.hits.fetch_add(1, Ordering::Relaxed);
        self.maybe_log_stats();
        Ok(Some(JsonRpcResponse::success(result, request.id.clone())))
    }

    /// Store a successful response in Redis. Fire-and-forget — Redis errors
    /// log a warning but don't affect the in-flight response. No-op for
    /// error responses or oversized payloads.
    pub async fn store(
        &self,
        request: &JsonRpcRequest,
        response: &JsonRpcResponse,
        block_hash: &[u8; 32],
    ) {
        let result = match response {
            JsonRpcResponse::Success { result, .. } => result,
            JsonRpcResponse::Error { .. } => return, // never cache errors
        };
        let bytes = match serde_json::to_vec(result) {
            Ok(b) => b,
            Err(e) => {
                warn!("metashrew_view cache: serialize for store failed: {}", e);
                return;
            }
        };
        if bytes.len() > MAX_VALUE_BYTES {
            self.stats.skipped.fetch_add(1, Ordering::Relaxed);
            return;
        }
        let key = build_key(request, block_hash);
        let mut conn = self.conn.clone();
        // SET with no TTL — entries are immutable per block_hash; LRU
        // eviction handles capacity. Redis must be configured with
        // maxmemory-policy=allkeys-lru in the cluster (which redis-0
        // already is per the operator default).
        if let Err(e) = conn.set::<_, _, ()>(&key, &bytes).await {
            warn!("metashrew_view cache: SET failed: {}", e);
            self.stats.errors.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Spawn a background tokio task that refreshes the served_height
    /// watermark + its block_hash every WATERMARK_REFRESH_INTERVAL_MS.
    /// If the hash at served_height changes (reorg), the entire local
    /// height_to_hash map is dropped so subsequent explicit-height lookups
    /// re-fetch from upstream.
    pub fn start_watermark_refresher(self: &Arc<Self>) {
        let me = Arc::clone(self);
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(Duration::from_millis(
                WATERMARK_REFRESH_INTERVAL_MS,
            ));
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                ticker.tick().await;
                if let Err(e) = me.refresh_watermark().await {
                    debug!("metashrew_view cache: watermark refresh failed: {}", e);
                }
            }
        });
        info!(
            "metashrew_view cache: watermark refresher started ({}ms interval)",
            WATERMARK_REFRESH_INTERVAL_MS
        );
    }

    async fn read_watermark(&self) -> Result<(u64, [u8; 32])> {
        let guard = self.watermark.read().await;
        match guard.as_ref() {
            Some(w) if w.refreshed_at.elapsed() < Duration::from_millis(WATERMARK_STALENESS_MS) => {
                Ok((w.served_height, w.served_hash))
            }
            _ => Err(anyhow!("watermark unavailable or stale (>5s old)")),
        }
    }

    async fn refresh_watermark(&self) -> Result<()> {
        let height = self.fetch_served_height().await?;
        let hash = self.fetch_block_hash_at(height).await?;
        // Reorg detection: if we have a previous watermark and the new
        // height-hash combo for the SAME height differs, the chain reorged
        // at or below this height. Drop the local map.
        {
            let prev = self.watermark.read().await.clone();
            if let Some(p) = prev {
                if p.served_height == height && p.served_hash != hash {
                    warn!(
                        "metashrew_view cache: reorg detected at height {} (hash changed); clearing local height_to_hash map",
                        height
                    );
                    self.height_to_hash.lock().await.clear();
                }
            }
        }
        let mut w = self.watermark.write().await;
        *w = Some(Watermark {
            served_height: height,
            served_hash: hash,
            refreshed_at: Instant::now(),
        });
        // Also seed the height_to_hash map with the current watermark so
        // explicit-height queries at the tip hit local immediately.
        self.height_to_hash.lock().await.put(height, hash);
        Ok(())
    }

    async fn fetch_served_height(&self) -> Result<u64> {
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "metashrew_height",
            "params": []
        });
        let r: Value = self
            .http
            .post(&self.metashrew_url)
            .json(&body)
            .send()
            .await?
            .json()
            .await?;
        let s = r
            .get("result")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("metashrew_height: no string result in {}", r))?;
        s.parse::<u64>()
            .map_err(|e| anyhow!("metashrew_height: result {:?} not a number: {}", s, e))
    }

    async fn fetch_block_hash_at(&self, height: u64) -> Result<[u8; 32]> {
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "metashrew_getblockhash",
            "params": [height]
        });
        let r: Value = self
            .http
            .post(&self.metashrew_url)
            .json(&body)
            .send()
            .await?
            .json()
            .await?;
        let s = r
            .get("result")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("metashrew_getblockhash({}): no string result in {}", height, r))?;
        // Result is a 0x-prefixed 32-byte hex string (or just hex).
        let s = s.trim_start_matches("0x");
        let bytes = hex::decode(s)
            .map_err(|e| anyhow!("metashrew_getblockhash({}): hex decode failed: {}", height, e))?;
        if bytes.len() != 32 {
            return Err(anyhow!(
                "metashrew_getblockhash({}): expected 32 bytes, got {}",
                height,
                bytes.len()
            ));
        }
        let mut out = [0u8; 32];
        out.copy_from_slice(&bytes);
        Ok(out)
    }

    fn maybe_log_stats(&self) {
        let hits = self.stats.hits.load(Ordering::Relaxed);
        let misses = self.stats.misses.load(Ordering::Relaxed);
        let total = hits + misses;
        if total > 0 && total % LOG_STATS_EVERY == 0 {
            let errors = self.stats.errors.load(Ordering::Relaxed);
            let skipped = self.stats.skipped.load(Ordering::Relaxed);
            let pct = (hits as f64) / (total as f64) * 100.0;
            info!(
                "metashrew_view cache: total={} hits={} ({:.1}%) misses={} errors={} skipped={}",
                total, hits, pct, misses, errors, skipped
            );
        }
    }
}

/// Build the cache key for a (request, block_hash) pair. The id is excluded
/// (varies per request); the block_tag is excluded (already resolved into
/// the block_hash component).
fn build_key(request: &JsonRpcRequest, block_hash: &[u8; 32]) -> String {
    let fn_name = request
        .params
        .get(0)
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let arg_hex = request
        .params
        .get(1)
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let mut h = blake3::Hasher::new();
    h.update(request.method.as_bytes());
    h.update(b"\x00");
    h.update(fn_name.as_bytes());
    h.update(b"\x00");
    h.update(arg_hex.as_bytes());
    h.update(b"\x00");
    h.update(block_hash);
    let digest = h.finalize();
    format!("{}{}", KEY_PREFIX, digest.to_hex())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn req(method: &str, params: Vec<Value>) -> JsonRpcRequest {
        JsonRpcRequest {
            jsonrpc: "2.0".into(),
            method: method.into(),
            params,
            id: json!(1),
        }
    }

    #[test]
    fn build_key_is_deterministic_and_id_independent() {
        let h = [0xab; 32];
        let r1 = req("metashrew_view", vec![json!("simulate"), json!("0xdeadbeef"), json!("latest")]);
        let mut r2 = r1.clone();
        r2.id = json!("different");
        // Different block_tag in r2 should still produce same key
        // (block_tag is resolved away before keying).
        r2.params[2] = json!("950000");
        assert_eq!(build_key(&r1, &h), build_key(&r2, &h));
    }

    #[test]
    fn build_key_differs_per_block_hash() {
        let h1 = [0xab; 32];
        let h2 = [0xcd; 32];
        let r = req("metashrew_view", vec![json!("simulate"), json!("0xdeadbeef")]);
        assert_ne!(build_key(&r, &h1), build_key(&r, &h2));
    }

    #[test]
    fn build_key_differs_per_args() {
        let h = [0xab; 32];
        let r1 = req("metashrew_view", vec![json!("simulate"), json!("0xaaaa")]);
        let r2 = req("metashrew_view", vec![json!("simulate"), json!("0xbbbb")]);
        assert_ne!(build_key(&r1, &h), build_key(&r2, &h));
    }
}
