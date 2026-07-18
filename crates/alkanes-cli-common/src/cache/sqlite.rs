//! Sqlite-backed [`AlkanesCache`] for persistent, on-disk caching.
//!
//! Opened by the `alkanes-cli` binary at `~/.alkanes/cache.sqlite3` by
//! default. Schema is one row per `(namespace, network, scope_tag,
//! key_suffix, scope_key)`. `scope_key` is the binary tip-hash for `Tip` or
//! block-hash for `History`, NULL for `Immutable`. We rely on sqlx-sqlite's
//! native async runtime (no `spawn_blocking` dance).
//!
//! Available behind the `cache-sqlite` feature.

use core::time::Duration;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Pool, Sqlite};

use super::{AlkanesCache, BlockHash, Bytes, CacheError, CacheKey, CacheResult, CacheScope};

/// DDL applied at open time. Each entry is one statement (no embedded
/// semicolons) — sqlx's `query()` only accepts a single statement at a time,
/// so we apply them sequentially rather than relying on string-splitting.
const SCHEMA_STATEMENTS: &[&str] = &[
    "CREATE TABLE IF NOT EXISTS cache_entries ( \
        namespace   TEXT NOT NULL, \
        network     TEXT NOT NULL, \
        scope_tag   TEXT NOT NULL, \
        key_suffix  BLOB NOT NULL, \
        scope_key   BLOB, \
        value       BLOB NOT NULL, \
        inserted    INTEGER NOT NULL, \
        ttl_secs    INTEGER, \
        PRIMARY KEY (namespace, network, scope_tag, key_suffix, scope_key) \
     )",
    "CREATE INDEX IF NOT EXISTS idx_cache_scope_tip \
        ON cache_entries(scope_tag, scope_key) \
        WHERE scope_tag = 'tip'",
];

/// Persistent cache. Cheap to clone (`Arc<Pool>` internally).
#[derive(Clone)]
pub struct SqliteCache {
    pool: Pool<Sqlite>,
}

impl core::fmt::Debug for SqliteCache {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SqliteCache").finish()
    }
}

fn err<E: core::fmt::Display>(e: E) -> CacheError {
    CacheError::Backend(e.to_string())
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

impl SqliteCache {
    /// Open the cache at `path`, creating it (and parent dirs) if missing.
    /// Runs migrations on first open.
    pub async fn open(path: impl AsRef<Path>) -> CacheResult<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(err)?;
        }
        let url = format!("sqlite://{}?mode=rwc", path.display());
        let opts: SqliteConnectOptions = url.parse().map_err(err)?;
        let opts = opts
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .synchronous(sqlx::sqlite::SqliteSynchronous::Normal)
            .busy_timeout(Duration::from_secs(5));
        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(opts)
            .await
            .map_err(err)?;
        for stmt in SCHEMA_STATEMENTS {
            sqlx::query(stmt).execute(&pool).await.map_err(err)?;
        }
        Ok(Self { pool })
    }

    /// Default location: `~/.alkanes/cache.sqlite3`. Falls back to the
    /// current dir if the home directory can't be resolved.
    pub async fn open_default() -> CacheResult<Self> {
        let path = default_cache_path();
        Self::open(path).await
    }

    /// Drop entries older than `older_than` regardless of scope. Useful for
    /// occasional house-cleaning; the cache is otherwise grow-only.
    pub async fn purge_older_than(&self, older_than: Duration) -> CacheResult<u64> {
        let cutoff = now_secs() - older_than.as_secs() as i64;
        let r = sqlx::query("DELETE FROM cache_entries WHERE inserted < ?")
            .bind(cutoff)
            .execute(&self.pool)
            .await
            .map_err(err)?;
        Ok(r.rows_affected())
    }
}

/// Resolve `~/.alkanes/cache.sqlite3` honoring `ALKANES_CACHE_DIR` env var.
pub fn default_cache_path() -> PathBuf {
    if let Ok(dir) = std::env::var("ALKANES_CACHE_DIR") {
        return PathBuf::from(dir).join("cache.sqlite3");
    }
    if let Some(home) = dirs::home_dir() {
        return home.join(".alkanes").join("cache.sqlite3");
    }
    PathBuf::from(".alkanes-cache.sqlite3")
}

#[async_trait]
impl AlkanesCache for SqliteCache {
    async fn get(&self, key: &CacheKey, scope: &CacheScope) -> CacheResult<Option<Bytes>> {
        let scope_key: Option<Vec<u8>> = scope.scope_bytes().map(|b| b.to_vec());
        let row: Option<(Vec<u8>, i64, Option<i64>)> = sqlx::query_as(
            r#"
            SELECT value, inserted, ttl_secs
            FROM cache_entries
            WHERE namespace = ? AND network = ? AND scope_tag = ?
              AND key_suffix = ? AND (scope_key IS ? OR scope_key = ?)
            "#,
        )
        .bind(key.namespace)
        .bind(&key.network)
        .bind(scope.tag())
        .bind(&key.suffix)
        // The double-bind covers `scope_key IS NULL` for Immutable and
        // `scope_key = ?` for Tip/History. We bind the same value twice; the
        // first becomes the IS-NULL test (works in sqlite when null is passed).
        .bind(scope_key.as_deref())
        .bind(scope_key.as_deref())
        .fetch_optional(&self.pool)
        .await
        .map_err(err)?;

        let Some((value, inserted, ttl_secs)) = row else {
            return Ok(None);
        };
        if let Some(ttl) = ttl_secs {
            if now_secs() >= inserted + ttl {
                return Ok(None);
            }
        }
        Ok(Some(value))
    }

    async fn put(
        &self,
        key: &CacheKey,
        scope: &CacheScope,
        value: Bytes,
        ttl: Option<Duration>,
    ) -> CacheResult<()> {
        let scope_key: Option<Vec<u8>> = scope.scope_bytes().map(|b| b.to_vec());
        let ttl_secs: Option<i64> = ttl.map(|d| d.as_secs() as i64);
        sqlx::query(
            r#"
            INSERT INTO cache_entries
                (namespace, network, scope_tag, key_suffix, scope_key, value, inserted, ttl_secs)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(namespace, network, scope_tag, key_suffix, scope_key) DO UPDATE SET
                value = excluded.value,
                inserted = excluded.inserted,
                ttl_secs = excluded.ttl_secs
            "#,
        )
        .bind(key.namespace)
        .bind(&key.network)
        .bind(scope.tag())
        .bind(&key.suffix)
        .bind(scope_key)
        .bind(value)
        .bind(now_secs())
        .bind(ttl_secs)
        .execute(&self.pool)
        .await
        .map_err(err)?;
        Ok(())
    }

    async fn on_reorg(&self, new_tip: BlockHash) -> CacheResult<()> {
        sqlx::query(
            r#"DELETE FROM cache_entries WHERE scope_tag = 'tip' AND scope_key IS NOT ?"#,
        )
        .bind(new_tip.to_vec())
        .execute(&self.pool)
        .await
        .map_err(err)?;
        Ok(())
    }

    fn backend_name(&self) -> &'static str {
        "sqlite"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn k(ns: &'static str, suffix: &[u8]) -> CacheKey {
        CacheKey::new(ns, "mainnet", suffix.to_vec())
    }

    #[tokio::test]
    async fn open_creates_db_and_roundtrips() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("cache.sqlite3");
        let c = SqliteCache::open(&path).await.unwrap();
        let key = k("getbytecode", b"4:512");
        c.put(&key, &CacheScope::Immutable, b"wasmbody".to_vec(), None)
            .await
            .unwrap();
        let got = c.get(&key, &CacheScope::Immutable).await.unwrap();
        assert_eq!(got.as_deref(), Some(b"wasmbody".as_ref()));
        // File actually exists.
        assert!(path.exists(), "sqlite file should be created");
    }

    #[tokio::test]
    async fn survives_reopen() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("cache.sqlite3");
        {
            let c = SqliteCache::open(&path).await.unwrap();
            c.put(
                &k("getbytecode", b"4:512"),
                &CacheScope::Immutable,
                b"wasmbody".to_vec(),
                None,
            )
            .await
            .unwrap();
        }
        let c2 = SqliteCache::open(&path).await.unwrap();
        let got = c2
            .get(&k("getbytecode", b"4:512"), &CacheScope::Immutable)
            .await
            .unwrap();
        assert_eq!(got.as_deref(), Some(b"wasmbody".as_ref()));
    }

    #[tokio::test]
    async fn reorg_drops_stale_tip_entries() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("cache.sqlite3");
        let c = SqliteCache::open(&path).await.unwrap();
        let h1 = [1u8; 32];
        let h2 = [2u8; 32];
        c.put(&k("simulate", b"a"), &CacheScope::Tip(h1), b"old".to_vec(), None)
            .await
            .unwrap();
        c.put(&k("simulate", b"b"), &CacheScope::Tip(h2), b"new".to_vec(), None)
            .await
            .unwrap();
        c.on_reorg(h2).await.unwrap();
        assert!(c
            .get(&k("simulate", b"a"), &CacheScope::Tip(h1))
            .await
            .unwrap()
            .is_none());
        assert_eq!(
            c.get(&k("simulate", b"b"), &CacheScope::Tip(h2))
                .await
                .unwrap()
                .as_deref(),
            Some(b"new".as_ref())
        );
    }

    #[tokio::test]
    async fn ttl_is_respected_on_get() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("cache.sqlite3");
        let c = SqliteCache::open(&path).await.unwrap();
        let key = k("getinventory", b"x");
        // Backdate via direct SQL — sqlx doesn't expose insert-time
        // control through the trait, so we test by inserting with a 0-second
        // TTL and an already-elapsed timestamp via raw query.
        c.put(&key, &CacheScope::Tip([0u8; 32]), b"v".to_vec(), Some(Duration::from_secs(60)))
            .await
            .unwrap();
        // Hit should succeed immediately.
        assert!(c.get(&key, &CacheScope::Tip([0u8; 32])).await.unwrap().is_some());
        // Force expiry by rewriting `inserted` 100 seconds in the past.
        sqlx::query("UPDATE cache_entries SET inserted = inserted - 100")
            .execute(&c.pool)
            .await
            .unwrap();
        assert!(c
            .get(&key, &CacheScope::Tip([0u8; 32]))
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn purge_older_than_works() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("cache.sqlite3");
        let c = SqliteCache::open(&path).await.unwrap();
        c.put(&k("a", b"x"), &CacheScope::Immutable, b"x".to_vec(), None)
            .await
            .unwrap();
        // Backdate.
        sqlx::query("UPDATE cache_entries SET inserted = inserted - 7200")
            .execute(&c.pool)
            .await
            .unwrap();
        let dropped = c.purge_older_than(Duration::from_secs(3600)).await.unwrap();
        assert_eq!(dropped, 1);
    }

    #[test]
    fn default_path_respects_env_var() {
        std::env::set_var("ALKANES_CACHE_DIR", "/tmp/alktest");
        let p = default_cache_path();
        assert_eq!(p, PathBuf::from("/tmp/alktest/cache.sqlite3"));
        std::env::remove_var("ALKANES_CACHE_DIR");
    }
}
