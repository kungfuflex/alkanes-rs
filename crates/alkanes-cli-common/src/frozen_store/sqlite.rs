//! Sqlite-backed [`FrozenStore`] — the native CLI driver.
//!
//! One row per frozen outpoint at `~/.alkanes/frozen.sqlite3`. Uses
//! sqlx-sqlite, matching [`crate::cache::sqlite`] so the two backends share
//! a `libsqlite3-sys` version.
//!
//! Available behind the `frozen-sqlite` feature.

use core::time::Duration;
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use anyhow::{Context, Result};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Pool, Sqlite};

use super::{normalize_outpoint, now_secs, FrozenRecord, FrozenStore};

/// DDL applied at open time. One statement per entry — sqlx's `query()`
/// takes a single statement at a time.
const SCHEMA_STATEMENTS: &[&str] = &[
    "CREATE TABLE IF NOT EXISTS frozen_utxos ( \
        outpoint  TEXT PRIMARY KEY, \
        reason    TEXT, \
        frozen_at INTEGER NOT NULL \
     )",
];

/// Persistent frozen set. Cheap to clone (`Arc<Pool>` internally).
#[derive(Clone)]
pub struct SqliteFrozenStore {
    pool: Pool<Sqlite>,
}

impl core::fmt::Debug for SqliteFrozenStore {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SqliteFrozenStore").finish()
    }
}

impl SqliteFrozenStore {
    /// Open the store at `path`, creating it (and parent dirs) if missing.
    pub async fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("creating frozen-store directory {}", parent.display())
            })?;
        }
        let url = format!("sqlite://{}?mode=rwc", path.display());
        let opts: SqliteConnectOptions = url
            .parse()
            .with_context(|| format!("parsing sqlite url for {}", path.display()))?;
        let opts = opts
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .synchronous(sqlx::sqlite::SqliteSynchronous::Normal)
            .busy_timeout(Duration::from_secs(5));
        // One connection, deliberately. The frozen set is tiny and
        // low-traffic — one `list()` per UTXO listing, one row per
        // freeze/unfreeze — and SQLite serializes writers regardless, so a
        // wider pool buys nothing and only multiplies concurrent opens.
        // Opens are the expensive part here: on a loaded box a single pool
        // open costs seconds, and sqlx's default 30s acquire timeout is
        // genuinely reachable when many stores are opened at once.
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(opts)
            .await
            .with_context(|| format!("opening frozen store at {}", path.display()))?;
        for stmt in SCHEMA_STATEMENTS {
            sqlx::query(stmt)
                .execute(&pool)
                .await
                .context("applying frozen-store schema")?;
        }
        Ok(Self { pool })
    }

    /// Open at [`default_frozen_db_path`].
    pub async fn open_default() -> Result<Self> {
        Self::open(default_frozen_db_path()).await
    }
}

/// Where the frozen set lives: `$ALKANES_FROZEN_DB`, else
/// `$ALKANES_DATA_DIR/frozen.sqlite3`, else `~/.alkanes/frozen.sqlite3`,
/// else `./.alkanes-frozen.sqlite3`.
///
/// Deliberately NOT keyed off `ALKANES_CACHE_DIR`, which locates
/// `cache.sqlite3`. A cache is disposable by definition and users delete it
/// to reclaim disk; the frozen set is a safety control, and losing it
/// re-arms coins the operator explicitly took off the table. Sharing the
/// directory would make "clear the cache" a way to lose an inscription.
pub fn default_frozen_db_path() -> PathBuf {
    if let Ok(path) = std::env::var("ALKANES_FROZEN_DB") {
        return PathBuf::from(path);
    }
    if let Ok(dir) = std::env::var("ALKANES_DATA_DIR") {
        return PathBuf::from(dir).join("frozen.sqlite3");
    }
    if let Some(home) = dirs::home_dir() {
        return home.join(".alkanes").join("frozen.sqlite3");
    }
    PathBuf::from(".alkanes-frozen.sqlite3")
}

fn row_to_record(outpoint: String, reason: Option<String>, frozen_at: i64) -> FrozenRecord {
    FrozenRecord {
        outpoint,
        reason,
        frozen_at: frozen_at.max(0) as u64,
    }
}

#[async_trait(?Send)]
impl FrozenStore for SqliteFrozenStore {
    async fn freeze(&self, outpoint: &str, reason: Option<&str>) -> Result<()> {
        let key = normalize_outpoint(outpoint)?;
        // Upsert: re-freezing with a new reason replaces the note and keeps
        // one row, matching the memory driver.
        sqlx::query(
            "INSERT INTO frozen_utxos (outpoint, reason, frozen_at) VALUES (?, ?, ?) \
             ON CONFLICT(outpoint) DO UPDATE SET reason = excluded.reason, \
             frozen_at = excluded.frozen_at",
        )
        .bind(&key)
        .bind(reason)
        .bind(now_secs() as i64)
        .execute(&self.pool)
        .await
        .with_context(|| format!("freezing {key}"))?;
        Ok(())
    }

    async fn unfreeze(&self, outpoint: &str) -> Result<bool> {
        let key = normalize_outpoint(outpoint)?;
        let r = sqlx::query("DELETE FROM frozen_utxos WHERE outpoint = ?")
            .bind(&key)
            .execute(&self.pool)
            .await
            .with_context(|| format!("unfreezing {key}"))?;
        Ok(r.rows_affected() > 0)
    }

    async fn get(&self, outpoint: &str) -> Result<Option<FrozenRecord>> {
        let key = normalize_outpoint(outpoint)?;
        let row: Option<(String, Option<String>, i64)> = sqlx::query_as(
            "SELECT outpoint, reason, frozen_at FROM frozen_utxos WHERE outpoint = ?",
        )
        .bind(&key)
        .fetch_optional(&self.pool)
        .await
        .with_context(|| format!("reading frozen record for {key}"))?;
        Ok(row.map(|(o, r, t)| row_to_record(o, r, t)))
    }

    async fn list(&self) -> Result<Vec<FrozenRecord>> {
        let rows: Vec<(String, Option<String>, i64)> =
            sqlx::query_as("SELECT outpoint, reason, frozen_at FROM frozen_utxos")
                .fetch_all(&self.pool)
                .await
                .context("listing frozen UTXOs")?;
        Ok(rows
            .into_iter()
            .map(|(o, r, t)| row_to_record(o, r, t))
            .collect())
    }

    async fn clear(&self) -> Result<()> {
        sqlx::query("DELETE FROM frozen_utxos")
            .execute(&self.pool)
            .await
            .context("clearing frozen UTXOs")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: &str = "c5520bb64d1a742a6bd62999267f683e1f0756481220ff2155d2be841a3d7b92:0";
    const B: &str = "c5520bb64d1a742a6bd62999267f683e1f0756481220ff2155d2be841a3d7b92:1";

    /// Each test gets its own file in a temp dir, so no test touches the
    /// operator's real `~/.alkanes/frozen.sqlite3`.
    async fn temp_store() -> (tempfile::TempDir, SqliteFrozenStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = SqliteFrozenStore::open(dir.path().join("frozen.sqlite3"))
            .await
            .unwrap();
        (dir, store)
    }

    #[tokio::test]
    async fn a_fresh_store_is_empty() {
        let (_d, store) = temp_store().await;
        assert!(store.list().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn freeze_then_get_round_trips_the_reason() {
        let (_d, store) = temp_store().await;
        store.freeze(A, Some("inscription")).await.unwrap();

        let rec = store.get(A).await.unwrap().expect("frozen");
        assert_eq!(rec.outpoint, A);
        assert_eq!(rec.reason.as_deref(), Some("inscription"));
    }

    #[tokio::test]
    async fn the_freeze_survives_reopening_the_file() {
        // This is the property the whole driver exists for: a freeze must
        // outlive the process, or it protects nothing across CLI runs.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("frozen.sqlite3");
        {
            let store = SqliteFrozenStore::open(&path).await.unwrap();
            store.freeze(A, Some("persisted")).await.unwrap();
        }
        let reopened = SqliteFrozenStore::open(&path).await.unwrap();
        let rec = reopened.get(A).await.unwrap().expect("still frozen");
        assert_eq!(rec.reason.as_deref(), Some("persisted"));
    }

    #[tokio::test]
    async fn refreezing_updates_the_reason_without_duplicating() {
        let (_d, store) = temp_store().await;
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
        let (_d, store) = temp_store().await;
        store.freeze(A, None).await.unwrap();

        assert!(store.unfreeze(A).await.unwrap());
        assert!(!store.unfreeze(A).await.unwrap());
    }

    #[tokio::test]
    async fn keys_are_normalized_so_the_same_coin_matches_either_spelling() {
        let (_d, store) = temp_store().await;
        store.freeze(&A.to_uppercase(), None).await.unwrap();
        assert!(store.get(A).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn a_malformed_outpoint_is_rejected_rather_than_stored() {
        let (_d, store) = temp_store().await;
        assert!(store.freeze("nope", None).await.is_err());
        assert!(store.list().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn list_returns_every_entry() {
        let (_d, store) = temp_store().await;
        store.freeze(A, Some("a")).await.unwrap();
        store.freeze(B, Some("b")).await.unwrap();

        let mut got: Vec<String> = store
            .list()
            .await
            .unwrap()
            .into_iter()
            .map(|r| r.outpoint)
            .collect();
        got.sort();
        assert_eq!(got, vec![A.to_string(), B.to_string()]);
    }

    #[tokio::test]
    async fn clear_empties_the_store() {
        let (_d, store) = temp_store().await;
        store.freeze(A, None).await.unwrap();
        store.clear().await.unwrap();
        assert!(store.list().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn the_default_path_is_not_under_the_cache_dir() {
        // Clearing the cache must never unfreeze a UTXO. Pinning this keeps
        // a future refactor from "tidying" the two stores into one dir.
        let cache = crate::cache::sqlite::default_cache_path();
        let frozen = default_frozen_db_path();
        assert_ne!(cache, frozen);
        assert_ne!(cache.file_name(), frozen.file_name());
    }
}
