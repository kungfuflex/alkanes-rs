use anyhow::Result;
use sqlx::PgPool;

/// Represents the last fully indexed block position
#[derive(Debug, Clone)]
pub struct Position {
    pub height: u64,
    pub block_hash: String,
}

pub struct ProgressStore {
    pool: PgPool,
}

impl ProgressStore {
    pub fn new(pool: PgPool) -> Self { Self { pool } }

    /// Get the current position (last fully indexed block)
    /// Returns None if no blocks have been indexed yet
    pub async fn get_position(&self) -> Result<Option<Position>> {
        let row: Option<(i64, String)> = sqlx::query_as(
            "SELECT height, block_hash FROM indexer_position WHERE id = 1"
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|(h, bh)| Position { height: h as u64, block_hash: bh }))
    }

    /// Update the position after successfully indexing a block
    /// This should only be called after the block is fully indexed
    pub async fn set_position(&self, height: u64, block_hash: &str) -> Result<()> {
        sqlx::query(
            "INSERT INTO indexer_position (id, height, block_hash)
             VALUES (1, $1, $2)
             ON CONFLICT (id) DO UPDATE SET height = $1, block_hash = $2"
        )
        .bind(height as i64)
        .bind(block_hash)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

/// Create the indexer_position table if it doesn't exist
/// This table has exactly one row that tracks our sync position
pub async fn ensure_position_table(pool: &PgPool) -> Result<()> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS indexer_position (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            height BIGINT NOT NULL,
            block_hash TEXT NOT NULL
        )"
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// Migrate from old kv_store to new position table if needed
pub async fn migrate_from_kv_store(pool: &PgPool) -> Result<()> {
    // Check if we already have a position
    let existing: Option<(i32,)> = sqlx::query_as(
        "SELECT id FROM indexer_position WHERE id = 1"
    )
    .fetch_optional(pool)
    .await?;

    if existing.is_some() {
        return Ok(()); // Already migrated
    }

    // Check if old kv_store exists and has a value
    let old_height: Option<(String,)> = sqlx::query_as(
        "SELECT value FROM kv_store WHERE key = 'last_processed_height'"
    )
    .fetch_optional(pool)
    .await
    .unwrap_or(None);

    if let Some((height_str,)) = old_height {
        if let Ok(height) = height_str.parse::<i64>() {
            // We need to fetch the block hash for this height
            // For now, insert with empty hash - it will be updated on next sync
            tracing::warn!(
                height,
                "Migrating from kv_store - block_hash will be set on next sync"
            );
            sqlx::query(
                "INSERT INTO indexer_position (id, height, block_hash) VALUES (1, $1, '')"
            )
            .bind(height)
            .execute(pool)
            .await?;
        }
    }

    Ok(())
}


